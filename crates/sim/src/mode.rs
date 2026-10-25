//! Game modes (PRD-0007): rules over the same turn order, so replays, bots and the lab play every
//! mode alike. Solo is GunBound's default: one life each, and the last team standing wins. In Score
//! each team has lives (its points), every death costs one, and the dead drop back in where they
//! pick, a few turns later (`docs/research/modes-score.md`). Tag and Jewel are named but not built
//! yet: a game refuses them (`Mode::built`), and so does a room.
//!
//! Each of a mode's rules is decided here, once: whether a team is out of the game (`out`), what a
//! death costs and whether the dead may come back (`died`), and how they come back (`RespawnRule`,
//! `pick`, `land_due`). `Game` holds the state and asks.

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    ballistics::{self, Flight},
    base::Void,
    game::{Death, Effect, Game, Leg, Phase, Seat, Team},
    items::percent_of,
    mines,
    movement::{self, Footing, Landing},
    referee,
    tuning::{
        MAX_FLIGHT_SECONDS, SKY,
        drops::GRAVITY,
        modes::{RESPAWN_HP_PERCENT, RESPAWN_TURNS_BEFORE, RESPAWN_WIND, SCORE_DEATH_COST, SCORE_EXTRA_LIVES, SCORE_RESPAWN_TURNS},
    },
    turn::{self, Clock},
    units::{TICKS_PER_SECOND, Vec2},
};

/// What a game is played for (GunBound's four modes: SW-modes, mechanics.md section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Mode {
    /// One life each: the last team standing wins.
    #[default]
    Solo,
    /// Team lives: every death costs one, and the dead drop back in where they pick.
    Score,
    /// Two mobiles each, switching between them. Not built yet.
    Tag,
    /// Shooting falling jewels for points. Not built yet.
    Jewel,
}

impl Mode {
    /// Every mode, in the order a room's options list them.
    pub const ALL: [Self; 4] = [Self::Solo, Self::Score, Self::Tag, Self::Jewel];

    /// The player-facing name: GunBound's own, which is plain words.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::Score => "Score",
            Self::Tag => "Tag",
            Self::Jewel => "Jewel",
        }
    }

    /// Whether this build plays it: a game refuses one that isn't, and so does a room.
    #[must_use]
    pub const fn built(self) -> bool {
        matches!(self, Self::Solo | Self::Score)
    }

    /// Whether the dead spin the slot machine: in Solo and Tag (WP06). In Score they drop back in
    /// instead, and there is none (GF-kurai, GF-vgg).
    #[must_use]
    pub const fn slot_machine(self) -> bool {
        matches!(self, Self::Solo | Self::Tag)
    }

    /// How the dead come back, in a mode where they do.
    #[must_use]
    pub const fn respawn(self) -> Option<RespawnRule> {
        match self {
            Self::Score => Some(RespawnRule {
                turns: SCORE_RESPAWN_TURNS.value,
                deaths: &[Death::Blasted, Death::Bunged],
            }),
            Self::Solo | Self::Tag | Self::Jewel => None,
        }
    }

    /// Whether each team has points: Score's lives.
    #[must_use]
    pub const fn counts_points(self) -> bool {
        matches!(self, Self::Score)
    }

    /// The points a team of `players` starts with: in Score, a life each and one more.
    const fn starting_points(self, players: i32) -> i32 {
        match self {
            Self::Score => players + SCORE_EXTRA_LIVES.value,
            Self::Solo | Self::Tag | Self::Jewel => 0,
        }
    }

    /// The points a death costs its team.
    const fn death_cost(self) -> i32 {
        match self {
            Self::Score => SCORE_DEATH_COST.value,
            Self::Solo | Self::Tag | Self::Jewel => 0,
        }
    }
}

/// How the dead come back in a mode: a dead player picks a column (`Event::Respawn`), and drops
/// down it from the sky some turns later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RespawnRule {
    /// Turns from a pick to its drop, counted from the turn after the one it was picked in
    /// (SW-modes).
    pub turns: i64,
    /// The deaths that may come back: a player who left never does.
    pub deaths: &'static [Death],
}

