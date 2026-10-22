//! Spoutpot: a hippo hauling a teapot water tank, hosing water from a twin-nozzle cannon, with
//! GunBound Turtle's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/spoutpot/rig.ron`. Turtle's shots are all airtime, so their clock (S2's spiral,
//! the SS opening) is fitted to CR-turtle's aiming tables (`docs/research/spoutpot.md`).

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// Turtle's arc, 25 to 50 degrees and all of it true, for every shot.
const AIM: AimRange = AimRange {
    min: 25,
    max: 50,
    true_min: 25,
    true_max: 50,
};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Spoutpot",
    behaves_like: "Turtle",
    // CR-mob (960) and SW-mob (950) disagree, so DragonBound's, which matches SW-mob (and OB).
    hp: cited(950, &["SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["CR-mob", "DB-mob"]),
    shield_regen: cited(0, &["CR-mob", "DB-mob"]),
    armor: cited(ArmorClass::Bionic, &["SW-mob", "DB-mob"]),
    weapon: cited(WeaponType::Physical, &["SW-mob", "DB-mob"]),
    base_delay: cited(490, &["CR-mob", "SW-mob", "SW-mds"]),
    // Turtle alone pays 12 a second, not 10.
    delay_per_second: cited(12, &["CR-mob", "SW-mob", "SW-delay", "SW-mds", "DB-mob"]),
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // CR-turtle says Turtle's shot weighs exactly what Armor's does (CONTRADICTION: 2.45 on the
    // same chart); the chart sets every other mobile's weight.
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.39 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: picked(
        70,
        "Poor mobility (SW-mob): a tiny, slow walking range that takes forever to reach a slope (CR-turtle). \
         OB gives every mobile but Knight 90, so it doesn't tell them apart.",
    ),
    hitbox: picked(
        Rect { x: -32.0, y: -52.0, w: 66.0, h: 52.0 },
        "Spoutpot's drawn hippo and teapot, a little lower than most: Turtle is tiny (CR-turtle). The rider, the lid and its \
         cushion, the lever, the tail and the snout's tip are cosmetic.",
    ),
    ground_probes: picked([-19.5, 16.0], "Spoutpot's drawn hind and fore feet."),
    cannon_pivot: picked((12.0, -41.0), "Spoutpot's drawn nozzle swivel, in the teapot's collar where its spout would be."),
    barrel_length: picked(25.0, "Spoutpot's drawn twin nozzle, swivel to mouths."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mob", "SW-mds"]),
        aim: cited(AIM, &["CR-mob", "OB"]),
    },
    s2: ShotTuning {
        // SW-mob and DB-mob say 480 (CONTRADICTION); CR-mob, SW-mds, CR-turtle and OB agree on 400.
        delay: cited(400, &["CR-mob", "SW-mds", "CR-turtle", "OB"]),
        aim: cited(AIM, &["CR-mob", "OB"]),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mob", "SW-mds"]),
        aim: cited(AIM, &["CR-mob", "OB"]),
    },
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites these lines: gravity (cbchui), the spiral's hertz on top of it (CR-turtle's long forks),
/// then the share of speed the SS's balls keep (CR-turtle's skybombs).
const GRAVITY: f64 = 590.0;
const HERTZ: f64 = 1.534;
const SPEED_KEPT: f64 = 0.464;

/// S1: a stream of water, moderate damage (SW-mob).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 130, max: 160 }, &["SW-mob", "DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(
    18.0,
    "Turtle's explosions are tiny and barely bunge (CR-turtle): a small round hole, though wider than one stream of S2's.",
);

/// S2: two streams winding about one path, each a separate hit, that cross over and over and
/// then come together for good (SW-mob, CR-turtle).
pub const S2_STREAM_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 110, max: 140 }, "SW-mob's and DB-mob's 220-280 for the pair, split evenly.");
pub const S2_STREAM_BLAST: Tuned<f64> = picked(14.0, "A tight splash: S2's near misses do very small damage, smaller than S1's (CR-turtle).");
/// Each stream's distance from the path at its widest, px.
pub const S2_AMPLITUDE: Tuned<f64> = picked(
    35.0,
    "At their widest the streams wrap a mobile's body without touching it (CR-turtle): 70 px apart clears \
     lobshot's widest bodies (66 px) by a little, as OB's 20 px clears its 38 px ones.",
);
/// Turns of the spiral per second: the streams cross twice a turn.
pub const S2_HERTZ: Tuned<f64> = fitted(
    HERTZ,
    &["CR-turtle"],
    "A long fork (40 degrees, 2.4 bars, one screen, and the rest of its table) meets its target on the streams' fifth crossing.",
);
/// The streams' phases: opposite each other, so they wind about the path and cross on it.
pub const S2_PHASES: Tuned<[f64; 2]> = cited([0.0, core::f64::consts::PI], &["OB"]);
/// The crossing at which the streams come together for good: they spiral six times, the fifth
/// crossing being the last a fork can use (CR-turtle). SW-mob says they come together after about
/// 3 s, CR-turtle about 2.5 s (CONTRADICTION): lobshot keeps CR-turtle's count of crossings, on a
/// clock fitted to its tables (`S2_HERTZ`).
pub const S2_MERGE_CROSSING: Tuned<f64> = cited(6.0, &["CR-turtle"]);

/// SS: one big blob of water that opens into six balls in flight; the balls fan out about the
/// way it was moving and fall faster than it did (CR-turtle, SW-mob). All six landing deal
/// 400-600 (SW-mob, DB-mob).
///
/// When it opens, in S2's crossings: the middle of S2's last spiral, just before its streams come
/// together for good (CR-turtle), so a little before S2 merges (SW-mob). In seconds sources give
/// 2.15-2.2 (CR-turtle), about 3 (DB-mob), 3.25 (WP06) and 1.5 (OB) (CONTRADICTION); only
/// CR-turtle's keeps SW-mob's order.
pub const SS_SPLIT_CROSSING: Tuned<f64> = cited(5.5, &["CR-turtle", "SW-mob"]);
/// Each ball's direction off the blob's as it opens, degrees: a fan, like a spread gun (CR-turtle).
pub const SS_FAN: Tuned<[f64; 6]> = cited([-7.5, -4.5, -1.5, 1.5, 4.5, 7.5], &["OB"]);
/// The share of the blob's speed each ball keeps: the SS "changes weight" as it opens (CR-turtle).
pub const SS_SPEED_KEPT: Tuned<f64> = fitted(
    SPEED_KEPT,
    &["CR-turtle"],
    "A one-screen skybomb (76 degrees, 3.91 bars) that opens near its peak lands its balls 795 px away.",
);
pub const SS_BALL_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 67, max: 100 }, "SW-mob's and DB-mob's 400-600 for the six, split evenly.");
pub const SS_BALL_BLAST: Tuned<f64> = picked(14.0, "S2's tight splash: six small holes.");
/// The blob, landing before it opens: "a weak, lousy hit", worse than a spread SS's (CR-turtle).
pub const SS_EARLY_DAMAGE: Tuned<DamageRange> = picked(SS_BALL_DAMAGE.value, "One ball's worth: weaker than even a crappy half hit (CR-turtle).");
pub const SS_EARLY_BLAST: Tuned<f64> = picked(S1_BLAST.value, "S1's small round hole.");

/// Seconds from the launch to S2's `crossing`-th crossing (fractional crossings fall between).
#[must_use]
pub fn crossing_seconds(crossing: f64) -> f64 {
    crossing / (2.0 * S2_HERTZ.value)
}
