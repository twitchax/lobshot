//! What the client knows: the room (from the server), and the match as two games. `confirmed` is
//! the server's events applied in order; `predicted` is `confirmed` plus this player's walks the
//! server hasn't echoed yet, so walking feels instant. The sim is deterministic, so once the echo
//! arrives the two agree; if the server says otherwise, the prediction is dropped.

use std::collections::VecDeque;

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{MemberId, RoomCode, RoomView, ServerMsg, Stage};
use lobshot_sim::{
    game::{Effect, Event, Game, Player, SuddenDeath, SuddenDeathKind, Team},
    log::Log,
    map::{MapAssets, MapRef},
    mode::Mode,
};

use crate::{
    assets::Masks,
    game::{MapPointer, TurnClock},
    net::Inbound,
    picks::Picks,
    sound::SoundLog,
    ui::ButtonSpots,
    web,
};

/// Which screen is up.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Title,
    /// Your mobile and items, outside any room (they're used in rooms and in practice).
    Garage,
    /// Asking for a name before joining.
    Name,
    /// Connected or connecting, waiting for the room.
    Joining,
    Lobby,
    Game,
    /// Kicked, or the room is gone.
    Gone,
}

/// A chat line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    pub name: String,
    pub text: String,
    pub team_only: bool,
}

/// The room, as the server last described it.
#[derive(Resource, Default)]
pub struct Session {
    pub code: Option<RoomCode>,
    pub me: Option<MemberId>,
    pub room: Option<RoomView>,
    pub chat: Vec<ChatLine>,
    /// The server's last refusal or a connection problem, shown briefly.
    pub notice: Option<String>,
    /// Practice against a bot, with no server.
    pub practice: bool,
    /// Players per team in practice: 1 (you against a bot) or 2 (you and a bot against two).
    pub practice_team_size: u8,
    /// What practice is played for: the title steps through the modes rooms play.
    pub practice_mode: Mode,
    /// The master removed this player (the token is revoked, so there is no rejoining).
    pub kicked: bool,
    /// Who sat where in the current (or just-finished) game, by seat.
    pub seats: Vec<MemberId>,
    /// The game ended and the room went back to its lobby; the client follows once the last shot
    /// and the result have had their moment.
    pub lobby_waiting: bool,
}

impl Session {
    /// This player's seat in the current game, if they have one.
    #[must_use]
    pub fn my_seat(&self) -> Option<u8> {
        let me = self.me?;
        self.seats.iter().position(|&id| id == me).and_then(|seat| u8::try_from(seat).ok())
    }

    /// This player's team in `game`, if they play in it.
    #[must_use]
    pub fn my_team(&self, game: &Game) -> Option<Team> {
        self.my_seat().and_then(|seat| game.players().get(usize::from(seat))).map(|player| player.team)
    }

    /// The name of whoever plays `seat`.
    #[must_use]
    pub fn seat_name(&self, seat: u8) -> String {
        self.seats.get(usize::from(seat)).map_or_else(|| format!("Seat {seat}"), |&id| self.name_of(id))
    }

    /// A member's display name.
    #[must_use]
    pub fn name_of(&self, id: MemberId) -> String {
        self.room
            .as_ref()
            .and_then(|room| room.members.iter().find(|m| m.id == id))
            .map_or_else(|| "?".to_owned(), |m| m.name.clone())
    }
}

/// A game in progress.
#[derive(Resource)]
pub struct Match {
    /// The game so far: its setup (the map to draw, each seat's loadout, sudden death) and the
    /// events applied to `confirmed`, which the scoreboard tallies when it ends.
    pub log: Log,
    pub confirmed: Game,
    pub predicted: Game,
    /// This player's walks, sent but not yet echoed.
    pub pending: VecDeque<Event>,
    /// The server's hash disagreed with ours: the client is out of sync and says so.
    pub desynced: bool,
}

impl Match {
    /// A match at the end of `log`, which `game` is.
    #[must_use]
    pub fn new(log: Log, game: Game) -> Self {
        Self {
            log,
            predicted: game.clone(),
            confirmed: game,
            pending: VecDeque::new(),
            desynced: false,
        }
    }

    /// Events applied so far: the log index of the next.
    #[must_use]
    pub fn applied(&self) -> u32 {
        u32::try_from(self.log.events.len()).unwrap_or(u32::MAX)
    }

