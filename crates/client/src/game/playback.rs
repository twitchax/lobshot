//! Showing what happened. Each accepted event's effects become a short scene, in stages (a Dual's
//! second shot, a drop at the turn's end): shells, strands, beacons, guides and drops fly their legs
//! in real time, blasts carve the terrain art as they land, bolts strike, numbers rise, mobiles fall,
//! teleport and trade places, and the turn passes when it's all over. Scenes play one at a time, in
//! order, on the schedule in `lobshot_protocol::pacing`, which the server waits out before starting
//! the next turn's clock.

use std::collections::VecDeque;

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_assets::Sound;
use lobshot_protocol::pacing::{self, FALL_SECONDS};
use lobshot_sim::{
    ballistics,
    game::{Death, Effect, Event, Leg, SuddenDeathKind, Team},
    items::Item,
    movement::Footing,
    tuning::TURN_TIME_LIMIT,
    units::{TICKS_PER_SECOND, Vec2 as SimVec2},
};

use super::{
    Focus, GameRoot,
    fx::{Bolts, Dot, pop, puff, rising},
    world,
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

/// When sudden death began, as its scene shows it, for the HUD's banner.
#[derive(Resource, Default)]
pub struct Herald(pub Option<(SuddenDeathKind, f64)>);

enum Cue {
    Explode {
        at: SimVec2,
        radius: f64,
    },
    Damage {
        seat: u8,
        amount: i32,
    },
    Shield {
        seat: u8,
        change: i32,
    },
    Heal {
        seat: u8,
        amount: i32,
    },
    Die {
        seat: u8,
    },
    Fall {
        seat: u8,
        to: Footing,
    },
    Turn {
        seat: u8,
    },
    Over {
        winner: Option<Team>,
    },
    Sound(Sound),
    /// A stage begins with this seat's volley: look at it.
    Focus {
        seat: u8,
    },
    Teleport {
        seat: u8,
        to: Footing,
    },
    Swap {
        seat: u8,
        with: u8,
    },
    Bolt {
        from: SimVec2,
        to: SimVec2,
    },
    Bounce {
        at: SimVec2,
    },
    /// Marblemole's SS grinding through a mobile.
    Spark {
        seat: u8,
    },
    Item {
        seat: u8,
        item: Item,
    },
    Say {
        seat: u8,
        words: &'static str,
        color: Color,
    },
    SuddenDeath(SuddenDeathKind),
}

/// Something flying its path: a shell, a beacon, a guide, a falling drop, or one strand of a helix,
/// which winds about the path (amplitude, hertz and phase, for `ballistics::helix_point`). It shows
/// once its first leg starts: Herdhound's guides and missiles leave the warhead mid-flight.
struct Flyer {
    legs: Vec<Leg>,
    strand: Option<(f64, f64, f64)>,
    /// When it leaves and when it stops, in the scene's time.
    from: f32,
    until: f32,
    entity: Entity,
}

/// A mobile moving in a scene, over `FALL_SECONDS`: falling (accelerating down), or gliding to
/// where a teammate stood (a swap).
struct Move {
    seat: u8,
    from: SimVec2,
    to: SimVec2,
    started: f32,
    fall: bool,
}

impl Move {
    fn new(seat: u8, from: SimVec2, to: SimVec2, started: f32, fall: bool) -> Self {
        Self { seat, from, to, started, fall }
    }
}

struct Scene {
    elapsed: f32,
    cues: Vec<(f32, Cue)>,
    next: usize,
    flyers: Vec<Flyer>,
    moves: Vec<Move>,
    end: f32,
}

fn ticks(count: u32) -> f32 {
    // Flights last seconds: tens of thousands of ticks at most, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let seconds = count as f32 / TICKS_PER_SECOND as f32;
    seconds
}

/// Where a flyer is `seconds` into its stage, following its legs (holding still between legs
/// while a tornado spins it), and winding about them if it's a strand; `None` before its first leg
/// starts.
fn flyer_at(legs: &[Leg], strand: Option<(f64, f64, f64)>, seconds: f32) -> Option<SimVec2> {
    // A clock reading in seconds, back to whole ticks.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tick = (f64::from(seconds) * f64::from(TICKS_PER_SECOND)) as u32;
    let mut last = None;
    for leg in legs {
        let point = |local: u32| match strand {
            Some((amplitude, hertz, phase)) => ballistics::helix_point(&leg.flight, local, leg.offset, amplitude, hertz, phase),
            None => leg.flight.at(local),
        };
        match tick.checked_sub(leg.offset) {
            Some(local) if local < leg.from => return last,
            Some(local) if local <= leg.to => return Some(point(local)),
            Some(_) => last = Some(point(leg.to)),
            None => return last,
        }
    }
    last
}

/// How far into its stage an effect's path starts, in seconds, if it has one.
fn first_leg_start(effect: &Effect) -> Option<f32> {
    match effect {
        Effect::Fired { legs, .. } | Effect::Helix { legs, .. } | Effect::Beacon { legs, .. } | Effect::Guide { legs, .. } | Effect::Dropped { legs, .. } => {
            legs.first().map(|leg| ticks(leg.offset + leg.from))
        }
        _ => None,
    }
}

fn enqueue(mut played: MessageReader<Played>, mut timeline: ResMut<Timeline>) {
    for scene in played.read() {
        timeline.queue.push_back(scene.clone());
    }
}

/// A flyer's sprite, out of sight until it leaves.
fn flyer_sprite(sprite: Sprite) -> impl Bundle {
    (GameRoot, sprite, Transform::from_xyz(0.0, 0.0, depth::SHOTS), Visibility::Hidden)
}

/// A dot `size` across in `color`.
fn dot_sprite(dot: &Dot, color: Color, size: f32) -> Sprite {
    Sprite {
        image: dot.0.clone(),
        color,
        custom_size: Some(Vec2::splat(size)),
        ..default()
    }
}

/// What a scene is made from.
struct Props<'a> {
    dot: &'a Dot,
    server: &'a AssetServer,
}

