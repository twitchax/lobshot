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
//!    did: damage to enemies, kills, and twice the cost of any harm to its own side. A bot that
//!    isn't sure of the wind or its hands (`Doubt`) values each by its mean over a few draws of
//!    both, so it takes the shot it expects to do most, not the one that only lands as it read.
//! 3. **Item.** A heal first when it is low; otherwise Dual (on S1 or S2) or Power Up when the
//!    chosen shot is strong and the item makes it better.
//!
//! It searches from where it stands. When its best shot from there is weak (behind a wall, say),
//! it runs the first two passes again from a few spots up to its move budget each way, and walks
//! to the spot with the best shot.
//!
//! In Tag it switches first when the mobile in play is low and the one sitting out has more left,
//! for the player is out when the one in play falls; and when its best shot is weak, it searches
//! with the other mobile too and switches if that one's is clearly better (`switch`).
//!
//! Difficulty is error around the search (`tuning::bot`): the bot misreads the wind before it
//! searches, then misses the aim and power it found, and it knows it may: it values shots over
//! that same error. All of it comes from a seed the server rolls, so a bot's turn is still a pure
//! function of its inputs.
//!
//! Dead, it plays the mode's objective (`while_dead`): it spins its slot machine (Solo), or picks
//! where to drop back in (Score, `respawn`): on firm ground, at a fair range from an enemy, with
//! shots at one from there.
//!
//! In Jewel, where shots hurt no one, it shoots for jewels and digs under enemies (`marks`,
//! `best_at`): from where it stands it searches a shot at the jewels worth points that take least
//! and at the nearest dearer one, and takes the best that takes one; when none does, it digs at the
//! nearest enemy's feet, walking if it must, for the ground it blows from under it, or a bunge.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

use crate::{
    game::{Contact, Death, Effect, Event, Game, Impact, Phase, Player, Shot},
    items::{Item, Loadout},
    jewels::Jewel,
    mines, mode,
    movement::Footing,
    referee::XorShift,
    shots,
    terrain::Terrain,
    tuning::{self, AimRange, Barrel, MobileTuning, bot::Noise},
    units::{Centis, Facing, Power, Vec2, Wind},
};

/// The loadout the server gives bots: the items SearchBot knows how to use, filling all six slots.
pub const LOADOUT: Loadout = Loadout::fixed([Item::Dual, Item::PowerUp, Item::Heal, Item::BigHeal]);

/// At or below this share of its max HP, a bot heals before it shoots.
const LOW_HP: f64 = 0.4;

/// A shot worth at least this much (about GunBound's lowest damage tier) is strong: it earns Dual
/// or Power Up, and the bot takes it from where it stands rather than look for a better spot.
const STRONG_SHOT: f64 = 150.0;

/// In Tag, a bot switches mobiles for a shot worth at least this much more than its own (a kill's
/// worth, or most of a solid hit).
const SWITCH_GAIN: f64 = 100.0;

/// A bot looking for a better spot tries walks of each third of its move budget, each way.
const WALK_PARTS: u16 = 3;

/// What a kill is worth on top of its damage, in damage.
const KILL_VALUE: f64 = 300.0;

/// How many draws of the wind and its hands a bot that isn't sure of them values each shot over
/// (`Doubt`): enough that a shot that only lands in a still wind and steady hands shows it.
const DOUBT_DRAWS: usize = 8;

/// In Jewel, it searches a shot at this many of the nearest jewels worth points.
const JEWEL_MARKS: usize = 2;

/// What a jewel's point is worth, in damage: a Sapphire's 5 make a strong shot.
const JEWEL_POINT_VALUE: f64 = 30.0;

/// What each pixel an enemy falls is worth, in damage: ground blown from under it, toward a bunge.
const DIG_VALUE: f64 = 1.0;

/// A dead bot looks for a drop point every this many pixels across the map.
const DROP_STEP: usize = 40;

/// The range it would like to drop in at from the nearest enemy, in pixels: three quarters of a
/// screen, far enough that a miss doesn't splash itself and near enough for a plain lob.
const DROP_RANGE: i32 = 600;

/// Firm ground has ground within `BRINK_DROP` px below the feet in every column `BRINK` px either
/// way: no gap, cliff or map edge to slide or be blasted off.
const BRINK: i32 = 40;
const BRINK_DROP: i32 = 30;

