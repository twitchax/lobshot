//! Mines: things a shot leaves on the map that outlive its turn (Tinbadger's tin beetles, GunBound
//! Raon's S2). A mine sits on the ground where its shell landed, part of the game state like a
//! mobile (so the state hash covers it, a replayed log rebuilds it, and a client that joins late
//! sees it). As its layer's turn comes up, each of its mines with an enemy in sense crawls toward
//! it, a walk at a time (`shots::trundle`), and goes off on touching a mobile, whoever walks onto
//! it. A blast or a bolt in reach smashes one; a mine whose ground is blown away falls, as a mobile
//! does, and one that leaves the map, or whose layer dies, is gone. The rules' numbers are in
//! `tuning::tinbadger`, and their sources in `docs/research/tinbadger.md`.
//!
//! Everything a mine does reaches the client as effects naming it by `MineId`: laid, crawled (a
//! stage of its own, before its layer's turn starts), fallen and gone.

use serde::{Deserialize, Serialize};

use crate::{
    damage::Blast,
    game::{Contact, Effect, Game, MobileKind},
    movement::{self, Footing, Landing},
    shots::trundle::{self, Gait, Heading, Start},
    tuning::{
        ArmorClass,
        tinbadger::{MECHANICAL_PULL, MINE_BLAST, MINE_CRAWL_SPEED, MINE_CRAWL_STEPS, MINE_DAMAGE, MINE_HEIGHT, MINE_SENSE},
    },
    units::{Facing, Vec2},
};

/// A mine's name for the game's whole length: the order it was laid in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MineId(pub u32);

/// A mine on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mine {
    pub id: MineId,
    /// Who laid it: it crawls as that seat's turn comes up, and goes when that seat dies.
    pub seat: u8,
    /// The ground it sits on, as a mobile stands.
    pub at: Footing,
    /// The way it last crawled (at first, the way its layer faced).
    pub facing: Facing,
}

impl Mine {
    /// Its middle, a few pixels over the ground: what touches a mobile, and what a blast reaches.
    #[must_use]
    pub fn middle(&self) -> Vec2 {
        Self::middle_at(self.at)
    }

    /// The middle of a mine sitting at `at`.
    #[must_use]
    pub fn middle_at(at: Footing) -> Vec2 {
        GAIT.middle(at)
    }
}

/// The mines on the map, in the order they were laid, and how many have been.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Minefield {
    mines: Vec<Mine>,
    laid: u32,
}

impl Minefield {
    #[must_use]
    pub fn mines(&self) -> &[Mine] {
        &self.mines
    }

    fn get(&self, id: MineId) -> Option<Mine> {
        self.mines.iter().find(|mine| mine.id == id).copied()
    }

    fn take(&mut self, id: MineId) -> Option<Mine> {
        let index = self.mines.iter().position(|mine| mine.id == id)?;
        Some(self.mines.remove(index))
    }

    fn put(&mut self, mine: Mine) {
        if let Some(slot) = self.mines.iter_mut().find(|other| other.id == mine.id) {
            *slot = mine;
        }
    }
}

/// How a mine crawls: like a mobile, a column a step.
const GAIT: Gait = Gait {
    speed: MINE_CRAWL_SPEED.value,
    climb: crate::tuning::CLIMB_PX.value,
    height: MINE_HEIGHT.value,
};

/// Whether a mine senses `mobile` at all: crawls toward it, and goes off touching it. A mobile that
/// floats would be left out here: Dragon "ignores... Raon mines" (SW-mob).
#[must_use]
pub fn senses(mobile: MobileKind) -> bool {
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
        | MobileKind::Tinbadger => true,
    }
}

/// The living, sensed mobile whose hitbox `at` is inside, if any.
fn touching(game: &Game, at: Vec2) -> Option<u8> {
    (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive && senses(player.mobile))
        .find(|(_, player)| player.pose().hit(player.tuning().hitbox.value, at))
        .map(|(seat, _)| seat)
}