/// A mode's state beyond Solo's: each team's points, and the dead who may come back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeState {
    mode: Mode,
    /// Team A's points, then team B's; both 0 in a mode that counts none.
    points: [i32; 2],
    /// The dead who may come back, in the order they died, with where each drops in once picked.
    respawns: Vec<Respawn>,
}

/// A dead seat that may come back, and where it drops in once its player has picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Respawn {
    pub seat: u8,
    pub pick: Option<DropPoint>,
}

/// Where and when a dead seat drops back in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropPoint {
    /// The column it drops down, from the sky.
    pub x: i32,
    /// It lands at the end of the turn that brings the game's turn count (`Game::turns`) to this.
    pub lands: i64,
}

impl ModeState {
    /// `mode`'s state as a game on `seats` starts: each team's points, and no one dead.
    pub(crate) fn new(mode: Mode, seats: &[Seat]) -> Self {
        let starting = |team| mode.starting_points(i32::try_from(seats.iter().filter(|seat| seat.team == team).count()).unwrap_or(0));
        Self {
            mode,
            points: [starting(Team::A), starting(Team::B)],
            respawns: Vec::new(),
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// `team`'s points, in a mode that counts them.
    #[must_use]
    pub fn points(&self, team: Team) -> Option<i32> {
        self.mode.counts_points().then(|| self.points[index(team)])
    }

    /// The dead who may come back, in the order they died.
    #[must_use]
    pub fn respawns(&self) -> &[Respawn] {
        &self.respawns
    }

    fn respawn_mut(&mut self, seat: u8) -> Option<&mut Respawn> {
        self.respawns.iter_mut().find(|respawn| respawn.seat == seat)
    }
}

fn index(team: Team) -> usize {
    match team {
        Team::A => 0,
        Team::B => 1,
    }
}

/// Whether `team` is out of the game: no one of it standing (in Score, everyone dead or waiting to
/// drop in is out too: SW-modes, GF-vgg), or, in Score, no lives left.
pub(crate) fn out(game: &Game, team: Team) -> bool {
    let standing = game.players().iter().any(|player| player.alive && player.team == team);
    match game.mode() {
        Mode::Score => !standing || game.mode_state.points(team).is_some_and(|points| points <= 0),
        Mode::Solo | Mode::Tag | Mode::Jewel => !standing,
    }
}

/// What `seat`'s death costs its team (in Score, a life, however it died), and, where the mode
/// brings that death back, its place among the dead who may come back.
pub(crate) fn died(game: &mut Game, seat: u8, death: Death, effects: &mut Vec<Effect>) {
    let Some(team) = game.players().get(usize::from(seat)).map(|player| player.team) else { return };
    let state = &mut game.mode_state;
    let cost = state.mode.death_cost();
    if cost > 0 {
        let points = &mut state.points[index(team)];
        *points = (*points - cost).max(0);
        effects.push(Effect::Points { team, points: *points });
    }
    if state.mode.respawn().is_some_and(|rule| rule.deaths.contains(&death)) {
        state.respawns.push(Respawn { seat, pick: None });
    }
}

/// A dead seat whose player left: it won't come back. Returns whether it might have.
pub(crate) fn forget(game: &mut Game, seat: u8) -> bool {
    let before = game.mode_state.respawns.len();
    game.mode_state.respawns.retain(|respawn| respawn.seat != seat);
    game.mode_state.respawns.len() < before
}

/// Whether `seat` may pick where to drop back in now (`Event::Respawn`): the game is on, no roll is
/// owed, and it is dead, may come back, and hasn't picked already. The dead pick on anyone's turn,
/// whenever they like (SW-modes): the game never waits for them.
pub(crate) fn check_pick(game: &Game, seat: u8) -> Void {
    ensure!(!matches!(game.phase(), Phase::Over { .. }), "the game is over");
    if let Some(roll) = game.pending_roll() {
        bail!("waiting for the server's {roll:?} roll");
    }
    ensure!(!game.player(seat)?.alive, "seat {seat} is alive: only the dead drop back in");
    ensure!(game.mode().respawn().is_some(), "the dead don't come back in {}", game.mode().name());
    match game.mode_state.respawns.iter().find(|respawn| respawn.seat == seat) {
        None => bail!("seat {seat} left the game: it doesn't come back"),
        Some(Respawn { pick: Some(point), .. }) => bail!("seat {seat} drops in at x = {} already", point.x),
        Some(Respawn { pick: None, .. }) => Ok(()),
    }
}

/// `seat` picks column `x`: it drops in at the end of the rule's `turns`th turn after this one.
pub(crate) fn pick(game: &mut Game, seat: u8, x: i32, effects: &mut Vec<Effect>) {
    let Some(rule) = game.mode().respawn() else { return };
    let lands = game.turns() + rule.turns + 1;
    if let Some(respawn) = game.mode_state.respawn_mut(seat) {
        respawn.pick = Some(DropPoint { x, lands });
        effects.push(Effect::RespawnPicked { seat, x, lands });
    }
}

/// Drops in every dead seat whose turn has come, in the order they died, at the end of a turn
/// before the next starts. Once a team is out, the rest are moot. A drop that misses the ground
/// costs nothing, but its player picks again and waits again (GF-vgg). One that lands on a mine
/// sets it off, as walking onto one does (`mines::trip`).
pub(crate) fn land_due(game: &mut Game, effects: &mut Vec<Effect>) {
    let turns = game.turns();
    let due: Vec<(u8, i32)> = game
        .mode_state
        .respawns
        .iter()
        .filter_map(|respawn| respawn.pick.filter(|point| point.lands <= turns).map(|point| (respawn.seat, point.x)))
        .collect();
    for (seat, x) in due {
        if game.decided() {
            break;
        }
        let (leg, to) = fall(game, x);
        effects.push(Effect::Respawned { seat, legs: vec![leg], to });
        match to {
            Some(footing) => {
                forget(game, seat);
                stand(game, seat, footing);
                if mines::trip(game, leg.to, effects) {
                    game.settle_all(effects);
                }
            }
            None => {
                if let Some(respawn) = game.mode_state.respawn_mut(seat) {
                    respawn.pick = None;
                }
            }
        }
    }
}

/// Where a mobile dropped down column `x` lands: it falls from the sky, the wind nudging it
/// (`RESPAWN_WIND`), to the first ground it touches, or out of the map. Mobiles and weather columns
/// don't stop it.
#[must_use]
pub fn fall(game: &Game, x: i32) -> (Leg, Option<Footing>) {
    let flight = Flight {
        origin: Vec2::new(f64::from(x) + 0.5, -SKY.value),
        velocity: Vec2::ZERO,
        accel: ballistics::acceleration(GRAVITY.value, game.wind(), RESPAWN_WIND.value),
    };
    let terrain = game.terrain();
    let (width, height) = (f64::from(terrain.width()), f64::from(terrain.height()));
    let leg = |to| Leg { flight, from: 0, to, offset: 0 };
    let last = MAX_FLIGHT_SECONDS.value * TICKS_PER_SECOND;
    for tick in 0..=last {
        let at = flight.at(tick);
        if at.x < 0.0 || at.x >= width || at.y >= height {
            return (leg(tick), None);
        }
        // In range: x is inside the map, and y above its bottom.
        #[allow(clippy::cast_possible_truncation)]
        let (column, row) = (at.x.floor() as i32, at.y.floor() as i32);
        if terrain.is_solid(column, row) {
            let to = match movement::settle(terrain, Footing { x: column, y: row }) {
                Landing::On(footing) => Some(footing),
                Landing::OffMap => None,
            };
            return (leg(tick), to);
        }
    }
    (leg(last), None)
}

/// Brings `seat` back at `footing`: alive with its HP and shield refilled (`RESPAWN_HP_PERCENT`),
/// facing the middle as a spawn does, and its turn the one after the next mover's
/// (`RESPAWN_TURNS_BEFORE`). Everything else stands as it was: items used stay used, an SS lock
/// holds, and so does brittleness (GF-lee). Its mines don't come back: they went with its death
/// (`mines::orphan`).
pub(crate) fn stand(game: &mut Game, seat: u8, footing: Footing) {
    let clock = comeback_clock(game, seat);
    let width = game.terrain().width();
    let player = &mut game.players[usize::from(seat)];
    let tuning = player.tuning();
    player.alive = true;
    player.hp = percent_of(tuning.hp.value, RESPAWN_HP_PERCENT.value).max(1);
    player.shield = percent_of(tuning.shield.value, RESPAWN_HP_PERCENT.value);
    player.facing = referee::facing(footing.x, width);
    player.clock = clock;
    game.place(seat, footing);
}

/// A returning seat's clock: level with the delay of whoever moves `RESPAWN_TURNS_BEFORE`th among
/// the living, and the latest to act, so the tie goes to them.
fn comeback_clock(game: &Game, seat: u8) -> Clock {
    let living = game.players().iter().enumerate().filter(|&(other, player)| player.alive && other != usize::from(seat));
    let order = turn::order(living.map(|(other, player)| (other, &player.clock)));
    let ahead = order.get(RESPAWN_TURNS_BEFORE.value.saturating_sub(1)).or(order.last());
    Clock {
        delay: ahead.map_or(0, |&other| game.players()[other].clock.delay),
        last_turn: game.turns(),
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        base::Res,
        game::{
            Event, MobileKind, Setup,
            testing::{answer_rolls, duel_setup, game_in, pass, seat},
        },
        map::NoAssets,
        terrain::Terrain,
        tuning::frostfin,
        units::{Facing, Wind},
    };

    /// Where the pit is: a shaft off the map just right of seat 0, at x = 300.
    const PIT: std::ops::Range<i32> = 330..350;

    /// Flat ground at y = 500 with a shaft through it at `PIT`.
    fn pitted() -> Terrain {
        Terrain::from_fn(2400, 600, |x, y| y >= 500 && !PIT.contains(&i32::try_from(x).unwrap())).unwrap()
    }

    /// A `mode` game on `pitted` ground, Coilbacks at `spawns` alternating teams from A, acting in
    /// seat order.
    fn game(mode: Mode, spawns: &[i32]) -> Game {
        let seats = (0..spawns.len())
            .map(|i| {
                let team = if i % 2 == 0 { Team::A } else { Team::B };
                seat(team, MobileKind::Coilback, spawns[i], Facing::Right)
            })
            .collect();
        game_in(mode, pitted(), seats)
    }

    /// A Score 2v2: seats 0 and 2 on team A, 1 and 3 on B.
    fn score_2v2() -> Game {
        game(Mode::Score, &[300, 900, 1500, 2100])
    }

    /// Seat 0 walks into the pit on its turn, which ends it.
    fn bunge_seat_0(game: &mut Game) -> Vec<Effect> {
        let effects = game
            .apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 60,
            })
            .unwrap();
        answer_rolls(game);
        effects
    }

