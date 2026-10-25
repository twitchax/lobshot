//! The host's half of a game. The sim never rolls (`DESIGN.md`, "Game model"): whoever hosts a
//! game (the server, the client's practice mode, the lab) brings the dice, sets the game up and
//! answers the rolls it owes. This module is what each roll ranges over and how a host stamps a
//! turn's time, so a game plays by the same rules wherever it is hosted.

use anyhow::ensure;
use serde::{Deserialize, Serialize};

use crate::{
    base::Res,
    drops::{Drop, DropKind},
    game::{Event, MobileKind, Roll, Seat, Setup, SuddenDeath, Team},
    items::Loadout,
    jewels::{JewelDrop, JewelKind},
    map::Arena,
    mode::Mode,
    tuning::jewels,
    units::{Centis, Facing, Wind},
    weather::{Forecast, WeatherKind},
};

/// A host's source of randomness.
pub trait Dice {
    /// A uniform roll in `0..n`; an `n` of 0 counts as 1.
    fn below(&mut self, n: u32) -> u32;

    /// Heads or tails.
    fn coin(&mut self) -> bool {
        self.below(2) == 1
    }
}

/// xorshift64: a tiny, fully specified generator, so a seed means the same rolls on every target.
/// SearchBot's noise, practice mode and the lab roll with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XorShift(u64);

impl XorShift {
    /// Any seed works: xorshift's one dead state, 0, is never reached.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

impl Dice for XorShift {
    fn below(&mut self, n: u32) -> u32 {
        u32::try_from(self.next_u64() % u64::from(n.max(1))).unwrap_or(0)
    }
}

/// Who sits down to a game: a team, a mobile (`None` is the lobby's Random) and a loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entrant {
    pub team: Team,
    pub mobile: Option<MobileKind>,
    pub items: Loadout,
}

/// Which of a map's variants a game plays on: rolled when there is more than one.
///
/// # Errors
///
/// Fails on a map with no variants.
pub fn arena(mut arenas: Vec<Arena>, dice: &mut impl Dice) -> Res<Arena> {
    ensure!(!arenas.is_empty(), "the map has no variants");
    let pick = if arenas.len() > 1 { roll_index(arenas.len(), dice) } else { 0 };
    Ok(arenas.swap_remove(pick))
}

/// Seats `entrants` on `arena` as a host does, to play `mode`: on its spawn columns, shuffled, each
/// mobile facing the middle; Random picks rolled, so the setup holds what each seat really plays; a
/// turn order with the teams alternating (`turn_order`); and the opening wind.
///
/// # Errors
///
/// Fails when the arena has fewer spawns than entrants.
pub fn setup(arena: &Arena, entrants: &[Entrant], max_wind: u8, sudden_death: Option<SuddenDeath>, mode: Mode, dice: &mut impl Dice) -> Res<Setup> {
    ensure!(
        arena.spawns.len() >= entrants.len(),
        "the map has spawns for only {} players, not {}",
        arena.spawns.len(),
        entrants.len()
    );
    let mut spawns = arena.spawns.clone();
    shuffle(&mut spawns, dice);
    let mobiles: Vec<MobileKind> = entrants.iter().map(|e| e.mobile.unwrap_or_else(|| MobileKind::ALL[roll_index(MobileKind::ALL.len(), dice)])).collect();
    let seats = entrants
        .iter()
        .zip(spawns)
        .zip(mobiles)
        .map(|((entrant, spawn_x), mobile)| Seat {
            team: entrant.team,
            mobile,
            spawn_x,
            facing: facing(spawn_x, arena.width),
            items: entrant.items,
        })
        .collect();
    let teams: Vec<Team> = entrants.iter().map(|e| e.team).collect();
    let order = turn_order(&teams, dice);
    Ok(Setup {
        map: arena.map.clone(),
        seats,
        order,
        wind: wind(max_wind, dice),
        sudden_death,
        mode,
    })
}

