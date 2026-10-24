//! Mines: things a shot leaves on the map that outlive its turn (Tinbadger's tin beetles, GunBound
//! Raon's S2). A mine sits on the ground where its shell landed, part of the game state like a
//! mobile, so the state hash covers it, a replayed log rebuilds it, and a client that joins late
//! sees it.
//!
//! As its layer's turn comes up, each of its mines with a mobile in sense (whoever's: the layer's
//! own side too) crawls toward the nearest, a walk at a time (`shots::trundle`), and goes off when
//! it reaches that mobile's middle. A mobile that walks onto one sets it off; any blast, beam or
//! bolt in reach sets it off (`set_off`), and its blast may set off the next; a tornado takes it.
//! A mine whose ground is blown away falls, as a mobile does, and one that leaves the map, or whose
//! layer dies, is gone. A mobile that floats passes over them all (`senses`). The rules' numbers
//! are in `tuning::tinbadger`, and their sources in `docs/research/tinbadger.md`.
//!
//! Everything a mine does reaches the client as effects naming it by `MineId`: laid, crawled (a
//! stage of its own, before its layer's turn starts), fallen and gone.

use serde::{Deserialize, Serialize};

use crate::{
    damage::Blast,
    game::{Contact, Effect, Game, Leg, MobileKind},
    movement::{self, Footing, Landing},
    shots::trundle::{self, Gait, Heading, Start},
    tuning::{
        ArmorClass,
        tinbadger::{BIONIC_PULL, MECHANICAL_PULL, MINE_BLAST, MINE_CAP, MINE_CRAWL_SPEED, MINE_CRAWL_STEPS, MINE_DAMAGE, MINE_HEIGHT, MINE_RADIUS, MINE_SENSE, MOBILE, SWEET_SPOT},
    },
    units::{Facing, Vec2},
    weather::WeatherKind,
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

/// Whether a mine senses `mobile` at all: crawls toward it, and goes off on it. One that floats
/// passes over them: Dragon "can fly over raon mines" (SW-mob, WP06, CR-blood).
#[must_use]
pub fn senses(mobile: MobileKind) -> bool {
    !mobile.floats()
}

/// How far from its hitbox a mine senses a mobile of `armor`, px: farthest a Mechanical one, least
/// a Bionic one (SW-mob, GF-lee, GF-max, NAMU).
fn sense_range(armor: ArmorClass) -> f64 {
    MINE_SENSE.value
        * match armor {
            ArmorClass::Mechanical => MECHANICAL_PULL.value,
            ArmorClass::Shield => 1.0,
            ArmorClass::Bionic => BIONIC_PULL.value,
        }
}

/// The living, sensed mobile whose sweet spot `at` is at: inside its hitbox, within `SWEET_SPOT` of
/// its middle column, where a mine goes off on it (CR-blood, GF-vgg, GF-max).
pub(crate) fn sweet_spot(game: &Game, at: Vec2) -> Option<u8> {
    (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive && senses(player.mobile))
        .find(|(_, player)| {
            let hitbox = player.tuning().hitbox.value;
            let body = player.pose().to_body(at);
            player.pose().hit(hitbox, at) && (body.x - (hitbox.x + hitbox.w / 2.0)).abs() <= SWEET_SPOT.value
        })
        .map(|(seat, _)| seat)
}

/// The mobile `mine` crawls toward: the nearest one in sense, measured from its hitbox and scaled by
/// its armor's pull (`sense_range`). Any mobile, the layer's own side and the layer itself too: the
/// beetles can't tell friend from foe (SNX-raon, WP06, NAMU, GF-vgg; SW-mob says enemy,
/// CONTRADICTION).
#[must_use]
pub fn quarry(game: &Game, mine: &Mine) -> Option<u8> {
    let at = mine.middle();
    (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive && senses(player.mobile))
        .map(|(seat, player)| (player.pose().gap(player.tuning().hitbox.value, at) / sense_range(player.tuning().armor.value), seat))
        .filter(|&(near, _)| near <= 1.0)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, seat)| seat)
}

