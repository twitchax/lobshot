//! Hookclaw: a hermit-crab mech that flings crescent boomerangs from its big claw, with GunBound
//! Boomer's behavior. Geometry (hitbox, probes, cannon) matches
//! its drawing in `assets/mobiles/hookclaw/rig.ron`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Hookclaw",
    behaves_like: "Boomer",
    // DragonBound lists 1100 (DB-mob).
    hp: cited(1000, &["SW-mob", "CR-mob"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Bionic, &["DB-mob"]),
    weapon: cited(WeaponType::Physical, &["DB-mob"]),
    base_delay: cited(480, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.25 bars, zero wind, flat ground."),
    // Boomer is "extremely sensitive" to wind (WP06). DBR's coefficients make it 1.9 times Armor's
    // (1.395 against 0.74, clone, unverified); UC-boomer's hook asks for more (CONTRADICTION). The
    // fit is to UC-boomer's saving, not its powers: it puts the calm 70-degree shot at 2.8 bars,
    // where CR-fixed's 2.25 at 60 degrees puts it at 2.64 (CONTRADICTION).
    wind_sensitivity: fitted(
        WIND_SENSITIVITY,
        &["UC-boomer"],
        "10 wind toward the target saves 0.45 bars on a 70-degree, one-screen shot, flat ground.",
    ),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -33.0, y: -54.0, w: 63.0, h: 54.0 },
        "Hookclaw's drawn jar and crab. The rider, eyestalks and leg tips are cosmetic.",
    ),
    ground_probes: picked([-14.0, 24.0], "Hookclaw's drawn hind and front leg tips."),
    cannon_pivot: picked(
        (15.0, -40.0),
        "Hookclaw's drawn claw shoulder, in the jar's mouth: high enough that the claw clears the face at 0 degrees and forward enough that it clears the jar at 90.",
    ),
    barrel_length: picked(31.0, "Hookclaw's drawn claw, shoulder to boomerang."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 90,
                true_min: 35,
                true_max: 65,
            },
            &["CR-mob"],
        ),
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 90,
                true_min: 35,
                true_max: 65,
            },
            &["CR-mob"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 90,
                true_min: 35,
                true_max: 65,
            },
            &["CR-mob"],
        ),
    },
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites these lines: gravity first (in calm air), then the wind sensitivity on top of it.
const GRAVITY: f64 = 528.8;
const WIND_SENSITIVITY: f64 = 2.53;

/// S1: a light boomerang; its hook comes from the wind (`WIND_SENSITIVITY`).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 120, max: 160 }, &["DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(22.0, "A light boomerang: a smaller blast than Coilback's heavy round.");

/// S2: four boomerangs travelling clustered (DB-mob).
pub const S2_BOOMERANG_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 57, max: 72 }, "DB-mob's 230-290 for the four, split evenly.");
pub const S2_BOOMERANG_BLAST: Tuned<f64> = picked(16.0, "Four small blasts that land close together.");
/// Each boomerang's aim (degrees) and power (bars) off the shot's: a tight square.
pub const S2_SPREAD: Tuned<[(i16, f64); 4]> = picked([(-1, -0.05), (1, -0.05), (-1, 0.05), (1, 0.05)], "Clustered (DB-mob): a degree and a power step apart.");

/// SS: turns golden after a few seconds in flight (DB-mob, SW-mob), for much more damage. One that
/// lands before it turns does S1 damage (SW-mob).
pub const SS_TRANSFORM_SECONDS: Tuned<f64> = picked(2.5, "Sources say only \"a few seconds\"; the one transform time DB-mob states is Armor's 2.5 s.");
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 350, max: 900 }, &["DB-mob"]);
pub const SS_BLAST: Tuned<f64> = picked(35.0, "The golden boomerang's blast; bigger than S1's.");
pub const SS_EARLY_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 120, max: 160 }, &["SW-mob", "DB-mob"]);

/// The damage multiplier on a boomerang the wind turned around in flight: a boomer shot or a
/// backshot.
pub const HOOK_DAMAGE: Tuned<f64> = cited(1.2, &["GF-max"]);
