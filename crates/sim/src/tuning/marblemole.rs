//! Marblemole: a mole in a mine cart that bowls static-charged marbles, with GunBound Grub's
//! behavior. Its shots land with the M3 sim work; until then they fly as Coilback's S1.
//! Geometry (hitbox, probes, cannon) is Coilback's until the drawing in
//! `assets/mobiles/marblemole/rig.ron` lands.

use super::{AimRange, ArmorClass, MobileTuning, Rect, ShotTuning, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Marblemole",
    behaves_like: "Grub",
    hp: cited(1000, &["SW-mob", "CR-mob", "DB-mob"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Bionic, &["DB-mob"]),
    weapon: cited(WeaponType::Electric, &["DB-mob"]),
    base_delay: cited(490, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.33 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    hitbox: picked(Rect { x: -30.0, y: -58.0, w: 64.0, h: 58.0 }, "Placeholder: Coilback's, until Marblemole's drawing lands."),
    ground_probes: picked([-18.0, 18.0], "Placeholder: Coilback's, until Marblemole's drawing lands."),
    cannon_pivot: picked((7.0, -64.0), "Placeholder: above Coilback's, so its whole aim range clears the body, until Marblemole's drawing lands."),
    barrel_length: picked(28.8, "Placeholder: Coilback's, until Marblemole's drawing lands."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 30,
                max: 60,
                true_min: 35,
                true_max: 55,
            },
            &["CR-mob"],
        ),
    },
    s2: ShotTuning {
        // SW-mob says 480 (CONTRADICTION with CR-mob and SW-mds); DragonBound agrees on 400.
        delay: cited(400, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 30,
                max: 60,
                true_min: 35,
                true_max: 55,
            },
            &["CR-mob"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob"]),
        aim: cited(
            AimRange {
                min: 30,
                max: 60,
                true_min: 30,
                true_max: 60,
            },
            &["CR-mob"],
        ),
    },
};

/// Scaled from Coilback's by the square of the power ratio (2.33 against 2.45 bars, since range
/// goes as power squared over gravity); `tool_fit` refines it.
const GRAVITY: f64 = 562.6;
