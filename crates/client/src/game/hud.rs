//! The HUD, in GunBound's functional layout: weather queue, wind dial and turn timer (with the
//! sudden-death countdown and, in Score, each team's lives) across the top; turn list, shots,
//! items, angle, power bar and move bar along the bottom; chat above the turn list; the aim arc,
//! with its true-angle wedge, around your mobile. In Tag, your mobile sitting out and its HP stand
//! beside the controls, with the button (and the key, F7 or T) that switches it in. Once you're dead, the
//! slot machine stands there, or in Score, where you'll drop back in and when; drops and drop
//! points coming down are marked on the map.

use bevy::{ecs::system::SystemParam, picking::Pickable, prelude::*, ui::InteractionDisabled};
use lobshot_sim::{
    drops::{DropKind, Falling},
    game::{Effect, Shot, SsState, Team},
    items::Item,
    mode::{DropPoint, Mode, Respawn},
    tag::Bench,
    tuning::AimRange,
    units::Vec2 as SimVec2,
    weather::{Weather, WeatherKind},
};

use super::{
    GameRoot,
    input::{Controls, MapPointer, item_key_label, my_turn},
    playback::{Ending, Herald, Outcome, TurnClock},
    world,
    world::{MobileView, Shown, current_seat, depth},
};
use crate::{
    assets::ImageCache,
    model::{Match, Played, Screen, Session, Sudden},
    picks,
    ui::{Clicked, UiAction, art, button, button_node, chosen, panel, team_color, text, theme},
};

/// The power bar's width, in UI pixels: 100 per bar, as in GunBound.
const POWER_WIDTH: f32 = 400.0;
/// An item slot on the item bar, in UI pixels; a two-slot item is two of these and a gap.
const ITEM: f32 = 38.0;
const ITEM_GAP: f32 = 4.0;
/// How long the sudden-death banner stays up, in seconds.
const SUDDEN_BANNER_SECONDS: f64 = 3.5;

/// A text the HUD keeps current.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Readout {
    Wind,
    Timer,
    Turn,
    Angle,
    /// The sudden-death countdown, or which one is on.
    Sudden,
    /// A weather that holds over the whole map (Lock, Balm).
    Weather,
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
#[derive(Component)]
struct ItemBar;
#[derive(Component)]
struct SlotMachine;
#[derive(Component)]
struct SpinButton;
/// One face of the slot machine's reel: a drop, or `None` for the idle "?".
#[derive(Component)]
struct ReelFace(Option<DropKind>);
#[derive(Component)]
struct ReelStatus;
/// A label over a drop on its way down.
#[derive(Component)]
struct DropMarker;
/// The row of team lives, shown in a mode that counts them.
#[derive(Component)]
struct PointsRow;
/// A team's lives.
#[derive(Component)]
struct PointsText(Team);
/// Where a dead player in Score drops back in, and when.
#[derive(Component)]
struct RespawnPanel;
#[derive(Component)]
struct RespawnStatus;

/// Tag: the mobile sitting out, and the switch.
#[derive(Component)]
struct TagPanel;

#[derive(Component)]
struct BenchStatus;

#[derive(Component)]
struct SwitchButton;

fn build(mut commands: Commands, server: Res<AssetServer>, mut cache: ResMut<ImageCache>) {
    // The HUD's see-through layout nodes let the pointer through to the map, where a dead player
    // in Score picks a drop point (`input::MapPointer`); its panels and buttons don't.
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
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).with_children(|hud| {
        hud.spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(top_bar);
        hud.spawn((
            Banner,
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
            Pickable::IGNORE,
            children![(text("", 30.0, theme::ACCENT), Pickable::IGNORE)],
        ));
        hud.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|bottom| bottom_bar(bottom, &server, &mut cache));
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
        clock.spawn((Readout::Sudden, text("", 13.0, theme::MUTED)));
        clock.spawn((Readout::Weather, text("", 13.0, theme::MUTED)));
        clock.spawn((
            PointsRow,
            Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            children![
                text("Lives", 13.0, theme::MUTED),
                (PointsText(Team::A), text("", 15.0, team_color(Team::A))),
                (PointsText(Team::B), text("", 15.0, team_color(Team::B))),
            ],
        ));
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

/// Chat over the turn list, the controls and (for the dead) the slot machine.
fn bottom_bar(bottom: &mut ChildSpawnerCommands, server: &AssetServer, cache: &mut ImageCache) {
    bottom.spawn((
        ChatLines,
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
        Pickable::IGNORE,
    ));
    bottom
        .spawn((
            Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            Pickable::IGNORE,
        ))
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
            row.spawn((
                SlotMachine,
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    width: Val::Px(150.0),
                    display: Display::None,
                    ..default()
                }),
            ))
            .with_children(|machine| slot_machine_parts(machine, server, cache));
            row.spawn((
                RespawnPanel,
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    width: Val::Px(150.0),
                    display: Display::None,
                    ..default()
                }),
                children![
                    text("Drop back in", 15.0, theme::ACCENT),
                    (
                        RespawnStatus,
                        text("", 12.0, theme::TEXT),
                        TextLayout::justify(Justify::Center),
                        Node { max_width: Val::Px(130.0), ..default() },
                    ),
                ],
            ));
            row.spawn((
                TagPanel,
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    width: Val::Px(150.0),
                    display: Display::None,
                    ..default()
                }),
                children![
                    text("Sitting out", 15.0, theme::ACCENT),
                    (
                        BenchStatus,
                        text("", 12.0, theme::TEXT),
                        TextLayout::justify(Justify::Center),
                        Node { max_width: Val::Px(130.0), ..default() },
                    ),
                    (SwitchButton, button("Switch (F7)", UiAction::Switch)),
                ],
            ));
        });
}

