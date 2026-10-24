//! Every gameplay number, each with its source (`DESIGN.md`, "Fidelity rules"): GunBound Classic +
//! Thor's Hammer first, DragonBound where those conflict or are silent, and a recorded pick where
//! every source is silent. Source keys (`SW-mds`, `CR-mob`, ...) are defined in `docs/research/`;
//! a test checks every key cited here exists there. Balance work edits this module and nothing else.

pub mod bot;
pub mod coilback;
pub mod drops;
pub mod frostfin;
pub mod herdhound;
pub mod hookclaw;
pub mod items;
pub mod lanternwing;
pub mod marblemole;
pub mod score;
pub mod sparkhorn;
pub mod spoutpot;
pub mod sudsotter;
pub mod thistleback;
pub mod tinbadger;
pub mod tuskrack;
pub mod weather;

use serde::{Deserialize, Serialize};

use crate::{game::Shot, units::Centis};

/// Where a number came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Stated by these sources (keys in `docs/research/`).
    Cited(&'static [&'static str]),
    /// Fitted so the game reproduces what these sources state; `how` names the target.
    Fitted { to: &'static [&'static str], how: &'static str },
    /// Our choice where the sources are silent or unusable; the reason says why.
    Picked(&'static str),
}

/// A number and its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuned<T> {
    pub value: T,
    pub source: Source,
}

#[must_use]
pub const fn cited<T>(value: T, keys: &'static [&'static str]) -> Tuned<T> {
    Tuned { value, source: Source::Cited(keys) }
}

#[must_use]
pub const fn fitted<T>(value: T, to: &'static [&'static str], how: &'static str) -> Tuned<T> {
    Tuned {
        value,
        source: Source::Fitted { to, how },
    }
}

#[must_use]
pub const fn picked<T>(value: T, why: &'static str) -> Tuned<T> {
    Tuned { value, source: Source::Picked(why) }
}

// ── Ballistics ───────────────────────────────────────────────────────────────

/// Launch speed per bar of power, in px/s. Gravity is per mobile, so only flight time depends on
/// this; range comes from each mobile's fitted gravity.
pub const LAUNCH_SPEED_PER_BAR: Tuned<f64> = picked(
    300.0,
    "Sets flight time: an 80-degree, one-screen super-high shot (3.8 bars, GN-aduka) flies about 4 s, \
     GunBound's Ultra High Angle threshold (SW-gp).",
);

/// Horizontal wind acceleration per wind unit, as a fraction of the shooter's gravity.
pub const WIND_X_PER_UNIT: Tuned<f64> = fitted(
    0.0062,
    &["CR-wind", "CR-philo"],
    "At fixed power, side wind moves the 60-degree, one-screen angle half a degree per unit.",
);

/// Vertical wind acceleration per wind unit, as a fraction of the shooter's gravity. Twice the
/// horizontal factor: the community's vertical and side-wind rules disagree by 2x if wind pushed
/// equally both ways, and each rule has two sources (docs/research/physics-fit.md).
pub const WIND_Y_PER_UNIT: Tuned<f64> = fitted(
    0.0114,
    &["CR-wind", "GN-aduka"],
    "0.1 bar less power per 7 units of up-wind (and more per down-wind) at about 2.45 bars.",
);

/// The longest a projectile may fly before it is discarded, in seconds. Shots always come down
/// (vertical wind is at most a third of gravity), so this is a guard, not a rule.
pub const MAX_FLIGHT_SECONDS: Tuned<u32> = picked(30, "A safety stop well past the longest real flight (~6 s).");

// ── Turns and delay ──────────────────────────────────────────────────────────

/// The turn timer.
pub const TURN_TIME_LIMIT: Tuned<Centis> = cited(Centis(2000), &["SW-delay", "GF-tiro", "WP06", "DB-game"]);

/// Delay added per second of turn time, for every mobile unless its own tuning says otherwise.
pub const DELAY_PER_SECOND: Tuned<u32> = cited(10, &["SW-delay", "SW-mds", "DB-play", "DB-mob"]);

