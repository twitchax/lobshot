//! SearchBot: picks a shot by searching aim, power, shot and item in the sim, the way a player
//! who has done their homework would. It sees only what a player sees (the game state, including
//! wind and the weather on the map): nothing the server hasn't rolled yet, nor what a Random
//! column hides. It plays every
//! mobile, since every mobile's shots are the sim's.
//!
//! Search, in three passes:
//!
//! 1. **Aim.** Shots that leave the same barrel leave it on the same flight, so one coarse grid
//!    per barrel (every 3 degrees, every 2 power steps; most mobiles have one) over the union of its
//!    allowed shots' aim ranges scores where that flight lands: a direct hit on the nearest enemy
//!    best (a false-angle hit a little less so), then distance from its center, and anything that
//!    would hit a teammate worst. The whole range, not just the true angles: from a slope or behind
//!    a hill only a false angle may clear the ground, and 70% damage beats none. A shot that does
//!    its work somewhere else (a walking sponge, `shots::aim_point`) is scored by where that is.
//! 2. **Shot.** For each shot it may fire (never a locked or disabled SS), the two best distinct
//!    trajectories in its own range, refined, are fired in a scratch game and valued by what they
//!    did: damage to enemies, kills, and twice the cost of any harm to its own side.
//! 3. **Item.** A heal first when it is low; otherwise Dual (on S1 or S2) or Power Up when the
//!    chosen shot is strong and the item makes it better.
//!
//! It searches from where it stands. When its best shot from there is weak (behind a wall, say),
//! it runs the first two passes again from a few spots up to its move budget each way, and walks
//! to the spot with the best shot.
//!
//! Difficulty is error around the search (`tuning::bot`): the bot misreads the wind before it
//! searches, then misses the aim and power it found. Both come from a seed the server rolls, so
//! a bot's turn is still a pure function of its inputs.

use serde::{Deserialize, Serialize};

use crate::{
    game::{Contact, Death, Effect, Event, Game, Impact, Phase, Player, Shot},
    items::{Item, Loadout},
    mines,
    referee::XorShift,
    shots,
    tuning::{self, AimRange, Barrel, bot::Noise},
    units::{Centis, Facing, Power, Vec2, Wind},
};

/// The loadout the server gives bots: the items SearchBot knows how to use, filling all six slots.
pub const LOADOUT: Loadout = Loadout::fixed([Item::Dual, Item::PowerUp, Item::Heal, Item::BigHeal]);

/// At or below this share of its max HP, a bot heals before it shoots.
const LOW_HP: f64 = 0.4;

/// A shot worth at least this much (about GunBound's lowest damage tier) is strong: it earns Dual
/// or Power Up, and the bot takes it from where it stands rather than look for a better spot.
const STRONG_SHOT: f64 = 150.0;

/// A bot looking for a better spot tries walks of each third of its move budget, each way.
const WALK_PARTS: u16 = 3;

/// What a kill is worth on top of its damage, in damage.
const KILL_VALUE: f64 = 300.0;

/// How well the bot reads the wind and how steady its hands are (`tuning::bot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Someone still learning.
    Easy,
    /// A friend who knows the game: what the server's bots play.
    Normal,
    /// Close to perfect.
    Hard,
}

impl Level {
    #[must_use]
    pub fn noise(self) -> &'static Noise {
        match self {
            Self::Easy => &tuning::bot::EASY,
            Self::Normal => &tuning::bot::NORMAL,
            Self::Hard => &tuning::bot::HARD,
        }
    }
}

