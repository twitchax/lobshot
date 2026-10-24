//! A WebSocket client for the server's integration and e2e tests: speaks the protocol in either
//! form, and plays a game to its end the way a real client does, replaying the log and checking
//! the server's hash after every event (`lobshot_server::tests::Replica`).

// Each test binary compiles this module and uses a different part of it.
#![allow(dead_code)]

use std::{net::SocketAddr, time::Duration};

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use lobshot_assets::maps::Maps;
use lobshot_protocol::{Action, ClientMsg, MemberId, RoomView, ServerMsg, Stage, Token};
use lobshot_server::tests::Replica;
use lobshot_sim::{
    bot::Level,
    game::{Game, Phase},
};

/// How long any one message may take to arrive before the test fails instead of hanging.
const PATIENCE: Duration = Duration::from_secs(30);

pub struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Speaks JSON text frames instead of postcard binary ones.
    pub json: bool,
}

impl Client {
    pub async fn connect(addr: SocketAddr, code: &str, json: bool) -> Self {
        let (ws, _) = connect_async(format!("ws://{addr}/ws/{code}")).await.expect("the socket opens");
        Self { ws, json }
    }

    pub async fn send(&mut self, msg: &ClientMsg) {
        let frame = if self.json {
            Message::Text(lobshot_protocol::to_json(msg).unwrap().into())
        } else {
            Message::Binary(lobshot_protocol::encode(msg).unwrap().into())
        };
        self.send_frame(frame).await;
    }

    /// Sends a frame as is, for messages no `ClientMsg` can hold.
    pub async fn send_frame(&mut self, frame: Message) {
        self.ws.send(frame).await.expect("the socket takes the message");
    }

    /// The next frame that isn't a ping or pong, or `None` once the server closed the socket.
    async fn frame(&mut self) -> Option<Message> {
        loop {
            let frame = tokio::time::timeout(PATIENCE, self.ws.next()).await.expect("the server answers in time");
            match frame {
                None | Some(Ok(Message::Close(_)) | Err(_)) => return None,
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(frame)) => return Some(frame),
            }
        }
    }

    /// The next message, which must come in the form this client last spoke.
    pub async fn recv(&mut self) -> ServerMsg {
        match self.frame().await.expect("the socket stays open") {
            Message::Binary(bytes) => {
                assert!(!self.json, "a binary answer to a JSON client");
                lobshot_protocol::decode(&bytes).unwrap()
            }
            Message::Text(text) => {
                assert!(self.json, "a JSON answer to a binary client");
                lobshot_protocol::from_json(text.as_str()).unwrap()
            }
            other => panic!("an unexpected frame: {other:?}"),
        }
    }

    /// Skips messages until one `want` accepts.
    pub async fn recv_until(&mut self, mut want: impl FnMut(&ServerMsg) -> bool) -> ServerMsg {
        loop {
            let msg = self.recv().await;
            if want(&msg) {
                return msg;
            }
        }
    }

    /// Every message left before the server closes the socket.
    pub async fn until_closed(&mut self) -> Vec<ServerMsg> {
        let mut messages = Vec::new();
        while let Some(frame) = self.frame().await {
            if let Message::Binary(bytes) = frame {
                messages.push(lobshot_protocol::decode(&bytes).unwrap());
            } else if let Message::Text(text) = frame {
                messages.push(lobshot_protocol::from_json(text.as_str()).unwrap());
            }
        }
        messages
    }

    /// Says hello and reads the answer: `Welcome`, then the room.
    pub async fn hello(&mut self, name: &str, token: Option<Token>) -> (MemberId, Token, RoomView) {
        self.send(&ClientMsg::Hello {
            name: name.into(),
            token,
            rider: "pip".into(),
        })
        .await;
        let ServerMsg::Welcome { you, token } = self.recv().await else { panic!("{name} wasn't welcomed") };
        let ServerMsg::Room(view) = self.recv().await else { panic!("{name} didn't get the room") };
        (you, token, view)
    }
}