/// It fires a coarse grid of shots from this many of the spots nearest its range, each every
/// `DROP_AIM_STEP` degrees and `DROP_POWER_STEP` power steps, to see which has the most shots at an
/// enemy.
const SHORTLIST: usize = 4;
const DROP_AIM_STEP: usize = 5;
const DROP_POWER_STEP: usize = 4;

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
    let targets = marks(game, &me);

    let mut events = Vec::new();
    let mut scratch = game.as_seen();
    if wants_to_rest(&scratch, seat) {
        let switch = Event::Switch { seat };
        scratch.apply(switch).ok()?;
        events.push(switch);
    }
    if let Some(heal) = heal(&scratch, seat) {
        scratch.apply(heal).ok()?;
        events.push(heal);
    }
    // It picks its spot and its shot in the wind as it reads it, and values each shot by what it
    // expects it to do, knowing it may have read the wind wrong and that its hands may slip.
    let noise = level.noise();
    let mut rng = XorShift::new(seed);
    scratch.wind = misread(scratch.wind(), noise, &mut rng);
    let slip = (within(&mut rng, noise.power_steps.value), within(&mut rng, noise.aim_degrees.value));
    let doubt = Doubt::new(scratch.wind(), noise, &mut rng);

    let Choice { switched, moves, best } = best_at(&scratch, seat, &targets, &doubt)?;
    if let Some(switched) = switched {
        scratch = switched;
        events.push(Event::Switch { seat });
    }
    let me = *scratch.players().get(usize::from(seat))?;
    events.extend(moves);
    if let Some(slot) = best.item {
        events.push(Event::UseItem { seat, slot, time: Centis(0) });
    }

    let (aim, power) = slipped(me.tuning(), best.shot, best.aim, best.power, slip);
    events.push(Event::Shoot {
        seat,
        shot: best.shot,
        aim,
        power,
        time: Centis(0),
    });
    Some(events)
}

/// The aim and power a bot's hands fire for `aim` and `power` when they slip by `slip` (power
/// steps, degrees), kept to the shot's range and the power bar.
fn slipped(tuning: &MobileTuning, shot: Shot, aim: i16, power: u8, (power_slip, aim_slip): (i32, i32)) -> (i16, Power) {
    let range = shot.tuning(tuning).aim.value;
    let aim = (aim + i16::try_from(aim_slip).unwrap_or(0)).clamp(range.min, range.max);
    let twentieths = (i32::from(power) + power_slip).clamp(0, i32::from(Power::MAX.twentieths()));
    (aim, Power::from_twentieths(u8::try_from(twentieths).unwrap_or(0)).unwrap_or(Power::MAX))
}

/// What a bot is unsure of as it values a shot: the true wind, about the one it read, and where
/// its hands will put the aim and power. It values a shot by its mean over `DOUBT_DRAWS` draws of
/// those, the same draws for every shot it weighs that turn, so the comparison is between shots,
/// not luck. Hard is sure of all three, and values each shot once, as it is.
struct Doubt {
    /// Each draw's wind, and its hands' slip in power steps and degrees.
    draws: Vec<(Wind, (i32, i32))>,
}

impl Doubt {
    /// No doubt: each shot valued as the search found it.
    #[cfg(test)]
    const NONE: Self = Self { draws: Vec::new() };

    fn new(read: Wind, noise: &Noise, rng: &mut XorShift) -> Self {
        let spreads = [noise.power_steps.value, noise.aim_degrees.value, noise.wind_strength.value, noise.wind_degrees.value];
        if spreads.iter().all(|&spread| spread == 0) {
            return Self { draws: Vec::new() };
        }
        let draws = (0..DOUBT_DRAWS)
            .map(|_| {
                let wind = misread(read, noise, rng);
                (wind, (within(rng, noise.power_steps.value), within(rng, noise.aim_degrees.value)))
            })
            .collect();
        Self { draws }
    }
}

/// What a dead bot does now, on anyone's turn, when the game lets it: spins its slot machine
/// (Solo), or picks where to drop back in (Score, `respawn`). Every host plays its dead bots by it.
#[must_use]
pub fn while_dead(game: &Game, seat: u8) -> Option<Event> {
    if game.can_spin(seat) {
        Some(Event::Spin { seat })
    } else if game.can_respawn(seat) {
        respawn(game, seat)
    } else {
        None
    }
}

/// What the bot shoots at, as a point to land on and the column it stands in.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mark {
    at: Vec2,
    x: i32,
    goal: Goal,
}

/// What a mark is, which says how the bot searches for a shot at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goal {
    /// An enemy mobile: a shot strikes it directly, and the bot walks to find a better spot when
    /// its shot from here is weak.
    Enemy,
    /// A jewel (Jewel): the aim pass scores a shot by how near it lands, wherever it lands, from
    /// where the bot stands; only a shot that takes it counts.
    Jewel,
    /// The ground at an enemy's feet (Jewel), to dig it out from under it, walking as for an
    /// enemy: the bot's shot when it can take no jewel.
    Feet,
}

impl Mark {
    fn of(player: &Player) -> Self {
        Self {
            at: player.center(),
            x: player.footing.x,
            goal: Goal::Enemy,
        }
    }
}

