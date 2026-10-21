//! A room: its members, the lobby before a game, and the game it is playing.
//!
//! `Room` is a plain state machine. Each input (a socket opening, a message, a socket closing, a
//! bot's plan coming back, or time passing) arrives with the time, changes the state, and queues
//! outputs: messages for sockets, sockets to close, bot turns to plan. Nothing here touches a
//! socket or reads a clock, so every rule is unit-tested directly. `task` is the thin async shell
//! that runs one room on one tokio task (`DESIGN.md`, "Architecture").

mod lobby;
mod play;
pub mod task;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, anyhow, bail, ensure};
use rand::{RngExt as _, rngs::StdRng};

use lobshot_protocol::{ClientMsg, Member, MemberId, RoomCode, RoomOptions, RoomView, ServerMsg, Stage, Token, clean_name};
use lobshot_sim::{
    base::{Res, Void},
    bot::{self, Level},
    game::{Event, Game, MAX_SEATS, MobileKind},
    items::Loadout,
};

use crate::maps::{Maps, check_asset_name};

use self::play::Play;

/// How many may watch a room besides its players.
pub const MAX_SPECTATORS: usize = 16;

/// Open sockets per room, said hello or not: every member plus a few still joining.
pub const MAX_SOCKETS: usize = MAX_SEATS + MAX_SPECTATORS + 8;

/// How long a member who dropped in the lobby keeps their place.
pub const LOBBY_GRACE: Duration = Duration::from_secs(30);

/// How long a room lives with no human connected.
pub const IDLE: Duration = Duration::from_mins(10);

/// The rider a bot wears.
const BOT_RIDER: &str = "pip";

/// A socket, unique across the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// How long things take. Tests shorten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// A bot "thinks" this long before it plays, so its turn reads as a turn.
    pub bot_delay: Duration,
    /// Whether a turn's clock waits for clients to show the last shot (`lobshot_protocol::pacing`).
    pub playback_wait: bool,
    /// A room with no human connected this long ends.
    pub idle: Duration,
}

impl Timing {
    #[must_use]
    pub const fn new(bot_delay: Duration) -> Self {
        Self {
            bot_delay,
            playback_wait: true,
            idle: IDLE,
        }
    }
}

/// What a room asks its shell to do, in order.
#[derive(Debug, Clone)]
pub enum Output {
    Send(ConnId, Arc<ServerMsg>),
    /// Close the socket once what was sent to it before is out.
    Close(ConnId),
    /// Plan a bot's turn off the room's task, then hand the result to `Room::planned`.
    Plan(BotTurn),
}

/// A bot's turn to plan: everything `lobshot_sim::bot::plan` needs, owned, so it can run on
/// `spawn_blocking`.
#[derive(Debug, Clone)]
pub struct BotTurn {
    /// Which turn this is, so a plan that comes back late is dropped.
    pub turn: u64,
    pub seat: u8,
    pub game: Box<Game>,
    pub seed: u64,
}

impl BotTurn {
    /// Runs the search. Blocking: about a second at most in a release build.
    #[must_use]
    pub fn plan(&self) -> Option<Vec<Event>> {
        bot::plan(&self.game, self.seat, Level::Normal, self.seed)
    }
}

/// A member and what only the server knows about them.
#[derive(Debug, Clone)]
struct Entry {
    member: Member,
    /// Rejoins as this member. Bots have none.
    token: Option<Token>,
    socket: Option<ConnId>,
    /// When their socket closed, while it stays closed.
    left: Option<Instant>,
}

impl Entry {
    fn is_human(&self) -> bool {
        !self.member.bot
    }
}

/// One room's whole state.
#[derive(Debug)]
pub struct Room {
    code: RoomCode,
    options: RoomOptions,
    /// In joining order, so the oldest member comes first.
    members: Vec<Entry>,
    next_id: u32,
    master: MemberId,
    /// Open sockets, and the member each speaks for once it said hello.
    sockets: HashMap<ConnId, Option<MemberId>>,
    play: Option<Play>,
    /// When the room last returned to the lobby (or opened): lobby grace counts from here, so
    /// someone who dropped mid-game gets the full grace once the game ends.
    lobby_since: Instant,
    /// Since when no human has been connected.
    empty_since: Option<Instant>,
    ended: bool,
    /// Turns started in this room, across games: names a turn for its bot plan.
    turns_started: u64,
    maps: Arc<Maps>,
    timing: Timing,
    rng: StdRng,
    out: Vec<Output>,
}

