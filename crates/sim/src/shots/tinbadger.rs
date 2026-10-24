//! Tinbadger (Raon): S1 throws three spinning cogs braided about one path, each its own hit, that
//! dig a lot of land; S2 lets two tin beetles loose, which fly through mobiles and land as mines
//! (`sim::mines`); the SS winds up one big beetle that lands, walks on the way it was flying
//! (turning round at walls it can't climb), and goes off on walking into a mobile or a mine, or
//! when its fuse burns down.

use super::{
    Gun,
    helix::{self, Helix},
    land_in_order,
    trundle::{self, Gait, Heading, Start, Walked},
};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Senses, Shot, Track},
    mines,
    movement::{self, Footing, Landing},
    tuning::tinbadger::{
        MINE_RADIUS, S1_AMPLITUDE, S1_COG_BLAST, S1_COG_CRATER, S1_COG_DAMAGE, S1_COGS, S1_HERTZ, S2_SHELLS, S2_SPREAD_BARS, SS_BLAST, SS_DAMAGE, SS_FUSE_SECONDS, SS_HEIGHT, SS_WALK_SPEED,
    },
    units::{Facing, Vec2, ticks},
};

/// What a beetle shell senses in flight: it flies through mobiles to the ground, where the mines
/// "have to hit ... first in order to explode" (GF-vgg, CR-blood, OB).
const THROUGH_MOBILES: Senses = Senses { mobiles: false, terrain: true };

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
/// mine on the ground it lands on, which goes off at once at a mobile's sweet spot ("unless they
/// land directly on their target (in which case they immediately explode)", GF-lee). A bolt or
/// beam the landing calls down sets it off too (CR-tooya: "Thor or lightning guarantee your shot
/// to explode on the enemy").
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
        |game, index| game.track_sensing(flights[index], 0, 0, THROUGH_MOBILES),
        |game, _, track, out| {
            if track.impact.contact == Contact::Terrain {
                mines::lay(game, gun.seat, open_before(track), facing, track.impact.tick, out);
                gun.follow_up(game, track.impact.at, track.impact.tick, track.marks, out);
            }
        },
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// Where a projectile that struck the ground at the end of `track` was a tick before: in the open.
fn open_before(track: &Track) -> Vec2 {
    track.last.flight.at(track.last_tick.saturating_sub(1).max(track.last.from))
}

/// The ground under a projectile that struck it at the end of `track`, if there is any.
fn ground_under(game: &Game, track: &Track) -> Option<Footing> {
    let open = open_before(track);
    // A landing is a pixel or so from the ground, inside the map: in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let open = Footing {
        x: open.x.floor() as i32,
        y: open.y.floor() as i32,
    };
    match movement::settle(game.terrain(), open) {
        Landing::On(footing) => Some(footing),
        Landing::OffMap => None,
    }
}

/// The way the big beetle walks from a landing at the end of `track`: the way it was flying, so a
/// wind that turned it round turns its walk round too (CR-tooya); straight down, the way its
/// launcher faces.
fn march_facing(track: &Track, facing: Facing) -> Facing {
    let across = track.last.flight.velocity_at(track.last_tick).x;
    if across > 0.0 {
        Facing::Right
    } else if across < 0.0 {
        Facing::Left
    } else {
        facing
    }
}

/// The big beetle's march from where its shell came down at the end of `track`: `None` if it
/// didn't come down on the ground. It stops at the first mobile it walks into (one that floats it
/// passes under, as it does a mine, `mines::senses`), or the first mine.
fn march(game: &Game, track: &Track, facing: Facing) -> Option<Walked> {
    if track.impact.contact != Contact::Terrain {
        return None;
    }
    let start = ground_under(game, track)?;
    let steps = ticks(SS_FUSE_SECONDS.value) / ticks(1.0 / MARCH.speed).max(1);
    // It meets a mine when their shells touch: a mine's size, and the beetle's half height.
    let mine_at = |at: Vec2| game.mines().iter().any(|mine| mine.middle().distance(at) <= MINE_RADIUS.value + SS_HEIGHT.value);
    let touch = |at: Vec2| {
        let mobile = (0_u8..)
            .zip(game.players())
            .find(|(_, player)| player.alive && mines::senses(player.mobile) && player.pose().hit(player.tuning().hitbox.value, at))
            .map(|(seat, _)| Contact::Mobile(seat));
        mobile.or_else(|| mine_at(at).then_some(Contact::Terrain))
    };
    let start = Start {
        at: start,
        facing: march_facing(track, facing),
        tick: track.impact.tick,
    };
    Some(trundle::walk(game.terrain(), start, &MARCH, Heading::Ahead, steps, touch))
}

