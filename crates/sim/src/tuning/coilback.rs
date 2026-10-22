//! Coilback: a snail-shell bell-mortar on treads, with GunBound Armor's behavior. Geometry
//! (hitbox, probes, cannon) matches its drawing in `assets/mobiles/coilback/rig.ron`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Coilback",
    behaves_like: "Armor",
    hp: cited(1000, &["SW-mob", "DB-mob"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["DB-mob"]),
    weapon: cited(WeaponType::Explosion, &["DB-mob"]),
    base_delay: cited(520, &["CR-mob", "SW-mds"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: cited(40, &["SW-mds"]),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.45 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "Armor is GunBound's reference shot; the wind rules are stated for it."),
    move_budget: cited(90, &["OB"]),
    hitbox: picked(
        Rect { x: -30.0, y: -58.0, w: 64.0, h: 58.0 },
        "Coilback's drawn body, shell included. The rider (WP06) and the antennae tips past x = 34 are cosmetic.",
    ),
    ground_probes: picked([-18.0, 18.0], "Coilback's tread contact points."),
    cannon_pivot: picked((7.0, -48.0), "Coilback's drawn turret cup."),
    barrel_length: picked(28.8, "Coilback's drawn bell mortar."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 55,
                true_min: 25,
                true_max: 40,
            },
            &["CR-mob", "OB"],
        ),
    },
    s2: ShotTuning {
        delay: cited(440, &["CR-mob", "SW-mds"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 55,
                true_min: 25,
                true_max: 40,
            },
            &["CR-mob", "OB"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob"]),
        // CR-mob gives SS 10-50 with no false range; OpenBound reads it as all true.
        aim: cited(
            AimRange {
                min: 10,
                max: 50,
                true_min: 10,
                true_max: 50,
            },
            &["CR-mob", "OB"],
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`.
const GRAVITY: f64 = 622.0;

/// S1: one heavy round with a wide blast, good for bunging.
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 120, max: 165 }, &["DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(30.0, "A wide blast for bunging (DB-mob), wider than OpenBound's flat 25.");

/// S2: two warheads. The first detonates on contact; the second carries on through the crater to
/// the next contact, so a direct hit lands both.
pub const S2_WARHEAD_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 115, max: 145 }, "DB-mob's 230-290 for the pair, split evenly.");
pub const S2_WARHEAD_BLAST: Tuned<f64> = picked(20.0, "Tighter than S1: the pair carves two craters.");

/// SS: a shell that expands in flight and then hits much harder. GunBound sources disagree on
/// when (3 s WP06, 1.8 s GF-tiro), so DragonBound's 2.5 s.
pub const SS_TRANSFORM_SECONDS: Tuned<f64> = cited(2.5, &["DB-mob"]);
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 350, max: 500 }, &["DB-mob"]);
pub const SS_BLAST: Tuned<f64> = picked(40.0, "The expanded shell's blast; bigger than S1's.");
/// Before it expands, the SS shell hits like S1.
pub const SS_EARLY_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 120, max: 165 }, "An SS that lands before expanding is an S1 round.");
