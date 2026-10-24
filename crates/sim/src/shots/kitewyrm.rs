//! Kitewyrm (Dragon): one light fireball; four rippling out of its mouth in two streams that spread
//! with range (`ripple`); and an SS orb that calls five mini-dragons down from a fan above where it
//! lands. Each hovers a beat where it appeared, then flies straight into that spot, neither gravity
//! nor wind moving it, and goes off on the first thing in its way: land over a target shields it.

use super::{Gun, Rack, land_in_order, ripple};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Leg, Shot},
    tuning::kitewyrm::{
        S1_BLAST, S1_DAMAGE, S2_FIREBALL_BLAST, S2_FIREBALL_DAMAGE, S2_RIPPLE_SECONDS, S2_SPREAD, SS_DISTANCE, SS_DRAGON_ACCEL, SS_DRAGON_BLAST, SS_DRAGON_DAMAGE, SS_DRAGON_SPEED, SS_FAN,
        SS_HOVER_SECONDS, SS_ORB_BLAST, SS_ORB_DAMAGE, SS_STAGGER_SECONDS,
    },
    units::{Vec2, ticks, to_radians},
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    match shot {
        Shot::S1 => {
            let Some(flight) = gun.launch(game, 0, 0.0) else { return };
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => ripple(
            game,
            gun,
            &Rack {
                spread: &S2_SPREAD.value,
                every: ticks(S2_RIPPLE_SECONDS.value),
                blast: S2_FIREBALL_BLAST.value,
                damage: S2_FIREBALL_DAMAGE.value,
                direct_crater: true,
            },
            effects,
        ),
        Shot::Ss => summon(game, gun, effects),
    }
}

/// The SS: the orb lands like any shell, then the mini-dragons come for where it landed, each
/// landing in the order it strikes, carrying what the orb's columns did to it as well as its own.
fn summon(game: &mut Game, gun: &Gun, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    let orb = gun.plain(game, flight, SS_ORB_BLAST.value, SS_ORB_DAMAGE.value, effects);
    if !matches!(orb.impact.contact, Contact::Terrain | Contact::Mobile(_)) {
        return;
    }
    let Some(facing) = game.players().get(usize::from(gun.seat)).map(|player| player.facing.signum()) else {
        return;
    };
    let dragons = mini_dragons(orb.impact.at, orb.impact.tick, facing);
    let (radius, damage) = (SS_DRAGON_BLAST.value, SS_DRAGON_DAMAGE.value);
    let (paths, mut landings) = land_in_order(
        game,
        dragons.len(),
        |game, index| game.track_launched(dragons[index].flight, dragons[index].leaves),
        |game, _, track, out| {
            let marks = orb.marks | track.marks;
            gun.land(game, &gun.blast(track.impact.at, radius, damage, marks.boost()), radius, &track.impact, marks, out);
        },
    );
    effects.extend(dragons.iter().zip(paths).map(|(dragon, legs)| Effect::Fired {
        seat: gun.seat,
        legs: std::iter::once(dragon.hover()).chain(legs).collect(),
    }));
    effects.append(&mut landings);
}

/// One mini-dragon: the stage ticks it appears and sets off at, and its flight from there.
#[derive(Debug, Clone, Copy)]
struct MiniDragon {
    appears: u32,
    leaves: u32,
    flight: Flight,
}

impl MiniDragon {
    /// Its hover where it appeared, until it sets off: a leg standing still.
    fn hover(&self) -> Leg {
        Leg {
            flight: Flight {
                origin: self.flight.origin,
                velocity: Vec2::ZERO,
                accel: Vec2::ZERO,
            },
            from: 0,
            to: self.leaves - self.appears,
            offset: self.appears,
        }
    }
}