/// Whether `mine` has a mobile in sense: it would crawl as its layer's turn comes up ("its antenna
/// will turn red", SW-mob).
#[must_use]
pub fn armed(game: &Game, mine: &Mine) -> bool {
    quarry(game, mine).is_some()
}

/// The tornado column `at` stands in, by index, if any: a tornado takes a mine in it, "whether they
/// crawl into it or not" (CR-blood, WP-early, NAMU).
fn tornado(game: &Game, at: Vec2) -> Option<u8> {
    let index = game.weather().columns.iter().position(|column| column.acts_as() == WeatherKind::Tornado && column.contains(at.x))?;
    u8::try_from(index).ok()
}

/// `seat` lays a mine where its shell came down at `at` (a point just above the ground it struck),
/// at stage tick `tick`, facing `facing`. It settles onto the ground below; with none, it is lost
/// off the map. A layer keeps at most `MINE_CAP` on the map: its oldest goes to make room. One that
/// lands at a mobile's sweet spot goes off at once, and one in a tornado is taken.
pub(crate) fn lay(game: &mut Game, seat: u8, at: Vec2, facing: Facing, tick: u32, effects: &mut Vec<Effect>) {
    // A landing is a pixel or so from the ground, inside the map: in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let open = Footing {
        x: at.x.floor() as i32,
        y: at.y.floor() as i32,
    };
    let Landing::On(footing) = movement::settle(game.terrain(), open) else { return };
    let field = &mut game.mines;
    if field.mines.iter().filter(|mine| mine.seat == seat).count() >= MINE_CAP.value
        && let Some(oldest) = field.mines.iter().find(|mine| mine.seat == seat).map(|mine| mine.id)
    {
        field.take(oldest);
        effects.push(Effect::MineGone { mine: oldest, tick: Some(tick) });
    }
    let mine = Mine {
        id: MineId(field.laid),
        seat,
        at: footing,
        facing,
    };
    field.laid += 1;
    field.mines.push(mine);
    effects.push(Effect::MineLaid { mine, tick });
    if tornado(game, mine.middle()).is_some() {
        game.mines.take(mine.id);
        effects.push(Effect::MineGone { mine: mine.id, tick: Some(tick) });
        return;
    }
    trip(game, tick, effects);
}

/// Whether `seat`'s mobile stands with its sweet spot on a mine.
pub(crate) fn under(game: &Game, seat: u8) -> bool {
    game.mines.mines.iter().any(|mine| sweet_spot(game, mine.middle()) == Some(seat))
}

/// Sets off every mine at a mobile's sweet spot, at stage tick `tick`: "Running over a Raon explodes
/// them immediately" (GF-lee). Returns whether any went.
pub(crate) fn trip(game: &mut Game, tick: u32, effects: &mut Vec<Effect>) -> bool {
    let mut tripped = false;
    while let Some((mine, seat)) = game.mines.mines.iter().find_map(|mine| sweet_spot(game, mine.middle()).map(|seat| (*mine, seat))) {
        blow(game, mine, Some(seat), tick, effects);
        tripped = true;
    }
    tripped
}

/// Sets off every mine within `reach` of `at`, at stage tick `tick`: each goes off where it is, and
/// its own blast may set off the next. Shots set them off, the beetles' own and anyone's, beams and
/// bolts too (GF-tiro, CR-icy, CR-tooya, GF-vgg, OB); GF-lee has them only "eliminate[d]". Every
/// blast calls it (`Game::explode_by`, so Skyeye's beams and the satellites' lasers too), and every
/// bolt (`shots::bolt::flash`). A shot that runs into a mine calls it where it meets the mine, with
/// `MINE_RADIUS` for its reach (Aduka's SS and Nak's SS hit them, WP06: Flarecrow's calls a beam
/// onto each it passes, Augerdillo's goes off on the first, through `crossed`). J.D.'s S2 pulls
/// them instead (`drag`).
pub(crate) fn set_off(game: &mut Game, at: Vec2, reach: f64, tick: u32, effects: &mut Vec<Effect>) {
    while let Some(mine) = game.mines.mines.iter().find(|mine| mine.middle().distance(at) <= reach).copied() {
        blow(game, mine, None, tick, effects);
    }
}