/// What the bot does with `seat`'s turn, as events to apply in order (the server stamps the
/// shot's time). `None` when it isn't `seat`'s turn or a roll is owed.
#[must_use]
pub fn plan(game: &Game, seat: u8, level: Level, seed: u64) -> Option<Vec<Event>> {
    if game.phase() != (Phase::Turn { seat }) || game.pending_roll().is_some() {
        return None;
    }
    let me = *game.players().get(usize::from(seat))?;
    let target = *game.players().iter().filter(|p| p.alive && p.team != me.team).min_by_key(|p| (p.footing.x - me.footing.x).abs())?;

    let mut events = Vec::new();
    let mut scratch = game.as_seen();
    if let Some(heal) = heal(&scratch, seat) {
        scratch.apply(heal).ok()?;
        events.push(heal);
    }
    // It picks its spot and its shot, and values them, in the wind as it reads it.
    let noise = level.noise();
    let mut rng = XorShift::new(seed);
    scratch.wind = misread(scratch.wind(), noise, &mut rng);

    let Spot { moves, mut best } = Spot::best(&scratch, seat, &target)?;
    for event in moves {
        scratch.apply(event).ok()?;
        events.push(event);
    }
    if let Some(boosted) = boost(&scratch, seat, &best) {
        best = boosted;
    }
    if let Some(slot) = best.item {
        events.push(Event::UseItem { seat, slot, time: Centis(0) });
    }

    let range = best.shot.tuning(me.tuning()).aim.value;
    let power = i32::from(best.power) + within(&mut rng, noise.power_steps.value);
    let aim = (best.aim + i16::try_from(within(&mut rng, noise.aim_degrees.value)).unwrap_or(0)).clamp(range.min, range.max);
    let power = Power::from_twentieths(u8::try_from(power.clamp(0, i32::from(Power::MAX.twentieths()))).ok()?).ok()?;
    events.push(Event::Shoot {
        seat,
        shot: best.shot,
        aim,
        power,
        time: Centis(0),
    });
    Some(events)
}

/// Where the bot shoots from, and its best shot there.
struct Spot {
    /// The walk there, then the turn toward the target, as events.
    moves: Vec<Event>,
    best: Candidate,
}

impl Spot {
    /// Where it stands, unless its best shot from there is weak; then the best of a few spots up
    /// to its whole move budget each way. Shorter walks come first, so a tie stays closer.
    fn best(game: &Game, seat: u8, target: &Player) -> Option<Self> {
        let me = game.players().get(usize::from(seat))?;
        let (scratch, moves) = walked(game, seat, None, target)?;
        let mut seen = vec![me.footing];
        let mut best = Self::searched(&scratch, moves, seat, target)?;
        if best.best.value >= STRONG_SHOT {
            return Some(best);
        }
        let toward = facing_toward(me, target);
        for part in 1..=WALK_PARTS {
            let steps = me.moves_left * part / WALK_PARTS;
            for facing in [toward, toward.flip()] {
                // A walk that ends where another did (against a wall, say) is that spot again.
                let Some((scratch, moves)) = walked(game, seat, Some((facing, steps)), target) else { continue };
                let footing = scratch.players()[usize::from(seat)].footing;
                if seen.contains(&footing) {
                    continue;
                }
                seen.push(footing);
                if let Some(spot) = Self::searched(&scratch, moves, seat, target)
                    && spot.best.beats(&best.best)
                {
                    best = spot;
                }
            }
        }
        Some(best)
    }

    /// The best shot from where `game` has the bot stand, after `moves`.
    fn searched(game: &Game, moves: Vec<Event>, seat: u8, target: &Player) -> Option<Self> {
        Some(Self {
            moves,
            best: best_shot(game, seat, target)?,
        })
    }
}

/// `game` after `walk`, if any, and turning to shoot at the target, with the moves that did it.
/// `None` when the walk ends the turn (off the map).
fn walked(game: &Game, seat: u8, walk: Option<(Facing, u16)>, target: &Player) -> Option<(Game, Vec<Event>)> {
    let mut scratch = game.clone();
    let mut moves = Vec::new();
    if let Some((facing, steps)) = walk {
        let event = Event::Walk { seat, facing, steps };
        scratch.apply(event).ok()?;
        moves.push(event);
        if scratch.phase() != (Phase::Turn { seat }) {
            return None;
        }
    }
    let me = *scratch.players().get(usize::from(seat))?;
    let facing = facing_to_shoot(&me, target);
    if me.facing != facing {
        let turn = Event::Walk { seat, facing, steps: 0 };
        scratch.apply(turn).ok()?;
        moves.push(turn);
    }
    Some((scratch, moves))
}

fn facing_toward(me: &Player, target: &Player) -> Facing {
    if target.footing.x >= me.footing.x { Facing::Right } else { Facing::Left }
}

/// The way to face to shoot at the target: toward it, or away for a mobile that fires over its
/// back.
fn facing_to_shoot(me: &Player, target: &Player) -> Facing {
    let toward = facing_toward(me, target);
    if me.tuning().s1.aim.value.backward() { toward.flip() } else { toward }
}