/// What an effect sends flying at `at`: a shell, the strands of a helix, a beacon, or a falling
/// drop (a bolt drops nothing: it strikes at once, `Bolt` next).
fn take_off(effect: &Effect, at: f32, props: &Props, commands: &mut Commands) -> Vec<Flyer> {
    let mut flyers = Vec::new();
    let mut fly = |legs: &[Leg], strand, until: f32, sprite: Sprite| {
        let entity = commands.spawn(flyer_sprite(sprite)).id();
        flyers.push(Flyer {
            legs: legs.to_vec(),
            strand,
            from: at,
            until,
            entity,
        });
    };
    match effect {
        Effect::Fired { legs, .. } => fly(legs, None, at + pacing::flight_seconds(legs), dot_sprite(props.dot, Color::srgb_u8(0x2a, 0x22, 0x1c), 10.0)),
        // Lanternwing's S2: each strand flies until it strikes.
        Effect::Helix { legs, amplitude, hertz, strands, .. } => {
            for strand in strands {
                let sprite = dot_sprite(props.dot, Color::srgb_u8(0xd6, 0x9b, 0xff), 9.0);
                fly(legs, Some((*amplitude, *hertz, strand.phase)), at + ticks(strand.to), sprite);
            }
        }
        Effect::Beacon { legs, .. } => fly(legs, None, at + pacing::flight_seconds(legs), dot_sprite(props.dot, Color::srgb_u8(0x7f, 0xf0, 0xff), 16.0)),
        // Herdhound's whistle: small and bright, where its missiles are headed.
        Effect::Guide { legs, .. } => fly(legs, None, at + pacing::flight_seconds(legs), dot_sprite(props.dot, Color::srgb_u8(0xff, 0xe2, 0x6b), 7.0)),
        Effect::Dropped { drop, legs, .. } if !legs.is_empty() => {
            let sprite = Sprite {
                image: props.server.load(lobshot_assets::drop_path(drop.kind)),
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            };
            fly(legs, None, at + pacing::flight_seconds(legs), sprite);
        }
        _ => {}
    }
    flyers
}

