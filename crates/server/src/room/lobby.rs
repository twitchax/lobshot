//! The lobby: members' own choices (team, mobile, rider, ready), the master's controls (options,
//! bots, kicks), and chat, which works in the lobby and in a game alike.

use std::time::Instant;

use anyhow::{Context as _, ensure};

use lobshot_protocol::{MAX_CHAT, Member, MemberId, RoomOptions, ServerMsg};
use lobshot_sim::{
    base::Void,
    bot,
    game::{MAX_SEATS, MobileKind, Team},
};

use super::{BOT_RIDER, Entry, MAX_SPECTATORS, Output, Room};

impl Room {
    pub(super) fn ensure_lobby(&self) -> Void {
        ensure!(self.play.is_none(), "not while a game is on");
        Ok(())
    }

    /// A member's own pre-game choice.
    pub(super) fn choose(&mut self, id: MemberId, change: impl FnOnce(&mut Member)) -> Void {
        self.ensure_lobby()?;
        change(&mut self.entry_mut(id)?.member);
        self.broadcast_room();
        Ok(())
    }

    /// Takes a seat on a team, or `None` to watch. Teams may run over `team_size` here; `Start`
    /// checks it, so players can shuffle around freely first.
    pub(super) fn choose_team(&mut self, id: MemberId, team: Option<Team>) -> Void {
        self.ensure_lobby()?;
        match (self.entry(id)?.member.team, team) {
            (None, Some(_)) => ensure!(self.seated() < MAX_SEATS, "the room seats {MAX_SEATS} players"),
            (Some(_), None) => ensure!(self.spectators() < MAX_SPECTATORS, "the room has room for {MAX_SPECTATORS} spectators"),
            _ => {}
        }
        self.choose(id, |member| member.team = team)
    }

    /// The team a newcomer takes in the lobby: the smaller one, if it has room.
    pub(super) fn open_team(&self) -> Option<Team> {
        let count = |team| self.members.iter().filter(|e| e.member.team == Some(team)).count();
        let team = if count(Team::B) < count(Team::A) { Team::B } else { Team::A };
        (count(team) < usize::from(self.options.team_size) && self.seated() < MAX_SEATS).then_some(team)
    }

    pub(super) fn configure(&mut self, id: MemberId, options: RoomOptions) -> Void {
        self.ensure_master(id)?;
        self.ensure_lobby()?;
        options.validate()?;
        // Refuse a map that can't be played now rather than at `Start`.
        let spawns = self.maps.arenas(&options.map)?.iter().map(|arena| arena.spawns.len()).min().unwrap_or(0);
        let players = 2 * usize::from(options.team_size);
        ensure!(spawns >= players, "{} has spawns for only {spawns} players, not {players}", options.map);
        self.options = options;
        self.broadcast_room();
        Ok(())
    }

    /// Seats a bot, named "Bot N" with the lowest free N. Bots are always ready, and bring
    /// SearchBot's loadout (`bot::LOADOUT`): the items it knows how to use.
    pub(super) fn add_bot(&mut self, id: MemberId, team: Team) -> Void {
        self.ensure_master(id)?;
        self.ensure_lobby()?;
        ensure!(self.seated() < MAX_SEATS, "the room seats {MAX_SEATS} players");
        // With n members, one of the first n + 1 names is free.
        let name = (1..=self.members.len() + 1)
            .map(|n| format!("Bot {n}"))
            .find(|name| self.members.iter().all(|e| e.member.name != *name))
            .context("no bot name is free")?;
        let bot = self.next_member_id();
        self.members.push(Entry {
            member: Member {
                id: bot,
                name,
                team: Some(team),
                mobile: Some(MobileKind::ALL[0]),
                items: bot::LOADOUT,
                rider: BOT_RIDER.to_owned(),
                ready: true,
                bot: true,
                connected: true,
            },
            token: None,
            socket: None,
            left: None,
        });
        self.broadcast_room();
        Ok(())
    }

    /// The master picks a bot's mobile, as a player picks their own.
    pub(super) fn bot_mobile(&mut self, id: MemberId, bot: MemberId, mobile: Option<MobileKind>) -> Void {
        self.ensure_master(id)?;
        ensure!(self.entry(bot)?.member.bot, "only a bot's mobile is the master's to pick");
        self.choose(bot, |member| member.mobile = mobile)
    }