/// Extra delay for passing, on top of the mobile's base delay and the time spent. GunBound's pass
/// column is exactly base + 200 (SW-mds); WP06 says a pass costs nothing extra (CONTRADICTION).
pub const PASS_DELAY: Tuned<u32> = cited(200, &["SW-mds"]);

/// On equal delay, the player who acted longest ago goes first (rather than the shooter again).
pub const TIE_GOES_TO_LONGEST_WAITING: Tuned<bool> = picked(
    true,
    "No public source; the DBR clone sorts by delay, then by last turn. OpenBound's stable sort lets the \
     shooter go again, which rewards nothing.",
);

/// The turns sudden death may start at: rooms pick one.
pub const SUDDEN_DEATH_TURNS: Tuned<[u32; 3]> = cited([40, 56, 72], &["OB"]);

/// Big Bomb's crater multiplier on every shot. Sources say only "bigger craters" (WP06, AK).
pub const BIG_BOMB_CRATER: Tuned<f64> = picked(1.5, "Half again reads as bigger at a glance without flattening a map in a few turns.");

// ── SS ───────────────────────────────────────────────────────────────────────

/// Own turns SS stays locked after it fires; it is usable again on the next. GunBound allows SS
/// once every 4 of a player's own shots (WP06, GF-tiro), and OpenBound locks it for 3 own turns;
/// AK says about 9 turns counting everyone's (CONTRADICTION).
pub const SS_LOCK_TURNS: Tuned<u8> = cited(3, &["WP06", "GF-tiro", "OB"]);

// ── Bolts ────────────────────────────────────────────────────────────────────

/// How far above the map's top bolts strike from and drops fall from, so a mobile standing near the
/// top is still struck from above.
pub const SKY: Tuned<f64> = picked(100.0, "Clears any hitbox standing on a map's top rows.");

/// How close a bolt's path must pass to a mobile to hurt it without striking it: OpenBound's shock
/// radius around its lightning.
pub const BOLT_REACH: Tuned<f64> = cited(25.0, &["OB"]);

/// The crater a bolt leaves where it strikes the ground.
pub const BOLT_CRATER: Tuned<f64> = picked(12.0, "A scorch, not a blast: bolts hurt, they don't dig.");

/// How long after a projectile lands the bolt it calls down strikes.
pub const BOLT_DELAY_SECONDS: Tuned<f64> = picked(0.25, "Sources only say the impact calls the bolt (DB-mob); a beat apart reads as cause and effect.");

// ── Weather ──────────────────────────────────────────────────────────────────

/// Upcoming weathers shown on the moondisk.
pub const WEATHER_QUEUE: Tuned<usize> = cited(5, &["DB-img-game", "DB-live"]);

/// Damage multiplier for a shot that passed through a Force column. WP06 and SW-gp say the bonus
/// grows with time in the beam (CONTRADICTION); GF-tiro states +33%.
pub const FORCE_DAMAGE: Tuned<f64> = cited(1.33, &["GF-tiro"]);

/// How long a Tornado holds a shot before flinging it back up, in seconds.
pub const TORNADO_HOLD_SECONDS: Tuned<f64> = picked(
    1.0,
    "Sources only say the shot orbits, then exits (DB-play); a second of spin reads clearly and costs the \
     shot time toward transforms like Coilback's SS.",
);

/// A weather column's width in pixels.
#[must_use]
pub fn column_width(kind: crate::weather::WeatherKind) -> f64 {
    use crate::weather::WeatherKind;
    match kind {
        WeatherKind::Tornado => TORNADO_WIDTH.value,
        WeatherKind::Random => RANDOM_WIDTH.value,
        // Wind, Lock, Balm and Skyeye never stand as columns.
        WeatherKind::Force | WeatherKind::Mirror | WeatherKind::Electricity | WeatherKind::Drain | WeatherKind::Wind | WeatherKind::Lock | WeatherKind::Balm | WeatherKind::Skyeye => BEAM_WIDTH.value,
    }
}

/// Force, Mirror, Electricity and Drain beams (OB gives Weakness Force's).
pub const BEAM_WIDTH: Tuned<f64> = cited(40.0, &["OB"]);