/// The best shot `seat` can take from where it stands, as it faces.
fn best_shot(game: &Game, seat: u8, target: &Player) -> Option<Candidate> {
    game.players().get(usize::from(seat))?;
    let search = Search::new(game, seat, target.center());
    let mut best: Option<Candidate> = None;
    for shot in [Shot::S1, Shot::S2, Shot::Ss] {
        if !game.can_shoot(seat, shot) {
            continue;
        }
        for (aim, power) in search.trajectories(shot) {
            let candidate = Candidate::fired(game, seat, None, shot, aim, power, search.proxy(shot, aim, power));
            if best.as_ref().is_none_or(|best| candidate.beats(best)) {
                best = Some(candidate);
            }
        }
    }
    best
}

/// The wind as a bot with `noise` reads it.
fn misread(wind: Wind, noise: &Noise, rng: &mut XorShift) -> Wind {
    let strength = i32::from(wind.strength) + within(rng, noise.wind_strength.value);
    let direction = i32::from(wind.direction) + within(rng, noise.wind_degrees.value);
    Wind {
        strength: u8::try_from(strength.clamp(0, i32::from(u8::MAX))).unwrap_or(0),
        direction: u16::try_from(direction.rem_euclid(360)).unwrap_or(0),
    }
}

/// Where the shared flight lands, as the aim pass sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Landing {
    /// On an enemy, this far from the target's center.
    Hit(f64),
    /// On the ground, this far from it.
    Near(f64),
    /// Off the map, or into the weather and away.
    Lost,
    /// On its own side.
    Own,
}

/// The aim pass: for every aim and power on a coarse grid per barrel, where each of that barrel's
/// shots does its work: where the barrel's flight lands, or somewhere else for a shot that walks on
/// (`shots::aim_point`).
struct Search<'g> {
    game: &'g Game,
    seat: u8,
    target: Vec2,
    grids: Vec<Grid>,
}

/// One barrel's share of the aim pass.
struct Grid {
    /// The allowed shots that leave this barrel, at most three.
    shots: Vec<Shot>,
    /// (aim, power in twentieths, where each of `shots` does its work, in order), over the union of
    /// those shots' ranges.
    cells: Vec<(i16, u8, [Landing; 3])>,
}

impl<'g> Search<'g> {
    fn new(game: &'g Game, seat: u8, target: Vec2) -> Self {
        let mut search = Self { game, seat, target, grids: Vec::new() };
        let tuning = game.players()[usize::from(seat)].tuning();
        let mut barrels: Vec<(Barrel, Vec<Shot>)> = Vec::new();
        for shot in [Shot::S1, Shot::S2, Shot::Ss].into_iter().filter(|&shot| game.can_shoot(seat, shot)) {
            let barrel = tuning.barrel(shot);
            match barrels.iter_mut().find(|(other, _)| *other == barrel) {
                Some((_, shots)) => shots.push(shot),
                None => barrels.push((barrel, vec![shot])),
            }
        }
        for (_, shots) in barrels {
            let ranges: Vec<AimRange> = shots.iter().map(|shot| shot.tuning(tuning).aim.value).collect();
            let (low, high) = (ranges.iter().map(|r| r.min).min().unwrap_or(0), ranges.iter().map(|r| r.max).max().unwrap_or(0));
            let mut cells = Vec::new();
            for aim in (low..=high).step_by(3) {
                for power in (0..=Power::MAX.twentieths()).step_by(2) {
                    cells.push((aim, power, search.landings(&shots, aim, power)));
                }
            }
            search.grids.push(Grid { shots, cells });
        }
        search
    }

    /// Where each of `shots`, all leaving one barrel, does its work from one launch: the shared
    /// flight's landing, unless the shot does it elsewhere.
    fn landings(&self, shots: &[Shot], aim: i16, power: u8) -> [Landing; 3] {
        let mut landings = [Landing::Lost; 3];
        let Some(flight) = shots.first().and_then(|&shot| self.game.launch(self.seat, shot, aim, f64::from(power) / 20.0)) else {
            return landings;
        };
        let shared = self.game.track(flight, 0, 0).impact;
        for (landing, &shot) in landings.iter_mut().zip(shots) {
            *landing = self.classify(shots::aim_point(self.game, self.seat, shot, flight, self.target).unwrap_or(shared));
        }
        landings
    }

    /// Where `shot` at `aim` and `power` does its work.
    fn land(&self, shot: Shot, aim: i16, power: u8) -> Landing {
        let Some(flight) = self.game.launch(self.seat, shot, aim, f64::from(power) / 20.0) else {
            return Landing::Lost;
        };
        self.classify(shots::aim_point(self.game, self.seat, shot, flight, self.target).unwrap_or_else(|| self.game.track(flight, 0, 0).impact))
    }

