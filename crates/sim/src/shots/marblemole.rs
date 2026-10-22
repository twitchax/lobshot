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
            let (paths, mut landings) = bounce::roll(game, &flights, &rules, |game, _, impact, marks, out| {
                let blast = gun.blast(impact.at, S2_MARBLE_BLAST.value, S2_MARBLE_DAMAGE.value, marks.boost());
                gun.land(game, &blast, S2_MARBLE_BLAST.value, impact, marks, out);
            });
            effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
            effects.append(&mut landings);
        }
        Shot::Ss => {
            let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: true });
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            pierce(game, gun, &track, effects);
            gun.land(
                game,
                &gun.blast_on(&track, SS_LAND_BLAST.value, SS_LAND_DAMAGE.value),
                SS_LAND_BLAST.value,
                &track.impact,
                track.marks,
                effects,
            );
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
    let boost = track.marks.boost() * gun.salvo.boost;
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

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{
            Effect, Event, MobileKind, Shot,
            testing::{damage_to, direct_hit_power, duel_of, explosions, shoot},
        },
        units::{Facing, Power, TICKS_PER_SECOND},
    };

    #[test]
    fn s1_is_the_weakest_shot() {
        let mut game = duel_of(MobileKind::Marblemole, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 45);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 45, power), 1);
        // 80-130 x 1.04 (Electric against Mechanical).
        assert!((83..=135).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_marbles_bounce_then_go_off_once_their_fuses_burn_down() {
        let mut game = duel_of(MobileKind::Marblemole, MobileKind::Coilback, 1500);
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.6));
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Fired { .. })).count(), 4);
        assert_eq!(explosions(&effects), 4, "{effects:?}");
        let bounces: Vec<u32> = effects.iter().filter_map(|e| if let Effect::Bounced { tick, .. } = e { Some(*tick) } else { None }).collect();
        let blasts: Vec<u32> = effects.iter().filter_map(|e| if let Effect::Exploded { tick, .. } = e { Some(*tick) } else { None }).collect();
        let fuse = 3 * TICKS_PER_SECOND;
        assert!(bounces.len() >= 4, "{effects:?}");
        assert!(blasts.iter().min().unwrap() >= &(bounces.iter().min().unwrap() + fuse), "nothing goes off before a fuse: {effects:?}");
        assert!(blasts.windows(2).all(|w| w[0] <= w[1]), "in tick order: {blasts:?}");
        assert_eq!(damage_to(&effects, 1), 0, "far from the target");
    }

    #[test]
    fn s2_marbles_bounce_off_the_side_of_the_map() {
        let mut game = duel_of(MobileKind::Marblemole, MobileKind::Coilback, 1500);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.6));
        assert!(effects.iter().any(|e| matches!(e, Effect::Bounced { at, .. } if at.x == 0.0)), "{effects:?}");
        assert_eq!(explosions(&effects), 4, "none fell out: {effects:?}");
    }

    #[test]
    fn s2_marbles_go_off_at_once_on_a_mobile() {
        let mut game = duel_of(MobileKind::Marblemole, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        assert!(damage_to(&effects, 1) > 0, "{effects:?}");
        let first = effects.iter().find_map(|e| if let Effect::Exploded { tick, .. } = e { Some(*tick) } else { None }).unwrap();
        assert!(first < 3 * TICKS_PER_SECOND, "the first goes off on touching it, not on a fuse: {first}");
    }

    #[test]
    fn ss_passes_through_mobiles_hurting_them_all_the_while() {
        let mut game = duel_of(MobileKind::Marblemole, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let Some(&Effect::Pierced { seat: 1, from, to }) = effects.iter().find(|e| matches!(e, Effect::Pierced { .. })) else {
            panic!("no touch: {effects:?}")
        };
        assert!(from < to);
        let pierced = effects.iter().position(|e| matches!(e, Effect::Pierced { .. })).unwrap();
        let landed = effects.iter().position(|e| matches!(e, Effect::Exploded { .. })).unwrap();
        assert!(pierced < landed, "it passes through, then lands: {effects:?}");
        // At least 400 x 1.04 from the touch, plus the blast where it lands under the target.
        assert!(damage_to(&effects, 1) >= 416, "{effects:?}");
    }
}
