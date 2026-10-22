//! Thistleback (Trico): a heavy cabbage; a cabbage with two sprouts circling it, each its own hit
//! (`orbit`); and an SS pumpkin that bursts into a ring of blasts spinning round where it lands,
//! hottest at a target's feet.

use super::{
    Gun,
    orbit::{self, Orbit},
};
use crate::{
    damage::{self, Blast},
    game::{Contact, Effect, Game, Shot, Track},
    tuning::thistleback::{S1_BLAST, S1_DAMAGE, S2_BALL_BLAST, S2_BALL_DAMAGE, S2_HERTZ, S2_ORBITERS, S2_RAMP_SECONDS, SS_BLAST, SS_BLAST_DAMAGE, SS_RING, SS_RING_RADIUS, SS_RING_SECONDS},
    units::{Vec2, ticks},
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            let shape = Orbit {
                hertz: S2_HERTZ.value,
                ramp: S2_RAMP_SECONDS.value,
                orbiters: &S2_ORBITERS.value,
            };
            let (legs, spin, orbiters, mut landings) = orbit::fly(game, flight, &shape, |game, _, impact, marks, out| {
                let blast = gun.blast(impact.at, S2_BALL_BLAST.value, S2_BALL_DAMAGE.value, marks.boost());
                gun.land(game, &blast, S2_BALL_BLAST.value, impact, marks, out);
            });
            effects.push(Effect::Orbit { seat: gun.seat, legs, spin, orbiters });
            effects.append(&mut landings);
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            burst(game, gun, &track, effects);
        }
    }
}

/// The SS pumpkin bursts where it lands (`burn`), and one bolt follows if Thunder is armed or it
/// passed through Electricity. One that left the map does nothing.
fn burst(game: &mut Game, gun: &Gun, track: &Track, effects: &mut Vec<Effect>) {
    if matches!(track.impact.contact, Contact::Gone | Contact::Column(_)) {
        return;
    }
    let blast = gun.blast_on(track, SS_BLAST.value, SS_BLAST_DAMAGE.value);
    burn(game, &blast, SS_BLAST.value * gun.salvo.crater, track.impact.tick, effects);
    gun.follow_up(game, blast.center, track.impact.tick, track.marks, effects);
}