/// Drags every mine within `reach` of `at` to the ground under `at`, at stage tick `tick`: J.D.'s S2
/// "sucks in surrounding mobiles, jewels and also mini-Raons" (WP06). One dragged under a mobile's
/// middle goes off there; one with no ground under `at` is gone.
// Lodestoad's S2 (J.D.'s) calls it once it lands; until then only the tests do.
#[cfg_attr(not(test), expect(dead_code, reason = "Lodestoad's S2 pull calls it once it merges"))]
pub(crate) fn drag(game: &mut Game, at: Vec2, reach: f64, tick: u32, effects: &mut Vec<Effect>) {
    // The pull's center is on the map, in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let under = Footing {
        x: at.x.floor() as i32,
        y: at.y.floor() as i32,
    };
    let landing = movement::settle(game.terrain(), under);
    let pulled: Vec<Mine> = game.mines.mines.iter().filter(|mine| mine.middle().distance(at) <= reach).copied().collect();
    for mine in pulled {
        match landing {
            Landing::On(to) => {
                game.mines.put(Mine { at: to, ..mine });
                effects.push(Effect::MineFell { mine: mine.id, to });
            }
            Landing::OffMap => {
                game.mines.take(mine.id);
                effects.push(Effect::MineGone { mine: mine.id, tick: Some(tick) });
            }
        }
    }
    trip(game, tick, effects);
}

/// Takes `mine` off the map and blows it up, at its middle, direct on the mobile at whose sweet spot
/// it went off.
fn blow(game: &mut Game, mine: Mine, direct: Option<u8>, tick: u32, effects: &mut Vec<Effect>) {
    if game.mines.take(mine.id).is_none() {
        return;
    }
    effects.push(Effect::MineGone { mine: mine.id, tick: Some(tick) });
    let blast = Blast {
        center: mine.middle(),
        radius: MINE_BLAST.value,
        damage: MINE_DAMAGE.value,
        weapon: MOBILE.weapon.value,
        false_angle: false,
        boost: 1.0,
    };
    game.explode(&blast, MINE_BLAST.value, direct, tick, effects);
}

/// The mines a path passes over, each with the stage tick it first comes within `MINE_RADIUS` of the
/// mine's middle, in that order: for a shot that meets mines on its way (Aduka's SS).
pub(crate) fn crossed(game: &Game, legs: &[Leg]) -> Vec<(Mine, u32)> {
    let mut met: Vec<(Mine, u32)> = Vec::new();
    for leg in legs {
        for tick in leg.from..=leg.to {
            let at = leg.flight.at(tick);
            for mine in &game.mines.mines {
                if mine.middle().distance(at) <= MINE_RADIUS.value && !met.iter().any(|(seen, _)| seen.id == mine.id) {
                    met.push((*mine, leg.offset + tick));
                }
            }
        }
    }
    met.sort_by_key(|&(mine, tick)| (tick, mine.id));
    met
}

