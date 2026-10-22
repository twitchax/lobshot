//! Sudsotter: an otter in a bathtub buggy that fires soap sponges and suds, with GunBound J.Frog's
//! behavior. J.Frog came late (GunBound World Champion), so its sources are fewer and thinner than
//! the Classic mobiles'; `docs/research/sudsotter.md` gathers them. Geometry (hitbox, probes,
//! cannon) matches its drawing in `assets/mobiles/sudsotter/rig.ron`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, lanternwing, picked};

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Sudsotter",
    behaves_like: "J.Frog",
    hp: cited(930, &["SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Bionic, &["SW-mob", "DB-mob", "SNX-frog"]),
    // WP06 counts J.Frog among the Hit (Physical) types (CONTRADICTION).
    weapon: cited(WeaponType::Laser, &["SW-mob", "DB-mob", "SNX-frog"]),
    // CR-bot gives 430 (and CR-frog "super low... like 670" for a shot); StrategyWiki and
    // DragonBound agree on 500 (CONTRADICTION; the fidelity rule takes DragonBound's side).
    base_delay: cited(500, &["SW-mob", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // DBR gives J.Frog Grub's coefficient (ax 61, clone, unverified), which would make it as heavy
    // as Marblemole's 2.33 (CONTRADICTION): CR-frog calls the shots "very very light... lighter
    // even than boomer's".
    gravity: fitted(
        GRAVITY,
        &["CR-frog"],
        "60 degrees lands one screen at 2.1 bars (CR-frog's cbchui-style figure), zero wind, flat ground.",
    ),
    wind_sensitivity: cited(1.0, &["CR-frog"]),
    move_budget: cited(90, &["OB"]),
    hitbox: picked(
        Rect { x: -35.0, y: -60.0, w: 66.0, h: 60.0 },
        "Sudsotter's drawn tub and the otter in it, shower cap included. The rider, the towel, the drifting bubbles and the claw feet's tips are cosmetic.",
    ),
    ground_probes: picked([-17.0, 15.0], "Sudsotter's drawn wheels, under the claw feet."),
    cannon_pivot: picked((28.0, -39.0), "Sudsotter's drawn tap valve, on the tub's front rim ahead of the otter."),
    barrel_length: picked(28.0, "Sudsotter's drawn tap, valve to the sponge at its spout."),
    s1: ShotTuning {
        // CR-bot agrees.
        delay: cited(250, &["SW-mob", "DB-mob", "CR-bot"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(250, &["SW-mob", "DB-mob", "CR-bot"]),
        aim: AIM,
    },
    ss: ShotTuning {
        // CR-bot gives 960, "(not sure)" (CONTRADICTION).
        delay: cited(800, &["SW-mob", "DB-mob"]),
        aim: AIM,
    },
};

/// Every shot's aim. No GunBound source gives J.Frog's angles (OB marks them "NEED SOURCE"); DBR's
/// clone does, with a note that they are still to be settled. Nothing gives a false angle.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 30,
        max: 60,
        true_min: 30,
        true_max: 60,
    },
    &["DBR"],
);

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 466.6;

/// Whether the weather leaves its shots' damage alone: J.Frog "is not affected by Thor, Thunder, or
/// any of the other weathers that affect damage" (WP06), so Force, Drain and Electricity's bolt do
/// nothing to them; Mirror and Tornado still turn them in flight. CR-frog says the opposite, "don't
/// be scared to use force/lightning/thor with frog" (CONTRADICTION).
pub const WEATHER_PROOF: Tuned<bool> = cited(true, &["WP06"]);

// ── Walking ──────────────────────────────────────────────────────────────────

/// How fast a sponge walks along level ground, px/s: on flat ground "I plant the shot about 1/7th
/// of a screen away from the intended target - about 4 parts out of a 30-part screen" (CR-frog),
/// for its 3 s walk.
pub const WALK_SPEED: Tuned<f64> = fitted(800.0 * 4.0 / 30.0 / 3.0, &["CR-frog"], "3 s of walking on flat ground covers 4 parts of a 30-part screen.");
/// Its speed up and down walls, as a share of `WALK_SPEED`: it "moves up and down a bit slower than
/// it moves horizontally" (CR-frog).
pub const CLIMB_SHARE: Tuned<f64> = picked(0.75, "A bit slower (CR-frog): three quarters.");
/// Its speed under a mobile, as a share: "when traveling under a mobile, the rolling shot tends to
/// slow down, so it handles easier than it looks" (SW-mob).
pub const UNDER_MOBILE_SHARE: Tuned<f64> = picked(0.5, "Half: a sponge that reaches its target early lingers under it.");

// ── S1 and S2 ────────────────────────────────────────────────────────────────

/// S1 and S2: a sponge that flies through mobiles, sticks where it lands and walks, S1 left
/// (counterclockwise around the ground) and S2 right (clockwise), on screen, whichever way the
/// shooter faces (WP06, SW-mob, CR-frog); it climbs walls and clings under ledges, ignores the
/// weather once down (CR-frog, WP06), and goes off after `SPONGE_FUSE_SECONDS`, wherever it is.
pub const SPONGE_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 150, max: 230 }, &["SW-mob", "DB-mob"]);
/// "A pretty nice hole like mage shot 1" (CR-frog).
pub const SPONGE_BLAST: Tuned<f64> = picked(lanternwing::S1_BLAST.value, "Lanternwing's S1 crater: \"a pretty nice hole like mage shot 1\" (CR-frog).");
/// From landing to going off: "rolling to the left for three seconds and then explodes" (SW-mob).
pub const SPONGE_FUSE_SECONDS: Tuned<f64> = cited(3.0, &["SW-mob", "DB-mob", "CR-frog"]);

// ── SS ───────────────────────────────────────────────────────────────────────

/// SS: a big blob of suds. A direct hit on a mobile in flight goes off at once, "doing little
/// damage" (SW-mob); on the ground it walks the way it was moving as it landed, so a wind that
/// turned it round turns its walk round too (SW-mob, WP06), climbing walls like the sponges, and
/// hurts every mobile within `SUDS_REACH` all the while (SW-mob, SNX-frog), until it goes off.
/// `min` is the most its blast deals; touching deals up to `max - min` more to each mobile.
pub const SUDS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 100, max: 700 }, &["SW-mob", "DB-mob"]);
/// The blast's own range, direct hit or near miss: little damage, topping at `SUDS_DAMAGE.min`.
pub const SUDS_BLAST_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 70, max: 100 }, "Little damage (SW-mob), up to the bottom of its 100-700.");
pub const SUDS_BLAST: Tuned<f64> = picked(20.0, "A small pop, not a crater: the suds do their work by touch.");
/// How close to a hitbox the suds hurt it: "within about 1/2-1cm of the mobile" (WP06), on an
/// 800 px screen about 30 cm wide.
pub const SUDS_REACH: Tuned<f64> = picked(20.0, "WP06's 1/2 to 1 cm is 13 to 27 px on an 800 px screen about 30 cm wide: the middle.");
/// Touch damage per second: a pass under a mobile on flat ground deals about 300 (GBM-frog, a later
/// version, has 300-350), and a speck of land it circles the whole time about 500 (CR-frog's
/// "typical SS damage: ~400-500").
pub const SUDS_TOUCH_PER_SECOND: Tuned<f64> = picked(60.0, "About 300 for a pass under a mobile and 500 for the whole walk against one.");
/// From landing to going off: it "just keeps walking for a long time" (CR-frog), long enough to
/// circle "a floating speck of land" and pass through its target "several times".
pub const SUDS_FUSE_SECONDS: Tuned<f64> = picked(8.0, "Long enough to circle a small island a couple of times at the walking speed.");
