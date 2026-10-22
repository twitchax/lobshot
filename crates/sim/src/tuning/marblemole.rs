//! Marblemole: a mole in a mine cart that bowls static-charged marbles, with GunBound Grub's
//! behavior. Geometry (hitbox, probes, cannon) matches
//! its drawing in `assets/mobiles/marblemole/rig.ron`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

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
    hitbox: picked(
        Rect { x: -34.0, y: -58.0, w: 64.0, h: 58.0 },
        "Marblemole's drawn cart and the mole in it, helmet included. The rider and the lever are cosmetic.",
    ),
    ground_probes: picked([-15.0, 13.0], "Marblemole's drawn cart wheels."),
    cannon_pivot: picked((28.0, -39.0), "Marblemole's drawn chute hinge, on the cart's front corner."),
    barrel_length: picked(28.0, "Marblemole's drawn chute, hinge to the marble at its lip."),
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
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 574.3;

/// S1: the weakest shot in the game (DB-mob).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 80, max: 130 }, &["DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(20.0, "A weak shot's small blast.");

/// S2: four marbles that bounce and roll, bounce off the map's sides, and go off about 3 s after
/// their first contact or at once on touching a mobile (DB-mob).
pub const S2_MARBLE_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 45, max: 55 }, "DB-mob's 180-220 for the four, split evenly.");
pub const S2_MARBLE_BLAST: Tuned<f64> = picked(18.0, "Four small blasts.");
/// Each marble's power (bars) off the shot's: they leave together and spread as they bounce.
pub const S2_SPREAD: Tuned<[f64; 4]> = picked([-0.15, -0.05, 0.05, 0.15], "A short line of marbles across the target.");
pub const S2_FUSE_SECONDS: Tuned<f64> = cited(3.0, &["DB-mob"]);
/// The share of a marble's speed into a surface that it keeps, bouncing back out.
pub const S2_RESTITUTION: Tuned<f64> = picked(0.5, "Lively for a few hops, then settling: the fuse outlasts the bouncing.");
/// The share of its speed along a surface a marble keeps through a bounce.
pub const S2_FRICTION: Tuned<f64> = picked(0.85, "Enough grip to stop rolling within the fuse on flat ground.");
/// Below this speed (px/s) after a bounce, a marble comes to rest until its fuse.
pub const S2_REST_SPEED: Tuned<f64> = picked(40.0, "Hops lower than a pixel or two read as rest.");
/// The most bounces a marble makes before it rests where it is.
pub const S2_MAX_BOUNCES: Tuned<u8> = picked(16, "A guard: more bounces than any fuse allows in practice.");

/// SS: a ball that passes through mobiles, hurting each the whole time it touches them, and blows
/// up on the ground (DB-mob). A touch deals at least `min`, more the longer it lasts, up to `max`.
pub const SS_TOUCH_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 400, max: 1000 }, &["DB-mob"]);
/// Damage per second of touch on top of `SS_TOUCH_DAMAGE.min`.
pub const SS_TOUCH_PER_SECOND: Tuned<f64> = picked(4000.0, "A ball crossing a body in about 0.1 s deals about 800; a long, slow touch reaches the max.");
pub const SS_LAND_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 80, max: 130 }, "Where it lands it blows up like an S1.");
pub const SS_LAND_BLAST: Tuned<f64> = picked(30.0, "A bigger crater than S1's.");