    fn classify(&self, impact: Impact) -> Landing {
        let me = self.game.players()[usize::from(self.seat)];
        match impact.contact {
            Contact::Gone | Contact::Column(_) => Landing::Lost,
            Contact::Mobile(hit) if self.game.players()[usize::from(hit)].team == me.team => Landing::Own,
            Contact::Mobile(_) => Landing::Hit(impact.at.distance(self.target)),
            Contact::Terrain => Landing::Near(impact.at.distance(self.target)),
        }
    }

    /// A landing's score for `shot` at `aim` (lower is better): a direct hit best, a false-angle
    /// one (70% damage) a little less so, then nearness.
    fn score(range: AimRange, aim: i16, landing: Landing) -> f64 {
        match landing {
            Landing::Hit(distance) if range.is_true(aim) => -1000.0 + distance,
            Landing::Hit(distance) => -700.0 + distance,
            Landing::Near(distance) => distance,
            Landing::Lost => f64::MAX / 2.0,
            Landing::Own => f64::MAX / 4.0,
        }
    }

    /// `shot`'s score at an aim and power off the grid.
    fn proxy(&self, shot: Shot, aim: i16, power: u8) -> f64 {
        let range = shot.tuning(self.game.players()[usize::from(self.seat)].tuning()).aim.value;
        Self::score(range, aim, self.land(shot, aim, power))
    }

    /// `shot`'s two best distinct trajectories within its range, each refined around its grid point.
    fn trajectories(&self, shot: Shot) -> Vec<(i16, u8)> {
        let range = shot.tuning(self.game.players()[usize::from(self.seat)].tuning()).aim.value;
        let Some(grid) = self.grids.iter().find(|grid| grid.shots.contains(&shot)) else {
            return Vec::new();
        };
        let index = grid.shots.iter().position(|&s| s == shot).unwrap_or(0);
        let mut ranked: Vec<(f64, i16, u8)> = grid
            .cells
            .iter()
            .filter(|(aim, ..)| range.contains(*aim))
            .map(|&(aim, power, landings)| (Self::score(range, aim, landings[index]), aim, power))
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(_, first, _)) = ranked.first() else { return Vec::new() };
        // A second trajectory well away from the first: a lob against a line drive, say.
        let second = ranked.iter().find(|(_, aim, _)| (aim - first).abs() >= 9);
        ranked.first().into_iter().chain(second).map(|&(_, aim, power)| self.refine(shot, range, aim, power)).collect()
    }

    /// The best aim and power for `shot` within two degrees and three power steps of a grid point.
    fn refine(&self, shot: Shot, range: AimRange, aim: i16, power: u8) -> (i16, u8) {
        let max = Power::MAX.twentieths();
        ((aim - 2).max(range.min)..=(aim + 2).min(range.max))
            .flat_map(|aim| (power.saturating_sub(3)..=(power + 3).min(max)).map(move |power| (aim, power)))
            .map(|(aim, power)| (Self::score(range, aim, self.land(shot, aim, power)), aim, power))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or((aim, power), |(_, aim, power)| (aim, power))
    }
}

/// A shot the bot might take, and what it did in a scratch game.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Candidate {
    item: Option<u8>,
    shot: Shot,
    aim: i16,
    power: u8,
    value: f64,
    /// The aim pass's score, to break ties between shots that did nothing.
    proxy: f64,
}

impl Candidate {
    /// Fires `shot` (after the item at `item`, if any) in a scratch copy of `game` and values it.
    fn fired(game: &Game, seat: u8, item: Option<u8>, shot: Shot, aim: i16, power: u8, proxy: f64) -> Self {
        let mut scratch = game.clone();
        let used = item.is_none_or(|slot| scratch.apply(Event::UseItem { seat, slot, time: Centis(0) }).is_ok());
        let effects = Power::from_twentieths(power)
            .ok()
            .filter(|_| used)
            .and_then(|power| scratch.apply(Event::Shoot { seat, shot, aim, power, time: Centis(0) }).ok());
        let value = effects.map_or(f64::MIN, |effects| value(game, seat, &effects));
        Self { item, shot, aim, power, value, proxy }
    }

    /// More value, then a better landing. Shots are tried cheapest first, so a tie keeps the
    /// cheaper.
    fn beats(&self, other: &Self) -> bool {
        (self.value, -self.proxy).partial_cmp(&(other.value, -other.proxy)).is_some_and(std::cmp::Ordering::is_gt)
    }
}

