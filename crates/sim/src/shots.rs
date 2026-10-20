//! Each mobile's shots, as behaviors over the shared flight and explosion machinery in `game`.

use crate::{
    damage::Blast,
    game::{Contact, Effect, Game, MobileKind, Shot, Track},
    tuning::{DamageRange, coilback},
    units::{Power, TICKS_PER_SECOND},
    weather::Weather,
};

/// Fires `shot` from `seat`. The caller has checked it is the seat's turn and the aim is in range.
pub(crate) fn fire(game: &mut Game, seat: u8, shot: Shot, aim: i16, power: Power, effects: &mut Vec<Effect>) {
    let Some(player) = game.players().get(usize::from(seat)).copied() else { return };
    let Some(flight) = game.launch(seat, aim, power.bars()) else { return };
    let tuning = player.tuning();
    let false_angle = !shot.tuning(tuning).aim.value.is_true(aim);
    let blast = |radius: f64, damage: DamageRange, track: &Track, forced: bool| Blast {
        center: track.impact.at,
        radius,
        damage,
        weapon: tuning.weapon.value,
        false_angle,
        boost: Weather::force_multiplier(forced),
    };

    match player.mobile {
        MobileKind::Coilback => match shot {
            Shot::S1 => {
                let track = game.track(flight, 0, 0);
                effects.push(Effect::Fired { seat, legs: track.legs.clone() });
                detonate(game, &blast(coilback::S1_BLAST.value, coilback::S1_DAMAGE.value, &track, track.forced), &track, effects);
            }
            Shot::S2 => {
                // Two warheads: the first goes off on contact, and the second carries on through
                // the fresh crater (or the mobile it just hit) to the next contact. Their effects
                // wait until the path's end is known, so `Fired` comes first.
                let (radius, damage) = (coilback::S2_WARHEAD_BLAST.value, coilback::S2_WARHEAD_DAMAGE.value);
                let mut blasts = Vec::new();
                let first = game.track(flight, 0, 0);
                detonate(game, &blast(radius, damage, &first, first.forced), &first, &mut blasts);
                let mut legs = first.legs.clone();
                if first.impact.contact != Contact::Gone {
                    let second = game.track(first.last.flight, first.last_tick + 1, first.last.offset);
                    detonate(game, &blast(radius, damage, &second, first.forced || second.forced), &second, &mut blasts);
                    legs.extend(second.legs);
                }
                effects.push(Effect::Fired { seat, legs });
                effects.append(&mut blasts);
            }
            Shot::Ss => {
                let track = game.track(flight, 0, 0);
                effects.push(Effect::Fired { seat, legs: track.legs.clone() });
                // Seconds to ticks: a small whole-tick count, exact in f64.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let transform = (coilback::SS_TRANSFORM_SECONDS.value * f64::from(TICKS_PER_SECOND)) as u32;
                let shell = if track.impact.tick >= transform {
                    blast(coilback::SS_BLAST.value, coilback::SS_DAMAGE.value, &track, track.forced)
                } else {
                    blast(coilback::S1_BLAST.value, coilback::SS_EARLY_DAMAGE.value, &track, track.forced)
                };
                detonate(game, &shell, &track, effects);
            }
        },
    }
}

