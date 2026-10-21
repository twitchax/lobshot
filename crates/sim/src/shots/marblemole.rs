//! Marblemole (Grub): the weakest shot in the game; four marbles that bounce, roll and go off
//! (`bounce`); and an SS ball that passes through mobiles, hurting each the whole time it touches
//! it, then blows up on the ground.

use super::{
    Gun,
    bounce::{self, Bounce},
};
use crate::{
    damage,
    game::{Effect, Game, Senses, Shot, Track},
    tuning::marblemole::{
        S1_BLAST, S1_DAMAGE, S2_FRICTION, S2_FUSE_SECONDS, S2_MARBLE_BLAST, S2_MARBLE_DAMAGE, S2_MAX_BOUNCES, S2_REST_SPEED, S2_RESTITUTION, S2_SPREAD, SS_LAND_BLAST, SS_LAND_DAMAGE, SS_TOUCH_DAMAGE,
        SS_TOUCH_PER_SECOND,
    },
    units::{seconds, ticks},
    weather::Weather,
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            let flights: Vec<_> = S2_SPREAD.value.iter().filter_map(|&bars| gun.launch(game, 0, bars)).collect();
            let rules = Bounce {
                restitution: S2_RESTITUTION.value,
                friction: S2_FRICTION.value,
                rest_speed: S2_REST_SPEED.value,
                fuse: ticks(S2_FUSE_SECONDS.value),
                max_bounces: S2_MAX_BOUNCES.value,
            };
            let (paths, mut landings) = bounce::roll(game, &flights, &rules, |game, _, impact, forced, out| {
                let blast = gun.blast(impact.at, S2_MARBLE_BLAST.value, S2_MARBLE_DAMAGE.value, Weather::force_multiplier(forced));
                gun.land(game, &blast, S2_MARBLE_BLAST.value, impact, out);
            });
            effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
            effects.append(&mut landings);
        }
        Shot::Ss => {
            let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: true });
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            pierce(game, gun, &track, effects);
            gun.land(game, &gun.blast_on(&track, SS_LAND_BLAST.value, SS_LAND_DAMAGE.value), SS_LAND_BLAST.value, &track.impact, effects);
        }
    }
}

/// One mobile the SS passed through: its first and last ticks inside, and how many in all.
struct Touch {
    seat: u8,
    from: u32,
    to: u32,
    ticks: u32,
}

/// Hurts each mobile the SS ball passed through, by how long it was inside: at least the touch
/// damage's min, plus `SS_TOUCH_PER_SECOND`, up to its max. Mobiles are where they stood at launch,
/// and each takes its damage as the ball leaves it.
fn pierce(game: &mut Game, gun: &Gun, track: &Track, effects: &mut Vec<Effect>) {
    let frames = game.hit_frames();
    let mut touches: Vec<Touch> = Vec::new();
    for leg in &track.legs {
        for tick in leg.from..=leg.to {
            let at = leg.flight.at(tick);
            for &(seat, ref frame) in &frames {
                if !frame.contains(at) {
                    continue;
                }
                let now = leg.offset + tick;
                match touches.iter_mut().find(|touch| touch.seat == seat) {
                    Some(touch) => {
                        touch.to = now;
                        touch.ticks += 1;
                    }
                    None => touches.push(Touch { seat, from: now, to: now, ticks: 1 }),
                }
            }
        }
    }
    touches.sort_by_key(|touch| (touch.to, touch.seat));
    let range = SS_TOUCH_DAMAGE.value;
    let boost = Weather::force_multiplier(track.forced) * gun.salvo.boost;
    for touch in touches {
        let base = (f64::from(range.min) + SS_TOUCH_PER_SECOND.value * seconds(touch.ticks)).min(f64::from(range.max));
        let player = game.players()[usize::from(touch.seat)];
        effects.push(Effect::Pierced {
            seat: touch.seat,
            from: touch.from,
            to: touch.to,
        });
        let amount = damage::scale(base, gun.tuning.weapon.value, player.tuning().armor.value, gun.false_angle, boost);
        game.hurt(touch.seat, amount, effects);
    }
}
