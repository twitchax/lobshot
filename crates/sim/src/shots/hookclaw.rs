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
            gun.land(game, &blast, blast.radius, &track.impact, effects);
        }
    }
}