/// How many own turns of SS lock each turn under Lock counts for: SS "will recharge twice as fast
/// during Item Locks" (WP06), and lobshot's SS lock counts own turns.
pub const LOCK_SS_RECHARGE: Tuned<u8> = cited(2, &["WP06"]);

/// What Balm (GunBound's Moon) heals as each player's turn starts, in percent of max HP. WP06 says
/// only a small heal each turn; OB heals once, when it comes up, a flat 35 (CONTRADICTION).
pub const BALM_HEAL_PERCENT: Tuned<u32> = cited(3, &["NAMU", "SW-weather"]);

/// Bionic mobiles heal this many times as much under Balm.
pub const BALM_BIONIC_FACTOR: Tuned<u32> = cited(2, &["WP06", "NAMU"]);

/// What a Drain column (GunBound's Weakness) leaves of a shot's damage. OB divides by 1.2 and
/// subtracts 10 (CONTRADICTION).
pub const DRAIN_DAMAGE: Tuned<f64> = cited(0.5, &["WP08"]);

/// A Random column, until a shot reveals it; then it takes its own kind's width.
pub const RANDOM_WIDTH: Tuned<f64> = cited(80.0, &["OB"]);

/// Tornado columns.
pub const TORNADO_WIDTH: Tuned<f64> = cited(70.0, &["OB"]);

// ── Movement ─────────────────────────────────────────────────────────────────

/// How far up a single 1 px step may climb, for every mobile unless its own tuning says otherwise.
pub const CLIMB_PX: Tuned<i32> = cited(6, &["OB"]);

/// How far down a single 1 px step may drop before the mobile falls instead.
pub const DROP_PX: Tuned<i32> = cited(5, &["OB"]);

/// How far above and below its feet a mobile looks for the ground under each probe, for tilt.
pub const TILT_SEARCH_PX: Tuned<i32> = cited(25, &["OB"]);

// ── Damage ───────────────────────────────────────────────────────────────────

/// Damage multiplier for a shot fired outside the mobile's true-angle range. GunBound sources put
/// it at 0.67 to 0.75 (GF-max, WP06; CONTRADICTION); GF-tiro states 70%.
pub const FALSE_ANGLE_DAMAGE: Tuned<f64> = cited(0.70, &["GF-tiro"]);

/// Armor-class advantage, in fractions of damage. GunBound: plus or minus 4 percentage points
/// (SW-mobC); DragonBound uses 15% (DB-mob; CONTRADICTION, possibly another version).
pub const TYPE_ADVANTAGE: Tuned<f64> = cited(0.04, &["SW-mobC"]);

/// A mobile's armor class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorClass {
    Mechanical,
    Shield,
    Bionic,
}

/// A weapon's damage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponType {
    Physical,
    Laser,
    Electric,
    Explosion,
}

impl WeaponType {
    /// The damage multiplier against `armor`: each weapon type is strong against one class and
    /// weak against another; explosions are neutral (SW-mobC, DB-mob).
    #[must_use]
    pub fn against(self, armor: ArmorClass) -> f64 {
        let edge = TYPE_ADVANTAGE.value;
        let (strong, weak) = match self {
            Self::Physical => (ArmorClass::Shield, ArmorClass::Mechanical),
            Self::Laser => (ArmorClass::Bionic, ArmorClass::Shield),
            Self::Electric => (ArmorClass::Mechanical, ArmorClass::Bionic),
            Self::Explosion => return 1.0,
        };
        if armor == strong {
            1.0 + edge
        } else if armor == weak {
            1.0 - edge
        } else {
            1.0
        }
    }
}

// ── Mobiles ──────────────────────────────────────────────────────────────────

/// A rectangle in a mobile's body frame: origin at the center of its ground contact, `+x` forward,
/// `y` down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// An aim range in whole degrees above the body's forward direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimRange {
    pub min: i16,
    pub max: i16,
    /// The full-damage part of the range; outside it, `FALSE_ANGLE_DAMAGE` applies.
    pub true_min: i16,
    pub true_max: i16,
}

impl AimRange {
    #[must_use]
    pub fn contains(self, aim: i16) -> bool {
        (self.min..=self.max).contains(&aim)
    }

    #[must_use]
    pub fn is_true(self, aim: i16) -> bool {
        (self.true_min..=self.true_max).contains(&aim)
    }
}