    /// Applies an event with no server (practice): the client is its own referee.
    ///
    /// # Errors
    ///
    /// Fails when the sim refuses the event.
    pub fn referee(&mut self, event: Event) -> lobshot_sim::base::Res<Vec<Effect>> {
        let effects = self.confirmed.apply(event)?;
        self.log.events.push(event);
        self.predicted = self.confirmed.clone();
        Ok(effects)
    }

    /// Applies one of this player's walks at once, ahead of the server.
    pub fn predict(&mut self, walk: Event) -> bool {
        if self.predicted.apply(walk).is_ok() {
            self.pending.push_back(walk);
            true
        } else {
            false
        }
    }

    /// The seat's player in the predicted game.
    #[must_use]
    pub fn player(&self, seat: u8) -> Option<&Player> {
        self.predicted.players().get(usize::from(seat))
    }

    /// Where sudden death stands now: the sim says whether it's on, the setup when it will be.
    #[must_use]
    pub fn sudden_death(&self) -> Sudden {
        sudden(self.predicted.sudden_death(), self.log.setup.sudden_death, self.predicted.turns())
    }

    /// Applies the server's next event; returns its effects for playback.
    fn confirm(&mut self, event: Event, hash: u64) -> Vec<Effect> {
        let effects = match self.confirmed.apply(event) {
            Ok(effects) => effects,
            Err(error) => {
                error!("the server's event {event:?} doesn't apply here: {error:#}");
                self.desynced = true;
                Vec::new()
            }
        };
        // Logged even when it didn't apply here: the server's log has it, and indices must agree.
        self.log.events.push(event);
        if self.confirmed.state_hash().ok() != Some(hash) {
            self.desynced = true;
        }
        if self.pending.front() == Some(&event) {
            self.pending.pop_front();
        } else {
            // The server went another way (a timeout, a refusal): drop the prediction.
            self.pending.clear();
        }
        self.predicted = self.confirmed.clone();
        let pending: Vec<Event> = self.pending.iter().copied().collect();
        for walk in pending {
            if self.predicted.apply(walk).is_err() {
                self.pending.clear();
                self.predicted = self.confirmed.clone();
                break;
            }
        }
        effects
    }
}

/// Where a game's sudden death stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sudden {
    /// The room plays without it.
    Never,
    /// It starts once this many more turns are done.
    In { turns: i64, kind: SuddenDeathKind },
    /// It's on: items are off, and SS follows `kind`.
    On(SuddenDeathKind),
}

/// Sudden death: `on` (the sim's `Game::sudden_death`) once it has started, otherwise counting the
/// turns until the setup's `rule` starts it.
#[must_use]
pub fn sudden(on: Option<SuddenDeathKind>, rule: Option<SuddenDeath>, turns: i64) -> Sudden {
    match (on, rule) {
        (Some(kind), _) => Sudden::On(kind),
        (None, Some(SuddenDeath { kind, turn })) if i64::from(turn) > turns => Sudden::In { turns: i64::from(turn) - turns, kind },
        (None, _) => Sudden::Never,
    }
}

/// A game the server sent while its map is still loading.
#[derive(Resource)]
pub struct Loading {
    pub log: Log,
    pub queued: Vec<(u32, Event, u64)>,
}

/// The systems that apply events to the match and announce each as `Played`. Playback runs after
/// them, so it never finds the game ahead of the scenes it has queued.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplyEvents;

/// An accepted event and its effects, in order, to show.
#[derive(Message, Debug, Clone)]
pub struct Played {
    pub event: Event,
    pub effects: Vec<Effect>,
}

