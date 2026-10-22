//! The weather queue (GunBound's moondisk): five upcoming weathers, the front one activating every
//! N turns, where N is the number of players (SW-weather). Column weathers (Force, Mirror, Tornado,
//! Electricity, Drain) stand on the map where the server rolled them; Wind rerolls the wind; Lock
//! and Balm hold over the whole map until the next weather comes up. The server rolls every
//! new queue entry and every new wind; the sim only says when one is owed (`Game::pending_roll`).

use serde::{Deserialize, Serialize};

use crate::tuning::{DRAIN_DAMAGE, FORCE_DAMAGE, WEATHER_QUEUE, column_width};

/// A weather.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeatherKind {
    /// Rerolls the wind (the only way it changes, WP06).
    Wind,
    /// A column that boosts any shot passing through it.
    Force,
    /// A column that sends shots back the way they came.
    Mirror,
    /// A column that flings shots back up.
    Tornado,
    /// A column that charges shots passing through it: where one lands, the Thunder item's bolt
    /// strikes (GF-tiro, GF-vgg).
    Electricity,
    /// A column that weakens any shot passing through it (GunBound's Weakness).
    Drain,
    /// No one may use an item; shields stop regenerating, and SS recharges twice as fast
    /// (GunBound's Item Lock, WP06).
    Lock,
    /// Each player heals a little as their turn starts, Bionic mobiles double (GunBound's Moon).
    Balm,
    /// A column hiding one of `HIDDEN`: a shot passing through meets what it hides, and the column
    /// shows it from then on (DB-play, OB).
    Random,
}

impl WeatherKind {
    /// Every kind the server may roll, in a fixed order.
    pub const ALL: [Self; 9] = [
        Self::Wind,
        Self::Force,
        Self::Mirror,
        Self::Tornado,
        Self::Electricity,
        Self::Drain,
        Self::Lock,
        Self::Balm,
        Self::Random,
    ];
    /// What a Random column may hide: the column weathers, never Wind, Lock or Balm (OB).
    pub const HIDDEN: [Self; 5] = [Self::Force, Self::Tornado, Self::Electricity, Self::Drain, Self::Mirror];

    /// Whether it stands on the map as a column.
    #[must_use]
    pub fn is_column(self) -> bool {
        match self {
            Self::Force | Self::Mirror | Self::Tornado | Self::Electricity | Self::Drain | Self::Random => true,
            Self::Wind | Self::Lock | Self::Balm => false,
        }
    }
}

/// A queued weather: its kind, where its column will stand, and what a Random one hides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forecast {
    pub kind: WeatherKind,
    /// The column's center (ignored for Wind, Lock and Balm).
    pub x: i32,
    /// What a Random column hides, one of `WeatherKind::HIDDEN`; `None` for every other kind.
    pub hides: Option<WeatherKind>,
}

impl Forecast {
    /// A forecast of any kind but Random, which also says what it hides.
    #[must_use]
    pub fn new(kind: WeatherKind, x: i32) -> Self {
        Self { kind, x, hides: None }
    }

    /// Whether `hides` fits `kind`: what a Random column hides, and nothing for any other.
    #[must_use]
    pub fn is_whole(&self) -> bool {
        match self.hides {
            Some(hidden) => self.kind == WeatherKind::Random && WeatherKind::HIDDEN.contains(&hidden),
            None => self.kind != WeatherKind::Random,
        }
    }
}

/// A weather column on the map, full height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub kind: WeatherKind,
    pub x: i32,
    /// What a Random column hides, until a shot passes through it.
    pub hides: Option<WeatherKind>,
}

impl Column {
    #[must_use]
    pub fn new(kind: WeatherKind, x: i32) -> Self {
        Self { kind, x, hides: None }
    }

    /// Whether world x lies inside the column.
    #[must_use]
    pub fn contains(&self, x: f64) -> bool {
        let half = column_width(self.kind) / 2.0;
        (f64::from(self.x) - half..f64::from(self.x) + half).contains(&x)
    }

    /// What it does to a shot: a Random column acts as what it hides.
    #[must_use]
    pub fn acts_as(&self) -> WeatherKind {
        self.hides.unwrap_or(self.kind)
    }

    /// Whether it still hides what it is (a Random column no shot has passed through).
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.hides.is_some()
    }

    /// The column showing what it hid, where it stood.
    #[must_use]
    pub fn revealed(self) -> Self {
        Self::new(self.acts_as(), self.x)
    }
}

/// The weather state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Weather {
    /// Upcoming, front first. Kept full: when it runs short, the server owes a roll.
    pub queue: Vec<Forecast>,
    /// Columns standing now. Repeats of one kind stack; a different kind clears them (WP06).
    pub columns: Vec<Column>,
    /// Turns until the front of the queue activates.
    pub turns_left: u32,
    /// The active weather rerolled the wind and the server hasn't sent it yet.
    pub wind_owed: bool,
    /// The weather that came up last: it holds until the next one does.
    pub now: Option<WeatherKind>,
}

impl Weather {
    /// A fresh queue that activates its first weather after `round` turns.
    #[must_use]
    pub fn new(round: u32) -> Self {
        Self { turns_left: round, ..Self::default() }
    }

    /// Whether the queue needs another forecast.
    #[must_use]
    pub fn short(&self) -> bool {
        self.queue.len() < WEATHER_QUEUE.value
    }