/// The mini-dragons an orb that landed at `at` at stage tick `landed` calls, fanned above it from
/// the shooter's side (`facing`, +1 right) forward, one after another, each heading for `at`.
fn mini_dragons(at: Vec2, landed: u32, facing: f64) -> Vec<MiniDragon> {
    let (every, hover) = (ticks(SS_STAGGER_SECONDS.value), ticks(SS_HOVER_SECONDS.value));
    (0_u32..)
        .zip(SS_FAN.value)
        .map(|(order, degrees)| {
            let angle = to_radians(degrees);
            // From the landing up to where it appears; it flies back down this line.
            let out = Vec2::new(facing * libm::sin(angle), -libm::cos(angle));
            let appears = landed + order * every;
            MiniDragon {
                appears,
                leaves: appears + hover,
                flight: Flight {
                    origin: at + out * SS_DISTANCE.value,
                    velocity: -out * SS_DRAGON_SPEED.value,
                    accel: -out * SS_DRAGON_ACCEL.value,
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            MobileKind, Team,
            testing::{damage_to, direct_hit_power, duel_of, explosions, game_on, seat, shoot},
        },
        terrain::Terrain,
        units::{Facing, Power, TICKS_PER_SECOND},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Kitewyrm, MobileKind::Coilback, distance)
    }

    fn fired(effects: &[Effect]) -> Vec<&[Leg]> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.as_slice()) } else { None }).collect()
    }

    /// The stage tick a path starts at, and where it is then.
    fn start(legs: &[Leg]) -> (u32, Vec2) {
        (legs[0].offset + legs[0].from, legs[0].flight.at(legs[0].from))
    }

    fn end(legs: &[Leg]) -> (u32, Vec2) {
        let last = legs.last().unwrap();
        (last.offset + last.to, last.flight.at(last.to))
    }

    #[test]
    fn s1_is_one_light_fireball() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        assert_eq!((fired(&effects).len(), explosions(&effects)), (1, 1), "{effects:?}");
        // 150-250, Explosion against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((150..=250).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_ripples_four_fireballs_out_of_its_mouth() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(2.2));
        let paths = fired(&effects);
        assert_eq!((paths.len(), explosions(&effects)), (4, 4), "{effects:?}");
        let every = ticks(S2_RIPPLE_SECONDS.value);
        let leaving: Vec<u32> = paths.iter().map(|legs| start(legs).0).collect();
        assert_eq!(leaving, [0, every, 2 * every, 3 * every]);
    }

    /// The most `shot` from seat 0 at `aim` deals seat 1, over every power.
    fn most(game: &Game, shot: Shot, aim: i16) -> i32 {
        (0..=Power::MAX.twentieths())
            .map(|t| damage_to(&shoot(&mut game.clone(), 0, shot, aim, Power::from_twentieths(t).unwrap()), 1))
            .max()
            .unwrap()
    }

    #[test]
    fn s2_lands_all_four_point_blank_and_scatters_past_a_screen() {
        // Shotgunned into a target right in front of its mouth, all four land (SW-mob, GF-lee):
        // 75-125 each, Explosion against Mechanical.
        let point_blank = most(&duel(80), Shot::S2, 30);
        assert!(point_blank >= 300, "{point_blank}");
        // More than a screen off, the streams have come apart: never all four (SW-mob).
        let far = most(&duel(1000), Shot::S2, 45);
        assert!(far > 0 && far < 300, "{far}");
    }

    #[test]
    fn ss_orb_calls_five_mini_dragons_from_a_fan_above_where_it_lands() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let paths = fired(&effects);
        assert_eq!(paths.len(), 6, "the orb, then five: {effects:?}");
        let (landed, at) = end(paths[0]);
        let every = ticks(SS_STAGGER_SECONDS.value);
        for (order, dragon) in (0_u32..).zip(&paths[1..]) {
            let (appears, from) = start(dragon);
            assert_eq!(appears, landed + order * every, "dragon {order}");
            assert!((from.distance(at) - SS_DISTANCE.value).abs() < 1e-6, "{order}: {from:?}");
            assert!(from.y < at.y - 200.0, "above the landing: {from:?}");
            assert_eq!(dragon[0].flight.velocity, Vec2::ZERO, "it hovers first");
            assert_eq!(dragon[0].to, ticks(SS_HOVER_SECONDS.value));
            // Then it heads straight for the landing, and strikes near it.
            let (_, struck) = end(dragon);
            assert!(struck.distance(at) < 40.0, "{order}: {struck:?} against {at:?}");
        }
        // The first comes from the shooter's side (seat 0 faces right, so from the left).
        assert!(start(paths[1]).1.x < at.x && start(paths[5]).1.x > at.x);
        assert_eq!(explosions(&effects), 6);
        // The orb's 20-40 and five dragons' 90-130 each, Explosion against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((450..=690).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn ss_mini_dragons_take_a_second_to_arrive_and_ignore_gravity_and_wind() {
        let mut windy = duel(800);
        windy.wind = crate::units::Wind::new(26, 30).unwrap();
        let power = direct_hit_power(&windy, 45);
        let effects = shoot(&mut windy, 0, Shot::Ss, 45, power);
        for dragon in &fired(&effects)[1..] {
            let flying = dragon[1];
            let direction = flying.flight.velocity * (1.0 / flying.flight.velocity.length());
            let along = flying.flight.accel * (1.0 / flying.flight.accel.length());
            assert!(direction.distance(along) < 1e-9, "accelerating straight along its line: {flying:?}");
            let arrives = f64::from(flying.to) / f64::from(TICKS_PER_SECOND);
            assert!((0.85..=1.05).contains(&arrives), "{arrives} s");
        }
    }

    #[test]
    fn ss_mini_dragons_strike_the_first_thing_in_their_way() {
        // In the open, every one strikes the target; under a roof 40 px over its head, every one
        // meets the roof first (SW-mob, WP06).
        let open = Terrain::from_fn(2400, 600, |_, y| y >= 500).unwrap();
        // A slit in the roof at the target's column lets it drop through to the ground as it spawns.
        let roofed = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((800..1400).contains(&x) && x != 1100 && (380..400).contains(&y))).unwrap();
        for (terrain, roof) in [(open, false), (roofed, true)] {
            let game = game_on(
                terrain,
                vec![seat(Team::A, MobileKind::Kitewyrm, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
            );
            let at = game.players()[1].center();
            for dragon in mini_dragons(at, 0, 1.0) {
                let impact = game.track_launched(dragon.flight, dragon.leaves).impact;
                if roof {
                    assert!(impact.contact == Contact::Terrain && (380.0..=400.0).contains(&impact.at.y), "{impact:?}");
                } else {
                    assert_eq!(impact.contact, Contact::Mobile(1));
                }
            }
        }
    }

    #[test]
    fn a_blast_that_burns_low_hurts_it_most_under_its_belly() {
        // Trico players "shotgun their SS at a dragon's belly" (WP06): Thistleback's low-burning
        // blast counts the ground under the hover as Kitewyrm's feet.
        let game = duel(800);
        let kitewyrm = game.players()[0];
        let blast = crate::damage::Blast {
            center: kitewyrm.footing.world(),
            radius: 25.0,
            damage: crate::tuning::DamageRange { min: 50, max: 120 },
            weapon: crate::tuning::WeaponType::Explosion,
            false_angle: false,
            boost: 1.0,
        };
        assert_eq!(crate::damage::low(&blast, &kitewyrm.target()), 120);
    }

    #[test]
    fn mini_dragons_fan_from_the_shooters_side_whichever_way_it_faces() {
        let at = Vec2::new(1000.0, 500.0);
        let right = mini_dragons(at, 0, 1.0);
        let left = mini_dragons(at, 0, -1.0);
        assert_eq!(right.len(), 5);
        for (r, l) in right.iter().zip(&left) {
            assert!((r.flight.origin.x - at.x + (l.flight.origin.x - at.x)).abs() < 1e-9, "mirrored");
            assert!((r.flight.origin.y - l.flight.origin.y).abs() < 1e-9);
        }
        assert!(right[0].flight.origin.x < at.x && left[0].flight.origin.x > at.x);
    }
}