/// A direct hit's damage: `max` dead center, `min` at the hitbox's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRange {
    pub min: i32,
    pub max: i32,
}

/// One of a mobile's three shots.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotTuning {
    pub delay: Tuned<u32>,
    pub aim: Tuned<AimRange>,
}

/// Everything about a mobile except its shots' behavior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobileTuning {
    /// The lobshot name.
    pub name: &'static str,
    /// The GunBound mobile whose behavior this is; source tags refer to it by that name.
    pub behaves_like: &'static str,
    pub hp: Tuned<i32>,
    /// A shield soaks damage before HP does; 0 for most mobiles.
    pub shield: Tuned<i32>,
    /// Shield restored each turn, up to `shield`.
    pub shield_regen: Tuned<i32>,
    pub armor: Tuned<ArmorClass>,
    pub weapon: Tuned<WeaponType>,
    pub base_delay: Tuned<u32>,
    pub delay_per_second: Tuned<u32>,
    /// Extra delay whenever this mobile uses an item (Armor's 40, SW-mds).
    pub item_surcharge: Tuned<u32>,
    /// Gravity on this mobile's shots, px/s². Per-mobile "shot weight".
    pub gravity: Tuned<f64>,
    /// Multiplier on wind acceleration (Boomer's is high).
    pub wind_sensitivity: Tuned<f64>,
    /// 1 px steps per turn.
    pub move_budget: Tuned<u16>,
    /// How far up a single 1 px step may climb (`CLIMB_PX` for most mobiles).
    pub climb_px: Tuned<i32>,
    pub hitbox: Tuned<Rect>,
    /// Body-frame x offsets of the two points whose ground heights set the body's tilt.
    pub ground_probes: Tuned<[f64; 2]>,
    /// The cannon's pivot in the body frame.
    pub cannon_pivot: Tuned<(f64, f64)>,
    /// Pivot to muzzle, along the barrel.
    pub barrel_length: Tuned<f64>,
    pub s1: ShotTuning,
    pub s2: ShotTuning,
    pub ss: ShotTuning,
    /// A second barrel some shots leave from instead of the cannon (Frostfin's tusk); `None` for
    /// most mobiles.
    pub second_barrel: Option<SecondBarrel>,
}

/// A barrel besides the cannon. It turns with the aim, as the cannon does, and the shots it names
/// leave its muzzle rather than the cannon's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondBarrel {
    /// Its pivot in the body frame.
    pub pivot: Tuned<(f64, f64)>,
    /// Pivot to muzzle, along it.
    pub length: Tuned<f64>,
    /// The shots that leave it.
    pub shots: &'static [Shot],
}

/// A barrel's geometry: where it pivots in the body frame, and its length, pivot to muzzle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Barrel {
    pub pivot: (f64, f64),
    pub length: f64,
}

impl MobileTuning {
    /// The barrel `shot` leaves: the second barrel if it names the shot, else the cannon.
    #[must_use]
    pub fn barrel(&self, shot: Shot) -> Barrel {
        match self.second_barrel {
            Some(second) if second.shots.contains(&shot) => Barrel {
                pivot: second.pivot.value,
                length: second.length.value,
            },
            _ => Barrel {
                pivot: self.cannon_pivot.value,
                length: self.barrel_length.value,
            },
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn each_weapon_is_strong_against_one_class_and_weak_against_another() {
        for weapon in [WeaponType::Physical, WeaponType::Laser, WeaponType::Electric] {
            let multipliers: Vec<f64> = [ArmorClass::Mechanical, ArmorClass::Shield, ArmorClass::Bionic].map(|armor| weapon.against(armor)).to_vec();
            assert_eq!(multipliers.iter().filter(|&&m| m > 1.0).count(), 1, "{weapon:?}");
            assert_eq!(multipliers.iter().filter(|&&m| m < 1.0).count(), 1, "{weapon:?}");
        }
        assert_eq!(WeaponType::Electric.against(ArmorClass::Mechanical), 1.04);
        assert_eq!(WeaponType::Explosion.against(ArmorClass::Shield), 1.0);
    }
}
