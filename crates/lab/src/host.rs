//! A headless host: the server's job for one game, without the clocks. It seats the game and
//! answers the sim's rolls through `sim::referee`, as the server does; plays dead bots (they spin
//! their slot machines, or pick where to drop back in); plays bot seats with SearchBot, charging
//! each bot turn the time a server's bot takes (`tuning::bot::THINK`); and hands agent seats back
//! to its caller, on their turns and when, dead, they may pick where to drop back in.

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use lobshot_protocol::RoomOptions;
use lobshot_sim::{
    base::{Res, Void},
    bot::{self, Level},
    game::{Effect, Event, Game, Phase},
    log::Log,
    map::{Arena, MapAssets},
    referee::{self, Entrant, XorShift},
    tuning,
};

/// An event the game accepted, and what it did.
#[derive(Debug, Clone, PartialEq)]
pub struct Happened {
    pub event: Event,
    pub effects: Vec<Effect>,
}

/// Everything a host needs to pick a game up where it left off: the log, the dice as they stand,
/// and who plays each seat. Its JSON is an agent session's save file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub log: Log,
    pub dice: XorShift,
    pub max_wind: u8,
    /// SearchBot at a level, or `None` for a seat the caller plays.
    pub bots: Vec<Option<Level>>,
}

/// One game, hosted.
#[derive(Debug, Clone)]
pub struct Host {
    game: Game,
    saved: Saved,
}

impl Host {
    /// Seats `entrants` on `arena` under `options`' rules (max wind, sudden death, mode), every
    /// roll from `dice`. `bots` says who plays each seat.
    ///
    /// # Errors
    ///
    /// Fails when the arena can't seat them or its map can't be loaded.
    pub fn seat(arena: &Arena, entrants: &[Entrant], options: &RoomOptions, bots: Vec<Option<Level>>, mut dice: XorShift, assets: &dyn MapAssets) -> Res<Self> {
        anyhow::ensure!(bots.len() == entrants.len(), "{} seats but {} players", entrants.len(), bots.len());
        let setup = referee::setup(arena, entrants, options.max_wind, options.sudden_death, options.mode, &mut dice)?;
        let game = Game::new(&setup, assets).context("setting up the game")?;
        Ok(Self {
            game,
            saved: Saved {
                log: Log { setup, events: Vec::new() },
                dice,
                max_wind: options.max_wind,
                bots,
            },
        })
    }

    /// Picks up a saved game by replaying its log.
    ///
    /// # Errors
    ///
    /// Fails when the log doesn't replay.
    pub fn load(saved: Saved, assets: &dyn MapAssets) -> Res<Self> {
        let game = saved.log.replay(assets).context("replaying the saved game")?;
        Ok(Self { game, saved })
    }

    #[must_use]
    pub fn game(&self) -> &Game {
        &self.game
    }

    #[must_use]
    pub fn log(&self) -> &Log {
        &self.saved.log
    }

    #[must_use]
    pub fn saved(&self) -> &Saved {
        &self.saved
    }

    /// SearchBot takes over `seat` at `level`: for a dead agent, whose slot machine it then spins.
    pub fn hand_to_bot(&mut self, seat: u8, level: Level) {
        if let Some(bot) = self.saved.bots.get_mut(usize::from(seat)) {
            *bot = Some(level);
        }
    }

    /// Whether SearchBot plays `seat` (and at what level).
    #[must_use]
    pub fn bot(&self, seat: u8) -> Option<Level> {
        self.saved.bots.get(usize::from(seat)).copied().flatten()
    }

    /// Applies `event` and logs it.
    ///
    /// # Errors
    ///
    /// Fails when the game refuses it; nothing changes then.
    pub fn commit(&mut self, event: Event) -> Res<Happened> {
        let effects = self.game.apply(event)?;
        self.saved.log.events.push(event);
        Ok(Happened { event, effects })
    }

    /// Answers every roll the game owes and plays every dead bot the game lets act (a spin, or a
    /// drop point: `bot::while_dead`), as the server does after each event.
    ///
    /// # Errors
    ///
    /// Fails when the game refuses a roll, or a dead bot's move it allowed: a sim bug.
    pub fn settle(&mut self, out: &mut Vec<Happened>) -> Void {
        loop {
            while let Some(roll) = self.game.pending_roll() {
                let event = referee::answer(roll, self.saved.max_wind, &mut self.saved.dice);
                out.push(self.commit(event).context("answering a roll")?);
            }
            let seats = u8::try_from(self.saved.bots.len()).unwrap_or(u8::MAX);
            let Some(event) = (0..seats).filter(|&seat| self.bot(seat).is_some()).find_map(|seat| bot::while_dead(&self.game, seat)) else {
                return Ok(());
            };
            out.push(self.commit(event).context("a dead bot's move")?);
        }
    }

