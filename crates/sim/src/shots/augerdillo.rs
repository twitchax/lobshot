//! Augerdillo (Nak): a regular round; a drill that strikes the ground, bores a mirror of its arc
//! through it and goes off where it comes up; and an SS armadillo that rolls through rock and goes
//! off only on a mobile.

use super::Gun;
use crate::{
    ballistics::Flight,
    body::HitFrame,
    damage,
    game::{Contact, Effect, Game, Impact, Leg, MobileKind, Senses, Shot, Track},
    tuning::{
        MAX_FLIGHT_SECONDS,
        augerdillo::{S1_BLAST, S1_DAMAGE, S2_BLAST, S2_BURROW_LIFT, S2_BURROW_SPEED, S2_DAMAGE, SS_BLAST, SS_DAMAGE},
    },
    units::{TICKS_PER_SECOND, Vec2},
    weather::Marks,
};

/// What the drill's flight stops at besides the map's edges: the ground. It looks for mobiles
/// itself (`first_strike`), so it can pass over those it can't strike.
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
            let track = game.track_sensing(flight, 0, 0, THROUGH_GROUND);
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            go_off(game, gun, &track, effects);
        }
    }
}

/// The SS armadillo goes off in the mobile it rolled into: its blast burns low (`damage::low`),
/// worth most at the target's feet, its weak spot. One that left the map does nothing.
fn go_off(game: &mut Game, gun: &Gun, track: &Track, effects: &mut Vec<Effect>) {
    if !matches!(track.impact.contact, Contact::Mobile(_)) {
        return;
    }
    let blast = gun.blast_on(track, SS_BLAST.value, SS_DAMAGE.value);
    game.explode_by(&blast, SS_BLAST.value * gun.salvo.crater, track.impact.tick, effects, |_, target| damage::low(&blast, target));
    gun.follow_up(game, blast.center, track.impact.tick, track.marks, effects);
}

/// Where a shot `seat` fires along `flight` does its work, for SearchBot's aim pass: where the S2
/// drill comes up (or what it strikes), and the mobile the SS rolls into through the ground.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Option<Impact> {
    let gravity = game.players().get(usize::from(seat))?.tuning().gravity.value;
    match shot {
        Shot::S1 => None,
        Shot::S2 => Some(drill(game, flight, gravity).impact),
        Shot::Ss => Some(game.track_sensing(flight, 0, 0, THROUGH_GROUND).impact),
    }
}

/// Whether the S2 drill can strike `mobile` directly, in flight or coming up under it: every
/// mobile but a floating one, which it passes (SW-mob: Dragon "cannot be directly hit by
/// Nakmachine's Shot 2"). Its blast still reaches one.
fn drill_strikes(mobile: MobileKind) -> bool {
    match mobile {
        MobileKind::Coilback
        | MobileKind::Lanternwing
        | MobileKind::Hookclaw
        | MobileKind::Sparkhorn
        | MobileKind::Marblemole
        | MobileKind::Spoutpot
        | MobileKind::Tuskrack
        | MobileKind::Sudsotter
        | MobileKind::Frostfin
        | MobileKind::Herdhound
        | MobileKind::Thistleback
        | MobileKind::Augerdillo => true,
        MobileKind::Kitewyrm => false,
    }
}

/// The S2's whole way.
struct Drilled {
    /// The drill's flight, then the digger's bore if it dug in.
    legs: Vec<Leg>,
    /// Where it goes off: in a mobile it struck, or where it came up (in a mobile standing there,
    /// or on open ground); `Gone` if it was lost off the map.
    impact: Impact,
    /// What the columns the drill flew through did to it.
    marks: Marks,
}

