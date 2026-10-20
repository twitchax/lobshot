//! What the client knows: the room (from the server), and the match as two games. `confirmed` is
//! the server's events applied in order; `predicted` is `confirmed` plus this player's walks the
//! server hasn't echoed yet, so walking feels instant. The sim is deterministic, so once the echo
//! arrives the two agree; if the server says otherwise, the prediction is dropped.

use std::collections::VecDeque;

use bevy::prelude::*;
use lobshot_protocol::{MemberId, RoomCode, RoomView, ServerMsg, Stage};
use lobshot_sim::{
    game::{Effect, Event, Game, Team},
    log::Log,
    map::{MapAssets, MapRef},
};

use crate::{assets::Masks, game::TurnClock, net::Inbound, sound::SoundLog, ui::ButtonSpots, web};

/// Which screen is up.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Title,
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
    /// The map the game is on, for drawing it.
    pub map: MapRef,
    pub confirmed: Game,
    pub predicted: Game,
    /// This player's walks, sent but not yet echoed.
    pub pending: VecDeque<Event>,
    /// Events applied to `confirmed` (the log index of the next).
    pub applied: u32,
    /// The server's hash disagreed with ours: the client is out of sync and says so.
    pub desynced: bool,
}

impl Match {
    #[must_use]
    pub fn new(map: MapRef, game: Game, applied: u32) -> Self {
        Self {
            map,
            predicted: game.clone(),
            confirmed: game,
            pending: VecDeque::new(),
            applied,
            desynced: false,
        }
    }

    /// Applies an event with no server (practice): the client is its own referee.
    ///
    /// # Errors
    ///
    /// Fails when the sim refuses the event.
    pub fn referee(&mut self, event: Event) -> lobshot_sim::base::Res<Vec<Effect>> {
        let effects = self.confirmed.apply(event)?;
        self.applied += 1;
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
        self.applied += 1;
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
                    next.set(Screen::Lobby);
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
                    if index == game.applied {
                        let effects = game.confirm(event, hash);
                        played.write(Played { event, effects });
                    } else {
                        warn!("event {index} arrived, but {} was next", game.applied);
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
    let mut started = Match::new(loading.log.setup.map.clone(), game, u32::try_from(loading.log.events.len()).unwrap_or(u32::MAX));
    for &(index, event, hash) in &loading.queued {
        if index == started.applied {
            let effects = started.confirm(event, hash);
            played.write(Played { event, effects });
        }
    }
    commands.insert_resource(started);
    commands.remove_resource::<Loading>();
    next.set(Screen::Game);
}

/// Lets the browser smoke test see the screen, the game's progress (applied, and shown), the
/// sounds, and where to click.
fn publish(screen: Res<State<Screen>>, game: Option<Res<Match>>, session: Res<Session>, clock: Res<TurnClock>, sounds: Res<SoundLog>, buttons: Res<ButtonSpots>) {
    web::publish(&web::Published {
        screen: format!("{:?}", screen.get()),
        turns: game.map(|game| game.confirmed.turns()),
        started: clock.starts,
        seat: session.my_seat(),
        sounds: &sounds,
        buttons: &buttons,
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
