//! Balance runs: thousands of 1v1 SearchBot games, every pair of mobiles on every map
//! variant, in parallel across games (never inside one). Each game is a `Spec`, and a spec is the
//! whole game: playing it again plays the same game, so any game in a report can be replayed.

use std::{
    fmt,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use lobshot_assets::{maps::Maps, mobile_slug};
use lobshot_protocol::RoomOptions;
use lobshot_sim::{
    base::Res,
    bot::{self, Level},
    game::{Effect, Event, MobileKind, Phase, Player, Shot, Team},
    log::Log,
    map::{Arena, MapRef},
    referee::{Entrant, XorShift},
};

use crate::host::{Happened, Host};

/// A game that hasn't ended by this many turns is a draw. Sudden death (from turn 40 by default)
/// ends real games long before.
pub const TURN_CAP: i64 = 300;

/// One balance game: who plays (seat 0 is team A, seat 1 team B), where, how well, and the seed
/// every roll comes from. Written `coilback.hookclaw.tidewall.A.normal.17`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    pub mobiles: [MobileKind; 2],
    pub map: String,
    pub variant: String,
    pub level: Level,
    pub seed: u64,
}

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b] = self.mobiles.map(mobile_slug);
        write!(f, "{a}.{b}.{}.{}.{}.{}", self.map, self.variant, level_name(self.level), self.seed)
    }
}

impl FromStr for Spec {
    type Err = anyhow::Error;

    fn from_str(id: &str) -> Res<Self> {
        let parts: Vec<&str> = id.split('.').collect();
        let &[a, b, map, variant, level, seed] = parts.as_slice() else {
            bail!("{id:?} isn't a game id: mobile.mobile.map.variant.level.seed");
        };
        Ok(Self {
            mobiles: [mobile(a)?, mobile(b)?],
            map: map.to_owned(),
            variant: variant.to_owned(),
            level: level.parse::<LevelName>()?.0,
            seed: seed.parse().with_context(|| format!("{seed:?} isn't a seed"))?,
        })
    }
}

/// A mobile by its lobshot name, any case.
///
/// # Errors
///
/// Fails on a name that isn't a mobile.
pub fn mobile(name: &str) -> Res<MobileKind> {
    MobileKind::ALL
        .into_iter()
        .find(|&mobile| mobile_slug(mobile) == name.to_lowercase())
        .with_context(|| format!("{name:?} isn't a mobile"))
}

/// A SearchBot level, as the command line and game ids write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelName(pub Level);

impl FromStr for LevelName {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> Res<Self> {
        [Level::Easy, Level::Normal, Level::Hard]
            .into_iter()
            .find(|&level| level_name(level) == name.to_lowercase())
            .map(Self)
            .with_context(|| format!("{name:?} isn't a level: easy, normal or hard"))
    }
}

#[must_use]
pub fn level_name(level: Level) -> &'static str {
    match level {
        Level::Easy => "easy",
        Level::Normal => "normal",
        Level::Hard => "hard",
    }
}

/// What one seat did over a game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Side {
    /// Its own turns.
    pub turns: u32,
    /// HP it took off the enemy, shields included.
    pub damage: i64,
    /// Delay its turns cost it.
    pub delay: u64,
    /// Shots that hurt the enemy.
    pub hits: u32,
    /// By shot (S1, S2, SS): turns it fired that shot, and their damage and delay.
    pub shots: [Usage; 3],
}

/// What turns that fired one shot added up to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub uses: u32,
    pub damage: i64,
    pub delay: u64,
}

/// How a game went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub spec: Spec,
    /// The winning seat; `None` for a draw (both died at once, or `TURN_CAP`).
    pub winner: Option<u8>,
    pub turns: i64,
    /// The seat that moved first.
    pub first: u8,
    pub sides: [Side; 2],
}

/// The arenas a run covers: every variant of each of `maps`, named by map and variant.
///
/// # Errors
///
/// Fails when a map can't be loaded.
pub fn arenas(maps: &Maps, names: &[String]) -> Res<Vec<(String, String)>> {
    let mut all = Vec::new();
    for name in names {
        for arena in maps.arenas(name)? {
            all.push((name.clone(), variant(&arena).to_owned()));
        }
    }
    Ok(all)
}

/// The variant id an arena plays; the built-in map's only one is `A`.
fn variant(arena: &Arena) -> &str {
    match &arena.map {
        MapRef::Asset { variant, .. } => variant,
        MapRef::Flat { .. } | MapRef::Hills { .. } => "A",
    }
}