/// The slot machine: a reel with a face per drop (one shows at a time), the Spin button and a line
/// saying what's going on.
fn slot_machine_parts(machine: &mut ChildSpawnerCommands, server: &AssetServer, cache: &mut ImageCache) {
    machine.spawn(text("Slot machine", 15.0, theme::ACCENT));
    machine
        .spawn((
            Node {
                width: Val::Px(64.0),
                height: Val::Px(64.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
        ))
        .with_children(|reel| {
            let face = |shown: bool| Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: if shown { Display::Flex } else { Display::None },
                ..default()
            };
            reel.spawn((ReelFace(None), face(true), children![text("?", 36.0, theme::ACCENT)]));
            for kind in DropKind::ALL {
                let image = cache.get(server, lobshot_assets::drop_path(kind));
                reel.spawn((ReelFace(Some(kind)), face(false), children![art(image, kind.name(), Vec2::splat(56.0), 12.0)]));
            }
        });
    machine.spawn((SpinButton, button("Spin", UiAction::Spin)));
    machine.spawn((
        ReelStatus,
        text("", 12.0, theme::TEXT),
        TextLayout::justify(Justify::Center),
        Node { max_width: Val::Px(130.0), ..default() },
    ));
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
    controls.spawn((
        ItemBar,
        Node {
            column_gap: Val::Px(ITEM_GAP),
            align_items: AlignItems::Center,
            min_height: Val::Px(ITEM),
            ..default()
        },
    ));
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

/// A turn-list row's delay: relative to `base`, except your own, which shows as it stands.
fn delay_label(delay: i64, base: Option<i64>, yours: bool) -> String {
    match base {
        Some(base) if !yours => format!("{:+}", delay - base),
        _ => delay.to_string(),
    }
}

/// The SS button's label: its lock in own turns left, or that sudden death took it.
fn ss_label(ss: SsState) -> String {
    match ss {
        SsState::Ready => shot_label(Shot::Ss).to_owned(),
        SsState::Locked(turns) => format!("SS in {turns}"),
        SsState::Off(_) | SsState::Armed(_) => "SS off".to_owned(),
    }
}

/// The aim as a player reads it: degrees above level, over the back too (`AimRange::elevation`).
fn angle_label(aim: i16, last: Option<i16>, range: Option<AimRange>) -> String {
    let shown = |aim: i16| range.map_or(aim, |range| range.elevation(aim));
    // The default font is ASCII only, so no degree sign until lobshot ships a font.
    match last {
        Some(last) => format!("Angle {} (last {})", shown(aim), shown(last)),
        None => format!("Angle {}", shown(aim)),
    }
}

/// The sudden-death readout under the turn.
fn sudden_label(sudden: Sudden) -> String {
    match sudden {
        Sudden::Never => String::new(),
        Sudden::In { turns: 1, .. } => "Sudden death next turn".to_owned(),
        Sudden::In { turns, .. } => format!("Sudden death in {turns} turns"),
        Sudden::On(kind) => format!("Sudden death: {}", kind.name()),
    }
}

fn weather_look(kind: WeatherKind) -> (&'static str, Color) {
    match kind {
        WeatherKind::Wind => ("W", Color::srgb_u8(0x8f, 0xd3, 0xff)),
        WeatherKind::Force => ("F", Color::srgb_u8(0xe8, 0x6a, 0x3a)),
        WeatherKind::Mirror => ("M", Color::srgb_u8(0xb7, 0x9c, 0xf2)),
        WeatherKind::Tornado => ("T", Color::srgb_u8(0x6f, 0xc4, 0x8a)),
        WeatherKind::Electricity => ("E", Color::srgb_u8(0xf2, 0xd6, 0x4b)),
        WeatherKind::Drain => ("D", Color::srgb_u8(0x7a, 0x5c, 0x9e)),
        WeatherKind::Lock => ("L", Color::srgb_u8(0x8a, 0x8f, 0x98)),
        WeatherKind::Balm => ("B", Color::srgb_u8(0xb8, 0xe6, 0xc9)),
        WeatherKind::Random => ("?", Color::srgb_u8(0xe0, 0xe0, 0xe0)),
        WeatherKind::Skyeye => ("S", Color::srgb_u8(0x9c, 0xf2, 0xa8)),
    }
}

/// What a weather that holds over the whole map does, while it is up; columns show on the map.
fn weather_label(weather: &Weather) -> String {
    match weather.now {
        Some(WeatherKind::Lock) => "Lock: no items".to_owned(),
        Some(WeatherKind::Balm) => "Balm: heals each turn".to_owned(),
        Some(WeatherKind::Skyeye) => format!("Skyeye Lv {}: beams on every landing", weather.skyeye.level()),
        _ => String::new(),
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
            Readout::Angle => {
                let range = session
                    .my_seat()
                    .and_then(|seat| game.predicted.players().get(usize::from(seat)))
                    .map(|me| controls.shot.tuning(me.tuning()).aim.value);
                angle_label(controls.aim, controls.last_aim, range)
            }
            Readout::Sudden if clock.seat.is_some() => sudden_label(game.sudden_death()),
            Readout::Sudden => String::new(),
            Readout::Weather => weather_label(game.predicted.weather()),
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
    tabs: Query<'w, 's, (Entity, &'static ShotTab, &'static mut BorderColor, &'static mut Node, &'static Children)>,
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
    // Spectators see SS as ready: it isn't theirs to lock.
    let ss = session.my_seat().and_then(|seat| game.predicted.ss_state(seat).ok()).unwrap_or(SsState::Ready);
    for (entity, tab, mut border, mut node, children) in &mut lists.tabs {
        let selected = tab.0 == controls.shot;
        node.border = UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 }));
        *border = BorderColor::all(if selected { theme::ACCENT } else { Color::NONE });
        if tab.0 != Shot::Ss {
            continue;
        }
        let label = ss_label(ss);
        for child in children.iter() {
            if let Ok(mut text) = lists.texts.get_mut(child)
                && **text != label
            {
                (**text).clone_from(&label);
            }
        }
        if ss == SsState::Ready {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }

    // The turn list: everyone alive, by delay (GunBound's ± list), small enough for eight. Delays
    // count from yours while you play; once you're dead, or watching, from whoever moves next.
    let mut order: Vec<(usize, &lobshot_sim::game::Player)> = game.predicted.players().iter().enumerate().filter(|(_, p)| p.alive).collect();
    order.sort_by_key(|(seat, p)| (p.clock.delay, p.clock.last_turn, *seat));
    let mine = session.my_seat().and_then(|seat| game.player(seat)).filter(|p| p.alive).map(|p| i64::from(p.clock.delay));
    let base = mine.or_else(|| order.first().map(|(_, p)| i64::from(p.clock.delay)));
    let current = current_seat(&game);
    for list in &lists.turns {
        commands.entity(list).despawn_children();
        commands.entity(list).with_children(|list| {
            list.spawn(text("Turn order", 12.0, theme::MUTED));
            for (seat, player) in &order {
                let name = u8::try_from(*seat).map_or_else(|_| String::new(), |seat| session.seat_name(seat));
                let delay = i64::from(player.clock.delay);
                let yours = mine.is_some() && Some(*seat) == session.my_seat().map(usize::from);
                let shown = format!("{name}  {}", delay_label(delay, base, yours));
                let marker = if current.map(usize::from) == Some(*seat) { "> " } else { "  " };
                list.spawn(text(format!("{marker}{shown}"), 13.0, team_color(player.team)));
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

fn banner(game: Option<Res<Match>>, outcome: Res<Outcome>, session: Res<Session>, time: Res<Time>, herald: Res<Herald>, banners: Query<&Children, With<Banner>>, mut texts: Query<&mut Text>) {
    let Some(game) = game else { return };
    let now = time.elapsed_secs_f64();
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
    } else if let Some((kind, at)) = herald.0
        && now - at < SUDDEN_BANNER_SECONDS
    {
        format!("Sudden death: {}! Items are off.", kind.name())
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
    // Centered on the barrel the shot leaves: Frostfin's S2 and SS leave its tusk.
    let (px, py) = player.tuning().barrel(controls.shot).pivot;
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

/// The loadout position this player used this turn, as the sim reports it: with two of the same
/// item, the armed one alone says which to highlight.
#[derive(Resource, Default)]
struct UsedSlot(Option<u8>);

fn track_used(mut played: MessageReader<Played>, session: Res<Session>, mut used: ResMut<UsedSlot>) {
    for effect in played.read().flat_map(|scene| &scene.effects) {
        if let Effect::ItemUsed { seat, slot, .. } = *effect
            && Some(seat) == session.my_seat()
        {
            used.0 = Some(slot);
        }
    }
}

/// One item on the item bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ItemSlot {
    position: u8,
    item: Item,
    used: bool,
    /// Used this turn: it rides on the shot.
    armed: bool,
    /// Pressing it now would use it.
    usable: bool,
}

/// The item bar for `seat`: its loadout by position, used ones marked, from the sim's state.
fn item_slots(game: &Match, seat: u8, my_turn: bool, used_slot: Option<u8>) -> Vec<ItemSlot> {
    let (Some(player), Some(start)) = (game.player(seat), game.log.setup.seats.get(usize::from(seat))) else {
        return Vec::new();
    };
    (0_u8..)
        .zip(start.items.positions())
        .filter_map(|(position, item)| {
            Some(ItemSlot {
                position,
                item: item?,
                used: player.items.get(usize::from(position)).is_some_and(Option::is_none),
                armed: player.armed.is_some() && used_slot == Some(position),
                usable: my_turn && game.predicted.can_use_item(seat, position),
            })
        })
        .collect()
}

/// What the item bar needs.
#[derive(SystemParam)]
struct ItemBarParts<'w, 's> {
    used: Res<'w, UsedSlot>,
    server: Res<'w, AssetServer>,
    cache: ResMut<'w, ImageCache>,
    bars: Query<'w, 's, Entity, With<ItemBar>>,
}

/// Rebuilds the item bar when what it shows changes (and only then: a rebuild between a press and
/// its release would lose the click). Used items are greyed, the armed one outlined, each labeled
/// with its key.
fn item_bar(game: Option<Res<Match>>, session: Res<Session>, clock: Res<TurnClock>, mut parts: ItemBarParts, mut shown: Local<Option<Vec<ItemSlot>>>, mut commands: Commands) {
    let Some(game) = game else { return };
    let Some(seat) = session.my_seat() else { return };
    let mine = my_turn(&session, &clock, &game).is_some();
    let slots = item_slots(&game, seat, mine, parts.used.0);
    if shown.as_ref() == Some(&slots) {
        return;
    }
    for bar in &parts.bars {
        commands.entity(bar).despawn_children();
        commands.entity(bar).with_children(|bar| {
            if slots.is_empty() {
                bar.spawn(text("No items", 13.0, theme::MUTED));
            }
            for slot in &slots {
                let size = f32::from(slot.item.slots());
                let node = Node {
                    width: Val::Px(ITEM * size + ITEM_GAP * (size - 1.0)),
                    height: Val::Px(ITEM),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                };
                let mut button = bar.spawn((button_node(UiAction::Item(slot.position), node), chosen(slot.armed)));
                if !slot.usable {
                    button.insert(InteractionDisabled);
                }
                button.with_children(|inside| {
                    inside.spawn(picks::item_art(&parts.server, &mut parts.cache, slot.item, ITEM - 8.0));
                    if slot.used {
                        inside.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.7)),
                        ));
                    }
                    if let Some(key) = item_key_label(slot.position) {
                        // On a pill over the icon's corner, so it reads on any icon.
                        inside.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(-3.0),
                                top: Val::Px(-5.0),
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            ZIndex(1),
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
                            children![text(key, 11.0, theme::ACCENT)],
                        ));
                    }
                });
            }
        });
    }
    *shown = Some(slots);
}