/// The way a mobile spawning at `spawn_x` faces: toward the middle, where the enemy most likely is.
#[must_use]
pub fn facing(spawn_x: i32, width: u32) -> Facing {
    if 2 * i64::from(spawn_x) < i64::from(width) { Facing::Right } else { Facing::Left }
}

/// Seats (by index into `teams`) in a random order with the teams alternating: each team
/// shuffled, then interleaved, starting with a random team. A bigger team's extra players go last.
pub fn turn_order(teams: &[Team], dice: &mut impl Dice) -> Vec<u8> {
    let mut sides = [Team::A, Team::B].map(|side| (0_u8..).zip(teams).filter(|&(_, &team)| team == side).map(|(seat, _)| seat).collect::<Vec<u8>>());
    for side in &mut sides {
        shuffle(side, dice);
    }
    if dice.coin() {
        sides.swap(0, 1);
    }
    let rounds = sides.iter().map(Vec::len).max().unwrap_or(0);
    (0..rounds).flat_map(|round| sides.iter().filter_map(move |side| side.get(round).copied())).collect()
}

/// Wind of any direction, its strength up to `max`.
pub fn wind(max: u8, dice: &mut impl Dice) -> Wind {
    Wind {
        strength: u8::try_from(dice.below(u32::from(max) + 1)).unwrap_or(max),
        direction: u16::try_from(dice.below(360)).unwrap_or(0),
    }
}

/// The event that answers `roll`: any weather, drop or jewel, anywhere across the map; wind up to
/// `max_wind`.
pub fn answer(roll: Roll, max_wind: u8, dice: &mut impl Dice) -> Event {
    match roll {
        Roll::Weather { width, odds } => {
            let kind = odds.pick(dice.below(odds.total()));
            let x = column(width, dice);
            let hides = (kind == WeatherKind::Random).then(|| WeatherKind::HIDDEN[roll_index(WeatherKind::HIDDEN.len(), dice)]);
            Event::Weather(Forecast { kind, x, hides })
        }
        Roll::Wind => Event::Wind(wind(max_wind, dice)),
        Roll::Drop { width } => Event::Drop(Drop {
            kind: DropKind::ALL[roll_index(DropKind::ALL.len(), dice)],
            x: column(width, dice),
        }),
        Roll::Jewel { width } => Event::Jewel(jewel(width, dice)),
    }
}

/// A jewel falling anywhere across the map: its kind by `tuning::jewels::ODDS`.
fn jewel(width: u32, dice: &mut impl Dice) -> JewelDrop {
    let odds = jewels::ODDS.value;
    let mut roll = dice.below(odds.iter().sum());
    let kind = JewelKind::ALL.into_iter().zip(odds).find(|&(_, weight)| {
        let hit = roll < weight;
        roll = roll.saturating_sub(weight);
        hit
    });
    JewelDrop {
        kind: kind.map_or(JewelKind::ALL[0], |(kind, _)| kind),
        x: column(width, dice),
    }
}

/// `event` stamped with its turn's `time`, as a host stamps what a player or bot did: a shot, a
/// pass or an item carries how long the turn took (the host measured it, not the player); other
/// events carry no time.
#[must_use]
pub fn stamp(event: Event, time: Centis) -> Event {
    match event {
        Event::Shoot { seat, shot, aim, power, .. } => Event::Shoot { seat, shot, aim, power, time },
        Event::Pass { seat, .. } => Event::Pass { seat, time },
        Event::UseItem { seat, slot, .. } => Event::UseItem { seat, slot, time },
        other => other,
    }
}

/// Fisher-Yates.
pub fn shuffle<T>(items: &mut [T], dice: &mut impl Dice) {
    for i in (1..items.len()).rev() {
        items.swap(i, roll_index(i + 1, dice));
    }
}

/// A column in `0..width`.
fn column(width: u32, dice: &mut impl Dice) -> i32 {
    i32::try_from(dice.below(width)).unwrap_or(i32::MAX)
}

