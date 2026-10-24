//! Tinbadger (Raon): S1 throws three spinning cogs braided about one path, each its own hit, that
//! dig a lot of land; S2 lets two tin beetles loose, which land as mines (`sim::mines`), or go off
//! at once on a mobile they land on; the SS winds up one big beetle that lands, marches the way
//! Tinbadger faces (turning round at walls it can't climb), and goes off on touching a mobile or
//! when its fuse burns down.

use super::{
    Gun,
    helix::{self, Helix},
    land_in_order,
    trundle::{self, Gait, Heading, Start, Walked},
};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Shot, Track},
    mines,
    tuning::tinbadger::{
        MINE_BLAST, MINE_DAMAGE, S1_AMPLITUDE, S1_COG_BLAST, S1_COG_CRATER, S1_COG_DAMAGE, S1_COGS, S1_HERTZ, S2_SHELLS, S2_SPREAD_BARS, SS_BLAST, SS_DAMAGE, SS_FUSE_SECONDS, SS_HEIGHT, SS_WALK_SPEED,
    },
    units::{Facing, Vec2, ticks},
};

/// How the big beetle walks.
const MARCH: Gait = Gait {
    speed: SS_WALK_SPEED.value,
    climb: crate::tuning::CLIMB_PX.value,
    height: SS_HEIGHT.value,
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    let facing = game.players()[usize::from(gun.seat)].facing;
    match shot {
        Shot::S1 => cogs(game, gun, flight, effects),
        Shot::S2 => beetles(game, gun, facing, effects),
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            let Some(walked) = march(game, &track, facing) else {
                // Off the map, nothing; on a mobile, it goes off there.
                effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
                gun.land(game, &gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value), SS_BLAST.value, &track.impact, track.marks, effects);
                return;
            };
            effects.push(Effect::Fired {
                seat: gun.seat,
                legs: track.legs.into_iter().chain(walked.legs).collect(),
            });
            let blast = gun.blast(walked.end.at, SS_BLAST.value, SS_DAMAGE.value, track.marks.boost());
            gun.land(game, &blast, SS_BLAST.value, &walked.end, track.marks, effects);
        }
    }
}

/// S1: the cogs, evenly spaced about the path.
fn cogs(game: &mut Game, gun: &Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let count = S1_COGS.value;
    // Three cogs: their count is tiny, exact in f64.
    #[allow(clippy::cast_precision_loss)]
    let phases: Vec<f64> = (0..count).map(|i| core::f64::consts::TAU * i as f64 / count as f64).collect();
    let shape = Helix {
        amplitude: S1_AMPLITUDE.value,
        hertz: S1_HERTZ.value,
        phases: &phases,
        merge: None,
    };
    let (legs, strands, mut landings) = helix::fly(game, flight, &shape, |game, _, impact, marks, out| {
        let blast = gun.blast(impact.at, S1_COG_BLAST.value, S1_COG_DAMAGE.value, marks.boost());
        gun.land(game, &blast, S1_COG_CRATER.value, impact, marks, out);
    });
    effects.push(Effect::Helix {
        seat: gun.seat,
        legs,
        amplitude: shape.amplitude,
        hertz: shape.hertz,
        strands,
        merge: shape.merge,
    });
    effects.append(&mut landings);
}

