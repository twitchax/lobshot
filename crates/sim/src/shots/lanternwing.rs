//! Lanternwing (Mage): a laser bolt; two strands on a helix about the flight path, each its own
//! hit; and an SS ball that strips the shields of every mobile near where it lands.

use super::{
    Gun,
    helix::{self, Helix},
};
use crate::{
    game::{Contact, Effect, Game, Shot},
    tuning::lanternwing::{S1_BLAST, S1_DAMAGE, S2_AMPLITUDE, S2_HERTZ, S2_PHASES, S2_STRAND_BLAST, S2_STRAND_DAMAGE, SS_BLAST, SS_DAMAGE, SS_STRIP_REACH},
    units::Vec2,
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
            };
            let (legs, strands, mut landings) = helix::fly(game, flight, &shape, |game, _, impact, marks, out| {
                let blast = gun.blast(impact.at, S2_STRAND_BLAST.value, S2_STRAND_DAMAGE.value, marks.boost());
                gun.land(game, &blast, S2_STRAND_BLAST.value, impact, marks, out);
            });
            effects.push(Effect::Helix {
                seat: gun.seat,
                legs,
                amplitude: shape.amplitude,
                hertz: shape.hertz,
                strands,
            });
            effects.append(&mut landings);
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            if matches!(track.impact.contact, Contact::Terrain | Contact::Mobile(_)) {
                strip(game, track.impact.at, track.impact.tick, effects);
            }
            gun.land(game, &gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value), SS_BLAST.value, &track.impact, track.marks, effects);
        }
    }
}

/// Strips the shield from every living mobile within reach of `at`, so the blast that follows hits
/// HP.
fn strip(game: &mut Game, at: Vec2, tick: u32, effects: &mut Vec<Effect>) {
    for (seat, player) in (0_u8..).zip(game.players.iter_mut()) {
        if player.alive && player.shield > 0 && player.pose().gap(player.tuning().hitbox.value, at) <= SS_STRIP_REACH.value {
            effects.push(Effect::ShieldStripped { seat, amount: player.shield, tick });
            player.shield = 0;
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        ballistics,
        game::{
            Effect, MobileKind, Shot,
            testing::{damage_to, direct_hit_power, duel_of, explosions, shoot},
        },
        units::Power,
    };

    #[test]
    fn s1_on_target_deals_its_listed_range() {
        let mut game = duel_of(MobileKind::Lanternwing, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 35);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 35, power), 1);
        // Laser is neutral against Mechanical.
        assert!((100..=160).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_winds_two_strands_about_its_path_and_each_strikes() {
        let mut game = duel_of(MobileKind::Lanternwing, MobileKind::Coilback, 1200);
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.6));
        let Some(Effect::Helix { legs, amplitude, hertz, strands, .. }) = effects.iter().find(|e| matches!(e, Effect::Helix { .. })) else {
            panic!("no helix: {effects:?}")
        };
        assert_eq!((*amplitude, *hertz, strands.len()), (15.0, 1.0, 2));
        assert_eq!(explosions(&effects), 2, "two strands, two blasts: {effects:?}");
        // Each strand stays within the amplitude of the path, and they part.
        let leg = legs[0];
        let tick = leg.to / 2;
        let center = leg.flight.at(tick);
        let [a, b] = [strands[0].phase, strands[1].phase].map(|phase| ballistics::helix_point(&leg.flight, tick, leg.offset, *amplitude, *hertz, phase));
        assert!(a.distance(center) <= 15.0 + 1e-9 && b.distance(center) <= 15.0 + 1e-9);
        assert!(a.distance(b) > 1.0, "{a:?} {b:?}");
    }

    #[test]
    fn s2_on_target_can_land_both_strands() {
        let mut game = duel_of(MobileKind::Lanternwing, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 35);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S2, 35, power), 1);
        assert!(dealt > 140, "more than one strand's max: {dealt}");
    }

    #[test]
    fn ss_strips_shields_before_it_hits() {
        let mut game = duel_of(MobileKind::Lanternwing, MobileKind::Sparkhorn, 800);
        let power = direct_hit_power(&game, 35);
        let effects = shoot(&mut game, 0, Shot::Ss, 35, power);
        let stripped = effects.iter().position(|e| matches!(e, Effect::ShieldStripped { seat: 1, amount: 300, .. })).expect("stripped");
        let blast = effects.iter().position(|e| matches!(e, Effect::Exploded { .. })).unwrap();
        assert!(stripped < blast, "{effects:?}");
        assert!(!effects.iter().any(|e| matches!(e, Effect::Shielded { seat: 1, .. })), "the blast meets no shield");
        // Laser is weak against Shield: 350-500 x 0.96.
        let dealt = damage_to(&effects, 1);
        assert!((336..=480).contains(&dealt), "{dealt} {effects:?}");
    }
}