/// The cues an effect at `at` adds to its scene.
fn cue(effect: Effect, at: f32, cues: &mut Vec<(f32, Cue)>) {
    match effect {
        Effect::Volley { seat, .. } => cues.push((at, Cue::Focus { seat })),
        Effect::Exploded { at: center, radius, .. } => {
            cues.push((at, Cue::Explode { at: center, radius }));
            cues.push((at, Cue::Sound(Sound::Blast)));
        }
        Effect::Bolt { from, to, .. } => cues.push((at, Cue::Bolt { from, to })),
        Effect::Bounced { at: point, .. } => cues.push((at, Cue::Bounce { at: point })),
        // Scheduled at its end; it sparks from its start.
        Effect::Pierced { seat, from, to } => {
            let start = at - ticks(to.saturating_sub(from));
            cues.extend([start, f32::midpoint(start, at), at].map(|when| (when, Cue::Spark { seat })));
        }
        Effect::Damaged { seat, amount } => {
            cues.push((at, Cue::Damage { seat, amount }));
            cues.push((at, Cue::Sound(Sound::Hit)));
        }
        Effect::Shielded { seat, amount } => cues.push((at, Cue::Shield { seat, change: -amount })),
        Effect::ShieldStripped { seat, amount, .. } => {
            cues.push((at, Cue::Shield { seat, change: -amount }));
            cues.push((
                at,
                Cue::Say {
                    seat,
                    words: "Shield broken",
                    color: theme::SHIELD,
                },
            ));
        }
        Effect::ShieldRestored { seat, amount } => cues.push((at, Cue::Shield { seat, change: amount })),
        Effect::Healed { seat, amount } => cues.push((at, Cue::Heal { seat, amount })),
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
        Effect::Teleported { seat, to, .. } => {
            cues.push((at, Cue::Teleport { seat, to }));
            cues.push((at, Cue::Sound(Sound::Land)));
        }
        // The schedule puts a swap where the glide ends; the glide starts `FALL_SECONDS` before.
        Effect::Swapped { seat, with } => cues.push(((at - FALL_SECONDS).max(0.0), Cue::Swap { seat, with })),
        Effect::ItemUsed { seat, item, .. } => {
            cues.push((at, Cue::Item { seat, item }));
            cues.push((at, Cue::Sound(Sound::Click)));
        }
        Effect::SsLocked { seat, .. } => cues.push((
            at,
            Cue::Say {
                seat,
                words: "SS locked",
                color: theme::MUTED,
            },
        )),
        Effect::SsReady { seat } => cues.push((
            at,
            Cue::Say {
                seat,
                words: "SS ready",
                color: theme::ACCENT,
            },
        )),
        Effect::SuddenDeath { kind } => cues.push((at, Cue::SuddenDeath(kind))),
        Effect::TurnStarted { seat } => cues.push((at, Cue::Turn { seat })),
        Effect::GameOver { winner } => cues.push((at, Cue::Over { winner })),
        // Flyers took off above; the HUD shows the rest from the game itself (the wind, the
        // moondisk, the reel).
        Effect::Fired { .. }
        | Effect::Helix { .. }
        | Effect::Beacon { .. }
        | Effect::Guide { .. }
        | Effect::Dropped { .. }
        | Effect::Walked { .. }
        | Effect::WindChanged(_)
        | Effect::Forecast(_)
        | Effect::WeatherChanged { .. }
        | Effect::Revealed { .. }
        | Effect::Spun { .. }
        | Effect::DropRolled { .. } => {}
    }
}

/// Turns one event's effects into a scene.
fn stage(played: Played, props: &Props, views: &mut Query<(&MobileView, &mut Shown)>, commands: &mut Commands) -> Scene {
    let mut cues = Vec::new();
    let mut flyers = Vec::new();
    let schedule = pacing::schedule(&played.effects);

    if let Event::Shoot { seat, aim, .. } = played.event {
        for (view, mut shown) in views.iter_mut() {
            if view.seat == seat {
                shown.aim = aim;
            }
        }
    }
    for (effect, at) in played.effects.into_iter().zip(schedule.at) {
        let launched = take_off(&effect, at, props, commands);
        // Shells, strands and beacons leave a cannon, as their first leg starts (a missile bursts
        // out of a warhead mid-flight); a drop falls quietly, and a guide rides the warhead's.
        let quiet = launched.is_empty() || matches!(effect, Effect::Dropped { .. } | Effect::Guide { .. });
        if let Some(start) = first_leg_start(&effect).filter(|_| !quiet) {
            cues.push((at + start, Cue::Sound(Sound::Fire)));
        }
        flyers.extend(launched);
        cue(effect, at, &mut cues);
    }
    // Stable, so cues at the same moment keep the sim's order (the turn passing last).
    cues.sort_by(|a, b| a.0.total_cmp(&b.0));
    Scene {
        elapsed: 0.0,
        cues,
        next: 0,
        flyers,
        moves: Vec::new(),
        end: schedule.end,
    }
}