/// The S2 drill along `flight`, on the game as it stands: it flies until it strikes a mobile (one
/// `drill_strikes`) or the ground, and from the ground bores on (`bore`). `gravity` is its
/// shooter's.
fn drill(game: &Game, flight: Flight, gravity: f64) -> Drilled {
    let path = game.track_sensing(flight, 0, 0, GROUND);
    if let Some((seat, tick)) = first_strike(game, &path) {
        // Flown again only so far, so the columns past the strike don't mark it.
        let track = game.track_until(flight, 0, 0, GROUND, tick).track;
        let impact = Impact {
            contact: Contact::Mobile(seat),
            ..track.impact
        };
        return Drilled {
            legs: track.legs,
            impact,
            marks: track.marks,
        };
    }
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

/// The first mobile the drill strikes along `path`, and the stage tick: one it can strike
/// (`drill_strikes`), at the first tick inside its hitbox, as `Game::fly` finds a mobile.
fn first_strike(game: &Game, path: &Track) -> Option<(u8, u32)> {
    let frames = strikeable(game);
    let last = path.legs.len().saturating_sub(1);
    for (index, leg) in path.legs.iter().enumerate() {
        for tick in leg.from..=leg.to {
            // Off the map's edge, it is gone before it can strike anything.
            if index == last && tick == leg.to && path.impact.contact == Contact::Gone {
                return None;
            }
            let at = leg.flight.at(tick);
            if let Some(&(seat, _)) = frames.iter().find(|(_, frame)| frame.contains(at)) {
                return Some((seat, leg.offset + tick));
            }
        }
    }
    None
}

/// The living mobiles the drill can strike, by seat.
fn strikeable(game: &Game) -> Vec<(u8, HitFrame)> {
    game.hit_frames()
        .into_iter()
        .filter(|&(seat, _)| game.players().get(usize::from(seat)).is_some_and(|player| drill_strikes(player.mobile)))
        .collect()
}

/// The bore on from where the drill struck the ground (`path`'s end): down into it at the angle and
/// speed it struck, the way it was heading, under `gravity` turned upward, until the first open
/// pixel it reaches. On level ground that is a mirror of its arc. Its leg, and where it came up: in
/// a mobile standing there (`drill_strikes`), or on open ground; `Gone` if it left the map's sides or
/// bottom first.
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
    let frames = strikeable(game);
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
            Event, Team,
            testing::{damage_to, duel_of, explosions, game_on, seat, shoot},
        },
        terrain::Terrain,
        tuning::augerdillo::MOBILE,
        units::{Facing, Power},
        weather::{Column, WeatherKind},
    };

    /// Augerdillo at x = 300 with its back to a Coilback `distance` px to its right, on flat ground.
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
        assert!(MOBILE.fires_rear());
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
    fn s2_strikes_a_mobile_in_flight_as_a_round_does() {
        let game = duel(800);
        // The S1's power: S1 and S2 leave the barrel on the same flight.
        let power = striking_power(&game, Shot::S1, 150);
        let flight = game.launch(0, Shot::S2, 150, power.bars()).unwrap();
        let drilled = drill(&game, flight, MOBILE.gravity.value);
        assert_eq!(drilled.legs.len(), 1, "no bore");
        assert_eq!(drilled.impact, game.track(flight, 0, 0).impact, "where any shell strikes it");
        let mut game = game;
        let dealt = damage_to(&shoot(&mut game, 0, Shot::S2, 150, power), 1);
        assert!(dealt >= 182, "{dealt}");
    }

    #[test]
    fn s2_strikes_as_tracking_through_the_weather_does() {
        // A column it passes after striking doesn't mark it; one it passes before does.
        let calm = duel(800);
        let power = striking_power(&calm, Shot::S1, 150);
        let flight = calm.launch(0, Shot::S2, 150, power.bars()).unwrap();
        for (x, forced) in [(700, true), (1150, false)] {
            let mut game = calm.clone();
            game.set_columns(vec![Column::new(WeatherKind::Force, x)]);
            let drilled = drill(&game, flight, MOBILE.gravity.value);
            assert_eq!(drilled.impact.contact, Contact::Mobile(1));
            assert_eq!(drilled.marks.forced, forced, "a column at {x}");
            assert_eq!(drilled.impact, game.track(flight, 0, 0).impact);
        }
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
    fn ss_that_meets_no_mobile_does_nothing() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::Ss, 150, Power::nearest(0.8));
        assert_eq!(explosions(&effects), 0, "through the ground and out of the map's bottom: {effects:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::SsLocked { seat: 0, .. })));
    }

    #[test]
    fn ss_hurts_most_at_the_weak_spot_low_on_a_target() {
        // Fired flat through the ground beside the target, it strikes low on its side; lobbed, it
        // strikes the top of its body.
        let low_and_high = |aim: i16| {
            let game = deep(600);
            let hitbox = game.players()[1].tuning().hitbox.value;
            (1..=Power::MAX.twentieths())
                .filter_map(|twentieths| {
                    let power = Power::from_twentieths(twentieths).unwrap();
                    let impact = aim_point(&game, 0, Shot::Ss, game.launch(0, Shot::Ss, aim, power.bars()).unwrap())?;
                    (impact.contact == Contact::Mobile(1)).then(|| (game.players()[1].pose().to_body(impact.at).y - hitbox.y, power))
                })
                .collect::<Vec<_>>()
        };
        let dealt = |aim: i16, power: Power| damage_to(&shoot(&mut deep(600), 0, Shot::Ss, aim, power), 1);
        let flat = low_and_high(170).into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).expect("a flat shot strikes it");
        let lob = low_and_high(135).into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).expect("a lob strikes it");
        assert!(flat.0 > lob.0, "the flat shot strikes lower: {flat:?} {lob:?}");
        let (low, high) = (dealt(170, flat.1), dealt(135, lob.1));
        assert!(low > high && high >= 336, "{low} {high}");
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
