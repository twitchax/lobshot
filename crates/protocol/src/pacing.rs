//! How long a client takes to show an accepted event: projectiles fly their legs in real time,
//! blasts and bolts land on their ticks, mobiles fall a beat after the last blast, and the turn
//! passes a moment after everything is still. An event's effects play in stages (see
//! `lobshot_sim::game::Effect`): a Dual's second volley, or a drop falling at a turn's end, starts
//! once everything before it has played out, falls included. Clients play each event on this
//! schedule, and the server starts the next turn's clock only once they're done, so a turn is never
//! spent watching the last shot land.

use lobshot_sim::{
    game::{Death, Effect, Leg},
    units::TICKS_PER_SECOND,
};

/// How long a mobile takes to fall to its new footing (and to trade places, for Swap).
pub const FALL_SECONDS: f32 = 0.35;

/// The beat after the last blast before anything falls (or drowns).
const SETTLE_SECONDS: f32 = 0.35;

/// When a blast's damage number appears, and when a blasted mobile dies, after the blast.
const DAMAGE_AFTER_BLAST: f32 = 0.05;
const DEATH_AFTER_BLAST: f32 = 0.1;

/// The pause after everything else before the turn passes, or the next stage starts.
const TAIL_SECONDS: f32 = 0.3;

/// How long sudden death's announcement holds before the next turn starts.
pub const SUDDEN_DEATH_SECONDS: f32 = 1.5;

/// When each of an event's effects happens, in seconds after the event, and when the scene ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// One time per effect, in order. A turn starting (with its shield and SS coming back) or the
    /// game ending happens at `end`, sudden death beginning just before.
    pub at: Vec<f32>,
    /// When the scene is over: 0 for an event with nothing to watch (a step, a forecast, a spin).
    pub end: f32,
}

/// Where the schedule is within the current stage.
#[derive(Debug, Clone, Copy, Default)]
struct Stage {
    /// When it started.
    base: f32,
    /// The latest blast, bolt or touch: damage and deaths follow it.
    blast: f32,
    /// The stage's last blast: what falls, falls after it.
    last_blast: f32,
    /// When this stage's falls happen, once the first is seen.
    falls_at: Option<f32>,
}

impl Stage {
    fn starting(base: f32) -> Self {
        Self {
            base,
            blast: base,
            last_blast: base,
            falls_at: None,
        }
    }

    /// A blast-like moment at the stage's `tick`.
    fn blast_at(&mut self, tick: u32) -> f32 {
        self.blast = self.base + ticks_to_seconds(tick);
        self.last_blast = self.last_blast.max(self.blast);
        self.blast
    }
}

