//! The lobshot server. One binary serves the client bundle, the WebSocket and the rooms, so
//! everything shares one origin. See `DESIGN.md` ("Architecture").

pub mod config;
pub mod connection;
pub mod rate;
pub mod room;
pub mod rooms;
pub mod server;
pub mod tests;
