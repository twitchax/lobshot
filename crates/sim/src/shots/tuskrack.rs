//! Tuskrack (Bigfoot): a rack of rockets. S1 ripples four missiles off the rack, spread wide; S2
//! throws one bundle of six grenades that bursts into three loose pairs a beat after the muzzle;
//! the SS ripples nine missiles, and those that strike a mobile carve nothing (WP06). Every
//! projectile lands on its own, in the order it hits (`land_in_order`).

use super::{Gun, land_in_order};
use crate::{
    ballistics::{self, Flight},
    game::{Contact, Effect, Game, Leg, Shot},
    tuning::{
        DamageRange,
        tuskrack::{
            S1_MISSILE_BLAST, S1_MISSILE_DAMAGE, S1_RIPPLE_SECONDS, S1_SPREAD, S2_BURST_SECONDS, S2_GRENADE_BLAST, S2_GRENADE_DAMAGE, S2_SPREAD, SS_DIRECT_CRATER, SS_MISSILE_BLAST, SS_MISSILE_DAMAGE,
            SS_RIPPLE_SECONDS, SS_SPREAD,
        },
    },
    units::{Facing, Vec2, ticks, to_radians},
    weather::Marks,
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    match shot {
        Shot::S1 => ripple(
            game,
            gun,
            &Rack {
                spread: &S1_SPREAD.value,
                every: ticks(S1_RIPPLE_SECONDS.value),
                blast: S1_MISSILE_BLAST.value,
                damage: S1_MISSILE_DAMAGE.value,
                direct_crater: true,
            },
            effects,
        ),
        Shot::S2 => burst(game, gun, effects),
        Shot::Ss => ripple(
            game,
            gun,
            &Rack {
                spread: &SS_SPREAD.value,
                every: ticks(SS_RIPPLE_SECONDS.value),
                blast: SS_MISSILE_BLAST.value,
                damage: SS_MISSILE_DAMAGE.value,
                direct_crater: SS_DIRECT_CRATER.value,
            },
            effects,
        ),
    }
}

/// Missiles rippling off the rack.
struct Rack {
    /// Each missile's aim (degrees) and power (bars) off the shot's, in the order they leave.
    spread: &'static [(i16, f64)],
    /// Ticks between one missile leaving and the next.
    every: u32,
    blast: f64,
    damage: DamageRange,
    /// Whether a missile that strikes a mobile carves the ground there.
    direct_crater: bool,
}