/// What a scene changes besides the mobiles.
#[derive(SystemParam)]
struct Stagehands<'w> {
    time: Res<'w, Time>,
    dot: Option<Res<'w, Dot>>,
    server: Res<'w, AssetServer>,
    art: Option<Res<'w, MapArt>>,
    images: ResMut<'w, Assets<Image>>,
    focus: ResMut<'w, Focus>,
    clock: ResMut<'w, TurnClock>,
    outcome: ResMut<'w, Outcome>,
    herald: ResMut<'w, Herald>,
    bolts: ResMut<'w, Bolts>,
    sounds: MessageWriter<'w, Play>,
}

fn play(
    mut timeline: ResMut<Timeline>,
    mut hands: Stagehands,
    mut views: Query<(&MobileView, &mut Shown)>,
    mut transforms: Query<(&mut Transform, &mut Visibility), Without<MobileView>>,
    mut commands: Commands,
) {
    let Some(dot) = hands.dot.as_deref().map(|dot| Dot(dot.0.clone())) else { return };
    if timeline.scene.is_none() {
        let Some(next) = timeline.queue.pop_front() else { return };
        let props = Props { dot: &dot, server: &hands.server };
        timeline.scene = Some(stage(next, &props, &mut views, &mut commands));
    }
    let Some(scene) = timeline.scene.as_mut() else { return };
    scene.elapsed += hands.time.delta_secs();
    let now = scene.elapsed;

    fly(&mut scene.flyers, now, &mut transforms, &mut hands.focus, &mut commands);
    while let Some((at, cue)) = scene.cues.get(scene.next) {
        if *at > now {
            break;
        }
        run(cue, now, &mut scene.moves, &dot, &mut views, &mut hands, &mut commands);
        scene.next += 1;
    }
    for change in &scene.moves {
        let t = f64::from(((now - change.started) / FALL_SECONDS).clamp(0.0, 1.0));
        // Falls accelerate, like a drop; swaps glide.
        let down = if change.fall { t * t } else { t };
        let foot = SimVec2::new(change.from.x + (change.to.x - change.from.x) * t, change.from.y + (change.to.y - change.from.y) * down);
        adjust(&mut views, change.seat, |shown| shown.foot = foot);
    }

    if now >= scene.end && scene.next >= scene.cues.len() {
        timeline.scene = None;
    }
}

/// Where a seat's mobile stands now, as shown.
fn foot_of(views: &Query<(&MobileView, &mut Shown)>, seat: u8) -> Option<SimVec2> {
    views.iter().find(|(view, _)| view.seat == seat).map(|(_, shown)| shown.foot)
}

