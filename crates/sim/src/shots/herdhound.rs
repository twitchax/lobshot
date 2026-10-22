//! Herdhound (Kalsiddon): every shot is a warhead that passes through mobiles and opens at its
//! apex into a guide, which flies on along the shot's path, and missiles that loop a second, then
//! home on it (`homing`). S1 opens into two missiles, S2 into four. The SS opens into four that
//! split in two as they stop looping, each half homing on a guide of its own: eight guides fanned
//! about the shot's path, beside the main one. A warhead that touches the ground first is a dud.

use super::{
    Gun,
    homing::{self, Herd, Missile, Opening, STEP},
};
use crate::{
    ballistics::Flight,
    game::{Effect, Game, Shot, Track},
    tuning::{
        DamageRange,
        herdhound::{
            DUD_BLAST, DUD_DAMAGE, HOMING_TURN, LOOP_SECONDS, LOOP_TURNS, MISSILE_SECONDS, MISSILE_SPEED, S1_BURST, S1_MISSILE_BLAST, S1_MISSILE_DAMAGE, S2_BURST, S2_MISSILE_BLAST, S2_MISSILE_DAMAGE,
            SS_BURST, SS_GUIDE_SPREAD, SS_MISSILE_BLAST, SS_MISSILE_DAMAGE, SS_SPLIT,
        },
    },
    units::{Vec2, ticks, to_radians},
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    let opened = match homing::open(game, flight) {
        Opening::Opened(track) => track,
        Opening::Dud(track) => {
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            gun.land(game, &gun.blast_on(&track, DUD_BLAST.value, DUD_DAMAGE.value), DUD_BLAST.value, &track.impact, track.marks, effects);
            return;
        }
    };
    // The main guide flies on along the shot's own path.
    let mut guides = vec![game.track(opened.last.flight, opened.last_tick, opened.last.offset)];
    let (missiles, damage, radius): (Vec<Missile>, DamageRange, f64) = match shot {
        Shot::S1 => (singles(&S1_BURST.value), S1_MISSILE_DAMAGE.value, S1_MISSILE_BLAST.value),
        Shot::S2 => (singles(&S2_BURST.value), S2_MISSILE_DAMAGE.value, S2_MISSILE_BLAST.value),
        Shot::Ss => {
            guides.extend(SS_GUIDE_SPREAD.value.iter().map(|&spread| fanned(game, &opened, spread)));
            (pairs(), SS_MISSILE_DAMAGE.value, SS_MISSILE_BLAST.value)
        }
    };
    let facing = game.players()[usize::from(gun.seat)].facing.signum();
    let (paths, mut landings) = homing::fly(game, &herd(), &opened, &guides, &missiles, Vec2::new(facing, 0.0), |game, _, impact, marks, out| {
        let blast = gun.blast(impact.at, radius, damage, marks.boost());
        gun.land(game, &blast, radius, impact, marks, out);
    });
    effects.push(Effect::Fired { seat: gun.seat, legs: opened.legs });
    effects.extend(guides.into_iter().map(|guide| Effect::Guide { seat: gun.seat, legs: guide.legs }));
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// How Herdhound's missiles fly (`tuning::herdhound`).
fn herd() -> Herd {
    let loop_steps = ticks(LOOP_SECONDS.value) / STEP;
    Herd {
        speed: MISSILE_SPEED.value,
        loop_steps,
        loop_turn: LOOP_TURNS.value * 2.0 * core::f64::consts::PI / LOOP_SECONDS.value,
        homing_turn: HOMING_TURN.value,
        lifetime: ticks(MISSILE_SECONDS.value),
    }
}

/// Missiles bursting out at `burst` degrees, each homing on the main guide.
fn singles(burst: &[f64]) -> Vec<Missile> {
    burst
        .iter()
        .map(|&degrees| Missile {
            heading: to_radians(degrees),
            homes: vec![(0.0, 0)],
        })
        .collect()
}

/// The SS's four, each splitting into two that home on guides of their own: the `i`-th half on
/// fanned guide `i` (guide 0 is the main one).
fn pairs() -> Vec<Missile> {
    (0..)
        .zip(SS_BURST.value)
        .map(|(pair, degrees): (usize, f64)| Missile {
            heading: to_radians(degrees),
            homes: (0..).zip(SS_SPLIT.value).map(|(half, split): (usize, f64)| (to_radians(split), 1 + 2 * pair + half)).collect(),
        })
        .collect()
}

/// A guide leaving where the warhead opened, `spread` px/s sideways off its velocity there.
fn fanned(game: &Game, opened: &Track, spread: f64) -> Track {
    let (last, tick) = (opened.last, opened.last_tick);
    let flight = Flight {
        origin: last.flight.at(tick),
        velocity: last.flight.velocity_at(tick) + Vec2::new(spread, 0.0),
        accel: last.flight.accel,
    };
    game.track(flight, 0, last.offset + tick)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::{
        herd,
        homing::{Opening, open},
    };
    use crate::{
        game::{
            Effect, Game, Leg, MobileKind, Shot,
            testing::{damage_to, direct_hit_power, duel_of, explosions, shoot},
        },
        terrain::Terrain,
        tuning::herdhound::{DUD_BLAST, LOOP_SECONDS},
        units::{Power, Vec2, ticks},
        weather::{Column, WeatherKind},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Herdhound, MobileKind::Coilback, distance)
    }

    fn fired(effects: &[Effect]) -> Vec<&[Leg]> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.as_slice()) } else { None }).collect()
    }

    fn guides(effects: &[Effect]) -> Vec<&[Leg]> {
        effects.iter().filter_map(|e| if let Effect::Guide { legs, .. } = e { Some(legs.as_slice()) } else { None }).collect()
    }

    fn blasts(effects: &[Effect]) -> Vec<(Vec2, u32)> {
        effects
            .iter()
            .filter_map(|e| if let Effect::Exploded { at, tick, .. } = e { Some((*at, *tick)) } else { None })
            .collect()
    }

    /// The stage tick a path starts at, and where it ends.
    fn start(legs: &[Leg]) -> u32 {
        legs[0].offset + legs[0].from
    }

    fn end(legs: &[Leg]) -> Vec2 {
        let last = legs.last().unwrap();
        last.flight.at(last.to)
    }

    /// Where a path is at stage tick `tick`.
    fn at(legs: &[Leg], tick: u32) -> Vec2 {
        let leg = legs.iter().find(|leg| (leg.offset + leg.from..=leg.offset + leg.to).contains(&tick)).unwrap();
        leg.flight.at(tick - leg.offset)
    }

    #[test]
    fn s1_opens_at_its_apex_into_two_missiles_that_strike_where_the_guide_lands() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        let paths = fired(&effects);
        assert_eq!(paths.len(), 3, "the warhead, then two missiles: {effects:?}");
        let warhead = paths[0].last().unwrap();
        assert!(
            warhead.flight.velocity_at(warhead.to).y >= 0.0 && warhead.flight.velocity_at(warhead.to - 1).y < 0.0,
            "it opens as it tops out"
        );
        let opened = warhead.offset + warhead.to;
        let guide = guides(&effects);
        assert_eq!(guide.len(), 1);
        assert!(guide.iter().chain(&paths[1..]).all(|path| start(path) == opened), "they leave the warhead as it opens");
        let landed = end(guide[0]);
        let strikes = blasts(&effects);
        assert_eq!(strikes.len(), 2);
        assert!(strikes.iter().all(|(at, _)| at.distance(landed) < 20.0), "where the guide landed, {landed:?}: {strikes:?}");
        // 75-110 each, Explosion against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((150..=220).contains(&dealt), "{dealt}");
        assert_eq!(damage_to(&effects, 0), 0);
    }

    #[test]
    fn missiles_loop_twice_in_a_second_then_home() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        let paths = fired(&effects);
        let opened = start(paths[1]);
        let burst = at(paths[1], opened);
        let (turn, loops) = (ticks(LOOP_SECONDS.value) / 2, ticks(LOOP_SECONDS.value));
        for missile in &paths[1..] {
            assert!(at(missile, opened + turn / 2).distance(burst) > 50.0, "half a turn takes it a loop's width away");
            assert!(at(missile, opened + turn).distance(burst) < 1.0, "one turn brings it back");
            assert!(at(missile, opened + loops).distance(burst) < 1.0, "and two");
        }
        // Then each heads for the guide.
        let guide = end(guides(&effects)[0]);
        assert!(paths[1..].iter().all(|missile| at(missile, opened + loops + 400).distance(guide) < burst.distance(guide)));
    }

    #[test]
    fn s2_opens_into_four_missiles() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        assert_eq!(fired(&effects).len(), 5);
        assert_eq!(explosions(&effects), 4);
        // 63-100 each.
        let dealt = damage_to(&effects, 1);
        assert!((252..=400).contains(&dealt), "{dealt}");
    }

    #[test]
    fn ss_opens_into_four_that_split_into_eight_each_homing_on_a_guide_of_its_own() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let paths = fired(&effects);
        assert_eq!(paths.len(), 1 + 4 + 8, "the warhead, the four, their eight halves");
        let guides = guides(&effects);
        assert_eq!(guides.len(), 9, "the main guide and one for each half");
        let opened = start(paths[1]);
        let split = opened + herd().loop_steps * super::STEP;
        let pairs = &paths[1..5];
        assert!(
            pairs.iter().all(|path| path.last().map(|leg| leg.offset + leg.to) == Some(split)),
            "the four split as they stop looping"
        );
        assert!(paths[5..].iter().all(|path| start(path) == split));
        let strikes = blasts(&effects);
        assert_eq!(strikes.len(), 8);
        let (left, right) = strikes.iter().fold((f64::MAX, f64::MIN), |(l, r), (at, _)| (l.min(at.x), r.max(at.x)));
        assert!(right - left > 30.0, "the eight land spread out: {strikes:?}");
        // 38-81 each; not all land on the target.
        let dealt = damage_to(&effects, 1);
        assert!((300..=650).contains(&dealt), "{dealt}");
    }

    #[test]
    fn a_warhead_that_touches_the_ground_before_its_apex_is_a_dud() {
        let mut game = duel(800);
        // A wall in front of the shooter.
        game.terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || (380..400).contains(&x)).unwrap();
        let effects = shoot(&mut game, 0, Shot::S2, 30, Power::nearest(3.0));
        assert_eq!(fired(&effects).len(), 1, "no missiles: {effects:?}");
        assert_eq!(guides(&effects).len(), 0);
        let craters: Vec<f64> = effects.iter().filter_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None }).collect();
        assert_eq!(craters, [DUD_BLAST.value]);
    }

    #[test]
    fn a_warhead_passes_through_mobiles_and_one_opened_inside_a_mobile_strikes_it_at_once() {
        let game = duel(150);
        let frames = game.hit_frames();
        let target = &frames[1].1;
        let range = MobileKind::Herdhound.tuning().s2.aim.value;
        let (aim, power) = (range.true_min..=range.true_max)
            .flat_map(|aim| (0..=80_u8).map(move |power| (aim, Power::from_twentieths(power).unwrap())))
            .find(|&(aim, power)| match open(&game, game.launch(0, aim, power.bars()).unwrap()) {
                Opening::Opened(track) => target.contains(track.last.flight.at(track.last_tick)),
                Opening::Dud(_) => false,
            })
            .expect("some shot opens inside the target");
        let mut game = game;
        let effects = shoot(&mut game, 0, Shot::S2, aim, power);
        let missiles = &fired(&effects)[1..];
        assert_eq!(missiles.len(), 4);
        assert!(missiles.iter().all(|path| path.len() == 1 && path[0].from == path[0].to), "each strikes as it bursts out: {effects:?}");
        assert!(damage_to(&effects, 1) >= 4 * 63, "{effects:?}");
    }

    #[test]
    fn missiles_too_close_to_the_ground_crash_while_they_loop() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::S2, 10, Power::nearest(1.0));
        let opened = start(fired(&effects)[1]);
        let loops = ticks(LOOP_SECONDS.value);
        assert!(blasts(&effects).iter().any(|&(_, tick)| tick < opened + loops), "{effects:?}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn missiles_carry_the_force_their_warhead_passed_through() {
        let mut calm = duel(800);
        let power = direct_hit_power(&calm, 45);
        let mut forced = calm.clone();
        // On the warhead's way up, well short of where it opens.
        forced.set_columns(vec![Column::new(WeatherKind::Force, 450)]);
        let calm_damage = damage_to(&shoot(&mut calm, 0, Shot::S1, 45, power), 1);
        let forced_damage = damage_to(&shoot(&mut forced, 0, Shot::S1, 45, power), 1);
        assert!((f64::from(forced_damage) / f64::from(calm_damage) - 1.33).abs() < 0.02, "{forced_damage} against {calm_damage}");
    }

    #[test]
    fn missiles_follow_their_guide_through_a_tornado() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        // Between where the warhead opens and the target.
        game.set_columns(vec![Column::new(WeatherKind::Tornado, 950)]);
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        let guide = guides(&effects)[0];
        assert!(guide.len() >= 2, "the tornado catches the guide: {guide:?}");
        let landed = end(guide);
        assert!(blasts(&effects).iter().all(|(at, _)| at.distance(landed) < 30.0), "{landed:?}: {effects:?}");
    }
}