/// Every game of a run: `games` per pair of `mobiles` (mirrors too), the arenas in rotation. A
/// pair of different mobiles plays each seed twice with the seats swapped, so neither keeps the
/// better spawn or the first move; a mirror plays a fresh seed every game.
#[must_use]
pub fn specs(mobiles: &[MobileKind], arenas: &[(String, String)], games: u32, level: Level, seed: u64) -> Vec<Spec> {
    let mut specs = Vec::new();
    for (i, &a) in mobiles.iter().enumerate() {
        for (j, &b) in mobiles.iter().enumerate().skip(i) {
            for game in 0..games {
                let (round, swapped) = if a == b { (game, false) } else { (game / 2, game % 2 == 1) };
                let (map, variant) = &arenas[usize::try_from(round).unwrap_or(0) % arenas.len().max(1)];
                specs.push(Spec {
                    mobiles: if swapped { [b, a] } else { [a, b] },
                    map: map.clone(),
                    variant: variant.clone(),
                    level,
                    seed: mix(seed, &[u64::try_from(i).unwrap_or(0), u64::try_from(j).unwrap_or(0), u64::from(round)]),
                });
            }
        }
    }
    specs
}

/// splitmix64 over a seed and some numbers: unrelated seeds for neighboring games.
fn mix(seed: u64, parts: &[u64]) -> u64 {
    parts.iter().fold(seed, |state, &part| {
        let mut z = (state ^ part).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    })
}

/// Plays one game, returning how it went and its log.
///
/// # Errors
///
/// Fails when the spec's map or variant doesn't exist, or the sim refuses an event the host chose.
pub fn play(spec: &Spec, maps: &Maps) -> Res<(Outcome, Log)> {
    let arena = maps
        .arenas(&spec.map)?
        .into_iter()
        .find(|arena| variant(arena) == spec.variant)
        .with_context(|| format!("{} has no variant {}", spec.map, spec.variant))?;
    let entrants = [Team::A, Team::B].map(|team| Entrant {
        team,
        mobile: Some(spec.mobiles[usize::from(team == Team::B)]),
        items: bot::LOADOUT,
    });
    let mut host = Host::seat(&arena, &entrants, &RoomOptions::default(), vec![Some(spec.level); 2], XorShift::new(spec.seed), maps)?;
    let first = host.log().setup.order[0];
    let mut sides = [Side::default(); 2];
    let mut out = Vec::new();
    while host.game().turns() < TURN_CAP {
        let before: Vec<u32> = host.game().players().iter().map(|p| p.clock.delay).collect();
        out.clear();
        let Some(seat) = host.step(&mut out)? else { break };
        let i = usize::from(seat);
        let delay = u64::from(host.game().players()[i].clock.delay.saturating_sub(before[i]));
        tally(&mut sides[i], enemy_damage(host.game().players(), seat, &out), &out, delay);
    }
    let winner = match host.game().phase() {
        Phase::Over { winner: Some(team) } => Some(u8::from(team == Team::B)),
        Phase::Over { winner: None } | Phase::Turn { .. } => None,
    };
    let outcome = Outcome {
        spec: spec.clone(),
        winner,
        turns: host.game().turns(),
        first,
        sides,
    };
    Ok((outcome, host.log().clone()))
}

/// Adds one turn to its side: the shot it fired, the `damage` that shot did, and what the turn
/// cost in delay.
fn tally(side: &mut Side, damage: i64, turn: &[Happened], delay: u64) {
    side.turns += 1;
    side.delay += delay;
    let Some(shot) = turn.iter().find_map(|h| match h.event {
        Event::Shoot { shot, .. } => Some(shot),
        _ => None,
    }) else {
        return;
    };
    side.damage += damage;
    side.hits += u32::from(damage > 0);
    let usage = &mut side.shots[shot_index(shot)];
    usage.uses += 1;
    usage.damage += damage;
    usage.delay += delay;
}

/// The HP (shields included) `seat`'s shot took off the other team in one of its turns: what its
/// `Shoot` did before any slot-machine drop at the turn's end.
#[must_use]
pub fn enemy_damage(players: &[Player], seat: u8, turn: &[Happened]) -> i64 {
    let team = players.get(usize::from(seat)).map(|p| p.team);
    let enemy = |hurt: u8| players.get(usize::from(hurt)).is_some_and(|p| Some(p.team) != team);
    turn.iter()
        .filter(|h| matches!(h.event, Event::Shoot { seat: shooter, .. } if shooter == seat))
        .flat_map(|h| h.effects.iter().take_while(|e| !matches!(e, Effect::Dropped { .. })))
        .filter_map(|e| match *e {
            Effect::Damaged { seat: hurt, amount } | Effect::Shielded { seat: hurt, amount } if enemy(hurt) => Some(i64::from(amount)),
            _ => None,
        })
        .sum()
}

#[must_use]
pub fn shot_index(shot: Shot) -> usize {
    match shot {
        Shot::S1 => 0,
        Shot::S2 => 1,
        Shot::Ss => 2,
    }
}