/// The slot machine's reel: when this player last spun, and what it landed on.
#[derive(Resource, Default)]
struct Reel {
    spun: Option<f64>,
    landed: Option<DropKind>,
}

/// The reel whirls at least this long, in seconds,
const REEL_SPIN: f64 = 1.2;
/// waits at most this long for the drop,
const REEL_WAIT: f64 = 4.0;
/// and shows each face this long while it whirls.
const REEL_STEP: f64 = 0.08;

/// The face the reel shows `since` seconds after a spin, and whether it's still whirling: it lands
/// on the drop once it has whirled a while, or gives up waiting for one.
fn reel_face(since: Option<f64>, landed: Option<DropKind>) -> (Option<DropKind>, bool) {
    match since {
        Some(since) if since < REEL_SPIN || (landed.is_none() && since < REEL_WAIT) => {
            // A few seconds of 80 ms steps.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let step = (since.max(0.0) / REEL_STEP) as usize;
            (Some(DropKind::ALL[step % DropKind::ALL.len()]), true)
        }
        _ => (landed, false),
    }
}

fn spin_reel(mut clicked: MessageReader<Clicked>, mut played: MessageReader<Played>, session: Res<Session>, time: Res<Time>, mut reel: ResMut<Reel>) {
    for Clicked(action) in clicked.read() {
        if *action == UiAction::Spin {
            reel.spun = Some(time.elapsed_secs_f64());
            reel.landed = None;
        }
    }
    for effect in played.read().flat_map(|scene| &scene.effects) {
        if let Effect::DropRolled { seat, drop } = *effect
            && Some(seat) == session.my_seat()
        {
            reel.landed = Some(drop.kind);
        }
    }
}