/// The enemy of `mine`'s layer it crawls toward: the nearest within `MINE_SENSE` of it, a
/// Mechanical one counting `MECHANICAL_PULL` times nearer.
fn quarry(game: &Game, mine: &Mine) -> Option<u8> {
    let team = game.players().get(usize::from(mine.seat))?.team;
    let at = mine.middle();
    (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive && player.team != team && senses(player.mobile))
        .map(|(seat, player)| {
            let gap = player.pose().gap(player.tuning().hitbox.value, at);
            let pull = if player.tuning().armor.value == ArmorClass::Mechanical { MECHANICAL_PULL.value } else { 1.0 };
            (gap / pull, seat)
        })
        .filter(|&(gap, _)| gap <= MINE_SENSE.value)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, seat)| seat)
}

/// Whether `mine` has an enemy of its layer's in sense: it would crawl as its layer's turn comes
/// up.
#[must_use]
pub fn armed(game: &Game, mine: &Mine) -> bool {
    quarry(game, mine).is_some()
}

/// `seat` lays a mine where its shell came down at `at` (a point just above the ground it struck),
/// at stage tick `tick`, facing `facing`. It settles onto the ground below; with none, it is lost
/// off the map. One that lands touching a mobile goes off at once.
pub(crate) fn lay(game: &mut Game, seat: u8, at: Vec2, facing: Facing, tick: u32, effects: &mut Vec<Effect>) {
    // A landing is a pixel or so from the ground, inside the map: in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let open = Footing {
        x: at.x.floor() as i32,
        y: at.y.floor() as i32,
    };
    let Landing::On(footing) = movement::settle(game.terrain(), open) else { return };
    let field = &mut game.mines;
    let mine = Mine {
        id: MineId(field.laid),
        seat,
        at: footing,
        facing,
    };
    field.laid += 1;
    field.mines.push(mine);
    effects.push(Effect::MineLaid { mine, tick });
    trip(game, tick, effects);
}

/// Whether `seat`'s mobile stands on a mine that senses it.
pub(crate) fn under(game: &Game, seat: u8) -> bool {
    game.mines.mines.iter().any(|mine| touching(game, mine.middle()) == Some(seat))
}

/// Sets off every mine touching a mobile it senses, at stage tick `tick`. Returns whether any went.
pub(crate) fn trip(game: &mut Game, tick: u32, effects: &mut Vec<Effect>) -> bool {
    let mut tripped = false;
    while let Some((mine, seat)) = game.mines.mines.iter().find_map(|mine| touching(game, mine.middle()).map(|seat| (*mine, seat))) {
        blow(game, mine, Some(seat), tick, effects);
        tripped = true;
    }
    tripped
}

/// Smashes every mine within `reach` of a blast or bolt at `center`, at stage tick `tick`: it is
/// gone, without going off. "Shooting them will eliminate the threat. Electrical splash damage will
/// also eliminate them" (GF-lee); OB sets them off instead (CONTRADICTION).
pub(crate) fn smash(game: &mut Game, center: Vec2, reach: f64, tick: u32, effects: &mut Vec<Effect>) {
    let (gone, kept): (Vec<Mine>, Vec<Mine>) = game.mines.mines.iter().partition(|mine| mine.middle().distance(center) <= reach);
    game.mines.mines = kept;
    effects.extend(gone.into_iter().map(|mine| Effect::MineGone { mine: mine.id, tick: Some(tick) }));
}

/// Takes `mine` off the map and blows it up, at its middle, direct on the mobile it touched.
fn blow(game: &mut Game, mine: Mine, direct: Option<u8>, tick: u32, effects: &mut Vec<Effect>) {
    if game.mines.take(mine.id).is_none() {
        return;
    }
    effects.push(Effect::MineGone { mine: mine.id, tick: Some(tick) });
    let blast = Blast {
        center: mine.middle(),
        radius: MINE_BLAST.value,
        damage: MINE_DAMAGE.value,
        weapon: crate::tuning::tinbadger::MOBILE.weapon.value,
        false_angle: false,
        boost: 1.0,
    };
    game.explode(&blast, MINE_BLAST.value, direct, tick, effects);
}