/// Plays one cue at `now`.
fn run(cue: &Cue, now: f32, moves: &mut Vec<Move>, dot: &Dot, views: &mut Query<(&MobileView, &mut Shown)>, hands: &mut Stagehands, commands: &mut Commands) {
    // Near a seat's mobile, `offset` px from its foot, in Bevy's world space (y up).
    let over = |views: &Query<(&MobileView, &mut Shown)>, seat: u8, offset: Vec2| foot_of(views, seat).map(|foot| world(foot) + offset);
    match *cue {
        Cue::Explode { at, radius } => {
            if let Some(art) = hands.art.as_deref() {
                art.carve(&mut hands.images, at, radius);
            }
            // Blast radii are tens of pixels.
            #[allow(clippy::cast_possible_truncation)]
            let size = (radius * 2.4) as f32;
            commands.spawn(puff(dot, at, size, Color::srgb_u8(0xff, 0xb3, 0x47), 0.45));
        }
        Cue::Damage { seat, amount } => {
            adjust(views, seat, |shown| shown.hp -= amount);
            if let Some(at) = over(views, seat, Vec2::new(0.0, 90.0)) {
                commands.spawn(rising(amount.to_string(), theme::ACCENT, 22.0, at));
            }
        }
        // Beside the damage number, shields to the left and heals to the right, so they read apart.
        Cue::Shield { seat, change } => {
            adjust(views, seat, |shown| shown.shield += change);
            if let Some(at) = over(views, seat, Vec2::new(-40.0, 84.0)) {
                commands.spawn(rising(format!("{change:+}"), theme::SHIELD, 16.0, at));
            }
        }
        Cue::Heal { seat, amount } => {
            adjust(views, seat, |shown| shown.hp += amount);
            if let Some(at) = over(views, seat, Vec2::new(40.0, 84.0)) {
                commands.spawn(rising(format!("+{amount}"), theme::GOOD, 22.0, at));
            }
        }
        Cue::Die { seat } => adjust(views, seat, |shown| shown.alive = false),
        Cue::Fall { seat, to } => {
            if let Some(from) = foot_of(views, seat) {
                moves.push(Move::new(seat, from, to.world(), now, true));
            }
        }
        Cue::Swap { seat, with } => {
            if let (Some(one), Some(other)) = (foot_of(views, seat), foot_of(views, with)) {
                moves.push(Move::new(seat, one, other, now, false));
                moves.push(Move::new(with, other, one, now, false));
            }
        }
        Cue::Teleport { seat, to } => {
            let flash = Color::srgb_u8(0x7f, 0xf0, 0xff);
            if let Some(from) = foot_of(views, seat) {
                commands.spawn(puff(dot, from + SimVec2::new(0.0, -30.0), 70.0, flash, 0.5));
            }
            commands.spawn(puff(dot, to.world() + SimVec2::new(0.0, -30.0), 70.0, flash, 0.5));
            adjust(views, seat, |shown| shown.foot = to.world());
        }
        Cue::Bolt { from, to } => hands.bolts.strike(from, to),
        Cue::Bounce { at } => {
            commands.spawn(puff(dot, at, 16.0, Color::srgb_u8(0xc9, 0xa8, 0x7c), 0.35));
        }
        Cue::Spark { seat } => {
            if let Some(foot) = foot_of(views, seat) {
                commands.spawn(puff(dot, foot + SimVec2::new(0.0, -28.0), 26.0, Color::srgb_u8(0xff, 0xf0, 0x80), 0.3));
            }
        }
        Cue::Item { seat, item } => {
            if let Some(at) = over(views, seat, Vec2::new(0.0, 140.0)) {
                let image = hands.server.load(lobshot_assets::item_path(item));
                commands.spawn(pop(image, Vec2::new(26.0 * f32::from(item.slots()), 26.0), at));
            }
        }
        Cue::Say { seat, words, color } => {
            if let Some(at) = over(views, seat, Vec2::new(0.0, 128.0)) {
                commands.spawn(rising(words, color, 14.0, at));
            }
        }
        Cue::SuddenDeath(kind) => hands.herald.0 = Some((kind, hands.time.elapsed_secs_f64())),
        Cue::Focus { seat } => {
            if let Some(foot) = foot_of(views, seat) {
                hands.focus.0 = Some(foot);
            }
        }
        Cue::Turn { seat } => {
            hands.clock.start(seat, hands.time.elapsed_secs_f64());
            if let Some(foot) = foot_of(views, seat) {
                hands.focus.0 = Some(foot);
            }
        }
        Cue::Over { winner } => hands.outcome.0 = Some(winner.map_or(Ending::Draw, Ending::Winner)),
        Cue::Sound(sound) => {
            hands.sounds.write(Play(sound));
        }
    }
}

/// Moves each flyer along its path once it has left (the camera following), and clears those that
/// have stopped.
fn fly(flyers: &mut Vec<Flyer>, now: f32, transforms: &mut Query<(&mut Transform, &mut Visibility), Without<MobileView>>, focus: &mut Focus, commands: &mut Commands) {
    flyers.retain(|flyer| {
        if now > flyer.until {
            commands.entity(flyer.entity).despawn();
            return false;
        }
        if now >= flyer.from
            && let (Some(at), Ok((mut transform, mut visibility))) = (flyer_at(&flyer.legs, flyer.strand, now - flyer.from), transforms.get_mut(flyer.entity))
        {
            transform.translation = world(at).extend(depth::SHOTS);
            *visibility = Visibility::Inherited;
            focus.0 = Some(at);
        }
        true
    });
}

