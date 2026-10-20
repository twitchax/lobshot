//! One WebSocket. Decodes what the client sends (binary frames: a version byte then postcard;
//! text frames: JSON), rate-limits it, forwards it to the room, and writes back whatever the room
//! sends, in the form the client last used (`DESIGN.md`, "Protocol"). A ping every 20 s keeps
//! proxies from dropping a quiet socket (a player lining up a shot sends nothing); a socket silent
//! for 60 s, pongs included, is gone.

use std::time::{Duration, Instant};

use axum::{
    body::Bytes,
    extract::ws::{Message, WebSocket},
};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::sync::mpsc;

use lobshot_protocol::{ClientMsg, ServerMsg};
use lobshot_sim::base::Res;

use crate::{
    rate::{Bucket, Rate, TOO_FAST},
    room::{
        ConnId,
        task::{Command, OUTBOX, RoomHandle},
    },
};

/// How often the server pings.
pub const HEARTBEAT: Duration = Duration::from_secs(20);

/// How long a socket may stay silent before it's closed.
pub const SILENCE: Duration = Duration::from_secs(60);

/// The largest frame a client may send. Client messages are tiny (a chat line is at most 200
/// characters), so anything bigger is a mistake or an attack.
pub const MAX_FRAME: usize = 16 * 1024;

/// How a client encodes its messages; the server answers the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Binary,
    Json,
}

/// Serves one socket on `room` until either side closes it.
pub async fn serve(socket: WebSocket, room: RoomHandle, conn: ConnId) {
    let (outbox, mut inbox) = mpsc::channel(OUTBOX);
    if !room.send(Command::Open { conn, outbox }).await {
        return;
    }
    let (mut sink, mut stream) = socket.split();
    let mut form = Form::Binary;
    let mut bucket = Bucket::new(Rate::MESSAGES, Instant::now());
    let mut heard = Instant::now();
    let mut heartbeat = tokio::time::interval_at((Instant::now() + HEARTBEAT).into(), HEARTBEAT);

    loop {
        tokio::select! {
            frame = stream.next() => {
                let Some(Ok(frame)) = frame else { break };
                heard = Instant::now();
                let Some((sent_as, decoded)) = read(&frame) else {
                    if matches!(frame, Message::Close(_)) {
                        break;
                    }
                    continue;
                };
                form = sent_as;
                let refusal = match decoded {
                    _ if !bucket.take(heard) => TOO_FAST.to_owned(),
                    Err(error) => format!("{error:#}"),
                    Ok(msg) => {
                        if !room.send(Command::Message(conn, msg)).await {
                            break;
                        }
                        continue;
                    }
                };
                if write(&mut sink, form, &ServerMsg::Rejected { reason: refusal }).await.is_err() {
                    break;
                }
            }
            msg = inbox.recv() => {
                // The room closed this socket (a kick, a newer socket for the player, the room
                // ending) once its last messages were sent.
                let Some(msg) = msg else { break };
                if write(&mut sink, form, &msg).await.is_err() {
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if heard.elapsed() >= SILENCE || sink.send(Message::Ping(Bytes::new())).await.is_err() {
                    break;
                }
            }
        }
    }
    // Best effort: the peer may already be gone.
    let _ = sink.send(Message::Close(None)).await;
    room.send(Command::Closed(conn)).await;
}

/// A client message from a data frame, and the form it came in. `None` for control frames.
fn read(frame: &Message) -> Option<(Form, Res<ClientMsg>)> {
    match frame {
        Message::Binary(bytes) => Some((Form::Binary, lobshot_protocol::decode(bytes))),
        Message::Text(text) => Some((Form::Json, lobshot_protocol::from_json(text.as_str()))),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) => None,
    }
}

fn encode(form: Form, msg: &ServerMsg) -> Res<Message> {
    Ok(match form {
        Form::Binary => Message::Binary(lobshot_protocol::encode(msg)?.into()),
        Form::Json => Message::Text(lobshot_protocol::to_json(msg)?.into()),
    })
}

async fn write<S>(sink: &mut S, form: Form, msg: &ServerMsg) -> Res<()>
where
    S: futures_util::Sink<Message, Error = axum::Error> + Unpin,
{
    sink.send(encode(form, msg)?).await?;
    Ok(())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn reads_both_forms_and_skips_control_frames() {
        let msg = ClientMsg::Ready(true);
        let binary = Message::Binary(lobshot_protocol::encode(&msg).unwrap().into());
        let (form, decoded) = read(&binary).unwrap();
        assert_eq!((form, decoded.unwrap()), (Form::Binary, msg.clone()));

        let text = Message::Text(lobshot_protocol::to_json(&msg).unwrap().into());
        let (form, decoded) = read(&text).unwrap();
        assert_eq!((form, decoded.unwrap()), (Form::Json, msg));

        assert!(read(&Message::Ping(Bytes::new())).is_none());
        assert!(read(&Message::Close(None)).is_none());
        let (form, garbage) = read(&Message::Text("{".into())).unwrap();
        assert!(form == Form::Json && garbage.is_err());
    }

    #[test]
    fn answers_in_the_form_asked() {
        let msg = ServerMsg::Rejected { reason: TOO_FAST.into() };
        let Message::Binary(bytes) = encode(Form::Binary, &msg).unwrap() else { panic!() };
        assert_eq!(lobshot_protocol::decode::<ServerMsg>(&bytes).unwrap(), msg);
        let Message::Text(text) = encode(Form::Json, &msg).unwrap() else { panic!() };
        assert_eq!(lobshot_protocol::from_json::<ServerMsg>(text.as_str()).unwrap(), msg);
    }
}
