//! Sparkhorn: a rhino-beetle with a tesla-coil horn and a crackling bubble shield, with GunBound
//! Lightning's behavior. Geometry (hitbox, probes, cannon) matches
//! its drawing in `assets/mobiles/sparkhorn/rig.ron`.
//!
//! Its S1 and S2 land a ball that calls bolts down onto the impact; the bolts' splash is GunBound
//! Lightning's constant part (at least 76, SW-mob, WP06), so the ball carries the rest of
//! DragonBound's range.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

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
    climb_px: super::CLIMB_PX,
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
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 550.5;

/// The flat damage a bolt's splash deals each mobile within `BOLT_REACH` of its path, once a turn
/// even with Dual (GF-max, WP06). Splash ignores armor, angle, weather and items (GF-max).
pub const SPLASH: Tuned<i32> = cited(76, &["SW-mob", "WP06", "GF-max"]);

/// S1: the ball lands, then a bolt strikes straight down that column; land above the target blocks
/// it (DB-mob; OpenBound strikes the highest solid point in the column).
pub const S1_BALL_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 74, max: 174 }, "DB-mob's 150-250, less the splash.");
pub const S1_BALL_BLAST: Tuned<f64> = picked(20.0, "A small ball: the bolt does the reaching.");

/// S2: the ball lands, then two bolts strike it from either side in a V (WP06).
pub const S2_BALL_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 74, max: 224 }, "DB-mob's 150-300, less one splash.");
pub const S2_BALL_BLAST: Tuned<f64> = picked(20.0, "As S1's.");
/// Each bolt's angle off the vertical.
pub const S2_BOLT_DEGREES: Tuned<f64> = cited(45.0, &["WP06"]);

/// SS: the ball lands, and every mobile within reach takes a bolt straight down its own column
/// (DB-mob). The ball itself only carves: DB-mob gives the SS no damage but its bolts'.
pub const SS_BALL_BLAST: Tuned<f64> = picked(30.0, "A crater to show where it landed.");
/// How far from the landing (to the hitbox) a mobile must be to take a bolt. OpenBound uses 100.
pub const SS_REACH: Tuned<f64> = picked(150.0, "DB-mob says a large radius; OpenBound's 100 is barely past a blast.");
pub const SS_BOLT_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 150, max: 200 }, &["DB-mob"]);
