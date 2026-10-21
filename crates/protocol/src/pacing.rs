//! How long a client takes to show an accepted event: the shell flies its legs in real time, blasts
//! land on their ticks, mobiles fall a beat after the last blast, and the turn passes a moment
//! after everything is still. Clients play each event on this schedule, and the server starts the
//! next turn's clock only once they're done, so a turn is never spent watching the last shot land.

use lobshot_sim::{
    game::{Death, Effect, Leg},
    units::TICKS_PER_SECOND,
};

/// How long a mobile takes to fall to its new footing.
pub const FALL_SECONDS: f32 = 0.35;

/// The beat after the last blast before anything falls (or drowns).
const SETTLE_SECONDS: f32 = 0.35;

/// When a blast's damage number appears, and when a blasted mobile dies, after the blast.
const DAMAGE_AFTER_BLAST: f32 = 0.05;
const DEATH_AFTER_BLAST: f32 = 0.1;

/// The pause after everything else before the turn passes.
const TAIL_SECONDS: f32 = 0.3;

/// When each of an event's effects happens, in seconds after the event, and when the scene ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// One time per effect, in order. A turn starting or the game ending happens at `end`.
    pub at: Vec<f32>,
    /// When the scene is over: 0 for an event with nothing to watch (a step, a forecast).
    pub end: f32,
}

/// The schedule for one event's effects.
#[must_use]
pub fn schedule(effects: &[Effect]) -> Schedule {
    let mut at = Vec::with_capacity(effects.len());
    let mut blast = 0.0_f32;
    let mut last_blast = 0.0_f32;
    let mut falls_at = None;
    let mut busy = 0.0_f32;
    // The finale's indices, filled with `end` once it is known.
    let mut finale = Vec::new();
    for (i, effect) in effects.iter().enumerate() {
        let time = match effect {
            Effect::Fired { legs, .. } => {
                busy = busy.max(flight_seconds(legs));
                0.0
            }
            Effect::Exploded { tick, .. } => {
                blast = ticks_to_seconds(*tick);
                last_blast = last_blast.max(blast);
                blast
            }
            Effect::Damaged { .. } => blast + DAMAGE_AFTER_BLAST,
            Effect::Died { death, .. } => match death {
                Death::Blasted => blast + DEATH_AFTER_BLAST,
                Death::Bunged => last_blast + SETTLE_SECONDS + FALL_SECONDS,
                Death::Forfeited => 0.0,
            },
            // Everything that falls falls together, after the blasts that shook it loose.
            Effect::Fell { .. } => *falls_at.get_or_insert(last_blast + SETTLE_SECONDS),
            Effect::TurnStarted { .. } | Effect::GameOver { .. } => {
                finale.push(i);
                0.0
            }
            Effect::Walked { .. } | Effect::WindChanged(_) | Effect::Forecast(_) | Effect::WeatherChanged { .. } => 0.0,
        };
        busy = busy.max(time);
        at.push(time);
    }
    let end = if busy > 0.0 { busy + FALL_SECONDS + TAIL_SECONDS } else { 0.0 };
    for i in finale {
        at[i] = end;
    }
    Schedule { at, end }
}

/// How long a shell flies, holds included.
#[must_use]
pub fn flight_seconds(legs: &[Leg]) -> f32 {
    legs.last().map_or(0.0, |leg| ticks_to_seconds(leg.offset + leg.to))
}

fn ticks_to_seconds(ticks: u32) -> f32 {
    // Flights last seconds: a few tens of thousands of ticks at most, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let seconds = ticks as f32 / TICKS_PER_SECOND as f32;
    seconds
}

#[cfg(test)]
mod unit {
    // Exact float equality is the claim here: a schedule's times are copied, or summed in the same
    // order the test sums them.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use pretty_assertions::assert_eq;

    use lobshot_sim::{
        game::{Event, Game, MobileKind, Seat, Setup, Shot, Team},
        items::Loadout,
        map::{MapRef, NoAssets},
        movement::Footing,
        units::{Centis, Facing, Power, Vec2, Wind},
    };

    use super::*;

    /// A lob across a flat duel, 700 px apart, in no wind: fired, flown, landed.
    fn shot_effects() -> Vec<Effect> {
        let seat = |team, spawn_x, facing| Seat {
            team,
            mobile: MobileKind::Coilback,
            spawn_x,
            facing,
            items: Loadout::default(),
        };
        let setup = Setup {
            map: MapRef::Flat { width: 1600, height: 900, ground: 700 },
            seats: vec![seat(Team::A, 400, Facing::Right), seat(Team::B, 1100, Facing::Left)],
            order: vec![0, 1],
            wind: Wind::new(0, 0).unwrap(),
            sudden_death: None,
        };
        let mut game = Game::new(&setup, &NoAssets).unwrap();
        while let Some(roll) = game.pending_roll() {
            let event = match roll {
                lobshot_sim::game::Roll::Wind => Event::Wind(Wind::new(0, 0).unwrap()),
                lobshot_sim::game::Roll::Weather { .. } => Event::Weather(lobshot_sim::weather::Forecast {
                    kind: lobshot_sim::weather::WeatherKind::Wind,
                    x: 0,
                }),
                lobshot_sim::game::Roll::Drop { .. } => Event::Drop(lobshot_sim::drops::Drop {
                    kind: lobshot_sim::drops::DropKind::Calm,
                    x: 0,
                }),
            };
            game.apply(event).unwrap();
        }
        game.apply(Event::Shoot {
            seat: 0,
            shot: Shot::S1,
            aim: 45,
            power: Power::from_twentieths(40).unwrap(),
            time: Centis(300),
        })
        .unwrap()
    }

    #[test]
    fn a_shot_lasts_its_flight_and_a_tail_and_the_turn_passes_at_the_end() {
        let effects = shot_effects();
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("no shot: {effects:?}")
        };
        let flight = flight_seconds(legs);
        assert!(flight > 1.0, "a 45 degree lob flies a while: {flight}");

        let schedule = schedule(&effects);
        assert_eq!(schedule.at.len(), effects.len());
        assert!(schedule.end >= flight + FALL_SECONDS + TAIL_SECONDS - 1e-4, "{schedule:?}");
        let turn = effects.iter().position(|e| matches!(e, Effect::TurnStarted { .. })).unwrap();
        assert_eq!(schedule.at[turn], schedule.end);
        assert!(schedule.at.iter().all(|&t| t <= schedule.end));
    }

    #[test]
    fn blasts_land_on_their_ticks_and_damage_follows() {
        let effects = vec![
            Effect::Exploded {
                at: Vec2::new(10.0, 10.0),
                radius: 30.0,
                tick: TICKS_PER_SECOND * 2,
                dirty: None,
            },
            Effect::Damaged { seat: 1, amount: 100 },
            Effect::Fell { seat: 1, to: Footing { x: 10, y: 20 } },
        ];
        let schedule = schedule(&effects);
        assert_eq!(schedule.at, [2.0, 2.0 + DAMAGE_AFTER_BLAST, 2.0 + SETTLE_SECONDS]);
        assert_eq!(schedule.end, 2.0 + SETTLE_SECONDS + FALL_SECONDS + TAIL_SECONDS);
    }

    #[test]
    fn nothing_to_watch_takes_no_time() {
        let schedule = schedule(&[Effect::TurnStarted { seat: 1 }]);
        assert_eq!(schedule, Schedule { at: vec![0.0], end: 0.0 });
        assert_eq!(super::schedule(&[]).end, 0.0);
    }
}
