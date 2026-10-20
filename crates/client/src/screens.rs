//! The screens outside the game: the title (new game, practice), the name prompt, and the lobby
//! (teams, readiness, the master's options, bots, kicks, the invite link).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lobshot_protocol::{ClientMsg, Member, RoomOptions, Token};
use lobshot_sim::game::Team;

use crate::{
    model::{Screen, Session},
    net::{Link, Socket},
    ui::{Clicked, UiAction, button, panel, team_color, text, theme},
    web::{self, Page, TextInput, Typed},
};

/// The root of whichever screen is up, so it can be torn down.
#[derive(Component)]
pub struct ScreenRoot;

/// Errors from browser calls (creating a room), shown on the title.
#[derive(Resource, Clone, Default)]
pub struct WebErrors(pub Arc<Mutex<Vec<String>>>);

fn clear(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

fn full_screen() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

/// Decides the first screen from the URL.
fn boot(mut session: ResMut<Session>, mut next: ResMut<NextState<Screen>>, mut socket: NonSendMut<Socket>, input: Res<TextInput>) {
    if let Page::Room(code) = web::page() {
        session.code = Some(code.clone());
        if let Some(name) = web::load("name") {
            socket.connect(&code, hello(&code, name));
            next.set(Screen::Joining);
        } else {
            input.open("Your name", lobshot_protocol::MAX_NAME, "");
            next.set(Screen::Name);
        }
    }
}

fn hello(code: &lobshot_protocol::RoomCode, name: String) -> ClientMsg {
    let token = web::load(&format!("token.{}", code.as_str())).map(Token);
    ClientMsg::Hello {
        name,
        token,
        rider: web::load("rider").unwrap_or_else(|| "pip".to_owned()),
    }
}

fn title(mut commands: Commands) {
    commands.spawn((
        ScreenRoot,
        full_screen(),
        children![
            text("lobshot", 72.0, theme::ACCENT),
            text("Turn-based artillery. Start a game, send the link.", 20.0, theme::MUTED),
            (
                Node {
                    margin: UiRect::top(Val::Px(28.0)),
                    ..default()
                },
                children![button("New game", UiAction::NewGame), button("Practice vs bot", UiAction::Practice)]
            ),
        ],
    ));
}

fn name_prompt(mut commands: Commands) {
    commands.spawn((
        ScreenRoot,
        full_screen(),
        children![text("What should we call you?", 28.0, theme::TEXT), text("Type it and press Enter.", 18.0, theme::MUTED)],
    ));
}

fn take_name(input: Res<TextInput>, session: Res<Session>, mut socket: NonSendMut<Socket>, mut next: ResMut<NextState<Screen>>) {
    for typed in input.take() {
        let Typed::Submitted(name) = typed else {
            input.open("Your name", lobshot_protocol::MAX_NAME, "");
            continue;
        };
        let Ok(name) = lobshot_protocol::clean_name(&name) else {
            input.open("Your name (1 to 16 characters)", lobshot_protocol::MAX_NAME, &name);
            continue;
        };
        web::save("name", &name);
        if let Some(code) = &session.code {
            socket.connect(code, hello(code, name));
            next.set(Screen::Joining);
        }
    }
}

fn joining(mut commands: Commands) {
    commands.spawn((ScreenRoot, full_screen(), children![text("Joining the room...", 24.0, theme::MUTED)]));
}

fn gone(mut commands: Commands, session: Res<Session>) {
    let kicked = session.kicked;
    let why = if kicked { "The room master removed you." } else { "Lost the connection to the room." };
    let root = commands.spawn((ScreenRoot, full_screen(), children![text(why, 24.0, theme::TEXT)])).id();
    commands.entity(root).with_children(|screen| {
        // A dropped connection can rejoin (the saved token takes the seat back); a kick can't.
        if !kicked {
            screen.spawn(button("Rejoin", UiAction::Rejoin));
        }
        screen.spawn(button("Back to the title", UiAction::Leave));
    });
}

/// Rebuilds the lobby whenever the room changes.
fn lobby(mut commands: Commands, session: Res<Session>, roots: Query<Entity, With<ScreenRoot>>) {
    if !session.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(room) = session.room.as_ref() else { return };
    let me = session.me;
    let master = me == Some(room.master);
    let mine = room.members.iter().find(|m| Some(m.id) == me);

    let lobby_root = commands
        .spawn((
            ScreenRoot,
            Node {
                padding: UiRect::all(Val::Px(24.0)),
                ..full_screen()
            },
        ))
        .id();
    commands.entity(lobby_root).with_children(|screen| {
        screen.spawn(text(format!("Room {}", room.code.as_str()), 36.0, theme::ACCENT));
        screen.spawn((
            Node {
                margin: UiRect::bottom(Val::Px(12.0)),
                ..default()
            },
            children![button("Copy invite link", UiAction::CopyLink)],
        ));

        // Teams side by side, spectators under them.
        screen.spawn(Node { column_gap: Val::Px(16.0), ..default() }).with_children(|row| {
            for team in [Team::A, Team::B] {
                row.spawn(panel(Node {
                    flex_direction: FlexDirection::Column,
                    min_width: Val::Px(260.0),
                    ..default()
                }))
                .with_children(|column| {
                    column.spawn(text(format!("Team {team:?}  ({} max)", room.options.team_size), 22.0, team_color(team)));
                    for member in room.members.iter().filter(|m| m.team == Some(team)) {
                        member_row(column, member, room.master, master && Some(member.id) != me);
                    }
                    if mine.is_some_and(|m| m.team != Some(team)) {
                        column.spawn(button("Join", UiAction::Join(Some(team))));
                    }
                    if master {
                        column.spawn(button("Add bot", UiAction::AddBot(team)));
                    }
                });
            }
        });
        let spectators: Vec<&Member> = room.members.iter().filter(|m| m.team.is_none()).collect();
        if !spectators.is_empty() {
            screen
                .spawn(panel(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(12.0)),
                    min_width: Val::Px(536.0),
                    ..default()
                }))
                .with_children(|column| {
                    column.spawn(text("Watching", 20.0, theme::MUTED));
                    for member in spectators {
                        member_row(column, member, room.master, master && Some(member.id) != me);
                    }
                });
        }

        // This player's controls, then the master's.
        screen
            .spawn(Node {
                margin: UiRect::top(Val::Px(16.0)),
                ..default()
            })
            .with_children(|row| {
                if let Some(mine) = mine
                    && mine.team.is_some()
                {
                    row.spawn(button(if mine.ready { "Not ready" } else { "Ready" }, UiAction::Ready(!mine.ready)));
                    row.spawn(button("Watch instead", UiAction::Join(None)));
                }
                if master {
                    row.spawn(button("Start", UiAction::Start));
                }
            });
        if master {
            options_row(screen, &room.options);
        } else {
            screen.spawn(text(describe(&room.options), 18.0, theme::MUTED));
        }
        if let Some(notice) = &session.notice {
            screen.spawn(text(notice.clone(), 18.0, theme::TEAM_B));
        }
    });
}