/// Handles every server message.
pub fn receive(
    mut inbound: MessageReader<Inbound>,
    mut session: ResMut<Session>,
    mut game: Option<ResMut<Match>>,
    mut loading: Option<ResMut<Loading>>,
    mut next: ResMut<NextState<Screen>>,
    mut played: MessageWriter<Played>,
    mut commands: Commands,
) {
    // A game that starts this frame: events after it in the same batch queue here, because the
    // inserted resource only appears next frame.
    let mut fresh: Option<Loading> = None;
    for Inbound(message) in inbound.read() {
        match message.clone() {
            ServerMsg::Welcome { you, token } => {
                session.me = Some(you);
                if let Some(code) = &session.code {
                    web::save(&format!("token.{}", code.as_str()), &token.0);
                }
            }
            ServerMsg::Room(room) => {
                if let Stage::Playing { seats } = &room.stage {
                    session.seats.clone_from(seats);
                    session.lobby_waiting = false;
                } else if game.is_some() {
                    // Let the game screen finish showing the end first.
                    session.lobby_waiting = true;
                } else {
                    commands.remove_resource::<Loading>();
                    // Every room update lands here; `set` would re-run the lobby's exit and enter
                    // (Bevy 0.20 transitions to the same state too), wiping the screen each time.
                    next.set_if_different(Screen::Lobby);
                }
                session.room = Some(room);
            }
            ServerMsg::Chat { name, text, team_only, .. } => session.chat.push(ChatLine { name, text, team_only }),
            ServerMsg::Emote { from, emote } => {
                let name = session.name_of(from);
                session.chat.push(ChatLine {
                    name,
                    text: format!("[{emote:?}]"),
                    team_only: false,
                });
            }
            ServerMsg::Game { log } => {
                commands.remove_resource::<Match>();
                fresh = Some(Loading { log, queued: Vec::new() });
                game = None;
                loading = None;
            }
            ServerMsg::Event { index, event, hash } => {
                if let Some(fresh) = fresh.as_mut() {
                    fresh.queued.push((index, event, hash));
                } else if let Some(game) = game.as_deref_mut() {
                    if index == game.applied() {
                        let effects = game.confirm(event, hash);
                        played.write(Played { event, effects });
                    } else {
                        warn!("event {index} arrived, but {} was next", game.applied());
                        game.desynced = true;
                    }
                } else if let Some(loading) = loading.as_deref_mut() {
                    loading.queued.push((index, event, hash));
                }
            }
            ServerMsg::Rejected { reason } => session.notice = Some(reason),
            ServerMsg::Kicked => {
                session.kicked = true;
                next.set(Screen::Gone);
            }
        }
    }
    if let Some(fresh) = fresh {
        commands.insert_resource(fresh);
    }
}

/// Starts the match once its map's mask has loaded.
pub fn finish_loading(loading: Option<Res<Loading>>, mut masks: Masks, mut commands: Commands, mut next: ResMut<NextState<Screen>>, mut played: MessageWriter<Played>) {
    let Some(loading) = loading else { return };
    if let MapRef::Asset { name, variant, .. } = &loading.log.setup.map
        && !masks.ready(name, variant)
    {
        masks.request(name, variant);
        return;
    }
    let assets: &dyn MapAssets = &masks;
    let game = match loading.log.replay(assets) {
        Ok(game) => game,
        Err(error) => {
            error!("couldn't replay the game: {error:#}");
            commands.remove_resource::<Loading>();
            return;
        }
    };
    let mut started = Match::new(loading.log.clone(), game);
    for &(index, event, hash) in &loading.queued {
        if index == started.applied() {
            let effects = started.confirm(event, hash);
            played.write(Played { event, effects });
        }
    }
    commands.insert_resource(started);
    commands.remove_resource::<Loading>();
    next.set(Screen::Game);
}

/// What the page shows besides the game: the sounds, the buttons, and the map under the pointer.
#[derive(SystemParam)]
struct PageParts<'w> {
    sounds: Res<'w, SoundLog>,
    buttons: Res<'w, ButtonSpots>,
    pointer: Option<Res<'w, MapPointer>>,
}

/// Lets the browser smoke test see the screen, the game's progress (applied, and shown), this
/// player's picks and state, the sounds, where to click, and the map column under the pointer.
fn publish(screen: Res<State<Screen>>, game: Option<Res<Match>>, session: Res<Session>, picks: Res<Picks>, clock: Res<TurnClock>, page: PageParts) {
    let seat = session.my_seat();
    let me = game.as_deref().zip(seat).and_then(|(game, seat)| {
        Some(web::Me {
            player: game.player(seat)?,
            ss: game.predicted.ss_state(seat).ok()?,
            can_spin: game.confirmed.can_spin(seat),
            can_respawn: game.confirmed.can_respawn(seat),
        })
    });
    web::publish(&web::Published {
        screen: format!("{:?}", screen.get()),
        turns: game.as_ref().map(|game| game.confirmed.turns()),
        started: clock.starts,
        seat,
        picks: &picks,
        me,
        sudden_death: game.as_ref().map(|game| game.sudden_death()),
        sounds: &page.sounds,
        buttons: &page.buttons,
        map_x: page.pointer.and_then(|pointer| pointer.0),
    });
}