/// What `me` may shoot at this turn: the nearest enemy, or in Jewel, the jewels worth points that
/// take least (`JEWEL_MARKS` of them, nearest first among equals) and the nearest of the dearer
/// ones (for a turn with SS or Dual), and the ground at the nearest enemy's feet. Empty when
/// there's nothing.
fn marks(game: &Game, me: &Player) -> Vec<Mark> {
    let enemy = game.players().iter().filter(|p| p.alive && p.team != me.team).min_by_key(|p| (p.footing.x - me.footing.x).abs());
    if !game.mode().jewels() {
        return enemy.map(Mark::of).into_iter().collect();
    }
    let feet = enemy.map(|enemy| Mark {
        at: enemy.footing.world(),
        x: enemy.footing.x,
        goal: Goal::Feet,
    });
    let away = |jewel: &Jewel| (jewel.at.x - me.footing.x).abs();
    let mut jewels: Vec<&Jewel> = game.jewels().iter().filter(|jewel| jewel.kind.points() > 0).collect();
    jewels.sort_by_key(|jewel| (jewel.kind.needs(), away(jewel), jewel.id));
    let cheap = jewels.len().min(JEWEL_MARKS);
    let dear = jewels[cheap..]
        .iter()
        .filter(|jewel| jewel.kind.needs() > jewels[0].kind.needs())
        .min_by_key(|jewel| (away(jewel), jewel.id));
    jewels[..cheap]
        .iter()
        .chain(dear)
        .map(|jewel| Mark {
            at: jewel.middle(),
            x: jewel.at.x,
            goal: Goal::Jewel,
        })
        .chain(feet)
        .collect()
}

/// The spot and shot the bot settles on: the game after a switch (Tag), if it switches first; the
/// walk there; and the shot, boosted if an item makes it better.
struct Choice {
    switched: Option<Game>,
    moves: Vec<Event>,
    best: Candidate,
}

/// The best spot and shot at the first of `marks` worth one: at each jewel from where the bot
/// stands, the best that takes one; if none does, at the enemy (or its feet), walking if it must,
/// and in Tag switching first if the other mobile has a clearly better shot (`switch`).
fn best_at(game: &Game, seat: u8, marks: &[Mark], doubt: &Doubt) -> Option<Choice> {
    let boosted = |switched: Option<Game>, Spot { moves, best }: Spot| {
        let mut there = switched.clone().unwrap_or_else(|| game.clone());
        for &event in &moves {
            there.apply(event).ok()?;
        }
        let best = boost(&there, seat, &best, doubt).unwrap_or(best);
        Some(Choice { switched, moves, best })
    };
    let better = |best: Choice, other: Choice| if other.best.beats(&best.best) { other } else { best };
    let jewels = marks.iter().filter(|mark| mark.goal == Goal::Jewel);
    let taken = jewels
        .filter_map(|mark| Spot::here(game, seat, mark, doubt))
        .filter_map(|spot| boosted(None, spot))
        .filter(|choice| choice.best.sure > 0.0)
        .reduce(better);
    taken.or_else(|| {
        marks
            .iter()
            .filter(|mark| mark.goal != Goal::Jewel)
            .filter_map(|mark| {
                let spot = Spot::best(game, seat, mark, doubt)?;
                match switch(game, seat, mark, &spot, doubt) {
                    Some((switched, other)) => boosted(Some(switched), other),
                    None => boosted(None, spot),
                }
            })
            .reduce(better)
    })
}

/// Where a dead bot drops back in, as `Event::Respawn`: of columns every `DROP_STEP` px, where a
/// drop would land in the wind as it is on firm ground (`firm`), the `SHORTLIST` nearest
/// `DROP_RANGE` from the nearest enemy; of those, the one with the most shots at an enemy from
/// there, then the nearest that range. `None` when it can't come back now, or nowhere is firm.
#[must_use]
pub fn respawn(game: &Game, seat: u8) -> Option<Event> {
    if !game.can_respawn(seat) {
        return None;
    }
    let seen = game.as_seen();
    let team = seen.players().get(usize::from(seat))?.team;
    let enemies: Vec<i32> = seen.players().iter().filter(|p| p.alive && p.team != team).map(|p| p.footing.x).collect();
    let width = usize::try_from(seen.terrain().width()).ok()?;
    let mut spots: Vec<(i32, i32, Footing)> = (DROP_STEP / 2..width)
        .step_by(DROP_STEP)
        .filter_map(|x| {
            let x = i32::try_from(x).ok()?;
            let footing = mode::fall(&seen, x).1.filter(|&footing| firm(seen.terrain(), footing))?;
            let range = enemies.iter().map(|enemy| (enemy - footing.x).abs()).min()?;
            Some(((range - DROP_RANGE).abs(), x, footing))
        })
        .collect();
    spots.sort_unstable_by_key(|&(off, x, _)| (off, x));
    spots.truncate(SHORTLIST);
    let (_, x, _) = spots.into_iter().max_by_key(|&(off, x, footing)| (shots_from(&seen, seat, footing), Reverse(off), Reverse(x)))?;
    Some(Event::Respawn { seat, x })
}