/// Blasts like `blast`, carving craters of `crater`: one at its center at `tick`, then `SS_RING`
/// evenly round it, clockwise on screen from the top, `SS_RING_SECONDS` apart. Each burns low
/// (`damage::low`): worth most at a mobile's feet, or in the ground under them.
fn burn(game: &mut Game, blast: &Blast, crater: f64, tick: u32, effects: &mut Vec<Effect>) {
    let gap = ticks(SS_RING_SECONDS.value);
    let ring = SS_RING.value;
    for step in 0..=ring {
        let center = if step == 0 {
            blast.center
        } else {
            // Angles grow clockwise on screen (y points down), and a quarter turn back from +x is
            // the top.
            let angle = core::f64::consts::PI * (2.0 * f64::from(step - 1) / f64::from(ring) - 0.5);
            blast.center + Vec2::new(libm::cos(angle), libm::sin(angle)) * SS_RING_RADIUS.value
        };
        let blast = Blast { center, ..*blast };
        game.explode_by(&blast, crater, tick + gap * step, effects, |_, target| damage::low(&blast, target));
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, MobileKind, Team,
            testing::{damage_to, direct_hit_power, duel_of, explosions, game_of, seat, shoot},
        },
        items::{Item, Loadout},
        tuning::WeaponType,
        units::{Centis, Facing, Power},
        weather::{Column, WeatherKind},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Thistleback, MobileKind::Coilback, distance)
    }

    fn orbit_of(effects: &[Effect]) -> (Vec<crate::game::Leg>, crate::ballistics::Spin, Vec<crate::game::Orbiter>) {
        effects
            .iter()
            .find_map(|e| {
                if let Effect::Orbit { legs, spin, orbiters, .. } = e {
                    Some((legs.clone(), *spin, orbiters.clone()))
                } else {
                    None
                }
            })
            .expect("an orbit")
    }

    #[test]
    fn s1_on_target_deals_its_listed_range() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 30);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 30, power), 1);
        // Explosion is neutral against every armor.
        assert!((100..=150).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_flies_three_cabbages_two_circling_the_middle_one_clockwise() {
        for facing in [Facing::Right, Facing::Left] {
            // Mid-map, so it lands either way.
            let mut game = game_of(vec![seat(Team::A, MobileKind::Thistleback, 1200, facing), seat(Team::B, MobileKind::Coilback, 2300, Facing::Left)]);
            let effects = shoot(&mut game, 0, Shot::S2, 40, Power::nearest(1.6));
            let (legs, spin, orbiters) = orbit_of(&effects);
            assert_eq!(orbiters.iter().map(|o| o.radius).collect::<Vec<_>>(), [0.0, 35.0, 35.0]);
            assert_eq!(explosions(&effects), 3, "{facing:?}: three cabbages, three blasts: {effects:?}");
            let leg = legs[0];
            let offset = |tick: u32, orbiter: usize| {
                let o = orbiters[orbiter];
                crate::ballistics::orbit_point(&leg.flight, tick, leg.offset, &spin, o.radius, o.phase) - leg.flight.at(tick)
            };
            // Past the swing out, the middle one is on the path and the others 35 px off it, opposite.
            let (early, late) = (crate::units::ticks(0.6), crate::units::ticks(0.8));
            assert!(offset(early, 0).length() < 1e-9);
            assert!((offset(early, 1).length() - 35.0).abs() < 1e-9);
            assert!((offset(early, 1) + offset(early, 2)).length() < 1e-9);
            // On screen (y down), a positive cross product turns clockwise, whichever way it flies.
            let (a, b) = (offset(early, 1), offset(late, 1));
            assert!(a.x * b.y - a.y * b.x > 0.0, "{facing:?}: {a:?} then {b:?}");
        }
    }

    #[test]
    fn s2_sprouts_swing_out_from_the_muzzle_and_spare_the_shooter() {
        for aim in [10, 25, 50] {
            for power in [0.6, 1.2, 2.4, 4.0] {
                let mut game = duel(800);
                let effects = shoot(&mut game, 0, Shot::S2, aim, Power::nearest(power));
                assert_eq!(damage_to(&effects, 0), 0, "{aim} at {power}: {effects:?}");
            }
        }
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::S2, 30, Power::nearest(2.4));
        let (legs, spin, orbiters) = orbit_of(&effects);
        let at = crate::ballistics::orbit_point(&legs[0].flight, 0, 0, &spin, orbiters[1].radius, orbiters[1].phase);
        assert!(at.distance(legs[0].flight.at(0)) < 1e-9, "all three leave the spoon together");
    }

    #[test]
    fn s2_on_target_can_land_all_three() {
        // The best of the true angles and the power steps that reach: GunBound's "if all hit"
        // (SW-mob).
        let best = (20..=40)
            .step_by(2)
            .flat_map(|aim| (44..=64).map(move |twentieths| (aim, twentieths)))
            .map(|(aim, twentieths)| {
                let mut game = duel(800);
                let effects = shoot(&mut game, 0, Shot::S2, aim, Power::from_twentieths(twentieths).unwrap());
                damage_to(&effects, 1)
            })
            .max()
            .unwrap();
        assert!(best > 2 * 103, "more than two cabbages' max: {best}");
        assert!(best <= 3 * 103, "{best}");
    }

    #[test]
    fn a_tornado_stops_the_circling() {
        let mut calm = duel(1500);
        let mut game = calm.clone();
        let x = game.players()[0].footing.x + 300;
        game.set_columns(vec![Column::new(WeatherKind::Tornado, x)]);
        let effects = shoot(&mut game, 0, Shot::S2, 40, Power::nearest(2.0));
        let (legs, spin, orbiters) = orbit_of(&effects);
        assert!(legs.len() >= 2, "the tornado broke the path: {legs:?}");
        assert_eq!(spin.until, legs[0].offset + legs[0].to, "it stops where the tornado caught it");
        let held = legs[1];
        let o = orbiters[1];
        let offset = |tick: u32| crate::ballistics::orbit_point(&held.flight, tick, held.offset, &spin, o.radius, o.phase) - held.flight.at(tick);
        assert!((offset(0) - offset(held.to)).length() < 1e-9, "it holds its place about the path");
        let (_, calm_spin, _) = orbit_of(&shoot(&mut calm, 0, Shot::S2, 40, Power::nearest(2.0)));
        assert_eq!(calm_spin.until, u32::MAX);
    }

    #[test]
    fn ss_bursts_nine_times_spinning_round_where_it_lands() {
        let mut game = duel(1500);
        let effects = shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(1.6));
        let blasts: Vec<(Vec2, f64, u32)> = effects
            .iter()
            .filter_map(|e| if let Effect::Exploded { at, radius, tick, .. } = e { Some((*at, *radius, *tick)) } else { None })
            .collect();
        assert_eq!(blasts.len(), 9, "{effects:?}");
        let (first, _, start) = blasts[0];
        let gap = ticks(0.1);
        for (step, &(at, radius, tick)) in (0_u32..).zip(&blasts) {
            assert_eq!((radius, tick), (25.0, start + gap * step));
            if step > 0 {
                assert!((at.distance(first) - 20.0).abs() < 1e-9, "{step}: {at:?}");
            }
        }
        assert!(blasts[1].0.y < first.y - 19.0, "the ring starts at the top: {blasts:?}");
        assert!(blasts[3].0.x > first.x + 19.0, "and turns clockwise: {blasts:?}");
        assert_eq!(damage_to(&effects, 1), 0, "far from the target");
        // A hole about 90 px across.
        for dx in [-40.0, 0.0, 40.0] {
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = ((first.x + dx) as i32, (first.y + 2.0) as i32);
            assert!(!game.terrain().is_solid(x, y), "{dx}");
        }
    }

    /// Everything a pumpkin bursting at `at` deals seat 1, a Coilback on flat ground.
    fn burnt(at: impl Fn(&crate::game::Player) -> Vec2) -> i32 {
        let mut game = duel(800);
        let blast = Blast {
            center: at(&game.players()[1]),
            radius: SS_BLAST.value,
            damage: SS_BLAST_DAMAGE.value,
            weapon: WeaponType::Explosion,
            false_angle: false,
            boost: 1.0,
        };
        let mut effects = Vec::new();
        burn(&mut game, &blast, SS_BLAST.value, 0, &mut effects);
        damage_to(&effects, 1)
    }

    #[test]
    fn ss_burns_hottest_at_the_feet() {
        // SW-mob and DB-mob's 250-500: the fit's two points, the foot of the target's front and
        // the top of its body.
        let hitbox = MobileKind::Coilback.tuning().hitbox.value;
        let front = hitbox.x + hitbox.w;
        let feet = burnt(|p| p.pose().to_world(Vec2::new(front - 0.5, -0.5)));
        let top = burnt(|p| p.pose().to_world(Vec2::new(hitbox.x + hitbox.w / 2.0, hitbox.y)));
        assert!((feet - 500).abs() <= 5, "at the front foot: {feet}");
        assert!((top - 250).abs() <= 5, "on top: {top}");
        // The ground at its feet counts as its feet; higher up its front is worth less, and less
        // the farther off it lands.
        let ground = burnt(|p| p.pose().to_world(Vec2::new(front + 1.0, 0.5)));
        let chest = burnt(|p| p.pose().to_world(Vec2::new(front - 0.5, hitbox.y / 2.0)));
        let short = burnt(|p| p.pose().to_world(Vec2::new(front + 15.0, 0.5)));
        assert!(feet > ground && ground > chest && chest > top && ground > short, "{feet} {ground} {chest} {top} {short}");
    }

    #[test]
    fn ss_at_the_feet_out_hurts_a_hit_up_top() {
        // Through the real shot, at the true angles and the power steps that reach: the best
        // pumpkin landing low on the target (or the ground at its feet) beats the best striking
        // high on its body.
        let hitbox = MobileKind::Coilback.tuning().hitbox.value;
        let (mut low, mut high) = (0, 0);
        for aim in (20..=40).step_by(2) {
            for twentieths in 44..=64 {
                let mut game = duel(800);
                let power = Power::from_twentieths(twentieths).unwrap();
                let Some(flight) = game.launch(0, Shot::Ss, aim, power.bars()) else { continue };
                let impact = game.track(flight, 0, 0).impact;
                let target = game.players()[1].pose();
                let at = target.to_body(impact.at);
                let dealt = damage_to(&shoot(&mut game, 0, Shot::Ss, aim, power), 1);
                if target.gap(hitbox, impact.at) < 5.0 {
                    if at.y > hitbox.y / 3.0 {
                        low = low.max(dealt);
                    } else if at.y < hitbox.y * 2.0 / 3.0 {
                        high = high.max(dealt);
                    }
                }
            }
        }
        assert!(low > high && high > 0, "low {low}, high {high}");
    }

    #[test]
    fn ss_calls_one_bolt_with_thunder() {
        let mut game = duel(800);
        game.players[0].items = Loadout::new(&[Item::Thunder]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(2.0));
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count(), 1, "one projectile, one bolt: {effects:?}");
    }
}