/// Plays as `me` until the game ends, from the next message on: replays everything the server
/// sends (the replica checks the hash after every event) and plays `me`'s turns with SearchBot,
/// and its drop points once dead in Score.
/// `last_view` is the room as the client last saw it, if a game was already on; a member without a
/// seat just follows. Returns the replica and the room as it returned to the lobby.
pub async fn play_out(client: &mut Client, me: MemberId, last_view: Option<&RoomView>, mut seed: u64) -> (Replica, RoomView) {
    let mut replica = Replica::default();
    let mut seat = last_view.and_then(|view| seat_of(view, me));
    loop {
        let msg = client.recv().await;
        replica.absorb(&msg).unwrap();
        if let ServerMsg::Room(view) = &msg {
            match &view.stage {
                Stage::Playing { .. } => seat = seat_of(view, me),
                // A lobby view still queued from before the game started doesn't end it.
                Stage::Lobby if replica.game().is_some() => return (replica, view.clone()),
                Stage::Lobby => {}
            }
        }
        if let Some(seat) = seat
            && let Some(actions) = replica.plan(seat, Level::Hard, seed)
        {
            seed += 1;
            for action in actions {
                client.send(&ClientMsg::Act(action)).await;
            }
        }
        // Dead in Score, it drops back in where SearchBot would.
        if let Some(pick) = seat.and_then(|seat| replica.respawn(seat)) {
            client.send(&ClientMsg::Act(pick)).await;
        }
    }
}

fn seat_of(view: &RoomView, me: MemberId) -> Option<u8> {
    let Stage::Playing { seats } = &view.stage else { return None };
    seats.iter().position(|&id| id == me).map(|seat| u8::try_from(seat).unwrap())
}

/// A client's copy of a game on a drawn map, rebuilt from what the server sends: the log replayed
/// on `maps` (the drawn maps the server serves), then each event. `Replica` plays synthetic maps
/// only, and checks hashes besides.
pub struct Follower {
    me: MemberId,
    maps: Maps,
    pub game: Option<Game>,
    /// `me`'s seat, once the game is on.
    pub seat: Option<u8>,
}

impl Follower {
    #[must_use]
    pub fn new(me: MemberId, maps: Maps) -> Self {
        Self { me, maps, game: None, seat: None }
    }

    /// Takes in one server message.
    pub fn absorb(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Room(view) => {
                if let Stage::Playing { seats } = view.stage {
                    self.seat = seats.iter().position(|&id| id == self.me).and_then(|seat| u8::try_from(seat).ok());
                }
            }
            ServerMsg::Game { log } => self.game = Some(log.replay(&self.maps).unwrap()),
            ServerMsg::Event { event, .. } => {
                if let Some(game) = self.game.as_mut() {
                    game.apply(event).unwrap();
                }
            }
            _ => {}
        }
    }

    /// The game and `me`'s seat, once it is `me`'s turn to act (no roll owed).
    #[must_use]
    pub fn my_turn(&self) -> Option<(&Game, u8)> {
        let (game, seat) = (self.game.as_ref()?, self.seat?);
        (game.phase() == (Phase::Turn { seat }) && game.pending_roll().is_none()).then_some((game, seat))
    }
}

/// Plays `client` (`me`, on a game on `maps`) by passing each of its turns `after` it comes up,
/// until the socket closes or goes quiet.
pub async fn pass_out(mut client: Client, me: MemberId, maps: Maps, after: Duration) {
    let mut follower = Follower::new(me, maps);
    let mut passed = None;
    loop {
        follower.absorb(client.recv().await);
        let Some((game, _)) = follower.my_turn() else { continue };
        if passed != Some(game.turns()) {
            passed = Some(game.turns());
            tokio::time::sleep(after).await;
            client.send(&ClientMsg::Act(Action::Pass)).await;
        }
    }
}