fn member_row(column: &mut ChildSpawnerCommands, member: &Member, master_id: lobshot_protocol::MemberId, can_kick: bool) {
    let mut label = member.name.clone();
    if member.id == master_id {
        label.push_str("  (host)");
    }
    if member.bot {
        label.push_str("  (bot)");
    }
    if !member.connected {
        label.push_str("  (away)");
    }
    let status = if member.ready { theme::GOOD } else { theme::MUTED };
    column
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(text(if member.ready { "ready" } else { "-" }, 16.0, status));
            row.spawn(text(label, 18.0, theme::TEXT));
            if can_kick {
                row.spawn(button("Kick", UiAction::Kick(member.id)));
            }
        });
}

fn describe(options: &RoomOptions) -> String {
    format!("{}  /  wind up to {}  /  {} s turns", options.map, options.max_wind, options.turn_seconds)
}

/// The master's option buttons: each one moves its option to the next value.
fn options_row(screen: &mut ChildSpawnerCommands, options: &RoomOptions) {
    let next_of = |values: &[u8], current: u8| values[(values.iter().position(|&v| v == current).unwrap_or(0) + 1) % values.len()];
    let with = |change: &dyn Fn(&mut RoomOptions)| {
        let mut options = options.clone();
        change(&mut options);
        UiAction::Configure(options)
    };
    let map = if options.map == "flat" { "tidewall" } else { "flat" };
    screen
        .spawn(Node {
            margin: UiRect::top(Val::Px(8.0)),
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(button(format!("Map: {}", options.map), with(&|o| map.clone_into(&mut o.map))));
            row.spawn(button(format!("Team size: {}", options.team_size), with(&|o| o.team_size = next_of(&[1, 2, 3, 4], o.team_size))));
            row.spawn(button(format!("Max wind: {}", options.max_wind), with(&|o| o.max_wind = next_of(&[0, 12, 26], o.max_wind))));
            row.spawn(button(format!("Turn: {} s", options.turn_seconds), with(&|o| o.turn_seconds = next_of(&[5, 10, 20], o.turn_seconds))));
        });
}

