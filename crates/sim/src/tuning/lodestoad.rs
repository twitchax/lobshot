//! Lodestoad: a toad driving a little tractor wound with copper coil, a big horseshoe magnet for a
//! cannon, with GunBound J.D.'s behavior. Geometry (hitbox, probes, cannon) matches its drawing in
//! `assets/mobiles/lodestoad/rig.ron`. Its sources are gathered in `docs/research/lodestoad.md`.
//!
//! S1 is an electric orb whose discharge splashes everyone near, once a turn. S2's magnet ball
//! passes through mobiles to the ground, digs a hole and pulls the mobiles near it in, machines
//! the most and creatures the least, over edges and off the map if nothing is below. The SS's
//! pyramid pushes them away, creatures less, hurting most at its center and more if they can't
//! be pushed all the way, and leaves a mobile on the very edge of a cliff rather than over it
//! (GunBound's 2004 patch). Both move a mobile by sliding it along the ground
//! (`movement::slide`).

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, ShotTuning, Tuned, WeaponType, cited, fitted, picked};

/// All true: "40-10 (40-10)" (CR-mob), "True Angle Range: FULL" (GF-tiro), 30 total and 30 true
/// (GF-vgg). DBR's clone aims 15-65 (CONTRADICTION; GF-lee's changelog says the arc was
/// "reduced" in 2004).
const AIM: Tuned<AimRange> = cited(
    AimRange {
        min: 10,
        max: 40,
        true_min: 10,
        true_max: 40,
    },
    &["CR-mob", "GF-tiro", "GF-vgg", "OB"],
);

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Lodestoad",
    behaves_like: "JD",
    // CR-mob (700) and SW-mob (750, which OB copies) disagree (CONTRADICTION), so DragonBound's.
    hp: cited(950, &["DB-mob"]),
    shield: cited(250, &["CR-mob", "SW-mob", "DB-mob", "GF-lee"]),
    // GunBound cut it from 30 to 20 on 19 April 2004 (GF-lee); DragonBound keeps 30.
    shield_regen: cited(20, &["CR-mob", "SW-mob", "GF-lee"]),
    armor: cited(ArmorClass::Shield, &["SW-mob", "DB-mob", "GF-vgg"]),
    weapon: cited(WeaponType::Electric, &["SW-mob", "DB-mob", "GF-vgg"]),
    // DragonBound gives 500.
    base_delay: cited(490, &["CR-mob", "SW-mds", "SW-mob"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.30 bars, zero wind, flat ground."),
    wind_sensitivity: picked(
        1.0,
        "No source gives a number; 'tied with Boomer for the lightest shot' (WP06-list) is its weight, which the fitted gravity carries.",
    ),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -33.0, y: -46.0, w: 64.0, h: 46.0 },
        "Lodestoad's drawn tractor and toad, fender to grille and wheels to the toad's eyes. The rider, the seat's back and the magnet's post are cosmetic.",
    ),
    ground_probes: picked([-18.0, 22.0], "Lodestoad's drawn drive and front wheels."),
    cannon_pivot: picked((20.0, -38.0), "Lodestoad's drawn magnet swivel, on its post on the tractor's hood."),
    barrel_length: picked(26.0, "Lodestoad's drawn horseshoe magnet, swivel to the spark between its poles."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "SW-mob"]),
        aim: AIM,
    },
    s2: ShotTuning {
        delay: cited(340, &["CR-mob", "SW-mds", "SW-mob", "DB-mob"]),
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
const GRAVITY: f64 = 551.8;

/// What share of a move each armor class makes: machines are pulled the most and creatures the
/// least.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shares {
    pub mechanical: f64,
    pub shield: f64,
    pub bionic: f64,
}

impl Shares {
    #[must_use]
    pub fn of(&self, armor: ArmorClass) -> f64 {
        match armor {
            ArmorClass::Mechanical => self.mechanical,
            ArmorClass::Shield => self.shield,
            ArmorClass::Bionic => self.bionic,
        }
    }
}

/// How long a pulled or pushed mobile slides, however far: DBR's client slides it in 250 ms.
pub const SLIDE_SECONDS: Tuned<f64> = cited(0.25, &["DBR"]);

/// S1: an electric orb, then its discharge. SW-mob gives 140-170 "includes aftershock" (as
/// DB-mob); GF-tiro says 110-130 for the hit and 70-85 electric, and GF-max averages a shot at 100
/// and 64 splash (CONTRADICTION). SW-mob's total comes first, less GF-max's splash.
pub const S1_DAMAGE: Tuned<DamageRange> = picked(DamageRange { min: 76, max: 106 }, "SW-mob's 140-170 with its aftershock, less the 64 splash.");
pub const S1_BLAST: Tuned<f64> = picked(25.0, "'Decent size holes' (GF-vgg): OpenBound's usual blast.");
/// The discharge: a flat amount to every mobile near the blast, once a turn even with Dual
/// (GF-max, GF-tiro, WP06). It ignores defense (GF-max), so armor, angle, weather and items too.
/// GF-vgg: "at least 60 damage from the splash"; GF-tiro gives 70-85 (CONTRADICTION).
pub const SPLASH: Tuned<i32> = cited(64, &["GF-max"]);
/// How far from the blast (to the hitbox) the discharge reaches: a "giant splash radius" (CR-all),
/// "a much wider area of damage then most basic attacks" (GF-lee), "a small area" (GF-tiro).
pub const SPLASH_REACH: Tuned<f64> = picked(50.0, "Twice the orb's blast: wide enough to catch a near miss, and two mobiles side by side.");

