//! Tinbadger: a badger tinker pushing a crate-cart full of wind-up tin beetles, with GunBound
//! Raon's behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/tinbadger/rig.ron`. Where the sources come from, and what they leave open:
//! `docs/research/tinbadger.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// Every shot's aim: 10 to 50, all of it true (CR-mob gives no true range for Raon, and OB keeps
/// the whole range true). DBR's clone has 30-60 (CONTRADICTION); GunBound's table comes first.
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 10,
        max: 50,
        true_min: 10,
        true_max: 50,
    },
    &["CR-mob", "OB"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Tinbadger",
    behaves_like: "Raon",
    // CR-mob gives 1050 and SW-mob 1000 (CONTRADICTION): DragonBound and OB side with SW-mob.
    hp: cited(1000, &["SW-mob", "DB-mob", "OB"]),
    shield: cited(0, &["SW-mob", "DB-mob"]),
    shield_regen: cited(0, &["SW-mob", "DB-mob"]),
    armor: cited(ArmorClass::Mechanical, &["SW-mob", "DB-mob"]),
    // GunBound's sources disagree: GF-lee (2003) has Laser, SW-mob Explosion (CONTRADICTION), so
    // DragonBound's, which agrees with GF-lee.
    weapon: cited(WeaponType::Laser, &["GF-lee", "DB-mob", "DB-img-types"]),
    base_delay: cited(500, &["CR-mob", "SW-mds", "DB-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.50 bars, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -34.0, y: -52.0, w: 70.0, h: 52.0 },
        "Tinbadger's drawn badger, goggles included, and its crate cart. The rider, the tail, the beetles' keys and the launcher are cosmetic.",
    ),
    ground_probes: picked([-18.0, 28.0], "Tinbadger's drawn front foot and the cart's front wheel."),
    cannon_pivot: picked((30.0, -38.0), "Tinbadger's drawn launcher hinge, on the crate's front corner."),
    barrel_length: picked(24.0, "Tinbadger's drawn launcher, hinge to the beetle in its bell."),
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
        aim: AIM,
    },
    second_barrel: None,
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 654.6;

// ── S1 ───────────────────────────────────────────────────────────────────────

/// S1: spinning cogs braided about one path, each its own hit, that "remove a lot of land"
/// (SW-mob). GunBound has three (SW-mob, DB-mob); OB braids four.
pub const S1_COGS: Tuned<usize> = cited(3, &["SW-mob", "DB-mob"]);
/// How far each cog swings off the path, px, and how fast they wind about it.
pub const S1_AMPLITUDE: Tuned<f64> = cited(12.0, &["OB"]);
pub const S1_HERTZ: Tuned<f64> = cited(1.0, &["OB"]);
/// Each cog's damage: SW-mob's and DB-mob's 100-160 for the three, split evenly.
pub const S1_COG_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 34, max: 54 }, "SW-mob's and DB-mob's 100-160 for the three cogs, split evenly.");
/// Each cog's reach: OB's tight r15.
pub const S1_COG_BLAST: Tuned<f64> = cited(15.0, &["OB"]);
/// Each cog's crater: wider than its reach, as the cogs "remove a lot of land" (SW-mob).
pub const S1_COG_CRATER: Tuned<f64> = picked(26.0, "Three cogs together cut a hole wider than any one S1 shell: a lot of land (SW-mob).");

// ── S2: the mines ────────────────────────────────────────────────────────────

/// S2: two shells, a little apart in power, each landing as a mine (`sim::mines`), or going off at
/// once on a mobile it lands on (GF-lee).
pub const S2_SHELLS: Tuned<usize> = cited(2, &["SW-mob", "GF-lee", "DB-mob", "OB"]);
/// How far apart in power the two shells leave, either side of the shot's, bars: OB's ±3 on its
/// 0-100 bar.
pub const S2_SPREAD_BARS: Tuned<f64> = cited(0.12, &["OB"]);
/// A mine's blast: the damage of one going off on a mobile it touched, "140-180 (per Raon mine)"
/// (SW-mob).
pub const MINE_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 140, max: 180 }, &["SW-mob", "DB-mob"]);
/// Its reach and crater: OB's tight r15.
pub const MINE_BLAST: Tuned<f64> = cited(15.0, &["OB"]);
/// How near an enemy must be for a mine to arm ("its antenna will turn red", SW-mob) and crawl
/// toward it, px from its hitbox: OB's 90. GunBound's sources say only "close" (SW-mob).
pub const MINE_SENSE: Tuned<f64> = cited(90.0, &["OB"]);
/// "Raons are more attracted to Mechanical mobiles than Bionics and Shields" (SW-mob), "the Raon
/// alertness range seems to be wider for metal tanks" (GF-lee): a Mechanical enemy counts this many
/// times nearer, for sensing and for picking a quarry. GF-lee says non-Bionic, Shields too
/// (CONTRADICTION).
pub const MECHANICAL_PULL: Tuned<f64> = picked(1.5, "More attracted, by no number (SW-mob, GF-lee): sensed half again as far, and picked over a nearer Bionic one.");
/// The most a mine crawls as its layer's turn comes up ("when the original launcher's turn comes",
/// SW-mob; "on your own turn", GF-lee), in steps: OB's 90, a mobile's move budget. DBR's clone
/// walks one at most 30 (CONTRADICTION).
pub const MINE_CRAWL_STEPS: Tuned<u32> = cited(90, &["OB"]);
/// How fast it crawls, px/s: OB walks everything a pixel a frame, at 60 frames a second.
pub const MINE_CRAWL_SPEED: Tuned<f64> = cited(60.0, &["OB"]);
/// How high a mine's middle stands over the ground, px: what touches a mobile, and what a blast
/// reaches. OB's mine is 8 px square.
pub const MINE_HEIGHT: Tuned<f64> = cited(4.0, &["OB"]);

// ── SS: the big beetle ───────────────────────────────────────────────────────

/// SS: a big beetle that walks "the direction it was shot at", turns round at walls "too steep to
/// climb", and goes off on walking into a mobile, "regardless of ally, enemy, or corpse", or when
/// its fuse burns down (SW-mob, OB). GF-lee's homes on the nearest mobile for about 3 s
/// (CONTRADICTION). A direct hit goes off at once.
pub const SS_DAMAGE: Tuned<DamageRange> = picked(
    DamageRange { min: 300, max: 500 },
    "No source gives a range (SW-mob's is blank, DB-mob's ??): GF-lee's 300+ at the bottom, OB's 500 at the top.",
);
pub const SS_BLAST: Tuned<f64> = cited(30.0, &["OB"]);
/// How fast it walks, px/s: OB walks everything a pixel a frame, at 60 frames a second.
pub const SS_WALK_SPEED: Tuned<f64> = cited(60.0, &["OB"]);
/// From landing to going off: 5 to 7 seconds (SW-mob). OB walks it 360 steps, 6 s at its pace; a
/// roll would take the shot out of the player's hands for no gain, so it is the middle.
pub const SS_FUSE_SECONDS: Tuned<f64> = picked(6.0, "The middle of SW-mob's 5 to 7 s, and OB's 360 steps at a pixel a frame.");
/// How high its middle stands over the ground, px.
pub const SS_HEIGHT: Tuned<f64> = picked(8.0, "A big beetle: twice a mine's height.");