/// Whether `footing` is firm ground to land on: every column `BRINK` px either way has ground
/// within `BRINK_DROP` px below the feet, so it isn't at the brink of a gap or the map's edge.
fn firm(terrain: &Terrain, footing: Footing) -> bool {
    (-BRINK..=BRINK).all(|dx| (footing.y..=footing.y + BRINK_DROP).any(|y| terrain.is_solid(footing.x + dx, y)))
}

/// How many of a coarse grid of S1 shots `seat` could fire from `footing`, back on its feet and
/// facing the nearest enemy, strike an enemy: how well placed the spot is.
fn shots_from(game: &Game, seat: u8, footing: Footing) -> usize {
    let mut scratch = game.clone();
    mode::stand(&mut scratch, seat, footing);
    let me = scratch.players[usize::from(seat)];
    let Some(target) = scratch.players().iter().filter(|p| p.alive && p.team != me.team).min_by_key(|p| (p.footing.x - me.footing.x).abs()) else {
        return 0;
    };
    scratch.players[usize::from(seat)].facing = facing_toward(&me, &Mark::of(target));
    let range = Shot::S1.tuning(me.tuning()).aim.value;
    let grid = (range.min..=range.max)
        .step_by(DROP_AIM_STEP)
        .flat_map(|aim| (0..=Power::MAX.twentieths()).step_by(DROP_POWER_STEP).map(move |power| (aim, power)));
    grid.filter(|&(aim, power)| {
        scratch
            .launch(seat, Shot::S1, aim, f64::from(power) / 20.0)
            .is_some_and(|flight| matches!(scratch.track(flight, 0, 0).impact.contact, Contact::Mobile(hit) if scratch.players()[usize::from(hit)].team != me.team))
    })
    .count()
}

/// Where the bot shoots from, and its best shot there.
struct Spot {
    /// The walk there, then the turn toward the target, as events.
    moves: Vec<Event>,
    best: Candidate,
}

impl Spot {
    /// Where it stands, turned to shoot at `target`.
    fn here(game: &Game, seat: u8, target: &Mark, doubt: &Doubt) -> Option<Self> {
        let (scratch, moves) = walked(game, seat, None, target)?;
        Self::searched(&scratch, moves, seat, target, doubt)
    }

    /// Where it stands, unless its best shot from there is weak, even were it sure of the wind and
    /// its hands; then the best of a few spots up to its whole move budget each way. Shorter walks
    /// come first, so a tie stays closer.
    fn best(game: &Game, seat: u8, target: &Mark, doubt: &Doubt) -> Option<Self> {
        let me = game.players().get(usize::from(seat))?;
        let (scratch, moves) = walked(game, seat, None, target)?;
        let mut seen = vec![me.footing];
        let mut best = Self::searched(&scratch, moves, seat, target, doubt)?;
        if best.best.sure >= STRONG_SHOT {
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
                if let Some(spot) = Self::searched(&scratch, moves, seat, target, doubt)
                    && spot.best.beats(&best.best)
                {
                    best = spot;
                }
            }
        }
        Some(best)
    }

    /// The best shot from where `game` has the bot stand, after `moves`.
    fn searched(game: &Game, moves: Vec<Event>, seat: u8, target: &Mark, doubt: &Doubt) -> Option<Self> {
        Some(Self {
            moves,
            best: best_shot(game, seat, target, doubt)?,
        })
    }
}

