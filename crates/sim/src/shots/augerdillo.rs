//! Augerdillo (Nak): a regular round; a drill that passes through mobiles, digs into the ground,
//! bores a mirror of its arc through it and goes off where it comes up; and an SS armadillo that
//! rolls through rock and goes off only in a mobile.

use super::{Gun, helix::cut};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Leg, Senses, Shot, Track},
    jewels, mines,
    tuning::{
        MAX_FLIGHT_SECONDS,
        augerdillo::{S1_BLAST, S1_DAMAGE, S2_BLAST, S2_BURROW_LIFT, S2_BURROW_SPEED, S2_DAMAGE, SS_BLAST, SS_DAMAGE},
    },
    units::{TICKS_PER_SECOND, Vec2},
    weather::Marks,
};

/// What the drill's flight stops at besides the map's edges: the ground. It passes through mobiles
/// until it has dug in (EXP, SAGE, DBR).
const GROUND: Senses = Senses { mobiles: false, terrain: true };

/// What the SS armadillo stops at besides the map's edges: a mobile, never the ground (SW-mob).
const THROUGH_GROUND: Senses = Senses { mobiles: true, terrain: false };

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
        }
        Shot::S2 => {
            let drilled = drill(game, flight, gun.tuning.gravity.value);
            effects.push(Effect::Fired { seat: gun.seat, legs: drilled.legs });
            let blast = gun.blast(drilled.impact.at, S2_BLAST.value, S2_DAMAGE.value, drilled.marks.boost());
            gun.land(game, &blast, S2_BLAST.value, &drilled.impact, drilled.marks, effects);
        }
        Shot::Ss => {
            // It goes off where it first touches a mobile, a direct hit worth most at the hitbox's
            // center: its weak spot, which only a shot from overlapping the target reaches.
            let track = game.track_sensing(flight, 0, 0, THROUGH_GROUND);
            // Or on a mine or a jewel it meets first: Nak's SS "explodes upon impact with a mobile,
            // a jewel, or a mini-Raon" (WP06). Its blast sets the mine off, or hits the jewel.
            let met = first_met(game, &track.legs).filter(|&(_, tick)| tick < track.impact.tick || track.impact.contact == Contact::Gone);
            if let Some((at, tick)) = met {
                effects.push(Effect::Fired {
                    seat: gun.seat,
                    legs: cut(track.legs, &[tick]),
                });
                let impact = Impact { tick, at, contact: Contact::Terrain };
                gun.land(
                    game,
                    &gun.blast(impact.at, SS_BLAST.value, SS_DAMAGE.value, track.marks.boost()),
                    SS_BLAST.value,
                    &impact,
                    track.marks,
                    effects,
                );
                return;
            }
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            gun.land(game, &gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value), SS_BLAST.value, &track.impact, track.marks, effects);
        }
    }
}

/// Where and when a path first meets a mine or a jewel, the mine first if both at once: the middle
/// of what it met, and the stage tick.
fn first_met(game: &Game, legs: &[Leg]) -> Option<(Vec2, u32)> {
    let mine = mines::crossed(game, legs).first().map(|&(mine, tick)| (mine.middle(), tick));
    let jewel = jewels::crossed(game, legs).first().map(|&(jewel, tick)| (jewel.middle(), tick));
    [mine, jewel].into_iter().flatten().min_by_key(|&(_, tick)| tick)
}

/// Where a shot `seat` fires along `flight` does its work, for SearchBot's aim pass: where the S2
/// drill comes up, and the mobile the SS rolls into through the ground.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Option<Impact> {
    let gravity = game.players().get(usize::from(seat))?.tuning().gravity.value;
    match shot {
        Shot::S1 => None,
        Shot::S2 => Some(drill(game, flight, gravity).impact),
        Shot::Ss => Some(game.track_sensing(flight, 0, 0, THROUGH_GROUND).impact),
    }
}

/// The S2's whole way.
struct Drilled {
    /// The drill's flight, then its bore if it dug in.
    legs: Vec<Leg>,
    /// Where it goes off: where it came up (in a mobile standing there, or on open ground); `Gone`
    /// if it was lost off the map.
    impact: Impact,
    /// What the columns the drill flew through did to it.
    marks: Marks,
}