/// What a shot's effects are worth to `seat`: damage to enemies, a bonus per kill, twice the cost
/// of any harm to its own side, itself included, and what each mine it lays that will crawl at an
/// enemy may do later (`MINE_WORTH`).
fn value(game: &Game, seat: u8, effects: &[Effect]) -> f64 {
    let team = |s: u8| game.players().get(usize::from(s)).map(|p| p.team);
    let mine = team(seat);
    let lasts = |id| !effects.iter().any(|effect| matches!(*effect, Effect::MineGone { mine, .. } if mine == id));
    effects
        .iter()
        .map(|effect| match *effect {
            // A mine crawls at the nearest mobile in sense, whoever's: it is worth something only
            // when that is an enemy.
            Effect::MineLaid { mine: laid, .. } if lasts(laid.id) && mines::quarry(game, &laid).is_some_and(|quarry| team(quarry) != mine) => tuning::bot::MINE_WORTH.value,
            Effect::Damaged { seat: hit, amount } | Effect::Shielded { seat: hit, amount } => {
                let amount = f64::from(amount);
                if team(hit) == mine { -2.0 * amount } else { amount }
            }
            Effect::Died {
                seat: hit,
                death: Death::Blasted | Death::Bunged,
            } => {
                if team(hit) == mine {
                    -2.0 * KILL_VALUE
                } else {
                    KILL_VALUE
                }
            }
            _ => 0.0,
        })
        .sum()
}

/// The heal `seat` uses first when it is low: Big Heal if it carries one, else Heal.
fn heal(game: &Game, seat: u8) -> Option<Event> {
    let me = game.players().get(usize::from(seat))?;
    if f64::from(me.hp) > f64::from(me.tuning().hp.value) * LOW_HP {
        return None;
    }
    [Item::BigHeal, Item::Heal]
        .into_iter()
        .find_map(|wanted| slot_of(game, me, seat, wanted))
        .map(|slot| Event::UseItem { seat, slot, time: Centis(0) })
}

/// Dual (for S1 or S2) or Power Up on a strong shot, if the bot carries one and it makes the shot
/// better.
fn boost(game: &Game, seat: u8, best: &Candidate) -> Option<Candidate> {
    if best.value < STRONG_SHOT || best.item.is_some() {
        return None;
    }
    let me = game.players().get(usize::from(seat))?;
    let wanted: &[Item] = if best.shot == Shot::Ss { &[Item::PowerUp] } else { &[Item::Dual, Item::PowerUp] };
    let slot = wanted.iter().find_map(|&item| slot_of(game, me, seat, item))?;
    let boosted = Candidate::fired(game, seat, Some(slot), best.shot, best.aim, best.power, best.proxy);
    (boosted.value > best.value).then_some(boosted)
}

/// Where `seat` carries `item`, if it may use it now.
fn slot_of(game: &Game, me: &Player, seat: u8, item: Item) -> Option<u8> {
    (0_u8..)
        .zip(me.items)
        .find(|&(slot, carried)| carried == Some(item) && game.can_use_item(seat, slot))
        .map(|(slot, _)| slot)
}

