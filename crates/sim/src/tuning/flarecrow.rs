//! Flarecrow: a crow signalman in a flare-mortar cart that fires over its shoulder, with GunBound
//! Aduka's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/flarecrow/rig.ron`.
//!
//! It fires from the rear (SW-mob, DBR): its aims run past 90, over its back. S1 is an electric
//! ball whose aftershock runs from mobile to mobile (CR-aduka). Its S2 flares and its SS call
//! Skyeye's beam (GunBound's Thor) down onto whatever they light, with or without Skyeye's weather,
//! and every beam's damage levels the one satellite up (GF-lee, CR-aduka). Where the sources come
//! from, and what they leave open: `docs/research/flarecrow.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// Aduka's 50-10, all of it true (CR-mob, CR-aduka), over its back: 130-170 above its forward
/// direction. DBR's clone gives 110-170, 10-70 backward (CONTRADICTION).
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 130,
        max: 170,
        true_min: 130,
        true_max: 170,
    },
    &["CR-mob", "CR-aduka", "SW-mob"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Flarecrow",
    behaves_like: "Aduka",
    hp: cited(1000, &["SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "DB-mob"]),
    // "Electricity/Laser" (SW-mob, DB-mob): S1 is "an electrical type shot" (CR-aduka), and the
    // beams its other shots call are Skyeye's laser (`weather::SKYEYE_WEAPON`).
    weapon: cited(WeaponType::Electric, &["SW-mob", "DB-mob", "CR-aduka", "SNX-aduka"]),
    base_delay: cited(510, &["CR-mob", "SW-mds", "SW-mob", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(
        GRAVITY,
        &["CR-fixed", "CR-aduka", "GN-aduka"],
        "60 degrees lands one screen at 2.375 bars, zero wind, flat ground: midway between the 2.35 of CR-fixed's chart \
         and CR-aduka's table and the 2.40 of GN-aduka's table (CR-aduka: \"for 1 full screen I use a tiny bit more power, like 2.4\").",
    ),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    // CR-aduka calls its movement short, SW-mob average (CONTRADICTION); neither gives a number.
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -32.0, y: -53.0, w: 64.0, h: 53.0 },
        "Flarecrow's drawn cart and the crow's body. The rider, the mortar, the crow's head and cap and the lantern are cosmetic.",
    ),
    ground_probes: picked([-18.0, 18.0], "Flarecrow's drawn cart wheels."),
    cannon_pivot: picked((-27.0, -36.0), "Flarecrow's drawn mortar swivel, on the cart's tail post."),
    barrel_length: picked(24.0, "Flarecrow's drawn flare mortar, swivel to the flare's cap."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "SW-mob", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds", "SW-mob", "DB-mob"]),
        aim: AIM,
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mds", "SW-mob", "DB-mob"]),
        aim: AIM,
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 591.5;

// ── S1 ───────────────────────────────────────────────────────────────────────

/// S1: an electric ball (SW-mob: "an electrical purple ball. Deals a weak aftershock"; CR-aduka:
/// like JD's). Its blast is the direct part; the aftershock comes on top.
pub const S1_DAMAGE: Tuned<DamageRange> = fitted(
    DamageRange { min: 70, max: 90 },
    &["SW-mob", "DB-mob", "CR-aduka"],
    "With the aftershock on top, a direct hit deals SW-mob's and DB-mob's 140-160 (\"includes aftershock\"); CR-aduka's typical 160.",
);
/// "An unremarkable small hole if it misses" (CR-aduka).
pub const S1_BLAST: Tuned<f64> = picked(18.0, "A small hole (CR-aduka), smaller than the common 25 (OB).");

/// The aftershock: a flat hit on every mobile near the landing, the one struck included, once a
/// turn (splash, GF-max): "about 70 points" (CR-aduka).
pub const S1_AFTERSHOCK: Tuned<i32> = cited(70, &["CR-aduka"]);
/// How far from the landing the aftershock reaches a hitbox.
pub const S1_AFTERSHOCK_REACH: Tuned<f64> = picked(
    75.0,
    "\"The splash effect is large, so if Aduka misses completely the splash can still hit\" (CR-aduka): three times a \
     bolt's reach (OB). Balance (PRD-0006 T-010): from twice.",
);
/// Each mobile the aftershock hits passes a weaker one on to its neighbors, and they to theirs:
/// "BOB will get some splash damage from you (much less, 15-30 points)" (CR-aduka), down "a line
/// of metal bots" in close quarters.
pub const S1_CHAIN: Tuned<i32> = picked(22, "The middle of CR-aduka's 15-30.");
/// How far from a mobile's center the aftershock passes on: to a mobile "right next to" it
/// (CR-aduka).
pub const S1_CHAIN_REACH: Tuned<f64> = picked(
    60.0,
    "Hitboxes are about 64 px wide, so this reaches about 28 px past a mobile's own edge: one standing beside it, not one a step away.",
);

// ── S2 ───────────────────────────────────────────────────────────────────────

/// S2: three tracer flares down one path, one after another; wherever each lands, Skyeye beams at
/// it, with or without its weather and no more during it (SW-mob, CR-aduka, GF-lee). The flares
/// themselves hurt and carve nothing: "entirely Thor damage, with no base damage of its own"
/// (CR-aduka). SW-mob's 140-500 is the three beams from level 1 up. The beams are Skyeye's own, so
/// Force, Drain and items leave them alone: "no added damage from force" (CR-aduka), no "stat
/// bonuses" (SNX-aduka).
pub const S2_FLARES: Tuned<u32> = cited(3, &["SW-mob", "CR-aduka", "DB-mob"]);
/// Seconds between one flare leaving and the next.
pub const S2_FLARE_SECONDS: Tuned<f64> = picked(
    0.3,
    "Each flare follows the one before and flies past where its beam dug, so a short shot drills toward the target \
     (CR-aduka); a beat longer than a beam's delay, so each beam strikes before the next flare arrives.",
);

// ── SS ───────────────────────────────────────────────────────────────────────

/// SS: a mini flare-cart that flies through land and mobiles alike and never goes off; each time
/// it passes into an enemy, Skyeye beams at that enemy, so it can light several, or one twice
/// through the weather (SW-mob, CR-aduka). Teammates it passes take nothing (CR-aduka).
///
/// Its beams hit harder than Skyeye's own: SW-mob's and DB-mob's SS 80-250 over S2's 140-500 for
/// three beams puts one SS beam at about 1.7 times an S2 beam at level 1 (80 over 47) and 1.5 at
/// the top (250 over 167).
pub const SS_BEAM_BOOST: Tuned<f64> = fitted(
    1.6,
    &["SW-mob", "DB-mob"],
    "SS beams at about 1.6 times Skyeye's at the same level: at level 1, 80 at a hitbox's edge, SW-mob's minimum. \
     CONTRADICTION: CR-aduka guesses 200-250 a beam even at a low level.",
);