/// Fires `rack`'s missiles one after another from where the shooter stands, each its own launch.
fn ripple(game: &mut Game, gun: &Gun, rack: &Rack, effects: &mut Vec<Effect>) {
    let launches: Vec<(Flight, u32)> = (0_u32..)
        .zip(rack.spread)
        .filter_map(|(order, &(aim, bars))| Some((gun.launch(game, aim, bars)?, order * rack.every)))
        .collect();
    let (paths, mut landings) = land_in_order(
        game,
        launches.len(),
        |game, index| game.track_launched(launches[index].0, launches[index].1),
        |game, _, track, out| {
            let crater = if rack.direct_crater || !matches!(track.impact.contact, Contact::Mobile(_)) {
                rack.blast
            } else {
                0.0
            };
            gun.land(game, &gun.blast_on(track, rack.blast, rack.damage), crater, &track.impact, track.marks, out);
        },
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// S2: the bundle flies until it bursts, then each grenade flies on from there with its own turn
/// and power. A bundle that strikes something first goes off there, all six grenades at once (a
/// point-blank "shotgun", GF-max).
fn burst(game: &mut Game, gun: &Gun, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    let Some(facing) = game.players().get(usize::from(gun.seat)).map(|player| player.facing) else {
        return;
    };
    let (radius, damage) = (S2_GRENADE_BLAST.value, S2_GRENADE_DAMAGE.value);
    let bundle = game.track(flight, 0, 0);
    let at = ticks(S2_BURST_SECONDS.value);
    let burst = if bundle.impact.tick > at { up_to(&bundle.legs, at) } else { None };
    let Some((before, burst, tick)) = burst else {
        effects.extend(S2_SPREAD.value.iter().map(|_| Effect::Fired {
            seat: gun.seat,
            legs: bundle.legs.clone(),
        }));
        let blast = gun.blast_on(&bundle, radius, damage);
        let mut blasts = Vec::new();
        for _ in S2_SPREAD.value {
            gun.land(game, &blast, radius, &bundle.impact, bundle.marks, &mut blasts);
        }
        effects.append(&mut blasts);
        return;
    };
    let marks = passed(game, &before);
    let grenades: Vec<Flight> = S2_SPREAD
        .value
        .iter()
        .map(|&(turn, bars)| Flight {
            origin: burst.flight.at(tick),
            velocity: spread(burst.flight.velocity_at(tick), facing, turn, bars),
            accel: burst.flight.accel,
        })
        .collect();
    let offset = burst.offset + tick;
    let (paths, mut landings) = land_in_order(
        game,
        grenades.len(),
        |game, index| game.track(grenades[index], 0, offset),
        |game, _, track, out| {
            let marks = marks | track.marks;
            gun.land(game, &gun.blast(track.impact.at, radius, damage, marks.boost()), radius, &track.impact, marks, out);
        },
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired {
        seat: gun.seat,
        legs: before.iter().copied().chain(legs).collect(),
    }));
    effects.append(&mut landings);
}

/// `legs` flown up to stage tick `at`: every leg until then, the last cut short at `at`, with that
/// last leg and its own tick there. A tick inside a tornado's hold waits for the fling. `None` if
/// the legs end before `at`.
fn up_to(legs: &[Leg], at: u32) -> Option<(Vec<Leg>, Leg, u32)> {
    let mut before = Vec::with_capacity(legs.len());
    for leg in legs {
        let tick = at.saturating_sub(leg.offset).max(leg.from);
        if tick <= leg.to {
            let cut = Leg { to: tick, ..*leg };
            before.push(cut);
            return Some((before, cut, tick));
        }
        before.push(*leg);
    }
    None
}

/// What the columns along `legs` did to a projectile: each kind it passed through marks it once,
/// as `Game::track` marks a whole flight.
fn passed(game: &Game, legs: &[Leg]) -> Marks {
    let mut marks = Marks::default();
    for column in &game.weather().columns {
        if legs.iter().any(|leg| (leg.from..=leg.to).any(|tick| column.contains(leg.flight.at(tick).x))) {
            marks.mark(column.acts_as());
        }
    }
    marks
}

/// `velocity` turned `turn` degrees up, for a shooter facing `facing`, and `bars` faster.
fn spread(velocity: Vec2, facing: Facing, turn: i16, bars: f64) -> Vec2 {
    let speed = velocity.length();
    if speed <= 0.0 {
        return velocity;
    }
    velocity.rotated(to_radians(f64::from(turn)) * facing.signum()) * ((speed + ballistics::launch_speed(bars)) / speed)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Effect, MobileKind, Shot, Team,
            testing::{damage_to, direct_hit_power, duel_of, explosions, game_on, seat, shoot},
        },
        terrain::Terrain,
        units::{Power, TICKS_PER_SECOND},
        weather::{Column, WeatherKind},
    };

    fn paths(effects: &[Effect]) -> Vec<&Vec<Leg>> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs) } else { None }).collect()
    }

    /// When each path leaves: its first leg's start.
    fn leaves(legs: &[Leg]) -> u32 {
        legs.first().map_or(0, |leg| leg.offset + leg.from)
    }

    #[test]
    fn s1_ripples_four_missiles_off_the_rack() {
        let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 700);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        let paths = paths(&effects);
        let every = ticks(S1_RIPPLE_SECONDS.value);
        assert_eq!(paths.iter().map(|legs| leaves(legs)).collect::<Vec<_>>(), [0, every, 2 * every, 3 * every]);
        assert_eq!(explosions(&effects), 4, "{effects:?}");
        // Explosion damage is neutral on Mechanical: the two down the line land direct, 42-57 each.
        let dealt = damage_to(&effects, 1);
        assert!((84..=228).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s1_spreads_wider_the_farther_it_flies() {
        let spread_at = |distance: i32| {
            let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, distance);
            let power = direct_hit_power(&game, 30);
            let effects = shoot(&mut game, 0, Shot::S1, 30, power);
            let xs: Vec<f64> = effects.iter().filter_map(|e| if let Effect::Exploded { at, .. } = e { Some(at.x) } else { None }).collect();
            xs.iter().copied().fold(f64::MIN, f64::max) - xs.iter().copied().fold(f64::MAX, f64::min)
        };
        let (near, far) = (spread_at(300), spread_at(1000));
        assert!(far > 2.0 * near && far > 60.0, "{near} then {far}");
    }

    #[test]
    fn the_two_missiles_down_the_line_dig_one_deeper_crater() {
        let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 1200);
        // Short of the target: all four land in open ground.
        let effects = shoot(&mut game, 0, Shot::S1, 30, Power::nearest(1.6));
        let craters: Vec<Vec2> = effects.iter().filter_map(|e| if let Effect::Exploded { at, .. } = e { Some(*at) } else { None }).collect();
        assert_eq!(craters.len(), 4);
        assert_eq!(damage_to(&effects, 1), 0);
        // The fourth flies the third's path into its crater and goes off deeper down.
        assert!((craters[3].x - craters[2].x).abs() < S1_MISSILE_BLAST.value && craters[3].y > craters[2].y + 5.0, "{craters:?}");
    }

    #[test]
    fn s2_bursts_into_three_pairs_a_beat_after_the_muzzle() {
        let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S2, 30, power);
        let paths = paths(&effects);
        assert_eq!(paths.len(), 6);
        let burst = ticks(S2_BURST_SECONDS.value);
        for legs in &paths {
            // One bundle until the burst, then a grenade of its own.
            assert_eq!(legs[0], paths[0][0]);
            assert_eq!((legs[0].to, legs[1].offset), (burst, burst));
            assert_eq!(legs[1].flight.origin, legs[0].flight.at(burst));
        }
        let mut velocities: Vec<Vec2> = paths.iter().map(|legs| legs[1].flight.velocity).collect();
        velocities.dedup();
        assert_eq!(velocities.len(), 6, "each its own way: {velocities:?}");
        let landed: Vec<f64> = effects.iter().filter_map(|e| if let Effect::Exploded { at, .. } = e { Some(at.x) } else { None }).collect();
        assert_eq!(landed.len(), 6);
        let reach = landed.iter().copied().fold(f64::MIN, f64::max) - landed.iter().copied().fold(f64::MAX, f64::min);
        assert!(reach > 60.0, "spread out: {landed:?}");
        // The middle pair, the bundle's own flight, lands on the target.
        assert!(damage_to(&effects, 1) >= 66, "{effects:?}");
    }

    #[test]
    fn s2_point_blank_goes_off_all_at_once() {
        // Up against a target standing on a ledge as high as the rack: it strikes before it can burst.
        let ledge = Terrain::from_fn(1200, 600, |x, y| y >= if x < 360 { 500 } else { 480 }).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Tuskrack, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 400, Facing::Left)];
        let mut game = game_on(ledge, seats);
        let effects = shoot(&mut game, 0, Shot::S2, 23, Power::nearest(1.0));
        assert_eq!(explosions(&effects), 6, "{effects:?}");
        let ticks: Vec<u32> = effects.iter().filter_map(|e| if let Effect::Exploded { tick, .. } = e { Some(*tick) } else { None }).collect();
        assert!(ticks.iter().all(|&tick| tick == ticks[0]) && ticks[0] <= super::ticks(S2_BURST_SECONDS.value), "{ticks:?}");
        // All six land direct: 33-45 each.
        assert!(damage_to(&effects, 1) >= 6 * 33, "{effects:?}");
    }

    #[test]
    fn a_column_the_bundle_crossed_before_bursting_marks_every_grenade() {
        let calm = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 800);
        let power = direct_hit_power(&calm, 30);
        let mut forced = calm.clone();
        // A Force beam just ahead of the muzzle: the bundle crosses it before it bursts.
        let x = forced.players()[0].footing.x + 50;
        forced.set_columns(vec![Column::new(WeatherKind::Force, x)]);
        let calm_damage = damage_to(&shoot(&mut calm.clone(), 0, Shot::S2, 30, power), 1);
        let forced_damage = damage_to(&shoot(&mut forced, 0, Shot::S2, 30, power), 1);
        assert!(f64::from(forced_damage) > 1.25 * f64::from(calm_damage), "{forced_damage} against {calm_damage}");
    }

    #[test]
    fn ss_ripples_nine_missiles() {
        let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 500);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::Ss, 30, power);
        let every = ticks(SS_RIPPLE_SECONDS.value);
        let leave: Vec<u32> = paths(&effects).iter().map(|legs| leaves(legs)).collect();
        assert_eq!(leave, (0..9).map(|i| i * every).collect::<Vec<_>>());
        assert_eq!(explosions(&effects), 9);
        assert!(damage_to(&effects, 1) >= 3 * 38, "{effects:?}");
    }

    #[test]
    fn an_ss_missile_that_strikes_a_mobile_carves_nothing() {
        // Far enough that the spread leaves some short and some long.
        let radii = |shot| {
            let mut game = duel_of(MobileKind::Tuskrack, MobileKind::Coilback, 1000);
            let power = direct_hit_power(&game, 30);
            let effects = shoot(&mut game, 0, shot, 30, power);
            effects
                .iter()
                .filter_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None })
                .collect::<Vec<f64>>()
        };
        let (ss, s1) = (radii(Shot::Ss), radii(Shot::S1));
        assert!(ss.contains(&0.0) && ss.contains(&SS_MISSILE_BLAST.value), "some strike the target, some the ground: {ss:?}");
        assert!(!s1.contains(&0.0), "S1's do carve: {s1:?}");
    }

    #[test]
    fn a_burst_inside_a_tornado_hold_waits_for_the_fling() {
        let leg = |offset: u32, from: u32, to: u32| Leg {
            flight: Flight {
                origin: Vec2::ZERO,
                velocity: Vec2::new(100.0, -100.0),
                accel: Vec2::new(0.0, 600.0),
            },
            from,
            to,
            offset,
        };
        let legs = [leg(0, 0, 100), leg(100 + TICKS_PER_SECOND, 0, 900)];
        let (before, at, tick) = up_to(&legs, 400).unwrap();
        assert_eq!((before.len(), at.offset, tick), (2, 100 + TICKS_PER_SECOND, 0));
        let (before, at, tick) = up_to(&legs, 50).unwrap();
        assert_eq!((before.len(), at.to, tick), (1, 50, 50));
        assert!(up_to(&legs, 100_000).is_none());
    }

    #[test]
    fn a_grenade_turns_up_for_either_facing() {
        let right = spread(Vec2::new(100.0, -100.0), Facing::Right, 10, 0.0);
        let left = spread(Vec2::new(-100.0, -100.0), Facing::Left, 10, 0.0);
        assert!(right.y < -100.0 && left.y < -100.0, "{right:?} {left:?}");
        assert!((right.x + left.x).abs() < 1e-9);
        let faster = spread(Vec2::new(300.0, 0.0), Facing::Right, 0, 1.0);
        assert!((faster.x - 600.0).abs() < 1e-9);
    }
}