impl Room {
    /// A new, empty room. `rng` rolls everything random: tokens, spawns, turn order, weather.
    #[must_use]
    pub fn new(code: RoomCode, maps: Arc<Maps>, timing: Timing, rng: StdRng, now: Instant) -> Self {
        Self {
            code,
            options: RoomOptions::default(),
            members: Vec::new(),
            next_id: 0,
            master: MemberId(0),
            sockets: HashMap::new(),
            play: None,
            lobby_since: now,
            empty_since: Some(now),
            ended: false,
            turns_started: 0,
            maps,
            timing,
            rng,
            out: Vec::new(),
        }
    }

    /// A socket opened on this room. It must say `Hello` before anything else.
    pub fn open(&mut self, conn: ConnId) {
        if self.sockets.len() >= MAX_SOCKETS {
            self.send(conn, ServerMsg::Rejected { reason: "the room is full".to_owned() });
            self.out.push(Output::Close(conn));
            return;
        }
        self.sockets.insert(conn, None);
    }

    /// A message from a socket. Anything refused is answered with `Rejected`.
    pub fn message(&mut self, conn: ConnId, msg: ClientMsg, now: Instant) {
        // A socket the room already closed (a kick, a newer socket for the same member).
        let Some(&speaker) = self.sockets.get(&conn) else { return };
        let result = match (speaker, msg) {
            (None, ClientMsg::Hello { name, token, rider }) => self.hello(conn, &name, token.as_ref(), &rider),
            (None, _) => Err(anyhow!("say hello first")),
            (Some(id), msg) => self.handle(id, msg, now),
        };
        if let Err(error) = result {
            self.send(conn, ServerMsg::Rejected { reason: format!("{error:#}") });
        }
        self.refresh(now);
    }

    /// A socket closed. Its member stays: in a game they keep their seat, and in the lobby they
    /// have `LOBBY_GRACE` to come back.
    pub fn closed(&mut self, conn: ConnId, now: Instant) {
        let Some(speaker) = self.sockets.remove(&conn) else { return };
        if let Some(entry) = speaker.and_then(|id| self.members.iter_mut().find(|e| e.member.id == id)) {
            entry.socket = None;
            entry.member.connected = false;
            entry.left = Some(now);
            self.broadcast_room();
        }
        self.refresh(now);
    }

    /// A bot's plan came back for `turn`.
    pub fn planned(&mut self, turn: u64, events: Option<Vec<Event>>, now: Instant) {
        self.play_planned(turn, events.unwrap_or_default(), now);
    }

    /// Time passed: acts on whatever came due (see `deadline`).
    pub fn tick(&mut self, now: Instant) {
        if self.empty_since.is_some_and(|since| now >= since + self.timing.idle) {
            self.end();
            return;
        }
        if self.play.is_some() {
            self.play_tick(now);
        } else {
            self.drop_leavers(now);
        }
    }

