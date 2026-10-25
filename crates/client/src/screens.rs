//! The screens outside the game: the title (new game, practice), the garage (your mobile and
//! items), the name prompt, and the lobby (teams, picks, readiness, the master's options, bots,
//! kicks, the invite link).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lobshot_protocol::{ClientMsg, Member, MemberId, RoomOptions, RoomView, Token};
use lobshot_sim::game::{MAX_SEATS, Team};

use crate::{
    assets::ImageCache,
    model::{Screen, Session},
    net::{Link, Socket},
    picks::{self, Picks},
    ui::{Clicked, UiAction, button, panel, small_button, team_color, text, theme},
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

/// The title: rebuilt whenever the session changes (the practice mode).
fn title(mut commands: Commands, session: Res<Session>, roots: Query<Entity, With<ScreenRoot>>) {
    if !session.is_changed() && !roots.is_empty() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let mode = session.practice_mode;
    let next = RoomOptions { mode, ..RoomOptions::default() }.next_mode().mode;
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
                children![
                    button("New game", UiAction::NewGame),
                    button("Practice vs bot", UiAction::Practice),
                    button("Practice 2v2", UiAction::PracticeTeams),
                    button("Mobile and items", UiAction::Garage)
                ]
            ),
            small_button(format!("Practice mode: {}", mode.name()), UiAction::PracticeMode(next)),
        ],
    ));
}

/// Your mobile and items, outside any room: rebuilt whenever they change.
fn garage(mut commands: Commands, session: Res<Session>, picks: Res<Picks>, server: Res<AssetServer>, mut cache: ResMut<ImageCache>, roots: Query<Entity, With<ScreenRoot>>) {
    if !picks.is_changed() && !session.is_changed() && !roots.is_empty() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let root = commands.spawn((ScreenRoot, full_screen())).id();
    commands.entity(root).with_children(|screen| {
        screen.spawn(text("Mobile and items", 36.0, theme::ACCENT));
        screen.spawn(text("You bring these into every room, and into practice.", 18.0, theme::MUTED));
        screen
            .spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                margin: UiRect::vertical(Val::Px(14.0)),
                width: Val::Px(PICKS_PANEL),
                ..default()
            }))
            .with_children(|column| {
                picks::mobile_picker(column, picks.mobile, None);
                picks::loadout_editor(column, &picks, &server, &mut cache);
            });
        if let Some(notice) = &session.notice {
            screen.spawn(text(notice.clone(), 18.0, theme::TEAM_B));
        }
        screen.spawn(button("Back", UiAction::Title));
    });
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

/// Whose mobile the lobby's picker sets: this player's (`None`), or, for the room master, a bot's.
#[derive(Resource, Default)]
pub struct PickingFor(pub Option<MemberId>);

/// What the lobby needs besides the room: this player's picks, whose the picker sets, and the art.
#[derive(bevy::ecs::system::SystemParam)]
struct LobbyArt<'w> {
    picks: Res<'w, Picks>,
    picking: Res<'w, PickingFor>,
    server: Res<'w, AssetServer>,
    cache: ResMut<'w, ImageCache>,
}

/// The picks panel: the picker and editor, and a panel's 12 px padding either side.
const PICKS_PANEL: f32 = picks::PICKS_WIDTH + 24.0;

/// The lobby's width for two team panels side by side.
const ROSTER_WIDTH: f32 = 2.0 * TEAM_WIDTH + 10.0;
const TEAM_WIDTH: f32 = 232.0;

/// Rebuilds the lobby whenever the room or this player's picks change, or it isn't up. Laid out for
/// eight players and a room of spectators in a 1024x768 window: the roster on the left, your picks
/// on the right, the buttons and the master's options under both.
fn lobby(mut commands: Commands, session: Res<Session>, mut art: LobbyArt, roots: Query<Entity, With<ScreenRoot>>) {
    if !session.is_changed() && !art.picks.is_changed() && !art.picking.is_changed() && !roots.is_empty() {
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
                padding: UiRect::all(Val::Px(12.0)),
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(6.0),
                ..full_screen()
            },
        ))
        .id();
    commands.entity(lobby_root).with_children(|screen| {
        screen
            .spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|header| {
                header.spawn(text(format!("Room {}", room.code.as_str()), 26.0, theme::ACCENT));
                header.spawn(small_button("Copy invite link", UiAction::CopyLink));
            });

        screen
            .spawn(Node {
                column_gap: Val::Px(14.0),
                align_items: AlignItems::FlexStart,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|main| {
                roster(main, room, me);
                main.spawn(panel(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    width: Val::Px(PICKS_PANEL),
                    ..default()
                }))
                .with_children(|column| {
                    // The master may be picking for one of the room's bots.
                    let bot = art.picking.0.filter(|_| master).and_then(|id| room.members.iter().find(|m| m.id == id && m.bot));
                    if let Some(bot) = bot {
                        picks::mobile_picker(column, bot.mobile, Some(&bot.name));
                        column.spawn(text("Bots bring SearchBot's items.", 13.0, theme::MUTED));
                    } else {
                        picks::mobile_picker(column, art.picks.mobile, None);
                        picks::loadout_editor(column, &art.picks, &art.server, &mut art.cache);
                    }
                });
            });

        // This player's controls, then the master's.
        screen.spawn(Node::default()).with_children(|row| {
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
            screen.spawn(text(describe(&room.options), 16.0, theme::MUTED));
        }
        if let Some(notice) = &session.notice {
            screen.spawn(text(notice.clone(), 18.0, theme::TEAM_B));
        }
    });
}