/// Where a shot from `seat` on `flight` does its work, for SearchBot's aim pass: where a beetle shell
/// comes down, through any mobile in its way, on a mobile if its mine would go off there at once;
/// and where the big beetle's march ends. The cogs land where they fly.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Option<Impact> {
    match shot {
        Shot::S1 => None,
        Shot::S2 => {
            let track = game.track_sensing(flight, 0, 0, THROUGH_MOBILES);
            let on = (track.impact.contact == Contact::Terrain)
                .then(|| ground_under(game, &track))
                .flatten()
                .and_then(|footing| mines::sweet_spot(game, mines::Mine::middle_at(footing)));
            Some(on.map_or(track.impact, |seat| Impact {
                contact: Contact::Mobile(seat),
                ..track.impact
            }))
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            let facing = game.players().get(usize::from(seat))?.facing;
            Some(march(game, &track, facing).map_or(track.impact, |walked| walked.end))
        }
    }
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
    fn s2_shells_fly_through_mobiles_to_the_ground() {
        let mut game = duel(800);
        // Each shell, on its own power, would strike the target square.
        let power = direct_hit_power_of(&game, Shot::S2, 20);
        let strike = |off: f64| game.track(game.launch(0, Shot::S2, 20, power.bars() + off).unwrap(), 0, 0).impact.contact;
        assert!([-S2_SPREAD_BARS.value, S2_SPREAD_BARS.value].into_iter().any(|off| strike(off) == Contact::Mobile(1)));
        let effects = shoot(&mut game, 0, Shot::S2, 20, power);
        let laid: Vec<i32> = effects.iter().filter_map(|e| if let Effect::MineLaid { mine, .. } = e { Some(mine.at.x) } else { None }).collect();
        assert!(laid.iter().any(|&x| x > game.players()[1].footing.x), "through it, to the ground beyond: {laid:?}");
    }

    #[test]
    fn s2_landing_at_an_enemys_middle_goes_off_at_once() {
        // GF-lee: "unless they land directly on their target (in which case they immediately
        // explode)"; the aim pass knows which landings do.
        let mut game = duel(800);
        let (power, aim) = (0..=Power::MAX.twentieths())
            .flat_map(|t| (20..=50).map(move |aim| (Power::from_twentieths(t).unwrap(), aim)))
            .find(|&(power, aim)| {
                let near = game.launch(0, Shot::S2, aim, power.bars() - S2_SPREAD_BARS.value).unwrap();
                aim_point(&game, 0, Shot::S2, near).is_some_and(|point| point.contact == Contact::Mobile(1))
            })
            .expect("a shell that comes down at its middle");
        let effects = shoot(&mut game, 0, Shot::S2, aim, power);
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { tick: Some(_), .. })), "{effects:?}");
        assert!(damage_to(&effects, 1) >= 134, "{effects:?}");
    }

    #[test]
    fn a_shot_sets_off_a_mine_in_reach() {
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
        let mine_blasts = effects
            .iter()
            .filter(|e| matches!(e, Effect::Exploded { radius, .. } if *radius == crate::tuning::tinbadger::MINE_BLAST.value))
            .count();
        assert!(mine_blasts >= 1, "set off, not just cleared: {effects:?}");
    }

    #[test]
    fn ss_walks_the_way_it_flies_and_goes_off_on_the_mobile_it_walks_into() {
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
        assert!(!walking.is_empty() && walking.iter().all(|leg| leg.flight.velocity.x > 0.0), "right, as it flew: {legs:?}");
        let dealt = damage_to(&effects, 1);
        assert!(dealt >= SS_DAMAGE.value.min, "{dealt}: {effects:?}");
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
    fn ss_walks_the_way_the_wind_turned_it() {
        // Lobbed right off a cliff into a gale blowing left: it lands below, moving left, and walks
        // left (CR-tooya: "you may suffer from watching your little clone walk in the opposite
        // direction that you shot it"). In calm air the same shot walks right.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 550 || (x < 400 && y >= 300)).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Tinbadger, 370, Facing::Right), seat(Team::B, MobileKind::Coilback, 1500, Facing::Left)];
        let calm = game_on(terrain, seats);
        let mut windy = calm.clone();
        windy.wind = crate::units::Wind::new(26, 180).unwrap();
        let power = (4..=40)
            .map(|t| Power::from_twentieths(t).unwrap())
            .find(|power| {
                let track = windy.track(windy.launch(0, Shot::Ss, 50, power.bars()).unwrap(), 0, 0);
                track.impact.at.y >= 549.0 && track.impact.at.x >= 420.0 && track.last.flight.velocity_at(track.last_tick).x < 0.0
            })
            .expect("a lob the wind turns round");
        let walks = |game: &mut Game| {
            let effects = shoot(game, 0, Shot::Ss, 50, power);
            fired(&effects)[0]
                .iter()
                .filter(|leg| leg.flight.accel == Vec2::ZERO && leg.flight.velocity.x != 0.0)
                .map(|leg| leg.flight.velocity.x.signum())
                .collect::<Vec<f64>>()
        };
        let (windward, calmly) = (walks(&mut windy), walks(&mut calm.clone()));
        assert!(windward.first().is_some_and(|&x| x < 0.0), "{windward:?}");
        assert!(calmly.first().is_some_and(|&x| x > 0.0), "{calmly:?}");
    }

    #[test]
    fn ss_goes_off_on_a_mine_in_its_way_and_sets_it_off() {
        // CR-icy: "A target is not just a player, it can be another Raon mine".
        let mut game = duel(1600);
        let power = Power::nearest(1.6);
        let landing = game.track(game.launch(0, Shot::Ss, 45, power.bars()).unwrap(), 0, 0).impact.at;
        let mut laid = Vec::new();
        mines::lay(&mut game, 1, Vec2::new(landing.x + 60.0, 499.5), Facing::Left, 0, &mut laid);
        let mine = game.mines()[0];
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let Some(&Effect::Exploded { at, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!()
        };
        assert!((at.x - mine.middle().x).abs() <= MINE_RADIUS.value + SS_HEIGHT.value + 1.0, "where it met the mine: {at:?}");
        assert!(
            effects.contains(&Effect::MineGone {
                mine: mine.id,
                tick: Some(effects.iter().find_map(|e| if let Effect::Exploded { tick, .. } = e { Some(*tick) } else { None }).unwrap())
            }),
            "{effects:?}"
        );
        assert_eq!(game.mines(), []);
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
