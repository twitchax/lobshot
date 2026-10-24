//! Augerdillo: an armadillo on a drill rig, firing drill shells over its back, with GunBound
//! Nak's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/augerdillo/rig.ron`. Its sources are gathered in `docs/research/augerdillo.md`.
//!
//! It fires from the rear (DB-mob, WP06-list): its aim ranges lie over its back, past straight up
//! (`AimRange::rear`), so GunBound's 50-10 is 130-170 here, and it turns its back on a target to
//! fire at it.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// GunBound's S1 and S2 range, 50-10 with 45-15 true (CR-mob), over the back. DBR's clone aims
/// 110-170, 10-70 backwards (CONTRADICTION); GF-max says a later GunBound made the whole arc true.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 130,
        max: 170,
        true_min: 135,
        true_max: 165,
    },
    &["CR-mob"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Augerdillo",
    behaves_like: "Nak",
    hp: cited(1100, &["CR-mob", "SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["CR-mob", "SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["CR-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "DB-mob"]),
    weapon: cited(WeaponType::Physical, &["SW-mob", "DB-mob"]),
    // SW-mds's pass and shot totals (and OB, which copies them) imply 510 (CONTRADICTION);
    // DragonBound agrees with CR-mob.
    base_delay: cited(520, &["CR-mob", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // The heaviest shot on CR-fixed's chart.
    gravity: fitted(
        GRAVITY,
        &["CR-fixed", "CR-nak"],
        "60 degrees over its back lands one screen at 2.66 bars, zero wind, flat ground: midway between CR-fixed's 2.70 and the Nak formula's 2.62.",
    ),
    // DBR's wind coefficient (ay 0.867 against Armor's 0.74, clone) points the other way
    // (CONTRADICTION).
    wind_sensitivity: fitted(
        WIND_SENSITIVITY,
        &["CR-nak", "WP06-list"],
        "Its very heavy shot is less affected by wind (WP06-list): CR-nak's vertical wind term, 14 up-wind at 60 degrees over one screen.",
    ),
    move_budget: picked(
        60,
        "Very low mobility (SW-mob, DB-mob), with no number anywhere: two thirds of everyone's 90 (OB), below Spoutpot's poor 70.",
    ),
    climb_px: picked(
        10,
        "High climbing ability (SW-mob, DB-mob): a 1 px step may rise 10 px, slopes up to about 84 degrees, where everyone else's 6 takes about 80.",
    ),
    hitbox: picked(
        Rect { x: -30.0, y: -46.0, w: 62.0, h: 46.0 },
        "Augerdillo's drawn rig, shell and head. The rider, the ears, the snout, the tail and the mortar are cosmetic.",
    ),
    ground_probes: picked([-20.0, 16.0], "Augerdillo's drawn rear and front road wheels."),
    cannon_pivot: picked((-15.0, -48.0), "Augerdillo's drawn mortar hub, in its yoke raised on the back of the shell."),
    barrel_length: picked(26.0, "Augerdillo's drawn mortar, hub to mouth."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds", "DB-mob"]),
        aim: AIM,
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mds", "DB-mob"]),
        aim: cited(
            AimRange {
                min: 130,
                max: 170,
                true_min: 130,
                true_max: 170,
            },
            &["CR-mob"],
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 737.6;

/// Fitted by `tool_fit`, which rewrites this line.
const WIND_SENSITIVITY: f64 = 0.67;

/// S1: a regular round (SW-mob, DB-mob).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 100, max: 150 }, &["SW-mob", "DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(
    20.0,
    "DBR digs Nak's S1 a crater 38 by 41, about Armor's S2 warhead's (34 by 24) and far short of its S1's (60 by 42): Coilback's S2 warhead's 20.",
);

/// S2: a drill that "digs into the ground upon contact, and returns to the surface where it
/// detonates" (SW-mob). It strikes a mobile it meets in flight as a round does: players drop it on
/// top of a target close by (WP06-list). From the ground it "resurfaces further in a reverse
/// parabolic arc of its original path through the air", pulled up by a "reverse gravity", and "the
/// faster the shot is going when it hits the ground, the further underground it will travel"
/// (WP06-list): it bores on down at the angle and speed it struck, the way it was heading, under
/// its gravity turned upward, and goes off at the first open pixel it reaches. DBR's clone does the
/// same with the drill's launch speed (CONTRADICTION; GunBound first). No wind reaches it in the
/// ground (DBR), and no weather column does either (our pick: it is in the rock). One that leaves
/// the map first is lost.
pub const S2_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 190, max: 300 }, &["SW-mob", "DB-mob"]);
pub const S2_BLAST: Tuned<f64> = picked(20.0, "DBR digs S2's crater as S1's (38 by 41), about Armor's S2 warhead's (34 by 24): Coilback's S2 warhead's 20.");
/// The bore's speed as a share of the speed the drill struck the ground at.
pub const S2_BURROW_SPEED: Tuned<f64> = cited(1.0, &["WP06-list"]);
/// The upward pull on the bore as a share of the shot's gravity: a mirror of its arc (WP06-list),
/// and DBR's digger weighs -360 to the drill's 360.
pub const S2_BURROW_LIFT: Tuned<f64> = cited(1.0, &["WP06-list", "DBR"]);

/// SS: a mini armadillo that "ignores the land, going right through it" and "explodes only when it
/// hits a player (whether ally or enemy)" (SW-mob); it flies until it hits one or leaves the map
/// (WP06-list), as heavy as S1 (WP06-list; CDW-pt says heavier, CONTRADICTION). SW-mob counts
/// corpses and CDW-pt doesn't (CONTRADICTION); lobshot clears the dead from the map. WP06-list puts
/// its most at about 300 (CONTRADICTION). It "won't do much damage unless you are lucky and
/// you hit the mobile's 'weak spot'" (SW-mob, DB-mob), which no source places; a low shot hits it
/// "doing the most dmg" (CR-kex). Our pick reads it as low on the body: its blast burns low
/// (`damage::low`), most at a target's feet and least at the top of its body, where a lob strikes.
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 350, max: 500 }, &["SW-mob", "DB-mob"]);
pub const SS_BLAST: Tuned<f64> = picked(25.0, "\"A bit larger explosion radius\" than S1's (WP06-list); DBR's crater is larger still (60 by 63).");
