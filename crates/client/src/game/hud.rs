//! The HUD, in GunBound's functional layout: weather queue, wind dial and turn timer across the top;
//! turn list, shots, angle, power bar and move bar along the bottom; chat above the turn list; and
//! the aim arc, with its true-angle wedge, around your mobile.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_sim::{game::Shot, weather::WeatherKind};

use super::{
    GameRoot,
    input::Controls,
    playback::{Ending, Outcome, TurnClock},
    world,
    world::{MobileView, Shown, current_seat},
};
use crate::{
    model::{Match, Screen, Session},
    ui::{UiAction, button, panel, team_color, text, theme},
};

/// The power bar's width, in UI pixels: 100 per bar, as in GunBound.
const POWER_WIDTH: f32 = 400.0;

/// A text the HUD keeps current.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Readout {
    Wind,
    Timer,
    Turn,
    Angle,
}

/// A bar the HUD keeps current.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Bar {
    /// The charged power.
    Power,
    /// Last turn's power, as a marker.
    Last,
    /// The move budget left.
    Moves,
}

#[derive(Component)]
struct WindArrow;
#[derive(Component)]
struct ShotTab(Shot);
#[derive(Component)]
struct WeatherSlot(usize);
#[derive(Component)]
struct TurnList;
#[derive(Component)]
struct ChatLines;
#[derive(Component)]
struct Banner;

fn build(mut commands: Commands) {
    let root = commands
        .spawn((
            GameRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .id();
    commands.entity(root).with_children(|hud| {
        hud.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::FlexStart,
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(top_bar);
        hud.spawn((
            Banner,
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
            children![text("", 30.0, theme::ACCENT)],
        ));
        hud.spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(bottom_bar);
    });
}

/// Weather queue, wind dial, timer.
fn top_bar(top: &mut ChildSpawnerCommands) {
    top.spawn(panel(Node {
        column_gap: Val::Px(6.0),
        align_items: AlignItems::Center,
        ..default()
    }))
    .with_children(|weather| {
        weather.spawn(text("Weather", 14.0, theme::MUTED));
        for slot in 0..5 {
            weather.spawn((WeatherSlot(slot), round(30.0), BackgroundColor(theme::BUTTON), children![text("", 14.0, theme::TEXT)]));
        }
    });
    top.spawn(panel(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        ..default()
    }))
    .with_children(|wind| {
        wind.spawn((
            round(64.0),
            BackgroundColor(theme::BUTTON),
            children![
                (
                    WindArrow,
                    Node {
                        width: Val::Px(50.0),
                        height: Val::Px(4.0),
                        ..default()
                    },
                    BackgroundColor(theme::ACCENT)
                ),
                (Readout::Wind, text("0", 18.0, theme::TEXT))
            ],
        ));
        wind.spawn(text("Wind", 13.0, theme::MUTED));
    });
    top.spawn(panel(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::FlexEnd,
        ..default()
    }))
    .with_children(|clock| {
        clock.spawn((Readout::Timer, text("20", 34.0, theme::ACCENT)));
        clock.spawn((Readout::Turn, text("", 15.0, theme::TEXT)));
    });
}

/// A circle `size` across, its content centered.
fn round(size: f32) -> Node {
    Node {
        width: Val::Px(size),
        height: Val::Px(size),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(Val::Percent(50.0)),
        ..default()
    }
}

/// Chat over the turn list and the controls.
fn bottom_bar(bottom: &mut ChildSpawnerCommands) {
    bottom.spawn((
        ChatLines,
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
    ));
    bottom
        .spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::FlexEnd,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                TurnList,
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    min_width: Val::Px(170.0),
                    ..default()
                }),
            ));
            row.spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            }))
            .with_children(controls);
        });
}

/// Shots, angle, pass and chat; then the power bar and the move bar.
fn controls(controls: &mut ChildSpawnerCommands) {
    controls
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|line| {
            for shot in [Shot::S1, Shot::S2, Shot::Ss] {
                line.spawn((ShotTab(shot), button(shot_label(shot), UiAction::Shot(shot))));
            }
            line.spawn((Readout::Angle, text("Angle --", 18.0, theme::TEXT)));
            line.spawn(button("Pass", UiAction::Pass));
            line.spawn(button("Chat", UiAction::Chat));
        });
    let track = |height: f32| {
        (
            Node {
                width: Val::Px(POWER_WIDTH),
                height: Val::Px(height),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        )
    };
    // The power bar: 4 bars of 100 px, ticks at each bar, last turn's power marked.
    controls.spawn(track(18.0)).with_children(|bar| {
        bar.spawn((
            Bar::Power,
            Node {
                width: Val::Px(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme::ACCENT),
        ));
        for tick in [100.0, 200.0, 300.0] {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(tick),
                    width: Val::Px(2.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(theme::MUTED),
            ));
        }
        bar.spawn((
            Bar::Last,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-10.0),
                width: Val::Px(3.0),
                height: Val::Percent(140.0),
                top: Val::Percent(-20.0),
                ..default()
            },
            BackgroundColor(theme::TEAM_B),
        ));
    });
    controls.spawn(track(6.0)).with_children(|bar| {
        bar.spawn((
            Bar::Moves,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme::GOOD),
        ));
    });
}

