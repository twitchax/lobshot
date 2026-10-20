//! Showing what happened. Each accepted event's effects become a short scene: the shell flies its
//! legs in real time, blasts carve the terrain art as they land, damage numbers rise, mobiles fall,
//! and the turn passes when it's all over. Scenes play one at a time, in order, on the schedule in
//! `lobshot_protocol::pacing`, which the server waits out before starting the next turn's clock.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lobshot_assets::Sound;
use lobshot_protocol::pacing::{self, FALL_SECONDS};
use lobshot_sim::{
    game::{Death, Effect, Event, Leg, Team},
    movement::Footing,
    tuning::TURN_TIME_LIMIT,
    units::{TICKS_PER_SECOND, Vec2 as SimVec2},
};

use super::{
    Focus, GameRoot, world,
    world::{MapArt, MobileView, Shown, depth},
};
use crate::{
    model::{ApplyEvents, Match, Played, Screen, Session},
    sound::Play,
    ui::theme,
};

/// The scenes still to play.
#[derive(Resource, Default)]
pub struct Timeline {
    queue: VecDeque<Played>,
    scene: Option<Scene>,
}

impl Timeline {
    /// Whether anything is playing or waiting.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.scene.is_some() || !self.queue.is_empty()
    }
}

/// The turn's start, for the timer.
#[derive(Resource, Default)]
pub struct TurnClock {
    pub seat: Option<u8>,
    pub started: f64,
    /// Turns started so far: one per turn shown, so a new turn is a new count.
    pub starts: u32,
}

impl TurnClock {
    fn start(&mut self, seat: u8, now: f64) {
        self.seat = Some(seat);
        self.started = now;
        self.starts += 1;
    }

    /// Whole seconds left at `now`, rounded up: a fresh 20 s turn reads 20 until a second passes.
    #[must_use]
    pub fn seconds_left(&self, session: &Session, now: f64) -> u32 {
        let limit = session.room.as_ref().map_or(f64::from(TURN_TIME_LIMIT.value.0) / 100.0, |room| f64::from(room.options.turn_seconds));
        let left = (limit - (now - self.started)).max(0.0);
        // A turn's limit is tens of seconds, never negative here: exact in a u32.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let seconds = left.ceil() as u32;
        seconds
    }
}

/// How a game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Winner(Team),
    Draw,
}

/// The game's result, once its last shot has played.
#[derive(Resource, Default)]
pub struct Outcome(pub Option<Ending>);

/// A soft round sprite for shells and blasts.
#[derive(Resource)]
struct Dot(Handle<Image>);

enum Cue {
    Explode { at: SimVec2, radius: f64 },
    Damage { seat: u8, amount: i32 },
    Die { seat: u8 },
    Fall { seat: u8, to: Footing },
    Turn { seat: u8 },
    Over { winner: Option<Team> },
    Sound(Sound),
}

struct Scene {
    elapsed: f32,
    cues: Vec<(f32, Cue)>,
    next: usize,
    shell: Option<(Vec<Leg>, f32, Entity)>,
    falls: Vec<(u8, SimVec2, SimVec2, f32)>,
    end: f32,
}

#[derive(Component)]
struct Fx {
    age: f32,
    life: f32,
    rise: f32,
}

/// Where a shell is `seconds` after launch, following its legs (holding still between legs while
/// a tornado spins it).
fn shell_at(legs: &[Leg], seconds: f32) -> Option<SimVec2> {
    // A clock reading in seconds, back to whole ticks.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tick = (f64::from(seconds) * f64::from(TICKS_PER_SECOND)) as u32;
    let mut last = None;
    for leg in legs {
        let local = tick.checked_sub(leg.offset);
        match local {
            Some(local) if local <= leg.to => return Some(leg.flight.at(local.max(leg.from))),
            Some(_) => last = Some(leg.flight.at(leg.to)),
            None => return last.or(Some(leg.flight.at(leg.from))),
        }
    }
    last
}

fn make_dot(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    const SIZE: u32 = 32;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (f64::from(x) + 0.5 - 16.0, f64::from(y) + 0.5 - 16.0);
            let distance = (dx * dx + dy * dy).sqrt() / 16.0;
            // A soft edge over the outer quarter.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let alpha = ((1.0 - distance).clamp(0.0, 0.25) * 4.0 * 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    commands.insert_resource(Dot(images.add(image)));
}

fn enqueue(mut played: MessageReader<Played>, mut timeline: ResMut<Timeline>) {
    for scene in played.read() {
        timeline.queue.push_back(scene.clone());
    }
}