/// Uniform in `-spread..=spread`.
fn within(rng: &mut XorShift, spread: i32) -> i32 {
    if spread == 0 {
        return 0;
    }
    let span = u64::try_from(2 * spread + 1).unwrap_or(1);
    i32::try_from(rng.next_u64() % span).unwrap_or(0) - spread
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Effect, MobileKind, Seat, Setup, SuddenDeath, SuddenDeathKind, Team,
            testing::{answer_rolls, damage_to, duel, duel_of, duel_setup, game_on, seat, start},
        },
        map::MapRef,
        terrain::Terrain,
    };

    #[test]
    fn a_hard_bot_hits_its_target() {
        let mut game = duel(700);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let mut damage = 0;
        for event in events {
            for effect in game.apply(event).unwrap() {
                if let Effect::Damaged { seat: 1, amount } = effect {
                    damage += amount;
                }
            }
        }
        assert!(damage >= 120, "a direct hit: {damage}");
    }

    #[test]
    fn it_turns_to_face_its_target_first() {
        let mut game = duel(700);
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        answer_rolls(&mut game);
        // Seat 1 starts facing left, toward seat 0: no turn needed. Face it away and ask again.
        game.apply(Event::Walk { seat: 1, facing: Facing::Right, steps: 0 }).unwrap();
        let events = plan(&game, 1, Level::Hard, 7).unwrap();
        assert_eq!(events[0], Event::Walk { seat: 1, facing: Facing::Left, steps: 0 });
        assert!(matches!(events[1], Event::Shoot { seat: 1, .. }));
    }

    #[test]
    fn it_turns_its_back_on_its_target_to_fire_over_it() {
        // Augerdillo fires over its back: facing its target, it turns away first.
        let mut game = duel_of(MobileKind::Augerdillo, MobileKind::Coilback, 700);
        assert_eq!(game.players()[0].facing, Facing::Right);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert_eq!(events[0], Event::Walk { seat: 0, facing: Facing::Left, steps: 0 });
        let effects = play(&mut game, events);
        assert!(damage_to(&effects, 1) > 0, "{effects:?}");
    }

    #[test]
    fn it_only_plans_its_own_turn_and_is_a_pure_function_of_its_seed() {
        let game = duel(700);
        assert_eq!(plan(&game, 1, Level::Hard, 7), None);
        assert_eq!(plan(&game, 0, Level::Easy, 99), plan(&game, 0, Level::Easy, 99));
    }

    /// Applies a plan, returning everything it did.
    fn play(game: &mut Game, events: Vec<Event>) -> Vec<Effect> {
        events.into_iter().flat_map(|event| game.apply(event).unwrap()).collect()
    }

    fn shot_of(events: &[Event]) -> Shot {
        events.iter().find_map(|e| if let Event::Shoot { shot, .. } = e { Some(*shot) } else { None }).unwrap()
    }

    #[test]
    fn it_plays_every_mobile() {
        for mobile in MobileKind::ALL {
            let mut game = duel_of(mobile, MobileKind::Coilback, 700);
            let events = plan(&game, 0, Level::Hard, 7).unwrap();
            let effects = play(&mut game, events);
            assert!(damage_to(&effects, 1) >= 80, "{mobile:?}: {effects:?}");
            assert_eq!(damage_to(&effects, 0), 0, "{mobile:?} hurt itself: {effects:?}");
        }
    }

    #[test]
    fn it_aims_each_barrels_shots_by_that_barrels_flight() {
        let game = duel_of(MobileKind::Frostfin, MobileKind::Coilback, 700);
        let target = game.players()[1];
        let search = Search::new(&game, 0, target.center());
        assert_eq!(search.grids.len(), 2, "Frostfin's cannon and its tusk");
        for shot in [Shot::S1, Shot::S2, Shot::Ss] {
            let (aim, power) = search.trajectories(shot)[0];
            let impact = game.track(game.launch(0, shot, aim, f64::from(power) / 20.0).unwrap(), 0, 0).impact;
            assert_eq!(impact.contact, Contact::Mobile(1), "{shot:?} at {aim} degrees, {power}/20 bars");
        }
        assert_eq!(Search::new(&duel(700), 0, target.center()).grids.len(), 1, "one barrel, one grid");
    }

    #[test]
    fn it_lands_a_walking_sponge_short_for_its_walk_to_carry_it_in() {
        // The target stands at the brink of a pit: a sponge aimed at it flies through and falls in,
        // so the bot must land one short, where its walk ends inside the target.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 && x < 1115).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Sudsotter, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)];
        let mut game = game_on(terrain, seats);
        game.players[0].ss_lock = 2;
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let effects = play(&mut game, events);
        assert!(damage_to(&effects, 1) >= 150, "{effects:?}");
    }

    #[test]
    fn it_never_tries_a_locked_or_disabled_ss() {
        let mut game = duel(700);
        game.players[0].ss_lock = 2;
        assert_ne!(shot_of(&plan(&game, 0, Level::Hard, 7).unwrap()), Shot::Ss);
        let setup = Setup {
            sudden_death: Some(SuddenDeath { kind: SuddenDeathKind::Double, turn: 0 }),
            ..duel_setup(700)
        };
        let game = start(&setup);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert_ne!(shot_of(&events), Shot::Ss);
        let mut game = game;
        play(&mut game, events);
    }

    #[test]
    fn it_heals_when_low() {
        let mut game = duel(700);
        game.players[0].items = LOADOUT.positions();
        game.players[0].hp = 300;
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let big_heal = (0_u8..).zip(LOADOUT.positions()).find(|(_, item)| *item == Some(Item::BigHeal)).unwrap().0;
        assert_eq!(events[0], Event::UseItem { seat: 0, slot: big_heal, time: Centis(0) });
        let effects = play(&mut game, events);
        assert!(effects.iter().any(|e| matches!(e, Effect::Healed { seat: 0, .. })), "{effects:?}");
    }

    #[test]
    fn it_doubles_or_powers_up_a_strong_shot() {
        let mut game = duel(700);
        game.players[0].items = LOADOUT.positions();
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let Some(&Event::UseItem { slot, .. }) = events.iter().find(|e| matches!(e, Event::UseItem { .. })) else {
            panic!("no item on a direct hit: {events:?}")
        };
        assert!(matches!(LOADOUT.positions()[usize::from(slot)], Some(Item::Dual | Item::PowerUp)));
        assert!(damage_to(&play(&mut game, events), 1) > 165, "more than one S1 could deal");
    }

    #[test]
    fn it_leaves_items_alone_in_sudden_death() {
        let setup = Setup {
            sudden_death: Some(SuddenDeath { kind: SuddenDeathKind::BigBomb, turn: 0 }),
            ..duel_setup(700)
        };
        let mut game = start(&setup);
        game.players[0].items = LOADOUT.positions();
        game.players[0].hp = 100;
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert!(!events.iter().any(|e| matches!(e, Event::UseItem { .. })), "{events:?}");
    }

    /// Times a plan for each mobile in a 4v4 on hills, in wind, with the bot's loadout. Run in
    /// release: `cargo test -p lobshot-sim --release --lib -- --ignored --nocapture tool_time`.
    #[test]
    #[ignore = "a tool, not a test: prints each mobile's planning time"]
    fn tool_time() {
        for mobile in MobileKind::ALL {
            let seats = (0..8)
                .map(|i| Seat {
                    team: if i % 2 == 0 { Team::A } else { Team::B },
                    mobile,
                    spawn_x: 200 + 250 * i,
                    facing: if i % 2 == 0 { Facing::Right } else { Facing::Left },
                    items: LOADOUT,
                })
                .collect();
            let setup = Setup {
                map: MapRef::Hills {
                    width: 2400,
                    height: 1000,
                    ground: 840,
                    amplitude: 120,
                    period: 900,
                },
                seats,
                order: (0..8).collect(),
                wind: Wind::new(18, 120).unwrap(),
                sudden_death: None,
            };
            let game = start(&setup);
            let began = std::time::Instant::now();
            let events = plan(&game, 0, Level::Normal, 7).unwrap();
            println!("{mobile:?}: {:?} for {events:?}", began.elapsed());
        }
    }

    #[test]
    fn the_levels_stay_ordered_hard_exact_and_easy_worst() {
        let spreads = |level: Level| {
            let noise = level.noise();
            [noise.power_steps.value, noise.aim_degrees.value, noise.wind_strength.value, noise.wind_degrees.value]
        };
        let (hard, normal, easy) = (spreads(Level::Hard), spreads(Level::Normal), spreads(Level::Easy));
        assert_eq!(hard, [0; 4], "Hard is the search's own answer");
        assert!(normal.iter().zip(&easy).all(|(n, e)| 0 < *n && n < e), "{normal:?} {easy:?}");
    }

    #[test]
    fn a_misread_wind_stays_within_its_spread_and_hard_reads_it_true() {
        let wind = Wind::new(3, 350).unwrap();
        let mut rng = XorShift::new(99);
        for _ in 0..200 {
            let read = misread(wind, Level::Normal.noise(), &mut rng);
            assert!(read.strength <= 3 + 8 && read.direction < 360, "{read:?}");
            let off = (i32::from(read.direction) - 350).rem_euclid(360);
            assert!(off <= 90 || off >= 270, "{read:?}");
        }
        assert_eq!(misread(wind, Level::Hard.noise(), &mut rng), wind);
    }

    #[test]
    fn normal_lands_between_hard_and_easy() {
        // The same duel in a crosswind, eight seeds a level: the damage each level deals.
        let mut game = duel(700);
        game.wind = Wind::new(14, 30).unwrap();
        let dealt = |level: Level| -> i32 {
            (1..=8)
                .map(|seed| {
                    let mut game = game.clone();
                    let events = plan(&game, 0, level, seed * 7919).unwrap();
                    damage_to(&play(&mut game, events), 1)
                })
                .sum()
        };
        let (hard, normal, easy) = (dealt(Level::Hard), dealt(Level::Normal), dealt(Level::Easy));
        assert!(hard > normal && normal > easy, "Hard {hard}, Normal {normal}, Easy {easy}");
    }

    #[test]
    fn noise_stays_within_its_spread() {
        let mut rng = XorShift::new(12345);
        let draws: Vec<i32> = (0..1000).map(|_| within(&mut rng, 3)).collect();
        assert!(draws.iter().all(|d| (-3..=3).contains(d)));
        assert!((-3..=3).all(|v| draws.contains(&v)), "every value comes up");
        assert_eq!(within(&mut XorShift::new(5), 0), 0);
    }
}