/// S2: a magnet ball that passes through mobiles ("shot 2 only can hit dirt, not bots", CR-all;
/// "shooting through your enemy", SNX-jd) and bursts in the ground, burning low (`damage::low`):
/// worth most bursting at or under a mobile's feet, through it ("you DO need a direct hit to
/// register any damage", GF-lee; "hit in the middle of the enemy mobile", GF-max).
pub const S2_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 150, max: 200 }, &["SW-mob", "DB-mob"]);
/// Its hole, which the mobiles it pulls fall into: "a deep, narrow hole" (GF-lee).
pub const S2_BLAST: Tuned<f64> = picked(30.0, "Wider than S1's, so a mobile pulled in drops into it.");
/// How far across and how far up or down from where it struck a mobile's middle may be for it to be
/// pulled: DBR's blast box for J.D.'s S2. "If the shot lands too far away, it won't have any
/// effect" (GF-vgg).
pub const PULL_REACH: Tuned<(f64, f64)> = cited((59.0, 40.0), &["DBR"]);
/// A pulled machine slides all the way to the column where the ball struck (DBR moves it there);
/// the others part of the way. Machines are "affected most heavily" (WP06-list), "more sensitive
/// to the pull than other mobiles" (SW-mob); creatures "almost not attracted at all" (CR-pb279),
/// "to a lesser degree" (GF-lee), not at all (GF-kurai). No source places shields.
pub const PULL_SHARES: Tuned<Shares> = picked(
    Shares {
        mechanical: 1.0,
        shield: 0.5,
        bionic: 0.25,
    },
    "Machines all the way, creatures a quarter; shields, which no source places, between.",
);
/// A pull drags a mobile over an edge, to fall: off cliffs and through gaps in the floor (CR-all,
/// GF-tiro), off the map (SW-mob, DB-mob).
pub const PULL_OVER_EDGES: Tuned<bool> = cited(true, &["CR-all", "GF-tiro", "SW-mob", "DB-mob"]);
/// A mobile that floats over the ground (Kitewyrm, Dragon's behavior) isn't pulled: Dragon "hovers
/// above the land, thus being (almost) immune to... the cakebot's weapon 2" (GF-tiro).
pub const PULLS_FLOATERS: Tuned<bool> = cited(false, &["GF-tiro"]);

/// SS: a pyramid that pushes the mobiles near where it strikes away, hurting most near its center
/// (SW-mob, DB-mob), and more if they can't be pushed all the way (`PIN_DAMAGE`).
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 250, max: 320 }, &["SW-mob", "DB-mob"]);
pub const SS_BLAST: Tuned<f64> = picked(40.0, "'A wide blasting radius on contact' (GF-lee): Coilback's SS blast.");
/// How far across and up or down from where it struck a mobile's middle may be for it to be
/// pushed: DBR's blast box for J.D.'s SS.
pub const PUSH_REACH: Tuned<(f64, f64)> = cited((63.0, 47.0), &["DBR"]);
/// How far from the column where it struck a pushed mobile ends up: DBR puts it there.
pub const PUSH_TO: Tuned<f64> = cited(100.0, &["DBR"]);
/// Creatures are pushed "less than other types" (WP06-list), or not at all (GF-lee).
pub const PUSH_SHARES: Tuned<Shares> = picked(
    Shares {
        mechanical: 1.0,
        shield: 1.0,
        bionic: 0.25,
    },
    "Creatures a quarter of the way, as S2 pulls them.",
);
/// A floating mobile is pushed like any other.
pub const PUSHES_FLOATERS: Tuned<bool> = picked(
    true,
    "No source exempts Dragon from the push, which no source ties to the ground as GF-tiro ties the pull; it still walks the ground (OB).",
);
/// A push leaves a mobile on the very edge of a cliff: GunBound's 2004 patch (GF-tiro, GF-vgg,
/// CR-fun, WP06-list, CR-pb279). SW-mob and DB-mob say it "can push people off a map"
/// (CONTRADICTION): the game before the patch, or DragonBound's own.
pub const PUSH_OVER_EDGES: Tuned<bool> = cited(false, &["GF-tiro", "GF-vgg", "CR-fun", "WP06-list"]);
/// What a mobile the push can't take all the way (a wall, or a cliff's edge) takes on top, for the
/// share of `PUSH_TO` it didn't go: "pushed until they can't be pushed further, but is still taking
/// damage. It results in 300+ damage" against a usual 250-270 (GF-max); "it does more damage if the
/// person is right up against an edge" (GF-vgg).
pub const PIN_DAMAGE: Tuned<f64> = picked(50.0, "A pinned direct hit tops 300, GF-max's '300+'.");