/// The teams side by side, and whoever's watching under them.
fn roster(main: &mut ChildSpawnerCommands, room: &RoomView, me: Option<MemberId>) {
    let master = me == Some(room.master);
    main.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(10.0),
        width: Val::Px(ROSTER_WIDTH),
        ..default()
    })
    .with_children(|roster| {
        roster.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|row| {
            for team in [Team::A, Team::B] {
                team_panel(row, room, team, me);
            }
        });
        let spectators: Vec<&Member> = room.members.iter().filter(|m| m.team.is_none()).collect();
        if spectators.is_empty() {
            return;
        }
        roster
            .spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            }))
            .with_children(|column| {
                column.spawn(text(format!("Watching  {}", spectators.len()), 16.0, theme::MUTED));
                column
                    .spawn(Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|names| {
                        for member in spectators {
                            spectator_chip(names, member, room.master, master && Some(member.id) != me);
                        }
                    });
            });
    });
}

/// A team's members, and the buttons to join it or (for the master) add a bot to it.
fn team_panel(row: &mut ChildSpawnerCommands, room: &RoomView, team: Team, me: Option<MemberId>) {
    let master = me == Some(room.master);
    let members: Vec<&Member> = room.members.iter().filter(|m| m.team == Some(team)).collect();
    let seated = room.members.iter().filter(|m| m.team.is_some()).count();
    let mine = room.members.iter().find(|m| Some(m.id) == me);
    row.spawn(panel(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(TEAM_WIDTH),
        row_gap: Val::Px(4.0),
        ..default()
    }))
    .with_children(|column| {
        column.spawn(text(format!("Team {team:?}  {}/{}", members.len(), room.options.team_size), 18.0, team_color(team)));
        for member in members {
            member_row(column, member, room.master, master && Some(member.id) != me);
        }
        column.spawn(Node::default()).with_children(|buttons| {
            if mine.is_some_and(|m| m.team != Some(team)) {
                buttons.spawn(small_button("Join", UiAction::Join(Some(team))));
            }
            if master && seated < MAX_SEATS {
                buttons.spawn(small_button("Add bot", UiAction::AddBot(team)));
            }
        });
    });
}

/// What sets a member apart: host, bot, away.
fn tags(member: &Member, master: MemberId) -> Vec<&'static str> {
    [(member.id == master, "host"), (member.bot, "bot"), (!member.connected, "away")]
        .into_iter()
        .filter_map(|(on, tag)| on.then_some(tag))
        .collect()
}

/// A seated member: readiness, name, then their mobile and tags underneath.
fn member_row(column: &mut ChildSpawnerCommands, member: &Member, master: MemberId, can_kick: bool) {
    let mut details = vec![picks::mobile_name(member.mobile)];
    details.extend(tags(member, master));
    column
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(9.0),
                    height: Val::Px(9.0),
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..default()
                },
                BackgroundColor(if member.ready { theme::GOOD } else { theme::BUTTON }),
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|who| {
                who.spawn(text(member.name.clone(), 15.0, theme::TEXT));
                who.spawn(text(details.join(", "), 11.0, theme::MUTED));
            });
            // The master picks the bots' mobiles.
            if can_kick && member.bot {
                row.spawn(small_button("Mobile", UiAction::PickFor(Some(member.id))));
            }
            if can_kick {
                row.spawn(small_button("Kick", UiAction::Kick(member.id)));
            }
        });
}