/// The slot machine's live parts.
#[derive(SystemParam)]
struct Machine<'w, 's> {
    reel: Res<'w, Reel>,
    time: Res<'w, Time>,
    views: Query<'w, 's, (&'static MobileView, &'static Shown)>,
    panels: Query<'w, 's, &'static mut Node, With<SlotMachine>>,
    faces: Query<'w, 's, (&'static ReelFace, &'static mut Node), Without<SlotMachine>>,
    spins: Query<'w, 's, (Entity, Has<InteractionDisabled>), With<SpinButton>>,
    statuses: Query<'w, 's, &'static mut Text, With<ReelStatus>>,
}

/// Shows the slot machine while this player is dead (as shown) in a mode that has one, and the
/// game is on. Spin works whenever the sim allows it, on anyone's turn.
fn slot_machine(game: Option<Res<Match>>, session: Res<Session>, mut machine: Machine, mut commands: Commands) {
    let Some(game) = game else { return };
    let seat = session.my_seat();
    let dead = seat.is_some_and(|seat| machine.views.iter().any(|(view, shown)| view.seat == seat && !shown.alive));
    let up = dead && game.predicted.mode().slot_machine() && current_seat(&game).is_some();
    let display = if up { Display::Flex } else { Display::None };
    for mut node in &mut machine.panels {
        if node.display != display {
            node.display = display;
        }
    }
    if !up {
        return;
    }
    let since = machine.reel.spun.map(|spun| machine.time.elapsed_secs_f64() - spun);
    let (face, whirling) = reel_face(since, machine.reel.landed);
    for (reel_face, mut node) in &mut machine.faces {
        let display = if reel_face.0 == face { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    let can_spin = !whirling && seat.is_some_and(|seat| game.confirmed.can_spin(seat));
    for (button, disabled) in &machine.spins {
        if can_spin && disabled {
            commands.entity(button).remove::<InteractionDisabled>();
        } else if !can_spin && !disabled {
            commands.entity(button).insert(InteractionDisabled);
        }
    }
    let status = match (whirling, machine.reel.landed, can_spin) {
        (true, _, _) => "Spinning...".to_owned(),
        (false, Some(kind), _) => format!("{}! Look out below.", kind.name()),
        (false, None, true) => "Spin to drop trouble on the living.".to_owned(),
        (false, None, false) => "The machine is resting.".to_owned(),
    };
    for mut text in &mut machine.statuses {
        if **text != status {
            (**text).clone_from(&status);
        }
    }
}

/// Something on its way down a column: a slot machine's drop, or a mobile dropping back in. What
/// it is, its column, and the turns until it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Incoming {
    what: String,
    x: i32,
    turns: u32,
}

/// What's coming down after `turns` turns, from the sim: the slot machines' drops, then the dead
/// who picked where to drop back in, each named by `name`.
fn incoming(drops: &[Falling], respawns: &[Respawn], turns: i64, name: impl Fn(u8) -> String) -> Vec<Incoming> {
    let left = |lands: i64| u32::try_from(lands - turns).unwrap_or(0);
    let drops = drops.iter().map(|falling| Incoming {
        what: falling.drop.kind.name().to_owned(),
        x: falling.drop.x,
        turns: left(falling.lands),
    });
    let respawns = respawns.iter().filter_map(|respawn| {
        let DropPoint { x, lands } = respawn.pick?;
        Some(Incoming {
            what: format!("{} drops", name(respawn.seat)),
            x,
            turns: left(lands),
        })
    });
    drops.chain(respawns).collect()
}

/// Marks each incoming drop's column, with what it is and the turns until it lands just over the
/// ground there.
fn drop_markers(game: Option<Res<Match>>, session: Res<Session>, markers: Query<Entity, With<DropMarker>>, mut shown: Local<Vec<Incoming>>, mut gizmos: Gizmos, mut commands: Commands) {
    let Some(game) = game else { return };
    let falling = incoming(game.predicted.drops(), game.predicted.respawns(), game.predicted.turns(), |seat| session.seat_name(seat));
    let terrain = game.predicted.terrain();
    let ground = |x: i32| (0..i32::try_from(terrain.height()).unwrap_or(0)).find(|&y| terrain.is_solid(x, y)).unwrap_or(0);
    for drop in &falling {
        let x = f64::from(drop.x);
        gizmos.line_2d(world(SimVec2::new(x, 0.0)), world(SimVec2::new(x, f64::from(ground(drop.x)))), theme::ACCENT.with_alpha(0.5));
    }
    if *shown == falling {
        return;
    }
    for marker in &markers {
        commands.entity(marker).despawn();
    }
    for drop in &falling {
        // Clear of the mobiles' name tags, which stand about 120 px over the ground.
        let at = world(SimVec2::new(f64::from(drop.x), f64::from(ground(drop.x) - 150)));
        commands.spawn((
            GameRoot,
            DropMarker,
            Text2d::new(format!("{} in {}", drop.what, drop.turns)),
            TextFont {
                font_size: bevy::text::FontSize::Px(14.0),
                ..default()
            },
            TextColor(theme::ACCENT),
            Transform::from_translation(at.extend(depth::OVERLAYS)),
        ));
    }
    *shown = falling;
}

/// Each team's lives, in a mode that counts them.
fn points(game: Option<Res<Match>>, mut rows: Query<&mut Node, With<PointsRow>>, mut texts: Query<(&PointsText, &mut Text)>) {
    let Some(game) = game else { return };
    let counted = game.predicted.points(Team::A).is_some();
    let display = if counted { Display::Flex } else { Display::None };
    for mut node in &mut rows {
        if node.display != display {
            node.display = display;
        }
    }
    for (team, mut text) in &mut texts {
        let label = game.predicted.points(team.0).map_or_else(String::new, |points| format!("{:?} {points}", team.0));
        if **text != label {
            **text = label;
        }
    }
}

/// What the respawn panel says: pick a point, or where and when you drop in.
fn respawn_status(can_pick: bool, pick: Option<DropPoint>, turns: i64) -> String {
    match (can_pick, pick) {
        (true, _) => "Click the map where you'll drop back in. The arrow keys look around.".to_owned(),
        (false, Some(DropPoint { x, lands })) if lands - turns <= 1 => format!("You drop in at x = {x} as this turn ends."),
        (false, Some(DropPoint { x, lands })) => format!("You drop in at x = {x} in {} turns.", lands - turns),
        (false, None) => "You won't drop back in.".to_owned(),
    }
}

/// Shows the respawn panel while this player is dead (as shown) in a mode where the dead come back,
/// and the game is on.
fn respawn_panel(
    game: Option<Res<Match>>,
    session: Res<Session>,
    views: Query<(&MobileView, &Shown)>,
    mut panels: Query<&mut Node, With<RespawnPanel>>,
    mut statuses: Query<&mut Text, With<RespawnStatus>>,
) {
    let (Some(game), Some(seat)) = (game, session.my_seat()) else { return };
    let dead = views.iter().any(|(view, shown)| view.seat == seat && !shown.alive);
    let up = dead && game.predicted.mode().respawn().is_some() && current_seat(&game).is_some();
    let display = if up { Display::Flex } else { Display::None };
    for mut node in &mut panels {
        if node.display != display {
            node.display = display;
        }
    }
    if !up {
        return;
    }
    let pick = game.confirmed.respawns().iter().find(|respawn| respawn.seat == seat).and_then(|respawn| respawn.pick);
    let status = respawn_status(game.confirmed.can_respawn(seat), pick, game.confirmed.turns());
    for mut text in &mut statuses {
        if **text != status {
            (**text).clone_from(&status);
        }
    }
}

/// What the Tag panel says of the mobile sitting out: its name and HP, and its shield if it has one.
fn bench_status(bench: &Bench) -> String {
    let tuning = bench.mobile.tuning();
    let shield = if tuning.shield.value > 0 { format!(", {} shield", bench.shield) } else { String::new() };
    format!("{}\n{}/{} HP{shield}", tuning.name, bench.hp, tuning.hp.value)
}

/// The Tag panel's live parts.
#[derive(SystemParam)]
struct TagParts<'w, 's> {
    clock: Res<'w, TurnClock>,
    views: Query<'w, 's, (&'static MobileView, &'static Shown)>,
    panels: Query<'w, 's, &'static mut Node, With<TagPanel>>,
    statuses: Query<'w, 's, &'static mut Text, With<BenchStatus>>,
    switches: Query<'w, 's, (Entity, Has<InteractionDisabled>), With<SwitchButton>>,
}

/// Shows this player's mobile sitting out while it plays Tag and is alive (as shown), and lets the
/// switch be pressed on its turn when the sim allows one.
fn tag_panel(game: Option<Res<Match>>, session: Res<Session>, mut parts: TagParts, mut commands: Commands) {
    let (Some(game), Some(seat)) = (game, session.my_seat()) else { return };
    let alive = parts.views.iter().any(|(view, shown)| view.seat == seat && shown.alive);
    let bench = game.confirmed.bench(seat).filter(|_| alive && game.predicted.mode() == Mode::Tag && current_seat(&game).is_some());
    let display = if bench.is_some() { Display::Flex } else { Display::None };
    for mut node in &mut parts.panels {
        if node.display != display {
            node.display = display;
        }
    }
    let Some(bench) = bench else { return };
    let status = bench_status(bench);
    for mut text in &mut parts.statuses {
        if **text != status {
            (**text).clone_from(&status);
        }
    }
    let can_switch = my_turn(&session, &parts.clock, &game).is_some() && game.predicted.can_switch(seat);
    for (button, disabled) in &parts.switches {
        if can_switch && disabled {
            commands.entity(button).remove::<InteractionDisabled>();
        } else if !can_switch && !disabled {
            commands.entity(button).insert(InteractionDisabled);
        }
    }
}

/// Where a click would drop this player back in: a line down the column under the pointer.
fn drop_point_preview(game: Option<Res<Match>>, session: Res<Session>, pointer: Res<MapPointer>, mut gizmos: Gizmos) {
    let (Some(game), Some(seat), Some(x)) = (game, session.my_seat(), pointer.0) else { return };
    if !game.confirmed.can_respawn(seat) {
        return;
    }
    let x = f64::from(x) + 0.5;
    let height = f64::from(game.predicted.terrain().height());
    gizmos.line_2d(world(SimVec2::new(x, 0.0)), world(SimVec2::new(x, height)), theme::GOOD.with_alpha(0.7));
}

fn reset(mut used: ResMut<UsedSlot>, mut reel: ResMut<Reel>) {
    *used = UsedSlot::default();
    *reel = Reel::default();
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UsedSlot>()
            .init_resource::<Reel>()
            .add_systems(OnEnter(Screen::Game), build)
            .add_systems(OnExit(Screen::Game), reset)
            .add_systems(
                Update,
                (
                    update,
                    weather_and_turns,
                    banner,
                    aim_arc,
                    columns,
                    (track_used, item_bar).chain(),
                    (spin_reel, slot_machine).chain(),
                    drop_markers,
                    points,
                    respawn_panel,
                    drop_point_preview,
                    tag_panel,
                )
                    .run_if(in_state(Screen::Game)),
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
    fn the_turn_list_counts_delay_from_you_or_the_next_mover() {
        assert_eq!(delay_label(780, Some(760), false), "+20");
        assert_eq!(delay_label(760, Some(760), true), "760", "your own stands as it is");
        assert_eq!(delay_label(700, Some(760), false), "-60");
        assert_eq!(delay_label(700, None, false), "700");
    }

    #[test]
    fn the_angle_reads_as_degrees_above_level_over_the_back_too() {
        use lobshot_sim::game::MobileKind;
        let behind = MobileKind::Flarecrow.tuning().s1.aim.value;
        assert_eq!(angle_label(150, Some(140), Some(behind)), "Angle 30 (last 40)");
        assert_eq!(angle_label(30, None, Some(MobileKind::Coilback.tuning().s1.aim.value)), "Angle 30");
        let rear = MobileKind::Augerdillo.tuning().s1.aim.value;
        assert_eq!(angle_label(150, Some(165), Some(rear)), "Angle 30 (last 15)");
        assert_eq!(angle_label(30, None, None), "Angle 30");
    }

    #[test]
    fn the_ss_button_shows_its_lock() {
        assert_eq!(ss_label(SsState::Ready), "SS");
        assert_eq!(ss_label(SsState::Locked(3)), "SS in 3");
        assert_eq!(ss_label(SsState::Off(lobshot_sim::game::SuddenDeathKind::Double)), "SS off");
        assert_eq!(ss_label(SsState::Armed(Item::Dual)), "SS off");
    }

    #[test]
    fn sudden_death_counts_down_in_turns() {
        use lobshot_sim::game::SuddenDeathKind;
        assert_eq!(sudden_label(Sudden::Never), "");
        assert_eq!(sudden_label(Sudden::In { turns: 12, kind: SuddenDeathKind::Double }), "Sudden death in 12 turns");
        assert_eq!(sudden_label(Sudden::In { turns: 1, kind: SuddenDeathKind::Double }), "Sudden death next turn");
        assert_eq!(sudden_label(Sudden::On(SuddenDeathKind::BigBomb)), "Sudden death: Big Bomb");
    }

    #[test]
    fn lock_balm_and_skyeye_say_what_they_do() {
        let up = |now: Option<WeatherKind>, xp: u32| {
            weather_label(&Weather {
                now,
                skyeye: lobshot_sim::weather::Skyeye { xp },
                ..Weather::default()
            })
        };
        assert_eq!(up(Some(WeatherKind::Lock), 0), "Lock: no items");
        assert_eq!(up(Some(WeatherKind::Balm), 0), "Balm: heals each turn");
        assert_eq!(up(Some(WeatherKind::Skyeye), 900), "Skyeye Lv 3: beams on every landing");
        assert_eq!(up(Some(WeatherKind::Force), 0), "", "a column shows on the map");
        assert_eq!(up(None, 0), "");
    }

    #[test]
    fn drops_and_drop_points_are_marked_with_the_turns_until_they_land() {
        use lobshot_sim::drops::Drop;
        let drops = [Falling {
            seat: 1,
            drop: Drop { kind: DropKind::Hammer, x: 300 },
            lands: 12,
        }];
        let respawns = [
            Respawn {
                seat: 2,
                pick: Some(DropPoint { x: 900, lands: 14 }),
            },
            Respawn { seat: 3, pick: None },
        ];
        let marked = incoming(&drops, &respawns, 10, |seat| format!("P{seat}"));
        let shown: Vec<(String, i32, u32)> = marked.into_iter().map(|i| (i.what, i.x, i.turns)).collect();
        assert_eq!(shown, [("Hammer".to_owned(), 300, 2), ("P2 drops".to_owned(), 900, 4)], "no mark until a point is picked");
    }

    #[test]
    fn the_respawn_panel_asks_for_a_point_then_counts_down() {
        assert!(respawn_status(true, None, 5).starts_with("Click the map"));
        assert_eq!(respawn_status(false, Some(DropPoint { x: 640, lands: 9 }), 5), "You drop in at x = 640 in 4 turns.");
        assert_eq!(respawn_status(false, Some(DropPoint { x: 640, lands: 9 }), 8), "You drop in at x = 640 as this turn ends.");
        assert_eq!(respawn_status(false, None, 5), "You won't drop back in.");
    }

    #[test]
    fn the_tag_panel_names_the_mobile_sitting_out_and_its_hp() {
        let bench = |mobile: lobshot_sim::game::MobileKind, hp, shield| Bench {
            mobile,
            hp,
            shield,
            ss_lock: 0,
            brittle: 0,
            switched: None,
        };
        assert_eq!(bench_status(&bench(lobshot_sim::game::MobileKind::Coilback, 640, 0)), "Coilback\n640/1000 HP");
        assert_eq!(bench_status(&bench(lobshot_sim::game::MobileKind::Lanternwing, 730, 85)), "Lanternwing\n730/730 HP, 85 shield");
    }

    #[test]
    fn the_reel_whirls_then_lands_or_gives_up() {
        assert_eq!(reel_face(None, None), (None, false), "idle");
        assert!(reel_face(Some(0.5), Some(DropKind::Hammer)).1, "whirls a while even with the drop known");
        assert_eq!(reel_face(Some(REEL_SPIN + 0.1), Some(DropKind::Hammer)), (Some(DropKind::Hammer), false));
        assert!(reel_face(Some(REEL_SPIN + 0.1), None).1, "waits for the drop");
        assert_eq!(reel_face(Some(REEL_WAIT + 0.1), None), (None, false), "gives up");
        let faces: Vec<Option<DropKind>> = (0..6).map(|step| reel_face(Some(f64::from(step) * REEL_STEP + 0.01), None).0).collect();
        assert_eq!(faces, DropKind::ALL.map(Some).to_vec(), "every face comes up in turn");
    }
}
