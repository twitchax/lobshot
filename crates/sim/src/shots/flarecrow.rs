//! Flarecrow (Aduka), which fires over its back: an electric ball whose aftershock runs on from
//! mobile to mobile; three flares down one path, each calling Skyeye's beam onto where it lands
//! (`skyeye`); and an SS that flies through land and mobiles alike, calling a heavier beam onto each
//! enemy it passes into.

use std::collections::VecDeque;

use super::{Gun, land_in_order, skyeye};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Senses, Shot, Track},
    mines,
    tuning::{
        flarecrow::{S1_AFTERSHOCK, S1_AFTERSHOCK_REACH, S1_BLAST, S1_CHAIN, S1_CHAIN_REACH, S1_DAMAGE, S2_FLARE_SECONDS, S2_FLARES, SS_BEAM_BOOST},
        weather::SKYEYE_DELAY_SECONDS,
    },
    units::{Vec2, ticks},
};

pub(super) fn fire(game: &mut Game, gun: &mut Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            let track = gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
            if landed(&track) {
                aftershock(game, gun, track.impact.at, track.impact.tick, effects);
            }
        }
        Shot::S2 => flares(game, gun, flight, effects),
        Shot::Ss => pass_through(game, gun, flight, effects),
    }
}

fn landed(track: &Track) -> bool {
    matches!(track.impact.contact, Contact::Terrain | Contact::Mobile(_))
}

/// S1's aftershock at `tick`: a flat hit on each mobile within reach of where the ball landed,
/// then a weaker one from each mobile it hit to each mobile beside that one, and on down the line.
/// Each is splash, landing once a turn on each mobile (`Salvo::splashed`), and each shows as an arc
/// (`Bolt`) from where it came.
fn aftershock(game: &mut Game, gun: &mut Gun, at: Vec2, tick: u32, effects: &mut Vec<Effect>) {
    let first = Wave {
        from: at,
        reach: S1_AFTERSHOCK_REACH.value,
        amount: S1_AFTERSHOCK.value,
    };
    let mut conducting: VecDeque<u8> = shock(game, gun, &first, tick, effects).into();
    while let Some(seat) = conducting.pop_front() {
        let next = Wave {
            from: game.players()[usize::from(seat)].center(),
            reach: S1_CHAIN_REACH.value,
            amount: S1_CHAIN.value,
        };
        conducting.extend(shock(game, gun, &next, tick, effects));
    }
}

/// One step of the aftershock, from a point.
struct Wave {
    from: Vec2,
    /// How far from `from` it reaches a hitbox.
    reach: f64,
    amount: i32,
}

/// `wave`'s amount to each living mobile within its reach that splash hasn't hit yet this turn, in
/// seat order. Returns those it hit, to pass it on.
fn shock(game: &mut Game, gun: &mut Gun, wave: &Wave, tick: u32, effects: &mut Vec<Effect>) -> Vec<u8> {
    let mut hit = Vec::new();
    for (index, splashed) in gun.salvo.splashed.iter_mut().enumerate().take(game.players().len()) {
        let player = game.players()[index];
        let Ok(seat) = u8::try_from(index) else { continue };
        if !player.alive || *splashed || player.pose().gap(player.tuning().hitbox.value, wave.from) > wave.reach {
            continue;
        }
        *splashed = true;
        effects.push(Effect::Bolt {
            from: wave.from,
            to: player.center(),
            tick,
        });
        game.hurt(seat, wave.amount, effects);
        hit.push(seat);
    }
    hit
}

