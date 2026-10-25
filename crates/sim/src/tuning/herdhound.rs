//! Herdhound: a sheepdog mech that herds woolly missiles toward its whistle, with GunBound
//! Kalsiddon's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/herdhound/rig.ron`.
//!
//! Every shot is a warhead that passes through mobiles and opens at its apex, as it starts to
//! fall: a guide (the whistle) flies on along the shot's path, and missiles burst out, loop for a
//! second, then home on the guide and hit wherever it lands (SW-mob, CR-kal, KAL-snx). A warhead
//! that touches the ground before it opens is a dud. Where the sources come from, and what they
//! leave open: `docs/research/herdhound.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// Kalsiddon's angles: "sort of like nak's old angle range" (CR-kal), which is 10-50 with 15-45
/// true (CR-mob). No source gives Kalsiddon's own, and DBR's Kalsiddon is an unfinished copy of
/// its Frog.
const AIM: Tuned<AimRange> = picked(
    AimRange {
        min: 10,
        max: 50,
        true_min: 15,
        true_max: 45,
    },
    "Nak's GunBound range (CR-mob), which CR-kal likens Kalsiddon's to; no source gives its own.",
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Herdhound",
    behaves_like: "Kalsiddon",
    hp: cited(1100, &["SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "DB-mob", "AK"]),
    // AK and Softnyx's own text (KAL-snx) call it electric (CONTRADICTION).
    weapon: cited(WeaponType::Explosion, &["SW-mob", "DB-mob"]),
    // CR-bot and OB say 510 (CONTRADICTION); DragonBound agrees with SW-mob.
    base_delay: cited(515, &["SW-mob", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    // KAL-dlh claims a -20 item delay (single source, unconfirmed).
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds); CR-kal says Kalsiddon's duals cost the usual delay."),
    gravity: fitted(
        GRAVITY,
        &["CR-kal", "CR-kal30"],
        "60 degrees lands one screen at 2.675 bars, zero wind, flat ground: midway between CR-kal's 2.65 and CR-kal30's 2.7.",
    ),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -33.0, y: -54.0, w: 64.0, h: 54.0 },
        "Herdhound's drawn coat and the back of its head. The rider, the crook, the wind-up key and the snout are cosmetic.",
    ),
    ground_probes: picked([-19.0, 17.0], "Herdhound's drawn rear and front wheels."),
    cannon_pivot: picked((20.0, -40.0), "Herdhound's drawn neck joint, at the front of its coat."),
    barrel_length: picked(27.0, "Herdhound's drawn head, neck joint to the woolly missile in its jaws."),
    s1: ShotTuning {
        delay: cited(250, &["SW-mob", "CR-bot", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(460, &["SW-mob", "CR-bot", "DB-mob"]),
        aim: AIM,
    },
    ss: ShotTuning {
        // SW-mob, DB-mob and OB say 800 (CONTRADICTION). Balance (PRD-0006 T-010) takes CR-bot's:
        // Herdhound led at both levels.
        delay: cited(880, &["CR-bot"]),
        // Nak's SS range is all true (CR-mob).
        aim: picked(
            AimRange {
                min: 10,
                max: 50,
                true_min: 10,
                true_max: 50,
            },
            "Nak's GunBound SS range (CR-mob), which CR-kal likens Kalsiddon's to; no source gives its own.",
        ),
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 748.5;

// ── Every shot ───────────────────────────────────────────────────────────────

/// How long the missiles loop once the warhead opens, before they home on their guides: "after
/// another second" (SW-mob), "for a second" (CR-kal). Rounded down to whole steering steps.
pub const LOOP_SECONDS: Tuned<f64> = cited(1.0, &["SW-mob", "CR-kal"]);

/// Whole turns each missile loops in that time: they "circle around twice" (KAL-gbomb).
pub const LOOP_TURNS: Tuned<f64> = cited(2.0, &["KAL-gbomb"]);

/// A missile's speed, px/s. Missiles are rockets: neither gravity nor wind moves them.
pub const MISSILE_SPEED: Tuned<f64> = picked(
    450.0,
    "Loops about 72 px across at two a second, so an apex lower than that over the ground or a mobile crashes them, \
     as close, flat shots do (SW-mob, CR-kal); and a missile reaches a guide a screen off in under two seconds. \
     Balance (PRD-0006 T-010): 360 let slow missiles settle onto the guide every time.",
);

/// How fast a homing missile may turn toward its guide, radians per second.
pub const HOMING_TURN: Tuned<f64> = picked(
    3.0,
    "Turns a missile around in about a second: \"partial homing towards the tracer\" (KAL-dlh), so it swings wide after \
     its loops and comes in on the guide at an angle (CR-pb218), and a guide that lands beside a target scatters them \
     about it rather than onto it. Balance (PRD-0006 T-010): at 6, missiles landed on the guide every time, which made \
     Herdhound the strongest mobile at Normal and the second at Hard.",
);

/// A missile that has hit nothing this long after it burst out goes off where it is.
pub const MISSILE_SECONDS: Tuned<f64> = picked(6.0, "A guard: a missile reaches its guide in about three seconds at most.");

/// A warhead that touches the ground before it opens goes off as a dud: "puny damage" (CR-kal),
/// but it carves the ground (KAL-snx).
pub const DUD_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 40, max: 60 }, "Puny next to an S1's 150-220 (CR-kal).");
pub const DUD_BLAST: Tuned<f64> = picked(28.0, "A whole warhead's crater: it destroys terrain (KAL-snx).");

// ── S1 ───────────────────────────────────────────────────────────────────────

/// S1: two missiles and a guide (SW-mob).
pub const S1_MISSILE_DAMAGE: Tuned<DamageRange> = picked(
    DamageRange { min: 64, max: 94 },
    "128-188 for the two: KAL-gma's typical 120-190 and inside CR-kal's 150-200 at the top, under SW-mob's 150-220. \
     Balance (PRD-0006 T-010): from SW-mob's split evenly (75-110), when Herdhound led at Normal.",
);
pub const S1_MISSILE_BLAST: Tuned<f64> = picked(24.0, "Missiles of \"decent damage and bunge\" (KAL-dlh).");
/// Each missile's heading as it bursts out, in degrees off the warhead's, counterclockwise on
/// screen; it loops that way round.
pub const S1_BURST: Tuned<[f64; 2]> = picked([50.0, -50.0], "A figure eight behind the opening point: the two loop away from each other.");

// ── S2 ───────────────────────────────────────────────────────────────────────

/// S2: four missiles and a guide (SW-mob).
pub const S2_MISSILE_DAMAGE: Tuned<DamageRange> = picked(
    DamageRange { min: 54, max: 85 },
    "216-340 for the four: KAL-gma's typical 220-340 and CR-kal's 230-350, under SW-mob's 250-400 (about 50-120 \
     each). Balance (PRD-0006 T-010): from SW-mob's split evenly (63-100), when Herdhound led at Normal.",
);
pub const S2_MISSILE_BLAST: Tuned<f64> = picked(20.0, "Four smaller blasts.");
pub const S2_BURST: Tuned<[f64; 4]> = picked([80.0, 30.0, -30.0, -80.0], "Two loops above the opening point and two below, each pair out of step.");

// ── SS ───────────────────────────────────────────────────────────────────────

/// SS: one huge warhead opens into four missiles, which split into eight as they stop looping,
/// each homing on a guide of its own (SW-mob, AK).
pub const SS_MISSILE_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 38, max: 81 }, "SW-mob's 300-650 for the eight, split evenly.");
pub const SS_MISSILE_BLAST: Tuned<f64> = picked(18.0, "Eight small blasts.");
pub const SS_BURST: Tuned<[f64; 4]> = picked([80.0, 30.0, -30.0, -80.0], "As S2's four.");
/// The halves' headings off their missile's as it splits, degrees.
pub const SS_SPLIT: Tuned<[f64; 2]> = picked([25.0, -25.0], "A visible fork, both halves still heading the way it was.");
/// The eight guides' sideways speed off the shot's own, px/s, as the warhead opens: they fly
/// alongside its path and land spread about where it would ("each guide following the main
/// guide", SW-mob). Guide `i` leads the missile that splits off `i`-th.
pub const SS_GUIDE_SPREAD: Tuned<[f64; 8]> = picked(
    [-105.0, -75.0, -45.0, -15.0, 15.0, 45.0, 75.0, 105.0],
    "About a mobile's width apart after a typical fall: a dead-center SS lands three or four on the target, \
     the \"pin-point precision\" its full damage needs (SW-mob).",
);
