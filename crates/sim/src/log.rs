//! The event log: a game's setup and every accepted event, in order. It is the wire format, the
//! replay format and the golden-test format, so its encoding is a long-term commitment: postcard
//! behind a leading version byte, plus a JSON form of the same types for people and LLM agents.

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    base::Res,
    game::{Event, Game, Setup},
    map::MapAssets,
};

/// Bumped whenever an encoded log from an older build would decode differently.
pub const WIRE_VERSION: u8 = 4;

/// A whole game, replayable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Log {
    pub setup: Setup,
    pub events: Vec<Event>,
}

impl Log {
    /// The compact form: the version byte, then postcard.
    ///
    /// # Errors
    ///
    /// Fails only if serialization does.
    pub fn encode(&self) -> Res<Vec<u8>> {
        let mut bytes = vec![WIRE_VERSION];
        bytes.extend(postcard::to_allocvec(self).context("encoding the log")?);
        Ok(bytes)
    }

    /// Reads the compact form.
    ///
    /// # Errors
    ///
    /// Fails on an empty buffer, another version, or malformed bytes.
    pub fn decode(bytes: &[u8]) -> Res<Self> {
        let (&version, body) = bytes.split_first().context("an empty log")?;
        ensure!(version == WIRE_VERSION, "log version {version}, but this build reads {WIRE_VERSION}");
        postcard::from_bytes(body).context("decoding the log")
    }

    /// The JSON form.
    ///
    /// # Errors
    ///
    /// Fails only if serialization does.
    pub fn to_json(&self) -> Res<String> {
        serde_json::to_string_pretty(self).context("writing the log as JSON")
    }

    /// Reads the JSON form.
    ///
    /// # Errors
    ///
    /// Fails on malformed JSON.
    pub fn from_json(json: &str) -> Res<Self> {
        serde_json::from_str(json).context("reading the log from JSON")
    }

    /// Plays the log from the setup, returning the final state. A drawn map's mask comes from
    /// `assets`.
    ///
    /// # Errors
    ///
    /// Fails if the setup is invalid or an event is rejected: a log must only hold accepted events.
    pub fn replay(&self, assets: &dyn MapAssets) -> Res<Game> {
        let mut game = Game::new(&self.setup, assets)?;
        for (index, event) in self.events.iter().enumerate() {
            game.apply(*event).with_context(|| format!("replaying event {index}: {event:?}"))?;
        }
        Ok(game)
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{MobileKind, Seat, Shot, Team},
        items::Loadout,
        map::{MapRef, NoAssets},
        mode::Mode,
        units::{Centis, Facing, Power, Wind},
        weather::{Forecast, WeatherKind},
    };

    fn log() -> Log {
        Log {
            setup: Setup {
                map: MapRef::Flat { width: 1600, height: 600, ground: 500 },
                seats: vec![
                    Seat {
                        team: Team::A,
                        mobile: MobileKind::Coilback,
                        spawn_x: 300,
                        facing: Facing::Right,
                        items: Loadout::default(),
                    },
                    Seat {
                        team: Team::B,
                        mobile: MobileKind::Coilback,
                        spawn_x: 1100,
                        facing: Facing::Left,
                        items: Loadout::default(),
                    },
                ],
                order: vec![1, 0],
                wind: Wind::new(7, 135).unwrap(),
                sudden_death: None,
                mode: Mode::Solo,
            },
            events: [Event::Weather(Forecast::new(WeatherKind::Wind, 0)); 5]
                .into_iter()
                .chain([
                    Event::Walk { seat: 1, facing: Facing::Left, steps: 12 },
                    Event::Shoot {
                        seat: 1,
                        shot: Shot::S1,
                        aim: 35,
                        power: Power::nearest(2.6),
                        time: Centis(840),
                    },
                    Event::Pass { seat: 0, time: Centis(2000) },
                    // The round of two turns is up: the Wind weather came up and owes new wind.
                    Event::Wind(Wind::new(20, 270).unwrap()),
                    Event::Weather(Forecast::new(WeatherKind::Mirror, 900)),
                ])
                .collect(),
        }
    }

    #[test]
    fn round_trips_through_both_forms() {
        let log = log();
        let bytes = log.encode().unwrap();
        assert_eq!(bytes[0], WIRE_VERSION);
        assert_eq!(Log::decode(&bytes).unwrap(), log);
        assert_eq!(Log::from_json(&log.to_json().unwrap()).unwrap(), log);
    }

    #[test]
    fn a_score_log_round_trips_and_a_log_from_before_modes_reads_as_solo() {
        let mut log = log();
        log.setup.mode = Mode::Score;
        log.events.push(Event::Respawn { seat: 0, x: 600 });
        assert_eq!(Log::decode(&log.encode().unwrap()).unwrap(), log);
        assert_eq!(Log::from_json(&log.to_json().unwrap()).unwrap(), log);
        let mut old: serde_json::Value = serde_json::from_str(&log.to_json().unwrap()).unwrap();
        old["setup"].as_object_mut().unwrap().remove("mode");
        assert_eq!(Log::from_json(&old.to_string()).unwrap().setup.mode, Mode::Solo);
    }

    #[test]
    fn refuses_other_versions_and_garbage() {
        let mut bytes = log().encode().unwrap();
        bytes[0] = WIRE_VERSION + 1;
        assert!(Log::decode(&bytes).is_err());
        assert!(Log::decode(&[]).is_err());
        assert!(Log::decode(&[WIRE_VERSION, 0xff, 0xff]).is_err());
    }

    #[test]
    fn replays_and_names_the_event_it_rejects() {
        let log = log();
        assert!(log.replay(&NoAssets).is_ok());
        let mut bad = log;
        bad.events.push(Event::Pass { seat: 0, time: Centis(0) });
        bad.events.push(Event::Pass { seat: 0, time: Centis(0) });
        let error = format!("{:#}", bad.replay(&NoAssets).unwrap_err());
        assert!(error.contains("replaying event"), "{error}");
    }
}