#[cfg(test)]
mod walking {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::{
            MobileKind, Team,
            testing::{damage_to, duel, game_on, seat},
        },
        terrain::Terrain,
    };

    /// Coilback (aim up to 55 degrees) at x=300 behind a wall 200 px tall from x=360 to 520, with
    /// its target at x=1100 on the far side. Ground is at y=600, and `pit` columns have none.
    fn behind_a_wall(pit: std::ops::Range<u32>) -> Game {
        let terrain = Terrain::from_fn(1600, 800, |x, y| !pit.contains(&x) && (y >= 600 || ((360..520).contains(&x) && y >= 400))).unwrap();
        game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Coilback, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        )
    }

    fn walks(events: &[Event]) -> Vec<(Facing, u16)> {
        events
            .iter()
            .filter_map(|e| if let Event::Walk { facing, steps, .. } = *e { Some((facing, steps)) } else { None })
            .collect()
    }

    #[test]
    fn it_walks_out_from_behind_a_wall_it_cannot_shoot_over() {
        let mut game = behind_a_wall(0..0);
        let target = game.players()[1];
        assert!(best_shot(&game, 0, &target).unwrap().value <= 0.0, "nothing clears the wall from here");
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert!(walks(&events).iter().any(|&(facing, steps)| facing == Facing::Left && steps > 0), "{events:?}");
        let effects: Vec<Effect> = events.into_iter().flat_map(|event| game.apply(event).unwrap()).collect();
        assert!(damage_to(&effects, 1) > 0, "{effects:?}");
    }

    #[test]
    fn it_stays_put_when_it_can_hit_from_where_it_stands() {
        let game = duel(700);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert!(walks(&events).iter().all(|&(_, steps)| steps == 0), "{events:?}");
    }

    #[test]
    fn a_walk_into_a_pit_is_no_spot() {
        let game = behind_a_wall(230..260);
        let target = game.players()[1];
        assert!(walked(&game, 0, Some((Facing::Left, 90)), &target).is_none());
        assert!(walked(&game, 0, Some((Facing::Left, 30)), &target).is_some(), "short of the pit");
    }
}

