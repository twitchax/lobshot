//! Lanternwing: a moth mech with a glowing lantern cannon, its wings aglow as its shield, with
//! GunBound Mage's behavior. Geometry (hitbox, probes, cannon) is Coilback's until the drawing in
//! `assets/mobiles/lanternwing/rig.ron` lands.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Lanternwing",
    behaves_like: "Mage",
    // CR-mob (730, 220, +30) and SW-mob (760, 220, +20) disagree, so DragonBound's, which matches CR-mob.
    hp: cited(730, &["CR-mob", "DB-mob"]),
    shield: cited(220, &["CR-mob", "SW-mob", "DB-mob"]),
    shield_regen: cited(30, &["CR-mob", "DB-mob"]),
    armor: cited(ArmorClass::Shield, &["DB-mob"]),
    weapon: cited(WeaponType::Laser, &["DB-mob"]),
    base_delay: cited(500, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.38 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    hitbox: picked(Rect { x: -30.0, y: -58.0, w: 64.0, h: 58.0 }, "Placeholder: Coilback's, until Lanternwing's drawing lands."),
    ground_probes: picked([-18.0, 18.0], "Placeholder: Coilback's, until Lanternwing's drawing lands."),
    cannon_pivot: picked((7.0, -48.0), "Placeholder: Coilback's, until Lanternwing's drawing lands."),
    barrel_length: picked(28.8, "Placeholder: Coilback's, until Lanternwing's drawing lands."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 15,
                max: 50,
                true_min: 15,
                true_max: 50,
            },
            &["CR-mob", "OB"],
        ),
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 15,
                max: 50,
                true_min: 15,
                true_max: 50,
            },
            &["CR-mob", "OB"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob"]),
        aim: cited(
            AimRange {
                min: 15,
                max: 50,
                true_min: 15,
                true_max: 50,
            },
            &["CR-mob", "OB"],
        ),
    },
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 587.0;

/// S1: a laser bolt with good bunge (DB-mob).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 100, max: 160 }, &["DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(30.0, "Good bunge (DB-mob): Coilback's wide S1 blast.");

/// S2: two strands on a helix about the flight path, each a separate hit (OB).
pub const S2_STRAND_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 95, max: 140 }, "DB-mob's 190-280 for the pair, split evenly.");
pub const S2_STRAND_BLAST: Tuned<f64> = picked(20.0, "Two tight blasts, like Coilback's S2 warheads.");
/// Each strand's distance from the flight path at its widest, px.
pub const S2_AMPLITUDE: Tuned<f64> = cited(15.0, &["OB"]);
/// Turns of the helix per second.
pub const S2_HERTZ: Tuned<f64> = cited(1.0, &["OB"]);
/// The strands' phases: opposite each other, so they wind about the path.
pub const S2_PHASES: Tuned<[f64; 2]> = cited([0.0, core::f64::consts::PI], &["OB"]);

/// SS: a large ball that strips the shields of every mobile within reach of where it lands, then
/// hits hard on a direct hit (DB-mob).
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 350, max: 500 }, &["DB-mob"]);
pub const SS_BLAST: Tuned<f64> = picked(40.0, "A large-radius ball (DB-mob): Coilback's expanded SS shell.");
/// How far from the landing (to the hitbox) the SS strips shields.
pub const SS_STRIP_REACH: Tuned<f64> = cited(100.0, &["OB"]);