pub struct ModelPlugin;

impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Screen>()
            .init_resource::<Session>()
            .add_message::<Played>()
            .add_systems(Update, ((receive, finish_loading).chain().in_set(ApplyEvents), publish));
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use lobshot_sim::{
        drops::{Drop, DropKind},
        game::{MobileKind, Roll, Seat, Setup},
        items::{Item, Loadout},
        map::NoAssets,
        units::{Centis, Facing, Wind},
        weather::{Forecast, WeatherKind},
    };

    use super::*;

    fn setup(items: &[Item]) -> Setup {
        let seat = |team, spawn_x, facing| Seat {
            team,
            mobile: MobileKind::Coilback,
            spawn_x,
            facing,
            items: Loadout::new(items).unwrap(),
        };
        Setup {
            map: MapRef::Flat { width: 1600, height: 900, ground: 700 },
            seats: vec![seat(Team::A, 400, Facing::Right), seat(Team::B, 1100, Facing::Left)],
            order: vec![0, 1],
            wind: Wind::default(),
            sudden_death: None,
            mode: Mode::Solo,
        }
    }

    /// Answers whatever the game owes, as a server would (any answer will do).
    fn answer_rolls(game: &mut Match) {
        while let Some(roll) = game.confirmed.pending_roll() {
            let event = match roll {
                Roll::Weather { .. } => Event::Weather(Forecast::new(WeatherKind::Force, 1)),
                Roll::Wind => Event::Wind(Wind::default()),
                Roll::Drop { .. } => Event::Drop(Drop { kind: DropKind::Calm, x: 1 }),
            };
            game.referee(event).unwrap();
        }
    }

    #[derive(Resource, Default)]
    struct Exits(u32);

    #[test]
    fn room_updates_in_the_lobby_stay_in_it() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<Screen>()
            .init_resource::<Session>()
            .init_resource::<Exits>()
            .add_message::<Inbound>()
            .add_message::<Played>()
            .add_systems(OnExit(Screen::Lobby), |mut exits: ResMut<Exits>| exits.0 += 1)
            .add_systems(Update, receive);
        let room = RoomView {
            code: lobshot_protocol::RoomCode::from_indices([0; lobshot_protocol::RoomCode::LEN]),
            master: MemberId(1),
            options: lobshot_protocol::RoomOptions::default(),
            members: Vec::new(),
            stage: Stage::Lobby,
        };
        for _ in 0..4 {
            app.world_mut().write_message(Inbound(ServerMsg::Room(room.clone())));
            app.update();
        }
        assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Lobby);
        assert_eq!(app.world().resource::<Exits>().0, 0, "each update re-entered the lobby, which clears it");
    }

    #[test]
    fn sudden_death_counts_down_until_the_sim_starts_it() {
        let kind = SuddenDeathKind::Double;
        let rule = Some(SuddenDeath { kind, turn: 40 });
        assert_eq!(sudden(None, None, 100), Sudden::Never);
        assert_eq!(sudden(None, rule, 0), Sudden::In { turns: 40, kind });
        assert_eq!(sudden(None, rule, 39), Sudden::In { turns: 1, kind });
        assert_eq!(sudden(Some(kind), rule, 40), Sudden::On(kind));
        assert_eq!(sudden(Some(kind), rule, 90), Sudden::On(kind));
    }

    #[test]
    fn a_match_reads_sudden_death_from_the_sim_and_its_setup() {
        let mut setup = setup(&[Item::Dual]);
        setup.sudden_death = Some(SuddenDeath { kind: SuddenDeathKind::BigBomb, turn: 40 });
        let game = Game::new(&setup, &NoAssets).unwrap();
        let mut game = Match::new(Log { setup, events: Vec::new() }, game);
        answer_rolls(&mut game);
        assert_eq!(
            game.sudden_death(),
            Sudden::In {
                turns: 40,
                kind: SuddenDeathKind::BigBomb
            }
        );
        // Seat 0 moves first; the countdown counts the turns the log has done.
        let before = game.applied();
        game.referee(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        assert_eq!(game.applied(), before + 1, "the log grows with each event");
        assert_eq!(
            game.sudden_death(),
            Sudden::In {
                turns: 39,
                kind: SuddenDeathKind::BigBomb
            }
        );
    }
}