    /// Whether an agent's seat, dead, may pick where to drop back in: the host waits for it.
    #[must_use]
    pub fn agent_may_respawn(&self) -> bool {
        let seats = u8::try_from(self.saved.bots.len()).unwrap_or(u8::MAX);
        (0..seats).any(|seat| self.bot(seat).is_none() && self.game.can_respawn(seat))
    }

    /// Settles, then plays one bot turn if a bot is to move. Returns the seat that played, or
    /// `None` when the game is over, an agent's seat is to move, or a dead agent may pick where to
    /// drop back in.
    ///
    /// # Errors
    ///
    /// Fails when the game refuses an event the host itself chose: a sim bug.
    pub fn step(&mut self, out: &mut Vec<Happened>) -> Res<Option<u8>> {
        self.settle(out)?;
        if self.agent_may_respawn() {
            return Ok(None);
        }
        let Phase::Turn { seat } = self.game.phase() else { return Ok(None) };
        let Some(level) = self.bot(seat) else { return Ok(None) };
        let (turns, think) = (self.game.turns(), tuning::bot::THINK.value);
        let seed = self.saved.dice.next_u64();
        // As the server plays a plan: in order, stamped, until the turn ends; one the game refuses
        // ends the plan, and a plan that leaves the turn open becomes a pass.
        for event in bot::plan(&self.game, seat, level, seed).unwrap_or_default() {
            match self.commit(referee::stamp(event, think)) {
                Ok(happened) => out.push(happened),
                Err(error) => {
                    tracing::warn!(%error, ?event, "the game refused a bot's planned event");
                    break;
                }
            }
            if self.moved_on(turns) {
                break;
            }
        }
        if !self.moved_on(turns) {
            out.push(self.commit(Event::Pass { seat, time: think }).context("a bot's pass")?);
        }
        self.settle(out)?;
        Ok(Some(seat))
    }

    /// Plays bot turns until an agent's seat is to move, the game is over, or `cap` turns are done.
    ///
    /// # Errors
    ///
    /// As `step`.
    pub fn run(&mut self, cap: i64, out: &mut Vec<Happened>) -> Void {
        while self.game.turns() < cap && self.step(out)?.is_some() {}
        self.settle(out)
    }