/// S2: the flares leave one after another down the same path, and each lands in turn; Skyeye beams
/// at each landing a beat later, and Thunder or Electricity calls its bolt too. A flare that leaves
/// the map calls nothing.
fn flares(game: &mut Game, gun: &Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let every = ticks(S2_FLARE_SECONDS.value);
    let count = S2_FLARES.value;
    let (paths, mut landings) = land_in_order(
        game,
        usize::try_from(count).unwrap_or(0),
        |game, index| game.track_launched(flight, u32::try_from(index).unwrap_or(0) * every),
        |game, _, track, out| {
            if !landed(track) {
                return;
            }
            gun.thunder(game, track.impact.at, track.impact.tick, track.marks, out);
            skyeye::beam(game, track.impact.at, track.impact.tick + ticks(SKYEYE_DELAY_SECONDS.value), out);
        },
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// The SS: it flies through everything until it leaves the map, and each time it passes into a
/// living enemy, Skyeye beams at that enemy's center a beat later, at `SS_BEAM_BOOST`.
fn pass_through(game: &mut Game, gun: &Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false });
    effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
    let delay = ticks(SKYEYE_DELAY_SECONDS.value);
    // It calls a beam onto each enemy it passes into, and each mine it meets (WP06: a mini-Raon
    // triggers it as an enemy does), in the order it meets them.
    let mut calls: Vec<(u32, Call)> = entries(game, gun.seat, &track).into_iter().map(|(seat, tick)| (tick, Call::Enemy(seat))).collect();
    calls.extend(mines::crossed(game, &track.legs).into_iter().map(|(mine, tick)| (tick, Call::Mine(mine.middle()))));
    calls.sort_by_key(|&(tick, _)| tick);
    for (tick, call) in calls {
        let target = match call {
            Call::Enemy(seat) => {
                let enemy = game.players()[usize::from(seat)];
                if !enemy.alive {
                    continue;
                }
                enemy.center()
            }
            Call::Mine(at) => at,
        };
        skyeye::beam_by(game, target, tick + delay, SS_BEAM_BOOST.value, effects);
    }
}

/// What the SS calls a beam onto as it passes.
#[derive(Debug, Clone, Copy)]
enum Call {
    Enemy(u8),
    Mine(Vec2),
}