/// The schedule for one event's effects.
#[must_use]
pub fn schedule(effects: &[Effect]) -> Schedule {
    let mut at = Vec::with_capacity(effects.len());
    let mut stage = Stage::default();
    let mut busy = 0.0_f32;
    // The finale's indices, filled with `end` once it is known.
    let mut finale = Vec::new();
    let mut sudden_death = None;
    for (i, effect) in effects.iter().enumerate() {
        let time = match effect {
            Effect::Volley { .. } | Effect::Dropped { .. } => {
                // A new stage waits until the one before has played out, falls included.
                stage = Stage::starting(if busy > 0.0 { busy + FALL_SECONDS + TAIL_SECONDS } else { stage.base });
                if let Effect::Dropped { legs, .. } = effect {
                    // A drop's landing is the stage's blast, for what follows it: a heal, the wind.
                    stage.blast += flight_seconds(legs);
                    stage.last_blast = stage.blast;
                    busy = busy.max(stage.blast);
                }
                stage.base
            }
            Effect::Fired { legs, .. } | Effect::Beacon { legs, .. } | Effect::Guide { legs, .. } => {
                busy = busy.max(stage.base + flight_seconds(legs));
                stage.base
            }
            Effect::Helix { strands, .. } => {
                let last = strands.iter().map(|strand| strand.to).max().unwrap_or(0);
                busy = busy.max(stage.base + ticks_to_seconds(last));
                stage.base
            }
            Effect::Orbit { orbiters, .. } => {
                let last = orbiters.iter().map(|orbiter| orbiter.to).max().unwrap_or(0);
                busy = busy.max(stage.base + ticks_to_seconds(last));
                stage.base
            }
            // A satellite's glide holds the scene until it arrives.
            Effect::Satellite { from, to, .. } => {
                busy = busy.max(stage.base + ticks_to_seconds(*to));
                stage.base + ticks_to_seconds(*from)
            }
            Effect::Exploded { tick, .. }
            | Effect::Bolt { tick, .. }
            | Effect::Beam { tick, .. }
            | Effect::Laser { tick, .. }
            | Effect::ShieldStripped { tick, .. }
            | Effect::Pierced { to: tick, .. } => stage.blast_at(*tick),
            Effect::Bounced { tick, .. } | Effect::Teleported { tick, .. } => stage.base + ticks_to_seconds(*tick),
            Effect::Damaged { .. } | Effect::Shielded { .. } | Effect::Healed { .. } | Effect::Frosted { .. } => stage.blast + DAMAGE_AFTER_BLAST,
            Effect::Died { death, .. } => match death {
                Death::Blasted => stage.blast + DEATH_AFTER_BLAST,
                Death::Bunged => stage.last_blast + SETTLE_SECONDS + FALL_SECONDS,
                Death::Forfeited => stage.base,
            },
            // Everything that falls falls together, after the blasts that shook it loose.
            Effect::Fell { .. } => *stage.falls_at.get_or_insert(stage.last_blast + SETTLE_SECONDS),
            Effect::Swapped { .. } => stage.base + FALL_SECONDS,
            Effect::WindChanged(_) => stage.blast,
            Effect::SuddenDeath { .. } => {
                sudden_death = Some(i);
                0.0
            }
            Effect::TurnStarted { .. } | Effect::GameOver { .. } | Effect::ShieldRestored { .. } | Effect::SsReady { .. } => {
                finale.push(i);
                0.0
            }
            Effect::Walked { .. }
            | Effect::Forecast(_)
            | Effect::WeatherChanged { .. }
            | Effect::Revealed { .. }
            | Effect::SkyeyeLevel { .. }
            | Effect::ItemUsed { .. }
            | Effect::SsLocked { .. }
            | Effect::Spun { .. }
            | Effect::DropRolled { .. } => stage.base,
        };
        busy = busy.max(time);
        at.push(time);
    }
    let mut end = if busy > 0.0 { busy + FALL_SECONDS + TAIL_SECONDS } else { 0.0 };
    if let Some(i) = sudden_death {
        at[i] = end;
        end += SUDDEN_DEATH_SECONDS;
    }
    for i in finale {
        at[i] = end;
    }
    Schedule { at, end }
}