    /// One turn passed; every `round` turns the front activates.
    pub fn tick(&mut self, round: u32) {
        self.turns_left = self.turns_left.saturating_sub(1);
        if self.turns_left > 0 || self.queue.is_empty() {
            return;
        }
        self.turns_left = round;
        let next = self.queue.remove(0);
        self.now = Some(next.kind);
        if next.kind.is_column() {
            if self.columns.first().is_some_and(|c| c.kind != next.kind) {
                self.columns.clear();
            }
            self.columns.push(Column {
                kind: next.kind,
                x: next.x,
                hides: next.hides,
            });
        } else {
            self.columns.clear();
            self.wind_owed |= next.kind == WeatherKind::Wind;
        }
    }

    /// Whether `kind` is the weather now.
    #[must_use]
    pub fn is(&self, kind: WeatherKind) -> bool {
        self.now == Some(kind)
    }
}

/// What the columns a projectile passed through did to it. Each kind marks it once, however many
/// of its columns it crosses (OB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Marks {
    /// Force: it hits harder.
    pub forced: bool,
    /// Drain: it hits softer.
    pub drained: bool,
    /// Electricity: a bolt strikes where it lands.
    pub charged: bool,
}

impl Marks {
    /// Marks the projectile with a column of `kind`. Returns whether the kind marks at all (the
    /// others turn it, or do nothing).
    pub fn mark(&mut self, kind: WeatherKind) -> bool {
        match kind {
            WeatherKind::Force => self.forced = true,
            WeatherKind::Drain => self.drained = true,
            WeatherKind::Electricity => self.charged = true,
            WeatherKind::Wind | WeatherKind::Mirror | WeatherKind::Tornado | WeatherKind::Lock | WeatherKind::Balm | WeatherKind::Random => return false,
        }
        true
    }

    /// The damage multiplier the marks come to.
    #[must_use]
    pub fn boost(self) -> f64 {
        let force = if self.forced { FORCE_DAMAGE.value } else { 1.0 };
        let drain = if self.drained { DRAIN_DAMAGE.value } else { 1.0 };
        force * drain
    }
}

impl std::ops::BitOr for Marks {
    type Output = Self;

    /// Both projectiles' marks, for a shot that carries on as another (Coilback's second warhead).
    fn bitor(self, other: Self) -> Self {
        Self {
            forced: self.forced || other.forced,
            drained: self.drained || other.drained,
            charged: self.charged || other.charged,
        }
    }
}

impl std::ops::BitOrAssign for Marks {
    fn bitor_assign(&mut self, other: Self) {
        *self = *self | other;
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    fn forecast(kind: WeatherKind, x: i32) -> Forecast {
        Forecast::new(kind, x)
    }

    #[test]
    fn the_front_activates_every_round() {
        let mut weather = Weather::new(2);
        weather.queue = vec![forecast(WeatherKind::Force, 100), forecast(WeatherKind::Force, 300), forecast(WeatherKind::Mirror, 500)];
        weather.tick(2);
        assert!(weather.columns.is_empty(), "one turn into a two-turn round");
        weather.tick(2);
        assert_eq!(weather.columns, [Column::new(WeatherKind::Force, 100)]);
        assert!(weather.short());
        weather.tick(2);
        weather.tick(2);
        assert_eq!(weather.columns.len(), 2, "a repeat of the same kind stacks");
        weather.tick(2);
        weather.tick(2);
        assert_eq!(weather.columns, [Column::new(WeatherKind::Mirror, 500)], "a different kind clears");
    }

    #[test]
    fn wind_clears_columns_and_owes_a_roll() {
        let mut weather = Weather::new(1);
        weather.queue = vec![forecast(WeatherKind::Tornado, 100), forecast(WeatherKind::Wind, 0)];
        weather.tick(1);
        weather.tick(1);
        assert!(weather.columns.is_empty() && weather.wind_owed);
    }

    #[test]
    fn lock_and_balm_clear_the_columns_and_hold_until_the_next() {
        let mut weather = Weather::new(1);
        weather.queue = vec![forecast(WeatherKind::Force, 100), forecast(WeatherKind::Lock, 0), forecast(WeatherKind::Balm, 0)];
        weather.tick(1);
        assert!(weather.is(WeatherKind::Force) && weather.columns.len() == 1);
        weather.tick(1);
        assert!(weather.is(WeatherKind::Lock) && weather.columns.is_empty() && !weather.wind_owed);
        weather.tick(1);
        assert!(weather.is(WeatherKind::Balm) && !weather.is(WeatherKind::Lock));
        weather.tick(1);
        assert!(weather.is(WeatherKind::Balm), "an empty queue leaves it up");
    }

    #[test]
    fn an_empty_queue_waits() {
        let mut weather = Weather::new(1);
        weather.tick(1);
        weather.tick(1);
        assert!(weather.columns.is_empty() && !weather.wind_owed);
    }

    #[test]
    fn force_and_drain_multiply_and_each_kind_marks_once() {
        let mut marks = Marks::default();
        assert!(marks.mark(WeatherKind::Force) && marks.mark(WeatherKind::Force));
        assert!((marks.boost() - FORCE_DAMAGE.value).abs() < 1e-12, "two Force columns boost once");
        assert!(marks.mark(WeatherKind::Drain));
        assert!((marks.boost() - FORCE_DAMAGE.value * DRAIN_DAMAGE.value).abs() < 1e-12);
        assert!(!marks.mark(WeatherKind::Mirror) && !marks.charged);
        assert_eq!(Marks::default() | Marks { charged: true, ..Marks::default() }, Marks { charged: true, ..Marks::default() });
    }

    #[test]
    fn columns_have_width() {
        let column = Column::new(WeatherKind::Tornado, 500);
        assert!(column.contains(500.0) && column.contains(466.0) && !column.contains(536.0));
    }
}
