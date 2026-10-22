//! Coilback (Armor): one heavy round; two warheads, the second carrying on through the first's
//! crater; and an SS shell that expands in flight and then hits much harder.

use super::Gun;
use crate::{
    game::{Contact, Effect, Game, Shot},
    tuning::coilback::{S1_BLAST, S1_DAMAGE, S2_WARHEAD_BLAST, S2_WARHEAD_DAMAGE, SS_BLAST, SS_DAMAGE, SS_EARLY_DAMAGE, SS_TRANSFORM_SECONDS},
    units::ticks,
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            // Two warheads: the first goes off on contact, and the second carries on through the
            // fresh crater (or the mobile it just hit) to the next contact. Their effects wait
            // until the path's end is known, so `Fired` comes first.
            let (radius, damage) = (S2_WARHEAD_BLAST.value, S2_WARHEAD_DAMAGE.value);
            let mut blasts = Vec::new();
            let first = game.track(flight, 0, 0);
            gun.land(game, &gun.blast_on(&first, radius, damage), radius, &first.impact, first.marks, &mut blasts);
            let mut legs = first.legs.clone();
            if first.impact.contact != Contact::Gone {
                let second = game.track(first.last.flight, first.last_tick + 1, first.last.offset);
                let marks = first.marks | second.marks;
                let blast = gun.blast(second.impact.at, radius, damage, marks.boost());
                gun.land(game, &blast, radius, &second.impact, marks, &mut blasts);
                legs.extend(second.legs);
            }
            effects.push(Effect::Fired { seat: gun.seat, legs });
            effects.append(&mut blasts);
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            let blast = if track.impact.tick >= ticks(SS_TRANSFORM_SECONDS.value) {
                gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value)
            } else {
                gun.blast_on(&track, S1_BLAST.value, SS_EARLY_DAMAGE.value)
            };
            gun.land(game, &blast, blast.radius, &track.impact, track.marks, effects);
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{
            Effect, Phase, Shot,
            testing::{damage_to, direct_hit_power, duel, explosions, shoot},
        },
        units::Power,
    };

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
        let effects = shoot(&mut game, 0, Shot::S1, 35, power);
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
        let effects = shoot(&mut game, 0, Shot::S2, 30, power);
        assert_eq!(explosions(&effects), 2, "{effects:?}");
        assert!(matches!(effects[1], Effect::Fired { .. }), "Fired comes before the blasts: {effects:?}");
        assert!(damage_to(&effects, 1) > 115, "both warheads land");
    }

    #[test]
    fn s2_into_the_ground_digs_two_craters() {
        let mut game = duel(1200);
        // Well short of the target: both warheads land in open ground.
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.5));
        assert_eq!(explosions(&effects), 2, "{effects:?}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn ss_hits_harder_once_it_has_expanded() {
        // A short shot lands well inside 2.5 s.
        let mut early = duel(400);
        let power = direct_hit_power(&early, 30);
        let early_effects = shoot(&mut early, 0, Shot::Ss, 30, power);
        assert!(flight_ticks(&early_effects) < 5120);
        // With aim capped at 50 degrees, only a long shot stays up past 2.5 s.
        let mut late = duel(1700);
        let power = direct_hit_power(&late, 50);
        let late_effects = shoot(&mut late, 0, Shot::Ss, 50, power);
        assert!(flight_ticks(&late_effects) >= 5120, "{late_effects:?}");
        let (early_damage, late_damage) = (damage_to(&early_effects, 1), damage_to(&late_effects, 1));
        assert!(late_damage >= 350 && early_damage <= 165, "{late_damage} vs {early_damage}");
    }
}