/// Turns lobby and title clicks into messages to the server, or browser calls.
fn act(mut clicked: MessageReader<Clicked>, mut socket: NonSendMut<Socket>, mut session: ResMut<Session>, errors: Res<WebErrors>, mut next: ResMut<NextState<Screen>>) {
    for Clicked(action) in clicked.read() {
        // A notice answers the last thing the player did; a new action makes it stale.
        session.notice = None;
        let message = match action.clone() {
            UiAction::NewGame => {
                web::create_room(Arc::clone(&errors.0));
                None
            }
            UiAction::Practice => {
                session.practice = true;
                next.set(Screen::Game);
                None
            }
            UiAction::CopyLink => {
                web::copy(&web::href());
                session.notice = Some("Invite link copied.".to_owned());
                None
            }
            UiAction::Leave => {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().set_href("/");
                }
                None
            }
            UiAction::Rejoin => {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
                None
            }
            UiAction::Join(team) => Some(ClientMsg::Team(team)),
            UiAction::Ready(ready) => Some(ClientMsg::Ready(ready)),
            UiAction::AddBot(team) => Some(ClientMsg::AddBot { team }),
            UiAction::Kick(member) => Some(ClientMsg::Kick(member)),
            UiAction::Start => Some(ClientMsg::Start),
            UiAction::Configure(options) => Some(ClientMsg::Configure(options)),
            UiAction::Shot(_) | UiAction::Pass | UiAction::Chat => None,
        };
        if let Some(message) = message {
            socket.send(&message);
        }
    }
}

/// Shows browser errors (creating a room) on the title.
fn show_web_errors(errors: Res<WebErrors>, mut session: ResMut<Session>) {
    if let Ok(mut errors) = errors.0.lock()
        && let Some(error) = errors.pop()
    {
        session.notice = Some(error);
    }
}

/// A connection dropped while in a room: say so, and offer the way back.
fn watch_link(socket: NonSend<Socket>, session: Res<Session>, screen: Res<State<Screen>>, mut next: ResMut<NextState<Screen>>) {
    let in_room = matches!(screen.get(), Screen::Joining | Screen::Lobby | Screen::Game);
    if in_room && !session.practice && socket.link() == Link::Closed {
        next.set(Screen::Gone);
    }
}

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WebErrors>()
            .add_systems(Startup, boot)
            .add_systems(OnEnter(Screen::Title), title)
            .add_systems(OnEnter(Screen::Name), name_prompt)
            .add_systems(OnEnter(Screen::Joining), joining)
            .add_systems(OnEnter(Screen::Gone), gone)
            .add_systems(OnExit(Screen::Title), clear)
            .add_systems(OnExit(Screen::Name), clear)
            .add_systems(OnExit(Screen::Joining), clear)
            .add_systems(OnExit(Screen::Lobby), clear)
            .add_systems(OnExit(Screen::Gone), clear)
            .add_systems(
                Update,
                (take_name.run_if(in_state(Screen::Name)), lobby.run_if(in_state(Screen::Lobby)), act, show_web_errors, watch_link),
            );
    }
}
