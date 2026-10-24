//! Lensowl: an owl astronomer on a brass telescope rover, a clockwork moon of a satellite hovering
//! over it, with GunBound A.Sate's behavior. Geometry (hitbox, probes, cannon) matches its drawing
//! in `assets/mobiles/lensowl/rig.ron`.
//!
//! Every shot is a tracer that hurts and carves nothing (WP06-list, GF-lee); once it lands, the
//! satellite fires lasers at it from wherever it hovers, and the first mobile or ground in a
//! laser's way takes it (SW-mob, SNX-asate, CR-sate). The satellite hovers straight over Lensowl
//! for S1 and S2, higher for S2, and moves over the landing for the SS (GF-asate2). Where the
//! sources come from, and what they leave open: `docs/research/lensowl.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};
use crate::game::Shot;

/// S1 and S2's angles. CR-mob gives no false range, so all of it is true; DBR's clone agrees.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 20,
        max: 60,
        true_min: 20,
        true_max: 60,
    },
    &["CR-mob", "GC-db", "DBR"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Lensowl",
    behaves_like: "A.Sate",
    // CR-mob (750, 200, +30; GC-db agrees) and SW-mob (760, 220, +20) disagree (CONTRADICTION), so
    // DragonBound's, which matches neither.
    hp: cited(800, &["DB-mob"]),
    shield: cited(210, &["DB-mob"]),
    shield_regen: cited(35, &["DB-mob"]),
    armor: cited(ArmorClass::Shield, &["SW-mob", "DB-mob"]),
    weapon: cited(WeaponType::Laser, &["SW-mob", "DB-mob"]),
    // SW-mob says 490 (CONTRADICTION); its own delay table's totals (SW-mds) add up from 480.
    base_delay: cited(480, &["CR-mob", "SW-mds", "GC-db", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // CR-60 needs 2.55 (CONTRADICTION); the weight's descriptions run from light (GF-tiro) to
    // heavy (WP06-list).
    gravity: fitted(GRAVITY, &["CR-fixed", "CR-sate"], "60 degrees lands one screen at 2.50 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -33.0, y: -56.0, w: 64.0, h: 56.0 },
        "Lensowl's drawn rover and owl. The rider, the ear tufts, the telescope and the satellite are cosmetic.",
    ),
    ground_probes: picked([-21.0, 21.0], "Lensowl's drawn rear and front wheels."),
    cannon_pivot: picked((17.0, -36.0), "Lensowl's drawn telescope mount, on the rover's front deck."),
    barrel_length: picked(30.0, "Lensowl's drawn telescope, mount to lens."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds", "DB-mob"]),
        aim: AIM,
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mds", "DB-mob"]),
        // S2's range, five degrees in at each end (GF-asate).
        aim: cited(
            AimRange {
                min: 25,
                max: 55,
                true_min: 25,
                true_max: 55,
            },
            &["CR-mob", "GC-db", "GF-asate"],
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 651.8;

/// How high over Lensowl's feet its satellite hovers for S1, px: "pretty low above the bot's head"
/// (WP06-gbart). The SS satellite is "lowered by around 2 mobile lengths" from S2's (GF-max), and
/// Lensowl, S1's, the SS's and S2's satellites are evenly spaced (GF-asate2): so S1's hovers about
/// two mobile lengths up.
pub const SATELLITE_HEIGHT: Tuned<f64> = fitted(130.0, &["GF-max", "GF-asate2"], "Two of Lensowl's lengths (its 64 px hitbox) over its feet, to the nearest 10 px.");

/// How many times S1's height S2's satellite rises to (SW-mob, GF-asate2). GF-vgg says twice
/// (CONTRADICTION).
pub const S2_RISE: Tuned<f64> = cited(3.0, &["SW-mob", "GF-asate2"]);

/// How many times S1's height the SS satellite hovers at, over Lensowl's feet: between S1's and
/// S2's, evenly spaced (GF-asate2; GF-asate says S2's, CONTRADICTION). Moving over the landing it
/// keeps that height, as it only moves sideways (GF-asate).
pub const SS_RISE: Tuned<f64> = cited(2.0, &["GF-asate2", "GF-max"]);

/// How long the satellite takes to rise or sink to a shot's height as it fires, seconds.
pub const RISE_SECONDS: Tuned<f64> = picked(0.25, "Quick enough to be in place before any tracer lands.");

/// How long after the tracer lands the first laser fires, seconds.
pub const LASER_DELAY_SECONDS: Tuned<f64> = picked(0.2, "Skyeye's beat: the satellite answers the tracer, readably apart.");

/// The time between one laser and the next (S2, SS), seconds.
pub const LASER_GAP_SECONDS: Tuned<f64> = picked(0.15, "Three or five beats in a row, read one at a time; no source times them.");

/// "Half a centimeter", on a 2004 screen: 800 px across about 30 cm. SW06-mob uses it for S1's
/// aim and the SS satellite's step.
const HALF_A_CENTIMETER: f64 = 13.0;

/// S1: the tracer lands, and the satellite fires one laser at it (SW-mob). Its inaccuracy is a
/// bias, not a roll: "When shooting from the right side of the opponent, the laser tends to shoot
/// about half a centimeter above the guide. When shooting on the left, it tends to shoot in a more
/// accurate manner" (SW06-mob; GF-max's laser "may swoop past the mobile's head"). So it aims this
/// far above the tracer when the satellite is right of it, and at it otherwise.
pub const S1_HIGH: Tuned<f64> = fitted(HALF_A_CENTIMETER, &["SW06-mob"], "Half a centimeter on an 800 px, 30 cm wide screen.");
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 100, max: 160 }, &["SW-mob", "DB-mob"]);
/// A narrow beam: "A near miss does no damage also" (CR-sate).
pub const S1_BLAST: Tuned<f64> = picked(15.0, "A narrow beam: a near miss does little (CR-sate).");

/// S2: three lasers from three times as high, one after another. The last (the lead) aims at the
/// tracer; the two before it lean left on screen, whichever way Lensowl faces, so they "always
/// rain down from left to right" onto it (CR-sate, GF-asate, SW06-mob). A lean is an angle at the
/// satellite, so they bunch when it fires straight down and spread with distance (GF-asate),
/// until "half your shot will miss" a screen away (CR-sate).
pub const S2_LASERS: Tuned<u32> = cited(3, &["SW-mob", "DB-mob", "GF-asate"]);
pub const S2_LEAN_DEGREES: Tuned<f64> = picked(
    1.5,
    "About 14 px between lasers fired straight down, 28 px half a screen off and 53 px a screen off, where one of three hits.",
);
/// GF-max's 80-90 a laser.
pub const S2_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 67, max: 93 }, "DB-mob's 200-280 for the three, split evenly.");
pub const S2_BLAST: Tuned<f64> = picked(15.0, "S1's narrow beam: bunched, they dig a narrow hole (CR-sate).");

/// SS: the satellite glides sideways over where the tracer landed, then sweeps from left to right,
/// firing five lasers straight down a step apart (SW-mob, GF-asate; WP06-list fans them,
/// CONTRADICTION; GF-asate counts 6 and NAMU 7). Then it goes back.
pub const SS_LASERS: Tuned<u32> = cited(5, &["SW-mob", "WP06-list"]);
pub const SS_STEP: Tuned<f64> = fitted(HALF_A_CENTIMETER, &["SW-mob"], "Half a centimeter on an 800 px, 30 cm wide screen.");
/// How long the satellite takes to glide over the landing, seconds.
pub const SS_GLIDE_SECONDS: Tuned<f64> = picked(0.6, "A glide the eye can follow across a screen; no source times it.");
pub const SS_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 80, max: 110 }, "SW-mob and DB-mob's 400-550 for the five, split evenly.");
pub const SS_BLAST: Tuned<f64> = picked(18.0, "A narrow, deep hole (GF-asate2), a little wider than S2's.");

/// How high over Lensowl's feet its satellite hovers while `shot` is picked, px.
#[must_use]
pub fn satellite(shot: Shot) -> f64 {
    let rise = match shot {
        Shot::S1 => 1.0,
        Shot::S2 => S2_RISE.value,
        Shot::Ss => SS_RISE.value,
    };
    SATELLITE_HEIGHT.value * rise
}