fn shot_label(shot: Shot) -> &'static str {
    match shot {
        Shot::S1 => "1",
        Shot::S2 => "2",
        Shot::Ss => "SS",
    }
}

fn weather_look(kind: WeatherKind) -> (&'static str, Color) {
    match kind {
        WeatherKind::Wind => ("W", Color::srgb_u8(0x8f, 0xd3, 0xff)),
        WeatherKind::Force => ("F", Color::srgb_u8(0xe8, 0x6a, 0x3a)),
        WeatherKind::Mirror => ("M", Color::srgb_u8(0xb7, 0x9c, 0xf2)),
        WeatherKind::Tornado => ("T", Color::srgb_u8(0x6f, 0xc4, 0x8a)),
    }
}

/// The HUD's live parts.
#[derive(SystemParam)]
struct Readouts<'w, 's> {
    texts: Query<'w, 's, (&'static Readout, &'static mut Text)>,
    bars: Query<'w, 's, (&'static Bar, &'static mut Node)>,
    arrow: Query<'w, 's, &'static mut UiTransform, With<WindArrow>>,
}

fn update(game: Option<Res<Match>>, session: Res<Session>, controls: Res<Controls>, clock: Res<TurnClock>, time: Res<Time>, mut readouts: Readouts) {
    let Some(game) = game else { return };
    let wind = game.predicted.wind();
    for mut transform in &mut readouts.arrow {
        // UI rotation turns clockwise on screen; wind directions count counterclockwise.
        transform.rotation = Rot2::degrees(-f32::from(wind.direction));
    }

    let seconds = clock.seconds_left(&session, time.elapsed_secs_f64());
    let whose = clock.seat.map_or_else(String::new, |seat| {
        if Some(seat) == session.my_seat() {
            return "Your turn".to_owned();
        }
        format!("{}'s turn", session.seat_name(seat))
    });
    for (readout, mut text) in &mut readouts.texts {
        let value = match readout {
            Readout::Wind => wind.strength.to_string(),
            Readout::Timer => {
                if clock.seat.is_some() {
                    seconds.to_string()
                } else {
                    String::new()
                }
            }
            Readout::Turn => whose.clone(),
            Readout::Angle => match controls.last_aim {
                // The default font is ASCII only, so no degree sign until lobshot ships a font.
                Some(last) => format!("Angle {} (last {last})", controls.aim),
                None => format!("Angle {}", controls.aim),
            },
        };
        if **text != value {
            **text = value;
        }
    }

    let moves = session
        .my_seat()
        .and_then(|seat| game.predicted.players().get(usize::from(seat)))
        .map(|me| f32::from(me.moves_left) / f32::from(me.tuning().move_budget.value.max(1)));
    for (bar, mut node) in &mut readouts.bars {
        match bar {
            Bar::Power => node.width = Val::Px(POWER_WIDTH * controls.power / 4.0),
            Bar::Last => node.left = Val::Px(controls.last_power.map_or(-10.0, |bars| POWER_WIDTH * bars / 4.0 - 1.0)),
            Bar::Moves => {
                if let Some(moves) = moves {
                    node.width = Val::Percent(100.0 * moves);
                }
            }
        }
    }
}

/// The HUD's lists and toggles.
#[derive(SystemParam)]
struct Lists<'w, 's> {
    slots: Query<'w, 's, (&'static WeatherSlot, &'static mut BackgroundColor, &'static Children)>,
    tabs: Query<'w, 's, (&'static ShotTab, &'static mut BorderColor, &'static mut Node)>,
    texts: Query<'w, 's, &'static mut Text>,
    turns: Query<'w, 's, Entity, With<TurnList>>,
    chats: Query<'w, 's, Entity, With<ChatLines>>,
}

fn weather_and_turns(game: Option<Res<Match>>, session: Res<Session>, controls: Res<Controls>, mut lists: Lists, mut commands: Commands) {
    let Some(game) = game else { return };
    if !game.is_changed() && !session.is_changed() && !controls.is_changed() {
        return;
    }
    let queue = &game.predicted.weather().queue;
    for (slot, mut color, children) in &mut lists.slots {
        let (label, shade) = queue.get(slot.0).map_or(("", theme::BUTTON), |forecast| weather_look(forecast.kind));
        color.0 = shade;
        for child in children.iter() {
            if let Ok(mut text) = lists.texts.get_mut(child) {
                label.clone_into(&mut **text);
            }
        }
    }
    for (tab, mut border, mut node) in &mut lists.tabs {
        let selected = tab.0 == controls.shot;
        node.border = UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 }));
        *border = BorderColor::all(if selected { theme::ACCENT } else { Color::NONE });
    }

    // The turn list: everyone alive, by delay, shown relative to you (GunBound's ± list).
    let mine = session.my_seat().and_then(|seat| game.predicted.players().get(usize::from(seat))).map(|p| i64::from(p.clock.delay));
    let mut order: Vec<(usize, &lobshot_sim::game::Player)> = game.predicted.players().iter().enumerate().filter(|(_, p)| p.alive).collect();
    order.sort_by_key(|(seat, p)| (p.clock.delay, p.clock.last_turn, *seat));
    let current = current_seat(&game);
    for list in &lists.turns {
        commands.entity(list).despawn_children();
        commands.entity(list).with_children(|list| {
            list.spawn(text("Turn order", 13.0, theme::MUTED));
            for (seat, player) in &order {
                let name = u8::try_from(*seat).map_or_else(|_| String::new(), |seat| session.seat_name(seat));
                let delay = i64::from(player.clock.delay);
                let shown = match mine {
                    Some(_) if Some(*seat) == session.my_seat().map(usize::from) => format!("{name}  {delay}"),
                    Some(mine) => format!("{name}  {:+}", delay - mine),
                    None => format!("{name}  {delay}"),
                };
                let marker = if current.map(usize::from) == Some(*seat) { "> " } else { "  " };
                list.spawn(text(format!("{marker}{shown}"), 15.0, team_color(player.team)));
            }
        });
    }
    for chat in &lists.chats {
        commands.entity(chat).despawn_children();
        commands.entity(chat).with_children(|lines| {
            for line in session.chat.iter().rev().take(6).rev() {
                let prefix = if line.team_only { "[team] " } else { "" };
                lines.spawn(text(format!("{prefix}{}: {}", line.name, line.text), 15.0, theme::TEXT));
            }
        });
    }
}