/// How long a projectile flies, holds included.
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
        ballistics::Spin,
        game::{Event, Game, MobileKind, Orbiter, Seat, Setup, Shot, Strand, SuddenDeathKind, Team},
        items::Loadout,
        map::{MapRef, NoAssets},
        movement::Footing,
        units::{Centis, Facing, Power, Vec2, Wind},
    };

    use super::*;

    /// A lob across a flat duel, 700 px apart, in no wind: fired, flown, landed.
    fn shot_effects() -> Vec<Effect> {
        shot_by(MobileKind::Coilback, Shot::S1)
    }

    /// `mobile`'s `shot` at the bottom of its true range across a flat duel, 700 px apart, in no
    /// wind.
    fn shot_by(mobile: MobileKind, shot: Shot) -> Vec<Effect> {
        let seat = |team, spawn_x, facing| Seat {
            team,
            mobile,
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
                lobshot_sim::game::Roll::Weather { .. } => Event::Weather(lobshot_sim::weather::Forecast::new(lobshot_sim::weather::WeatherKind::Wind, 0)),
                lobshot_sim::game::Roll::Drop { .. } => Event::Drop(lobshot_sim::drops::Drop {
                    kind: lobshot_sim::drops::DropKind::Calm,
                    x: 0,
                }),
            };
            game.apply(event).unwrap();
        }
        let aim = if mobile == MobileKind::Coilback { 45 } else { shot.tuning(mobile.tuning()).aim.value.true_min };
        game.apply(Event::Shoot {
            seat: 0,
            shot,
            aim,
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
    fn a_frost_shows_with_the_damage_it_follows() {
        let effects = vec![
            Effect::Exploded {
                at: Vec2::new(10.0, 10.0),
                radius: 20.0,
                tick: TICKS_PER_SECOND,
                dirty: None,
            },
            Effect::Damaged { seat: 1, amount: 150 },
            Effect::Frosted { seat: 1, brittle: 5 },
        ];
        assert_eq!(schedule(&effects).at, [1.0, 1.0 + DAMAGE_AFTER_BLAST, 1.0 + DAMAGE_AFTER_BLAST]);
    }

    #[test]
    fn nothing_to_watch_takes_no_time() {
        let schedule = schedule(&[Effect::TurnStarted { seat: 1 }]);
        assert_eq!(schedule, Schedule { at: vec![0.0], end: 0.0 });
        assert_eq!(super::schedule(&[]).end, 0.0);
    }

    #[test]
    fn a_second_volley_starts_once_the_first_has_played_out() {
        let effects = vec![
            Effect::Volley { seat: 0, shot: Shot::S1 },
            Effect::Exploded {
                at: Vec2::new(10.0, 10.0),
                radius: 30.0,
                tick: TICKS_PER_SECOND,
                dirty: None,
            },
            Effect::Fell { seat: 0, to: Footing { x: 10, y: 20 } },
            Effect::Volley { seat: 0, shot: Shot::S1 },
            Effect::Exploded {
                at: Vec2::new(10.0, 10.0),
                radius: 30.0,
                tick: TICKS_PER_SECOND,
                dirty: None,
            },
            Effect::Damaged { seat: 1, amount: 100 },
        ];
        let schedule = schedule(&effects);
        let second = 1.0 + SETTLE_SECONDS + FALL_SECONDS + TAIL_SECONDS;
        assert_eq!(schedule.at, [0.0, 1.0, 1.0 + SETTLE_SECONDS, second, second + 1.0, second + 1.0 + DAMAGE_AFTER_BLAST]);
    }

    #[test]
    fn a_drop_falls_after_the_turn_has_played_out_and_acts_where_it_lands() {
        let fall = Leg {
            flight: lobshot_sim::ballistics::Flight {
                origin: Vec2::ZERO,
                velocity: Vec2::ZERO,
                accel: Vec2::new(0.0, 600.0),
            },
            from: 0,
            to: TICKS_PER_SECOND,
            offset: 0,
        };
        let drop = lobshot_sim::drops::Drop {
            kind: lobshot_sim::drops::DropKind::Chest,
            x: 0,
        };
        let effects = vec![
            Effect::Exploded {
                at: Vec2::ZERO,
                radius: 30.0,
                tick: TICKS_PER_SECOND,
                dirty: None,
            },
            Effect::Dropped { seat: 2, drop, legs: vec![fall] },
            Effect::Healed { seat: 1, amount: 120 },
            Effect::TurnStarted { seat: 1 },
        ];
        let schedule = schedule(&effects);
        let stage = 1.0 + FALL_SECONDS + TAIL_SECONDS;
        assert_eq!(schedule.at[1], stage);
        assert_eq!(schedule.at[2], stage + 1.0 + DAMAGE_AFTER_BLAST, "the chest heals on landing");
        assert_eq!(schedule.end, stage + 1.0 + DAMAGE_AFTER_BLAST + FALL_SECONDS + TAIL_SECONDS);
    }

    #[test]
    fn sudden_death_holds_its_announcement_before_the_next_turn() {
        let schedule = schedule(&[Effect::SuddenDeath { kind: SuddenDeathKind::Double }, Effect::TurnStarted { seat: 0 }]);
        assert_eq!(
            schedule,
            Schedule {
                at: vec![0.0, SUDDEN_DEATH_SECONDS],
                end: SUDDEN_DEATH_SECONDS
            }
        );
    }

    #[test]
    fn a_spin_and_its_roll_are_instant() {
        let drop = lobshot_sim::drops::Drop {
            kind: lobshot_sim::drops::DropKind::Bolt,
            x: 5,
        };
        assert_eq!(schedule(&[Effect::Spun { seat: 3 }]).end, 0.0);
        assert_eq!(schedule(&[Effect::DropRolled { seat: 3, drop }]).end, 0.0);
    }

    #[test]
    fn bolts_helices_and_bounces_land_on_their_ticks() {
        let effects = vec![
            Effect::Volley { seat: 0, shot: Shot::S2 },
            Effect::Helix {
                seat: 0,
                legs: Vec::new(),
                amplitude: 15.0,
                hertz: 1.0,
                strands: vec![Strand { phase: 0.0, to: TICKS_PER_SECOND }, Strand { phase: 3.0, to: 2 * TICKS_PER_SECOND }],
                merge: None,
            },
            Effect::Bolt {
                from: Vec2::ZERO,
                to: Vec2::new(0.0, 100.0),
                tick: TICKS_PER_SECOND / 2,
            },
            Effect::Shielded { seat: 1, amount: 76 },
            Effect::Bounced { at: Vec2::ZERO, tick: TICKS_PER_SECOND },
        ];
        let schedule = schedule(&effects);
        assert_eq!(schedule.at, [0.0, 0.0, 0.5, 0.5 + DAMAGE_AFTER_BLAST, 1.0]);
        assert_eq!(schedule.end, 2.0 + FALL_SECONDS + TAIL_SECONDS, "the last strand is the last thing to land");
    }

    #[test]
    fn a_walking_sponge_goes_off_as_its_walk_ends_and_the_scene_waits_for_it() {
        let effects = shot_by(MobileKind::Sudsotter, Shot::S1);
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("no shot: {effects:?}")
        };
        let landed = legs.iter().find(|leg| leg.flight.accel == Vec2::ZERO).map(|leg| leg.offset).unwrap();
        let walk = flight_seconds(legs) - ticks_to_seconds(landed);
        assert!((walk - 3.0).abs() < 1e-3, "it walks 3 s: {walk}");
        let schedule = schedule(&effects);
        let blast = effects.iter().position(|e| matches!(e, Effect::Exploded { .. })).unwrap();
        assert_eq!(schedule.at[blast], flight_seconds(legs), "it goes off where its path ends");
        assert!(schedule.end > schedule.at[blast]);
    }

    #[test]
    fn a_guide_keeps_the_scene_going_until_it_lands() {
        let leg = |from: u32, to: u32, offset: u32| Leg {
            flight: lobshot_sim::ballistics::Flight {
                origin: Vec2::ZERO,
                velocity: Vec2::new(100.0, 0.0),
                accel: Vec2::new(0.0, 600.0),
            },
            from,
            to,
            offset,
        };
        let second = TICKS_PER_SECOND;
        let effects = vec![
            Effect::Volley { seat: 0, shot: Shot::S1 },
            Effect::Fired { seat: 0, legs: vec![leg(0, second, 0)] },
            // Herdhound's guide flies on from where the warhead opened, past its missile's strike.
            Effect::Guide {
                seat: 0,
                legs: vec![leg(second, 3 * second, 0)],
            },
            Effect::Fired {
                seat: 0,
                legs: vec![leg(0, second, second)],
            },
            Effect::Exploded {
                at: Vec2::ZERO,
                radius: 20.0,
                tick: 2 * second,
                dirty: None,
            },
        ];
        let schedule = schedule(&effects);
        assert_eq!(schedule.at, [0.0, 0.0, 0.0, 0.0, 2.0]);
    }

    #[test]
    fn an_orbit_lasts_until_its_last_orbiter_stops() {
        let spin = Spin { hertz: 0.5, ramp: 0.5, until: u32::MAX };
        let orbiter = |radius, to| Orbiter { radius, phase: 0.0, to };
        let effects = vec![
            Effect::Volley { seat: 0, shot: Shot::S2 },
            Effect::Orbit {
                seat: 0,
                legs: Vec::new(),
                spin,
                orbiters: vec![orbiter(0.0, TICKS_PER_SECOND), orbiter(35.0, 3 * TICKS_PER_SECOND), orbiter(35.0, 2 * TICKS_PER_SECOND)],
            },
        ];
        let schedule = schedule(&effects);
        assert_eq!(schedule.at, [0.0, 0.0]);
        assert_eq!(schedule.end, 3.0 + FALL_SECONDS + TAIL_SECONDS);
    }

    #[test]
    fn a_satellite_glides_from_its_start_and_its_lasers_strike_on_their_ticks() {
        let second = TICKS_PER_SECOND;
        let effects = vec![
            Effect::Volley { seat: 0, shot: Shot::Ss },
            Effect::Satellite {
                seat: 0,
                at: Vec2::ZERO,
                from: second,
                to: 3 * second,
            },
            Effect::Laser {
                seat: 0,
                from: Vec2::ZERO,
                to: Vec2::new(0.0, 100.0),
                tick: second / 2,
            },
            Effect::Damaged { seat: 1, amount: 90 },
        ];
        let schedule = schedule(&effects);
        assert_eq!(schedule.at, [0.0, 1.0, 0.5, 0.5 + DAMAGE_AFTER_BLAST]);
        assert_eq!(schedule.end, 3.0 + FALL_SECONDS + TAIL_SECONDS, "the scene waits for the glide");
    }

    #[test]
    fn every_effect_of_a_real_game_is_scheduled_in_order_within_the_scene() {
        // An M3 shot of every kind, from a real game: nothing is scheduled past its scene's end.
        for mobile in MobileKind::ALL {
            for shot in [Shot::S1, Shot::S2, Shot::Ss] {
                let effects = shot_by(mobile, shot);
                let schedule = schedule(&effects);
                assert_eq!(schedule.at.len(), effects.len());
                assert!(schedule.at.iter().all(|&t| (0.0..=schedule.end).contains(&t)), "{mobile:?} {shot:?}: {schedule:?}");
                assert!(schedule.end > 0.0);
            }
        }
    }
}