/// Takes every mine standing in a tornado: as the weather changes, the columns that come up take
/// the mines under them.
pub(crate) fn sweep(game: &mut Game, effects: &mut Vec<Effect>) {
    let (gone, kept): (Vec<Mine>, Vec<Mine>) = game.mines.mines.iter().partition(|mine| tornado(game, mine.middle()).is_some());
    game.mines.mines = kept;
    effects.extend(gone.into_iter().map(|mine| Effect::MineGone { mine: mine.id, tick: None }));
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

/// As `seat`'s turn comes up, each of its mines with a mobile in sense crawls toward the middle of
/// its quarry, one after another, each walk a stage of its own (`Effect::MineCrawled`): up to
/// `MINE_CRAWL_STEPS`, stopping at a wall or once there. It goes off at the first mobile's sweet
/// spot it reaches, and a tornado on its way takes it. A mine already at a sweet spot goes off at
/// once.
pub(crate) fn crawl(game: &mut Game, seat: u8, effects: &mut Vec<Effect>) {
    let ids: Vec<MineId> = game.mines.mines.iter().filter(|mine| mine.seat == seat).map(|mine| mine.id).collect();
    for id in ids {
        if game.decided() {
            return;
        }
        // An earlier mine's blast may have set this one off, or dropped it.
        let Some(mine) = game.mines.get(id) else { continue };
        if let Some(hit) = sweet_spot(game, mine.middle()) {
            blow(game, mine, Some(hit), 0, effects);
            game.settle_all(effects);
            continue;
        }
        let Some(target) = quarry(game, &mine) else { continue };
        // The quarry's middle column: its center, a world point well inside the map.
        #[allow(clippy::cast_possible_truncation)]
        let to = game.players()[usize::from(target)].center().x.floor() as i32;
        let start = Start {
            at: mine.at,
            facing: mine.facing,
            tick: 0,
        };
        let stop = |at: Vec2| sweet_spot(game, at).map(Contact::Mobile).or_else(|| tornado(game, at).map(Contact::Column));
        let walked = trundle::walk(game.terrain(), start, &GAIT, Heading::Toward(to), MINE_CRAWL_STEPS.value, stop);
        effects.push(Effect::MineCrawled { seat, mine: id, legs: walked.legs });
        match walked.end.contact {
            Contact::Gone | Contact::Column(_) => {
                game.mines.take(id);
                effects.push(Effect::MineGone { mine: id, tick: Some(walked.end.tick) });
            }
            Contact::Mobile(hit) => blow(game, Mine { at: walked.footing, ..mine }, Some(hit), walked.end.tick, effects),
            Contact::Terrain => game.mines.put(Mine {
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
            Event, Phase, Shot, Team,
            testing::{damage_to, duel_of, game_of, pass, seat, shoot},
        },
        log::Log,
        map::NoAssets,
        shots::bolt,
        units::{Centis, Power},
        weather::Column,
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

    fn gone(effects: &[Effect]) -> Vec<MineId> {
        effects.iter().filter_map(|e| if let Effect::MineGone { mine, .. } = e { Some(*mine) } else { None }).collect()
    }

    /// The column of `seat`'s middle.
    fn middle_column(game: &Game, seat: usize) -> i32 {
        #[allow(clippy::cast_possible_truncation)]
        let x = game.players()[seat].center().x.floor() as i32;
        x
    }

    /// The nearest column whose mine sits more than `gap` px from `seat`'s hitbox, on the side
    /// toward x = 0.
    fn column_short_of(game: &Game, seat: usize, gap: f64) -> i32 {
        let player = game.players()[seat];
        (0..middle_column(game, seat))
            .rev()
            .find(|&x| player.pose().gap(player.tuning().hitbox.value, Mine::middle_at(Footing { x, y: 500 })) > gap)
            .unwrap()
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
        assert_eq!(game.mines(), [laid], "far from any mobile, it waits");
    }

    #[test]
    fn as_its_layers_turn_comes_up_a_mine_crawls_to_the_middle_of_a_mobile_in_sense_and_goes_off() {
        let mut game = duel(MobileKind::Coilback);
        let start = column_short_of(&game, 1, 60.0);
        lay_at(&mut game, 0, start);
        let effects = pass(&mut game, 0);
        assert_eq!(crawls(&effects), [], "not on the other side's turn: {effects:?}");
        assert_eq!(game.mines().len(), 1);
        let target = game.players()[1];
        let effects = pass(&mut game, 1);
        assert_eq!(crawls(&effects), [MineId(0)], "{effects:?}");
        let gone = effects.iter().position(|e| matches!(e, Effect::MineGone { mine: MineId(0), tick: Some(_) })).unwrap();
        let crawled = effects.iter().position(|e| matches!(e, Effect::MineCrawled { .. })).unwrap();
        let turn = effects.iter().position(|e| matches!(e, Effect::TurnStarted { seat: 0 })).unwrap();
        assert!(crawled < gone && gone < turn, "it crawls, goes off, then the turn starts: {effects:?}");
        // At the sweet spot, near the middle of its body: a direct hit, 140-180, Laser against
        // Mechanical.
        let Some(&Effect::Exploded { at, tick, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!()
        };
        let hitbox = target.tuning().hitbox.value;
        assert!((target.pose().to_body(at).x - (hitbox.x + hitbox.w / 2.0)).abs() <= SWEET_SPOT.value, "{at:?}");
        assert!((134..=180).contains(&damage_to(&effects, 1)), "{effects:?}");
        // At walking pace, a column a step.
        #[allow(clippy::cast_possible_truncation)]
        let columns = f64::from(at.x.floor() as i32 - start);
        let walked = f64::from(tick) * crate::units::TICK * MINE_CRAWL_SPEED.value;
        assert!((walked - columns).abs() <= 1.0, "{walked} steps for {columns} columns");
        assert_eq!(game.mines(), []);
    }

    #[test]
    fn a_mine_with_no_one_in_sense_stays_put_and_one_crawls_at_most_its_steps() {
        let mut game = duel(MobileKind::Coilback);
        let reach = MINE_SENSE.value * MECHANICAL_PULL.value;
        // Out of sense, and in sense but farther than one crawl reaches.
        let far = column_short_of(&game, 1, reach + 20.0);
        let near = column_short_of(&game, 1, reach - 10.0);
        lay_at(&mut game, 0, far);
        lay_at(&mut game, 0, near);
        let effects = round(&mut game);
        assert_eq!(crawls(&effects), [MineId(1)], "{effects:?}");
        assert_eq!(game.mines()[1].at.x, near + i32::try_from(MINE_CRAWL_STEPS.value).unwrap());
        assert_eq!(game.mines()[0].at.x, far, "the far one never moved");
        assert_eq!(damage_to(&effects, 1), 0);
    }

    #[test]
    fn a_mechanical_mobile_draws_a_mine_from_farther_than_a_bionic_one() {
        // A Mechanical Coilback, and a mine just within a crawl of its middle; then a Bionic
        // Marblemole on the mine's other side, nearer it, but not so near as to outweigh its armor.
        let coilback = |marblemole: i32| {
            game_of(vec![
                seat(Team::A, MobileKind::Tinbadger, 300, Facing::Right),
                seat(Team::B, MobileKind::Marblemole, marblemole, Facing::Right),
                seat(Team::B, MobileKind::Coilback, 1100, Facing::Left),
            ])
        };
        let probe = coilback(700);
        let mine = column_short_of(&probe, 2, 45.0);
        let gap = |game: &Game, seat: usize| {
            let player = game.players()[seat];
            player.pose().gap(player.tuning().hitbox.value, Mine::middle_at(Footing { x: mine, y: 500 }))
        };
        let mut game = (600..mine).rev().map(coilback).find(|game| (25.0..35.0).contains(&gap(game, 1))).expect("a spot for the Marblemole");
        assert!(gap(&game, 1) < gap(&game, 2), "nearer the Bionic one");
        lay_at(&mut game, 0, mine);
        let effects = round(&mut game);
        assert!(damage_to(&effects, 2) > 0 && damage_to(&effects, 1) == 0, "it went for the Coilback: {effects:?}");
        // The same gap from a Bionic mobile alone is out of sense.
        let mut lone = duel(MobileKind::Marblemole);
        let out = column_short_of(&lone, 1, MINE_SENSE.value * BIONIC_PULL.value + 5.0);
        lay_at(&mut lone, 0, out);
        assert_eq!(crawls(&round(&mut lone)), [], "Bionics draw them least");
    }

    #[test]
    fn a_mine_cant_tell_friend_from_foe_its_own_layer_included() {
        let mut game = duel(MobileKind::Coilback);
        // Just ahead of Tinbadger, far from the Coilback.
        let ahead = middle_column(&game, 0) + 60;
        lay_at(&mut game, 0, ahead);
        pass(&mut game, 0);
        let effects = pass(&mut game, 1);
        assert_eq!(crawls(&effects), [MineId(0)], "{effects:?}");
        assert!(damage_to(&effects, 0) > 0, "into its own layer: {effects:?}");
    }

    #[test]
    fn a_floating_mobile_passes_over_mines() {
        assert!(!senses(MobileKind::Kitewyrm));
        assert!(MobileKind::ALL.into_iter().filter(|mobile| !mobile.floats()).all(senses));
        let mut game = duel(MobileKind::Kitewyrm);
        let near = column_short_of(&game, 1, 20.0);
        lay_at(&mut game, 0, near);
        assert_eq!(crawls(&round(&mut game)), [], "nothing in sense");
        // Under it, as it walks over: nothing.
        let under = middle_column(&game, 1);
        assert_eq!(gone(&lay_at(&mut game, 0, under)), [], "not even at its middle");
        pass(&mut game, 0);
        let effects = game.apply(Event::Walk { seat: 1, facing: Facing::Left, steps: 60 }).unwrap();
        assert_eq!(gone(&effects), [], "{effects:?}");
        assert_eq!(game.mines().len(), 2);
    }

    #[test]
    fn a_blast_or_bolt_in_reach_sets_a_mine_off_and_its_blast_the_next() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 0, 700);
        lay_at(&mut game, 0, 712);
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
        assert_eq!(gone(&effects), [MineId(0), MineId(1)], "a chain: {effects:?}");
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Exploded { radius, .. } if *radius == MINE_BLAST.value)).count(), 2);
        assert!(effects.iter().all(|e| !matches!(e, Effect::MineGone { tick, .. } if *tick != Some(50))));
        let strike = bolt::from_sky(&game, 900.5);
        bolt::flash(&mut game, &strike, 12.0, 70, &mut effects);
        assert_eq!(game.mines(), [], "the bolt set off the third");
    }

    #[test]
    fn skyeyes_beam_and_a_satellites_lasers_and_pins_set_off_a_mine_they_strike_by() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 1, 650);
        let mine = game.mines()[0];
        let mut effects = Vec::new();
        crate::shots::skyeye::beam(&mut game, mine.middle(), 10, &mut effects);
        assert!(gone(&effects).contains(&mine.id), "Skyeye: {effects:?}");
        for mobile in [MobileKind::Lensowl, MobileKind::Needlemouse] {
            let mut game = duel_of(mobile, MobileKind::Coilback, 800);
            lay_at(&mut game, 1, 650);
            let mine = game.mines()[0];
            let range = Shot::S1.tuning(mobile.tuning()).aim.value;
            let miss = |aim: i16, power: Power| game.track(game.launch(0, Shot::S1, aim, power.bars()).unwrap(), 0, 0).impact.at.distance(mine.middle());
            let (aim, power) = (range.min..=range.max)
                .flat_map(|aim| (0..=Power::MAX.twentieths()).map(move |t| (aim, Power::from_twentieths(t).unwrap())))
                .min_by(|a, b| miss(a.0, a.1).total_cmp(&miss(b.0, b.1)))
                .unwrap();
            assert!(miss(aim, power) < 6.0, "{mobile:?}'s tracer comes down by the mine");
            let effects = shoot(&mut game, 0, Shot::S1, aim, power);
            assert!(gone(&effects).contains(&mine.id), "{mobile:?}: {effects:?}");
            assert_eq!(game.mines(), [], "{mobile:?}");
        }
    }

    #[test]
    fn a_pull_drags_mines_in_reach_to_its_center_and_one_under_a_mobiles_middle_goes_off() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 1, 600);
        lay_at(&mut game, 1, 640);
        lay_at(&mut game, 1, 900);
        let mut effects = Vec::new();
        drag(&mut game, Vec2::new(620.5, 480.0), 40.0, 30, &mut effects);
        assert_eq!(
            effects,
            [
                Effect::MineFell {
                    mine: MineId(0),
                    to: Footing { x: 620, y: 500 }
                },
                Effect::MineFell {
                    mine: MineId(1),
                    to: Footing { x: 620, y: 500 }
                }
            ]
        );
        assert_eq!(game.mines()[2].at.x, 900, "out of reach");
        let middle = game.players()[1].center();
        let mut effects = Vec::new();
        drag(&mut game, Vec2::new(middle.x, 480.0), 600.0, 30, &mut effects);
        assert_eq!(game.mines(), [], "dragged under the Coilback's middle, they all went off");
        assert!(damage_to(&effects, 1) > 0, "{effects:?}");
    }

    #[test]
    fn a_tornado_takes_a_mine_under_it_or_laid_in_it() {
        let mut game = duel(MobileKind::Coilback);
        lay_at(&mut game, 0, 600);
        lay_at(&mut game, 0, 1000);
        game.set_columns(vec![Column::new(WeatherKind::Tornado, 600)]);
        let mut effects = Vec::new();
        sweep(&mut game, &mut effects);
        assert_eq!(effects, [Effect::MineGone { mine: MineId(0), tick: None }]);
        let effects = lay_at(&mut game, 0, 605);
        assert_eq!(gone(&effects), [MineId(2)], "{effects:?}");
        assert_eq!(game.mines().iter().map(|mine| mine.id).collect::<Vec<_>>(), [MineId(1)]);
    }

    #[test]
    fn a_layer_keeps_eight_mines_its_oldest_making_room() {
        let mut game = duel(MobileKind::Coilback);
        for x in 0..8 {
            lay_at(&mut game, 0, 500 + 20 * x);
        }
        assert_eq!(game.mines().len(), MINE_CAP.value);
        let effects = lay_at(&mut game, 0, 700);
        assert_eq!(gone(&effects), [MineId(0)], "{effects:?}");
        assert_eq!(game.mines().len(), MINE_CAP.value);
        lay_at(&mut game, 1, 720);
        assert_eq!(game.mines().len(), MINE_CAP.value + 1, "the other side's are its own");
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
        let ahead = middle_column(&game, 0) + 40;
        lay_at(&mut game, 1, ahead);
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
    fn a_mine_landing_at_a_mobiles_middle_goes_off_at_once_and_one_at_its_edge_waits() {
        let mut game = duel(MobileKind::Coilback);
        let edge = column_short_of(&game, 1, 0.0) + 3;
        assert!(game.players()[1].pose().hit(game.players()[1].tuning().hitbox.value, Mine::middle_at(Footing { x: edge, y: 500 })));
        assert_eq!(gone(&lay_at(&mut game, 0, edge)), [], "inside its hitbox, off its middle");
        let middle = middle_column(&game, 1);
        let effects = lay_at(&mut game, 0, middle);
        assert!(matches!(effects[..], [Effect::MineLaid { .. }, Effect::MineGone { tick: Some(0), .. }, ..]), "{effects:?}");
        assert!((134..=180).contains(&damage_to(&effects, 1)), "{effects:?}");
    }

    #[test]
    fn a_replayed_log_lays_and_moves_the_same_mines() {
        let mut game = duel(MobileKind::Coilback);
        let mut setup = crate::game::testing::duel_setup(800);
        setup.seats[0].mobile = MobileKind::Tinbadger;
        let mut fresh = Game::new(&setup, &NoAssets).unwrap();
        let mut events: Vec<Event> = Vec::new();
        // The opening forecasts, as `start` answered them.
        while let Some(roll) = fresh.pending_roll() {
            let event = match roll {
                crate::game::Roll::Weather { .. } => Event::Weather(crate::weather::Forecast::new(WeatherKind::Force, 1)),
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
}