    fn respawn(game: &mut Game, seat: u8, x: i32) -> Res<Vec<Effect>> {
        game.apply(Event::Respawn { seat, x })
    }

    /// Passes for whoever's turn it is, `turns` times, and returns everything that happened.
    fn pass_turns(game: &mut Game, turns: usize) -> Vec<Effect> {
        (0..turns)
            .flat_map(|_| {
                let Phase::Turn { seat } = game.phase() else { panic!("the game is over") };
                pass(game, seat)
            })
            .collect()
    }

    fn turn_starts(effects: &[Effect]) -> Vec<u8> {
        effects.iter().filter_map(|e| if let Effect::TurnStarted { seat } = e { Some(*seat) } else { None }).collect()
    }

    #[test]
    fn modes_have_their_own_names_and_solo_and_score_are_built() {
        let mut names: Vec<&str> = Mode::ALL.iter().map(|mode| mode.name()).collect();
        names.dedup();
        assert_eq!(names, ["Solo", "Score", "Tag", "Jewel"]);
        assert_eq!(Mode::default(), Mode::Solo);
        assert_eq!(Mode::ALL.map(Mode::built), [true, true, false, false]);
        for mode in [Mode::Tag, Mode::Jewel] {
            let setup = Setup { mode, ..duel_setup(800) };
            let error = Game::new(&setup, &NoAssets).unwrap_err().to_string();
            assert_eq!(error, format!("{} mode isn't built yet", mode.name()));
        }
    }

