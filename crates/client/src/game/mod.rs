//! The game screen: the world (map, mobiles, shots), the HUD, input, sounds, and practice against a
//! bot.
//!
//! The world is drawn in sim coordinates turned upright (`world()`), through a camera that always
//! shows GunBound's 800×600 view, letterboxed to 4:3 so a "screen" stays a fixed ruler. A
//! backdrop camera clears the whole window first, and the UI camera draws the HUD over everything.

mod fx;
mod hud;
mod input;
mod playback;
mod practice;
mod scoreboard;
mod sounds;
mod world;

use bevy::{
    camera::{ScalingMode, Viewport, visibility::RenderLayers},
    prelude::*,
    ui::IsDefaultUiCamera,
    window::PrimaryWindow,
};
use lobshot_sim::units::Vec2 as SimVec2;

use crate::{model::Screen, ui::theme};

pub use playback::{Timeline, TurnClock};

/// The view: one GunBound screen, 800×600.
pub const VIEW: Vec2 = Vec2::new(800.0, 600.0);

/// Render layers nothing world-side uses, so the backdrop and UI cameras draw only what they own.
const BACKDROP_LAYER: usize = 31;
const UI_LAYER: usize = 30;

/// The camera that looks at the world.
#[derive(Component)]
pub struct GameCamera;

/// The root of everything spawned for a game, torn down on leaving it.
#[derive(Component)]
pub struct GameRoot;

/// Where the camera wants to look, in sim coordinates.
#[derive(Resource, Default)]
pub struct Focus(pub Option<SimVec2>);

/// A sim point (y down) as a Bevy point (y up). Sim maps are a few thousand pixels, far inside
/// f32's exact range.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn world(point: SimVec2) -> Vec2 {
    Vec2::new(point.x as f32, -point.y as f32)
}

fn cameras(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(theme::BACKDROP),
            ..default()
        },
        RenderLayers::layer(BACKDROP_LAYER),
    ));
    commands.spawn((
        Camera2d,
        GameCamera,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(0x8e, 0xc5, 0xe8)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed { width: VIEW.x, height: VIEW.y },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(VIEW.x / 2.0, -VIEW.y / 2.0, 0.0),
    ));
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(UI_LAYER),
    ));
}

/// Keeps the game camera's viewport the largest 4:3 rectangle in the window, and turns it off
/// outside the game (so the title and lobby sit on the plain backdrop).
fn letterbox(windows: Query<&Window, With<PrimaryWindow>>, mut camera: Query<&mut Camera, With<GameCamera>>, screen: Res<State<Screen>>) {
    let (Ok(window), Ok(mut camera)) = (windows.single(), camera.single_mut()) else { return };
    camera.is_active = *screen.get() == Screen::Game;
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }
    // 4:3, as wide as fits: width = min(w, h * 4/3).
    let width = size.x.min(size.y * 4 / 3);
    let height = width * 3 / 4;
    let viewport = Viewport {
        physical_position: UVec2::new((size.x - width) / 2, (size.y - height) / 2),
        physical_size: UVec2::new(width, height),
        ..default()
    };
    if camera.viewport.as_ref().map(|v| (v.physical_position, v.physical_size)) != Some((viewport.physical_position, viewport.physical_size)) {
        camera.viewport = Some(viewport);
    }
}

/// Glides the camera toward the focus (the active mobile, or a shell in flight), keeping the view
/// inside the map.
fn follow(focus: Res<Focus>, game: Option<Res<crate::model::Match>>, time: Res<Time>, mut camera: Query<&mut Transform, With<GameCamera>>) {
    let (Some(game), Some(target), Ok(mut camera)) = (game, focus.0, camera.single_mut()) else {
        return;
    };
    let terrain = game.predicted.terrain();
    let map = world(SimVec2::new(f64::from(terrain.width()), f64::from(terrain.height())));
    let clamp = |value: f32, low: f32, high: f32| if low > high { f32::midpoint(low, high) } else { value.clamp(low, high) };
    let want = world(target);
    let want = Vec2::new(clamp(want.x, VIEW.x / 2.0, map.x - VIEW.x / 2.0), clamp(want.y, map.y + VIEW.y / 2.0, -VIEW.y / 2.0));
    let blend = 1.0 - (-6.0 * time.delta_secs()).exp();
    let now = camera.translation.truncate().lerp(want, blend);
    camera.translation = now.extend(camera.translation.z);
}

/// How long the result and the scoreboard stay up, once the last shot has shown, before an online
/// game follows its room back to the lobby: long enough to read eight players' tallies.
const RESULT_SECONDS: f64 = 10.0;

/// Leaving a game. Online, the room went back to its lobby when the game ended; follow it once
/// the last shot, the result and the scoreboard have shown. In practice, Esc goes back to the
/// title.
fn wrap_up(
    mut session: ResMut<crate::model::Session>,
    timeline: Res<Timeline>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut since: Local<Option<f64>>,
    mut next: ResMut<NextState<Screen>>,
) {
    if session.practice && keyboard.just_pressed(KeyCode::Escape) {
        next.set(Screen::Title);
        return;
    }
    if !session.lobby_waiting || timeline.busy() {
        *since = None;
        return;
    }
    let now = time.elapsed_secs_f64();
    if now - *since.get_or_insert(now) >= RESULT_SECONDS {
        *since = None;
        session.lobby_waiting = false;
        next.set(Screen::Lobby);
    }
}

fn leave(mut commands: Commands, roots: Query<Entity, With<GameRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<world::MapArt>();
    commands.remove_resource::<crate::model::Match>();
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>()
            .add_systems(Startup, cameras)
            .add_systems(Update, (letterbox, (follow, wrap_up).run_if(in_state(Screen::Game))))
            .add_systems(OnExit(Screen::Game), leave)
            .add_plugins((
                world::WorldPlugin,
                playback::PlaybackPlugin,
                fx::FxPlugin,
                scoreboard::ScoreboardPlugin,
                hud::HudPlugin,
                input::InputPlugin,
                practice::PracticePlugin,
                sounds::SoundsPlugin,
            ));
    }
}