/// Turns one event's effects into a scene.
fn stage(played: Played, dot: &Dot, views: &mut Query<(&MobileView, &mut Shown)>, commands: &mut Commands) -> Scene {
    let mut cues = Vec::new();
    let mut shell = None;
    let schedule = pacing::schedule(&played.effects);

    if let Event::Shoot { seat, aim, .. } = played.event {
        for (view, mut shown) in views.iter_mut() {
            if view.seat == seat {
                shown.aim = aim;
            }
        }
    }
    for (effect, at) in played.effects.into_iter().zip(schedule.at) {
        match effect {
            Effect::Fired { legs, .. } => {
                let duration = pacing::flight_seconds(&legs);
                let entity = commands
                    .spawn((
                        GameRoot,
                        Sprite {
                            image: dot.0.clone(),
                            color: Color::srgb_u8(0x2a, 0x22, 0x1c),
                            custom_size: Some(Vec2::splat(10.0)),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, depth::SHOTS),
                    ))
                    .id();
                shell = Some((legs, duration, entity));
                cues.push((at, Cue::Sound(Sound::Fire)));
            }
            Effect::Exploded { at: center, radius, .. } => {
                cues.push((at, Cue::Explode { at: center, radius }));
                cues.push((at, Cue::Sound(Sound::Blast)));
            }
            Effect::Damaged { seat, amount } => {
                cues.push((at, Cue::Damage { seat, amount }));
                cues.push((at, Cue::Sound(Sound::Hit)));
            }
            Effect::Died { seat, death } => {
                cues.push((at, Cue::Die { seat }));
                // A player leaving isn't an explosion.
                if death != Death::Forfeited {
                    cues.push((at, Cue::Sound(Sound::Death)));
                }
            }
            Effect::Fell { seat, to } => {
                cues.push((at, Cue::Fall { seat, to }));
                cues.push((at + FALL_SECONDS, Cue::Sound(Sound::Land)));
            }
            Effect::TurnStarted { seat } => cues.push((at, Cue::Turn { seat })),
            Effect::GameOver { winner } => cues.push((at, Cue::Over { winner })),
            Effect::Walked { .. } | Effect::WindChanged(_) | Effect::Forecast(_) | Effect::WeatherChanged { .. } => {}
        }
    }
    // Stable, so cues at the same moment keep the sim's order (the turn passing last).
    cues.sort_by(|a, b| a.0.total_cmp(&b.0));
    Scene {
        elapsed: 0.0,
        cues,
        next: 0,
        shell,
        falls: Vec::new(),
        end: schedule.end,
    }
}

/// What a scene changes besides the mobiles.
#[derive(SystemParam)]
struct Stagehands<'w> {
    dot: Option<Res<'w, Dot>>,
    art: Option<Res<'w, MapArt>>,
    images: ResMut<'w, Assets<Image>>,
    focus: ResMut<'w, Focus>,
    clock: ResMut<'w, TurnClock>,
    outcome: ResMut<'w, Outcome>,
    sounds: MessageWriter<'w, Play>,
}

fn play(
    mut timeline: ResMut<Timeline>,
    time: Res<Time>,
    mut hands: Stagehands,
    mut views: Query<(&MobileView, &mut Shown)>,
    mut transforms: Query<&mut Transform, Without<MobileView>>,
    mut commands: Commands,
) {
    let Some(dot) = hands.dot.as_deref() else { return };
    let dot = Dot(dot.0.clone());
    if timeline.scene.is_none() {
        let Some(next) = timeline.queue.pop_front() else { return };
        timeline.scene = Some(stage(next, &dot, &mut views, &mut commands));
    }
    let Some(scene) = timeline.scene.as_mut() else { return };
    scene.elapsed += time.delta_secs();
    let now = scene.elapsed;

    if let Some((legs, duration, entity)) = &scene.shell {
        if now <= *duration {
            if let (Some(at), Ok(mut transform)) = (shell_at(legs, now), transforms.get_mut(*entity)) {
                transform.translation = world(at).extend(depth::SHOTS);
                hands.focus.0 = Some(at);
            }
        } else {
            commands.entity(*entity).despawn();
            scene.shell = None;
        }
    }

    while let Some((at, cue)) = scene.cues.get(scene.next) {
        if *at > now {
            break;
        }
        match cue {
            Cue::Explode { at, radius } => {
                if let Some(art) = hands.art.as_deref() {
                    art.carve(&mut hands.images, *at, *radius);
                }
                commands.spawn(puff(&dot, *at, *radius));
            }
            Cue::Damage { seat, amount } => {
                for (view, mut shown) in &mut views {
                    if view.seat == *seat {
                        shown.hp -= amount;
                        commands.spawn((
                            GameRoot,
                            Fx { age: 0.0, life: 1.1, rise: 45.0 },
                            Text2d::new(amount.to_string()),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(22.0),
                                ..default()
                            },
                            TextColor(theme::ACCENT),
                            Transform::from_translation((world(shown.foot) + Vec2::new(0.0, 90.0)).extend(depth::OVERLAYS + 1.0)),
                        ));
                    }
                }
            }
            Cue::Die { seat } => {
                for (view, mut shown) in &mut views {
                    if view.seat == *seat {
                        shown.alive = false;
                    }
                }
            }
            Cue::Fall { seat, to } => {
                for (view, shown) in &views {
                    if view.seat == *seat {
                        scene.falls.push((*seat, shown.foot, to.world(), now));
                    }
                }
            }
            Cue::Turn { seat } => {
                hands.clock.start(*seat, time.elapsed_secs_f64());
                for (view, shown) in &views {
                    if view.seat == *seat {
                        hands.focus.0 = Some(shown.foot);
                    }
                }
            }
            Cue::Over { winner } => hands.outcome.0 = Some(winner.map_or(Ending::Draw, Ending::Winner)),
            Cue::Sound(sound) => {
                hands.sounds.write(Play(*sound));
            }
        }
        scene.next += 1;
    }

    for &(seat, from, to, started) in &scene.falls {
        let t = f64::from(((now - started) / FALL_SECONDS).clamp(0.0, 1.0));
        for (view, mut shown) in &mut views {
            if view.seat == seat {
                // Accelerating, like a drop.
                shown.foot = SimVec2::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t * t);
            }
        }
    }

    if now >= scene.end && scene.next >= scene.cues.len() {
        timeline.scene = None;
    }
}