/// A spectator: a name (and tags) in a wrapping row.
fn spectator_chip(names: &mut ChildSpawnerCommands, member: &Member, master: MemberId, can_kick: bool) {
    let mut label = vec![member.name.clone()];
    label.extend(tags(member, master).into_iter().map(|tag| format!("({tag})")));
    let label = label.join(" ");
    names
        .spawn(Node {
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|chip| {
            chip.spawn(text(label, 14.0, theme::TEXT));
            if can_kick {
                chip.spawn(small_button("Kick", UiAction::Kick(member.id)));
            }
        });
}

fn describe(options: &RoomOptions) -> String {
    let size = options.team_size;
    let mut parts = vec![
        options.mode.name().to_owned(),
        options.map.clone(),
        format!("{size}v{size}"),
        format!("wind up to {}", options.max_wind),
        format!("{} s turns", options.turn_seconds),
    ];
    if options.random_teams {
        parts.push("random teams".to_owned());
    }
    parts.push(
        options
            .sudden_death
            .map_or_else(|| "no sudden death".to_owned(), |rule| format!("sudden death: {} at turn {}", rule.kind.name(), rule.turn)),
    );
    parts.join(", ")
}

/// The master's option buttons: each one moves its option to the next value.
fn options_row(screen: &mut ChildSpawnerCommands, options: &RoomOptions) {
    // The lobby offers the drawn maps, then the server's flat field for testing.
    let maps: Vec<&str> = lobshot_assets::MAPS.iter().copied().chain([lobshot_assets::FLAT]).collect();
    let on_off = |on: bool| if on { "on" } else { "off" };
    let configure = UiAction::Configure;
    screen
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            max_width: Val::Px(980.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(small_button(format!("Mode: {}", options.mode.name()), configure(options.next_mode())));
            row.spawn(small_button(format!("Map: {}", options.map), configure(options.next_map(&maps))));
            row.spawn(small_button(format!("Team size: {}", options.team_size), configure(options.next_team_size())));
            row.spawn(small_button(format!("Max wind: {}", options.max_wind), configure(options.next_max_wind())));
            row.spawn(small_button(format!("Turn: {} s", options.turn_seconds), configure(options.next_turn_seconds())));
            let random_teams = RoomOptions {
                random_teams: !options.random_teams,
                ..options.clone()
            };
            row.spawn(small_button(format!("Random teams: {}", on_off(options.random_teams)), configure(random_teams)));
            let sudden = options.sudden_death.map_or("off", |rule| rule.kind.name());
            row.spawn(small_button(format!("Sudden death: {sudden}"), configure(options.next_sudden_death())));
            if let Some(rule) = options.sudden_death {
                row.spawn(small_button(format!("At turn {}", rule.turn), configure(options.next_sudden_death_turn())));
            }
        });
}

/// Turns lobby and title clicks into messages to the server, or browser calls.
fn act(
    mut clicked: MessageReader<Clicked>,
    mut socket: NonSendMut<Socket>,
    mut session: ResMut<Session>,
    mut picks: ResMut<Picks>,
    mut picking: ResMut<PickingFor>,
    errors: Res<WebErrors>,
    mut next: ResMut<NextState<Screen>>,
) {
    for Clicked(action) in clicked.read() {
        // A notice answers the last thing the player did; a new action makes it stale.
        session.notice = None;
        let message = match action.clone() {
            UiAction::NewGame => {
                web::create_room(Arc::clone(&errors.0));
                None
            }
            UiAction::Practice | UiAction::PracticeTeams => {
                session.practice = true;
                session.practice_team_size = if *action == UiAction::PracticeTeams { 2 } else { 1 };
                next.set(Screen::Game);
                None
            }
            UiAction::PracticeMode(mode) => {
                session.practice_mode = mode;
                None
            }
            UiAction::Garage => {
                next.set(Screen::Garage);
                None
            }
            UiAction::Title => {
                next.set(Screen::Title);
                None
            }
            // A pick for a bot goes to the room, and the picker comes back to this player's own.
            UiAction::PickMobile(mobile) => {
                let bot = picking.0.take();
                if bot.is_none() {
                    picks.pick(mobile);
                }
                bot.map(|bot| ClientMsg::BotMobile { bot, mobile })
            }
            UiAction::PickFor(target) => {
                picking.0 = target;
                None
            }
            UiAction::AddItem(item) => {
                if let Err(refused) = picks.add(item) {
                    session.notice = Some(refused.to_string());
                }
                None
            }
            UiAction::RemoveItem(position) => {
                picks.remove(position);
                None
            }
            UiAction::ClearItems => {
                picks.clear();
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
            UiAction::Shot(_) | UiAction::Item(_) | UiAction::Spin | UiAction::Pass | UiAction::Chat => None,
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
            .init_resource::<PickingFor>()
            .add_systems(Startup, boot)
            .add_systems(OnEnter(Screen::Name), name_prompt)
            .add_systems(OnEnter(Screen::Joining), joining)
            .add_systems(OnEnter(Screen::Gone), gone)
            .add_systems(OnExit(Screen::Title), clear)
            .add_systems(OnExit(Screen::Garage), clear)
            .add_systems(OnExit(Screen::Name), clear)
            .add_systems(OnExit(Screen::Joining), clear)
            .add_systems(OnExit(Screen::Lobby), (clear, |mut picking: ResMut<PickingFor>| picking.0 = None))
            .add_systems(OnExit(Screen::Gone), clear)
            .add_systems(
                Update,
                (
                    take_name.run_if(in_state(Screen::Name)),
                    title.run_if(in_state(Screen::Title)),
                    (garage.run_if(in_state(Screen::Garage)), lobby.run_if(in_state(Screen::Lobby))).after(picks::DrawPortraits),
                    act,
                    show_web_errors,
                    watch_link,
                ),
            );
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn everyone_else_reads_the_options_in_a_line() {
        let options = RoomOptions {
            team_size: 4,
            random_teams: true,
            ..RoomOptions::default()
        };
        assert_eq!(describe(&options), "Solo, tidewall, 4v4, wind up to 26, 20 s turns, random teams, sudden death: Double at turn 40");
        let options = RoomOptions {
            sudden_death: None,
            mode: lobshot_sim::mode::Mode::Score,
            ..RoomOptions::default()
        };
        assert!(describe(&options).starts_with("Score, ") && describe(&options).ends_with("no sudden death"));
    }
}
