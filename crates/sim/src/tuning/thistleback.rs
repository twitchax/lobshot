//! Thistleback: a hedgehog gardener pushing a wheelbarrow catapult, with GunBound Trico's
//! behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/thistleback/rig.ron`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Thistleback",
    behaves_like: "Trico",
    // CR-mob (1050) and SW-mob (1100, as OB) disagree (CONTRADICTION), so DragonBound's, which
    // matches CR-mob.
    hp: cited(1050, &["CR-mob", "DB-mob"]),
    shield: cited(0, &["CR-mob", "SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["CR-mob", "DB-mob"]),
    armor: cited(ArmorClass::Bionic, &["SW-mob", "DB-mob"]),
    weapon: cited(WeaponType::Explosion, &["SW-mob", "DB-mob"]),
    base_delay: cited(490, &["CR-mob", "SW-mds", "SW-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.65 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -34.0, y: -48.0, w: 71.0, h: 48.0 },
        "Thistleback's drawn hedgehog and wheelbarrow. The rider, the quill tips and the catapult arm are cosmetic.",
    ),
    ground_probes: picked([-21.0, 26.0], "Thistleback's drawn hind feet and barrow wheel."),
    cannon_pivot: picked((33.0, -39.0), "Thistleback's drawn catapult hinge, on the barrow's front rim."),
    barrel_length: picked(28.0, "Thistleback's drawn catapult arm, hinge to the cabbage in its spoon."),
    // DBR's clone gives 10-60 (CONTRADICTION); GunBound's table comes first, and OB agrees.
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "SW-mob"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 50,
                true_min: 20,
                true_max: 40,
            },
            &["CR-mob", "OB"],
        ),
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds", "SW-mob"]),
        aim: cited(
            AimRange {
                min: 10,
                max: 50,
                true_min: 20,
                true_max: 40,
            },
            &["CR-mob", "OB"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mob"]),
        aim: cited(
            AimRange {
                min: 20,
                max: 40,
                true_min: 20,
                true_max: 40,
            },
            &["CR-mob", "OB"],
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 748.0;

/// S1: a heavy cabbage that bursts where it lands (SW-mob).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 100, max: 150 }, &["SW-mob", "DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(30.0, "High bunge (SW-mob): Coilback's wide S1 blast.");

/// S2: three cabbages, two circling the middle one clockwise (GB-wiki) as they fly, each its own
/// hit (OB, SW-mob). The middle one flies the shot's own path.
pub const S2_BALL_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 57, max: 103 }, "SW-mob and DB-mob's 170-310 for the three, split evenly.");
pub const S2_BALL_BLAST: Tuned<f64> = picked(20.0, "Three tight blasts, like the other many-shot S2s.");
/// Each cabbage's distance from the path (px) and where on its circle it starts (radians
/// clockwise from `+x` on screen): one on the path, two either side of it in a line.
pub const S2_ORBITERS: Tuned<[(f64, f64); 3]> = cited([(0.0, 0.0), (35.0, 0.0), (35.0, core::f64::consts::PI)], &["OB"]);
/// Turns of the outer two about the middle one per second.
pub const S2_HERTZ: Tuned<f64> = cited(0.5, &["OB"]);
/// How long the outer two take to swing out from the path to their full radius, seconds (OB
/// ramps them in at 2 a second).
pub const S2_RAMP_SECONDS: Tuned<f64> = cited(0.5, &["OB"]);

/// SS: a fireball that bursts into many blasts where it lands, spinning round the impact and
/// removing much land; its damage peaks when it lands at a target's feet (SW-mob, DB-mob).
/// OpenBound's burst: one blast at the impact, then one at each of the 8 compass points 20 px
/// around it, 0.1 s apart (OB). How the feet matter is our pick, as no source says: each blast
/// burns low (`damage::low`), worth most at or under a mobile's feet and least at the top of its
/// body, so a pumpkin at the feet burns hottest and digs the ground out from under them.
///
/// The blasts round the first, one at each compass point.
pub const SS_RING: Tuned<u32> = cited(8, &["OB"]);
pub const SS_RING_RADIUS: Tuned<f64> = cited(20.0, &["OB"]);
pub const SS_RING_SECONDS: Tuned<f64> = cited(0.1, &["OB"]);
/// Each blast's reach and crater: nine of them carve a hole about 90 px across.
pub const SS_BLAST: Tuned<f64> = cited(25.0, &["OB"]);
/// Each of the nine blasts' damage range.
pub const SS_BLAST_DAMAGE: Tuned<DamageRange> = fitted(
    DamageRange { min: 30, max: 75 },
    &["SW-mob", "DB-mob"],
    "The nine blasts total 500 against a Coilback on flat ground when the pumpkin strikes the foot of its front, and 250 when it strikes the top of its body.",
);