/// A blast's flash, fading as it grows.
fn puff(dot: &Dot, at: SimVec2, radius: f64) -> impl Bundle {
    // Blast radii are tens of pixels.
    #[allow(clippy::cast_possible_truncation)]
    let size = (radius * 2.4) as f32;
    (
        GameRoot,
        Fx { age: 0.0, life: 0.45, rise: 0.0 },
        Sprite {
            image: dot.0.clone(),
            color: Color::srgb_u8(0xff, 0xb3, 0x47),
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(world(at).extend(depth::SHOTS + 1.0)),
    )
}

/// A fading effect: a blast puff or a damage number.
type Fading = (Entity, &'static mut Fx, &'static mut Transform, Option<&'static mut Sprite>, Option<&'static mut TextColor>);

fn fade(mut effects: Query<Fading>, time: Res<Time>, mut commands: Commands) {
    for (entity, mut fx, mut transform, sprite, text) in &mut effects {
        fx.age += time.delta_secs();
        let left = 1.0 - (fx.age / fx.life).clamp(0.0, 1.0);
        transform.translation.y += fx.rise * time.delta_secs();
        if let Some(mut sprite) = sprite {
            sprite.color.set_alpha(left);
            transform.scale = Vec3::splat(1.0 + (1.0 - left) * 0.6);
        }
        if let Some(mut text) = text {
            text.0.set_alpha(left);
        }
        if fx.age >= fx.life {
            commands.entity(entity).despawn();
        }
    }
}

/// When nothing is playing, the clock follows the sim's turn: it starts a game's first turn (whose
/// `TurnStarted` came from setting the game up, not from an event) and recovers anything missed.
fn adopt_turn(game: Option<Res<Match>>, timeline: Res<Timeline>, time: Res<Time>, mut clock: ResMut<TurnClock>, mut focus: ResMut<Focus>, views: Query<(&MobileView, &Shown)>) {
    let Some(game) = game else { return };
    if timeline.busy() {
        return;
    }
    let current = super::world::current_seat(&game);
    if clock.seat != current {
        match current {
            Some(seat) => clock.start(seat, time.elapsed_secs_f64()),
            // The game is over: no one's turn, and nothing started.
            None => clock.seat = None,
        }
        if let Some((_, shown)) = views.iter().find(|(view, _)| Some(view.seat) == current) {
            focus.0 = Some(shown.foot);
        }
    }
}

fn reset(mut timeline: ResMut<Timeline>, mut clock: ResMut<TurnClock>, mut outcome: ResMut<Outcome>) {
    *timeline = Timeline::default();
    *clock = TurnClock::default();
    outcome.0 = None;
}

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .init_resource::<TurnClock>()
            .init_resource::<Outcome>()
            .add_systems(Startup, make_dot)
            .add_systems(OnExit(Screen::Game), reset)
            .add_systems(Update, (enqueue, play, adopt_turn, fade).chain().after(ApplyEvents).run_if(in_state(Screen::Game)));
    }
}
