//! A game in a room: setting it up from the lobby, turning members' actions into sim events,
//! answering the rolls the sim asks for, the turn clock, and bot turns.
//!
//! Every accepted event goes through `commit`: applied to the server's `Game`, appended to the log
//! and broadcast with the state hash after it. Then `settle` answers whatever rolls the sim now
//! owes (`DESIGN.md`, "Game model": the server owns randomness), plays dead bots (they spin their
//! slot machines, or pick where to drop back in), and starts the next turn or ends the game. A turn's clock starts once clients have shown the
//! turn starting (`Showing`): the sim resolves a shot at once, but a player watches it fly.

use std::time::{Duration, Instant};

use anyhow::{Context as _, bail, ensure};
use rand::{RngExt as _, rngs::StdRng, seq::SliceRandom as _};

use lobshot_protocol::{Action, MemberId, ServerMsg, pacing};
use lobshot_sim::{
    base::Void,
    bot,
    game::{Effect, Event, Game, Phase, Team},
    log::Log,
    referee::{self, Dice, Entrant},
    units::Centis,
};

use super::{BotTurn, Output, Room};

/// The room's rng as the referee's dice.
struct Rolls<'a>(&'a mut StdRng);

impl Dice for Rolls<'_> {
    fn below(&mut self, n: u32) -> u32 {
        self.0.random_range(0..n.max(1))
    }
}

/// The game a room is playing.
#[derive(Debug)]
pub(super) struct Play {
    pub game: Game,
    pub log: Log,
    /// Who plays each seat.
    pub seats: Vec<MemberId>,
    pub turn: Option<Turn>,
    /// The turn timer.
    pub limit: Duration,
    pub showing: Showing,
}

/// What clients are showing. They play each event's scene in turn (`lobshot_protocol::pacing`),
/// each once it has arrived and the one before it is over, and start a turn's clock as the scene
/// holding its `TurnStarted` ends. So a scene in the middle of a turn (a dead player's spin) holds
/// up whatever is committed after it, but never the turn already running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Showing {
    /// When the last scene queued ends.
    until: Instant,
    /// When the scene that started the latest turn ends.
    turn: Instant,
}

impl Showing {
    fn new(now: Instant) -> Self {
        Self { until: now, turn: now }
    }

    /// Queues the scene for an event's effects, committed at `now`.
    fn queue(&mut self, effects: &[Effect], now: Instant) {
        self.until = self.until.max(now) + scene(effects);
        if effects.iter().any(|effect| matches!(effect, Effect::TurnStarted { .. })) {
            self.turn = self.until;
        }
    }
}

/// How long clients take to show one event's effects.
fn scene(effects: &[Effect]) -> Duration {
    Duration::try_from_secs_f32(pacing::schedule(effects).end).unwrap_or_default()
}

/// The turn being played.
#[derive(Debug, Clone, Copy)]
pub(super) struct Turn {
    /// Names this turn among every turn the room has run.
    serial: u64,
    seat: u8,
    /// `Game::turns` when it began; a change means it ended.
    turns: i64,
    started: Instant,
    /// For a bot's seat, where its turn is.
    bot: Option<Bot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bot {
    /// Waiting out the think delay.
    Thinking { until: Instant },
    /// Its plan is being searched off the room's task.
    Planning,
}

impl Play {
    /// The turn timer's expiry, or the bot's think delay, whichever is sooner.
    pub fn deadline(&self) -> Option<Instant> {
        let turn = self.turn?;
        let timer = turn.started + self.limit;
        Some(match turn.bot {
            Some(Bot::Thinking { until }) => until.min(timer),
            _ => timer,
        })
    }

    /// Turn time as the sim counts it: centiseconds since the turn began, at most the timer.
    fn elapsed(&self, now: Instant) -> Centis {
        let elapsed = self.turn.map_or(Duration::ZERO, |turn| now.saturating_duration_since(turn.started)).min(self.limit);
        Centis(u32::try_from(elapsed.as_millis() / 10).unwrap_or(u32::MAX))
    }

    /// Whether `turn` is over: its player acted, or the game ended.
    fn moved_on(&self, turn: &Turn) -> bool {
        self.game.turns() != turn.turns || matches!(self.game.phase(), Phase::Over { .. })
    }
}

impl Room {
    /// The master starts a game once everyone seated is ready: builds the setup (rolling spawns,
    /// turn order and the opening wind), sends everyone the log, and answers the opening rolls.
    pub(super) fn start(&mut self, id: MemberId, now: Instant) -> Void {
        self.ensure_master(id)?;
        self.ensure_lobby()?;
        let seated: Vec<usize> = (0..self.members.len()).filter(|&i| self.members[i].member.team.is_some()).collect();
        if let Some(waiting) = seated.iter().map(|&i| &self.members[i].member).find(|m| !m.ready) {
            bail!("{} isn't ready", waiting.name);
        }
        let teams: Vec<Team> = if self.options.random_teams {
            self.random_teams(seated.len())
        } else {
            seated.iter().filter_map(|&i| self.members[i].member.team).collect()
        };
        for team in [Team::A, Team::B] {
            let players = teams.iter().filter(|&&t| t == team).count();
            ensure!(players > 0, "team {team:?} has no players");
            ensure!(
                players <= usize::from(self.options.team_size),
                "team {team:?} has {players} players, but teams are {}",
                self.options.team_size
            );
        }

        // The referee seats everyone and rolls the variant, spawns, Random picks, order and wind.
        let entrants: Vec<Entrant> = seated
            .iter()
            .zip(&teams)
            .map(|(&i, &team)| Entrant {
                team,
                mobile: self.members[i].member.mobile,
                items: self.members[i].member.items,
                second: self.members[i].member.second,
            })
            .collect();
        let mut dice = Rolls(&mut self.rng);
        let setup = referee::arena(self.maps.arenas(&self.options.map)?, &mut dice)
            .and_then(|arena| referee::setup(&arena, &entrants, self.options.max_wind, self.options.sudden_death, self.options.mode, &mut dice))
            .with_context(|| format!("seating a game on {}", self.options.map))?;
        let game = Game::new(&setup, self.maps.as_ref()).context("setting up the game")?;

        for (&i, &team) in seated.iter().zip(&teams) {
            self.members[i].member.team = Some(team);
        }
        let log = Log { setup, events: Vec::new() };
        self.play = Some(Play {
            game,
            log: log.clone(),
            seats: seated.iter().map(|&i| self.members[i].member.id).collect(),
            turn: None,
            limit: Duration::from_secs(u64::from(self.options.turn_seconds)),
            showing: Showing::new(now),
        });
        tracing::info!(
            room = self.code.as_str(),
            players = seated.len(),
            map = self.options.map,
            mode = self.options.mode.name(),
            "game started"
        );
        self.broadcast_room();
        self.broadcast(ServerMsg::Game { log });
        self.settle(now);
        Ok(())
    }

    /// `n` teams split evenly at random; a random team takes the odd player.
    fn random_teams(&mut self, n: usize) -> Vec<Team> {
        let first = if self.rng.random() { Team::A } else { Team::B };
        let second = if first == Team::A { Team::B } else { Team::A };
        let mut teams: Vec<Team> = (0..n).map(|i| if i % 2 == 0 { first } else { second }).collect();
        teams.shuffle(&mut self.rng);
        teams
    }

