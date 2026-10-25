//! A room's options, the values each may take, and how the lobby steps through them. The lists
//! below are the one place those values live: `validate` checks against them and the `next_*`
//! steps cycle through them.

use anyhow::ensure;
use serde::{Deserialize, Serialize};

use lobshot_sim::{
    base::Res,
    game::{SuddenDeath, SuddenDeathKind},
    mode::Mode,
    tuning::SUDDEN_DEATH_TURNS,
};

/// Players per team, 1v1 to 4v4.
pub const TEAM_SIZES: [u8; 4] = [1, 2, 3, 4];

/// The strongest wind the Wind weather may roll.
pub const MAX_WINDS: [u8; 3] = [0, 12, 26];

/// The turn timer, in seconds.
pub const TURN_SECONDS: [u8; 3] = [5, 10, 20];

/// What the room master sets before a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomOptions {
    /// A drawn map's name (`tidewall`), or `flat` for the synthetic test map.
    pub map: String,
    /// Players per team, 1 to 4.
    pub team_size: u8,
    /// The strongest wind the Wind weather may roll: 0, 12 or 26.
    pub max_wind: u8,
    /// The turn timer: 5, 10 or 20 seconds.
    pub turn_seconds: u8,
    /// Shuffle teams at the start instead of keeping members' picks.
    pub random_teams: bool,
    /// What sudden death does and when it starts (one of `SUDDEN_DEATH_TURNS`), or `None`.
    pub sudden_death: Option<SuddenDeath>,
    /// What the game is played for: one of `modes()`. A client from before modes sends none: Solo.
    #[serde(default)]
    pub mode: Mode,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            map: "tidewall".to_owned(),
            team_size: 1,
            max_wind: 26,
            turn_seconds: 20,
            random_teams: false,
            // A private server's changelog gives Double at 40 turns as the default (RD-origins snippet).
            sudden_death: Some(SuddenDeath {
                kind: SuddenDeathKind::Double,
                turn: SUDDEN_DEATH_TURNS.value[0],
            }),
            mode: Mode::Solo,
        }
    }
}

/// The modes a room may play: those this build plays (`Mode::built`), in `Mode::ALL`'s order.
#[must_use]
pub fn modes() -> Vec<Mode> {
    Mode::ALL.into_iter().filter(|mode| mode.built()).collect()
}

impl RoomOptions {
    /// Checks the ranges.
    ///
    /// # Errors
    ///
    /// Fails on a value the room can't use.
    pub fn validate(&self) -> Res<()> {
        let (fewest, most) = (TEAM_SIZES[0], TEAM_SIZES[TEAM_SIZES.len() - 1]);
        ensure!(TEAM_SIZES.contains(&self.team_size), "teams are {fewest} to {most} players, not {}", self.team_size);
        ensure!(MAX_WINDS.contains(&self.max_wind), "max wind is {}, not {}", one_of(&MAX_WINDS), self.max_wind);
        ensure!(TURN_SECONDS.contains(&self.turn_seconds), "turns are {} seconds, not {}", one_of(&TURN_SECONDS), self.turn_seconds);
        ensure!(!self.map.is_empty(), "pick a map");
        if let Some(SuddenDeath { turn, .. }) = self.sudden_death {
            let turns = SUDDEN_DEATH_TURNS.value;
            ensure!(turns.contains(&turn), "sudden death starts at turn {turns:?}, not {turn}");
        }
        let modes = modes();
        let names: Vec<&str> = modes.iter().map(|mode| mode.name()).collect();
        ensure!(modes.contains(&self.mode), "{} mode isn't built yet: rooms play {}", self.mode.name(), one_of(&names));
        Ok(())
    }

    /// These options with the map moved on to the one after it in `maps` (the lobby's list),
    /// wrapping around; the first if the map isn't there.
    #[must_use]
    pub fn next_map(&self, maps: &[&str]) -> Self {
        Self {
            map: next_of(maps, self.map.as_str()).to_owned(),
            ..self.clone()
        }
    }

