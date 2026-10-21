//! Hookclaw: a hermit-crab mech that flings crescent boomerangs from its big claw, with GunBound
//! Boomer's behavior. Its shots land with the M3 sim work; until then they fly as Coilback's S1.
//! Geometry (hitbox, probes, cannon) is Coilback's until the drawing in
//! `assets/mobiles/hookclaw/rig.ron` lands.

use super::{AimRange, ArmorClass, MobileTuning, Rect, ShotTuning, WeaponType, cited, fitted, picked};

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
    wind_sensitivity: picked(1.9, "Boomer is very wind-sensitive (WP06); DBR's wind coefficient is 1.395 against Armor's 0.74 (clone, unverified)."),
    move_budget: cited(90, &["OB"]),
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

/// Scaled from Coilback's by the square of the power ratio (2.25 against 2.45 bars, since range
/// goes as power squared over gravity); `tool_fit` refines it.
const GRAVITY: f64 = 524.6;