    /// A move from the member in the current seat.
    pub(super) fn act(&mut self, id: MemberId, action: Action, now: Instant) -> Void {
        let play = self.play.as_ref().context("no game is on")?;
        let seat = play.seats.iter().position(|&member| member == id).context("you aren't playing in this game")?;
        let seat = u8::try_from(seat).context("too many seats")?;
        let time = play.elapsed(now);
        self.commit(action.event(seat, time), now)?;
        self.settle(now);
        Ok(())
    }

    /// A kicked member's mobile dies where it stands, if they are playing and it is alive; dead and
    /// due to drop back in (Score), it never does.
    pub(super) fn forfeit(&mut self, id: MemberId, now: Instant) -> Void {
        let Some(play) = &self.play else { return Ok(()) };
        let Some(seat) = play.seats.iter().position(|&member| member == id) else { return Ok(()) };
        let seat = u8::try_from(seat).context("too many seats")?;
        if !play.game.can_forfeit(seat) {
            return Ok(());
        }
        self.commit(Event::Forfeit { seat }, now)?;
        self.settle(now);
        Ok(())
    }

    /// The turn timer ran out (the server passes for the player), or a bot finished thinking.
    pub(super) fn play_tick(&mut self, now: Instant) {
        let Some(play) = &mut self.play else { return };
        let Some(turn) = play.turn else { return };
        if now >= turn.started + play.limit {
            let time = play.elapsed(now);
            self.commit_or_abort(Event::Pass { seat: turn.seat, time }, now);
            self.settle(now);
        } else if let Some(Bot::Thinking { until }) = turn.bot
            && now >= until
        {
            let game = Box::new(play.game.clone());
            play.turn = Some(Turn { bot: Some(Bot::Planning), ..turn });
            let seed = self.rng.random();
            self.out.push(Output::Plan(BotTurn {
                turn: turn.serial,
                seat: turn.seat,
                game,
                seed,
            }));
        }
    }

    /// A bot's plan for turn `serial` came back: play it, stamping the turn time on its shot. A
    /// plan that comes back after its turn ended (a kick, the timer) is dropped; one that doesn't
    /// end the turn becomes a pass.
    pub(super) fn play_planned(&mut self, serial: u64, events: Vec<Event>, now: Instant) {
        let Some(play) = &self.play else { return };
        let Some(turn) = play.turn.filter(|turn| turn.serial == serial && turn.bot == Some(Bot::Planning)) else {
            return;
        };
        let time = play.elapsed(now);
        for event in events {
            let event = referee::stamp(event, time);
            if let Err(error) = self.commit(event, now) {
                tracing::warn!(%error, ?event, "the game refused a bot's planned event");
                break;
            }
            if self.play.as_ref().is_none_or(|play| play.moved_on(&turn)) {
                break;
            }
        }
        if self.play.as_ref().is_some_and(|play| !play.moved_on(&turn)) {
            self.commit_or_abort(Event::Pass { seat: turn.seat, time }, now);
        }
        self.settle(now);
    }

    /// Applies an accepted event, logs it and broadcasts it with the state hash after it.
    fn commit(&mut self, event: Event, now: Instant) -> Void {
        let play = self.play.as_mut().context("no game is on")?;
        let index = u32::try_from(play.log.events.len()).context("the log is full")?;
        let effects = play.game.apply(event)?;
        play.showing.queue(&effects, now);
        play.log.events.push(event);
        // The event is applied, so it must go out either way; a zero hash makes every client
        // report a desync rather than drift silently.
        let hash = play.game.state_hash().unwrap_or_else(|error| {
            tracing::error!(%error, "hashing the game state");
            0
        });
        self.broadcast(ServerMsg::Event { index, event, hash });
        Ok(())
    }

    /// Commits an event the server itself chose (a roll, a timeout's pass). The sim refusing one
    /// is a bug; ending the game beats leaving it stuck with nobody able to move.
    fn commit_or_abort(&mut self, event: Event, now: Instant) {
        if let Err(error) = self.commit(event, now) {
            tracing::error!(room = self.code.as_str(), %error, ?event, "the game refused a server event; ending it");
            self.finish(now);
        }
    }

    /// After an event: answers every roll the sim now owes, plays every dead bot the game lets act,
    /// then starts the next turn or, if the game is over, returns to the lobby.
    fn settle(&mut self, now: Instant) {
        self.answer_rolls(now);
        self.play_dead_bots(now);
        let Some(play) = &self.play else { return };
        match play.game.phase() {
            Phase::Over { winner } => {
                tracing::info!(room = self.code.as_str(), ?winner, turns = play.game.turns(), "game over");
                self.finish(now);
            }
            Phase::Turn { seat } => {
                if play.turn.is_none_or(|turn| play.moved_on(&turn)) {
                    self.begin_turn(seat, now);
                }
            }
        }
    }

    fn answer_rolls(&mut self, now: Instant) {
        while let Some(roll) = self.play.as_ref().and_then(|play| play.game.pending_roll()) {
            let event = referee::answer(roll, self.options.max_wind, &mut Rolls(&mut self.rng));
            self.commit_or_abort(event, now);
        }
    }

    /// A dead bot acts whenever the game lets it, on anyone's turn, as a player would: it spins its
    /// slot machine, or picks where to drop back in (`bot::while_dead`).
    fn play_dead_bots(&mut self, now: Instant) {
        let seats = self.play.as_ref().map_or(0, |play| play.seats.len());
        for seat in (0..seats).filter_map(|seat| u8::try_from(seat).ok()) {
            let Some(play) = self.play.as_ref() else { return };
            if !play.seats.get(usize::from(seat)).is_some_and(|&id| self.is_bot(id)) {
                continue;
            }
            let Some(event) = bot::while_dead(&play.game, seat) else { continue };
            // The game said the bot could act, so refusing it is a sim bug, but one that leaves
            // nobody stuck: the game goes on without it.
            if let Err(error) = self.commit(event, now) {
                tracing::warn!(room = self.code.as_str(), %error, ?event, "the game refused a dead bot");
                continue;
            }
            self.answer_rolls(now);
        }
    }

    fn is_bot(&self, id: MemberId) -> bool {
        self.members.iter().any(|e| e.member.id == id && e.member.bot)
    }

    fn begin_turn(&mut self, seat: u8, now: Instant) {
        self.turns_started += 1;
        let bot = self.play.as_ref().and_then(|play| play.seats.get(usize::from(seat))).is_some_and(|&id| self.is_bot(id));
        let Some(play) = &mut self.play else { return };
        // The clock starts as clients show the turn starting, not when the sim started it.
        let started = if self.timing.playback_wait { play.showing.turn.max(now) } else { now };
        play.turn = Some(Turn {
            serial: self.turns_started,
            seat,
            turns: play.game.turns(),
            started,
            bot: bot.then_some(Bot::Thinking { until: started + self.timing.bot_delay }),
        });
    }

