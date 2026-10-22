//! SearchBot's hands and eyes, by difficulty. Its search is exact, so difficulty is error added
//! around it (`DESIGN.md`, "Bots and the lab"): it misreads the wind before it searches, as
//! people misjudge the wind more than the angle, then its hands miss the aim and power it found.
//! No source describes GunBound's bots, so every number is a pick, set so a 4v4 of Normal bots
//! lasts about as long as a GunBound 4v4 (`lobshot_lab`'s `tool_length`).

use super::{Tuned, picked};
use crate::units::Centis;

/// How far off a bot may be, each error uniform within its spread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noise {
    /// Power, in steps of 0.05 bar.
    pub power_steps: Tuned<i32>,
    /// Aim, in degrees.
    pub aim_degrees: Tuned<i32>,
    /// The wind's strength as it reads it, in wind units.
    pub wind_strength: Tuned<i32>,
    /// The wind's direction as it reads it, in degrees.
    pub wind_degrees: Tuned<i32>,
}

/// Close to perfect: what the search found, in the wind as it is.
pub const HARD: Noise = Noise {
    power_steps: picked(0, "Hard is the search's own answer."),
    aim_degrees: picked(0, "Hard is the search's own answer."),
    wind_strength: picked(0, "Hard reads the wind exactly."),
    wind_degrees: picked(0, "Hard reads the wind exactly."),
};

/// A friend who knows the game. The spreads are as large as a person's plausibly are: smaller ones
/// leave a 4v4 shorter still, and these stop lengthening it much (`lobshot_lab`'s `tool_length`), since on a
/// crowded map most misses still hurt someone.
pub const NORMAL: Noise = Noise {
    power_steps: picked(5, "A quarter bar: slice shots slip, and drag shots jitter (GF-tiro, CR-using)."),
    aim_degrees: picked(3, "Angles are read off the HUD in whole degrees, and a few slip."),
    wind_strength: picked(8, "People misjudge the wind more than the angle: about a third of its range."),
    wind_degrees: picked(90, "A quarter turn either way: the dial's arrow is easy to misjudge for its effect."),
};

/// Someone still learning: about 1.6 times Normal's error.
pub const EASY: Noise = Noise {
    power_steps: picked(8, "Clearly worse than Normal, still in the neighborhood."),
    aim_degrees: picked(5, "Clearly worse than Normal."),
    wind_strength: picked(13, "Half the wind's range."),
    wind_degrees: picked(150, "Barely reads the dial."),
};

/// How long a bot's turn takes, as every host stamps it: the server waits this long before a bot
/// plays (`LOBSHOT_BOT_DELAY_MS`'s default), and practice and the lab charge it. Turn time adds to
/// delay, so it decides when a bot moves again.
pub const THINK: Tuned<Centis> = picked(Centis(120), "Long enough to read as a turn: a person's quick one.");
