//! Spoutpot (Turtle): a stream of water; two streams that spiral about one path, crossing over and
//! over until they come together for good (`helix`, merging); and an SS blob that opens in flight
//! into six balls, which fan out about the way it was going and fall faster (`barrage`, from the
//! opening). Every time here is a crossing of S2's spiral (`tuning::spoutpot`).

use super::{
    Gun, barrage_from,
    helix::{self, Helix},
};
use crate::{
    ballistics::Flight,
    game::{Course, Effect, Game, Impact, Senses, Shot, Track},
    tuning::spoutpot::{
        S1_BLAST, S1_DAMAGE, S2_AMPLITUDE, S2_HERTZ, S2_MERGE_CROSSING, S2_PHASES, S2_STREAM_BLAST, S2_STREAM_DAMAGE, SS_BALL_BLAST, SS_BALL_DAMAGE, SS_EARLY_BLAST, SS_EARLY_DAMAGE, SS_FAN,
        SS_SPEED_KEPT, SS_SPLIT_CROSSING, crossing_seconds,
    },
    units::{ticks, to_radians},
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            let shape = Helix {
                amplitude: S2_AMPLITUDE.value,
                hertz: S2_HERTZ.value,
                phases: &S2_PHASES.value,
                merge: Some(ticks(crossing_seconds(S2_MERGE_CROSSING.value))),
            };
            let (legs, strands, mut landings) = helix::fly(game, flight, &shape, |game, _, impact, marks, out| {
                let blast = gun.blast(impact.at, S2_STREAM_BLAST.value, S2_STREAM_DAMAGE.value, marks.boost());
                gun.land(game, &blast, S2_STREAM_BLAST.value, impact, marks, out);
            });
            effects.push(Effect::Helix {
                seat: gun.seat,
                legs,
                amplitude: shape.amplitude,
                hertz: shape.hertz,
                strands,
                merge: shape.merge,
            });
            effects.append(&mut landings);
        }
        Shot::Ss => {
            let Course { track: blob, flying } = game.track_until(flight, 0, 0, Senses::ALL, ticks(crossing_seconds(SS_SPLIT_CROSSING.value)));
            effects.push(Effect::Fired { seat: gun.seat, legs: blob.legs.clone() });
            if flying {
                open(game, gun, &blob, effects);
            } else {
                gun.land(
                    game,
                    &gun.blast_on(&blob, SS_EARLY_BLAST.value, SS_EARLY_DAMAGE.value),
                    SS_EARLY_BLAST.value,
                    &blob.impact,
                    blob.marks,
                    effects,
                );
            }
        }
    }
}