/// A uniform index into a list of `len` (at most `u32::MAX` long, as every list here is).
fn roll_index(len: usize, dice: &mut impl Dice) -> usize {
    usize::try_from(dice.below(u32::try_from(len).unwrap_or(u32::MAX))).unwrap_or(0)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::BTreeSet;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{Game, Shot},
        map::{MapRef, NoAssets},
        units::Power,
        weather::Odds,
    };

    fn field() -> Arena {
        Arena {
            map: MapRef::Flat { width: 1600, height: 900, ground: 700 },
            width: 1600,
            spawns: vec![200, 1400, 500, 1100, 350, 1250, 650, 950],
        }
    }

    fn entrants(teams: &[Team]) -> Vec<Entrant> {
        teams
            .iter()
            .map(|&team| Entrant {
                team,
                mobile: None,
                items: Loadout::default(),
            })
            .collect()
    }

    #[test]
    fn xorshift_never_sticks_at_zero_and_rolls_within_range() {
        let mut dice = XorShift::new(0);
        let rolls: Vec<u32> = (0..600).map(|_| dice.below(6)).collect();
        assert!(rolls.iter().all(|&r| r < 6));
        assert_eq!(rolls.iter().collect::<BTreeSet<_>>().len(), 6, "every face comes up");
        assert_eq!(dice.below(0), 0, "an empty range counts as one");
        assert_eq!(XorShift::new(7), XorShift::new(6), "the low bit is forced on");
    }

    #[test]
    fn a_setup_seats_everyone_on_distinct_spawns_facing_the_middle() {
        let arena = field();
        let teams = [Team::A, Team::B, Team::A, Team::B];
        for seed in 0..50 {
            let setup = setup(&arena, &entrants(&teams), 26, None, Mode::Solo, &mut XorShift::new(seed)).unwrap();
            let spawns: BTreeSet<i32> = setup.seats.iter().map(|s| s.spawn_x).collect();
            assert_eq!(spawns.len(), 4);
            assert!(spawns.iter().all(|x| arena.spawns.contains(x)));
            for seat in &setup.seats {
                assert_eq!(seat.facing, if seat.spawn_x < 800 { Facing::Right } else { Facing::Left });
            }
            assert_eq!(setup.seats.iter().map(|s| s.team).collect::<Vec<_>>(), teams);
            assert!(setup.wind.strength <= 26 && setup.wind.direction < 360);
            Game::new(&setup, &NoAssets).unwrap();
        }
    }

    #[test]
    fn picks_stand_and_random_ones_roll_every_mobile() {
        let arena = field();
        let mut chosen = entrants(&[Team::A, Team::B]);
        chosen[0].mobile = Some(MobileKind::Hookclaw);
        let mut rolled = BTreeSet::new();
        for seed in 0..200 {
            let setup = setup(&arena, &chosen, 0, None, Mode::Solo, &mut XorShift::new(seed)).unwrap();
            assert_eq!(setup.seats[0].mobile, MobileKind::Hookclaw);
            rolled.insert(format!("{:?}", setup.seats[1].mobile));
        }
        assert_eq!(rolled.len(), MobileKind::ALL.len());
    }

    #[test]
    fn a_setup_carries_its_mode_into_the_game() {
        let setup = setup(&field(), &entrants(&[Team::A, Team::B, Team::A, Team::B]), 26, None, Mode::Score, &mut XorShift::new(4)).unwrap();
        assert_eq!(setup.mode, Mode::Score);
        let game = Game::new(&setup, &NoAssets).unwrap();
        assert_eq!((game.mode(), game.points(Team::A)), (Mode::Score, Some(3)));
    }

    #[test]
    fn a_setup_needs_a_spawn_per_entrant() {
        let mut arena = field();
        arena.spawns.truncate(1);
        assert!(setup(&arena, &entrants(&[Team::A, Team::B]), 26, None, Mode::Solo, &mut XorShift::new(1)).is_err());
    }

    #[test]
    fn turn_orders_alternate_teams_and_both_teams_go_first() {
        let teams = [Team::A, Team::A, Team::A, Team::B, Team::B];
        let mut firsts = BTreeSet::new();
        for seed in 0..100 {
            let order = turn_order(&teams, &mut XorShift::new(seed));
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, [0, 1, 2, 3, 4]);
            let sides: Vec<Team> = order.iter().map(|&s| teams[usize::from(s)]).collect();
            assert!(sides[..4].windows(2).all(|w| w[0] != w[1]), "{sides:?}");
            assert_eq!(sides[4], Team::A, "the bigger team's extra goes last");
            firsts.insert(sides[0]);
        }
        assert_eq!(firsts.len(), 2);
    }

    #[test]
    fn rolls_range_over_every_kind_and_the_whole_map() {
        let mut dice = XorShift::new(3);
        let (mut weathers, mut drops, mut hidden) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        for _ in 0..400 {
            let Event::Weather(forecast) = answer(Roll::Weather { width: 50, odds: Odds::new(|_| 1) }, 26, &mut dice) else {
                panic!()
            };
            assert!((0..50).contains(&forecast.x));
            assert!(forecast.is_whole(), "a Random forecast says what it hides: {forecast:?}");
            weathers.insert(format!("{:?}", forecast.kind));
            hidden.extend(forecast.hides.map(|kind| format!("{kind:?}")));
            let Event::Drop(drop) = answer(Roll::Drop { width: 50 }, 26, &mut dice) else { panic!() };
            assert!((0..50).contains(&drop.x));
            drops.insert(format!("{:?}", drop.kind));
            let Event::Wind(wind) = answer(Roll::Wind, 5, &mut dice) else { panic!() };
            assert!(wind.strength <= 5 && wind.direction < 360);
        }
        assert_eq!((weathers.len(), drops.len()), (WeatherKind::ALL.len(), DropKind::ALL.len()));
        assert_eq!(hidden.len(), WeatherKind::HIDDEN.len(), "{hidden:?}");
    }

    #[test]
    fn jewels_roll_every_kind_by_its_odds_across_the_whole_map() {
        let mut dice = XorShift::new(11);
        let mut counts = [0_u32; 4];
        for _ in 0..4000 {
            let Event::Jewel(drop) = answer(Roll::Jewel { width: 50 }, 26, &mut dice) else { panic!() };
            assert!((0..50).contains(&drop.x));
            counts[JewelKind::ALL.iter().position(|&kind| kind == drop.kind).unwrap()] += 1;
        }
        let odds = jewels::ODDS.value;
        let total: u32 = odds.iter().sum();
        for (count, weight) in counts.into_iter().zip(odds) {
            let expected = 4000 * weight / total;
            assert!(count.abs_diff(expected) < expected / 5 + 20, "{counts:?} against {odds:?}");
        }
    }

    #[test]
    fn variants_roll_only_when_there_is_a_choice() {
        let mut dice = XorShift::new(9);
        let before = dice;
        assert_eq!(arena(vec![field()], &mut dice).unwrap(), field());
        assert_eq!(dice, before, "one variant rolls nothing");
        assert!(arena(Vec::new(), &mut dice).is_err());
    }

    #[test]
    fn stamps_carry_the_hosts_time_on_timed_events_only() {
        let time = Centis(321);
        let shot = Event::Shoot {
            seat: 1,
            shot: Shot::S2,
            aim: 40,
            power: Power::MAX,
            time: Centis(0),
        };
        assert_eq!(
            stamp(shot, time),
            Event::Shoot {
                seat: 1,
                shot: Shot::S2,
                aim: 40,
                power: Power::MAX,
                time
            }
        );
        assert_eq!(stamp(Event::Pass { seat: 0, time: Centis(5) }, time), Event::Pass { seat: 0, time });
        assert_eq!(stamp(Event::UseItem { seat: 0, slot: 2, time: Centis(0) }, time), Event::UseItem { seat: 0, slot: 2, time });
        assert_eq!(stamp(Event::Spin { seat: 3 }, time), Event::Spin { seat: 3 });
    }
}