    /// Back to the lobby: everyone readies up again for the next game.
    fn finish(&mut self, now: Instant) {
        self.play = None;
        self.lobby_since = now;
        for entry in self.members.iter_mut().filter(|e| e.is_human()) {
            entry.member.ready = false;
        }
        self.broadcast_room();
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::{collections::HashSet, path::Path};

    use pretty_assertions::assert_eq;

    use tempfile::TempDir;

    use lobshot_protocol::{ClientMsg, RoomOptions, Stage};
    use lobshot_sim::{
        bot,
        game::{MobileKind, Shot, SuddenDeath, SuddenDeathKind},
        items::{Item, Loadout},
        map::{MapRef, NoAssets},
        mode::Mode,
        terrain::Terrain,
        units::{Facing, Power, TICKS_PER_SECOND, Vec2},
    };

    use super::*;
    use crate::{
        room::{ConnId, Timing, testing::Harness},
        tests::{Replica, write_map, write_map_variants},
    };

    fn options(team_size: u8) -> RoomOptions {
        RoomOptions {
            map: "flat".into(),
            team_size,
            max_wind: 12,
            turn_seconds: 10,
            random_teams: false,
            sudden_death: None,
            mode: Mode::Solo,
        }
    }

    /// Two humans on conns 1 (team A, master) and 2 (team B), ready on `flat`.
    fn duel() -> (Harness, MemberId, MemberId) {
        let mut h = Harness::new();
        let a = h.join(1, "A");
        let b = h.join(2, "B");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::Ready(true));
        h.send(2, ClientMsg::Ready(true));
        (h, a, b)
    }

    fn log(h: &Harness) -> Log {
        h.room.play.as_ref().unwrap().log.clone()
    }

    fn game(h: &Harness) -> &Game {
        &h.room.play.as_ref().unwrap().game
    }

    fn alive(h: &Harness, seat: u8) -> bool {
        game(h).players()[usize::from(seat)].alive
    }

    /// `n` humans on conns 1 to `n` (1 the master), seated in joining order, alternating teams,
    /// all ready to play with `options`. Once the game starts, seat `i` is conn `i + 1`.
    fn humans(h: &mut Harness, n: u64, options: RoomOptions) -> Vec<MemberId> {
        let mut people = vec![h.join(1, "P1")];
        h.send(1, ClientMsg::Configure(options));
        people.extend((2..=n).map(|conn| h.join(conn, &format!("P{conn}"))));
        for conn in 1..=n {
            h.send(conn, ClientMsg::Ready(true));
        }
        people
    }

    /// Whose turn it is, and the conn of the human playing it among `people` (see `humans`).
    fn turn_of(h: &Harness, people: &[MemberId]) -> (u8, Option<u64>) {
        let Phase::Turn { seat } = game(h).phase() else { panic!("the game is over") };
        let id = h.room.play.as_ref().unwrap().seats[usize::from(seat)];
        (seat, people.iter().position(|&p| p == id).map(|i| u64::try_from(i).unwrap() + 1))
    }

    /// Where each seat spawns on `pits`.
    const PIT_SPAWNS: [i32; 8] = [100, 300, 500, 700, 900, 1100, 1300, 1500];

    /// Writes `pits` into `dist`: a field with a shaft down off the map ten steps right of each
    /// spawn, so a player who walks right falls to their death, as in battle.
    fn write_pits(dist: &Path) {
        let shaft = |x: u32| PIT_SPAWNS.iter().any(|&spawn| (spawn + 10..spawn + 13).contains(&i32::try_from(x).unwrap()));
        let mask = Terrain::from_fn(1600, 600, |x, y| y >= 400 && !shaft(x)).unwrap();
        write_map(dist, "pits", &mask, &PIT_SPAWNS).unwrap();
    }

    fn pits(team_size: u8) -> RoomOptions {
        RoomOptions { map: "pits".into(), ..options(team_size) }
    }

    fn score(options: RoomOptions) -> RoomOptions {
        RoomOptions { mode: Mode::Score, ..options }
    }

    /// Passes for whoever's turn it is among `people` until `seat` is back on its feet.
    fn pass_until_alive(h: &mut Harness, people: &[MemberId], seat: u8) {
        for _ in 0..12 {
            if alive(h, seat) {
                return;
            }
            let (_, conn) = turn_of(h, people);
            h.send(conn.unwrap(), ClientMsg::Act(Action::Pass));
        }
        panic!("seat {seat} never dropped back in");
    }

    /// Bots that play the moment their turn comes up, and turns that start at once: for tests
    /// whose clock stands still while clients would still be showing a shot or a fall.
    fn at_once() -> Timing {
        Timing {
            playback_wait: false,
            ..Timing::new(Duration::ZERO)
        }
    }

    /// The fatal walk on `pits`.
    const INTO_THE_PIT: Action = Action::Walk { facing: Facing::Right, steps: 30 };

    /// Plays a `humans` game until `victim` is dead: they walk into their pit, everyone else passes.
    fn walk_into_the_pit(h: &mut Harness, people: &[MemberId], victim: u8) {
        for _ in 0..50 {
            if !alive(h, victim) {
                return;
            }
            let (seat, conn) = turn_of(h, people);
            let action = if seat == victim { INTO_THE_PIT } else { Action::Pass };
            h.send(conn.unwrap(), ClientMsg::Act(action));
        }
        panic!("seat {victim} never died");
    }

    fn seat_of(h: &Harness, id: MemberId) -> u8 {
        let play = h.room.play.as_ref().unwrap();
        u8::try_from(play.seats.iter().position(|&m| m == id).unwrap()).unwrap()
    }

    /// The member whose turn it is, and their conn in `duel`.
    fn current(h: &Harness, a: MemberId) -> (u8, u64) {
        let Phase::Turn { seat } = h.room.play.as_ref().unwrap().game.phase() else {
            panic!("the game is over")
        };
        let conn = if seat == seat_of(h, a) { 1 } else { 2 };
        (seat, conn)
    }

    #[test]
    fn start_needs_everyone_seated_ready_and_two_teams_within_size() {
        let mut h = Harness::new();
        h.join(1, "A");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), ["team B has no players"]);