fn banner(game: Option<Res<Match>>, outcome: Res<Outcome>, session: Res<Session>, banners: Query<&Children, With<Banner>>, mut texts: Query<&mut Text>) {
    let Some(game) = game else { return };
    let message = if game.desynced {
        "Out of sync with the server. Reload the page to rejoin.".to_owned()
    } else if let Some(ending) = outcome.0 {
        let result = match (ending, session.my_team(&game.predicted)) {
            (Ending::Winner(team), Some(mine)) if team == mine => "You win!".to_owned(),
            (Ending::Winner(_), Some(_)) => "You lose.".to_owned(),
            (Ending::Winner(team), None) => format!("Team {team:?} wins."),
            (Ending::Draw, _) => "A draw.".to_owned(),
        };
        if session.practice { format!("{result}  (Esc to leave)") } else { result }
    } else {
        String::new()
    };
    for children in &banners {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && **text != message
            {
                (**text).clone_from(&message);
            }
        }
    }
}

/// The aim arc around your mobile on your turn: the shot's whole range dim, its true range bright,
/// the needle at your aim, a tick at last turn's.
fn aim_arc(game: Option<Res<Match>>, session: Res<Session>, controls: Res<Controls>, clock: Res<TurnClock>, views: Query<(&MobileView, &Shown)>, mut gizmos: Gizmos) {
    let (Some(game), Some(seat)) = (game, session.my_seat()) else { return };
    if clock.seat != Some(seat) {
        return;
    }
    let Some(player) = game.predicted.players().get(usize::from(seat)) else { return };
    let Some((_, shown)) = views.iter().find(|(view, _)| view.seat == seat) else { return };
    let pose = lobshot_sim::body::Pose {
        foot: shown.foot,
        facing: shown.facing,
        tilt: shown.tilt,
    };
    let (px, py) = player.tuning().cannon_pivot.value;
    let center = pose.to_world(lobshot_sim::units::Vec2::new(px, py));
    let range = controls.shot.tuning(player.tuning()).aim.value;
    let point = |aim: i16, radius: f64| world(center + pose.aim_direction(aim) * radius);
    let arc = |from: i16, to: i16, radius: f64| (from..=to).map(move |aim| point(aim, radius)).collect::<Vec<_>>();
    gizmos.linestrip_2d(arc(range.min, range.max, 62.0), Color::srgba(1.0, 1.0, 1.0, 0.35));
    gizmos.linestrip_2d(arc(range.true_min, range.true_max, 62.0), theme::GOOD);
    gizmos.linestrip_2d(arc(range.true_min, range.true_max, 64.0), theme::GOOD);
    gizmos.line_2d(world(center), point(controls.aim, 78.0), theme::TEAM_B);
    if let Some(last) = controls.last_aim {
        gizmos.line_2d(point(last, 58.0), point(last, 68.0), theme::ACCENT);
    }
}

/// Weather columns standing on the map.
fn columns(game: Option<Res<Match>>, mut gizmos: Gizmos) {
    let Some(game) = game else { return };
    let height = f64::from(game.predicted.terrain().height());
    for column in &game.predicted.weather().columns {
        let width = lobshot_sim::tuning::column_width(column.kind);
        let (_, color) = weather_look(column.kind);
        let x = f64::from(column.x);
        for edge in [x - width / 2.0, x + width / 2.0] {
            gizmos.line_2d(
                world(lobshot_sim::units::Vec2::new(edge, 0.0)),
                world(lobshot_sim::units::Vec2::new(edge, height)),
                color.with_alpha(0.7),
            );
        }
    }
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Game), build)
            .add_systems(Update, (update, weather_and_turns, banner, aim_arc, columns).run_if(in_state(Screen::Game)));
    }
}