/// Drops every mine whose ground is gone: onto the ground below, or off the map. Returns whether
/// any moved.
pub(crate) fn settle(game: &mut Game, effects: &mut Vec<Effect>) -> bool {
    let mut moved = false;
    for mine in game.mines.mines.clone() {
        match movement::settle(game.terrain(), mine.at) {
            Landing::On(to) if to == mine.at => {}
            Landing::On(to) => {
                game.mines.put(Mine { at: to, ..mine });
                effects.push(Effect::MineFell { mine: mine.id, to });
                moved = true;
            }
            Landing::OffMap => {
                game.mines.take(mine.id);
                effects.push(Effect::MineGone { mine: mine.id, tick: None });
                moved = true;
            }
        }
    }
    moved
}

/// `seat` is dead: its mines go with it (OB), for a mine moves on its layer's turns alone.
pub(crate) fn orphan(game: &mut Game, seat: u8, effects: &mut Vec<Effect>) {
    let (gone, kept): (Vec<Mine>, Vec<Mine>) = game.mines.mines.iter().partition(|mine| mine.seat == seat);
    game.mines.mines = kept;
    effects.extend(gone.into_iter().map(|mine| Effect::MineGone { mine: mine.id, tick: None }));
}

/// As `seat`'s turn comes up, each of its mines with an enemy in sense crawls toward it, one after
/// another, each walk a stage of its own (`Effect::MineCrawled`): up to `MINE_CRAWL_STEPS`, stopping
/// at a wall, under its quarry, or on touching any mobile, where it goes off.
pub(crate) fn crawl(game: &mut Game, seat: u8, effects: &mut Vec<Effect>) {
    let ids: Vec<MineId> = game.mines.mines.iter().filter(|mine| mine.seat == seat).map(|mine| mine.id).collect();
    for id in ids {
        if game.decided() {
            return;
        }
        // An earlier mine's blast may have set this one off, or dropped it.
        let Some(mine) = game.mines.get(id) else { continue };
        if let Some(hit) = touching(game, mine.middle()) {
            blow(game, mine, Some(hit), 0, effects);
            game.settle_all(effects);
            continue;
        }
        let Some(target) = quarry(game, &mine) else { continue };
        let to = game.players()[usize::from(target)].footing.x;
        let walked = trundle::walk(
            game.terrain(),
            Start {
                at: mine.at,
                facing: mine.facing,
                tick: 0,
            },
            &GAIT,
            Heading::Toward(to),
            MINE_CRAWL_STEPS.value,
            |at| touching(game, at),
        );
        effects.push(Effect::MineCrawled { seat, mine: id, legs: walked.legs });
        match walked.end.contact {
            Contact::Gone => {
                game.mines.take(id);
                effects.push(Effect::MineGone { mine: id, tick: Some(walked.end.tick) });
            }
            Contact::Mobile(hit) => blow(game, Mine { at: walked.footing, ..mine }, Some(hit), walked.end.tick, effects),
            Contact::Terrain | Contact::Column(_) => game.mines.put(Mine {
                at: walked.footing,
                facing: walked.facing,
                ..mine
            }),
        }
        game.settle_all(effects);
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, Phase, Team,
            testing::{damage_to, duel_of, game_of, pass, seat, shoot},
        },
        log::Log,
        map::NoAssets,
        units::{Centis, Power},
    };

    /// A Tinbadger at x = 300 facing right, and a `target` 800 px away, on flat ground at 500.
    fn duel(target: MobileKind) -> Game {
        duel_of(MobileKind::Tinbadger, target, 800)
    }

    /// Lays a mine for `seat` on the ground at column `x`, as a shell landing there would.
    fn lay_at(game: &mut Game, seat: u8, x: i32) -> Vec<Effect> {
        let mut effects = Vec::new();
        lay(game, seat, Vec2::new(f64::from(x) + 0.5, 499.5), Facing::Right, 0, &mut effects);
        effects
    }

    /// Plays passes until it is seat 0's turn again, returning everything that happened.
    fn round(game: &mut Game) -> Vec<Effect> {
        let mut effects = pass(game, 0);
        while game.phase() != (Phase::Turn { seat: 0 }) {
            let Phase::Turn { seat } = game.phase() else { break };
            effects.extend(pass(game, seat));
        }
        effects
    }

    fn crawls(effects: &[Effect]) -> Vec<MineId> {
        effects.iter().filter_map(|e| if let Effect::MineCrawled { mine, .. } = e { Some(*mine) } else { None }).collect()
    }

    #[test]
    fn a_mine_stays_on_the_map_turn_after_turn_in_the_state_and_its_hash() {
        let mut game = duel(MobileKind::Coilback);
        let before = game.state_hash().unwrap();
        let effects = lay_at(&mut game, 0, 600);
        let laid = game.mines()[0];
        assert_eq!(effects, [Effect::MineLaid { mine: laid, tick: 0 }]);
        assert_eq!(laid.at, Footing { x: 600, y: 500 });
        assert_ne!(game.state_hash().unwrap(), before, "the hash covers the mines");
        for _ in 0..3 {
            round(&mut game);
        }
        assert_eq!(game.mines(), [laid], "far from any enemy, it waits");
    }

    #[test]
    fn as_its_layers_turn_comes_up_a_mine_crawls_into_an_enemy_in_sense_and_goes_off() {
        let mut game = duel(MobileKind::Coilback);
        let target = game.players()[1].footing.x;
        lay_at(&mut game, 0, target - 80);
        let effects = pass(&mut game, 0);
        assert!(crawls(&effects).is_empty(), "not on the enemy's turn: {effects:?}");
        assert_eq!(game.mines().len(), 1);
        let effects = pass(&mut game, 1);
        assert_eq!(crawls(&effects), [MineId(0)], "{effects:?}");
        let gone = effects.iter().position(|e| matches!(e, Effect::MineGone { mine: MineId(0), tick: Some(_) })).unwrap();
        let crawled = effects.iter().position(|e| matches!(e, Effect::MineCrawled { .. })).unwrap();
        let turn = effects.iter().position(|e| matches!(e, Effect::TurnStarted { seat: 0 })).unwrap();
        assert!(crawled < gone && gone < turn, "it crawls, goes off, then the turn starts: {effects:?}");
        // A direct hit by its own blast: 140-180, Explosion against Mechanical.
        assert!((140..=180).contains(&damage_to(&effects, 1)), "{effects:?}");
        assert_eq!(game.mines(), []);
    }

    #[test]
    fn a_crawl_reaches_its_target_at_walking_pace() {
        let mut game = duel(MobileKind::Coilback);
        let target = game.players()[1];
        lay_at(&mut game, 0, target.footing.x - 80);
        pass(&mut game, 0);
        let effects = pass(&mut game, 1);
        let Some(Effect::MineCrawled { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::MineCrawled { .. })) else {
            panic!("{effects:?}")
        };
        let end = legs.last().unwrap();
        let reached = end.flight.at(end.to);
        assert!(target.pose().hit(target.tuning().hitbox.value, reached), "it stops inside the target: {reached:?}");
        let Some(&Effect::Exploded { tick, at, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!()
        };
        assert_eq!(at, reached);
        // A pixel a step at MINE_CRAWL_SPEED, from 80 px off to the hitbox's edge.
        let steps = f64::from(tick) * crate::units::TICK * MINE_CRAWL_SPEED.value;
        let gap = target.pose().gap(
            target.tuning().hitbox.value,
            Mine {
                at: Footing { x: target.footing.x - 80, y: 500 },
                ..game_mine()
            }
            .middle(),
        );
        assert!((steps - gap).abs() <= 2.0, "{steps} steps for a {gap} px gap");
    }

    fn game_mine() -> Mine {
        Mine {
            id: MineId(0),
            seat: 0,
            at: Footing { x: 0, y: 0 },
            facing: Facing::Right,
        }
    }

    #[test]
    fn a_mine_with_no_enemy_in_sense_stays_put_and_one_crawls_at_most_its_steps() {
        let mut game = duel(MobileKind::Coilback);
        let target = game.players()[1].footing.x;
        // Out of sense: past MINE_SENSE (and its Mechanical pull) from the hitbox.
        lay_at(&mut game, 0, target - 200);
        // In sense, but farther than one crawl reaches.
        lay_at(&mut game, 0, target - 30 - 120);
        let effects = round(&mut game);
        assert_eq!(crawls(&effects), [MineId(1)], "{effects:?}");
        let moved = game.mines()[1];
        assert_eq!(moved.at.x, target - 150 + i32::try_from(MINE_CRAWL_STEPS.value).unwrap());
        assert_eq!(game.mines()[0].at.x, target - 200, "the far one never moved");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn a_mechanical_enemy_pulls_harder_than_a_nearer_bionic_one() {
        // Tinbadger, then a Bionic Marblemole and a Mechanical Coilback on the other team, the mine
        // between them, nearer the Marblemole.
        let seats = vec![
            seat(Team::A, MobileKind::Tinbadger, 300, Facing::Right),
            seat(Team::B, MobileKind::Marblemole, 900, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1100, Facing::Left),
        ];
        let mut game = game_of(seats);
        let between = 990;
        let gap = |game: &Game, seat: usize| {
            let player = game.players()[seat];
            player.pose().gap(player.tuning().hitbox.value, GAIT.middle(Footing { x: between, y: 500 }))
        };
        assert!(gap(&game, 1) < gap(&game, 2) && gap(&game, 2) < MECHANICAL_PULL.value * gap(&game, 1), "nearer the Bionic one");
        lay_at(&mut game, 0, between);
        let effects = round(&mut game);
        assert!(damage_to(&effects, 2) > 0 && damage_to(&effects, 1) == 0, "it went for the Coilback: {effects:?}");
    }

    #[test]
    fn a_blast_or_bolt_in_reach_smashes_a_mine_without_setting_it_off() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 0, 700);
        lay_at(&mut game, 0, 700 + 12);
        lay_at(&mut game, 0, 900);
        let mut effects = Vec::new();
        let blast = Blast {
            center: Vec2::new(690.0, 498.0),
            radius: 20.0,
            damage: MINE_DAMAGE.value,
            weapon: crate::tuning::WeaponType::Explosion,
            false_angle: false,
            boost: 1.0,
        };
        game.explode(&blast, 20.0, None, 50, &mut effects);
        let gone: Vec<MineId> = effects
            .iter()
            .filter_map(|e| if let Effect::MineGone { mine, tick: Some(50) } = e { Some(*mine) } else { None })
            .collect();
        assert_eq!(gone, [MineId(0)], "only the one in reach: {effects:?}");
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count(), 1, "smashed, not set off");
        let strike = crate::shots::bolt::from_sky(&game, 900.5);
        crate::shots::bolt::flash(&mut game, &strike, 12.0, 70, &mut effects);
        assert_eq!(game.mines().iter().map(|mine| mine.id).collect::<Vec<_>>(), [MineId(1)], "the bolt took the third");
    }

    #[test]
    fn a_mine_whose_ground_is_blown_away_falls_and_one_with_none_left_is_gone() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 0, 700);
        lay_at(&mut game, 0, 750);
        // A pit under the first; the second's column cut through to the bottom.
        for y in 500..530 {
            game.terrain.carve(Vec2::new(700.5, f64::from(y) + 0.5), 3.0);
        }
        for y in 500..600 {
            game.terrain.carve(Vec2::new(750.5, f64::from(y) + 0.5), 0.6);
        }
        let mut effects = Vec::new();
        game.settle_all(&mut effects);
        assert!(
            effects.iter().any(|e| matches!(e, Effect::MineFell { mine: MineId(0), to } if to.x == 700 && to.y > 525)),
            "into the pit: {effects:?}"
        );
        assert_eq!(game.mines()[0].at.y, game.terrain().surface_at_or_below(700, 500).unwrap());
        assert!(effects.contains(&Effect::MineGone { mine: MineId(1), tick: None }), "{effects:?}");
        assert_eq!(game.mines().len(), 1);
    }

    #[test]
    fn a_mine_goes_with_its_layer() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 0, 600);
        let effects = game.apply(Event::Forfeit { seat: 0 }).unwrap();
        assert!(effects.contains(&Effect::MineGone { mine: MineId(0), tick: None }), "{effects:?}");
        assert_eq!(game.mines(), []);
    }

    #[test]
    fn walking_onto_a_mine_sets_it_off_under_the_walker() {
        let mut game = duel(MobileKind::Coilback);
        let walker = game.players()[0];
        lay_at(&mut game, 1, walker.footing.x + 40);
        let effects = game
            .apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 60,
            })
            .unwrap();
        let Some(&Effect::Walked { steps, .. }) = effects.iter().find(|e| matches!(e, Effect::Walked { .. })) else {
            panic!()
        };
        assert!(steps < 60, "it stopped on the mine: {effects:?}");
        assert!(damage_to(&effects, 0) > 0, "{effects:?}");
        assert_eq!(game.mines(), []);
        assert_eq!(game.phase(), Phase::Turn { seat: 0 }, "a walker that lives walks on");
    }

    #[test]
    fn a_shell_landing_on_a_mobile_lays_a_mine_that_goes_off_at_once() {
        let mut game = duel(MobileKind::Coilback);
        let under = game.players()[1].footing.x;
        let effects = lay_at(&mut game, 0, under);
        assert!(matches!(effects[..], [Effect::MineLaid { .. }, Effect::MineGone { tick: Some(0), .. }, ..]), "{effects:?}");
        assert!((140..=180).contains(&damage_to(&effects, 1)), "{effects:?}");
    }

    #[test]
    fn a_replayed_log_lays_and_moves_the_same_mines() {
        let mut game = duel(MobileKind::Coilback);
        let setup = crate::game::testing::duel_setup(800);
        let mut setup = setup;
        setup.seats[0].mobile = MobileKind::Tinbadger;
        let mut fresh = Game::new(&setup, &NoAssets).unwrap();
        let mut events: Vec<Event> = Vec::new();
        // The opening forecasts, as `start` answered them.
        while let Some(roll) = fresh.pending_roll() {
            let event = match roll {
                crate::game::Roll::Weather { .. } => Event::Weather(crate::weather::Forecast::new(crate::weather::WeatherKind::Force, 1)),
                crate::game::Roll::Wind => Event::Wind(crate::units::Wind::default()),
                crate::game::Roll::Drop { .. } => unreachable!(),
            };
            fresh.apply(event).unwrap();
            events.push(event);
        }
        assert_eq!(fresh.state_hash().unwrap(), game.state_hash().unwrap());
        let shot = Event::Shoot {
            seat: 0,
            shot: crate::game::Shot::S2,
            aim: 45,
            power: Power::nearest(1.6),
            time: Centis(0),
        };
        shoot(&mut game, 0, crate::game::Shot::S2, 45, Power::nearest(1.6));
        events.push(shot);
        assert_eq!(game.mines().len(), 2, "two shells, two mines");
        let replayed = Log { setup, events }.replay(&NoAssets).unwrap();
        assert_eq!(replayed.mines(), game.mines());
        assert_eq!(replayed.state_hash().unwrap(), game.state_hash().unwrap());
    }

    #[test]
    fn every_mobile_so_far_is_sensed() {
        assert!(MobileKind::ALL.into_iter().all(senses));
    }
}