/// The S2 drill along `flight`, on the game as it stands: it flies through mobiles until it strikes
/// the ground, and from there bores on (`bore`). `gravity` is its shooter's.
fn drill(game: &Game, flight: Flight, gravity: f64) -> Drilled {
    let path = game.track_sensing(flight, 0, 0, GROUND);
    let mut legs = path.legs.clone();
    let impact = if path.impact.contact == Contact::Terrain {
        let (leg, came_up) = bore(game, &path, gravity);
        legs.push(leg);
        came_up
    } else {
        path.impact
    };
    Drilled { legs, impact, marks: path.marks }
}

/// The bore on from where the drill struck the ground (`path`'s end): down into it at the angle and
/// speed it struck, the way it was heading, under `gravity` turned upward, until the first open
/// pixel it reaches. On level ground that is a mirror of its arc. Its leg, and where it came up: in
/// a mobile standing there, or on open ground; `Gone` if it left the map's sides or bottom first. A
/// floating mobile (`MobileKind::floats`) isn't there to come up in (`Game::grounded_frames`): its
/// blast only reaches one across the gap under it.
fn bore(game: &Game, path: &Track, gravity: f64) -> (Leg, Impact) {
    let struck = path.last.flight.velocity_at(path.last_tick);
    // Struck going up (the underside of a ledge), it still bores on down, as DBR's digger does.
    let digger = Flight {
        origin: path.impact.at,
        velocity: Vec2::new(struck.x, struck.y.abs()) * S2_BURROW_SPEED.value,
        accel: Vec2::new(0.0, -gravity * S2_BURROW_LIFT.value),
    };
    let terrain = game.terrain();
    let (width, height) = (f64::from(terrain.width()), f64::from(terrain.height()));
    let frames = game.grounded_frames();
    let guard = MAX_FLIGHT_SECONDS.value * TICKS_PER_SECOND;
    let mut came_up = Impact {
        tick: guard,
        at: digger.at(guard),
        contact: Contact::Gone,
    };
    for tick in 1..=guard {
        let at = digger.at(tick);
        if at.x < 0.0 || at.x >= width || at.y >= height {
            came_up = Impact { tick, at, contact: Contact::Gone };
            break;
        }
        // Inside the map: x and y are within i32.
        #[allow(clippy::cast_possible_truncation)]
        if !terrain.is_solid(at.x.floor() as i32, at.y.floor() as i32) {
            let contact = frames.iter().find(|(_, frame)| frame.contains(at)).map_or(Contact::Terrain, |&(seat, _)| Contact::Mobile(seat));
            came_up = Impact { tick, at, contact };
            break;
        }
    }
    let leg = Leg {
        flight: digger,
        from: 0,
        to: came_up.tick,
        offset: path.impact.tick,
    };
    (
        leg,
        Impact {
            tick: path.impact.tick + came_up.tick,
            ..came_up
        },
    )
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, MobileKind, Team,
            testing::{damage_to, duel_of, explosions, game_on, seat, shoot},
        },
        terrain::Terrain,
        tuning::augerdillo::MOBILE,
        units::{Facing, Power},
    };

    /// Augerdillo at x = 300, its back to a Coilback `distance` px to its right, on flat ground.
    fn duel(distance: i32) -> Game {
        let mut game = duel_of(MobileKind::Augerdillo, MobileKind::Coilback, distance);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        game
    }

    /// `duel`, on flat ground 500 px deep, where a bore comes up before the map's bottom.
    fn deep(distance: i32) -> Game {
        let terrain = Terrain::from_fn(2400, 1000, |_, y| y >= 500).unwrap();
        game_on(
            terrain,
            vec![
                seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left),
                seat(Team::B, MobileKind::Coilback, 300 + distance, Facing::Left),
            ],
        )
    }

    /// The power step whose `shot` from seat 0 at `aim` does its work closest to seat 1's center,
    /// among those that strike it.
    fn striking_power(game: &Game, shot: Shot, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let flight = game.launch(0, shot, aim, power.bars())?;
                let impact = aim_point(game, 0, shot, flight).unwrap_or_else(|| game.track(flight, 0, 0).impact);
                (impact.contact == Contact::Mobile(1)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .expect("some power strikes")
    }

    #[test]
    fn it_fires_over_its_back() {
        let mut game = duel(800);
        let flight = game.launch(0, Shot::S1, 150, 2.0).unwrap();
        assert!(flight.velocity.x > 0.0 && flight.velocity.y < 0.0, "facing left, it lobs to the right: {flight:?}");
        assert!(MOBILE.s1.aim.value.backward() && MOBILE.ss.aim.value.backward());
        let power = striking_power(&game, Shot::S1, 150);
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S1, 150, power), 1);
        // 100-150 x 0.96 (Physical against Mechanical).
        assert!((96..=144).contains(&dealt), "{dealt}");
    }

    #[test]
    fn s2_bores_a_mirror_of_its_arc_and_goes_off_where_it_comes_up() {
        let mut game = deep(1600);
        let effects = shoot(&mut game, 0, Shot::S2, 140, Power::nearest(1.6));
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("{effects:?}")
        };
        assert_eq!(legs.len(), 2, "a flight and a bore: {legs:?}");
        let (flown, bored) = (legs[0], legs[1]);
        let struck = flown.flight.at(flown.to);
        assert_eq!(bored.flight.origin, struck);
        assert_eq!(bored.offset, flown.offset + flown.to, "it bores on at once");
        // Through level ground it bores a mirror of its arc: as deep under where it struck as the
        // apex was above it, coming up as far on as the apex was before it.
        let apex = (0..flown.to).map(|tick| flown.flight.at(tick)).min_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
        let deepest = (0..=bored.to).map(|tick| bored.flight.at(tick)).max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
        assert!((deepest.y - struck.y - (struck.y - apex.y)).abs() < 1.0, "{deepest:?} {struck:?} {apex:?}");
        assert!((deepest.x - struck.x - (struck.x - apex.x)).abs() < 1.0, "{deepest:?} {struck:?} {apex:?}");
        let up = bored.flight.at(bored.to);
        assert!((up.y - 500.0).abs() < 1.0, "back at the surface: {up:?}");
        assert!((up.x - deepest.x - (deepest.x - struck.x)).abs() < 1.0, "{up:?}");
        let blasts: Vec<(Vec2, u32)> = effects
            .iter()
            .filter_map(|e| if let Effect::Exploded { at, tick, .. } = e { Some((*at, *tick)) } else { None })
            .collect();
        assert_eq!(blasts, [(up, bored.offset + bored.to)], "one blast, where it came up");
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (struck.x as i32, struck.y as i32);
        assert!(game.terrain().is_solid(x, y + 2), "no crater where it dug in");
    }

    #[test]
    fn s2_struck_faster_bores_farther() {
        let game = deep(1600);
        let reach = |power: f64| {
            let drilled = drill(&game, game.launch(0, Shot::S2, 140, power).unwrap(), MOBILE.gravity.value);
            drilled.legs[1].flight.at(drilled.legs[1].to).x - drilled.legs[1].flight.origin.x
        };
        assert!(reach(1.8) > reach(1.2) * 1.5, "{} {}", reach(1.8), reach(1.2));
    }

    #[test]
    fn s2_comes_up_under_a_target_on_the_far_side_of_a_hill() {
        // A hill between them, its top 200 px up: the drill digs into the near slope and comes up
        // past the crest, on the far side, under the target standing there.
        let terrain = Terrain::from_fn(2400, 1200, |x, y| {
            let hill = 200.0 - (f64::from(x) - 800.0).abs() * 0.5;
            f64::from(y) >= 900.0 - hill.max(0.0)
        })
        .unwrap();
        let game = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left), seat(Team::B, MobileKind::Coilback, 960, Facing::Left)],
        );
        let hits: Vec<(i16, Power)> = (135..=165)
            .flat_map(|aim| (1..=Power::MAX.twentieths()).map(move |twentieths| (aim, Power::from_twentieths(twentieths).unwrap())))
            .filter(|&(aim, power)| {
                let flight = game.launch(0, Shot::S2, aim, power.bars()).unwrap();
                let drilled = drill(&game, flight, MOBILE.gravity.value);
                let dug = drilled.legs.first().map(|leg| leg.flight.at(leg.to).x);
                drilled.legs.len() == 2 && dug.is_some_and(|x| x < 800.0) && drilled.impact.contact == Contact::Mobile(1)
            })
            .collect();
        let &(aim, power) = hits.first().expect("a drill dug into the near slope that comes up in the target");
        let effects = shoot(&mut game.clone(), 0, Shot::S2, aim, power);
        // 190-300 x 0.96 (Physical against Mechanical).
        assert!(damage_to(&effects, 1) >= 182, "{effects:?}");
    }

    #[test]
    fn s2_passes_through_a_mobile_in_flight_and_digs_in_beyond_it() {
        // At a power that would strike the target with any shell, the drill flies through it and
        // into the ground beyond.
        let game = deep(800);
        let power = striking_power(&game, Shot::S1, 150);
        let flight = game.launch(0, Shot::S2, 150, power.bars()).unwrap();
        assert_eq!(game.track(flight, 0, 0).impact.contact, Contact::Mobile(1));
        let drilled = drill(&game, flight, MOBILE.gravity.value);
        assert_eq!(drilled.legs.len(), 2, "it dug in: {:?}", drilled.legs);
        let frame = &game.hit_frames()[1].1;
        assert!((0..=drilled.legs[0].to).any(|tick| frame.contains(drilled.legs[0].flight.at(tick))), "through the target");
        assert!(drilled.legs[1].flight.origin.x > game.players()[1].center().x, "into the ground beyond it");
    }

    #[test]
    fn s2_dropped_on_a_target_close_by_comes_up_under_it() {
        // Players drop it on a target near them (WP06-list): through it, into the ground at its feet
        // and straight back up under it.
        let game = deep(150);
        let hits = (135..=165)
            .flat_map(|aim| (1..=Power::MAX.twentieths()).map(move |twentieths| (aim, Power::from_twentieths(twentieths).unwrap())))
            .filter(|&(aim, power)| aim_point(&game, 0, Shot::S2, game.launch(0, Shot::S2, aim, power.bars()).unwrap()).is_some_and(|impact| impact.contact == Contact::Mobile(1)))
            .count();
        assert!(hits > 0);
    }

    #[test]
    fn s2_only_grazes_a_floating_mobile_it_comes_up_under() {
        // Kitewyrm floats over the ground: the drill can't come up in it (SW-mob: Dragon "cannot be
        // directly hit by Nakmachine's Shot 2"), and its blast reaches it only across the gap under
        // it, "(almost) immune" (TIR). A Coilback standing there takes it full.
        let come_up_under = |mobile: MobileKind| {
            let terrain = Terrain::from_fn(2400, 1000, |_, y| y >= 500).unwrap();
            let game = game_on(terrain, vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left), seat(Team::B, mobile, 1100, Facing::Left)]);
            let foot = game.players()[1].footing.world();
            let (aim, power) = (135..=165)
                .flat_map(|aim| (1..=Power::MAX.twentieths()).map(move |twentieths| (aim, Power::from_twentieths(twentieths).unwrap())))
                .min_by(|a, b| {
                    let off = |(aim, power): (i16, Power)| drill(&game, game.launch(0, Shot::S2, aim, power.bars()).unwrap(), MOBILE.gravity.value).impact.at.distance(foot);
                    off(*a).total_cmp(&off(*b))
                })
                .unwrap();
            let impact = drill(&game, game.launch(0, Shot::S2, aim, power.bars()).unwrap(), MOBILE.gravity.value).impact;
            assert!(impact.at.distance(foot) < 3.0, "{mobile:?}: {impact:?}");
            (impact.contact, damage_to(&shoot(&mut game.clone(), 0, Shot::S2, aim, power), 1))
        };
        let (contact, standing) = come_up_under(MobileKind::Coilback);
        assert_eq!(contact, Contact::Mobile(1));
        let (contact, floating) = come_up_under(MobileKind::Kitewyrm);
        assert_eq!(contact, Contact::Terrain, "never in it");
        assert!(floating > 0 && floating * 3 < standing, "it grazes it: {floating} against {standing}");
    }

    #[test]
    fn s2_lost_off_the_map_does_nothing() {
        // Fired at the map's left edge, it flies out of the side.
        let mut game = duel_of(MobileKind::Augerdillo, MobileKind::Coilback, 800);
        let effects = shoot(&mut game, 0, Shot::S2, 140, Power::MAX);
        assert_eq!(explosions(&effects), 0, "{effects:?}");
        // A lob whose arc is deeper than the ground under it bores out of the map's bottom.
        let mut game = duel(1600);
        let effects = shoot(&mut game, 0, Shot::S2, 140, Power::nearest(1.6));
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("{effects:?}")
        };
        assert!(legs[1].flight.at(legs[1].to).y >= 600.0, "out of the bottom: {legs:?}");
        assert_eq!(explosions(&effects), 0, "{effects:?}");
    }

    #[test]
    fn s2_through_a_thin_floor_goes_off_under_it() {
        // A shelf 6 px thick over open air: the drill bores through it and goes off at its underside.
        let terrain = Terrain::from_fn(2400, 900, |x, y| (y >= 500 && x < 700) || ((700..1100).contains(&x) && (480..486).contains(&y)) || y >= 800).unwrap();
        let mut game = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left), seat(Team::B, MobileKind::Coilback, 1800, Facing::Left)],
        );
        let power = (1..=Power::MAX.twentieths())
            .map(|twentieths| Power::from_twentieths(twentieths).unwrap())
            .find(|power| {
                let at = game.track(game.launch(0, Shot::S2, 145, power.bars()).unwrap(), 0, 0).impact.at;
                (750.0..1050.0).contains(&at.x) && at.y < 486.0
            })
            .expect("a power that lands on the shelf");
        let drilled = drill(&game, game.launch(0, Shot::S2, 145, power.bars()).unwrap(), MOBILE.gravity.value);
        assert_eq!(drilled.impact.contact, Contact::Terrain);
        assert!((486.0..487.0).contains(&drilled.impact.at.y), "just under the shelf: {:?}", drilled.impact);
        assert_eq!(explosions(&shoot(&mut game, 0, Shot::S2, 145, power)), 1);
    }

    #[test]
    fn ss_rolls_through_a_hill_and_goes_off_only_in_a_mobile() {
        // A hill between them, its top 200 px up: the SS passes through it and strikes the target.
        let terrain = Terrain::from_fn(2400, 1200, |x, y| {
            let hill = 200.0 - (f64::from(x) - 700.0).abs() * 0.5;
            f64::from(y) >= 900.0 - hill.max(0.0)
        })
        .unwrap();
        let mut game = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        );
        let power = striking_power(&game, Shot::Ss, 160);
        let flight = game.launch(0, Shot::Ss, 160, power.bars()).unwrap();
        assert_eq!(game.track(flight, 0, 0).impact.contact, Contact::Terrain, "a shell would hit the hill");
        let before = game.terrain().clone();
        let effects = shoot(&mut game, 0, Shot::Ss, 160, power);
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("{effects:?}")
        };
        let crossed = legs.iter().any(|leg| {
            (leg.from..=leg.to).any(|tick| {
                let at = leg.flight.at(tick);
                #[allow(clippy::cast_possible_truncation)]
                let solid = before.is_solid(at.x as i32, at.y as i32);
                solid
            })
        });
        assert!(crossed, "it went through the hill");
        assert_eq!(explosions(&effects), 1, "{effects:?}");
        // 350-500 x 0.96 (Physical against Mechanical).
        assert!((336..=480).contains(&damage_to(&effects, 1)), "{effects:?}");
    }

    #[test]
    fn ss_goes_off_on_a_mine_it_meets_and_sets_it_off() {
        // WP06: Nak's SS "explodes upon impact with a mobile, a jewel, or a mini-Raon". A mine on the
        // ground where the SS goes into it, short of any mobile.
        let mut game = duel(800);
        let power = Power::nearest(0.8);
        let flight = game.launch(0, Shot::Ss, 150, power.bars()).unwrap();
        let track = game.track_sensing(flight, 0, 0, THROUGH_GROUND);
        let leg = track.legs[0];
        let into = (leg.from..=leg.to).map(|tick| leg.flight.at(tick)).find(|at| at.y >= 496.0).unwrap();
        let mut laid = Vec::new();
        mines::lay(&mut game, 1, Vec2::new(into.x, 499.5), Facing::Left, 0, &mut laid);
        let mine = game.mines()[0];
        let effects = shoot(&mut game, 0, Shot::Ss, 150, power);
        let Some(&Effect::Exploded { at, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!("{effects:?}")
        };
        assert_eq!(at, mine.middle(), "it went off on the mine");
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { mine: id, tick: Some(_) } if *id == mine.id)), "{effects:?}");
        assert_eq!(game.mines(), []);
    }

    #[test]
    fn ss_that_meets_no_mobile_does_nothing() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::Ss, 150, Power::nearest(0.8));
        assert_eq!(explosions(&effects), 0, "through the ground and out of the map's bottom: {effects:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::SsLocked { seat: 0, .. })));
    }

    #[test]
    fn ss_is_a_direct_hit_worth_most_overlapping_its_target() {
        // It goes off where it first touches a mobile: lobbed in from afar, the edge of its body;
        // fired from overlapping it, point blank, near its center, its weak spot (GF-max).
        let afar = deep(600);
        let power = striking_power(&afar, Shot::Ss, 150);
        let impact = aim_point(&afar, 0, Shot::Ss, afar.launch(0, Shot::Ss, 150, power.bars()).unwrap()).unwrap();
        let target = afar.players()[1];
        let hitbox = target.tuning().hitbox.value;
        assert!(
            target.pose().gap(hitbox, impact.at) < 1e-9 && afar.players()[1].center().distance(impact.at) > 15.0,
            "at its edge: {impact:?}"
        );
        let from_afar = damage_to(&shoot(&mut afar.clone(), 0, Shot::Ss, 150, power), 1);
        // A step behind it, 25 px down: the mortar's mouth at 170 is inside the target, at the height
        // of its center.
        let terrain = Terrain::from_fn(2400, 1000, |x, y| y >= if x < 322 { 525 } else { 500 }).unwrap();
        let mut close = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left), seat(Team::B, MobileKind::Coilback, 343, Facing::Left)],
        );
        let muzzle = close.launch(0, Shot::Ss, 170, 1.0).unwrap().origin;
        assert!(close.players()[1].center().distance(muzzle) < 5.0, "{muzzle:?}");
        let overlapping = damage_to(&shoot(&mut close, 0, Shot::Ss, 170, Power::nearest(1.0)), 1);
        // 350-500 x 0.96 (Physical against Mechanical).
        assert!(overlapping >= 460 && from_afar < overlapping - 50 && from_afar >= 336, "{overlapping} against {from_afar}");
    }

    #[test]
    fn ss_goes_off_on_a_teammate_too() {
        let mut game = game_on(
            Terrain::from_fn(2400, 1000, |_, y| y >= 500).unwrap(),
            vec![
                seat(Team::A, MobileKind::Augerdillo, 300, Facing::Left),
                seat(Team::B, MobileKind::Coilback, 1500, Facing::Left),
                seat(Team::A, MobileKind::Coilback, 800, Facing::Right),
            ],
        );
        let center = game.players()[2].center();
        let power = (1..=Power::MAX.twentieths())
            .map(|twentieths| Power::from_twentieths(twentieths).unwrap())
            .find(|power| aim_point(&game, 0, Shot::Ss, game.launch(0, Shot::Ss, 150, power.bars()).unwrap()).is_some_and(|impact| impact.contact == Contact::Mobile(2)))
            .expect("a power that rolls into the teammate");
        let effects = shoot(&mut game, 0, Shot::Ss, 150, power);
        assert!(damage_to(&effects, 2) > 0, "{center:?} {effects:?}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn s2_spares_its_shooter_and_ends_its_turn() {
        for aim in [130, 150, 170] {
            for power in [1.2, 2.4, 4.0] {
                let mut game = duel(800);
                let effects = shoot(&mut game, 0, Shot::S2, aim, Power::nearest(power));
                assert_eq!(damage_to(&effects, 0), 0, "{aim} at {power}: {effects:?}");
                assert!(effects.iter().any(|e| matches!(e, Effect::TurnStarted { seat: 1 })));
            }
        }
    }
}
