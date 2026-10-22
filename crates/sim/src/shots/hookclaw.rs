//! Hookclaw (Boomer): boomerangs. Their hook is the wind (`tuning::hookclaw`'s fitted
//! sensitivity), and one the wind turns around in flight, a boomer shot or a backshot, deals more
//! (GF-max). S1 is one, S2 four in a cluster, and the SS turns golden if it stays up long enough.

use super::{Gun, barrage};
use crate::{
    game::{Effect, Game, Shot},
    tuning::hookclaw::{HOOK_DAMAGE, S1_BLAST, S1_DAMAGE, S2_BOOMERANG_BLAST, S2_BOOMERANG_DAMAGE, S2_SPREAD, SS_BLAST, SS_DAMAGE, SS_EARLY_DAMAGE, SS_TRANSFORM_SECONDS},
    units::ticks,
};

pub(super) fn fire(game: &mut Game, gun: &mut Gun, shot: Shot, effects: &mut Vec<Effect>) {
    gun.hook = HOOK_DAMAGE.value;
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            let flights: Vec<_> = S2_SPREAD.value.iter().filter_map(|&(aim, bars)| gun.launch(game, aim, bars)).collect();
            let gun = &*gun;
            let (paths, mut landings) = barrage(game, &flights, |game, _, track, out| {
                gun.land(
                    game,
                    &gun.blast_on(track, S2_BOOMERANG_BLAST.value, S2_BOOMERANG_DAMAGE.value),
                    S2_BOOMERANG_BLAST.value,
                    &track.impact,
                    track.marks,
                    out,
                );
            });
            effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
            effects.append(&mut landings);
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
            Contact, Effect, Event, Leg, MobileKind, Shot,
            testing::{damage_to, direct_hit_power, duel_of, shoot},
        },
        units::{Facing, Power, Wind},
    };

    #[test]
    fn s1_on_target_deals_its_listed_range() {
        let mut game = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 50);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 50, power), 1);
        // Physical is weak against Mechanical: 120-160 x 0.96.
        assert!((115..=153).contains(&dealt), "{dealt}");
    }

    #[test]
    fn a_backshot_hooks_round_on_the_wind_and_hits_harder() {
        // Facing away from the target with a gale blowing toward it: the boomerang heads off the
        // wrong way, turns, and comes back.
        let mut game = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 500);
        game.wind = Wind::new(26, 0).unwrap();
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let target = game.players()[1].center();
        let (aim, power) = (60..=89)
            .flat_map(|aim| (20..=80).map(move |twentieths| (aim, Power::from_twentieths(twentieths).unwrap())))
            .filter_map(|(aim, power)| {
                let track = game.track(game.launch(0, Shot::S1, aim, power.bars())?, 0, 0);
                (track.impact.contact == Contact::Mobile(1)).then(|| (track.impact.at.distance(target), aim, power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, aim, power)| (aim, power))
            .expect("a backshot reaches");
        let effects = shoot(&mut game, 0, Shot::S1, aim, power);
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!()
        };
        assert!(legs.iter().any(Leg::hooked), "{legs:?}");
        // 120-160, x 0.96 for Physical against Mechanical, x 1.2 for the hook; false angle above 65.
        let dealt = damage_to(&effects, 1);
        let least = if aim > 65 { 96 } else { 138 };
        assert!((least..=184).contains(&dealt), "{dealt} at {aim}");
    }

    #[test]
    fn s2_throws_four_boomerangs_in_a_cluster() {
        let mut game = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 50);
        let effects = shoot(&mut game, 0, Shot::S2, 50, power);
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Fired { .. })).count(), 4);
        // At least two of the four land on the target: each is 57-72.
        assert!(damage_to(&effects, 1) > 110, "{effects:?}");
    }

    #[test]
    fn ss_turns_golden_only_after_long_enough_in_the_air() {
        let mut short = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 400);
        let power = direct_hit_power(&short, 40);
        let early = damage_to(&shoot(&mut short, 0, Shot::Ss, 40, power), 1);
        let mut long = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 800);
        let power = direct_hit_power(&long, 80);
        let late = damage_to(&shoot(&mut long, 0, Shot::Ss, 80, power), 1);
        // Golden: 350-900 x 0.96, at 70% outside the true angles.
        assert!(early <= 153 && late >= 235, "{early} then {late}");
    }
}