/// `game` after `walk`, if any, and turning to shoot at the target, with the moves that did it.
/// `None` when the walk ends the turn (off the map).
fn walked(game: &Game, seat: u8, walk: Option<(Facing, u16)>, target: &Mark) -> Option<(Game, Vec<Event>)> {
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

fn facing_toward(me: &Player, target: &Mark) -> Facing {
    if target.x >= me.footing.x { Facing::Right } else { Facing::Left }
}

/// The way to face to shoot at the target: toward it, or away for a mobile that fires over its
/// back.
fn facing_to_shoot(me: &Player, target: &Mark) -> Facing {
    let toward = facing_toward(me, target);
    if me.tuning().s1.aim.value.backward() { toward.flip() } else { toward }
}

/// The best shot `seat` can take from where it stands, as it faces, by what it expects each to do
/// (`Doubt`).
fn best_shot(game: &Game, seat: u8, target: &Mark, doubt: &Doubt) -> Option<Candidate> {
    game.players().get(usize::from(seat))?;
    let search = Search::new(game, seat, target);
    let mut best: Option<Candidate> = None;
    for shot in [Shot::S1, Shot::S2, Shot::Ss] {
        if !game.can_shoot(seat, shot) {
            continue;
        }
        for (aim, power) in search.trajectories(shot) {
            let candidate = Candidate::new(None, shot, aim, power, search.proxy(shot, aim, power)).fired(game, seat, doubt);
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
    /// Whether the target is a mobile (`Mark::mobile`).
    mobile: bool,
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
    fn new(game: &'g Game, seat: u8, target: &Mark) -> Self {
        let mut search = Self {
            game,
            seat,
            target: target.at,
            mobile: target.goal == Goal::Enemy,
            grids: Vec::new(),
        };
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
            Contact::Mobile(_) if self.mobile => Landing::Hit(impact.at.distance(self.target)),
            Contact::Mobile(_) | Contact::Terrain => Landing::Near(impact.at.distance(self.target)),
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
    /// What the bot expects it to do, over its doubt (`Doubt`).
    value: f64,
    /// What it does in the wind as the bot read it, fired just as found: whether it can do any good
    /// from here at all.
    sure: f64,
    /// The aim pass's score, to break ties between shots that did nothing.
    proxy: f64,
}

impl Candidate {
    /// A shot to value (`fired`).
    fn new(item: Option<u8>, shot: Shot, aim: i16, power: u8, proxy: f64) -> Self {
        Self {
            item,
            shot,
            aim,
            power,
            value: f64::MIN,
            sure: f64::MIN,
            proxy,
        }
    }

    /// Fires the shot (after its item, if any) in scratch copies of `game` and values it: as found,
    /// in the wind as read (`sure`), and, if that does any good, over each of the doubt's draws,
    /// whose mean is `value`.
    fn fired(self, game: &Game, seat: u8, doubt: &Doubt) -> Self {
        let Some(me) = game.players().get(usize::from(seat)) else { return self };
        let tuning = me.tuning();
        let once = |wind: Option<Wind>, slip: (i32, i32)| {
            let mut scratch = game.clone();
            if let Some(wind) = wind {
                scratch.wind = wind;
            }
            let used = self.item.is_none_or(|slot| scratch.apply(Event::UseItem { seat, slot, time: Centis(0) }).is_ok());
            let (aim, power) = slipped(tuning, self.shot, self.aim, self.power, slip);
            let effects = used
                .then(|| {
                    scratch
                        .apply(Event::Shoot {
                            seat,
                            shot: self.shot,
                            aim,
                            power,
                            time: Centis(0),
                        })
                        .ok()
                })
                .flatten();
            effects.map(|effects| value(game, seat, &effects))
        };
        let Some(sure) = once(None, (0, 0)) else { return self };
        // A shot that does no good even as found isn't worth weighing over the doubt.
        let value = if doubt.draws.is_empty() || sure <= 0.0 {
            sure
        } else {
            let total: f64 = doubt.draws.iter().map(|&(wind, slip)| once(Some(wind), slip).unwrap_or(0.0)).sum();
            total / f64::from(u32::try_from(doubt.draws.len()).unwrap_or(1))
        };
        Self { value, sure, ..self }
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
        .sum::<f64>()
        + if game.mode().jewels() { jewel_value(game, seat, effects) } else { 0.0 }
}

/// What a shot's effects are worth to `seat` in Jewel, beyond `value`: each jewel's points to the
/// team that took it (`JEWEL_POINT_VALUE` a point, against it for the other side's), and each
/// pixel an enemy fell (`DIG_VALUE`), its own side's twice over against it.
fn jewel_value(game: &Game, seat: u8, effects: &[Effect]) -> f64 {
    let team = |s: u8| game.players().get(usize::from(s)).map(|p| p.team);
    let mine = team(seat);
    effects
        .iter()
        .map(|effect| match *effect {
            Effect::JewelTaken { id, team: taker, .. } | Effect::JewelGone { id, team: Some(taker) } => {
                let points = game.jewels().iter().find(|jewel| jewel.id == id).map_or(0, |jewel| jewel.kind.points());
                let sign = if Some(taker) == mine { 1.0 } else { -1.0 };
                sign * f64::from(points) * JEWEL_POINT_VALUE
            }
            Effect::Fell { seat: fell, to } => {
                let drop = game.players().get(usize::from(fell)).map_or(0, |player| to.y - player.footing.y);
                let drop = f64::from(drop.max(0)) * DIG_VALUE;
                if team(fell) == mine { -2.0 * drop } else { drop }
            }
            _ => 0.0,
        })
        .sum()
}

/// Tag: whether `seat` switches out the mobile in play to spare it: it is low (`LOW_HP`), and the
/// one sitting out has a bigger share of its HP left.
fn wants_to_rest(game: &Game, seat: u8) -> bool {
    let (Some(me), Some(bench)) = (game.players().get(usize::from(seat)), game.bench(seat)) else {
        return false;
    };
    let share = |hp: i32, max: i32| f64::from(hp) / f64::from(max.max(1));
    let now = share(me.hp, me.tuning().hp.value);
    game.can_switch(seat) && now <= LOW_HP && share(bench.hp, bench.mobile.tuning().hp.value) > now
}

/// Tag: when the best shot from `spot` is weak and `seat` may switch, the game switched and the
/// other mobile's best spot, if its shot is clearly better (`SWITCH_GAIN` more).
fn switch(game: &Game, seat: u8, target: &Mark, spot: &Spot, doubt: &Doubt) -> Option<(Game, Spot)> {
    if spot.best.sure >= STRONG_SHOT || !game.can_switch(seat) {
        return None;
    }
    let mut switched = game.clone();
    switched.apply(Event::Switch { seat }).ok()?;
    let other = Spot::best(&switched, seat, target, doubt)?;
    (other.best.value >= spot.best.value + SWITCH_GAIN).then_some((switched, other))
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
fn boost(game: &Game, seat: u8, best: &Candidate, doubt: &Doubt) -> Option<Candidate> {
    // In Jewel a shot short of what a jewel needs may take it with Dual: weak isn't hopeless.
    if (best.sure < STRONG_SHOT && !game.mode().jewels()) || best.item.is_some() {
        return None;
    }
    let me = game.players().get(usize::from(seat))?;
    let wanted: &[Item] = if best.shot == Shot::Ss { &[Item::PowerUp] } else { &[Item::Dual, Item::PowerUp] };
    let slot = wanted.iter().find_map(|&item| slot_of(game, me, seat, item))?;
    let boosted = Candidate::new(Some(slot), best.shot, best.aim, best.power, best.proxy).fired(game, seat, doubt);
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
        mode::Mode,
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
        let search = Search::new(&game, 0, &Mark::of(&target));
        assert_eq!(search.grids.len(), 2, "Frostfin's cannon and its tusk");
        for shot in [Shot::S1, Shot::S2, Shot::Ss] {
            let (aim, power) = search.trajectories(shot)[0];
            let impact = game.track(game.launch(0, shot, aim, f64::from(power) / 20.0).unwrap(), 0, 0).impact;
            assert_eq!(impact.contact, Contact::Mobile(1), "{shot:?} at {aim} degrees, {power}/20 bars");
        }
        assert_eq!(Search::new(&duel(700), 0, &Mark::of(&target)).grids.len(), 1, "one barrel, one grid");
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
                    second: None,
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
                mode: Mode::Solo,
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
    fn a_bot_unsure_of_the_wind_and_its_hands_draws_its_doubt_about_what_it_read() {
        let read = Wind::new(10, 90).unwrap();
        assert!(Doubt::new(read, Level::Hard.noise(), &mut XorShift::new(5)).draws.is_empty(), "Hard is sure");
        let noise = Level::Normal.noise();
        let doubt = Doubt::new(read, noise, &mut XorShift::new(5));
        assert_eq!(doubt.draws.len(), DOUBT_DRAWS);
        for &(wind, (power, aim)) in &doubt.draws {
            assert!((2..=18).contains(&wind.strength) && (0..=180).contains(&wind.direction), "{wind:?}");
            assert!(power.abs() <= noise.power_steps.value && aim.abs() <= noise.aim_degrees.value);
        }
        assert!(doubt.draws.windows(2).any(|pair| pair[0] != pair[1]), "draws differ");
    }

    #[test]
    fn a_shot_is_worth_what_it_does_on_average_over_the_doubt() {
        let game = duel(700);
        let target = Mark::of(&game.players()[1]);
        let found = best_shot(&game, 0, &target, &Doubt::NONE).unwrap();
        assert!(found.sure >= 120.0 && found.value.to_bits() == found.sure.to_bits(), "a direct hit, valued as found: {found:?}");
        // Two draws: the wind as read and steady hands, then a hand that slips two bars long.
        let doubt = Doubt {
            draws: vec![(game.wind(), (0, 0)), (game.wind(), (40, 0))],
        };
        let doubted = Candidate::new(None, found.shot, found.aim, found.power, found.proxy).fired(&game, 0, &doubt);
        assert_eq!(doubted.sure.to_bits(), found.sure.to_bits());
        assert!((doubted.value - found.sure / 2.0).abs() < 1e-9, "half the hit, for half the draws: {doubted:?}");
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
mod dead {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            MobileKind, Team,
            testing::{answer_rolls, game_in, seat},
        },
        mode::Mode,
    };

    /// Gaps in the ground: one off the map just right of seat 0 (x = 300), and one where a bot would
    /// most like to drop, `DROP_RANGE` short of the enemy at x = 1700.
    const GAPS: [std::ops::Range<i32>; 2] = [330..350, 1060..1130];

    fn gapped() -> Terrain {
        Terrain::from_fn(2400, 600, |x, y| {
            let x = i32::try_from(x).unwrap();
            y >= 500 && !GAPS.iter().any(|gap| gap.contains(&x))
        })
        .unwrap()
    }

    /// A `mode` 2v2 on `gapped` ground in which seat 0 has just walked off into the first gap.
    fn after_a_death(mode: Mode) -> Game {
        let seats = vec![
            seat(Team::A, MobileKind::Coilback, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1700, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 600, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 2100, Facing::Left),
        ];
        let mut game = game_in(mode, gapped(), seats);
        game.apply(Event::Walk {
            seat: 0,
            facing: Facing::Right,
            steps: 60,
        })
        .unwrap();
        answer_rolls(&mut game);
        assert!(!game.players()[0].alive);
        game
    }

    #[test]
    fn a_dead_bot_drops_in_on_firm_ground_with_a_shot_at_an_enemy() {
        let game = after_a_death(Mode::Score);
        let Some(Event::Respawn { seat: 0, x }) = respawn(&game, 0) else { panic!("no pick") };
        let footing = mode::fall(&game, x).1.expect("it lands");
        assert_eq!(footing.y, 500, "on the ground");
        for gap in GAPS {
            assert!(footing.x < gap.start - BRINK || footing.x >= gap.end + BRINK, "x = {} at the brink of {gap:?}", footing.x);
        }
        assert!(shots_from(&game, 0, footing) > 0, "a shot at an enemy from x = {}", footing.x);
        let mut game = game;
        assert!(game.apply(Event::Respawn { seat: 0, x }).is_ok());
    }

    #[test]
    fn firm_ground_has_ground_all_round_and_the_brink_does_not() {
        let terrain = gapped();
        let at = |x| Footing { x, y: 500 };
        assert!(firm(&terrain, at(900)));
        assert!(!firm(&terrain, at(GAPS[1].start - 10)), "at a gap's edge");
        assert!(!firm(&terrain, at(10)), "at the map's edge");
        assert!(firm(&terrain, at(GAPS[1].end + BRINK)));
    }

    #[test]
    fn the_dead_spin_in_solo_and_pick_a_drop_point_in_score() {
        let solo = after_a_death(Mode::Solo);
        assert_eq!(while_dead(&solo, 0), Some(Event::Spin { seat: 0 }));
        assert_eq!(respawn(&solo, 0), None);
        let mut score = after_a_death(Mode::Score);
        assert!(matches!(while_dead(&score, 0), Some(Event::Respawn { seat: 0, .. })));
        assert_eq!(while_dead(&score, 1), None, "the living do nothing");
        let pick = while_dead(&score, 0).unwrap();
        score.apply(pick).unwrap();
        assert_eq!(while_dead(&score, 0), None, "once picked, it waits");
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
        let target = Mark::of(&game.players()[1]);
        assert!(best_shot(&game, 0, &target, &Doubt::NONE).unwrap().value <= 0.0, "nothing clears the wall from here");
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
        let target = Mark::of(&game.players()[1]);
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
        mode::Mode,
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
                        second: None,
                    },
                    Seat {
                        team: Team::B,
                        mobile: MobileKind::Coilback,
                        spawn_x: 1440,
                        facing: Facing::Left,
                        items: Loadout::default(),
                        second: None,
                    },
                ],
                order: vec![1, 0],
                wind: Wind::new(24, direction).unwrap(),
                sudden_death: None,
                mode: Mode::Solo,
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

#[cfg(test)]
mod tag {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::{
            MobileKind, Seat, Team,
            testing::{damage_to, game_in, seat},
        },
        mode::Mode,
        terrain::Terrain,
    };

    /// A Tag duel on flat ground at y = 600: seat 0 plays `first` with `second` sitting out at x =
    /// 300, seat 1 a Coilback pair at x = 1100. `wall` stands a wall 200 px tall from x = 360 to 520.
    fn duel(first: MobileKind, second: MobileKind, wall: bool) -> Game {
        let terrain = Terrain::from_fn(1600, 800, |x, y| y >= 600 || (wall && (360..520).contains(&x) && y >= 400)).unwrap();
        let pair = |team, first, second, x, facing| Seat {
            second: Some(second),
            ..seat(team, first, x, facing)
        };
        let seats = vec![
            pair(Team::A, first, second, 300, Facing::Right),
            pair(Team::B, MobileKind::Coilback, MobileKind::Coilback, 1100, Facing::Left),
        ];
        game_in(Mode::Tag, terrain, seats)
    }

    fn switches(events: &[Event]) -> usize {
        events.iter().filter(|event| matches!(event, Event::Switch { .. })).count()
    }

    #[test]
    fn a_low_bot_switches_out_to_spare_its_mobile_in_play() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Hookclaw, false);
        game.players[0].hp = 300;
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert_eq!(events.first(), Some(&Event::Switch { seat: 0 }), "first thing: {events:?}");
        assert_eq!(switches(&events), 1);
        let mut played = game.clone();
        for event in events {
            played.apply(event).unwrap();
        }
        assert_eq!(played.bench(0).map(|bench| (bench.mobile, bench.hp)), Some((MobileKind::Coilback, 300)), "the Coilback rests");
    }

    #[test]
    fn a_low_bot_with_a_lower_mobile_sitting_out_stays() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Hookclaw, false);
        game.players[0].hp = 300;
        game.mode_state.bench_mut(0).unwrap().hp = 100;
        assert_eq!(switches(&plan(&game, 0, Level::Hard, 7).unwrap()), 0);
    }

    #[test]
    fn a_bot_switches_in_the_mobile_with_the_clearly_better_shot() {
        // Pinned behind the wall, the Coilback (55 degrees at most) can't clear it; the Hookclaw
        // (90) lobs over.
        let mut game = duel(MobileKind::Coilback, MobileKind::Hookclaw, true);
        game.players[0].moves_left = 0;
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        assert_eq!(switches(&events), 1, "{events:?}");
        let damage: i32 = events.into_iter().map(|event| damage_to(&game.apply(event).unwrap(), 1)).sum();
        assert!(damage > 0, "and hits");
    }

    #[test]
    fn a_bot_with_a_good_shot_keeps_its_mobile() {
        let game = duel(MobileKind::Coilback, MobileKind::Hookclaw, false);
        assert_eq!(switches(&plan(&game, 0, Level::Hard, 7).unwrap()), 0);
    }
}

#[cfg(test)]
mod jewel {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::{
            Setup, Team,
            testing::{answer_rolls, duel_setup, game_in, seat, start},
        },
        jewels::{self, JewelDrop, JewelKind},
        mode::Mode,
        terrain::Terrain,
    };

    fn duel() -> Game {
        start(&Setup { mode: Mode::Jewel, ..duel_setup(800) })
    }

    fn drop_at(game: &mut Game, kind: JewelKind, x: i32) {
        jewels::drop(game, JewelDrop { kind, x }, &mut Vec::new());
    }

    fn play(game: &mut Game, events: Vec<Event>) -> Vec<Effect> {
        events.into_iter().flat_map(|event| game.apply(event).unwrap()).collect()
    }

    #[test]
    fn it_shoots_the_jewel_in_reach_and_takes_it() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 750);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let effects = play(&mut game, events);
        assert!(effects.iter().any(|e| matches!(e, Effect::JewelTaken { team: Team::A, .. })), "{effects:?}");
        assert_eq!(game.points(Team::A), Some(5));
    }

    #[test]
    fn it_leaves_a_black_jewel_alone_and_digs_under_its_enemy_instead() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Onyx, 750);
        let me = game.players()[0];
        let feet = Mark {
            at: game.players()[1].footing.world(),
            x: game.players()[1].footing.x,
            goal: Goal::Feet,
        };
        assert_eq!(marks(&game, &me), [feet], "no jewel worth a shot");
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let effects = play(&mut game, events);
        assert!(!effects.iter().any(|e| matches!(e, Effect::JewelTaken { .. })), "{effects:?}");
        assert!(
            effects.iter().any(|e| matches!(e, Effect::Exploded { at, dirty: Some(_), .. } if at.distance(feet.at) < 45.0)),
            "it dug at its feet: {effects:?}"
        );
    }

    #[test]
    fn it_takes_the_richer_of_two_jewels_it_can_take_counting_what_this_turn_dealt() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 500);
        drop_at(&mut game, JewelKind::Topaz, 760);
        let topaz = *game.jewels().last().unwrap();
        // One S1 can't take a Topaz's 200, but one that already took 150 this turn (a mine's, say).
        let mut alone = game.clone();
        let events = plan(&alone, 0, Level::Hard, 7).unwrap();
        assert!(play(&mut alone, events).contains(&Effect::Points { team: Team::A, points: 5 }), "the Sapphire, alone");
        jewels::hit(&mut game, topaz.id, 150, 0, &mut Vec::new());
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let effects = play(&mut game, events);
        assert!(effects.iter().any(|e| matches!(e, Effect::JewelTaken { id, .. } if *id == topaz.id)), "{effects:?}");
        assert_eq!(game.points(Team::A), Some(10));
    }

    #[test]
    fn a_dead_bot_picks_where_to_drop_back_in() {
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 && !(330..350).contains(&x)).unwrap();
        let seats = vec![
            seat(Team::A, crate::game::MobileKind::Coilback, 300, Facing::Right),
            seat(Team::B, crate::game::MobileKind::Coilback, 1100, Facing::Left),
            seat(Team::A, crate::game::MobileKind::Coilback, 1700, Facing::Left),
        ];
        let mut game = game_in(Mode::Jewel, terrain, seats);
        game.apply(Event::Walk {
            seat: 0,
            facing: Facing::Right,
            steps: 60,
        })
        .unwrap();
        answer_rolls(&mut game);
        assert!(!game.players()[0].alive);
        let Some(Event::Respawn { seat: 0, x }) = while_dead(&game, 0) else { panic!() };
        assert!(!(320..360).contains(&x), "not down the pit: {x}");
    }
}