#[cfg(test)]
mod over_a_hill {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::{Effect, MobileKind, Seat, Setup, Team, testing::answer_rolls},
        items::Loadout,
        map::{MapRef, NoAssets},
        units::Wind,
    };

    /// The practice layout: rolling hills (slopes up to about 23 degrees) with a crest between the
    /// two, in strong wind from every side. The true angles alone can't always reach.
    #[test]
    fn a_hard_bot_reaches_its_target_over_a_hill_in_any_wind() {
        for direction in [0_u16, 90, 160, 270] {
            let setup = Setup {
                map: MapRef::Hills {
                    width: 1800,
                    height: 1000,
                    ground: 840,
                    amplitude: 120,
                    period: 900,
                },
                seats: vec![
                    Seat {
                        team: Team::A,
                        mobile: MobileKind::Coilback,
                        spawn_x: 360,
                        facing: Facing::Right,
                        items: Loadout::default(),
                    },
                    Seat {
                        team: Team::B,
                        mobile: MobileKind::Coilback,
                        spawn_x: 1440,
                        facing: Facing::Left,
                        items: Loadout::default(),
                    },
                ],
                order: vec![1, 0],
                wind: Wind::new(24, direction).unwrap(),
                sudden_death: None,
            };
            let mut game = Game::new(&setup, &NoAssets).unwrap();
            answer_rolls(&mut game);
            let damage: i32 = plan(&game, 1, Level::Hard, 7)
                .unwrap()
                .into_iter()
                .flat_map(|event| game.apply(event).unwrap())
                .map(|effect| if let Effect::Damaged { seat: 0, amount } = effect { amount } else { 0 })
                .sum();
            assert!(damage > 0, "wind toward {direction}: the bot should reach its target");
        }
    }
}
