//! The weather queue (GunBound's moondisk): five upcoming weathers, the front one activating every
//! N turns, where N is the number of players (SW-weather). Column weathers (Force, Mirror, Tornado)
//! stand on the map where the server rolled them; Wind rerolls the wind. The server rolls every
//! new queue entry and every new wind; the sim only says when one is owed (`Game::pending_roll`).

use serde::{Deserialize, Serialize};

use crate::tuning::{FORCE_DAMAGE, WEATHER_QUEUE, column_width};

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
}

impl WeatherKind {
    /// Every kind the server may roll, in a fixed order.
    pub const ALL: [Self; 4] = [Self::Wind, Self::Force, Self::Mirror, Self::Tornado];

    /// Whether it stands on the map as a column.
    #[must_use]
    pub fn is_column(self) -> bool {
        self != Self::Wind
    }
}

/// A queued weather: its kind, and where its column will stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forecast {
    pub kind: WeatherKind,
    /// The column's center (ignored for Wind).
    pub x: i32,
}

/// A weather column on the map, full height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub kind: WeatherKind,
    pub x: i32,
}

impl Column {
    /// Whether world x lies inside the column.
    #[must_use]
    pub fn contains(&self, x: f64) -> bool {
        let half = column_width(self.kind) / 2.0;
        (f64::from(self.x) - half..f64::from(self.x) + half).contains(&x)
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
        if next.kind == WeatherKind::Wind {
            self.columns.clear();
            self.wind_owed = true;
        } else {
            if self.columns.first().is_some_and(|c| c.kind != next.kind) {
                self.columns.clear();
            }
            self.columns.push(Column { kind: next.kind, x: next.x });
        }
    }

    /// The damage multiplier for a shot that passed through `forced` Force columns.
    #[must_use]
    pub fn force_multiplier(forced: bool) -> f64 {
        if forced { FORCE_DAMAGE.value } else { 1.0 }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    fn forecast(kind: WeatherKind, x: i32) -> Forecast {
        Forecast { kind, x }
    }

    #[test]
    fn the_front_activates_every_round() {
        let mut weather = Weather::new(2);
        weather.queue = vec![forecast(WeatherKind::Force, 100), forecast(WeatherKind::Force, 300), forecast(WeatherKind::Mirror, 500)];
        weather.tick(2);
        assert!(weather.columns.is_empty(), "one turn into a two-turn round");
        weather.tick(2);
        assert_eq!(weather.columns, [Column { kind: WeatherKind::Force, x: 100 }]);
        assert!(weather.short());
        weather.tick(2);
        weather.tick(2);
        assert_eq!(weather.columns.len(), 2, "a repeat of the same kind stacks");
        weather.tick(2);
        weather.tick(2);
        assert_eq!(weather.columns, [Column { kind: WeatherKind::Mirror, x: 500 }], "a different kind clears");
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
    fn an_empty_queue_waits() {
        let mut weather = Weather::new(1);
        weather.tick(1);
        weather.tick(1);
        assert!(weather.columns.is_empty() && !weather.wind_owed);
    }

    #[test]
    fn columns_have_width() {
        let column = Column { kind: WeatherKind::Tornado, x: 500 };
        assert!(column.contains(500.0) && column.contains(466.0) && !column.contains(536.0));
    }
}