    /// These options with the next team size, wrapping around.
    #[must_use]
    pub fn next_team_size(&self) -> Self {
        Self {
            team_size: next_of(&TEAM_SIZES, self.team_size),
            ..self.clone()
        }
    }

    /// These options with the next max wind, wrapping around.
    #[must_use]
    pub fn next_max_wind(&self) -> Self {
        Self {
            max_wind: next_of(&MAX_WINDS, self.max_wind),
            ..self.clone()
        }
    }

    /// These options with the next turn time, wrapping around.
    #[must_use]
    pub fn next_turn_seconds(&self) -> Self {
        Self {
            turn_seconds: next_of(&TURN_SECONDS, self.turn_seconds),
            ..self.clone()
        }
    }

    /// These options with the next mode a room may play (`modes`), wrapping around.
    #[must_use]
    pub fn next_mode(&self) -> Self {
        Self {
            mode: next_of(&modes(), self.mode),
            ..self.clone()
        }
    }

    /// These options with the next sudden death: off, then each kind, then off again, keeping its
    /// turn (the first of `SUDDEN_DEATH_TURNS` coming from off).
    #[must_use]
    pub fn next_sudden_death(&self) -> Self {
        let kinds: Vec<Option<SuddenDeathKind>> = std::iter::once(None).chain(SuddenDeathKind::ALL.map(Some)).collect();
        let turn = self.sudden_death.map_or(SUDDEN_DEATH_TURNS.value[0], |rule| rule.turn);
        Self {
            sudden_death: next_of(&kinds, self.sudden_death.map(|rule| rule.kind)).map(|kind| SuddenDeath { kind, turn }),
            ..self.clone()
        }
    }

    /// These options with sudden death at the next of `SUDDEN_DEATH_TURNS`; unchanged while it's
    /// off.
    #[must_use]
    pub fn next_sudden_death_turn(&self) -> Self {
        Self {
            sudden_death: self.sudden_death.map(|rule| SuddenDeath {
                turn: next_of(&SUDDEN_DEATH_TURNS.value, rule.turn),
                ..rule
            }),
            ..self.clone()
        }
    }
}

/// The value after `current` in `values`, wrapping around; the first if `current` isn't there (or
/// `current` itself if `values` is empty).
fn next_of<T: PartialEq + Copy>(values: &[T], current: T) -> T {
    let at = values.iter().position(|&value| value == current).map_or(0, |at| at + 1);
    values.get(at % values.len().max(1)).copied().unwrap_or(current)
}

