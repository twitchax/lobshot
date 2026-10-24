//! lobshot's game rules. Pure and deterministic: no Bevy, no I/O, no clocks. The server runs it
//! authoritatively, the client runs the same code to replay events and predict movement, and the
//! lab runs it headless. See `DESIGN.md` ("Game model").

pub mod ballistics;
pub mod base;
pub mod body;
pub mod bot;
pub mod damage;
pub mod drops;
pub mod game;
pub mod hash;
pub mod items;
pub mod log;
pub mod map;
pub mod mines;
pub mod movement;
pub mod referee;
pub mod score;
mod shots;
pub mod terrain;
pub mod tuning;
pub mod turn;
pub mod units;
pub mod weather;