    /// When `tick` next has something to do.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        if self.ended {
            return None;
        }
        let expiry = self.empty_since.map(|since| since + self.timing.idle);
        let next = match &self.play {
            Some(play) => play.deadline(),
            None => self.members.iter().filter_map(|e| e.left).map(|left| left.max(self.lobby_since) + LOBBY_GRACE).min(),
        };
        expiry.into_iter().chain(next).min()
    }

    /// Whether the room is over: its shell stops and it leaves the registry.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// What the room asked for since the last call, in order.
    pub fn take_output(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.out)
    }

    /// What every member sees outside the game.
    #[must_use]
    pub fn view(&self) -> RoomView {
        RoomView {
            code: self.code.clone(),
            master: self.master,
            options: self.options.clone(),
            members: self.members.iter().map(|e| e.member.clone()).collect(),
            stage: match &self.play {
                Some(play) => Stage::Playing { seats: play.seats.clone() },
                None => Stage::Lobby,
            },
        }
    }

    fn handle(&mut self, id: MemberId, msg: ClientMsg, now: Instant) -> Void {
        match msg {
            ClientMsg::Hello { .. } => bail!("you already said hello"),
            ClientMsg::Chat { text, team_only } => self.chat(id, &text, team_only),
            ClientMsg::Emote(emote) => {
                self.broadcast(ServerMsg::Emote { from: id, emote });
                Ok(())
            }
            ClientMsg::Team(team) => self.choose_team(id, team),
            ClientMsg::Mobile(mobile) => self.choose(id, |member| member.mobile = mobile),
            ClientMsg::Loadout(items) => self.choose(id, |member| member.items = items),
            ClientMsg::Rider(rider) => {
                check_asset_name(&rider, "rider")?;
                self.choose(id, |member| member.rider = rider)
            }
            ClientMsg::Ready(ready) => self.choose(id, |member| member.ready = ready),
            ClientMsg::Configure(options) => self.configure(id, options),
            ClientMsg::AddBot { team } => self.add_bot(id, team),
            ClientMsg::Kick(target) => self.kick(id, target, now),
            ClientMsg::Start => self.start(id, now),
            ClientMsg::Act(action) => self.act(id, action, now),
        }
    }

    /// Answers `Hello`: `Welcome`, then the room, then the game if one is on. A known token
    /// rejoins as its member; anyone else joins as someone new.
    fn hello(&mut self, conn: ConnId, name: &str, token: Option<&Token>, rider: &str) -> Void {
        let known = token.and_then(|token| self.members.iter().position(|e| e.token.as_ref() == Some(token)));
        let index = match known {
            Some(index) => {
                let entry = &mut self.members[index];
                // The same player on a newer socket (a reload, a second tab): the older one goes.
                if let Some(old) = entry.socket.replace(conn) {
                    self.sockets.remove(&old);
                    self.out.push(Output::Close(old));
                }
                entry.member.connected = true;
                entry.left = None;
                index
            }
            None => self.join(conn, name, rider)?,
        };
        let entry = &self.members[index];
        let (you, token) = (entry.member.id, entry.token.clone().context("only humans say hello")?);
        self.sockets.insert(conn, Some(you));
        self.send(conn, ServerMsg::Welcome { you, token });
        self.broadcast_room();
        if let Some(play) = &self.play {
            let log = play.log.clone();
            self.send(conn, ServerMsg::Game { log });
        }
        Ok(())
    }

    /// Adds a new human member and returns their index. In the lobby they take a seat on the
    /// smaller team if it has room; otherwise, and always mid-game, they watch.
    fn join(&mut self, conn: ConnId, name: &str, rider: &str) -> Res<usize> {
        let name = clean_name(name)?;
        check_asset_name(rider, "rider")?;
        let team = if self.play.is_none() { self.open_team() } else { None };
        ensure!(team.is_some() || self.spectators() < MAX_SPECTATORS, "the room is full");
        let id = self.next_member_id();
        let token = Token(format!("{:032x}", self.rng.random::<u128>()));
        self.members.push(Entry {
            member: Member {
                id,
                name,
                team,
                mobile: Some(MobileKind::ALL[0]),
                items: Loadout::default(),
                rider: rider.to_owned(),
                ready: false,
                bot: false,
                connected: true,
            },
            token: Some(token),
            socket: Some(conn),
            left: None,
        });
        // The creator is the first master; a room left with only bots takes the next human.
        if !self.members.iter().any(|e| e.member.id == self.master && e.is_human()) {
            self.master = id;
        }
        Ok(self.members.len() - 1)
    }

    fn next_member_id(&mut self) -> MemberId {
        let id = MemberId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Removes a member. If they were the master, the oldest remaining human takes over.
    fn remove_member(&mut self, id: MemberId) {
        self.members.retain(|e| e.member.id != id);
        if id == self.master
            && let Some(next) = self.members.iter().find(|e| e.is_human())
        {
            self.master = next.member.id;
        }
    }

    /// Lobby members whose grace ran out leave.
    fn drop_leavers(&mut self, now: Instant) {
        let gone: Vec<MemberId> = self
            .members
            .iter()
            .filter(|e| e.left.is_some_and(|left| now >= left.max(self.lobby_since) + LOBBY_GRACE))
            .map(|e| e.member.id)
            .collect();
        for &id in &gone {
            self.remove_member(id);
        }
        if !gone.is_empty() {
            self.broadcast_room();
        }
    }

    /// Starts or stops the idle clock as humans come and go.
    fn refresh(&mut self, now: Instant) {
        if self.members.iter().any(|e| e.is_human() && e.socket.is_some()) {
            self.empty_since = None;
        } else {
            self.empty_since.get_or_insert(now);
        }
    }

    /// Ends the room: every socket closes.
    fn end(&mut self) {
        self.ended = true;
        self.out.extend(self.sockets.drain().map(|(conn, _)| Output::Close(conn)));
    }

    fn entry(&self, id: MemberId) -> Res<&Entry> {
        self.members.iter().find(|e| e.member.id == id).context("there is no such member")
    }

    fn entry_mut(&mut self, id: MemberId) -> Res<&mut Entry> {
        self.members.iter_mut().find(|e| e.member.id == id).context("there is no such member")
    }

    fn seated(&self) -> usize {
        self.members.iter().filter(|e| e.member.team.is_some()).count()
    }

    fn spectators(&self) -> usize {
        self.members.len() - self.seated()
    }

    fn ensure_master(&self, id: MemberId) -> Void {
        ensure!(id == self.master, "only the room master can do that");
        Ok(())
    }

    fn send(&mut self, conn: ConnId, msg: ServerMsg) {
        self.out.push(Output::Send(conn, Arc::new(msg)));
    }

    /// Sends one message to every connected member `to` picks.
    fn send_to(&mut self, to: impl Fn(&Member) -> bool, msg: ServerMsg) {
        let msg = Arc::new(msg);
        for entry in &self.members {
            if let Some(conn) = entry.socket
                && to(&entry.member)
            {
                self.out.push(Output::Send(conn, Arc::clone(&msg)));
            }
        }
    }

    fn broadcast(&mut self, msg: ServerMsg) {
        self.send_to(|_| true, msg);
    }

    fn broadcast_room(&mut self) {
        let view = self.view();
        self.broadcast(ServerMsg::Room(view));
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A room driven by hand: sockets are numbers, mailboxes collect what each would receive,
    //! and time moves only when a test says so.

    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::{collections::HashSet, path::Path};

    use rand::SeedableRng as _;

    use super::*;

    pub struct Harness {
        pub room: Room,
        pub now: Instant,
        mailboxes: HashMap<ConnId, Vec<ServerMsg>>,
        pub closed: HashSet<ConnId>,
        pub plans: Vec<BotTurn>,
    }

    impl Harness {
        /// A room whose bots play the moment their turn comes up.
        pub fn new() -> Self {
            Self::with_timing(Timing::new(Duration::ZERO))
        }

        pub fn with_timing(timing: Timing) -> Self {
            Self::build(timing, Path::new("no-such-dist"))
        }

        /// A room whose drawn maps come from `dist`.
        pub fn with_dist(dist: &Path) -> Self {
            Self::build(Timing::new(Duration::ZERO), dist)
        }

        fn build(timing: Timing, dist: &Path) -> Self {
            let now = Instant::now();
            let maps = Arc::new(Maps::new(dist));
            let room = Room::new(RoomCode::from_indices([0; 8]), maps, timing, StdRng::seed_from_u64(7), now);
            Self {
                room,
                now,
                mailboxes: HashMap::new(),
                closed: HashSet::new(),
                plans: Vec::new(),
            }
        }

        pub fn open(&mut self, conn: u64) {
            self.room.open(ConnId(conn));
            self.pump();
        }

        /// Opens socket `conn` and says hello as `name`; returns the new member's id.
        pub fn join(&mut self, conn: u64, name: &str) -> MemberId {
            self.open(conn);
            self.send(
                conn,
                ClientMsg::Hello {
                    name: name.into(),
                    token: None,
                    rider: "pip".into(),
                },
            );
            match self.take(conn).first() {
                Some(ServerMsg::Welcome { you, .. }) => *you,
                other => panic!("{name} wasn't welcomed: {other:?}"),
            }
        }

        pub fn send(&mut self, conn: u64, msg: ClientMsg) {
            self.room.message(ConnId(conn), msg, self.now);
            self.pump();
        }

        pub fn close(&mut self, conn: u64) {
            self.room.closed(ConnId(conn), self.now);
            self.pump();
        }

        /// Moves the clock forward by `by`, stopping at each deadline on the way to let the room
        /// act at that moment, as its task does.
        pub fn advance(&mut self, by: Duration) {
            let until = self.now + by;
            let mut ticks = 0;
            while let Some(deadline) = self.room.deadline().filter(|&deadline| deadline <= until) {
                self.now = self.now.max(deadline);
                self.room.tick(self.now);
                self.pump();
                ticks += 1;
                assert!(ticks < 10_000, "the room keeps asking to tick: a deadline that never moves");
            }
            self.now = until;
        }

        /// Plans every bot turn the room asked for and hands the plans back, until none is left.
        pub fn run_bots(&mut self) {
            self.advance(Duration::ZERO);
            while !self.plans.is_empty() {
                for turn in std::mem::take(&mut self.plans) {
                    let events = turn.plan();
                    self.room.planned(turn.turn, events, self.now);
                    self.pump();
                }
                self.advance(Duration::ZERO);
            }
        }

        /// Everything sent to `conn` since the last call.
        pub fn take(&mut self, conn: u64) -> Vec<ServerMsg> {
            self.mailboxes.remove(&ConnId(conn)).unwrap_or_default()
        }

        /// The reasons `conn` was refused, since the last `take`.
        pub fn rejections(&mut self, conn: u64) -> Vec<String> {
            self.take(conn)
                .into_iter()
                .filter_map(|msg| match msg {
                    ServerMsg::Rejected { reason } => Some(reason),
                    _ => None,
                })
                .collect()
        }

        pub fn member(&self, id: MemberId) -> Member {
            self.room.entry(id).unwrap().member.clone()
        }

        fn pump(&mut self) {
            for output in self.room.take_output() {
                match output {
                    Output::Send(conn, msg) => self.mailboxes.entry(conn).or_default().push((*msg).clone()),
                    Output::Close(conn) => {
                        self.closed.insert(conn);
                    }
                    Output::Plan(turn) => self.plans.push(turn),
                }
            }
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::{testing::Harness, *};
    use lobshot_sim::game::Team;

    #[test]
    fn hello_is_answered_with_welcome_then_the_room() {
        let mut h = Harness::new();
        h.open(1);
        h.send(
            1,
            ClientMsg::Hello {
                name: "  Pip ".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let messages = h.take(1);
        let [ServerMsg::Welcome { you, token }, ServerMsg::Room(view)] = messages.as_slice() else {
            panic!("{messages:?}")
        };
        assert_eq!(token.0.len(), 32);
        assert_eq!(view.master, *you, "the first member is master");
        assert_eq!(view.members[0].name, "Pip", "names are cleaned");
        assert_eq!(view.members[0].team, Some(Team::A));
        assert!(view.members[0].connected && !view.members[0].ready && !view.members[0].bot);
    }

    #[test]
    fn the_first_message_must_be_hello() {
        let mut h = Harness::new();
        h.open(1);
        h.send(1, ClientMsg::Ready(true));
        assert_eq!(h.rejections(1), ["say hello first"]);
        h.send(
            1,
            ClientMsg::Hello {
                name: " ".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        assert_eq!(h.rejections(1), ["a name can't be empty"]);
        h.send(
            1,
            ClientMsg::Hello {
                name: "Pip".into(),
                token: None,
                rider: "../x".into(),
            },
        );
        assert_eq!(h.rejections(1), ["\"../x\" is not a rider name"]);
        h.join(1, "Pip");
        h.send(
            1,
            ClientMsg::Hello {
                name: "Pip".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        assert_eq!(h.rejections(1), ["you already said hello"]);
    }

    #[test]
    fn joiners_fill_the_smaller_team_then_watch() {
        let mut h = Harness::new();
        let ids: Vec<MemberId> = (0..3).map(|conn| h.join(conn, &format!("P{conn}"))).collect();
        let teams: Vec<Option<Team>> = ids.into_iter().map(|id| h.member(id).team).collect();
        assert_eq!(teams, [Some(Team::A), Some(Team::B), None], "a 1v1 room seats two");
    }

    #[test]
    fn a_token_rejoins_as_the_same_member_and_replaces_the_old_socket() {
        let mut h = Harness::new();
        h.open(1);
        h.send(
            1,
            ClientMsg::Hello {
                name: "Pip".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you, token }) = h.take(1).into_iter().next() else { panic!() };

        h.open(2);
        h.send(
            2,
            ClientMsg::Hello {
                name: "Someone".into(),
                token: Some(token.clone()),
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you: again, token: same }) = h.take(2).into_iter().next() else {
            panic!()
        };
        assert_eq!((again, &same), (you, &token));
        assert!(h.closed.contains(&ConnId(1)), "the older socket closes");
        assert_eq!(h.room.view().members.len(), 1);

        // The old socket's late close doesn't disconnect the member.
        h.close(1);
        assert!(h.member(you).connected);

        // An unknown token joins as someone new.
        h.open(3);
        h.send(
            3,
            ClientMsg::Hello {
                name: "Other".into(),
                token: Some(Token("nope".into())),
                rider: "pip".into(),
            },
        );
        assert!(matches!(h.take(3).first(), Some(ServerMsg::Welcome { you: other, .. }) if *other != you));
    }

    #[test]
    fn the_oldest_human_takes_over_when_the_master_leaves() {
        let mut h = Harness::new();
        let first = h.join(1, "First");
        h.send(1, ClientMsg::AddBot { team: Team::B });
        let second = h.join(2, "Second");
        let third = h.join(3, "Third");
        assert_eq!(h.room.view().master, first);

        h.close(1);
        h.advance(LOBBY_GRACE.saturating_sub(Duration::from_secs(1)));
        assert_eq!(h.room.view().master, first, "still in their grace period");
        h.advance(Duration::from_secs(1));
        let view = h.room.view();
        assert_eq!(view.master, second, "the bot is older, but not human");
        assert!(view.members.iter().all(|m| m.id != first));

        h.send(2, ClientMsg::Kick(third));
        assert_eq!(h.room.view().master, second);
    }

    #[test]
    fn a_member_who_returns_within_the_grace_keeps_their_place() {
        let mut h = Harness::new();
        h.open(1);
        h.send(
            1,
            ClientMsg::Hello {
                name: "Pip".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you, token }) = h.take(1).into_iter().next() else { panic!() };
        h.join(2, "Other");
        h.close(1);
        assert!(!h.member(you).connected);
        h.advance(Duration::from_secs(20));
        h.open(3);
        h.send(
            3,
            ClientMsg::Hello {
                name: "Pip".into(),
                token: Some(token),
                rider: "pip".into(),
            },
        );
        h.advance(Duration::from_secs(60));
        assert!(h.member(you).connected);
        assert_eq!(h.room.view().master, you);
    }

    #[test]
    fn a_room_with_no_human_for_ten_minutes_ends() {
        let mut h = Harness::new();
        h.advance(IDLE.saturating_sub(Duration::from_secs(1)));
        assert!(!h.room.ended(), "a fresh room waits for its creator");
        h.join(1, "Pip");
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.advance(IDLE);
        assert!(!h.room.ended());

        h.close(1);
        h.advance(IDLE.saturating_sub(Duration::from_secs(1)));
        assert!(!h.room.ended(), "bots don't keep a room alive, but ten minutes haven't passed");
        h.open(2);
        h.advance(Duration::from_secs(1));
        assert!(h.room.ended());
        assert!(h.closed.contains(&ConnId(2)), "sockets that never said hello close too");
    }

    #[test]
    fn sockets_past_the_cap_are_turned_away() {
        let mut h = Harness::new();
        for conn in 0..u64::try_from(MAX_SOCKETS).unwrap() {
            h.open(conn);
        }
        h.open(999);
        h.advance(Duration::ZERO);
        assert_eq!(h.rejections(999), ["the room is full"]);
        assert!(h.closed.contains(&ConnId(999)));
    }

    #[test]
    fn the_room_holds_eight_players_and_sixteen_spectators() {
        let mut h = Harness::new();
        h.join(0, "Master");
        h.send(
            0,
            ClientMsg::Configure(RoomOptions {
                map: "flat".into(),
                team_size: 4,
                ..RoomOptions::default()
            }),
        );
        for _ in 0..7 {
            h.send(0, ClientMsg::AddBot { team: Team::B });
        }
        h.send(0, ClientMsg::AddBot { team: Team::A });
        assert_eq!(h.rejections(0), ["the room seats 8 players"]);
        for conn in 1..=16 {
            let id = h.join(conn, &format!("S{conn}"));
            assert_eq!(h.member(id).team, None);
        }
        h.open(17);
        h.send(
            17,
            ClientMsg::Hello {
                name: "Late".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        assert_eq!(h.rejections(17), ["the room is full"]);
        h.send(1, ClientMsg::Team(Some(Team::A)));
        assert_eq!(h.rejections(1), ["the room seats 8 players"]);
        h.send(0, ClientMsg::Team(None));
        assert_eq!(h.rejections(0), ["the room has room for 16 spectators"]);
    }
}
