//! Lodestoad (J.D.): an electric orb whose discharge splashes everyone near it; a magnet ball
//! that passes through mobiles, digs in where it strikes the ground and pulls the mobiles near it
//! in; and an SS pyramid that pushes them away, hurting most those it can't push all the way. A
//! pulled or pushed mobile slides along the ground by the walking rules (`movement::slide`): walls
//! stop it, a pull drags it over edges and into holes (off the map, with nothing below), and a
//! push leaves it on the very edge. A floating mobile is pushed but not pulled.

use super::{Gun, bolt};
use crate::{
    ballistics::Flight,
    damage,
    game::{Contact, Effect, Game, Impact, Player, Senses, Shot, Track},
    mines,
    movement::{self, Edge},
    tuning::{
        Tuned,
        lodestoad::{
            PIN_DAMAGE, PULL_OVER_EDGES, PULL_REACH, PULL_SHARES, PULLS_FLOATERS, PUSH_OVER_EDGES, PUSH_REACH, PUSH_SHARES, PUSH_TO, PUSHES_FLOATERS, S1_BLAST, S1_DAMAGE, S2_BLAST, S2_DAMAGE,
            SLIDE_SECONDS, SPLASH, SPLASH_REACH, SS_BLAST, SS_DAMAGE, Shares,
        },
    },
    units::{Facing, ticks},
};

/// Which way a field moves the mobiles in its reach.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Way {
    /// To the column where it struck.
    Pull,
    /// Away, to this far from that column.
    Push(f64),
}

/// How a magnetic shot moves the mobiles near where it struck: those whose middle (the hitbox's
/// center) is within `reach` of it, across and up or down, move their class's share of the way.
struct Field {
    way: Way,
    reach: (f64, f64),
    shares: Shares,
    edge: Edge,
    /// Whether it moves a mobile that floats over the ground (`MobileKind::floats`).
    floaters: bool,
}

impl Field {
    fn pull() -> Self {
        Self {
            way: Way::Pull,
            reach: PULL_REACH.value,
            shares: PULL_SHARES.value,
            edge: edge(PULL_OVER_EDGES),
            floaters: PULLS_FLOATERS.value,
        }
    }

    fn push() -> Self {
        Self {
            way: Way::Push(PUSH_TO.value),
            reach: PUSH_REACH.value,
            shares: PUSH_SHARES.value,
            edge: edge(PUSH_OVER_EDGES),
            floaters: PUSHES_FLOATERS.value,
        }
    }
}

/// Whether a field takes a mobile over an edge, as tuned.
fn edge(over: Tuned<bool>) -> Edge {
    if over.value { Edge::Drops } else { Edge::Holds }
}

pub(super) fn fire(game: &mut Game, gun: &mut Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => orb(game, gun, flight, effects),
        Shot::S2 => magnet(game, gun, flight, effects),
        Shot::Ss => pyramid(game, gun, flight, effects),
    }
}

fn landed(track: &Track) -> bool {
    matches!(track.impact.contact, Contact::Terrain | Contact::Mobile(_))
}

/// S1: the orb lands like any shell, and its discharge splashes every mobile near the blast that
/// splash hasn't hit this turn.
fn orb(game: &mut Game, gun: &mut Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let track = gun.plain(game, flight, S1_BLAST.value, S1_DAMAGE.value, effects);
    if landed(&track) {
        // The discharge spreads from where the orb burst: a bolt of no length.
        let discharge = bolt::Strike {
            from: track.impact.at,
            at: track.impact.at,
            contact: track.impact.contact,
        };
        bolt::splash(game, &mut gun.salvo.splashed, &discharge, SPLASH.value, SPLASH_REACH.value, effects);
    }
}

/// S2: the ball flies through mobiles to the ground and bursts there, burning low; then it pulls
/// the mobiles near it in, and the mines near it, and its bolt and beam follow (Thunder, Electricity, Skyeye) onto
/// wherever they now stand.
fn magnet(game: &mut Game, gun: &Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let track = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: true });
    effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
    if !landed(&track) {
        return;
    }
    let blast = gun.blast_on(&track, S2_BLAST.value, S2_DAMAGE.value);
    game.explode_by(&blast, S2_BLAST.value * gun.salvo.crater, track.impact.tick, effects, |_, target| damage::low(&blast, target));
    shove(game, &track.impact, heading(&track), &Field::pull(), effects);
    // The mines near it are dragged into its hole, under whoever it pulled there.
    mines::drag(game, track.impact.at, PULL_REACH.value.0, track.impact.tick, effects);
    gun.follow_up(game, blast.center, track.impact.tick, track.marks, effects);
}

