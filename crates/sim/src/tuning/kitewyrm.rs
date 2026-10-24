//! Kitewyrm: a gecko flying a festival paper-dragon kite over a little reel cart, with GunBound
//! Dragon's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/kitewyrm/rig.ron`. What each source says about Dragon, and what lobshot picked
//! where they are silent: `docs/research/kitewyrm.md`.
//!
//! It floats (`FLOATS`): its body hovers `HOVER` px over the ground, so what rolls or walks along
//! the ground passes under it. S1 is one light fireball; S2 four, rippling out in two streams that
//! spread with range; the SS a red orb that calls five mini-dragons down from a fan above where it
//! lands, each flying into that spot and going off on the first thing in its way.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// Dragon's angles: 60 to 30, all of them true (CR-mob, OB); "angle range: 30 degrees" for every
/// shot (GF-lee). DBR's 15-50 is its Mage's, copied into an entry marked unfinished.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 30,
        max: 60,
        true_min: 30,
        true_max: 60,
    },
    &["CR-mob", "OB", "GF-lee"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Kitewyrm",
    behaves_like: "Dragon",
    // DragonBound lists 1250 (DB-mob; CONTRADICTION).
    hp: cited(1000, &["SW-mob", "CR-mob", "OB"]),
    shield: cited(0, &["SW-mob", "DB-mob", "OB"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob", "OB"]),
    armor: cited(ArmorClass::Bionic, &["SW-mob", "DB-mob", "GF-lee"]),
    weapon: cited(WeaponType::Explosion, &["SW-mob", "DB-mob", "GF-lee"]),
    // OB's delay table gives Dragon 710 / 760 / 910 / 1310, Nak's and Aduka's (CONTRADICTION).
    base_delay: cited(550, &["CR-mob", "SW-mob", "DB-mob", "GF-lee"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // CR-fixed's chart predates Dragon. Its shots are "lightweight" (SW-mob) and GF-lee weighs all
    // three Light, as it weighs only JD's and Lightning's: the chart puts both at 2.30. DBR's shot
    // weight (`ax`) for Dragon is its Mage's, in an entry marked unfinished, so it says nothing.
    gravity: fitted(
        GRAVITY,
        &["CR-fixed", "GF-lee", "SW-mob"],
        "60 degrees lands one screen at 2.30 bars, zero wind, flat ground: CR-fixed's power for JD and Lightning, the shots GF-lee also calls Light.",
    ),
    // DBR's wind coefficient (ay 1.78 against its Mage's 0.78) sits in the same unfinished entry.
    wind_sensitivity: cited(1.4 / 1.2, &["OB"]),
    move_budget: picked(
        150,
        "\"Very high mobility\" (SW-mob), \"a fairly long walking distance\" (GF-lee): GF-lee's top movement rating, 5 of 5, a step past Bigfoot's 4 (Tuskrack's 120) as that is past Armor's 3 (everyone's 90, OB).",
    ),
    climb_px: picked(
        18,
        "It \"can climb just about anything that isn't sloped inwards\" (GF-lee; \"very steep terrain\", WP06, AK): GF-lee's top climbing rating, 5 of 5, at 4 px a step from Bigfoot's 1 (Tuskrack's 2 px) and Armor's 2 (everyone's 6, OB).",
    ),
    hitbox: picked(
        Rect {
            x: -35.0,
            y: -60.0,
            w: 70.0,
            h: 60.0 - HOVER.value,
        },
        "Kitewyrm's drawn kite, from its tail's root to the back of its head, floating `HOVER` over the ground. The rider, the gecko, the horns, the snout, the tail and its ribbons, the string and the reel cart are cosmetic.",
    ),
    ground_probes: picked([-12.0, 12.0], "Kitewyrm's drawn cart wheels."),
    cannon_pivot: picked((20.0, -40.0), "Kitewyrm's drawn neck joint, where the kite's head turns."),
    barrel_length: picked(30.0, "Kitewyrm's drawn head, neck joint to the mouth."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mob", "DB-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mob", "DB-mob", "GF-lee"]),
        aim: AIM,
    },
    ss: ShotTuning {
        // DB-mob says 800 (CONTRADICTION); GF-lee's 1390 is 550 + 840.
        delay: cited(840, &["CR-mob", "SW-mob", "GF-lee"]),
        aim: AIM,
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 555.5;

// ── Floating ─────────────────────────────────────────────────────────────────

/// Whether it floats over the ground: it "can fly over raon mines" and "cannot be directly hit by:
/// rolling Grub's Shot 2 (NOTE: can be hit by shot2 if it bounces into dragon), J.Frog's shots,
/// Nakmachine's Shot 2" (SW-mob). What travels along the ground, walking, rolling or burrowing, finds
/// nothing of it to touch (`MobileKind::floats`), and blasts there reach it only across `HOVER`
/// ("Frog shots can only hit if timed precisely, as the explosion will just blow away the ground
/// under Dragon", WP06). It still walks, and falls, on the ground like anyone (OB).
pub const FLOATS: Tuned<bool> = cited(true, &["SW-mob", "WP06", "AK", "GF-lee"]);

/// How far over the ground its body floats, px: the hitbox's bottom edge. OB's Dragon hitbox ends 16
/// px above the ground, where every other mobile's reaches it (to 5 px), so a marble rolling along
/// passes under and one bouncing into its belly strikes it (SW-mob).
pub const HOVER: Tuned<f64> = cited(16.0, &["OB"]);

// ── S1 ───────────────────────────────────────────────────────────────────────

/// S1: "a single, powerful fireball. It is lightweight" (SW-mob); "deceptively light but bunges only
/// as lightly as other standard weapon 1 types" (GF-lee).
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 150, max: 250 }, &["SW-mob", "DB-mob"]);
pub const S1_BLAST: Tuned<f64> = cited(25.0, &["OB"]);

// ── S2 ───────────────────────────────────────────────────────────────────────

/// S2: four fireballs "that each do around 100 damage" (SW-mob, GF-cyber), in "two parallel
/// streams of two" that "spread out very fast" (GF-lee): "impossible to get all 4 fireballs to hit
/// an opponent more than a screen away" (SW-mob).
pub const S2_FIREBALL_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 75, max: 125 }, "SW-mob's and DB-mob's 300-500 for the four, split evenly.");
pub const S2_FIREBALL_BLAST: Tuned<f64> = cited(25.0, &["OB"]);
/// Each fireball's aim (degrees) and power (bars) off the shot's, in the order they leave: one
/// stream 3 degrees low with more power (OB's +5 and +10 of its 100), the other 3 degrees high.
pub const S2_SPREAD: Tuned<[(i16, f64); 4]> = cited([(-3, 0.2), (3, 0.0), (-3, 0.4), (3, 0.0)], &["OB"]);
/// How long after each fireball the next leaves the mouth.
pub const S2_RIPPLE_SECONDS: Tuned<f64> = cited(0.2, &["OB"]);

// ── SS ───────────────────────────────────────────────────────────────────────

/// SS: "a red orb" (SW-mob), a tracer (GF-lee, GF-cyber), that "itself deals minor damage" (SW-mob).
pub const SS_ORB_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 20, max: 40 }, "Minor damage (SW-mob): a tenth of S1's.");
pub const SS_ORB_BLAST: Tuned<f64> = picked(15.0, "A tracer's small scorch, not a bunging blast.");

