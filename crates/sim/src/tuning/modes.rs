//! The modes' numbers (PRD-0007; mechanics.md section 7). Score's sources, and every pick behind
//! these, are in `docs/research/modes-score.md`; Tag's in `docs/research/modes-tag.md`.

use super::{Tuned, cited, picked};

/// A Score team's lives over its players: "one more than the number of players".
pub const SCORE_EXTRA_LIVES: Tuned<i32> = cited(1, &["SW-modes", "GF-tiro", "GF-vgg", "GF-kurai"]);

/// What a death costs its team in Score: a life, however it died (blasted or bunged alike).
pub const SCORE_DEATH_COST: Tuned<i32> = cited(1, &["SW-modes", "GF-tiro", "GF-vgg", "NAMU"]);

/// Turns from a dead player's pick to its drop in Score, counted from the turn after the one it
/// picked in (SW-modes); GF-vgg and GF-kurai count from the pick itself (CONTRADICTION).
pub const SCORE_RESPAWN_TURNS: Tuned<i64> = cited(4, &["SW-modes", "GF-vgg", "GF-kurai", "GF-super", "NAMU"]);

/// How hard the wind pushes a mobile dropping back in, against a shot's 1.0.
pub const RESPAWN_WIND: Tuned<f64> = picked(
    0.25,
    "The sources say only \"to a small degree\" (SW-modes): a quarter drifts a drop about 30 px across a \
     map's height in the strongest wind.",
);

/// The share of its max HP (and shield) a mobile comes back with, in percent.
pub const RESPAWN_HP_PERCENT: Tuned<u32> = picked(100, "The sources are silent; a team's life is a whole mobile.");

/// Turns others take, once a mobile has dropped back in, before its own: "it will be your turn
/// exactly 2 turns after you drop in" (GF-super), so one.
pub const RESPAWN_TURNS_BEFORE: Tuned<usize> = cited(1, &["GF-super"]);

/// The share of its max HP a Tag player's second mobile starts with, in percent: "The first one has
/// full HP, and the second one has half HP" (SW-modes). Its shield starts full: the sources speak of
/// HP alone, and a shield regenerates in play anyway. DragonBound instead brings the backup in at
/// half HP once the first falls (DB-lobby).
pub const TAG_SECOND_HP_PERCENT: Tuned<u32> = cited(50, &["SW-modes", "GF-vgg", "GF-kurai", "GF-super", "NAMU"]);