    #[test]
    fn score_teams_start_with_a_life_a_player_and_one_more() {
        let game = score_2v2();
        assert_eq!((game.points(Team::A), game.points(Team::B)), (Some(3), Some(3)));
        let uneven = self::game(Mode::Score, &[300, 900, 1500]);
        assert_eq!((uneven.points(Team::A), uneven.points(Team::B)), (Some(3), Some(2)), "each team counts its own");
        let solo = self::game(Mode::Solo, &[300, 900]);
        assert_eq!(solo.points(Team::A), None, "Solo counts no points");
        assert_eq!(solo.mode(), Mode::Solo);
    }

    #[test]
    fn a_death_costs_its_team_a_life_and_the_dead_may_come_back() {
        let mut game = score_2v2();
        let effects = bunge_seat_0(&mut game);
        let died = effects.iter().position(|e| *e == Effect::Died { seat: 0, death: Death::Bunged }).unwrap();
        assert_eq!(effects[died + 1], Effect::Points { team: Team::A, points: 2 }, "{effects:?}");
        assert_eq!(game.points(Team::A), Some(2));
        assert_eq!(game.points(Team::B), Some(3));
        assert_eq!(game.respawns(), [Respawn { seat: 0, pick: None }]);
        assert!(game.can_respawn(0));
        assert!(!game.can_respawn(1), "the living don't");
        assert!(matches!(game.phase(), Phase::Turn { seat: 1 }), "the game goes on: seat 2 still stands");
    }

