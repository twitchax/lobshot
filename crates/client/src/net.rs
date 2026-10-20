//! The room's WebSocket. Server messages become Bevy `Inbound` messages; anything can send with
//! `Socket::send`. Binary frames (a version byte, then postcard) both ways.

use bevy::prelude::*;
use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};
use lobshot_protocol::{ClientMsg, RoomCode, ServerMsg};

use crate::web;

/// A server message, as a Bevy message.
#[derive(Message, Debug, Clone)]
pub struct Inbound(pub ServerMsg);

/// The connection state, for the screens to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Link {
    #[default]
    Closed,
    Connecting,
    Open,
}

/// The socket. Not `Send` on the web (it wraps a JS object), so it lives as a non-send resource.
#[derive(Default)]
pub struct Socket {
    pair: Option<(WsSender, WsReceiver)>,
    link: Link,
    /// Sent once the socket opens.
    hello: Option<ClientMsg>,
}

impl Socket {
    /// Connects to a room, sending `hello` as soon as the socket opens.
    pub fn connect(&mut self, code: &RoomCode, hello: ClientMsg) {
        match ewebsock::connect(web::socket_url(code), ewebsock::Options::default()) {
            Ok(pair) => {
                self.pair = Some(pair);
                self.link = Link::Connecting;
                self.hello = Some(hello);
            }
            Err(error) => {
                error!("couldn't open the room's socket: {error}");
                self.link = Link::Closed;
            }
        }
    }

    #[must_use]
    pub fn link(&self) -> Link {
        self.link
    }

    /// Sends a message if the socket is open (silently dropped otherwise: the screens show the
    /// link state, and reconnecting resends the hello).
    pub fn send(&mut self, message: &ClientMsg) {
        let Some((sender, _)) = self.pair.as_mut() else { return };
        if self.link != Link::Open {
            return;
        }
        match lobshot_protocol::encode(message) {
            Ok(bytes) => sender.send(WsMessage::Binary(bytes)),
            Err(error) => error!("couldn't encode {message:?}: {error:#}"),
        }
    }
}

/// Drains the socket into `Inbound` messages.
pub fn poll(mut socket: NonSendMut<Socket>, mut inbound: MessageWriter<Inbound>) {
    let mut events = Vec::new();
    if let Some((_, receiver)) = socket.pair.as_ref() {
        while let Some(event) = receiver.try_recv() {
            events.push(event);
        }
    }
    for event in events {
        match event {
            WsEvent::Opened => {
                socket.link = Link::Open;
                if let Some(hello) = socket.hello.clone() {
                    socket.send(&hello);
                }
            }
            WsEvent::Message(WsMessage::Binary(bytes)) => match lobshot_protocol::decode::<ServerMsg>(&bytes) {
                Ok(message) => {
                    inbound.write(Inbound(message));
                }
                Err(error) => warn!("dropped a frame: {error:#}"),
            },
            WsEvent::Message(WsMessage::Text(text)) => match lobshot_protocol::from_json::<ServerMsg>(&text) {
                Ok(message) => {
                    inbound.write(Inbound(message));
                }
                Err(error) => warn!("dropped a frame: {error:#}"),
            },
            WsEvent::Message(_) => {}
            WsEvent::Error(error) => {
                warn!("socket error: {error}");
                socket.link = Link::Closed;
            }
            WsEvent::Closed => socket.link = Link::Closed,
        }
    }
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(Socket::default()).add_message::<Inbound>().add_systems(PreUpdate, poll);
    }
}