/// Values as a player reads a choice: "5, 10 or 20".
fn one_of<T: ToString>(values: &[T]) -> String {
    match values.split_last() {
        Some((last, [])) => last.to_string(),
        Some((last, rest)) => format!("{} or {}", rest.iter().map(T::to_string).collect::<Vec<_>>().join(", "), last.to_string()),
        None => String::new(),
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn values_step_on_and_wrap_around() {
        assert_eq!(next_of(&TEAM_SIZES, 1), 2);
        assert_eq!(next_of(&TEAM_SIZES, 4), 1);
        assert_eq!(next_of(&MAX_WINDS, 7), 0, "an unknown value starts over");
        assert_eq!(next_of::<u8>(&[], 7), 7);
        let options = RoomOptions::default();
        assert_eq!(options.next_team_size().team_size, 2);
        assert_eq!(options.next_max_wind().max_wind, 0);
        assert_eq!(options.next_turn_seconds().turn_seconds, 5);
    }

    #[test]
    fn maps_step_through_the_lobbys_list() {
        let maps = ["tidewall", "rimewake", "flat"];
        let options = RoomOptions::default();
        let stepped: Vec<String> = std::iter::successors(Some(options.clone()), |o| Some(o.next_map(&maps))).skip(1).take(3).map(|o| o.map).collect();
        assert_eq!(stepped, ["rimewake", "flat", "tidewall"]);
        let gone = RoomOptions { map: "gone".to_owned(), ..options };
        assert_eq!(gone.next_map(&maps).map, "tidewall", "an unknown map starts over");
    }

    #[test]
    fn sudden_death_steps_its_kinds_through_off_and_keeps_its_turn() {
        let at = |kind, turn| Some(SuddenDeath { kind, turn });
        let with = |sudden_death| RoomOptions { sudden_death, ..RoomOptions::default() };
        assert_eq!(with(None).next_sudden_death().sudden_death, at(SuddenDeathKind::BigBomb, 40));
        assert_eq!(with(at(SuddenDeathKind::BigBomb, 56)).next_sudden_death().sudden_death, at(SuddenDeathKind::Double, 56));
        assert_eq!(with(at(SuddenDeathKind::Double, 72)).next_sudden_death().sudden_death, at(SuddenDeathKind::SsDeath, 72));
        assert_eq!(with(at(SuddenDeathKind::SsDeath, 72)).next_sudden_death().sudden_death, None);
        assert_eq!(with(at(SuddenDeathKind::Double, 72)).next_sudden_death_turn().sudden_death, at(SuddenDeathKind::Double, 40));
        assert_eq!(with(None).next_sudden_death_turn().sudden_death, None, "nothing to move while it's off");
    }

    /// The steps and `validate` share their lists: wherever the lobby steps, the room accepts.
    #[test]
    fn every_step_lands_on_a_valid_option() {
        let steps: [fn(&RoomOptions) -> RoomOptions; 6] = [
            RoomOptions::next_team_size,
            RoomOptions::next_max_wind,
            RoomOptions::next_turn_seconds,
            RoomOptions::next_sudden_death,
            RoomOptions::next_sudden_death_turn,
            RoomOptions::next_mode,
        ];
        let mut options = RoomOptions::default();
        for round in 0..12 {
            for step in steps {
                options = step(&options);
                assert!(options.validate().is_ok(), "round {round}: {options:?}");
            }
        }
    }

    #[test]
    fn refusals_name_the_allowed_values() {
        let refusal = |options: RoomOptions| options.validate().unwrap_err().to_string();
        let options = RoomOptions::default();
        assert_eq!(refusal(RoomOptions { team_size: 5, ..options.clone() }), "teams are 1 to 4 players, not 5");
        assert_eq!(refusal(RoomOptions { max_wind: 13, ..options.clone() }), "max wind is 0, 12 or 26, not 13");
        assert_eq!(refusal(RoomOptions { turn_seconds: 7, ..options.clone() }), "turns are 5, 10 or 20 seconds, not 7");
        assert_eq!(refusal(RoomOptions { mode: Mode::Tag, ..options }), "Tag mode isn't built yet: rooms play Solo, Score or Jewel");
        assert_eq!(one_of(&[3]), "3");
    }

    #[test]
    fn modes_step_through_those_built_and_default_to_solo() {
        let options = RoomOptions::default();
        assert_eq!(options.mode, Mode::Solo);
        assert_eq!(modes(), [Mode::Solo, Mode::Score, Mode::Jewel]);
        assert_eq!(options.next_mode().mode, Mode::Score);
        assert_eq!(options.next_mode().next_mode().mode, Mode::Jewel);
        assert_eq!(options.next_mode().next_mode().next_mode().mode, Mode::Solo, "wrapping around");
        let unbuilt = RoomOptions { mode: Mode::Tag, ..options };
        assert_eq!(unbuilt.next_mode().mode, Mode::Solo, "an unbuilt mode starts over");
    }

    #[test]
    fn options_from_before_modes_play_solo() {
        let mut json: serde_json::Value = serde_json::to_value(RoomOptions::default()).unwrap();
        json.as_object_mut().unwrap().remove("mode");
        assert_eq!(serde_json::from_value::<RoomOptions>(json).unwrap().mode, Mode::Solo);
    }
}