/// The SS blob opens where `blob` stopped: six balls fan out about its direction, each keeping a
/// share of its speed and all of its marks, and land in turn.
fn open(game: &mut Game, gun: &Gun, blob: &Track, effects: &mut Vec<Effect>) {
    let (leg, tick) = (blob.last, blob.last_tick);
    let velocity = leg.flight.velocity_at(tick) * SS_SPEED_KEPT.value;
    let balls: Vec<Flight> = SS_FAN
        .value
        .iter()
        .map(|&degrees| Flight {
            origin: leg.flight.at(tick),
            velocity: velocity.rotated(to_radians(degrees)),
            accel: leg.flight.accel,
        })
        .collect();
    let (paths, mut landings) = barrage_from(game, &balls, leg.offset + tick, |game, _, ball, out| {
        let marks = blob.marks | ball.marks;
        let blast = gun.blast(ball.impact.at, SS_BALL_BLAST.value, SS_BALL_DAMAGE.value, marks.boost());
        gun.land(game, &blast, SS_BALL_BLAST.value, &ball.impact, marks, out);
    });
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// Where a shot on `flight` does its work, for SearchBot's aim pass: the SS's balls fan out about
/// the way the blob was going when it opened, slower, so they come down where a ball flying that
/// way from there would, short of where the blob was headed. One that lands before it opens is no
/// SS at all (`super::ripe`).
pub(super) fn aim_point(game: &Game, shot: Shot, flight: Flight) -> Option<Impact> {
    if shot != Shot::Ss {
        return None;
    }
    let opens = crossing_seconds(SS_SPLIT_CROSSING.value);
    let Course { track: blob, flying } = game.track_until(flight, 0, 0, Senses::ALL, ticks(opens));
    if !flying {
        return super::ripe(blob.impact, opens);
    }
    let (leg, tick) = (blob.last, blob.last_tick);
    let middle = Flight {
        origin: leg.flight.at(tick),
        velocity: leg.flight.velocity_at(tick) * SS_SPEED_KEPT.value,
        accel: leg.flight.accel,
    };
    Some(game.track(middle, 0, leg.offset + tick).impact)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::super::helix::{self, Helix};
    use crate::{
        ballistics,
        game::{
            Contact, Effect, Event, Game, Leg, MobileKind, Shot,
            testing::{damage_to, direct_hit_power, duel_of, explosions, shoot},
        },
        tuning::spoutpot::{S2_AMPLITUDE, S2_HERTZ, S2_MERGE_CROSSING, S2_PHASES, SS_SPLIT_CROSSING, crossing_seconds},
        units::{Centis, Power, Vec2, ticks},
        weather::{Column, WeatherKind},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Spoutpot, MobileKind::Coilback, distance)
    }

    /// The most `shot` at `aim` deals seat 1, over every power step, and the power that does it.
    fn best(game: &Game, shot: Shot, aim: i16) -> (i32, Power) {
        (0..=Power::MAX.twentieths())
            .map(|twentieths| {
                let power = Power::from_twentieths(twentieths).unwrap();
                (damage_to(&shoot(&mut game.clone(), 0, shot, aim, power), 1), power)
            })
            .max_by_key(|&(damage, _)| damage)
            .unwrap()
    }

    fn fired(effects: &[Effect]) -> Vec<&Vec<Leg>> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs) } else { None }).collect()
    }

    #[test]
    fn s1_on_target_deals_its_listed_range() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 35);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 35, power), 1);
        // Physical is weak against Mechanical: 130-160 x 0.96.
        assert!((124..=153).contains(&dealt), "{dealt}");
    }

    #[test]
    fn a_turn_costs_twelve_delay_a_second() {
        let mut game = duel(800);
        let event = Event::Shoot {
            seat: 0,
            shot: Shot::S1,
            aim: 35,
            power: Power::nearest(2.2),
            time: Centis(1000),
        };
        game.apply(event).unwrap();
        assert_eq!(game.players()[0].clock.delay, 490 + 250 + 120);
    }

    #[test]
    fn s2_streams_cross_over_and_over_then_come_together_for_good() {
        let mut game = duel(1200);
        let effects = shoot(&mut game, 0, Shot::S2, 50, Power::nearest(3.0));
        let Some(Effect::Helix {
            legs, amplitude, hertz, strands, merge, ..
        }) = effects.iter().find(|e| matches!(e, Effect::Helix { .. }))
        else {
            panic!("no helix: {effects:?}")
        };
        let merge = merge.expect("Spoutpot's streams come together");
        assert_eq!((*amplitude, *hertz, strands.len()), (S2_AMPLITUDE.value, S2_HERTZ.value, 2));
        assert_eq!(merge, ticks(crossing_seconds(S2_MERGE_CROSSING.value)));
        let leg = legs[0];
        let at = |tick: u32, phase: f64| ballistics::helix_point(&leg.flight, tick, 0, *amplitude, *hertz, phase, Some(merge));
        let apart = |tick: u32| at(tick, strands[0].phase).distance(at(tick, strands[1].phase));
        // Widest halfway between crossings; together on each crossing, and for good from the sixth.
        assert!((apart(ticks(crossing_seconds(4.5))) - 2.0 * amplitude).abs() < 0.1);
        assert!(apart(ticks(crossing_seconds(5.0))) < 0.5);
        assert!(apart(ticks(crossing_seconds(5.5))) > 2.0 * amplitude - 0.1);
        for tick in [merge, merge + 100, merge + 1000] {
            assert_eq!(at(tick, strands[0].phase), leg.flight.at(tick));
            assert_eq!(at(tick, strands[1].phase), leg.flight.at(tick));
        }
    }

    #[test]
    fn s2_with_the_airtime_to_come_together_lands_both_streams_on_one_spot() {
        // A 50-degree shot across 1.4 screens stays up past the sixth crossing.
        let mut game = duel(1100);
        let power = direct_hit_power(&game, 50);
        let flight = game.launch(0, Shot::S2, 50, power.bars()).unwrap();
        assert!(game.track(flight, 0, 0).impact.tick > ticks(crossing_seconds(S2_MERGE_CROSSING.value)));
        let effects = shoot(&mut game, 0, Shot::S2, 50, power);
        let blasts: Vec<Vec2> = effects.iter().filter_map(|e| if let Effect::Exploded { at, .. } = e { Some(*at) } else { None }).collect();
        assert_eq!(blasts.len(), 2, "{effects:?}");
        assert!(blasts[0].distance(blasts[1]) < 1.0, "{blasts:?}");
        // Two Physical hits on Mechanical, each 110-140 x 0.96.
        let dealt = damage_to(&effects, 1);
        assert!((210..=268).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_short_of_coming_together_lands_both_streams_only_as_they_cross() {
        // Half a screen out it lands well before the streams come together: some powers fork the
        // target with both, the rest land only one, a half hit.
        let game = duel(400);
        let hits: Vec<i32> = (0..=Power::MAX.twentieths())
            .map(|twentieths| damage_to(&shoot(&mut game.clone(), 0, Shot::S2, 40, Power::from_twentieths(twentieths).unwrap()), 1))
            .collect();
        assert!(hits.iter().any(|&dealt| dealt > 210), "a fork: {hits:?}");
        assert!(hits.iter().any(|&dealt| (100..=140).contains(&dealt)), "a half hit: {hits:?}");
    }

    #[test]
    fn s2_streams_at_their_widest_pass_either_side_of_a_body() {
        // CR-turtle: at their widest the streams wrap a body without touching it. A path falling
        // straight onto the target's center as they part widest: neither stream strikes it.
        let mut game = duel(800);
        let center = game.players()[1].center();
        let speed = 1200.0;
        let flight = ballistics::Flight {
            origin: Vec2::new(center.x, center.y - speed * crossing_seconds(0.5)),
            velocity: Vec2::new(0.0, speed),
            accel: Vec2::ZERO,
        };
        let shape = Helix {
            amplitude: S2_AMPLITUDE.value,
            hertz: S2_HERTZ.value,
            phases: &S2_PHASES.value,
            merge: None,
        };
        let mut contacts = Vec::new();
        helix::fly(&mut game, flight, &shape, |_, _, impact, _, _| contacts.push(impact.contact));
        assert_eq!(contacts, [Contact::Terrain, Contact::Terrain]);
    }

    #[test]
    fn ss_opens_into_six_balls_in_flight() {
        let mut game = duel(1200);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, Power::nearest(3.0));
        let split = ticks(crossing_seconds(SS_SPLIT_CROSSING.value));
        let paths = fired(&effects);
        assert_eq!(paths.len(), 7, "the blob, then six balls: {effects:?}");
        let blob = paths[0].last().unwrap();
        assert_eq!(blob.offset + blob.to, split, "the blob stops as it opens");
        let opening = blob.flight.at(blob.to);
        for ball in &paths[1..] {
            assert_eq!((ball[0].offset, ball[0].from), (split, 0));
            assert_eq!(ball[0].flight.origin, opening);
            // Each keeps under half the blob's speed, so the six fall away from where it was going.
            assert!(ball[0].flight.velocity.length() < blob.flight.velocity_at(blob.to).length() / 2.0);
        }
        assert_eq!(explosions(&effects), 6);
    }

    #[test]
    fn ss_balls_carry_what_the_blob_passed_through() {
        // CR-turtle: with Electricity "you get a hit for every ball in the SS". The blob passes
        // through the column early on, and each ball calls its own bolt where it lands.
        let mut game = duel(1200);
        let x = game.players()[0].footing.x + 120;
        game.set_columns(vec![Column::new(WeatherKind::Electricity, x)]);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, Power::nearest(3.0));
        assert_eq!(fired(&effects).len(), 7, "{effects:?}");
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count(), 6);
    }

    #[test]
    fn ss_opened_just_short_of_the_target_lands_all_six() {
        // A one-screen shot at 50 degrees opens a split second before it lands (CR-turtle's
        // timebomb), and all six balls hit: Physical on Mechanical, 400-600 x 0.96.
        let (dealt, power) = best(&duel(800), Shot::Ss, 50);
        assert!((384..=576).contains(&dealt), "{dealt} at {power:?}");
    }

    #[test]
    fn ss_landing_before_it_opens_is_a_weak_hit() {
        let mut game = duel(400);
        let power = direct_hit_power(&game, 35);
        let effects = shoot(&mut game, 0, Shot::Ss, 35, power);
        assert_eq!((fired(&effects).len(), explosions(&effects)), (1, 1), "{effects:?}");
        let dealt = damage_to(&effects, 1);
        assert!((64..=96).contains(&dealt), "{dealt}");
    }
}