    fn moved_on(&self, turns: i64) -> bool {
        self.game.turns() != turns || matches!(self.game.phase(), Phase::Over { .. })
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use lobshot_assets::{FLAT, maps::Maps};
    use lobshot_sim::{
        game::{MobileKind, Team},
        units::Centis,
    };

    use super::*;

    fn duel(bots: Vec<Option<Level>>, seed: u64) -> Host {
        let maps = Maps::new(std::path::Path::new("no-assets"));
        let [arena] = maps.arenas(FLAT).unwrap().try_into().unwrap();
        let entrants = [Team::A, Team::B].map(|team| Entrant {
            team,
            mobile: Some(MobileKind::Coilback),
            items: bot::LOADOUT,
            second: None,
        });
        Host::seat(&arena, &entrants, &RoomOptions::default(), bots, XorShift::new(seed), &maps).unwrap()
    }

    #[test]
    fn bots_play_a_game_to_the_end_and_its_log_replays() {
        let mut host = duel(vec![Some(Level::Hard); 2], 5);
        let mut out = Vec::new();
        host.run(1000, &mut out).unwrap();
        assert!(matches!(host.game().phase(), Phase::Over { .. }), "{:?}", host.game().phase());
        assert_eq!(out.iter().map(|h| h.event).collect::<Vec<_>>(), host.log().events);
        let replayed = host.log().replay(&lobshot_sim::map::NoAssets).unwrap();
        assert_eq!(replayed.state_hash().unwrap(), host.game().state_hash().unwrap());
    }

    #[test]
    fn a_seed_is_a_game() {
        let play = |seed| {
            let mut host = duel(vec![Some(Level::Normal); 2], seed);
            host.run(1000, &mut Vec::new()).unwrap();
            host.log().clone()
        };
        assert_eq!(play(11), play(11));
        assert_ne!(play(11), play(12));
    }

    #[test]
    fn bot_turns_are_charged_the_servers_think_time() {
        let mut host = duel(vec![Some(Level::Normal); 2], 3);
        let mut out = Vec::new();
        host.step(&mut out).unwrap();
        let timed: Vec<Centis> = out
            .iter()
            .filter_map(|h| match h.event {
                Event::Shoot { time, .. } | Event::Pass { time, .. } | Event::UseItem { time, .. } => Some(time),
                _ => None,
            })
            .collect();
        assert_ne!(timed, []);
        assert!(timed.iter().all(|&time| time == tuning::bot::THINK.value), "{timed:?}");
    }

    #[test]
    fn an_agents_seat_is_handed_back() {
        let mut host = duel(vec![None, Some(Level::Normal)], 8);
        host.run(1000, &mut Vec::new()).unwrap();
        assert_eq!(host.game().phase(), Phase::Turn { seat: 0 });
        assert_eq!(host.game().pending_roll(), None, "settled before handing back");
        let mut again = Vec::new();
        assert_eq!(host.step(&mut again).unwrap(), None);
        assert_eq!(again, []);
    }

    #[test]
    fn a_saved_game_picks_up_where_it_left_off() {
        let mut host = duel(vec![None, Some(Level::Normal)], 21);
        host.run(1000, &mut Vec::new()).unwrap();
        host.commit(Event::Pass { seat: 0, time: Centis(300) }).unwrap();
        let json = serde_json::to_string(host.saved()).unwrap();
        let mut loaded = Host::load(serde_json::from_str(&json).unwrap(), &lobshot_sim::map::NoAssets).unwrap();
        assert_eq!(loaded.game().state_hash().unwrap(), host.game().state_hash().unwrap());
        host.run(1000, &mut Vec::new()).unwrap();
        loaded.run(1000, &mut Vec::new()).unwrap();
        assert_eq!(loaded.log(), host.log(), "the dice come back as they stood");
    }

    /// Plays 128 4v4 rooms of Coilbacks per level as the server seats them (its `flat` map, the
    /// bot loadout, the default rules: max wind 26, Double at 40) and prints how long they ran and
    /// how often a shot hurt an enemy: what `tuning::bot`'s spreads were picked against. Run in
    /// release: `cargo test -p lobshot-lab --release --lib -- --ignored --nocapture tool_length`.
    #[test]
    #[ignore = "a tool, not a test: prints game lengths and hit rates by level"]
    fn tool_length() {
        const GAMES: u64 = 128;
        let maps = Maps::new(std::path::Path::new("no-assets"));
        let [arena] = maps.arenas(FLAT).unwrap().try_into().unwrap();
        let entrants: Vec<Entrant> = (0..8)
            .map(|i| Entrant {
                team: if i % 2 == 0 { Team::A } else { Team::B },
                mobile: Some(MobileKind::Coilback),
                items: bot::LOADOUT,
                second: None,
            })
            .collect();
        for level in [Level::Hard, Level::Normal, Level::Easy] {
            let games: Vec<(i64, u32, u32)> = std::thread::scope(|scope| {
                let running: Vec<_> = (1..=GAMES)
                    .map(|seed| {
                        let (arena, entrants, maps) = (&arena, &entrants, &maps);
                        scope.spawn(move || {
                            let mut host = Host::seat(arena, entrants, &RoomOptions::default(), vec![Some(level); 8], XorShift::new(seed * 7919), maps).unwrap();
                            let (mut shots, mut hits) = (0, 0);
                            let mut out = Vec::new();
                            while host.game().turns() < 4000 {
                                out.clear();
                                let Some(seat) = host.step(&mut out).unwrap() else { break };
                                if out.iter().any(|h| matches!(h.event, Event::Shoot { seat: s, .. } if s == seat)) {
                                    shots += 1;
                                    hits += u32::from(crate::balance::enemy_damage(host.game().players(), seat, &out) > 0);
                                }
                            }
                            (host.game().turns(), shots, hits)
                        })
                    })
                    .collect();
                running.into_iter().map(|game| game.join().unwrap()).collect()
            });
            let mut turns: Vec<i64> = games.iter().map(|game| game.0).collect();
            turns.sort_unstable();
            let (shots, hits) = games.iter().fold((0, 0), |(s, h), game| (s + game.1, h + game.2));
            let past = turns.iter().filter(|&&t| t > 40).count();
            let at = |share: usize| turns[turns.len() * share / 4];
            println!(
                "{level:?}: median {} turns (quartiles {} to {}, range {} to {}), past turn 40 in {past}/{GAMES}, {hits} of {shots} shots hurt an enemy ({:.0}%)",
                at(2),
                at(1),
                at(3),
                turns[0],
                turns[turns.len() - 1],
                f64::from(hits) * 100.0 / f64::from(shots)
            );
        }
    }
}
