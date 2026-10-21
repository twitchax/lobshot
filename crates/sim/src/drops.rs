//! The dead players' slot machine (WP06): in Solo, a dead player spins it, and what it lands on
//! falls onto the map some turns later. The server rolls the result (`Roll::Drop`); how each drop
//! behaves lands with the slot-machine rules.

use serde::{Deserialize, Serialize};

/// What the slot machine can drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DropKind {
    /// Blows up where it lands; the wind carries it as it falls.
    Dynamite,
    /// Lands hard on whatever is under it.
    Hammer,
    /// A bolt straight down, which the wind can't move.
    Bolt,
    /// A treasure chest.
    Chest,
    /// The wind dies.
    Calm,
    /// The wind turns around.
    Reverse,
}

impl DropKind {
    /// Every drop; the server rolls one uniformly.
    pub const ALL: [Self; 6] = [Self::Dynamite, Self::Hammer, Self::Bolt, Self::Chest, Self::Calm, Self::Reverse];
}

/// A rolled drop: what, and the column it falls down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drop {
    pub kind: DropKind,
    pub x: i32,
}