/// Where the mini-dragons appear around the orb's landing: degrees off straight up, toward the
/// way the shooter faces, in the order they appear. "Five mini-dragons appear over the landing spot
/// in something of a half circle" (GF-lee; five in WP06, GF-cyber, DB-mob and OB). SW-mob and AK
/// say four (CONTRADICTION): five sources against two.
pub const SS_FAN: Tuned<[f64; 5]> = cited([-40.0, -20.0, 0.0, 20.0, 40.0], &["OB", "GF-lee", "WP06", "GF-cyber"]);
/// How far from the landing they appear, px.
pub const SS_DISTANCE: Tuned<f64> = cited(300.0, &["OB"]);
/// How long after one mini-dragon appears the next does.
pub const SS_STAGGER_SECONDS: Tuned<f64> = cited(0.3, &["OB"]);
/// How long each hovers where it appeared before it flies: OB's 18-frame spawning animation at 20
/// frames a second.
pub const SS_HOVER_SECONDS: Tuned<f64> = cited(0.9, &["OB"]);
/// A mini-dragon's speed as it sets off, px/s, straight for the landing: neither gravity nor wind
/// moves it (OB).
pub const SS_DRAGON_SPEED: Tuned<f64> = cited(100.0, &["OB"]);
/// Its acceleration along that line, px/s²: OB's 200, doubled by OB's integration (`openbound.md`,
/// 3.1), so it arrives in OB's one second.
pub const SS_DRAGON_ACCEL: Tuned<f64> = cited(400.0, &["OB"]);
/// Each mini-dragon goes off on the first thing in its way, mobile or ground (GF-cyber): land above
/// the target blocks them (SW-mob, WP06).
pub const SS_DRAGON_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 90, max: 130 }, "SW-mob's and DB-mob's 450-650 for the five, split evenly (GF-cyber: 100 each).");
pub const SS_DRAGON_BLAST: Tuned<f64> = cited(25.0, &["OB"]);
