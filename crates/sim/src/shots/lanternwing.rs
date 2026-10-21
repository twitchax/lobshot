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
    weather::Weather,
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
            let (legs, strands, mut landings) = helix::fly(game, flight, &shape, |game, _, impact, forced, out| {
                let blast = gun.blast(impact.at, S2_STRAND_BLAST.value, S2_STRAND_DAMAGE.value, Weather::force_multiplier(forced));
                gun.land(game, &blast, S2_STRAND_BLAST.value, impact, out);
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
            gun.land(game, &gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value), SS_BLAST.value, &track.impact, effects);
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