/// The SS: the pyramid lands like any shell, pushes the mobiles near it away, and hurts more each
/// one it couldn't push all the way; then its bolt and beam follow.
fn pyramid(game: &mut Game, gun: &Gun, flight: Flight, effects: &mut Vec<Effect>) {
    let track = game.track(flight, 0, 0);
    effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
    if !landed(&track) {
        return;
    }
    let direct = if let Contact::Mobile(seat) = track.impact.contact { Some(seat) } else { None };
    let blast = gun.blast_on(&track, SS_BLAST.value, SS_DAMAGE.value);
    game.explode(&blast, SS_BLAST.value * gun.salvo.crater, direct, track.impact.tick, effects);
    for (seat, short) in shove(game, &track.impact, heading(&track), &Field::push(), effects) {
        let Some(target) = game.players().get(usize::from(seat)).filter(|p| p.alive).map(Player::target) else {
            continue;
        };
        let pinned = PIN_DAMAGE.value * short / PUSH_TO.value;
        game.hurt(seat, damage::scale(pinned, blast.weapon, &target, blast.false_angle, blast.boost), effects);
    }
    gun.follow_up(game, blast.center, track.impact.tick, track.marks, effects);
}

/// Which way a projectile was going as it struck: a push from dead center goes that way.
fn heading(track: &Track) -> Facing {
    if track.last.flight.velocity_at(track.last_tick).x < 0.0 { Facing::Left } else { Facing::Right }
}

