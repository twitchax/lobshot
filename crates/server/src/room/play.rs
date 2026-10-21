//! A game in a room: setting it up from the lobby, turning members' actions into sim events,
//! answering the rolls the sim asks for, the turn clock, and bot turns.
//!
//! Every accepted event goes through `commit`: applied to the server's `Game`, appended to the log
//! and broadcast with the state hash after it. Then `settle` answers whatever rolls the sim now
//! owes (`DESIGN.md`, "Game model": the server owns randomness) and starts the next turn or ends
//! the game. A turn's clock starts once clients have shown everything before it
//! (`lobshot_protocol::pacing`): the sim resolves a shot at once, but a player watches it fly.

use std::time::{Duration, Instant};

use anyhow::{Context as _, bail, ensure};
use rand::{RngExt as _, seq::SliceRandom as _};

use lobshot_protocol::{Action, MemberId, ServerMsg, pacing};
use lobshot_sim::{
    base::Void,
    drops::{Drop, DropKind},
    game::{Event, Game, MobileKind, Phase, Roll, Seat, Setup, Team},
    log::Log,
    units::{Centis, Facing, Wind},
    weather::{Forecast, WeatherKind},
};

use super::{BotTurn, Output, Room};

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
    /// How long clients will take to show what was committed since the last turn began.
    pub showing: Duration,
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

        let arena = self.maps.arena(&self.options.map)?;
        ensure!(arena.spawns.len() >= seated.len(), "{} has spawns for only {} players", self.options.map, arena.spawns.len());
        let mut spawns = arena.spawns;
        spawns.shuffle(&mut self.rng);
        // Random picks are rolled here, so the log holds the mobile each seat actually plays.
        let mobiles: Vec<MobileKind> = seated
            .iter()
            .map(|&i| self.members[i].member.mobile.unwrap_or_else(|| MobileKind::ALL[self.rng.random_range(0..MobileKind::ALL.len())]))
            .collect();
        let seats = seated
            .iter()
            .zip(&teams)
            .zip(&spawns)
            .zip(&mobiles)
            .map(|(((&i, &team), &spawn_x), &mobile)| Seat {
                team,
                mobile,
                spawn_x,
                // Facing the middle, where the enemy most likely is.
                facing: if 2 * i64::from(spawn_x) < i64::from(arena.width) { Facing::Right } else { Facing::Left },
                items: self.members[i].member.items,
            })
            .collect();
        let order = self.turn_order(&teams);
        let wind = self.roll_wind();
        let setup = Setup {
            map: arena.map,
            seats,
            order,
            wind,
            sudden_death: self.options.sudden_death,
        };
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
            showing: Duration::ZERO,
        });
        tracing::info!(room = self.code.as_str(), players = seated.len(), map = self.options.map, "game started");
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

    /// Seats in a random order with teams alternating: each team shuffled, then interleaved,
    /// starting with a random team. A bigger team's extra players go last.
    fn turn_order(&mut self, teams: &[Team]) -> Vec<u8> {
        let mut sides = [Team::A, Team::B].map(|side| (0_u8..).zip(teams).filter(|&(_, &team)| team == side).map(|(seat, _)| seat).collect::<Vec<u8>>());
        for side in &mut sides {
            side.shuffle(&mut self.rng);
        }
        if self.rng.random() {
            sides.swap(0, 1);
        }
        let rounds = sides.iter().map(Vec::len).max().unwrap_or(0);
        (0..rounds).flat_map(|round| sides.iter().filter_map(move |side| side.get(round).copied())).collect()
    }

    fn roll_wind(&mut self) -> Wind {
        Wind {
            strength: self.rng.random_range(0..=self.options.max_wind),
            direction: self.rng.random_range(0..360),
        }
    }

    fn roll(&mut self, roll: Roll) -> Event {
        match roll {
            Roll::Weather { width } => Event::Weather(Forecast {
                kind: WeatherKind::ALL[self.rng.random_range(0..WeatherKind::ALL.len())],
                x: self.rng.random_range(0..i32::try_from(width).unwrap_or(i32::MAX).max(1)),
            }),
            Roll::Wind => Event::Wind(self.roll_wind()),
            Roll::Drop { width } => Event::Drop(Drop {
                kind: DropKind::ALL[self.rng.random_range(0..DropKind::ALL.len())],
                x: self.rng.random_range(0..i32::try_from(width).unwrap_or(i32::MAX).max(1)),
            }),
        }
    }

    /// A move from the member in the current seat.
    pub(super) fn act(&mut self, id: MemberId, action: Action, now: Instant) -> Void {
        let play = self.play.as_ref().context("no game is on")?;
        let seat = play.seats.iter().position(|&member| member == id).context("you aren't playing in this game")?;
        let seat = u8::try_from(seat).context("too many seats")?;
        let time = play.elapsed(now);
        let event = match action {
            Action::Walk { facing, steps } => Event::Walk { seat, facing, steps },
            Action::Shoot { shot, aim, power } => Event::Shoot { seat, shot, aim, power, time },
            Action::Pass => Event::Pass { seat, time },
            Action::Item { slot } => Event::UseItem { seat, slot },
            Action::Spin => Event::Spin { seat },
        };
        self.commit(event)?;
        self.settle(now);
        Ok(())
    }

    /// A kicked member's mobile dies where it stands, if they are playing and it is alive.
    pub(super) fn forfeit(&mut self, id: MemberId, now: Instant) -> Void {
        let Some(play) = &self.play else { return Ok(()) };
        let Some(seat) = play.seats.iter().position(|&member| member == id) else { return Ok(()) };
        if !play.game.players().get(seat).is_some_and(|player| player.alive) {
            return Ok(());
        }
        self.commit(Event::Forfeit {
            seat: u8::try_from(seat).context("too many seats")?,
        })?;
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
            let event = match event {
                Event::Shoot { seat, shot, aim, power, .. } => Event::Shoot { seat, shot, aim, power, time },
                Event::Pass { seat, .. } => Event::Pass { seat, time },
                other => other,
            };
            if let Err(error) = self.commit(event) {
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
    fn commit(&mut self, event: Event) -> Void {
        let play = self.play.as_mut().context("no game is on")?;
        let index = u32::try_from(play.log.events.len()).context("the log is full")?;
        let effects = play.game.apply(event)?;
        play.showing += Duration::try_from_secs_f32(pacing::schedule(&effects).end).unwrap_or_default();
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
        if let Err(error) = self.commit(event) {
            tracing::error!(room = self.code.as_str(), %error, ?event, "the game refused a server event; ending it");
            self.finish(now);
        }
    }

    /// After an event: answers every roll the sim now owes, then starts the next turn or, if the
    /// game is over, returns to the lobby.
    fn settle(&mut self, now: Instant) {
        while let Some(roll) = self.play.as_ref().and_then(|play| play.game.pending_roll()) {
            let event = self.roll(roll);
            self.commit_or_abort(event, now);
        }
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

    fn begin_turn(&mut self, seat: u8, now: Instant) {
        self.turns_started += 1;
        let Some(play) = &mut self.play else { return };
        let bot = play.seats.get(usize::from(seat)).is_some_and(|&id| self.members.iter().any(|e| e.member.id == id && e.member.bot));
        // The clock starts as clients finish showing the last turn, not when the sim resolved it.
        let showing = std::mem::take(&mut play.showing);
        let started = if self.timing.playback_wait { now + showing } else { now };
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

    use pretty_assertions::assert_eq;

    use tempfile::TempDir;

    use lobshot_protocol::{ClientMsg, RoomOptions, Stage};
    use lobshot_sim::{
        game::Shot,
        map::{MapRef, NoAssets},
        terrain::Terrain,
        units::Power,
    };

    use super::*;
    use crate::{
        room::{ConnId, Timing, testing::Harness},
        tests::{Replica, write_map},
    };

    fn options(team_size: u8) -> RoomOptions {
        RoomOptions {
            map: "flat".into(),
            team_size,
            max_wind: 12,
            turn_seconds: 10,
            random_teams: false,
            sudden_death: None,
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
    fn drawn_maps_play_variant_a_pinned_by_its_mask_hash() {
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
    fn turn_order_alternates_teams() {
        let mut h = Harness::new();
        for _ in 0..50 {
            let teams = [Team::A, Team::A, Team::B, Team::B, Team::A, Team::B, Team::A];
            let order = h.room.turn_order(&teams);
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, [0, 1, 2, 3, 4, 5, 6]);
            let sides: Vec<Team> = order.iter().map(|&seat| teams[usize::from(seat)]).collect();
            assert!(sides[..6].windows(2).all(|pair| pair[0] != pair[1]), "{sides:?}");
            assert_eq!(sides[6], Team::A, "the bigger team's extra player goes last");
        }
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
        let (shot, aim, power) = (Shot::S1, 45, Power::nearest(2.0));
        let mut game = h.room.play.as_ref().unwrap().game.clone();
        let effects = game.apply(Event::Shoot { seat, shot, aim, power, time: Centis(0) }).unwrap();
        h.send(conn, ClientMsg::Act(Action::Shoot { shot, aim, power }));
        (Duration::from_secs_f32(pacing::schedule(&effects).end), conn)
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
        // A pass (and the rolls after it) have nothing to watch: the next clock starts at once.
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
        let (showing, _) = lob(&mut h, human);
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
        let mut h = Harness::new();
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
}