        h.join(2, "B");
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), ["B isn't ready"]);
        h.join(3, "Watcher");
        h.send(2, ClientMsg::Ready(true));
        h.send(2, ClientMsg::Team(Some(Team::A)));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), ["team A has 2 players, but teams are 1"]);

        h.send(2, ClientMsg::Team(Some(Team::B)));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), Vec::<String>::new(), "spectators needn't be ready");
        assert!(matches!(h.room.view().stage, Stage::Playing { .. }));
    }

    #[test]
    fn a_map_that_cant_load_keeps_the_room_in_the_lobby() {
        let (mut h, ..) = duel();
        h.room.options.map = "gone".into();
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1).len(), 1);
        assert_eq!(h.room.view().stage, Stage::Lobby);
    }

    #[test]
    fn starting_sends_the_room_the_log_and_the_answered_opening_rolls() {
        let (mut h, a, b) = duel();
        h.take(2);
        h.send(1, ClientMsg::Start);

        let messages = h.take(2);
        let ServerMsg::Room(view) = &messages[0] else { panic!("{messages:?}") };
        let Stage::Playing { seats } = &view.stage else { panic!() };
        assert_eq!(seats, &[a, b]);
        let ServerMsg::Game { log } = &messages[1] else { panic!("{messages:?}") };
        assert_eq!(log.events, []);

        // Five forecasts fill the weather queue before anyone may move.
        let mut replica = Replica::default();
        replica.absorb(&messages[1]).unwrap();
        for (index, msg) in messages[2..].iter().enumerate() {
            let ServerMsg::Event {
                index: i,
                event: Event::Weather(forecast),
                ..
            } = msg
            else {
                panic!("{msg:?}")
            };
            assert_eq!(*i as usize, index);
            assert!((0..1600).contains(&forecast.x));
            replica.absorb(msg).unwrap();
        }
        assert_eq!(messages.len(), 2 + 5);
        let game = replica.game().unwrap();
        assert_eq!(game.pending_roll(), None);
        assert_eq!(game.state_hash().unwrap(), h.room.play.as_ref().unwrap().game.state_hash().unwrap());

        let setup = &log.setup;
        assert_eq!(setup.map, MapRef::Flat { width: 1600, height: 900, ground: 700 });
        assert_ne!(setup.seats[0].spawn_x, setup.seats[1].spawn_x);
        for seat in &setup.seats {
            assert!([200, 1400, 500, 1100, 350, 1250, 650, 950].contains(&seat.spawn_x));
            assert_eq!(seat.facing, if seat.spawn_x < 800 { Facing::Right } else { Facing::Left }, "facing the middle");
        }
        assert!(setup.wind.strength <= 12 && setup.wind.direction < 360);
    }

    #[test]
    fn drawn_maps_play_pinned_by_their_mask_hash() {
        let dist = TempDir::new().unwrap();
        let mask = Terrain::from_fn(1000, 600, |_, y| y >= 400).unwrap();
        write_map(dist.path(), "pond", &mask, &[100, 900]).unwrap();
        let mut h = Harness::with_dist(dist.path());
        h.join(1, "A");
        h.join(2, "B");
        h.send(1, ClientMsg::Configure(RoomOptions { map: "pond".into(), ..options(1) }));
        h.send(1, ClientMsg::Ready(true));
        h.send(2, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), Vec::<String>::new());

        let setup = log(&h).setup;
        assert_eq!(
            setup.map,
            MapRef::Asset {
                name: "pond".into(),
                variant: "A".into(),
                mask_hash: mask.mask_hash(),
            }
        );
        let mut spawns: Vec<(i32, Facing)> = setup.seats.iter().map(|seat| (seat.spawn_x, seat.facing)).collect();
        spawns.sort_by_key(|&(x, _)| x);
        assert_eq!(spawns, [(100, Facing::Right), (900, Facing::Left)]);
    }

    #[test]
    fn each_game_rolls_which_variant_it_plays() {
        let dist = TempDir::new().unwrap();
        let (low, high) = (Terrain::from_fn(1000, 600, |_, y| y >= 400).unwrap(), Terrain::from_fn(1000, 600, |_, y| y >= 300).unwrap());
        write_map_variants(dist.path(), "pond", &[("A", &low, &[100, 900]), ("B", &high, &[100, 900])]).unwrap();
        let mut h = Harness::with_dist(dist.path());
        h.join(1, "A");
        h.join(2, "B");
        h.send(1, ClientMsg::Configure(RoomOptions { map: "pond".into(), ..options(1) }));
        let mut played = HashSet::new();
        for _ in 0..20 {
            h.send(1, ClientMsg::Ready(true));
            h.send(2, ClientMsg::Ready(true));
            h.send(1, ClientMsg::Start);
            assert_eq!(h.rejections(1), Vec::<String>::new());
            let MapRef::Asset { variant, mask_hash, .. } = log(&h).setup.map else { panic!("a drawn map") };
            assert_eq!(mask_hash, if variant == "A" { low.mask_hash() } else { high.mask_hash() });
            played.insert(variant);
            h.room.finish(h.now);
        }
        assert_eq!(played, HashSet::from(["A".to_owned(), "B".to_owned()]));
    }

    #[test]
    fn random_teams_split_the_seated_evenly() {
        let mut h = Harness::new();
        h.join(1, "Master");
        h.send(1, ClientMsg::Configure(RoomOptions { random_teams: true, ..options(2) }));
        for _ in 0..3 {
            h.send(1, ClientMsg::AddBot { team: Team::A });
        }
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), Vec::<String>::new());
        let teams: Vec<Team> = log(&h).setup.seats.iter().map(|seat| seat.team).collect();
        assert_eq!(teams.iter().filter(|&&t| t == Team::A).count(), 2, "{teams:?}");
        let members: Vec<Option<Team>> = h.room.view().members.iter().map(|m| m.team).collect();
        assert_eq!(members, teams.into_iter().map(Some).collect::<Vec<_>>(), "members show their new teams");
    }

    #[test]
    fn only_the_current_seat_acts_and_refusals_change_nothing() {
        let (mut h, a, b) = duel();
        h.send(1, ClientMsg::Start);
        let (seat, conn) = current(&h, a);
        let other = 3 - conn;
        let before = log(&h).events.len();

        h.send(other, ClientMsg::Act(Action::Pass));
        assert_eq!(h.rejections(other), [format!("it is seat {seat}'s turn, not seat {}'s", 1 - seat)]);
        h.send(
            conn,
            ClientMsg::Act(Action::Shoot {
                shot: Shot::S1,
                aim: 90,
                power: Power::MAX,
            }),
        );
        assert_eq!(h.rejections(conn).len(), 1, "out of the aim range");
        assert_eq!(log(&h).events.len(), before);

        h.join(3, "Watcher");
        h.send(3, ClientMsg::Act(Action::Pass));
        assert_eq!(h.rejections(3), ["you aren't playing in this game"]);
        assert_ne!(a, b);
    }

    #[test]
    fn accepted_actions_are_broadcast_with_the_hash_and_shots_are_timed_by_the_server() {
        let (mut h, a, _) = duel();
        h.send(1, ClientMsg::Start);
        let mut replica = Replica::default();
        for msg in h.take(1) {
            replica.absorb(&msg).unwrap();
        }
        let (seat, conn) = current(&h, a);

        h.advance(Duration::from_millis(3_456));
        h.send(conn, ClientMsg::Act(Action::Walk { facing: Facing::Left, steps: 5 }));
        h.send(
            conn,
            ClientMsg::Act(Action::Shoot {
                shot: Shot::S1,
                aim: 45,
                power: Power::nearest(2.0),
            }),
        );
        let messages = h.take(1);
        for msg in &messages {
            replica.absorb(msg).unwrap();
        }
        let shot = log(&h).events.iter().find_map(|event| match *event {
            Event::Shoot { seat: s, time, .. } if s == seat => Some(time),
            _ => None,
        });
        assert_eq!(shot, Some(Centis(345)));
        assert!(matches!(messages[0], ServerMsg::Event { event: Event::Walk { .. }, .. }));
        assert_eq!(replica.game().unwrap().turns(), 1);
    }

    #[test]
    fn the_turn_timer_passes_for_a_player_who_waits() {
        let (mut h, a, _) = duel();
        h.send(1, ClientMsg::Start);
        let (seat, _) = current(&h, a);
        h.advance(Duration::from_millis(9_999));
        assert_eq!(current(&h, a).0, seat);
        h.advance(Duration::from_millis(1));
        assert_eq!(log(&h).events.last(), Some(&Event::Pass { seat, time: Centis(1000) }));
        assert_ne!(current(&h, a).0, seat);
        assert_eq!(h.room.deadline(), Some(h.now + Duration::from_secs(10)), "the next turn's timer");
    }

    /// A lob from whoever's turn it is in `duel`: what clients will spend showing it, and the
    /// conn that fired it.
    fn lob(h: &mut Harness, a: MemberId) -> (Duration, u64) {
        let (seat, conn) = current(h, a);
        (lob_as(h, seat, conn), conn)
    }

    /// A lob from `seat`, played by `conn`: what clients will spend showing it.
    fn lob_as(h: &mut Harness, seat: u8, conn: u64) -> Duration {
        let (shot, aim, power) = (Shot::S1, 45, Power::nearest(2.0));
        let effects = game(h).clone().apply(Event::Shoot { seat, shot, aim, power, time: Centis(0) }).unwrap();
        h.send(conn, ClientMsg::Act(Action::Shoot { shot, aim, power }));
        scene(&effects)
    }

    #[test]
    fn the_next_turns_clock_starts_once_clients_have_shown_the_shot() {
        let (mut h, a, _) = duel();
        h.send(1, ClientMsg::Start);
        let (showing, shooter) = lob(&mut h, a);
        assert!(showing > Duration::from_secs(1), "a lob takes a while to watch: {showing:?}");
        let (seat, conn) = current(&h, a);
        assert_ne!(conn, shooter);
        assert_eq!(h.room.deadline(), Some(h.now + showing + Duration::from_secs(10)));

        // A move while the shot is still landing on screens counts no turn time.
        h.advance(showing / 2);
        h.send(conn, ClientMsg::Act(Action::Pass));
        let pass = log(&h).events.iter().rev().find(|event| matches!(event, Event::Pass { .. })).copied();
        assert_eq!(pass, Some(Event::Pass { seat, time: Centis(0) }));
        // A pass has nothing to watch, but clients show it only once the shot is over, and start
        // the next turn's clock then.
        let rest = showing.checked_sub(showing / 2).unwrap();
        assert_eq!(h.room.deadline(), Some(h.now + rest + Duration::from_secs(10)));
        h.advance(rest);
        assert_eq!(h.room.deadline(), Some(h.now + Duration::from_secs(10)));
    }

    #[test]
    fn without_the_playback_wait_the_clock_starts_at_once() {
        let mut h = Harness::with_timing(Timing {
            playback_wait: false,
            ..Timing::new(Duration::ZERO)
        });
        let a = h.join(1, "A");
        h.join(2, "B");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::Ready(true));
        h.send(2, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        lob(&mut h, a);
        assert_eq!(h.room.deadline(), Some(h.now + Duration::from_secs(10)));
    }

    #[test]
    fn a_bots_think_delay_starts_once_clients_have_shown_the_shot() {
        let mut h = Harness::with_timing(Timing::new(Duration::from_millis(1200)));
        let human = h.join(1, "Pip");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        // Get to the human's turn: let the bot's opening turn, if it has one, play out.
        while h.room.play.as_ref().unwrap().game.phase() != (Phase::Turn { seat: seat_of(&h, human) }) {
            h.advance(Duration::from_millis(100));
            h.run_bots();
        }
        // Let the bot's opening shot, if any, finish showing, then lob until the bot is next: a bot
        // that used an item can sit out the human's next turn too.
        let shown = h.room.play.as_ref().unwrap().showing.until.saturating_duration_since(h.now);
        h.advance(shown);
        let showing = loop {
            let (showing, _) = lob(&mut h, human);
            if h.room.play.as_ref().unwrap().game.phase() != (Phase::Turn { seat: seat_of(&h, human) }) {
                break showing;
            }
            h.advance(showing);
        };
        h.advance(showing + Duration::from_millis(1199));
        assert_eq!(h.plans.len(), 0, "still showing the shot, then thinking");
        h.advance(Duration::from_millis(1));
        assert_eq!(h.plans.len(), 1);
    }

    #[test]
    fn a_disconnected_player_keeps_their_seat_and_their_turns_time_out() {
        let (mut h, a, b) = duel();
        h.send(1, ClientMsg::Start);
        h.close(2);
        h.advance(Duration::from_secs(120));
        let play = h.room.play.as_ref().unwrap();
        assert_eq!(play.seats, [a, b]);
        assert!(play.game.turns() >= 12, "passes on both sides, every ten seconds");
        assert!(!h.member(b).connected);
    }

    #[test]
    fn mid_game_joiners_watch_and_get_the_log() {
        let (mut h, ..) = duel();
        h.send(1, ClientMsg::Start);
        h.open(3);
        h.send(
            3,
            ClientMsg::Hello {
                name: "Late".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let messages = h.take(3);
        let [ServerMsg::Welcome { you, .. }, ServerMsg::Room(view), ServerMsg::Game { log: sent }] = messages.as_slice() else {
            panic!("{messages:?}")
        };
        assert_eq!(sent, &log(&h));
        assert_eq!(view.members.iter().find(|m| m.id == *you).unwrap().team, None);
    }

    #[test]
    fn a_rejoining_player_gets_their_seat_and_the_log() {
        let mut h = Harness::new();
        h.open(1);
        h.send(
            1,
            ClientMsg::Hello {
                name: "A".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you, token }) = h.take(1).into_iter().next() else { panic!() };
        h.join(2, "B");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::Ready(true));
        h.send(2, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        h.close(1);
        h.advance(Duration::from_secs(25));

        h.open(5);
        h.send(
            5,
            ClientMsg::Hello {
                name: "A".into(),
                token: Some(token),
                rider: "pip".into(),
            },
        );
        let messages = h.take(5);
        assert!(matches!(messages[0], ServerMsg::Welcome { you: again, .. } if again == you));
        let ServerMsg::Game { log: sent } = &messages[2] else { panic!("{messages:?}") };
        let mut replica = Replica::default();
        replica.absorb(&messages[2]).unwrap();
        assert_eq!(sent.replay(&NoAssets).unwrap().state_hash().unwrap(), h.room.play.as_ref().unwrap().game.state_hash().unwrap());
        assert_eq!(seat_of(&h, you), 0, "same seat");
        assert!(h.member(you).connected);
    }

    #[test]
    fn a_kick_mid_game_forfeits_the_seat_and_can_end_the_game() {
        let (mut h, a, b) = duel();
        h.send(1, ClientMsg::Start);
        h.take(1);
        h.take(2);
        h.send(1, ClientMsg::Kick(b));
        assert_eq!(h.take(2), [ServerMsg::Kicked]);
        assert!(h.closed.contains(&ConnId(2)));

        let messages = h.take(1);
        let forfeit = messages.iter().any(|msg| matches!(msg, ServerMsg::Event { event: Event::Forfeit { seat: 1 }, .. }));
        assert!(forfeit, "{messages:?}");
        let view = h.room.view();
        assert_eq!(view.stage, Stage::Lobby, "the last of team B left");
        assert_eq!(view.members.iter().map(|m| m.id).collect::<Vec<_>>(), [a]);
        assert!(!h.member(a).ready, "everyone readies again");
    }

    #[test]
    fn a_kicked_bots_late_plan_is_dropped() {
        let mut h = Harness::with_timing(Timing::new(Duration::from_secs(1)));
        h.join(1, "Master");
        h.send(1, ClientMsg::Configure(options(2)));
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        // Pass until a bot's turn comes up and it starts planning.
        while h.plans.is_empty() {
            if let Phase::Turn { seat: 0 } = h.room.play.as_ref().unwrap().game.phase() {
                h.send(1, ClientMsg::Act(Action::Pass));
            }
            h.advance(Duration::from_secs(1));
        }
        let turn = h.plans.pop().unwrap();
        let bot = h.room.play.as_ref().unwrap().seats[usize::from(turn.seat)];
        h.send(1, ClientMsg::Kick(bot));
        let before = log(&h).events.len();
        h.room.planned(turn.turn, turn.plan(), h.now);
        assert_eq!(log(&h).events.len(), before, "the plan's turn is over");
    }

    #[test]
    fn a_bot_game_plays_to_the_end_and_returns_to_the_lobby() {
        // The human passes the moment their turn comes up, while clients would still be showing
        // the bot's shot: with the playback wait, the bot's next turn would wait for that.
        let mut h = Harness::with_timing(at_once());
        let human = h.join(1, "Pip");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        let mut replica = Replica::default();
        // The human always passes; the bot shoots until it wins.
        for _ in 0..200 {
            for msg in h.take(1) {
                replica.absorb(&msg).unwrap();
            }
            let Some(play) = &h.room.play else { break };
            if play.game.phase() == (Phase::Turn { seat: 0 }) {
                h.send(1, ClientMsg::Act(Action::Pass));
            }
            h.run_bots();
        }
        assert_eq!(h.room.view().stage, Stage::Lobby);
        let game = replica.game().unwrap();
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::B) });
        assert!(!h.member(human).ready);
        assert!(h.room.view().members.iter().any(|m| m.bot && m.ready), "bots stay ready");
    }

    #[test]
    fn bots_wait_out_their_think_delay() {
        let mut h = Harness::with_timing(Timing::new(Duration::from_millis(1200)));
        h.join(1, "Pip");
        h.send(1, ClientMsg::Configure(options(1)));
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        if h.room.play.as_ref().unwrap().game.phase() == (Phase::Turn { seat: 0 }) {
            h.send(1, ClientMsg::Act(Action::Pass));
        }
        h.advance(Duration::from_millis(1199));
        assert_eq!(h.plans.len(), 0);
        h.advance(Duration::from_millis(1));
        assert_eq!(h.plans.len(), 1);
        assert_eq!(h.plans[0].seat, 1);
    }

    #[test]
    fn a_4v4_seats_eight_and_its_turn_order_alternates_teams() {
        let mut h = Harness::new();
        let people = humans(&mut h, 8, options(4));
        let teams: Vec<Option<Team>> = people.iter().map(|&id| h.member(id).team).collect();
        assert_eq!(teams, [Team::A, Team::B].repeat(4).into_iter().map(Some).collect::<Vec<_>>(), "joiners fill the smaller team");
        let watcher = h.join(9, "Watcher");
        assert_eq!(h.member(watcher).team, None, "eight seats are all there are");

        h.send(3, ClientMsg::Team(Some(Team::B)));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), ["team B has 5 players, but teams are 4"]);
        h.send(3, ClientMsg::Team(Some(Team::A)));
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), Vec::<String>::new());

        let setup = log(&h).setup;
        assert_eq!(setup.seats.len(), 8);
        let mut spawns: Vec<i32> = setup.seats.iter().map(|seat| seat.spawn_x).collect();
        spawns.sort_unstable();
        spawns.dedup();
        assert_eq!(spawns.len(), 8, "a spawn each");
        let sides: Vec<Team> = setup.order.iter().map(|&seat| setup.seats[usize::from(seat)].team).collect();
        assert!(sides.windows(2).all(|pair| pair[0] != pair[1]), "{sides:?}");
    }

    #[test]
    fn random_picks_are_rolled_at_the_start_and_logged() {
        let mut h = Harness::new();
        let people = humans(&mut h, 8, options(4));
        h.send(1, ClientMsg::Mobile(Some(MobileKind::Hookclaw)));
        for conn in 2..=8 {
            h.send(conn, ClientMsg::Mobile(None));
        }
        h.take(2);
        h.send(1, ClientMsg::Start);

        let messages = h.take(2);
        let Some(ServerMsg::Game { log: sent }) = messages.iter().find(|msg| matches!(msg, ServerMsg::Game { .. })) else {
            panic!("{messages:?}")
        };
        let mobiles: Vec<MobileKind> = sent.setup.seats.iter().map(|seat| seat.mobile).collect();
        assert_eq!(mobiles[0], MobileKind::Hookclaw, "a pick is kept");
        let rolled: HashSet<MobileKind> = mobiles[1..].iter().copied().collect();
        assert!(rolled.len() > 1, "Random is rolled per seat: {mobiles:?}");
        let played: Vec<MobileKind> = game(&h).players().iter().map(|player| player.mobile).collect();
        assert_eq!(played, mobiles, "the log holds what each seat plays");
        assert!(people[1..].iter().all(|&id| h.member(id).mobile.is_none()), "Random stays their pick for the next game");
    }

    #[test]
    fn in_tag_each_seat_plays_its_two_picks_and_switches_over_the_wire() {
        let mut h = Harness::new();
        let people = humans(&mut h, 2, RoomOptions { mode: Mode::Tag, ..options(1) });
        h.send(1, ClientMsg::Second(Some(MobileKind::Hookclaw)));
        h.send(2, ClientMsg::Mobile(Some(MobileKind::Sparkhorn)));
        h.send(1, ClientMsg::Start);
        let setup = log(&h).setup;
        assert_eq!(setup.mode, Mode::Tag);
        let pairs: Vec<(MobileKind, Option<MobileKind>)> = setup.seats.iter().map(|seat| (seat.mobile, seat.second)).collect();
        assert_eq!(pairs[0], (MobileKind::Coilback, Some(MobileKind::Hookclaw)), "a pick is kept");
        assert_eq!(pairs[1].0, MobileKind::Sparkhorn);
        assert!(pairs[1].1.is_some(), "a Random second is rolled: {pairs:?}");

        let (seat, conn) = turn_of(&h, &people);
        let (conn, other) = (conn.unwrap(), 3 - conn.unwrap());
        let before = game(&h).players()[usize::from(seat)].mobile;
        h.take(other);
        h.send(conn, ClientMsg::Act(Action::Switch));
        assert_eq!(h.rejections(conn), Vec::<String>::new());
        assert_eq!(game(&h).bench(seat).map(|bench| bench.mobile), Some(before), "the other sits out");
        assert!(h.take(other).iter().any(|msg| matches!(msg, ServerMsg::Event { event: Event::Switch { .. }, .. })), "broadcast");
        h.send(conn, ClientMsg::Act(Action::Switch));
        assert_eq!(h.rejections(conn), [format!("seat {seat} has switched this turn: once a turn")]);
    }

    #[test]
    fn the_setup_carries_loadouts_and_sudden_death() {
        let mut h = Harness::new();
        let pip = h.join(1, "Pip");
        let sudden_death = SuddenDeath { kind: SuddenDeathKind::Double, turn: 40 };
        h.send(
            1,
            ClientMsg::Configure(RoomOptions {
                sudden_death: Some(sudden_death),
                ..options(1)
            }),
        );
        h.send(1, ClientMsg::AddBot { team: Team::B });
        let loadout = Loadout::new(&[Item::Dual, Item::Heal, Item::Thunder]).unwrap();
        h.send(1, ClientMsg::Loadout(loadout));
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);

        let setup = log(&h).setup;
        assert_eq!(setup.sudden_death, Some(sudden_death));
        let pip = usize::from(seat_of(&h, pip));
        assert_eq!(setup.seats[pip].items, loadout);
        assert_eq!(setup.seats[1 - pip].items, bot::LOADOUT, "the bot's");
        assert_eq!(game(&h).players()[pip].items, loadout.positions());
    }

    #[test]
    fn scenes_play_one_at_a_time_and_a_turn_starts_as_its_scene_ends() {
        let start = Instant::now();
        let blast = |seconds: u32| Effect::Exploded {
            at: Vec2::new(10.0, 10.0),
            radius: 30.0,
            tick: seconds * TICKS_PER_SECOND,
            dirty: None,
        };
        let turn = Effect::TurnStarted { seat: 1 };
        let shot = [blast(2), turn.clone()];
        // A stand-in for a spin's effects: something to watch, but no turn in it.
        let spin = [blast(1)];
        let (shot_ends, spin_takes) = (start + scene(&shot), scene(&spin));
        assert!(spin_takes > Duration::ZERO);

        let mut showing = Showing::new(start);
        showing.queue(&shot, start);
        assert_eq!(showing, Showing { until: shot_ends, turn: shot_ends });
        // A spin that arrives while the shot plays waits for it, and the turn the shot started
        // keeps its clock.
        showing.queue(&spin, start + Duration::from_secs(1));
        assert_eq!(
            showing,
            Showing {
                until: shot_ends + spin_takes,
                turn: shot_ends
            }
        );
        // One that arrives with nothing playing plays at once.
        let later = start + Duration::from_mins(1);
        showing.queue(&spin, later);
        assert_eq!(
            showing,
            Showing {
                until: later + spin_takes,
                turn: shot_ends
            }
        );
        // A pass while it plays starts the next turn once it is over: not before, or the turn
        // would run while clients still show the spin, and not later.
        showing.queue(&[turn], later + Duration::from_millis(100));
        assert_eq!(
            showing,
            Showing {
                until: later + spin_takes,
                turn: later + spin_takes
            }
        );
    }

    #[test]
    fn a_living_player_or_a_watcher_cant_spin() {
        let (mut h, ..) = duel();
        h.send(1, ClientMsg::Start);
        h.join(3, "Watcher");
        let before = log(&h).events.len();
        for conn in [1, 2] {
            h.send(conn, ClientMsg::Act(Action::Spin));
            assert_eq!(h.rejections(conn).len(), 1, "conn {conn}'s mobile is alive");
        }
        h.send(3, ClientMsg::Act(Action::Spin));
        assert_eq!(h.rejections(3), ["you aren't playing in this game"]);
        assert_eq!(log(&h).events.len(), before);
    }

    /// Needs the M3 slot machine (PRD-0004 T-005): `main`'s sim refuses `Event::Spin`.
    #[test]
    fn a_dead_players_spin_works_on_anyones_turn_and_leaves_the_clock_alone() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::with_dist(dist.path());
        let people = humans(&mut h, 4, pits(2));
        h.send(1, ClientMsg::Start);
        let victim = seat_of(&h, people[0]);
        walk_into_the_pit(&mut h, &people, victim);

        // Two seconds into someone else's turn, the dead player spins.
        h.advance(Duration::from_secs(2));
        let turn = h.room.play.as_ref().unwrap().turn.unwrap();
        let deadline = h.room.deadline();
        let mut replay = game(&h).clone();
        let before = log(&h).events.len();
        h.send(1, ClientMsg::Act(Action::Spin));
        assert_eq!(h.rejections(1), Vec::<String>::new());
        let events = log(&h).events.split_off(before);
        let [Event::Spin { seat }, Event::Drop(drop)] = events.as_slice() else { panic!("{events:?}") };
        assert_eq!(*seat, victim);
        assert!((0..1600).contains(&drop.x), "{drop:?}");
        let after = h.room.play.as_ref().unwrap().turn.unwrap();
        assert_eq!((after.serial, after.seat, after.started), (turn.serial, turn.seat, turn.started), "the same turn, on the same clock");
        assert_eq!(h.room.deadline(), deadline);

        // Clients show the shot that ends this turn after the spin, so the next turn waits for both.
        let spun = h.now + events.iter().map(|&event| scene(&replay.apply(event).unwrap())).sum::<Duration>();
        h.advance(Duration::from_secs(1));
        let (seat, conn) = turn_of(&h, &people);
        let shot = lob_as(&mut h, seat, conn.unwrap());
        assert_eq!(h.room.deadline(), Some(spun.max(h.now) + shot + Duration::from_secs(10)));
    }

    /// Needs the M3 slot machine (PRD-0004 T-005): on `main`, `Game::can_spin` is always false.
    #[test]
    fn dead_bots_spin_whenever_the_game_lets_them() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::build(at_once(), dist.path());
        let host = h.join(1, "Host");
        h.send(1, ClientMsg::Configure(pits(2)));
        for team in [Team::A, Team::B, Team::B] {
            h.send(1, ClientMsg::AddBot { team });
        }
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        let seats = h.room.play.as_ref().unwrap().seats.clone();
        let host_seat = seat_of(&h, host);
        // The host's teammate, so the game outlasts its death.
        let victim = seat_of(&h, h.room.view().members[1].id);

        // The host and the bots pass, except the victim, which walks into its pit; then a round
        // more. However the game rations spins, no dead bot is ever left able to spin.
        let mut turns_dead = 0;
        for _ in 0..50 {
            if turns_dead == 8 {
                break;
            }
            h.advance(Duration::ZERO);
            let Phase::Turn { seat } = game(&h).phase() else { panic!("the game ended") };
            if seat == host_seat {
                h.send(1, ClientMsg::Act(Action::Pass));
            } else {
                let turn = h.plans.pop().expect("a bot's turn asks for a plan");
                let event = if seat == victim {
                    Event::Walk { seat, facing: Facing::Right, steps: 30 }
                } else {
                    Event::Pass { seat, time: Centis(0) }
                };
                h.planned(&turn, Some(vec![event]));
            }
            for seat in (0_u8..).take(seats.len()) {
                assert!(!game(&h).can_spin(seat), "seat {seat} could spin but didn't");
            }
            if !alive(&h, victim) {
                turns_dead += 1;
            }
        }

        assert_eq!(turns_dead, 8, "the victim never died");
        let events = log(&h).events;
        let spins: Vec<usize> = (0..events.len()).filter(|&i| events[i] == Event::Spin { seat: victim }).collect();
        assert!(!spins.is_empty(), "the dead bot never spun");
        assert!(spins.iter().all(|&i| matches!(events.get(i + 1), Some(Event::Drop(_)))), "each spin's drop is rolled at once");
    }

    /// Needs the M3 slot machine (PRD-0004 T-005): `main`'s sim refuses `Event::Spin`.
    #[test]
    fn in_a_4v4_a_dead_player_who_leaves_and_rejoins_still_spins() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::with_dist(dist.path());
        let (first, token) = h.join_with_token(1, "Victim");
        h.send(1, ClientMsg::Configure(pits(4)));
        let mut people = vec![first];
        people.extend((2..=8).map(|conn| h.join(conn, &format!("P{conn}"))));
        for conn in 1..=8 {
            h.send(conn, ClientMsg::Ready(true));
        }
        h.send(1, ClientMsg::Start);
        let victim = seat_of(&h, first);
        walk_into_the_pit(&mut h, &people, victim);

        h.close(1);
        // A round goes by without them; their seat has no turns to time out.
        for _ in 0..7 {
            let (_, conn) = turn_of(&h, &people);
            h.send(conn.unwrap(), ClientMsg::Act(Action::Pass));
        }
        h.open(9);
        h.send(
            9,
            ClientMsg::Hello {
                name: "Victim".into(),
                token: Some(token),
                rider: "pip".into(),
            },
        );
        assert!(matches!(h.take(9).first(), Some(ServerMsg::Welcome { you, .. }) if *you == first));
        let before = log(&h).events.len();
        h.send(9, ClientMsg::Act(Action::Spin));
        assert_eq!(h.rejections(9), Vec::<String>::new());
        let events = log(&h).events.split_off(before);
        assert!(matches!(events.as_slice(), [Event::Spin { seat }, Event::Drop(_)] if *seat == victim), "{events:?}");
    }

    #[test]
    fn in_score_a_death_costs_a_life_and_a_dead_players_pick_drops_them_back_in() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::with_dist(dist.path());
        let people = humans(&mut h, 4, score(pits(2)));
        h.send(1, ClientMsg::Start);
        assert_eq!(log(&h).setup.mode, Mode::Score);
        let victim = seat_of(&h, people[0]);
        let team = game(&h).players()[usize::from(victim)].team;
        walk_into_the_pit(&mut h, &people, victim);
        assert_eq!(game(&h).points(team), Some(2));

        // No slot machine: the dead pick where to drop back in, on anyone's turn.
        h.send(1, ClientMsg::Act(Action::Spin));
        assert_eq!(h.rejections(1), ["there's no slot machine in Score: the dead drop back in"]);
        let turn = h.room.play.as_ref().unwrap().turn.unwrap();
        h.send(1, ClientMsg::Act(Action::Respawn { x: 800 }));
        assert_eq!(h.rejections(1), Vec::<String>::new());
        assert_eq!(log(&h).events.last(), Some(&Event::Respawn { seat: victim, x: 800 }));
        let after = h.room.play.as_ref().unwrap().turn.unwrap();
        assert_eq!((after.serial, after.started), (turn.serial, turn.started), "the turn and its clock go on");
        h.send(1, ClientMsg::Act(Action::Respawn { x: 900 }));
        assert_eq!(h.rejections(1).len(), 1, "one pick at a time");

        pass_until_alive(&mut h, &people, victim);
        let back = game(&h).players()[usize::from(victim)];
        assert!((back.footing.x - 800).abs() <= 20, "where it picked, the wind nudging it: {:?}", back.footing);
        assert_eq!(game(&h).points(team), Some(2), "dropping back in is free");
    }

    #[test]
    fn in_score_dead_bots_pick_where_to_drop_back_in_at_once() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::build(at_once(), dist.path());
        let host = h.join(1, "Host");
        h.send(1, ClientMsg::Configure(score(pits(2))));
        for team in [Team::A, Team::B, Team::B] {
            h.send(1, ClientMsg::AddBot { team });
        }
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        let host_seat = seat_of(&h, host);
        let victim = seat_of(&h, h.room.view().members[1].id);
        for _ in 0..8 {
            if !alive(&h, victim) {
                break;
            }
            h.advance(Duration::ZERO);
            let Phase::Turn { seat } = game(&h).phase() else { panic!("the game ended") };
            if seat == host_seat {
                h.send(1, ClientMsg::Act(Action::Pass));
            } else {
                let turn = h.plans.pop().expect("a bot's turn asks for a plan");
                let event = if seat == victim {
                    Event::Walk { seat, facing: Facing::Right, steps: 30 }
                } else {
                    Event::Pass { seat, time: Centis(0) }
                };
                h.planned(&turn, Some(vec![event]));
            }
        }
        assert!(!alive(&h, victim), "the victim never died");
        let events = log(&h).events;
        let died = events.iter().rposition(|event| matches!(event, Event::Walk { seat, .. } if *seat == victim)).unwrap();
        assert!(matches!(events[died + 1], Event::Respawn { seat, .. } if seat == victim), "{:?}", &events[died..]);
        assert!(!game(&h).can_respawn(victim), "picked, it waits");
    }

    #[test]
    fn in_score_a_kicked_player_waiting_to_drop_back_in_never_does() {
        let dist = TempDir::new().unwrap();
        write_pits(dist.path());
        let mut h = Harness::with_dist(dist.path());
        let people = humans(&mut h, 4, score(pits(2)));
        h.send(1, ClientMsg::Start);
        let victim = seat_of(&h, people[1]);
        walk_into_the_pit(&mut h, &people, victim);
        h.send(2, ClientMsg::Act(Action::Respawn { x: 800 }));
        assert_eq!(game(&h).respawns().len(), 1);
        let team = game(&h).players()[usize::from(victim)].team;
        let points = game(&h).points(team);
        assert_eq!(points, Some(2));
        h.send(1, ClientMsg::Kick(people[1]));
        assert_eq!(log(&h).events.last(), Some(&Event::Forfeit { seat: victim }));
        assert_eq!(game(&h).respawns(), []);
        assert_eq!(game(&h).points(team), points, "its death was counted once");
    }

    #[test]
    fn in_a_4v4_a_kicked_players_mobile_dies_and_the_game_goes_on() {
        let mut h = Harness::new();
        let people = humans(&mut h, 8, options(4));
        h.send(1, ClientMsg::Start);
        // The master can't kick themselves, so play on until it is someone else's turn.
        while turn_of(&h, &people).1 == Some(1) {
            h.send(1, ClientMsg::Act(Action::Pass));
        }

        // Off their turn: their mobile dies, and the turn goes on as it was.
        let (current, _) = turn_of(&h, &people);
        let turn = h.room.play.as_ref().unwrap().turn.unwrap().serial;
        let idle = (0..8).find(|&seat| seat != current && seat != 0).unwrap();
        h.send(1, ClientMsg::Kick(people[usize::from(idle)]));
        assert_eq!(log(&h).events.last(), Some(&Event::Forfeit { seat: idle }));
        assert!(!alive(&h, idle));
        assert_eq!(game(&h).phase(), Phase::Turn { seat: current });
        assert_eq!(h.room.play.as_ref().unwrap().turn.unwrap().serial, turn);

        // On their turn: their mobile dies and the next player's turn begins.
        h.send(1, ClientMsg::Kick(people[usize::from(current)]));
        assert!(!alive(&h, current));
        assert!(h.room.play.as_ref().unwrap().turn.unwrap().serial > turn);
        assert!(matches!(h.room.view().stage, Stage::Playing { .. }), "three a side play on");

        for _ in 0..12 {
            let (seat, conn) = turn_of(&h, &people);
            assert!(![idle, current].contains(&seat), "a kicked seat's turn came up");
            h.send(conn.unwrap(), ClientMsg::Act(Action::Pass));
        }
    }

    #[test]
    fn eight_bots_plan_one_turn_at_a_time_and_the_room_answers_meanwhile() {
        let mut h = Harness::with_timing(at_once());
        h.join(1, "Host");
        h.send(1, ClientMsg::Team(None));
        h.send(1, ClientMsg::Configure(options(4)));
        for team in [Team::A, Team::B].repeat(4) {
            h.send(1, ClientMsg::AddBot { team });
        }
        h.send(1, ClientMsg::Start);
        assert_eq!(h.rejections(1), Vec::<String>::new(), "a spectating master needn't be ready");

        for _ in 0..16 {
            h.advance(Duration::ZERO);
            let Phase::Turn { seat } = game(&h).phase() else { break };
            assert_eq!(h.plans.iter().map(|turn| turn.seat).collect::<Vec<_>>(), [seat], "the bot whose turn it is, alone");
            let turn = h.plans.pop().unwrap();
            // The search runs off the room's task; the room keeps serving while it does.
            h.take(1);
            h.send(
                1,
                ClientMsg::Chat {
                    text: "still here".into(),
                    team_only: false,
                },
            );
            assert!(h.take(1).iter().any(|msg| matches!(msg, ServerMsg::Chat { .. })));
            let events = turn.plan();
            assert!(events.as_ref().is_some_and(|events| events.iter().any(|e| matches!(e, Event::Shoot { .. }))), "{events:?}");
            h.planned(&turn, events);
        }
    }
}
