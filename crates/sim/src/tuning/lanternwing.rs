//! Lanternwing: a moth mech with a glowing lantern cannon, its wings aglow as its shield, with
//! GunBound Mage's behavior. Its shots land with the M3 sim work; until then they fly as Coilback's S1.
//! Geometry (hitbox, probes, cannon) is Coilback's until the drawing in
//! `assets/mobiles/lanternwing/rig.ron` lands.

use super::{AimRange, ArmorClass, MobileTuning, Rect, ShotTuning, WeaponType, cited, fitted, picked};

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

/// Scaled from Coilback's by the square of the power ratio (2.38 against 2.45 bars, since range
/// goes as power squared over gravity); `tool_fit` refines it.
const GRAVITY: f64 = 587.0;