/// S2: the shells, spread in power either side of the shot's, in the order they land: each lays a
/// mine on the ground it lands on, or goes off at once on a mobile ("unless they land directly on
/// their target (in which case they immediately explode)", GF-lee).
fn beetles(game: &mut Game, gun: &Gun, facing: Facing, effects: &mut Vec<Effect>) {
    let count = S2_SHELLS.value;
    let flights: Vec<Flight> = (0..count)
        .filter_map(|i| {
            // Two shells: -1 and +1 times the spread.
            #[allow(clippy::cast_precision_loss)]
            let offset = (2.0 * i as f64 - (count - 1) as f64) * S2_SPREAD_BARS.value;
            gun.launch(game, 0, offset)
        })
        .collect();
    let (paths, mut landings) = land_in_order(
        game,
        flights.len(),
        |game, index| game.track(flights[index], 0, 0),
        |game, _, track, out| match track.impact.contact {
            Contact::Terrain => {
                mines::lay(game, gun.seat, open_before(track), facing, track.impact.tick, out);
                gun.follow_up(game, track.impact.at, track.impact.tick, track.marks, out);
            }
            Contact::Mobile(_) => gun.land(game, &gun.blast_on(track, MINE_BLAST.value, MINE_DAMAGE.value), MINE_BLAST.value, &track.impact, track.marks, out),
            Contact::Gone | Contact::Column(_) => {}
        },
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// Where a projectile that struck the ground at the end of `track` was a tick before: in the open.
fn open_before(track: &Track) -> Vec2 {
    track.last.flight.at(track.last_tick.saturating_sub(1).max(track.last.from))
}

/// The big beetle's march from where its shell came down at the end of `track`, the way its
/// launcher faces: `None` if it didn't come down on the ground.
fn march(game: &Game, track: &Track, facing: Facing) -> Option<Walked> {
    if track.impact.contact != Contact::Terrain {
        return None;
    }
    let open = open_before(track);
    // A landing is a pixel or so from the ground, inside the map: in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let start = crate::movement::Footing {
        x: open.x.floor() as i32,
        y: open.y.floor() as i32,
    };
    let crate::movement::Landing::On(start) = crate::movement::settle(game.terrain(), start) else {
        return None;
    };
    let steps = ticks(SS_FUSE_SECONDS.value) / ticks(1.0 / MARCH.speed).max(1);
    let touch = |at: Vec2| {
        (0_u8..)
            .zip(game.players())
            .find(|(_, player)| player.alive && mines::senses(player.mobile) && player.pose().hit(player.tuning().hitbox.value, at))
            .map(|(seat, _)| seat)
    };
    let start = Start {
        at: start,
        facing,
        tick: track.impact.tick,
    };
    Some(trundle::walk(game.terrain(), start, &MARCH, Heading::Ahead, steps, touch))
}

/// Where a shot from `seat` on `flight` does its work, for SearchBot's aim pass: where the big
/// beetle's march ends. The cogs and the beetle shells land where they fly.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Option<Impact> {
    if shot != Shot::Ss {
        return None;
    }
    let track = game.track(flight, 0, 0);
    let facing = game.players().get(usize::from(seat))?.facing;
    Some(march(game, &track, facing).map_or(track.impact, |walked| walked.end))
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Leg, MobileKind, Team,
            testing::{damage_to, direct_hit_power, direct_hit_power_of, duel_of, game_on, seat, shoot},
        },
        terrain::Terrain,
        tuning::tinbadger::S1_COG_CRATER,
        units::{Power, TICKS_PER_SECOND},
    };

    /// A Tinbadger at x = 300 facing right, and a Coilback `distance` px away, on flat ground.
    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Tinbadger, MobileKind::Coilback, distance)
    }

    fn fired(effects: &[Effect]) -> Vec<&[Leg]> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.as_slice()) } else { None }).collect()
    }

    #[test]
    fn s1_braids_three_cogs_about_one_path_that_dig_wide() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        let Some(Effect::Helix { strands, amplitude, .. }) = effects.iter().find(|e| matches!(e, Effect::Helix { .. })) else {
            panic!("{effects:?}")
        };
        assert_eq!((strands.len(), *amplitude), (3, 12.0));
        let craters: Vec<f64> = effects.iter().filter_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None }).collect();
        assert!(!craters.is_empty() && craters.iter().all(|&r| r == S1_COG_CRATER.value), "{craters:?}");
        // All three on target: up to 160, Explosion against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((60..=160).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn s2_lays_two_mines_apart_that_stay_after_the_turn() {
        let mut game = duel(1600);
        let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.6));
        assert_eq!(fired(&effects).len(), 2, "{effects:?}");
        let laid: Vec<i32> = effects.iter().filter_map(|e| if let Effect::MineLaid { mine, .. } = e { Some(mine.at.x) } else { None }).collect();
        assert_eq!(laid.len(), 2, "{effects:?}");
        assert!((laid[0] - laid[1]).abs() > 20, "apart: {laid:?}");
        assert_eq!(damage_to(&effects, 1), 0, "{effects:?}");
        assert_eq!(game.mines().len(), 2, "on the map after the turn");
        assert!(game.mines().iter().all(|mine| mine.seat == 0 && mine.facing == Facing::Right));
    }

    #[test]
    fn s2_landing_directly_on_an_enemy_goes_off_at_once() {
        // GF-lee: "unless they land directly on their target (in which case they immediately
        // explode)". Both shells, a little apart in power, strike a target this close.
        let mut game = duel(500);
        let power = direct_hit_power_of(&game, Shot::S2, 20);
        let both = [-S2_SPREAD_BARS.value, S2_SPREAD_BARS.value]
            .iter()
            .all(|&off| game.track(game.launch(0, Shot::S2, 20, power.bars() + off).unwrap(), 0, 0).impact.contact == Contact::Mobile(1));
        let effects = shoot(&mut game, 0, Shot::S2, 20, power);
        let dealt = damage_to(&effects, 1);
        let least = if both { 2 * MINE_DAMAGE.value.min } else { MINE_DAMAGE.value.min } * 96 / 100;
        assert!((least..=2 * MINE_DAMAGE.value.max).contains(&dealt), "{dealt}: {effects:?}");
        assert!(!effects.iter().any(|e| matches!(e, Effect::MineLaid { .. })) || !both, "{effects:?}");
    }

    #[test]
    fn a_blast_smashes_a_mine_without_setting_it_off() {
        let mut game = duel(1600);
        shoot(&mut game, 0, Shot::S2, 45, Power::nearest(1.6));
        let mine = game.mines()[0];
        crate::game::testing::pass_until(&mut game, 0);
        // Seat 0 lobs an S1 onto it.
        let power = (0..=Power::MAX.twentieths())
            .map(|t| Power::from_twentieths(t).unwrap())
            .min_by(|a, b| {
                let off = |power: &Power| game.track(game.launch(0, Shot::S1, 45, power.bars()).unwrap(), 0, 0).impact.at.distance(mine.middle());
                off(a).total_cmp(&off(b))
            })
            .unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { mine: id, tick: Some(_) } if *id == mine.id)), "{effects:?}");
        assert!(!game.mines().iter().any(|m| m.id == mine.id));
        let mine_blasts = effects.iter().filter(|e| matches!(e, Effect::Exploded { radius, .. } if *radius == MINE_BLAST.value)).count();
        assert_eq!(mine_blasts, 0, "smashed, not set off: {effects:?}");
    }

    #[test]
    fn ss_marches_the_way_it_faces_and_goes_off_on_the_mobile_it_walks_into() {
        let mut game = duel(800);
        let power = (0..=Power::MAX.twentieths())
            .map(|t| Power::from_twentieths(t).unwrap())
            .find(|power| {
                let track = game.track(game.launch(0, Shot::Ss, 45, power.bars()).unwrap(), 0, 0);
                (900.0..1000.0).contains(&track.impact.at.x) && track.impact.contact == Contact::Terrain
            })
            .unwrap();
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let legs = fired(&effects)[0];
        let walking: Vec<&Leg> = legs.iter().filter(|leg| leg.flight.accel == Vec2::ZERO && leg.flight.velocity.x != 0.0).collect();
        assert!(!walking.is_empty() && walking.iter().all(|leg| leg.flight.velocity.x > 0.0), "right, as it faces: {legs:?}");
        let dealt = damage_to(&effects, 1);
        assert!(dealt >= 300, "{dealt}: {effects:?}");
    }

    #[test]
    fn ss_turns_round_at_a_wall_and_goes_off_when_its_fuse_burns_down() {
        // A tall wall 60 px past where it lands, and no one within reach: it walks to the wall,
        // turns, and walks back until its fuse is out.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || (x >= 1000 && y >= 400)).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Tinbadger, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1500, Facing::Left)];
        let mut game = game_on(terrain, seats);
        let power = (0..=Power::MAX.twentieths())
            .map(|t| Power::from_twentieths(t).unwrap())
            .find(|power| {
                let track = game.track(game.launch(0, Shot::Ss, 45, power.bars()).unwrap(), 0, 0);
                (900.0..960.0).contains(&track.impact.at.x) && track.impact.at.y > 495.0
            })
            .unwrap();
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let legs = fired(&effects)[0];
        assert!(legs.iter().any(|leg| leg.flight.velocity.x < 0.0 && leg.flight.accel == Vec2::ZERO), "back from the wall: {legs:?}");
        let landed = legs.iter().find(|leg| leg.flight.accel == Vec2::ZERO).unwrap().offset;
        let Some(&Effect::Exploded { tick, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!()
        };
        let fuse = f64::from(tick - landed) / f64::from(TICKS_PER_SECOND);
        assert!((5.0..=7.0).contains(&fuse), "SW-mob's 5 to 7 s: {fuse}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn the_aim_pass_sees_where_the_big_beetle_goes_off() {
        let game = duel(800);
        let flight = game.launch(0, Shot::Ss, 45, 1.6).unwrap();
        let track = game.track(flight, 0, 0);
        let point = aim_point(&game, 0, Shot::Ss, flight).unwrap();
        assert!(point.tick > track.impact.tick && point.at.x > track.impact.at.x, "{point:?} after {:?}", track.impact);
        assert_eq!(aim_point(&game, 0, Shot::S1, flight), None, "S1 lands where it flies");
    }
}