    /// The master removes a member: their token stops working and their socket closes. Mid-game
    /// their mobile dies where it stands, as a leaver's would. They can come back through the
    /// link as someone new.
    pub(super) fn kick(&mut self, id: MemberId, target: MemberId, now: Instant) -> Void {
        self.ensure_master(id)?;
        ensure!(target != id, "you can't kick yourself");
        let socket = self.entry(target)?.socket;
        // Gone before the forfeit goes out: the last thing a kicked player hears is `Kicked`.
        self.remove_member(target);
        if let Some(conn) = socket {
            self.sockets.remove(&conn);
            self.send(conn, ServerMsg::Kicked);
            self.out.push(Output::Close(conn));
        }
        self.forfeit(target, now)?;
        self.broadcast_room();
        Ok(())
    }

    /// Chat to everyone, or to the sender's team only. Spectators' team chat stays among
    /// spectators.
    pub(super) fn chat(&mut self, id: MemberId, text: &str, team_only: bool) -> Void {
        let text = text.trim();
        ensure!(!text.is_empty(), "say something");
        ensure!(text.chars().count() <= MAX_CHAT, "chat messages are at most {MAX_CHAT} characters");
        ensure!(!text.chars().any(char::is_control), "chat can't hold control characters");
        let sender = &self.entry(id)?.member;
        let team = sender.team;
        let msg = ServerMsg::Chat {
            from: id,
            name: sender.name.clone(),
            text: text.to_owned(),
            team_only,
        };
        if team_only {
            self.send_to(|member| member.team == team, msg);
        } else {
            self.broadcast(msg);
        }
        Ok(())
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use lobshot_protocol::{ClientMsg, Emote, Stage, Token};
    use lobshot_sim::{
        game::{SuddenDeath, SuddenDeathKind},
        items::{Item, Loadout},
        terrain::Terrain,
        tuning::SUDDEN_DEATH_TURNS,
    };

    use super::*;
    use crate::{
        room::{ConnId, testing::Harness},
        tests::write_map,
    };

    fn flat(team_size: u8) -> ClientMsg {
        ClientMsg::Configure(RoomOptions {
            map: "flat".into(),
            team_size,
            ..RoomOptions::default()
        })
    }

    #[test]
    fn members_pick_their_team_mobile_rider_and_readiness() {
        let mut h = Harness::new();
        let pip = h.join(1, "Pip");
        h.send(1, ClientMsg::Team(Some(Team::B)));
        h.send(1, ClientMsg::Mobile(Some(MobileKind::Hookclaw)));
        let loadout = Loadout::new(&[Item::Teleport, Item::PowerUp, Item::Heal, Item::WindFlip]).unwrap();
        h.send(1, ClientMsg::Loadout(loadout));
        h.send(1, ClientMsg::Rider("pip-2".into()));
        h.send(1, ClientMsg::Ready(true));
        let member = h.member(pip);
        assert_eq!(
            (member.team, member.mobile, member.rider.as_str(), member.ready),
            (Some(Team::B), Some(MobileKind::Hookclaw), "pip-2", true)
        );
        assert_eq!(member.items, loadout);
        assert!(h.take(1).iter().all(|msg| matches!(msg, ServerMsg::Room(_))), "each change is broadcast");
        h.send(1, ClientMsg::Mobile(None));
        assert_eq!(h.member(pip).mobile, None, "Random");

        h.send(1, ClientMsg::Rider("../../etc".into()));
        assert_eq!(h.rejections(1), ["\"../../etc\" is not a rider name"]);
        h.send(1, ClientMsg::Team(None));
        assert_eq!(h.member(pip).team, None);
    }

    #[test]
    fn only_the_master_configures_adds_bots_kicks_and_starts() {
        let mut h = Harness::new();
        let master = h.join(1, "Master");
        let other = h.join(2, "Other");
        let bot_mobile = ClientMsg::BotMobile { bot: master, mobile: None };
        for msg in [flat(2), ClientMsg::AddBot { team: Team::A }, bot_mobile, ClientMsg::Kick(master), ClientMsg::Start] {
            h.send(2, msg);
            assert_eq!(h.rejections(2), ["only the room master can do that"]);
        }
        h.send(1, ClientMsg::Kick(master));
        assert_eq!(h.rejections(1), ["you can't kick yourself"]);
        h.send(1, ClientMsg::Kick(other));
        assert_eq!(h.rejections(1), Vec::<String>::new());
    }

    #[test]
    fn the_master_picks_a_bots_mobile_but_no_players() {
        let mut h = Harness::new();
        let master = h.join(1, "Master");
        let other = h.join(2, "Other");
        h.send(1, ClientMsg::AddBot { team: Team::B });
        let bot = h.room.view().members.iter().find(|m| m.bot).unwrap().id;
        h.send(1, ClientMsg::BotMobile { bot, mobile: Some(MobileKind::Sparkhorn) });
        assert_eq!(h.member(bot).mobile, Some(MobileKind::Sparkhorn));
        h.send(1, ClientMsg::BotMobile { bot, mobile: None });
        assert_eq!(h.member(bot).mobile, None, "Random");
        for target in [master, other] {
            h.send(1, ClientMsg::BotMobile { bot: target, mobile: None });
            assert_eq!(h.rejections(1), ["only a bot's mobile is the master's to pick"]);
        }
    }

    #[test]
    fn options_are_validated_and_the_map_must_exist() {
        let mut h = Harness::new();
        h.join(1, "Master");
        h.send(1, flat(5));
        assert_eq!(h.rejections(1), ["teams are 1 to 4 players, not 5"]);
        h.send(
            1,
            ClientMsg::Configure(RoomOptions {
                map: "nowhere".into(),
                ..RoomOptions::default()
            }),
        );
        let reasons = h.rejections(1);
        assert!(reasons.len() == 1 && reasons[0].starts_with("there is no map \"nowhere\""), "{reasons:?}");
        h.send(1, flat(2));
        assert_eq!(h.room.view().options.team_size, 2);
    }

    #[test]
    fn sudden_death_is_off_or_a_kind_from_a_listed_turn() {
        let mut h = Harness::new();
        h.join(1, "Master");
        let with = |sudden_death| {
            ClientMsg::Configure(RoomOptions {
                map: "flat".into(),
                sudden_death,
                ..RoomOptions::default()
            })
        };
        h.send(1, with(Some(SuddenDeath { kind: SuddenDeathKind::Double, turn: 41 })));
        assert_eq!(h.rejections(1), ["sudden death starts at turn [40, 56, 72], not 41"]);
        for kind in [SuddenDeathKind::BigBomb, SuddenDeathKind::Double, SuddenDeathKind::SsDeath] {
            for turn in SUDDEN_DEATH_TURNS.value {
                let sudden_death = Some(SuddenDeath { kind, turn });
                h.send(1, with(sudden_death));
                assert_eq!(h.rejections(1), Vec::<String>::new());
                assert_eq!(h.room.view().options.sudden_death, sudden_death);
            }
        }
        h.send(1, with(None));
        assert_eq!(h.room.view().options.sudden_death, None, "off");
    }

    #[test]
    fn a_map_must_have_a_spawn_for_every_seat_of_the_team_size() {
        let dist = TempDir::new().unwrap();
        write_map(dist.path(), "nook", &Terrain::from_fn(800, 400, |_, y| y >= 300).unwrap(), &[100, 300, 500, 700]).unwrap();
        let mut h = Harness::with_dist(dist.path());
        h.join(1, "Master");
        let nook = |team_size| {
            ClientMsg::Configure(RoomOptions {
                map: "nook".into(),
                team_size,
                ..RoomOptions::default()
            })
        };
        h.send(1, nook(3));
        assert_eq!(h.rejections(1), ["nook has spawns for only 4 players, not 6"]);
        h.send(1, nook(2));
        assert_eq!(h.rejections(1), Vec::<String>::new());
        h.send(1, flat(4));
        assert_eq!(h.rejections(1), Vec::<String>::new(), "flat seats eight");
    }

    #[test]
    fn bots_are_named_seated_and_ready() {
        let mut h = Harness::new();
        h.join(1, "Master");
        h.send(1, flat(4));
        for _ in 0..3 {
            h.send(1, ClientMsg::AddBot { team: Team::B });
        }
        let bots: Vec<Member> = h.room.view().members.into_iter().filter(|m| m.bot).collect();
        let names: Vec<&str> = bots.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Bot 1", "Bot 2", "Bot 3"]);
        assert!(bots.iter().all(|m| m.ready && m.connected && m.team == Some(Team::B)));
        assert!(bots.iter().all(|m| m.items == bot::LOADOUT && m.mobile.is_some()), "SearchBot's loadout, and no Random pick");

        h.send(1, ClientMsg::Kick(bots[1].id));
        h.send(1, ClientMsg::AddBot { team: Team::A });
        let names: Vec<String> = h.room.view().members.into_iter().filter(|m| m.bot).map(|m| m.name).collect();
        assert_eq!(names, ["Bot 1", "Bot 3", "Bot 2"], "the lowest free number");
    }

    #[test]
    fn a_kick_in_the_lobby_revokes_the_token_and_closes_the_socket() {
        let mut h = Harness::new();
        h.join(1, "Master");
        h.open(2);
        h.send(
            2,
            ClientMsg::Hello {
                name: "Pest".into(),
                token: None,
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you, token }) = h.take(2).into_iter().next() else { panic!() };

        h.take(2);
        h.send(1, ClientMsg::Kick(you));
        assert_eq!(h.take(2), [ServerMsg::Kicked]);
        assert!(h.closed.contains(&ConnId(2)));
        assert!(h.room.view().members.iter().all(|m| m.id != you));
        // Their socket is gone from the room: anything it still sends is ignored.
        h.take(1);
        h.send(2, ClientMsg::Chat { text: "hey".into(), team_only: false });
        assert!(h.take(1).is_empty() && h.take(2).is_empty());

        h.open(3);
        h.send(
            3,
            ClientMsg::Hello {
                name: "Pest".into(),
                token: Some(token),
                rider: "pip".into(),
            },
        );
        let Some(ServerMsg::Welcome { you: again, token: Token(_) }) = h.take(3).into_iter().next() else {
            panic!()
        };
        assert_ne!(again, you, "the old token no longer rejoins");
    }

    #[test]
    fn team_chat_reaches_only_the_team_and_spectators_only_spectators() {
        let mut h = Harness::new();
        h.join(1, "A1");
        h.send(1, flat(2));
        h.join(2, "B1");
        h.join(3, "A2");
        h.join(4, "B2");
        let watcher = h.join(5, "Watcher");
        h.join(6, "Watcher2");
        assert_eq!(h.member(watcher).team, None);
        for conn in 1..=6 {
            h.take(conn);
        }

        let heard = |h: &mut Harness| -> Vec<u64> { (1..=6).filter(|&conn| h.take(conn).iter().any(|m| matches!(m, ServerMsg::Chat { .. }))).collect() };
        h.send(1, ClientMsg::Chat { text: " gl ".into(), team_only: true });
        assert_eq!(heard(&mut h), [1, 3]);
        h.send(5, ClientMsg::Chat { text: "nice".into(), team_only: true });
        assert_eq!(heard(&mut h), [5, 6]);
        h.send(5, ClientMsg::Chat { text: "hi all".into(), team_only: false });
        assert_eq!(heard(&mut h), [1, 2, 3, 4, 5, 6]);

        h.send(2, ClientMsg::Chat { text: "  gg  ".into(), team_only: false });
        assert_eq!(
            h.take(1),
            [ServerMsg::Chat {
                from: h.room.view().members[1].id,
                name: "B1".into(),
                text: "gg".into(),
                team_only: false,
            }]
        );
    }

    #[test]
    fn chat_must_be_short_and_say_something() {
        let mut h = Harness::new();
        h.join(1, "Pip");
        for (text, reason) in [
            ("   ", "say something"),
            (&"a".repeat(MAX_CHAT + 1), "chat messages are at most 200 characters"),
            ("bell\u{7}", "chat can't hold control characters"),
        ] {
            h.send(1, ClientMsg::Chat { text: text.into(), team_only: false });
            assert_eq!(h.rejections(1), [reason]);
        }
        h.send(
            1,
            ClientMsg::Chat {
                text: "a".repeat(MAX_CHAT),
                team_only: false,
            },
        );
        assert_eq!(h.rejections(1), Vec::<String>::new());
    }

    #[test]
    fn everyone_sees_emotes() {
        let mut h = Harness::new();
        h.join(1, "Pip");
        h.join(2, "Other");
        let watcher = h.join(3, "Watcher");
        for conn in 1..=3 {
            h.take(conn);
        }
        h.send(3, ClientMsg::Emote(Emote::GoodGame));
        for conn in 1..=3 {
            assert_eq!(h.take(conn), [ServerMsg::Emote { from: watcher, emote: Emote::GoodGame }]);
        }
    }

    #[test]
    fn lobby_choices_are_refused_mid_game() {
        let mut h = Harness::new();
        h.join(1, "Pip");
        h.send(1, flat(1));
        h.send(1, ClientMsg::AddBot { team: Team::B });
        h.send(1, ClientMsg::Ready(true));
        h.send(1, ClientMsg::Start);
        assert!(matches!(h.room.view().stage, Stage::Playing { .. }));
        h.take(1);
        for msg in [
            ClientMsg::Ready(false),
            ClientMsg::Team(None),
            ClientMsg::Mobile(Some(MobileKind::Coilback)),
            ClientMsg::Mobile(None),
            ClientMsg::Loadout(Loadout::new(&[Item::Heal]).unwrap()),
            ClientMsg::Rider("pip".into()),
            flat(1),
            ClientMsg::AddBot { team: Team::A },
            ClientMsg::Start,
        ] {
            h.send(1, msg);
            assert_eq!(h.rejections(1), ["not while a game is on"]);
        }
        h.advance(Duration::ZERO);
    }
}
