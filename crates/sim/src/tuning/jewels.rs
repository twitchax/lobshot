//! Jewel mode's numbers (PRD-0007 T-003; mechanics.md section 7). Their sources, and every pick
//! behind these, are in `docs/research/modes-jewel.md`.

use super::{Tuned, cited, picked};

/// What each jewel is worth to the team that takes it, by `JewelKind` (`sim::jewels`): blue 5,
/// yellow 10, red 25, and black -5.
pub const POINTS: Tuned<[i32; 4]> = cited([5, 10, 25, -5], &["SW-modes", "GF-cyber", "NAMU", "GF-vgg"]);

/// The damage each must take in one turn to be taken, by `JewelKind`: "100 damage for 5 or -5
/// points, 200 damage for 10 points, and 300 damage for 25 points" (SW-modes). GF-cyber gives
/// 200, 300, 400 and 200 (CONTRADICTION; `SW-modes` first).
pub const NEEDS: Tuned<[i32; 4]> = cited([100, 200, 300, 100], &["SW-modes"]);

/// The points a team plays to, whatever its size: "The first team to reach 100 points or exceed
/// 100 points wins" (SW-modes).
pub const TARGET_POINTS: Tuned<i32> = cited(100, &["SW-modes", "WP06", "GF-kurai", "GF-cyber", "GF-vgg", "GF-super", "NAMU"]);

/// Turns from a bunged player's pick to its drop, counted as Score's are: "They will take two
/// turns to respawn" (SW-modes, GF-vgg).
pub const RESPAWN_TURNS: Tuned<i64> = cited(2, &["SW-modes", "GF-vgg"]);

/// Rounds (a turn a seat) an Onyx sits before it turns blue: "a certain number of turns"
/// (SW-modes), "a set amount of time" (GF-cyber).
pub const ONYX_ROUNDS: Tuned<i64> = picked(2, "The sources give no number: two rounds lets every seat pass it by twice before it pays.");

/// How many jewels fall as the game starts.
pub const OPENING: Tuned<u32> = picked(
    4,
    "The sources say only that they fall from the sky and lie scattered about (SW-modes, GF-vgg): a few to aim at from the first turn.",
);

/// The most on the map at once: a turn's end brings one more while there are fewer.
pub const MOST: Tuned<usize> = picked(
    8,
    "Silent sources: enough that most seats have one in reach while the dearer ones wait for an SS or a Dual, few enough to read the map at a glance.",
);

/// How big a jewel is, px: a square sitting on the ground.
pub const SIZE: Tuned<f64> = picked(
    32.0,
    "Half a mobile's height: small next to a mobile, as GunBound's \"weird little things\" are (GF-super), and big enough that a fair shot at one takes it.",
);

/// How often each kind falls, by `JewelKind`, out of their sum.
pub const ODDS: Tuned<[u32; 4]> = picked(
    [6, 2, 1, 1],
    "Silent sources: the cheap ones most, the 25 rarely, and a black one now and then to read before shooting.",
);