/// Slides every living mobile whose middle is within the field's reach of `impact` along the
/// ground, all at once from the impact's tick (a pull drags them over edges, into holes and on),
/// then drops any it took off the map. Returns, for each one a wall or an edge stopped short of
/// its due, how many pixels short.
fn shove(game: &mut Game, impact: &Impact, heading: Facing, field: &Field, effects: &mut Vec<Effect>) -> Vec<(u8, f64)> {
    let mut short = Vec::new();
    for index in 0..game.players().len() {
        let Ok(seat) = u8::try_from(index) else { continue };
        let player = game.players()[index];
        let (center, foot) = (player.center(), player.footing.world());
        let floats = player.mobile.floats() && !field.floaters;
        if !player.alive || floats || (impact.at.x - center.x).abs() > field.reach.0 || (impact.at.y - center.y).abs() > field.reach.1 {
            continue;
        }
        let across = (impact.at.x - foot.x).abs();
        let toward = if impact.at.x < foot.x { Facing::Left } else { Facing::Right };
        let (direction, way) = match field.way {
            Way::Pull => (toward, across),
            // Struck in its own column: dead center.
            Way::Push(to) if across < 0.5 => (heading, to),
            Way::Push(to) => (toward.flip(), to - across),
        };
        let tuning = player.tuning();
        // A few dozen pixels, never negative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let due = (way * field.shares.of(tuning.armor.value)).round().max(0.0) as u32;
        let path = movement::slide(game.terrain(), player.footing, direction, tuning.climb_px.value, due, field.edge);
        let Some(&end) = path.last() else {
            if due > 0 {
                short.push((seat, f64::from(due)));
            }
            continue;
        };
        let moved = end.x.abs_diff(player.footing.x);
        if moved < due {
            short.push((seat, f64::from(due - moved)));
        }
        game.place(seat, end);
        effects.push(Effect::Slid {
            seat,
            path,
            from: impact.tick,
            to: impact.tick + ticks(SLIDE_SECONDS.value),
        });
    }
    game.settle_all(effects);
    short
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Death, Event, MobileKind, Team,
            testing::{damage_to, direct_hit_power, duel_of, game_of, game_on, seat, shoot},
        },
        movement::Footing,
        terrain::Terrain,
        units::{Centis, Power, Vec2},
    };

    /// Lands `field` at `at` (struck while flying `heading`), at the stage's tick 0, and returns its
    /// effects and who it stopped short.
    fn land(game: &mut Game, field: &Field, at: Vec2, heading: Facing) -> (Vec<Effect>, Vec<(u8, f64)>) {
        let mut effects = Vec::new();
        let impact = Impact { tick: 0, at, contact: Contact::Terrain };
        let short = shove(game, &impact, heading, field, &mut effects);
        (effects, short)
    }

    fn slides(effects: &[Effect]) -> Vec<(u8, Vec<Footing>)> {
        effects
            .iter()
            .filter_map(|e| if let Effect::Slid { seat, path, .. } = e { Some((*seat, path.clone())) } else { None })
            .collect()
    }

    /// Lodestoad at 300 and `mobile` at 1000 facing it, on the flat duel map (ground at 500).
    fn flat(mobile: MobileKind) -> Game {
        game_of(vec![seat(Team::A, MobileKind::Lodestoad, 300, Facing::Right), seat(Team::B, mobile, 1000, Facing::Left)])
    }

    /// The ground in front of the mobile at 1000, `across` px toward Lodestoad.
    fn before(across: f64) -> Vec2 {
        Vec2::new(1000.5 - across, 500.0)
    }

    #[test]
    fn s1_is_an_electric_orb_and_its_discharge() {
        let mut game = duel_of(MobileKind::Lodestoad, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        // The orb's 76-106, Electric against Mechanical (+4%), and the flat 64 discharge.
        let dealt = damage_to(&effects, 1);
        assert!((79 + 64..=110 + 64).contains(&dealt), "{dealt} {effects:?}");
        assert!(
            effects.contains(&Effect::Damaged { seat: 1, amount: 64 }) || effects.contains(&Effect::Shielded { seat: 1, amount: 64 }),
            "{effects:?}"
        );
        assert_eq!(slides(&effects), [], "S1 moves no one");
    }

    /// Drops `shot` straight down onto the ground at `x`, once for each salvo in `salvos` (a Dual's
    /// two), and returns the effects.
    fn drop_shots(game: &mut Game, shot: Shot, x: f64, salvos: usize) -> Vec<Effect> {
        let mut salvo = super::super::Salvo::new(0, None, None);
        let mut effects = Vec::new();
        for _ in 0..salvos {
            let mut gun = Gun {
                salvo: &mut salvo,
                seat: 0,
                shot,
                aim: 30,
                bars: 2.0,
                tuning: game.players()[0].tuning(),
                false_angle: false,
                hook: 1.0,
            };
            let flight = Flight {
                origin: Vec2::new(x, 300.0),
                velocity: Vec2::ZERO,
                accel: Vec2::new(0.0, 600.0),
            };
            match shot {
                Shot::S1 => orb(game, &mut gun, flight, &mut effects),
                Shot::S2 => magnet(game, &gun, flight, &mut effects),
                Shot::Ss => pyramid(game, &gun, flight, &mut effects),
            }
        }
        effects
    }

    fn drop_orbs(game: &mut Game, x: f64, salvos: usize) -> Vec<Effect> {
        drop_shots(game, Shot::S1, x, salvos)
    }

    #[test]
    fn the_discharge_reaches_a_near_miss_and_lands_once_a_turn() {
        // Sparkhorn's hitbox reaches 32 px in front of its foot: the orb lands 40 px past that.
        let mut game = flat(MobileKind::Sparkhorn);
        let effects = drop_orbs(&mut game.clone(), 1000.5 - 32.0 - 40.0, 1);
        assert_eq!(damage_to(&effects, 1), SPLASH.value, "the discharge alone, flat against a shield: {effects:?}");
        let effects = drop_orbs(&mut game.clone(), 1000.5 - 32.0 - 60.0, 1);
        assert_eq!(damage_to(&effects, 1), 0, "out of its reach");
        let effects = drop_orbs(&mut game, 1000.5 - 32.0 - 40.0, 2);
        assert_eq!(damage_to(&effects, 1), SPLASH.value, "once a turn, even with Dual");
    }

    #[test]
    fn s2_flies_through_mobiles_to_the_ground_and_hurts_most_at_their_feet() {
        let mut game = duel_of(MobileKind::Lodestoad, MobileKind::Coilback, 800);
        let target = game.players()[1];
        // The power whose ball bursts in the ground closest under the target's feet.
        let foot = target.footing.world();
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let flight = game.launch(0, Shot::S2, 30, power.bars())?;
                let impact = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: true }).impact;
                (impact.contact == Contact::Terrain).then(|| (impact.at.distance(foot), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .unwrap();
        assert_eq!(
            game.track(game.launch(0, Shot::S2, 30, power.bars()).unwrap(), 0, 0).impact.contact,
            Contact::Mobile(1),
            "a shell would strike it"
        );
        let effects = shoot(&mut game, 0, Shot::S2, 30, power);
        // 150-200 by height up the body, at its feet the most; Electric against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((190..=208).contains(&dealt), "{dealt} {effects:?}");
    }

    #[test]
    fn the_magnet_pulls_a_machine_all_the_way_to_where_it_struck() {
        let mut game = flat(MobileKind::Coilback);
        let (effects, short) = land(&mut game, &Field::pull(), before(40.0), Facing::Right);
        let moved = slides(&effects);
        assert_eq!(moved.len(), 1, "only the mobile in reach: {effects:?}");
        let (seat, path) = &moved[0];
        assert_eq!((*seat, path.len()), (1, 40), "a footing a step");
        assert!(path.iter().all(|footing| footing.y == 500), "along the ground");
        assert_eq!(game.players()[1].footing, Footing { x: 960, y: 500 }, "into the column it struck");
        assert_eq!(game.players()[0].footing.x, 300, "out of reach, unmoved");
        assert_eq!(short, [], "nothing stopped it");
    }

    #[test]
    fn machines_are_pulled_the_most_and_creatures_the_least() {
        let mut moved = Vec::new();
        for mobile in [MobileKind::Coilback, MobileKind::Sparkhorn, MobileKind::Hookclaw] {
            let mut game = flat(mobile);
            land(&mut game, &Field::pull(), before(40.0), Facing::Right);
            moved.push(1000 - game.players()[1].footing.x);
        }
        assert_eq!(moved, [40, 20, 10], "Mechanical, Shield, Bionic");
    }

    #[test]
    fn the_magnet_reaches_only_so_far_across_and_up_from_a_mobiles_middle() {
        let (across, height) = PULL_REACH.value;
        let center = flat(MobileKind::Coilback).players()[1].center();
        for (offset, moves) in [
            (Vec2::new(-(across - 1.0), 20.0), true),
            (Vec2::new(-(across + 1.0), 20.0), false),
            (Vec2::new(-20.0, -(height - 1.0)), true),
            (Vec2::new(-20.0, -(height + 1.0)), false),
        ] {
            let mut game = flat(MobileKind::Coilback);
            let (effects, _) = land(&mut game, &Field::pull(), center + offset, Facing::Right);
            assert_eq!(!slides(&effects).is_empty(), moves, "{offset:?}");
        }
    }

    /// Ground at 500 to x = 1000, then a pit to 1100 (bottomless, or with a floor at 580), then
    /// ground again; `mobile` stands at `x`, facing left at Lodestoad at 300.
    fn by_a_pit(mobile: MobileKind, x: i32, floor: bool) -> Game {
        let pit = Terrain::from_fn(2400, 600, |x, y| if (1000..1100).contains(&x) { floor && y >= 580 } else { y >= 500 }).unwrap();
        game_on(pit, vec![seat(Team::A, MobileKind::Lodestoad, 300, Facing::Right), seat(Team::B, mobile, x, Facing::Left)])
    }

    #[test]
    fn a_pull_drags_a_mobile_over_an_edge_and_off_the_map() {
        let mut game = by_a_pit(MobileKind::Coilback, 980, false);
        let (effects, _) = land(&mut game, &Field::pull(), Vec2::new(1030.0, 505.0), Facing::Right);
        let (_, path) = &slides(&effects)[0];
        assert_eq!(path.last(), Some(&Footing { x: 1000, y: 500 }), "over the edge: {path:?}");
        let slid = effects.iter().position(|e| matches!(e, Effect::Slid { .. })).unwrap();
        let died = effects.iter().position(|e| *e == Effect::Died { seat: 1, death: Death::Bunged }).expect("dragged off the map");
        assert!(slid < died, "{effects:?}");
        assert!(!game.players()[1].alive);
    }

    #[test]
    fn a_mobile_dragged_into_a_pit_with_a_floor_falls_in_and_slides_on() {
        let mut game = by_a_pit(MobileKind::Coilback, 980, true);
        let (effects, _) = land(&mut game, &Field::pull(), Vec2::new(1030.0, 505.0), Facing::Right);
        let (_, path) = &slides(&effects)[0];
        let edge = path.iter().position(|footing| *footing == Footing { x: 1000, y: 500 }).expect("over the edge");
        assert_eq!(path[edge + 1], Footing { x: 1000, y: 580 }, "down to the floor");
        assert_eq!(game.players()[1].footing, Footing { x: 1030, y: 580 }, "and on to where it struck");
        assert!(game.players()[1].alive);
    }

    #[test]
    fn the_pyramid_pushes_mobiles_away_and_creatures_less_far() {
        for (mobile, share) in [(MobileKind::Sparkhorn, 1.0), (MobileKind::Coilback, 1.0), (MobileKind::Hookclaw, 0.25)] {
            let mut game = flat(mobile);
            land(&mut game, &Field::push(), before(40.0), Facing::Right);
            // From 40 px off to PUSH_TO, or its share of that.
            #[allow(clippy::cast_possible_truncation)]
            let pushed = ((PUSH_TO.value - 40.0) * share).round() as i32;
            assert_eq!(game.players()[1].footing, Footing { x: 1000 + pushed, y: 500 }, "{mobile:?}");
        }
    }

    #[test]
    fn a_push_from_dead_center_goes_the_way_the_pyramid_flew() {
        for heading in [Facing::Left, Facing::Right] {
            let mut game = flat(MobileKind::Coilback);
            land(&mut game, &Field::push(), Vec2::new(1000.5, 470.0), heading);
            assert_eq!(game.players()[1].footing.x, 1000 + 100 * heading.sign(), "{heading:?}");
        }
    }

    #[test]
    fn a_push_leaves_a_mobile_on_the_very_edge_of_a_cliff_and_short() {
        let mut game = by_a_pit(MobileKind::Coilback, 980, false);
        let (effects, short) = land(&mut game, &Field::push(), Vec2::new(960.5, 500.0), Facing::Right);
        assert_eq!(game.players()[1].footing, Footing { x: 999, y: 500 }, "{effects:?}");
        assert!(game.players()[1].alive);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Fell { .. } | Effect::Died { .. })), "{effects:?}");
        assert_eq!(short, [(1, 61.0)], "80 px due, 19 taken");
    }

    #[test]
    fn a_wall_stops_a_slide() {
        // A wall 40 px tall just behind the target.
        let wall = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((1010..1030).contains(&x) && y >= 460)).unwrap();
        let mut game = game_on(
            wall,
            vec![seat(Team::A, MobileKind::Lodestoad, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1000, Facing::Left)],
        );
        let (_, short) = land(&mut game, &Field::push(), before(40.0), Facing::Right);
        assert_eq!(game.players()[1].footing, Footing { x: 1009, y: 500 });
        assert_eq!(short, [(1, 51.0)]);
    }

    #[test]
    fn a_pinned_mobile_takes_more_from_the_pyramid() {
        // The same SS at a Coilback in the open, and with a wall at its back.
        let wall = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((1012..1060).contains(&x) && y >= 440)).unwrap();
        let open = Terrain::from_fn(2400, 600, |_, y| y >= 500).unwrap();
        let mut dealt = Vec::new();
        for terrain in [open, wall] {
            let mut game = game_on(
                terrain,
                vec![seat(Team::A, MobileKind::Lodestoad, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1000, Facing::Left)],
            );
            let power = direct_hit_power_of(&game, 30);
            dealt.push(damage_to(&shoot(&mut game, 0, Shot::Ss, 30, power), 1));
        }
        assert!(dealt[1] > dealt[0] + 20, "{dealt:?}");
        assert!(dealt[1] > 300, "GF-max's '300+': {dealt:?}");
    }

    /// The power whose SS strikes seat 1 closest to its center, from seat 0 at `aim`.
    fn direct_hit_power_of(game: &Game, aim: i16) -> Power {
        crate::game::testing::direct_hit_power_of(game, Shot::Ss, aim)
    }

    #[test]
    fn everyone_in_reach_is_moved_the_shooter_and_its_team_too() {
        let mut game = game_of(vec![
            seat(Team::A, MobileKind::Lodestoad, 960, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1040, Facing::Left),
            seat(Team::A, MobileKind::Sparkhorn, 1300, Facing::Left),
        ]);
        let (effects, _) = land(&mut game, &Field::push(), Vec2::new(1000.5, 500.0), Facing::Right);
        let moved: Vec<u8> = slides(&effects).into_iter().map(|(seat, _)| seat).collect();
        assert_eq!(moved, [0, 1], "the shooter and the enemy in reach, not the teammate past it: {effects:?}");
        assert_eq!((game.players()[0].footing.x, game.players()[1].footing.x), (900, 1100));
    }

    #[test]
    fn the_magnet_drags_mines_into_its_hole_under_whoever_it_pulled_there() {
        let mut game = flat(MobileKind::Coilback);
        // A beetle 50 px past the ball's landing: past its blast, in its reach.
        let mut laid = Vec::new();
        crate::mines::lay(&mut game, 1, Vec2::new(1010.5, 499.5), Facing::Left, 0, &mut laid);
        let mine = game.mines()[0].id;
        let effects = drop_shots(&mut game, Shot::S2, 960.5, 1);
        let slid = effects.iter().position(|e| matches!(e, Effect::Slid { seat: 1, .. })).expect("pulled");
        let dragged = effects.iter().position(|e| matches!(e, Effect::MineFell { mine: m, .. } if *m == mine)).expect("dragged in");
        let gone = effects.iter().position(|e| matches!(e, Effect::MineGone { mine: m, tick: Some(_) } if *m == mine)).expect("set off");
        assert!(slid < dragged && dragged < gone, "{effects:?}");
        assert_eq!(game.players()[1].footing.x, 960, "pulled to the bottom of the hole");
        assert_eq!(game.mines(), [], "it went off under the Coilback");
    }

    #[test]
    fn a_floating_mobile_is_pushed_but_not_pulled() {
        for (field, moves) in [(Field::pull(), false), (Field::push(), true)] {
            let mut game = flat(MobileKind::Kitewyrm);
            assert!(game.players()[1].mobile.floats());
            let (effects, _) = land(&mut game, &field, before(30.0), Facing::Right);
            assert_eq!(!slides(&effects).is_empty(), moves, "{effects:?}");
        }
    }

    #[test]
    fn a_slide_starts_at_the_blast_and_lasts_a_quarter_second() {
        let mut game = flat(MobileKind::Coilback);
        let (effects, _) = land(&mut game, &Field::pull(), before(40.0), Facing::Right);
        let Some(Effect::Slid { from, to, .. }) = effects.iter().find(|e| matches!(e, Effect::Slid { .. })) else {
            panic!("{effects:?}")
        };
        assert_eq!((*from, *to), (0, ticks(0.25)));
    }

    #[test]
    fn a_fired_s2_pulls_its_target_into_its_hole_and_replays_the_same() {
        let game = duel_of(MobileKind::Lodestoad, MobileKind::Coilback, 800);
        // Short of the target, in reach of its feet.
        let target = game.players()[1].footing.world();
        let (power, _) = (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let flight = game.launch(0, Shot::S2, 30, power.bars())?;
                let impact = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: true }).impact;
                (impact.contact == Contact::Terrain).then(|| (power, (impact.at.x - (target.x - 30.0)).abs()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let event = Event::Shoot {
            seat: 0,
            shot: Shot::S2,
            aim: 30,
            power,
            time: Centis(0),
        };
        let mut played = game.clone();
        let effects = played.apply(event).unwrap();
        let blast = effects.iter().position(|e| matches!(e, Effect::Exploded { .. })).unwrap();
        let slid = effects.iter().position(|e| matches!(e, Effect::Slid { seat: 1, .. })).expect("pulled");
        assert!(blast < slid, "{effects:?}");
        let pulled = played.players()[1].footing;
        assert!(pulled.x < 1100 && pulled.y > 500, "toward it, and down: {pulled:?}");
        let mut again = game;
        again.apply(event).unwrap();
        assert_eq!(again.state_hash().unwrap(), played.state_hash().unwrap());
    }

    #[test]
    fn a_shot_off_the_map_moves_no_one() {
        let mut game = duel_of(MobileKind::Lodestoad, MobileKind::Coilback, 800);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let effects = shoot(&mut game, 0, Shot::Ss, 20, Power::MAX);
        assert_eq!(slides(&effects), [], "{effects:?}");
    }
}