/// Changes how one seat's mobile is shown.
fn adjust(views: &mut Query<(&MobileView, &mut Shown)>, seat: u8, change: impl FnOnce(&mut Shown)) {
    if let Some((_, mut shown)) = views.iter_mut().find(|(view, _)| view.seat == seat) {
        change(&mut shown);
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

fn reset(mut timeline: ResMut<Timeline>, mut clock: ResMut<TurnClock>, mut outcome: ResMut<Outcome>, mut herald: ResMut<Herald>) {
    *timeline = Timeline::default();
    *clock = TurnClock::default();
    outcome.0 = None;
    herald.0 = None;
}

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .init_resource::<TurnClock>()
            .init_resource::<Outcome>()
            .init_resource::<Herald>()
            .add_systems(OnExit(Screen::Game), reset)
            .add_systems(Update, (enqueue, play, adopt_turn).chain().after(ApplyEvents).run_if(in_state(Screen::Game)));
    }
}

#[cfg(test)]
mod unit {
    // Exact float equality is the claim: the times are copied or summed, not computed apart.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use lobshot_sim::{ballistics::Flight, units::Vec2 as SimVec2};

    use super::*;

    fn leg(from: u32, to: u32, offset: u32) -> Leg {
        Leg {
            flight: Flight {
                origin: SimVec2::new(0.0, 0.0),
                velocity: SimVec2::new(240.0, -240.0),
                accel: SimVec2::new(0.0, 600.0),
            },
            from,
            to,
            offset,
        }
    }

    #[test]
    fn a_flyer_follows_its_legs_and_waits_out_the_gap_between_them() {
        let second = TICKS_PER_SECOND;
        let legs = [leg(0, second, 0), leg(0, second, 2 * second)];
        let at = |seconds: f32| flyer_at(&legs, None, seconds).unwrap();
        assert_eq!(at(0.0), legs[0].flight.at(0));
        assert_eq!(at(0.5), legs[0].flight.at(second / 2));
        assert_eq!(at(1.5), legs[0].flight.at(second), "held between legs, as a tornado holds it");
        assert_eq!(at(2.5), legs[1].flight.at(second / 2));
        assert_eq!(at(9.0), legs[1].flight.at(second), "stopped at the end");
    }

    #[test]
    fn a_flyer_shows_once_its_first_leg_starts() {
        // A missile bursting out of Herdhound's warhead a second into the stage.
        let second = TICKS_PER_SECOND;
        let legs = [leg(0, second, second)];
        assert_eq!(flyer_at(&legs, None, 0.5), None, "not out yet");
        assert_eq!(flyer_at(&legs, None, 1.5), Some(legs[0].flight.at(second / 2)));
        let late = Effect::Fired { seat: 0, legs: legs.to_vec() };
        assert_eq!(first_leg_start(&late), Some(1.0), "it sounds as it bursts out");
        // A path carried on from another's tick, as a guide carries on its warhead's flight.
        let carried = [leg(second, 2 * second, 0)];
        assert_eq!(flyer_at(&carried, None, 0.5), None);
        assert_eq!(flyer_at(&carried, None, 1.5), Some(carried[0].flight.at(3 * second / 2)));
    }

    #[test]
    fn a_pierce_sparks_through_its_window_and_a_swap_glides_up_to_its_time() {
        let mut cues = Vec::new();
        cue(Effect::Pierced { seat: 1, from: 0, to: TICKS_PER_SECOND }, 3.0, &mut cues);
        let times: Vec<f32> = cues.iter().map(|(at, _)| *at).collect();
        assert_eq!(times, [2.0, 2.5, 3.0], "scheduled at its end, from its start");
        cues.clear();
        cue(Effect::Swapped { seat: 0, with: 2 }, 2.0, &mut cues);
        assert!(matches!(cues[..], [(at, Cue::Swap { seat: 0, with: 2 })] if at == 2.0 - FALL_SECONDS));
    }

    #[test]
    fn a_strand_winds_about_the_path_where_the_sim_puts_it() {
        let legs = [leg(0, TICKS_PER_SECOND, 0)];
        let strand = Some((12.0, 2.0, 1.0));
        let tick = TICKS_PER_SECOND / 4;
        assert_eq!(flyer_at(&legs, strand, 0.25).unwrap(), ballistics::helix_point(&legs[0].flight, tick, 0, 12.0, 2.0, 1.0));
        assert_ne!(flyer_at(&legs, strand, 0.25), flyer_at(&legs, None, 0.25));
    }
}
