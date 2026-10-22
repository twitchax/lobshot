//! Tuskrack: a walrus riding a tank-sled, a rack of fireworks rockets strapped to its back, with
//! GunBound Bigfoot's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/tuskrack/rig.ron`. What each source says about Bigfoot is in
//! `docs/research/tuskrack.md`.
//!
//! Every shot is a spread of light projectiles, each its own blast: four missiles, six grenades,
//! nine missiles. A shot's listed damage (SW-mob, DB-mob) is all of them landing, so each carries
//! an even share.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Tuskrack",
    behaves_like: "Bigfoot",
    hp: cited(1100, &["SW-mob", "CR-mob", "DB-mob"]),
    shield: cited(0, &["CR-mob", "DB-mob"]),
    shield_regen: cited(0, &["CR-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "DB-mob"]),
    weapon: cited(WeaponType::Explosion, &["SW-mob", "DB-mob"]),
    base_delay: cited(520, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // CR-fixed's cbchui chart gives Bigfoot "??", and GF-tiro calls all three of its shots heavy.
    // DBR's shot weight (`ax`, clone, unverified) tracks the chart's powers as their square root
    // (docs/research/mechanics.md, 2.3), and Bigfoot's 88 is the heaviest: the formula tests turn
    // it into a chart power (`INFERRED`), and the fit lands on that.
    gravity: fitted(
        GRAVITY,
        &["CR-fixed", "DBR"],
        "60 degrees lands one screen, zero wind, flat ground, at the cbchui power DBR's shot weight implies (formula.rs, INFERRED).",
    ),
    wind_sensitivity: picked(1.0, "DBR gives Bigfoot Armor's own wind coefficient (ay 0.74, clone, unverified)."),
    move_budget: picked(
        120,
        "\"High mobility\" (SW-mob, DB-mob), with no number anywhere: a third more than everyone's 90 (OB), whose one longer budget is Knight's 100.",
    ),
    // Climbing matters "for finding higher angles" (SW-mob): the aim turns with the body, and the
    // slopes it can't climb are the ones that would tilt it up.
    climb_px: picked(2, "\"Poor climbing\" (SW-mob, DB-mob): slopes up to about 63 degrees, where everyone else's 6 px takes about 80."),
    hitbox: picked(
        Rect { x: -36.0, y: -50.0, w: 72.0, h: 50.0 },
        "Tuskrack's drawn sled and walrus, tusks included: the roster's widest and lowest, as OB's Bigfoot (46 px wide against Armor's 30), within every mobile's spawn room. The rider and the rockets are cosmetic.",
    ),
    ground_probes: picked([-22.0, 22.0], "Tuskrack's drawn rear sprocket and front ski."),
    cannon_pivot: picked(
        (0.0, -52.0),
        "Tuskrack's drawn swivel, atop the post on the walrus's back: high enough that the rack clears the walrus at every aim.",
    ),
    barrel_length: picked(28.0, "Tuskrack's drawn rack, swivel to the bottom rocket's nose."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
        // DBR aims 20-45 (clone, unverified).
        aim: cited(
            AimRange {
                min: 18,
                max: 40,
                true_min: 23,
                true_max: 35,
            },
            &["CR-mob", "OB"],
        ),
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 18,
                max: 40,
                true_min: 23,
                true_max: 35,
            },
            &["CR-mob", "OB"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 23,
                max: 35,
                true_min: 23,
                true_max: 35,
            },
            &["CR-mob", "OB"],
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 755.4;

/// S1: four missiles, fairly weak, spreading wide over a long shot; great for bunging (SW-mob).
pub const S1_MISSILE_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 42, max: 57 }, "SW-mob's and DB-mob's 170-230 for the four, split evenly; GF-max puts each near 50.");
pub const S1_MISSILE_BLAST: Tuned<f64> = picked(
    20.0,
    "Two thirds of Coilback's S1, as DBR's Bigfoot missile crater is of Armor's (39 against 60, clone, unverified): four of them bunge.",
);
/// Each missile's aim (degrees) and power (bars) off the shot's, in the order they leave: two
/// straight down the line, one after the other, dig into the same crater.
pub const S1_SPREAD: Tuned<[(i16, f64); 4]> = cited([(3, 0.0), (-3, 0.0), (0, 0.0), (0, 0.0)], &["OB"]);
/// How long after each missile the next leaves the rack.
pub const S1_RIPPLE_SECONDS: Tuned<f64> = cited(0.2, &["OB"]);

/// S2: six grenades that leave as one bundle and spread soon after launch (SW-mob, DB-mob), into
/// three loose pairs (WP06).
pub const S2_GRENADE_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 33, max: 45 }, "SW-mob's and DB-mob's 200-270 for the six, split evenly.");
pub const S2_GRENADE_BLAST: Tuned<f64> = picked(18.0, "Six small blasts; DBR digs a grenade's crater about as big as a missile's (clone, unverified).");
/// How long the bundle flies before it bursts.
pub const S2_BURST_SECONDS: Tuned<f64> = picked(0.2, "\"Soon after launch\" (SW-mob); OB's first grenades leave at 0.2 s.");
/// Each grenade's turn (degrees, up for the way the shooter faces) and power (bars) off the
/// bundle's as it bursts: three pairs a step and a half of power apart, each pair side by side.
pub const S2_SPREAD: Tuned<[(i16, f64); 6]> = picked(
    [(-1, -0.15), (1, -0.15), (-1, 0.0), (1, 0.0), (-1, 0.15), (1, 0.15)],
    "Three loose pairs (WP06), spread by OB's 4 of its 100 power points (0.16 bars) to the bar's step.",
);

/// SS: nine missiles, like S1's (SW-mob, DB-mob). WP06, OB and DBR fire eight, GF-max twelve
/// (CONTRADICTION), so GunBound's own wiki and DragonBound's agreement: nine.
pub const SS_MISSILE_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 38, max: 55 }, "SW-mob's and DB-mob's 350-500 for the nine, split evenly.");
pub const SS_MISSILE_BLAST: Tuned<f64> = picked(20.0, "As S1's missiles.");
/// Each missile's aim (degrees) and power (bars) off the shot's, in the order they leave.
pub const SS_SPREAD: Tuned<[(i16, f64); 9]> = picked(
    [(3, 0.0), (-3, 0.0), (0, 0.0), (3, 0.0), (-3, 0.0), (0, 0.0), (3, 0.0), (-3, 0.0), (0, 0.0)],
    "OB's SS spread (3 degrees either side, then down the line), in threes for SW-mob's nine.",
);
pub const SS_RIPPLE_SECONDS: Tuned<f64> = cited(0.125, &["OB"]);
/// Whether an SS missile that strikes a mobile carves the ground: the SS does "no land damage on a
/// direct hit" (WP06), so a direct hit doesn't bunge its target out from under the missiles after it.
pub const SS_DIRECT_CRATER: Tuned<bool> = cited(false, &["WP06"]);