fn detonate(game: &mut Game, blast: &Blast, track: &Track, effects: &mut Vec<Effect>) {
    let direct = match track.impact.contact {
        Contact::Gone | Contact::Column(_) => return,
        Contact::Terrain => None,
        Contact::Mobile(seat) => Some(seat),
    };
    game.explode(blast, direct, track.impact.tick, effects);
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{Contact, Effect, Event, Game, Phase, Shot, testing::duel},
        units::{Centis, Facing, Power},
        weather::{Column, WeatherKind},
    };

    /// The power step whose shot strikes seat 1's hitbox closest to its center.
    fn direct_hit_power(game: &Game, aim: i16) -> Power {
        let target = game.players()[1];
        let center = target.pose().center(target.tuning().hitbox.value);
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, aim, power.bars())?, 0, 0).impact;
                (impact.contact == Contact::Mobile(1)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .expect("some power hits")
    }

    fn shoot(game: &mut Game, shot: Shot, aim: i16, power: Power) -> Vec<Effect> {
        game.apply(Event::Shoot {
            seat: 0,
            shot,
            aim,
            power,
            time: Centis(0),
        })
        .unwrap()
    }

    fn damage_to(effects: &[Effect], seat: u8) -> i32 {
        effects
            .iter()
            .map(|e| if let Effect::Damaged { seat: s, amount } = e { if *s == seat { *amount } else { 0 } } else { 0 })
            .sum()
    }

    fn explosions(effects: &[Effect]) -> usize {
        effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count()
    }

    fn flight_ticks(effects: &[Effect]) -> u32 {
        effects
            .iter()
            .find_map(|e| if let Effect::Fired { legs, .. } = e { legs.last().map(|leg| leg.offset + leg.to) } else { None })
            .unwrap()
    }

    #[test]
    fn s1_on_target_deals_its_listed_range_and_ends_the_turn() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 35);
        let effects = shoot(&mut game, Shot::S1, 35, power);
        assert!(matches!(effects[0], Effect::Fired { seat: 0, .. }), "{effects:?}");
        assert_eq!(explosions(&effects), 1);
        let dealt = damage_to(&effects, 1);
        assert!((120..=165).contains(&dealt), "{dealt} {effects:?}");
        assert_eq!(game.phase(), Phase::Turn { seat: 1 });
        assert_eq!(game.players()[0].clock.delay, 770);
    }

    #[test]
    fn s2_lands_two_warheads_on_a_direct_hit() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, Shot::S2, 30, power);
        assert_eq!(explosions(&effects), 2, "{effects:?}");
        assert!(matches!(effects[0], Effect::Fired { .. }), "Fired comes first: {effects:?}");
        assert!(damage_to(&effects, 1) > 115, "both warheads land");
    }

    #[test]
    fn s2_into_the_ground_digs_two_craters() {
        let mut game = duel(1200);
        // Well short of the target: both warheads land in open ground.
        let effects = shoot(&mut game, Shot::S2, 45, Power::nearest(1.5));
        assert_eq!(explosions(&effects), 2, "{effects:?}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn ss_hits_harder_once_it_has_expanded() {
        // A short shot lands well inside 2.5 s.
        let mut early = duel(400);
        let power = direct_hit_power(&early, 30);
        let early_effects = shoot(&mut early, Shot::Ss, 30, power);
        assert!(flight_ticks(&early_effects) < 5120);
        // With aim capped at 50 degrees, only a long shot stays up past 2.5 s.
        let mut late = duel(1700);
        let power = direct_hit_power(&late, 50);
        let late_effects = shoot(&mut late, Shot::Ss, 50, power);
        assert!(flight_ticks(&late_effects) >= 5120, "{late_effects:?}");
        let (early_damage, late_damage) = (damage_to(&early_effects, 1), damage_to(&late_effects, 1));
        assert!(late_damage >= 350 && early_damage <= 165, "{late_damage} vs {early_damage}");
    }

    #[test]
    fn force_boosts_a_shot_that_passes_through_it() {
        let mut calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let mut forced = calm.clone();
        let midway = forced.players()[0].footing.x + 400;
        forced.set_columns(vec![Column { kind: WeatherKind::Force, x: midway }]);
        let calm_damage = damage_to(&shoot(&mut calm, Shot::S1, 35, power), 1);
        let forced_effects = shoot(&mut forced, Shot::S1, 35, power);
        assert!(damage_to(&forced_effects, 1) > calm_damage, "{forced_effects:?}");
    }

    #[test]
    fn a_shot_off_the_side_of_the_map_does_nothing() {
        let mut game = duel(800);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let effects = shoot(&mut game, Shot::S1, 30, Power::MAX);
        assert_eq!(explosions(&effects), 0, "{effects:?}");
    }
}