    #[test]
    fn a_pick_drops_in_at_the_end_of_the_fourth_turn_after_the_one_it_was_made_in() {
        let mut game = score_2v2();
        bunge_seat_0(&mut game);
        // Seat 1's turn is the first after the death; the pick comes during it, so it doesn't count.
        let turns = game.turns();
        assert_eq!(respawn(&mut game, 0, 1000).unwrap(), [Effect::RespawnPicked { seat: 0, x: 1000, lands: turns + 5 }]);
        assert_eq!(game.respawns()[0].pick, Some(DropPoint { x: 1000, lands: turns + 5 }));
        assert!(!game.can_respawn(0), "one pick at a time");
        let waiting = pass_turns(&mut game, 4);
        assert!(!waiting.iter().any(|e| matches!(e, Effect::Respawned { .. })), "{waiting:?}");
        assert!(!game.players()[0].alive);
        let landing = pass_turns(&mut game, 1);
        let Some(Effect::Respawned { seat: 0, legs, to }) = landing.iter().find(|e| matches!(e, Effect::Respawned { .. })) else {
            panic!("no drop: {landing:?}")
        };
        assert_eq!(*to, Some(Footing { x: 1000, y: 500 }), "straight down in no wind");
        assert!(legs[0].flight.origin.y < 0.0, "from the sky");
        let back = game.players()[0];
        assert!(back.alive && back.footing == Footing { x: 1000, y: 500 });
        assert_eq!((back.hp, back.shield), (back.tuning().hp.value, back.tuning().shield.value), "whole again");
        assert_eq!(back.facing, Facing::Right, "toward the middle (x = 1200), as a spawn faces");
        assert_eq!(game.respawns(), []);
        assert_eq!(game.points(Team::A), Some(2), "coming back is free");
    }

    #[test]
    fn a_respawned_mobile_moves_second_after_it_lands() {
        let mut game = score_2v2();
        bunge_seat_0(&mut game);
        respawn(&mut game, 0, 1000).unwrap();
        // Everyone passes alike, so the next mover after the landing has the least delay and
        // seat 0 comes level with it, losing the tie.
        let mut after = pass_turns(&mut game, 5);
        after.extend(pass_turns(&mut game, 2));
        let starts = turn_starts(&after);
        let landed = after.iter().position(|e| matches!(e, Effect::Respawned { .. })).unwrap();
        let next = turn_starts(&after[landed..]);
        assert_eq!(next.len(), 3, "{starts:?}");
        assert_ne!(next[0], 0);
        assert_eq!(next[1], 0, "the second turn after it lands is its own: {next:?}");
    }

