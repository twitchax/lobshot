//! Needlemouse: a mouse knight in a thimble helmet on a wind-up toy horse, a floating pincushion
//! for a satellite, with GunBound Knight's behavior. Geometry (hitbox, probes, cannon) matches its
//! drawing in `assets/mobiles/needlemouse/rig.ron`.
//!
//! Lensowl's satellite with swords for lasers: every shot is a tracer, and once it lands the
//! pincushion over Needlemouse fires pins at it, each a projectile flying straight at the tracer
//! from where it leaves the cushion, so they converge "in a reverse cone" (GF-asate, OB) and
//! whatever is in their way takes them (SNX-knight, NAMU). Where the sources come from, and what
//! they leave open: `docs/research/needlemouse.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};
use crate::game::Shot;

/// Every shot's angles: the widest true range in the game (WP06-list). DBR's clone aims 20-60.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 10,
        max: 80,
        true_min: 10,
        true_max: 80,
    },
    &["CR-mob", "GC-db", "OB", "WP06-list"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Needlemouse",
    behaves_like: "Knight",
    // DragonBound gives it 1500 (CONTRADICTION); GunBound's sources agree on 1000.
    hp: cited(1000, &["SW-mob", "CR-mob", "GC-db", "OB"]),
    shield: cited(0, &["SW-mob", "GC-db", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "GC-db", "DB-mob"]),
    // "the Gunbound's original Korean site lists Knight as a Metalic type" (GF-lee); GF-tiro says
    // Creature (CONTRADICTION).
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "GF-lee", "GF-cyber", "NAMU", "DB-mob"]),
    weapon: cited(WeaponType::Laser, &["SW-mob", "GF-lee", "GF-cyber", "DB-mob"]),
    // GF-lee's totals imply more (CONTRADICTION).
    base_delay: cited(550, &["SW-mob", "CR-mob", "GC-db", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // Knight's shot weighs what Lightning's does (WP06-list, CR-1sd, CR-3bar); CR-fixed charts
    // Lightning's at 2.30.
    gravity: fitted(
        GRAVITY,
        &["WP06-list", "CR-1sd", "CR-3bar", "CR-fixed"],
        "Lightning's weight: 60 degrees lands one screen at 2.30 bars, zero wind, flat ground.",
    ),
    // OB gives Knight and Dragon a wind influence of 1.4 against everyone else's 1.2; no GunBound
    // source sets it apart.
    wind_sensitivity: picked(1.0, "No GunBound source sets it apart from the reference shot."),
    // "GREAT mobility" (GF-tiro), 4 of 5 (GF-lee), average (SW-mob); NAMU says slow
    // (CONTRADICTION).
    move_budget: cited(100, &["OB", "GF-tiro"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -32.0, y: -52.0, w: 64.0, h: 52.0 },
        "Needlemouse's drawn toy horse and the mouse on it. The rider, the plume, the needle and the pincushion are cosmetic.",
    ),
    ground_probes: picked([-20.0, 20.0], "Needlemouse's drawn rear and front wheels."),
    cannon_pivot: picked((17.0, -49.0), "Needlemouse's drawn mouse paw, on the horse's neck."),
    barrel_length: picked(30.0, "Needlemouse's drawn sewing needle, hand to point."),
    s1: ShotTuning {
        delay: cited(250, &["SW-mob", "CR-mob", "GC-db", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(400, &["SW-mob", "CR-mob", "GC-db", "DB-mob"]),
        aim: AIM,
    },
    ss: ShotTuning {
        delay: cited(840, &["SW-mob", "CR-mob", "GC-db", "DB-mob"]),
        aim: AIM,
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 556.9;

/// How high over Needlemouse's feet its pincushion hovers for S1, px.
pub const SATELLITE_HEIGHT: Tuned<f64> = cited(150.0, &["OB"]);

/// S2's pincushion hovers twice as high (SW-mob; OB's 300).
pub const S2_RISE: Tuned<f64> = cited(2.0, &["SW-mob", "OB"]);

/// How long the pincushion takes to rise or sink to a shot's height as it fires, seconds.
pub const RISE_SECONDS: Tuned<f64> = picked(0.25, "Lensowl's: quick enough to be in place before any tracer lands.");

/// How long after the tracer lands the pins leave the cushion (S1, S2), seconds.
pub const PIN_DELAY_SECONDS: Tuned<f64> = picked(0.2, "Lensowl's laser delay: the cushion answers the tracer, readably apart.");

/// How fast a pin flies, px/s: straight, untouched by gravity or wind (GF-lee: they "just
/// repeatedly pound the spot").
pub const PIN_SPEED: Tuned<f64> = cited(800.0, &["OB"]);

/// How far apart the pins leave the cushion, across the line to the tracer, px.
pub const PIN_SPACING: Tuned<f64> = cited(15.0, &["OB"]);

/// How long a pin may fly before it is gone, seconds: a tornado or a mirror can send one off on a
/// straight line that never comes down.
pub const PIN_LIFE_SECONDS: Tuned<f64> = picked(3.0, "A pin crosses a 2400 px map in 3 s; one still flying by then has missed.");

/// Each pin's reach. Knight bunges far less than A.Sate (GF-lee: "VERY little bunging power"; its
/// SS has "only 3/4 of it's bunging"), so its crater is smaller than its reach.
pub const PIN_BLAST: Tuned<f64> = cited(25.0, &["OB"]);
pub const PIN_CRATER: Tuned<f64> = picked(12.0, "Little bunging (GF-lee): about half the pin's reach.");

/// S1: three pins (NAMU, OB), aimed as A.Sate's S1 laser is: "A little inaccurate like A.Sate
/// Shot1" (SW06-mob). SW-mob and DB-mob's 200-250 for the three, split evenly (OB's 80 a sword).
pub const S1_PINS: Tuned<u32> = cited(3, &["NAMU", "OB"]);
pub const S1_HIGH: Tuned<f64> = cited(super::lensowl::S1_HIGH.value, &["SW06-mob"]);
pub const S1_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 67, max: 83 }, "SW-mob and DB-mob's 200-250 for the three, split evenly.");

/// S2: five pins from twice as high (SW-mob, NAMU, OB), straight at the tracer: "more accurate than
/// A.Sate's Shot 2" (SW-mob).
pub const S2_PINS: Tuned<u32> = cited(5, &["SW-mob", "NAMU", "OB"]);
pub const S2_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 46, max: 58 }, "SW-mob and DB-mob's 230-290 for the five, split evenly.");

/// SS: "the sword moves right above where the guide lands and shoots many mini-swords down"
/// (SW-mob): seven (NAMU, OB), from 300 px over the tracer, swept back and forth across it, each
/// `SS_SWEEP` · cos(60° · i) left of it, the first `SS_FIRST_SECONDS` after the tracer lands and the
/// rest `SS_GAP_SECONDS` apart (OB). Each flies at the tracer.
pub const SS_PINS: Tuned<u32> = cited(7, &["NAMU", "OB"]);
pub const SS_HEIGHT: Tuned<f64> = cited(300.0, &["OB"]);
pub const SS_SWEEP: Tuned<f64> = cited(45.0, &["OB"]);
pub const SS_FIRST_SECONDS: Tuned<f64> = cited(0.4, &["OB"]);
pub const SS_GAP_SECONDS: Tuned<f64> = cited(1.0 / 7.5, &["OB"]);
pub const SS_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 57, max: 79 }, "SW-mob and DB-mob's 400-550 for the seven, split evenly.");

/// How high over Needlemouse's feet its pincushion hovers while `shot` is picked, px: the SS's
/// waits at S2's height before it moves over the tracer.
#[must_use]
pub fn satellite(shot: Shot) -> f64 {
    let rise = match shot {
        Shot::S1 => 1.0,
        Shot::S2 | Shot::Ss => S2_RISE.value,
    };
    SATELLITE_HEIGHT.value * rise
}