/// Each time `track` passes into an enemy of `shooter`'s, in order: the enemy, and the stage tick.
/// Mobiles are where they stood at launch.
fn entries(game: &Game, shooter: u8, track: &Track) -> Vec<(u8, u32)> {
    let team = game.players()[usize::from(shooter)].team;
    let enemies: Vec<_> = game.hit_frames().into_iter().filter(|&(seat, _)| game.players()[usize::from(seat)].team != team).collect();
    let mut inside = vec![false; enemies.len()];
    let mut entries = Vec::new();
    for leg in &track.legs {
        for tick in leg.from..=leg.to {
            let at = leg.flight.at(tick);
            for (was, &(seat, ref frame)) in inside.iter_mut().zip(&enemies) {
                let now = frame.contains(at);
                if now && !*was {
                    entries.push((seat, leg.offset + tick));
                }
                *was = now;
            }
        }
    }
    entries
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, Leg, MobileKind, Team,
            testing::{damage_to, direct_hit_power, game_of, seat, shoot},
        },
        items::{Item, Loadout},
        units::{Centis, Facing, Power},
        weather::{Column, WeatherKind},
    };

    /// Flarecrow at x = 300 with its back to a Coilback `distance` px away, so its shots fly at it.
    fn duel(distance: i32) -> Game {
        game_of(vec![
            seat(Team::A, MobileKind::Flarecrow, 300, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 300 + distance, Facing::Left),
        ])
    }

    fn paths(effects: &[Effect]) -> Vec<Vec<Leg>> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.clone()) } else { None }).collect()
    }

    fn beams(effects: &[Effect]) -> Vec<Vec2> {
        effects.iter().filter_map(|e| if let Effect::Beam { to, .. } = *e { Some(to) } else { None }).collect()
    }

    fn arcs(effects: &[Effect]) -> usize {
        effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count()
    }

    #[test]
    fn it_fires_over_its_back() {
        // Facing right in the middle of the map, every aim it has sends the shot left.
        let mut game = game_of(vec![seat(Team::A, MobileKind::Flarecrow, 1200, Facing::Right), seat(Team::B, MobileKind::Coilback, 2000, Facing::Left)]);
        let range = MobileKind::Flarecrow.tuning().s1.aim.value;
        assert!(range.backward());
        for aim in [range.min, range.max] {
            let flight = game.launch(0, Shot::S1, aim, 2.0).unwrap();
            assert!(flight.velocity.x < 0.0 && flight.origin.x < 1200.0, "{aim}: {flight:?}");
        }
        let effects = shoot(&mut game, 0, Shot::S1, 140, Power::nearest(2.0));
        let landed = paths(&effects)[0].last().map(|leg| leg.flight.at(leg.to)).unwrap();
        assert!(landed.x < 1000.0, "{landed:?}");
    }

    #[test]
    fn s1_on_target_deals_its_range_with_the_aftershock() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 130);
        let effects = shoot(&mut game, 0, Shot::S1, 130, power);
        let dealt = damage_to(&effects, 1);
        // The ball's 70-90 is Electric, 4% strong against Mechanical Coilback; the aftershock's 70
        // is flat.
        assert!((140..=163).contains(&dealt), "{dealt}: {effects:?}");
        assert_eq!(arcs(&effects), 1, "one arc, to the target: {effects:?}");
    }

    #[test]
    fn s1_shocks_a_mobile_it_misses_within_reach() {
        let game = duel(800);
        let target = game.players()[1];
        let reach = |game: &Game, power: Power| {
            let flight = game.launch(0, Shot::S1, 130, power.bars())?;
            let impact = game.track(flight, 0, 0).impact;
            (impact.contact == Contact::Terrain).then(|| target.pose().gap(target.tuning().hitbox.value, impact.at))
        };
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|t| Power::from_twentieths(t).ok())
            .find(|&power| reach(&game, power).is_some_and(|gap| (S1_BLAST.value..S1_AFTERSHOCK_REACH.value).contains(&gap)))
            .expect("a near miss past the blast, inside the aftershock");
        let effects = shoot(&mut game.clone(), 0, Shot::S1, 130, power);
        assert_eq!(damage_to(&effects, 1), S1_AFTERSHOCK.value, "the aftershock alone: {effects:?}");
    }

    #[test]
    fn s1_aftershock_runs_down_a_line_of_mobiles_weaker() {
        // Three enemies side by side, a fourth well off.
        let mut game = game_of(vec![
            seat(Team::A, MobileKind::Flarecrow, 300, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1000, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1080, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1160, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1600, Facing::Left),
        ]);
        // A flat shot into the first one's near face, out of the aftershock's own reach of the
        // second.
        let near_face = game.players()[1].center().x - 30.0;
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|t| Power::from_twentieths(t).ok())
            .find(|&power| {
                let impact = game.track(game.launch(0, Shot::S1, 170, power.bars()).unwrap(), 0, 0).impact;
                impact.contact == Contact::Mobile(1) && impact.at.x <= near_face
            })
            .expect("a flat shot into its near face");
        let effects = shoot(&mut game, 0, Shot::S1, 170, power);
        assert!(damage_to(&effects, 1) >= 140, "{effects:?}");
        assert_eq!([2, 3, 4].map(|seat| damage_to(&effects, seat)), [S1_CHAIN.value, S1_CHAIN.value, 0], "{effects:?}");
        assert_eq!(arcs(&effects), 3, "{effects:?}");
    }

    #[test]
    fn s1_aftershock_lands_once_a_turn_even_with_dual() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 130);
        game.players[0].items = Loadout::new(&[Item::Dual]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 130, power);
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Volley { .. })).count(), 2);
        assert_eq!(arcs(&effects), 1, "{effects:?}");
    }

    #[test]
    fn s2_fires_three_flares_down_one_path_and_beams_where_each_lands() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 130);
        let effects = shoot(&mut game, 0, Shot::S2, 130, power);
        let flares = paths(&effects);
        assert_eq!(flares.len(), 3);
        let every = ticks(S2_FLARE_SECONDS.value);
        for (order, legs) in (0_u32..).zip(&flares) {
            assert_eq!((legs[0].flight, legs[0].offset), (flares[0][0].flight, order * every));
        }
        let struck = beams(&effects);
        assert_eq!(struck.len(), 3, "{effects:?}");
        for to in struck {
            assert!(to.distance(game.players()[1].center()) < 40.0, "a beam struck the target: {to:?}");
        }
        // Each beam is Skyeye's at level 1, 50-100; the flares themselves hurt nothing.
        let dealt = damage_to(&effects, 1);
        assert!((150..=300).contains(&dealt), "{dealt}");
        assert_eq!(game.weather().skyeye.xp, u32::try_from(dealt).unwrap(), "they level Skyeye up");
    }

    #[test]
    fn s2_flares_drill_on_past_where_the_one_before_landed() {
        // A low shot that lands on open ground well short of the target.
        let mut game = duel(1200);
        let effects = shoot(&mut game, 0, Shot::S2, 165, Power::nearest(1.4));
        let landed: Vec<f64> = paths(&effects).iter().map(|legs| legs.last().map(|leg| leg.flight.at(leg.to).x).unwrap()).collect();
        assert_eq!(landed.len(), 3);
        assert!(landed[0] < landed[1] && landed[1] < landed[2], "each flies through the last one's crater: {landed:?}");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn s2_flares_off_the_map_call_nothing() {
        let mut game = game_of(vec![seat(Team::A, MobileKind::Flarecrow, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)]);
        let effects = shoot(&mut game, 0, Shot::S2, 160, Power::MAX);
        assert_eq!(paths(&effects).len(), 3);
        assert!(beams(&effects).is_empty(), "{effects:?}");
    }

    /// Two enemies in a row with a teammate between them, and the power whose SS passes into all
    /// three.
    fn line_up() -> (Game, Power) {
        let game = game_of(vec![
            seat(Team::A, MobileKind::Flarecrow, 300, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 520, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 620, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 720, Facing::Left),
        ]);
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|t| Power::from_twentieths(t).ok())
            .find(|&power| {
                let flight = game.launch(0, Shot::Ss, 170, power.bars()).unwrap();
                let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false });
                entries(&game, 0, &track).len() == 2
            })
            .expect("a flat SS through both enemies");
        (game, power)
    }

    #[test]
    fn ss_flies_through_land_and_beams_each_enemy_it_passes_into() {
        let (mut game, power) = line_up();
        let effects = shoot(&mut game, 0, Shot::Ss, 170, power);
        let legs = &paths(&effects)[0];
        assert_eq!(legs.len(), 1);
        let end = legs[0].flight.at(legs[0].to);
        assert!(end.y >= f64::from(game.terrain().height()) || end.x >= f64::from(game.terrain().width()), "it left the map: {end:?}");
        let struck = beams(&effects);
        assert_eq!(struck.len(), 2, "one beam each for the enemies, none for the teammate: {effects:?}");
        assert!(damage_to(&effects, 1) > 0 && damage_to(&effects, 3) > 0, "{effects:?}");
        assert_eq!(damage_to(&effects, 2), 0, "{effects:?}");
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::Exploded { at, .. } if at.y > 500.0)),
            "it carved nothing under the ground: {effects:?}"
        );
    }

    #[test]
    fn ss_beams_a_mine_it_passes_over_and_sets_it_off() {
        // WP06: Aduka's SS "triggers Thor's Hammer when it hits an enemy mobile ..., a jewel, or a
        // mini-Raon". A mine on the ground where its path goes into it.
        let (mut game, power) = line_up();
        let flight = game.launch(0, Shot::Ss, 170, power.bars()).unwrap();
        let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false });
        let leg = track.legs[0];
        let into = (leg.from..=leg.to).map(|tick| leg.flight.at(tick)).find(|at| at.y >= 496.0).unwrap();
        let mut laid = Vec::new();
        mines::lay(&mut game, 1, Vec2::new(into.x, 499.5), Facing::Left, 0, &mut laid);
        let mine = game.mines()[0];
        assert!(mine.middle().distance(into) <= crate::tuning::tinbadger::MINE_RADIUS.value, "{into:?} {mine:?}");
        let effects = shoot(&mut game, 0, Shot::Ss, 170, power);
        assert_eq!(beams(&effects).len(), 3, "two enemies and the mine: {effects:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { mine: id, tick: Some(_) } if *id == mine.id)), "{effects:?}");
        assert_eq!(game.mines(), []);
    }

    #[test]
    fn ss_beams_hit_harder_than_skyeyes_own() {
        let (mut game, power) = line_up();
        let effects = shoot(&mut game, 0, Shot::Ss, 170, power);
        // Level 1's 50-100, times 1.6; Laser is neutral against Mechanical.
        for seat in [1, 3] {
            let dealt = damage_to(&effects, seat);
            assert!((80..=160).contains(&dealt), "seat {seat}: {dealt}");
        }
    }

    #[test]
    fn ss_beams_an_enemy_again_when_the_weather_sends_it_back_through() {
        let (game, _) = line_up();
        // Just past the first enemy, a mirror.
        let mut mirrored = game.clone();
        mirrored.players[3].alive = false;
        let far_edge = mirrored.players()[1].center().x + 40.0;
        // Columns stand on whole pixels; this one's left edge is just past the enemy.
        #[allow(clippy::cast_possible_truncation)]
        mirrored.set_columns(vec![Column::new(WeatherKind::Mirror, far_edge.round() as i32 + 20)]);
        let twice = (0..=Power::MAX.twentieths()).filter_map(|t| Power::from_twentieths(t).ok()).find(|&power| {
            let flight = mirrored.launch(0, Shot::Ss, 170, power.bars()).unwrap();
            let track = mirrored.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false });
            entries(&mirrored, 0, &track).iter().filter(|(seat, _)| *seat == 1).count() == 2
        });
        let power = twice.expect("a power whose SS comes back through the enemy");
        let effects = shoot(&mut mirrored, 0, Shot::Ss, 170, power);
        assert_eq!(beams(&effects).len(), 2, "{effects:?}");
    }
}