    #[test]
    fn a_drop_that_misses_the_ground_costs_nothing_and_its_player_picks_again() {
        let mut game = score_2v2();
        bunge_seat_0(&mut game);
        respawn(&mut game, 0, PIT.start + 5).unwrap();
        let effects = pass_turns(&mut game, 5);
        assert!(effects.iter().any(|e| matches!(e, Effect::Respawned { seat: 0, to: None, .. })), "{effects:?}");
        assert!(!game.players()[0].alive);
        assert_eq!(game.points(Team::A), Some(2), "no life lost (GF-vgg)");
        assert_eq!(game.respawns(), [Respawn { seat: 0, pick: None }]);
        assert!(game.can_respawn(0), "it picks again");
    }

    #[test]
    fn the_wind_nudges_a_drop_a_little() {
        let mut game = score_2v2();
        game.wind = Wind::new(26, 0).unwrap();
        let (leg, to) = fall(&game, 1000);
        let drift = to.unwrap().x - 1000;
        assert!((10..=40).contains(&drift), "a small push downwind at the strongest wind: {drift} px");
        assert!(leg.to > 0);
        game.wind = Wind::new(26, 180).unwrap();
        assert!(fall(&game, 1000).1.unwrap().x < 1000, "the other way in the other wind");
    }

    #[test]
    fn a_team_with_no_one_standing_loses_with_lives_left() {
        // 1v1 Score plays as Solo (NAMU): the lone player's death leaves its team no one standing.
        let mut game = game(Mode::Score, &[300, 1500]);
        let effects = bunge_seat_0(&mut game);
        assert_eq!(game.points(Team::A), Some(1));
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::B) });
        assert!(effects.contains(&Effect::GameOver { winner: Some(Team::B) }));
        assert!(!game.can_respawn(0), "the game is over");
    }

    #[test]
    fn a_team_out_of_lives_loses_though_it_still_stands() {
        let mut game = score_2v2();
        game.mode_state.points = [1, 3];
        bunge_seat_0(&mut game);
        assert!(game.players()[2].alive, "seat 2 still stands");
        assert_eq!(game.points(Team::A), Some(0));
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::B) });
    }

    #[test]
    fn the_dead_spin_in_solo_and_drop_back_in_score() {
        let mut solo = game(Mode::Solo, &[300, 900, 1500, 2100]);
        bunge_seat_0(&mut solo);
        assert!(solo.can_spin(0) && !solo.can_respawn(0));
        assert_eq!(respawn(&mut solo, 0, 1000).unwrap_err().to_string(), "the dead don't come back in Solo");
        assert!(solo.respawns().is_empty() && solo.points(Team::A).is_none(), "a Solo death costs nothing");

        let mut score = score_2v2();
        bunge_seat_0(&mut score);
        assert!(!score.can_spin(0), "no slot machine in Score");
        let error = score.apply(Event::Spin { seat: 0 }).unwrap_err().to_string();
        assert_eq!(error, "there's no slot machine in Score: the dead drop back in");
    }

    #[test]
    fn picks_are_the_deads_once_and_on_the_map() {
        let mut game = score_2v2();
        assert_eq!(respawn(&mut game, 1, 1000).unwrap_err().to_string(), "seat 1 is alive: only the dead drop back in");
        bunge_seat_0(&mut game);
        assert_eq!(respawn(&mut game, 0, 2400).unwrap_err().to_string(), "drop point x 2400 is off the map");
        assert!(respawn(&mut game, 0, -1).is_err());
        let before = game.clone();
        assert!(respawn(&mut game, 0, 2399).is_ok(), "the last column is on the map");
        assert_eq!(respawn(&mut game, 0, 500).unwrap_err().to_string(), "seat 0 drops in at x = 2399 already");
        assert_ne!(game, before);
    }

    #[test]
    fn a_player_who_leaves_never_comes_back() {
        // Alive: a forfeit is a death that costs a life, and it doesn't come back.
        let mut game = score_2v2();
        let effects = game.apply(Event::Forfeit { seat: 1 }).unwrap();
        assert!(effects.contains(&Effect::Points { team: Team::B, points: 2 }), "{effects:?}");
        assert!(!game.can_respawn(1) && !game.can_forfeit(1));
        assert_eq!(respawn(&mut game, 1, 1000).unwrap_err().to_string(), "seat 1 left the game: it doesn't come back");
        // Dead and waiting to drop in: leaving drops the pick.
        bunge_seat_0(&mut game);
        respawn(&mut game, 0, 1000).unwrap();
        assert!(game.can_forfeit(0));
        assert_eq!(game.apply(Event::Forfeit { seat: 0 }).unwrap(), []);
        assert_eq!(game.respawns(), []);
        assert_eq!(game.points(Team::A), Some(2), "its death was counted once");
        assert!(game.apply(Event::Forfeit { seat: 0 }).is_err(), "already out");
    }

    #[test]
    fn a_respawned_mobile_keeps_its_items_ss_lock_and_brittleness() {
        let mut game = score_2v2();
        let items = game.players[0].items;
        game.players[0].items[0] = None;
        game.players[0].ss_lock = 2;
        game.players[0].brittle = frostfin::BRITTLE_PER_HIT.value;
        bunge_seat_0(&mut game);
        respawn(&mut game, 0, 1000).unwrap();
        pass_turns(&mut game, 5);
        let back = game.players()[0];
        assert!(back.alive);
        assert_eq!(back.items[1..], items[1..]);
        assert_eq!(back.items[0], None, "used stays used");
        assert_eq!(back.ss_lock, 2, "the lock stands");
        assert_eq!(back.brittle, frostfin::BRITTLE_PER_HIT.value, "brittleness carries between lives (GF-lee)");
    }

    #[test]
    fn a_drop_onto_a_mine_sets_it_off() {
        let mut game = score_2v2();
        // Seat 3's mine, out of every mobile's sense, where seat 0 will drop.
        let mut laid = Vec::new();
        mines::lay(&mut game, 3, Vec2::new(1200.5, 499.5), Facing::Left, 0, &mut laid);
        let mine = game.mines()[0];
        bunge_seat_0(&mut game);
        respawn(&mut game, 0, mine.at.x).unwrap();
        let effects = pass_turns(&mut game, 5);
        let landed = effects.iter().position(|e| matches!(e, Effect::Respawned { seat: 0, to: Some(_), .. })).unwrap();
        let after = &effects[landed..];
        let Effect::Respawned { legs, .. } = &after[0] else { unreachable!() };
        assert!(
            after.contains(&Effect::MineGone { mine: mine.id, tick: Some(legs[0].to) }),
            "it goes off as the mobile lands: {after:?}"
        );
        assert!(after.iter().any(|e| matches!(e, Effect::Damaged { seat: 0, .. })), "{after:?}");
        assert_eq!(game.mines(), [], "{after:?}");
        assert!(game.players()[0].alive, "one mine's blast doesn't kill a whole mobile");
    }

    #[test]
    fn a_layers_mines_go_with_its_death_and_dont_come_back_with_it() {
        let mut game = score_2v2();
        let mut laid = Vec::new();
        mines::lay(&mut game, 0, Vec2::new(1200.5, 499.5), Facing::Right, 0, &mut laid);
        let mine = game.mines()[0];
        let effects = bunge_seat_0(&mut game);
        assert!(effects.contains(&Effect::MineGone { mine: mine.id, tick: None }), "{effects:?}");
        respawn(&mut game, 0, 600).unwrap();
        pass_turns(&mut game, 5);
        assert!(game.players()[0].alive);
        assert_eq!(game.mines(), [], "a new life lays its own");
    }

    #[test]
    fn a_pick_works_on_anyones_turn_and_the_game_never_waits_for_one() {
        let mut game = score_2v2();
        bunge_seat_0(&mut game);
        // No one picks for seat 0: the living play on.
        let effects = pass_turns(&mut game, 6);
        assert!(!turn_starts(&effects).contains(&0));
        assert!(game.can_respawn(0), "it may still pick, whenever it likes");
        assert!(matches!(game.phase(), Phase::Turn { seat } if seat != 0));
        assert!(respawn(&mut game, 0, 1000).is_ok());
    }
}