/// Plays every spec on `threads` threads. Results come back in `specs`' order, the same however
/// many threads ran them.
///
/// # Errors
///
/// Fails on the first game that fails, naming it.
pub fn run(specs: &[Spec], maps: &Maps, threads: usize, progress: &(dyn Fn(usize) + Sync)) -> Res<Vec<(Outcome, Log)>> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let mut results: Vec<(usize, Res<(Outcome, Log)>)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(spec) = specs.get(i) else { break };
                        mine.push((i, play(spec, maps).with_context(|| format!("game {spec}"))));
                        progress(done.fetch_add(1, Ordering::Relaxed) + 1);
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
    });
    anyhow::ensure!(results.len() == specs.len(), "a worker thread panicked");
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use pretty_assertions::assert_eq;

    use lobshot_assets::FLAT;

    use super::*;

    fn maps() -> Maps {
        Maps::new(Path::new("no-assets"))
    }

    fn flat() -> Vec<(String, String)> {
        arenas(&maps(), &[FLAT.to_owned()]).unwrap()
    }

    #[test]
    fn game_ids_round_trip() {
        let spec = Spec {
            mobiles: [MobileKind::Coilback, MobileKind::Hookclaw],
            map: "tidewall".into(),
            variant: "B".into(),
            level: Level::Normal,
            seed: 17,
        };
        assert_eq!(spec.to_string(), "coilback.hookclaw.tidewall.B.normal.17");
        assert_eq!(spec.to_string().parse::<Spec>().unwrap(), spec);
        assert_eq!("Coilback.HOOKCLAW.tidewall.B.Normal.17".parse::<Spec>().unwrap(), spec);
        for bad in [
            "",
            "coilback.hookclaw.tidewall.B.normal",
            "pip.hookclaw.tidewall.B.normal.1",
            "coilback.hookclaw.tidewall.B.great.1",
            "coilback.hookclaw.tidewall.B.normal.x",
        ] {
            assert!(bad.parse::<Spec>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn specs_cover_every_pair_and_swap_seats_on_each_seed() {
        let arenas = vec![("a".to_owned(), "A".to_owned()), ("a".to_owned(), "B".to_owned()), ("b".to_owned(), "A".to_owned())];
        let mobiles = [MobileKind::Coilback, MobileKind::Hookclaw, MobileKind::Sparkhorn];
        let specs = specs(&mobiles, &arenas, 6, Level::Normal, 1);
        assert_eq!(specs.len(), 6 * 6, "three mirrors and three pairs");

        let pair: Vec<&Spec> = specs
            .iter()
            .filter(|s| s.mobiles.contains(&MobileKind::Coilback) && s.mobiles.contains(&MobileKind::Hookclaw))
            .collect();
        for two in pair.chunks(2) {
            assert_eq!(two[0].seed, two[1].seed);
            assert_eq!((two[0].mobiles[0], two[0].mobiles[1]), (two[1].mobiles[1], two[1].mobiles[0]));
            assert_eq!((&two[0].map, &two[0].variant), (&two[1].map, &two[1].variant));
        }
        let places: Vec<(&str, &str)> = pair.iter().step_by(2).map(|s| (s.map.as_str(), s.variant.as_str())).collect();
        assert_eq!(places, [("a", "A"), ("a", "B"), ("b", "A")], "the arenas rotate");

        let mirror: Vec<u64> = specs.iter().filter(|s| s.mobiles == [MobileKind::Sparkhorn; 2]).map(|s| s.seed).collect();
        let mut distinct = mirror.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), 6, "a mirror never replays a seed");
    }

    #[test]
    fn a_game_plays_out_and_its_numbers_add_up() {
        let spec = &specs(&[MobileKind::Coilback, MobileKind::Marblemole], &flat(), 2, Level::Hard, 4)[0];
        let (outcome, log) = play(spec, &maps()).unwrap();
        assert!(outcome.winner.is_some() && outcome.turns < TURN_CAP, "{outcome:?}");
        assert_eq!(outcome.first, log.setup.order[0]);
        for side in outcome.sides {
            assert_eq!(side.shots.iter().map(|u| u.damage).sum::<i64>(), side.damage);
            assert!(side.shots.iter().map(|u| u.delay).sum::<u64>() <= side.delay, "passes cost delay too");
            assert!(side.hits <= side.turns);
        }
        let loser = 1 - outcome.winner.unwrap();
        assert!(outcome.sides[usize::from(1 - loser)].damage > 0, "the winner hurt somebody");
        assert_eq!(play(spec, &maps()).unwrap(), (outcome, log), "a spec is a game");
    }

    #[test]
    fn runs_come_back_in_order_on_any_number_of_threads() {
        let specs = specs(&[MobileKind::Coilback, MobileKind::Sparkhorn], &flat(), 2, Level::Hard, 9);
        let one = run(&specs, &maps(), 1, &|_| {}).unwrap();
        let many = run(&specs, &maps(), 4, &|_| {}).unwrap();
        assert_eq!(one, many);
        assert_eq!(one.iter().map(|(o, _)| &o.spec).collect::<Vec<_>>(), specs.iter().collect::<Vec<_>>());
    }

    #[test]
    fn a_missing_variant_is_refused() {
        let mut spec = specs(&[MobileKind::Coilback], &flat(), 1, Level::Hard, 1).remove(0);
        spec.variant = "Z".into();
        assert!(play(&spec, &maps()).is_err());
    }
}
