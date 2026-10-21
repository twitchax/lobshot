//! Sparkhorn: a rhino-beetle with a tesla-coil horn and a crackling bubble shield, with GunBound
//! Lightning's behavior. Its shots land with the M3 sim work; until then they fly as Coilback's S1.
//! Geometry (hitbox, probes, cannon) is Coilback's until the drawing in
//! `assets/mobiles/sparkhorn/rig.ron` lands.

use super::{AimRange, ArmorClass, MobileTuning, Rect, ShotTuning, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Sparkhorn",
    behaves_like: "Lightning",
    // CR-mob (750, 300, +20) and SW-mob (760, 220, +20) disagree, so DragonBound's, which matches CR-mob.
    hp: cited(750, &["CR-mob", "DB-mob"]),
    shield: cited(300, &["CR-mob", "DB-mob"]),
    shield_regen: cited(20, &["CR-mob", "SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Shield, &["DB-mob"]),
    weapon: cited(WeaponType::Electric, &["DB-mob"]),
    base_delay: cited(500, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.30 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    hitbox: picked(
        Rect { x: -32.0, y: -54.0, w: 64.0, h: 54.0 },
        "Sparkhorn's drawn wing cases, thorax and head, down to its wheels. The rider, the capacitor and the short horn are cosmetic.",
    ),
    ground_probes: picked([-23.0, 23.0], "Sparkhorn's drawn rear and front wheels."),
    cannon_pivot: picked((25.0, -23.0), "Sparkhorn's drawn neck: the head turns with the horn."),
    barrel_length: picked(27.6, "Sparkhorn's drawn head and horn, neck to discharge ball."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
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
        delay: cited(300, &["CR-mob", "SW-mds"]),
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
        delay: cited(800, &["CR-mob"]),
        aim: cited(
            AimRange {
                min: 23,
                max: 35,
                true_min: 23,
                true_max: 35,
            },
            &["OB"],
        ),
    },
};

/// Scaled from Coilback's by the square of the power ratio (2.30 against 2.45 bars, since range
/// goes as power squared over gravity); `tool_fit` refines it.
const GRAVITY: f64 = 548.2;
