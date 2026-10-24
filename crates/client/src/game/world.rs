//! The world: map art (background and carvable terrain tiles) and the mobiles, drawn from the
//! cutout rigs `cargo xtask art` renders. What a mobile looks like right now is its `Shown` state,
//! which the playback timeline moves through each shot; between shots it settles on the predicted
//! game.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    sprite::Anchor,
};
use lobshot_assets::ART_SCALE;
use lobshot_sim::{
    game::{MobileKind, Phase},
    map::MapRef,
    mines::MineId,
    terrain::Terrain,
    units::{Facing, Vec2 as SimVec2},
};

use super::{GameRoot, Timeline, fx::Dot, world};
use crate::{
    assets::Rigs,
    model::{Match, Screen, Session},
    ui::{team_color, theme},
};

/// Terrain art is cut into tiles this many art pixels square, so a crater re-uploads only the
/// tiles it touches.
const TILE: u32 = 512;

/// Draw order, back to front.
pub mod depth {
    pub const BACKGROUND: f32 = -100.0;
    pub const TERRAIN: f32 = 0.0;
    pub const MOBILES: f32 = 10.0;
    pub const SHOTS: f32 = 20.0;
    pub const OVERLAYS: f32 = 30.0;
}

/// One terrain tile.
struct ArtTile {
    image: Handle<Image>,
    /// Its top-left corner and size, in art pixels.
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// The map's art.
#[derive(Resource)]
pub struct MapArt {
    /// Art pixels per world pixel.
    scale: f32,
    /// The drawn terrain while it loads (drawn maps only).
    source: Option<Handle<Image>>,
    tiles: Vec<ArtTile>,
}

impl MapArt {
    /// Carves the art to match a sim crater: clears every art pixel whose center is inside the
    /// circle, and scorches a thin ring around it.
    pub fn carve(&self, images: &mut Assets<Image>, center: SimVec2, radius: f64) {
        let scale = f64::from(self.scale);
        let (cx, cy, r) = (center.x * scale, center.y * scale, radius * scale);
        let scorch = r + 3.0 * scale;
        for tile in &self.tiles {
            let (x0, y0) = (f64::from(tile.x), f64::from(tile.y));
            let (x1, y1) = (x0 + f64::from(tile.w), y0 + f64::from(tile.h));
            if cx + scorch < x0 || cx - scorch > x1 || cy + scorch < y0 || cy - scorch > y1 {
                continue;
            }
            let Some(mut image) = images.get_mut(&tile.image) else { continue };
            let Some(data) = image.data.as_mut() else { continue };
            for py in 0..tile.h {
                let dy = y0 + f64::from(py) + 0.5 - cy;
                if dy.abs() > scorch {
                    continue;
                }
                for px in 0..tile.w {
                    let dx = x0 + f64::from(px) + 0.5 - cx;
                    let distance = (dx * dx + dy * dy).sqrt();
                    if distance > scorch {
                        continue;
                    }
                    let at = ((py * tile.w + px) * 4) as usize;
                    if distance <= r {
                        data[at + 3] = 0;
                    } else if data[at + 3] > 0 {
                        for channel in &mut data[at..at + 3] {
                            *channel = channel.saturating_mul(3) / 5;
                        }
                    }
                }
            }
        }
    }
}

/// A mobile on screen.
#[derive(Component)]
pub struct MobileView {
    pub seat: u8,
}

/// What a mobile looks like right now. Playback moves it through a shot; otherwise it follows the
/// predicted game.
#[derive(Component, Clone, Copy)]
pub struct Shown {
    pub foot: SimVec2,
    pub tilt: f64,
    pub facing: Facing,
    pub hp: i32,
    pub shield: i32,
    pub alive: bool,
    /// The aim to draw the barrels at (this player's live aim, or another's last shot).
    pub aim: i16,
    /// How brittle Frostfin's ice has left it (`Player::brittle`).
    pub brittle: u8,
}

/// The name and HP bar over a mobile (not rotated or mirrored with it).
#[derive(Component)]
pub struct Overlay {
    pub seat: u8,
}

#[derive(Component)]
struct HpFill;

#[derive(Component)]
struct ShieldFill;

/// The tag beside the HP bar of a mobile Frostfin's ice left brittle.
#[derive(Component)]
struct BrittleTag;

/// A brittle mobile's tag: how much more Physical damage it takes, as GunBound's defence counter
/// reads (GF-ice). Empty while it takes no more.
#[must_use]
pub fn brittle_label(brittle: u8) -> String {
    if brittle == 0 { String::new() } else { format!("-{brittle}% DEF") }
}

#[derive(Component)]
struct Placeholder;

#[derive(Component)]
struct Dressed;

#[derive(Component)]
pub struct CannonPart;

#[derive(Component)]
struct WheelPart;

fn art_scale_f32() -> f32 {
    // ART_SCALE is 2.0.
    #[allow(clippy::cast_possible_truncation)]
    let scale = ART_SCALE as f32;
    scale
}

fn as_f32(value: f64) -> f32 {
    // Screen-space values: tens to thousands of pixels or radians, far inside f32's range.
    #[allow(clippy::cast_possible_truncation)]
    let value = value as f32;
    value
}

/// Starts the map's art once a match exists.
fn build_map(game: Option<Res<Match>>, art: Option<Res<MapArt>>, server: Res<AssetServer>, mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let (Some(game), None) = (game, art) else { return };
    let terrain = game.predicted.terrain();
    let map_size = Vec2::new(as_f32(f64::from(terrain.width())), as_f32(f64::from(terrain.height())));
    match &game.log.setup.map {
        MapRef::Asset { name, variant, .. } => {
            commands.spawn((
                GameRoot,
                Sprite {
                    image: server.load(lobshot_assets::background_path(name)),
                    custom_size: Some(map_size),
                    ..default()
                },
                Transform::from_xyz(map_size.x / 2.0, -map_size.y / 2.0, depth::BACKGROUND),
            ));
            commands.insert_resource(MapArt {
                scale: art_scale_f32(),
                source: Some(server.load(lobshot_assets::terrain_path(name, variant))),
                tiles: Vec::new(),
            });
        }
        MapRef::Flat { .. } | MapRef::Hills { .. } => {
            commands.spawn((
                GameRoot,
                Sprite {
                    color: Color::srgb_u8(0x9f, 0xcf, 0xea),
                    custom_size: Some(map_size),
                    ..default()
                },
                Transform::from_xyz(map_size.x / 2.0, -map_size.y / 2.0, depth::BACKGROUND),
            ));
            let pixels = synthetic_art(terrain);
            let art = slice(&mut images, &pixels, terrain.width(), terrain.height(), 1.0, &mut commands);
            commands.insert_resource(art);
        }
    }
}

/// Procedural art for synthetic maps: turf on top, banded earth below, a dark outline.
fn synthetic_art(terrain: &Terrain) -> Vec<u8> {
    let (width, height) = (terrain.width(), terrain.height());
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for x in 0..width {
        let column = i32::try_from(x).unwrap_or(0);
        let mut depth = 0;
        for y in 0..height {
            let row = i32::try_from(y).unwrap_or(0);
            if !terrain.is_solid(column, row) {
                depth = 0;
                continue;
            }
            depth += 1;
            let rgb: [u8; 3] = match depth {
                1..=2 => [0x2b, 0x1d, 0x14],
                3..=8 => [0x5f, 0xa8, 0x4a],
                9..=11 => [0x46, 0x7d, 0x37],
                _ if (depth / 14) % 2 == 0 => [0x8a, 0x5a, 0x3b],
                _ => [0x7a, 0x4f, 0x33],
            };
            let at = ((y * width + x) * 4) as usize;
            pixels[at..at + 3].copy_from_slice(&rgb);
            pixels[at + 3] = 255;
        }
    }
    pixels
}

/// Cuts RGBA art into tiles and spawns their sprites.
fn slice(images: &mut Assets<Image>, pixels: &[u8], width: u32, height: u32, scale: f32, commands: &mut Commands) -> MapArt {
    let mut tiles = Vec::new();
    for ty in (0..height).step_by(TILE as usize) {
        for tx in (0..width).step_by(TILE as usize) {
            let (w, h) = (TILE.min(width - tx), TILE.min(height - ty));
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for row in ty..ty + h {
                let start = ((row * width + tx) * 4) as usize;
                data.extend_from_slice(&pixels[start..start + (w * 4) as usize]);
            }
            let image = Image::new(
                Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                data,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            let handle = images.add(image);
            let (wf, hf) = (as_f32(f64::from(w)) / scale, as_f32(f64::from(h)) / scale);
            let (xf, yf) = (as_f32(f64::from(tx)) / scale, as_f32(f64::from(ty)) / scale);
            commands.spawn((
                GameRoot,
                Sprite {
                    image: handle.clone(),
                    custom_size: Some(Vec2::new(wf, hf)),
                    ..default()
                },
                Transform::from_xyz(xf + wf / 2.0, -(yf + hf / 2.0), depth::TERRAIN),
            ));
            tiles.push(ArtTile { image: handle, x: tx, y: ty, w, h });
        }
    }
    MapArt { scale, source: None, tiles }
}

/// Slices a drawn map's terrain once it has loaded.
fn slice_drawn(art: Option<ResMut<MapArt>>, mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let Some(mut art) = art else { return };
    let Some(source) = art.source.clone() else { return };
    let Some(image) = images.get(&source) else { return };
    let (width, height) = (image.width(), image.height());
    let Some(pixels) = image.data.clone() else {
        warn!("the terrain art has no CPU copy; it can't be carved");
        art.source = None;
        return;
    };
    if image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb || pixels.len() != (width * height * 4) as usize {
        warn!("the terrain art is {:?}, not 8-bit RGBA", image.texture_descriptor.format);
    }
    let sliced = slice(&mut images, &pixels, width, height, art.scale, &mut commands);
    art.tiles = sliced.tiles;
    art.source = None;
}

fn load_rigs(game: Option<Res<Match>>, mut rigs: ResMut<Rigs>, server: Res<AssetServer>) {
    let Some(game) = game else { return };
    for player in game.predicted.players() {
        rigs.request(&server, player.mobile);
    }
}

/// Spawns each seat's mobile (as a placeholder until its rig arrives) and its overlay.
fn spawn_mobiles(game: Option<Res<Match>>, session: Res<Session>, views: Query<&MobileView>, mut commands: Commands) {
    let Some(game) = game else { return };
    if !views.is_empty() {
        return;
    }
    for (seat, player) in game.predicted.players().iter().enumerate() {
        let Ok(seat) = u8::try_from(seat) else { continue };
        let shown = Shown {
            foot: player.footing.world(),
            tilt: player.tilt,
            facing: player.facing,
            hp: player.hp,
            shield: player.shield,
            alive: player.alive,
            aim: player.tuning().s1.aim.value.true_min,
            brittle: player.brittle,
        };
        let hitbox = player.tuning().hitbox.value;
        let color = team_color(player.team);
        commands
            .spawn((GameRoot, MobileView { seat }, shown, Transform::default(), Visibility::default()))
            .with_children(|root| {
                root.spawn((
                    Placeholder,
                    Sprite {
                        color,
                        custom_size: Some(Vec2::new(as_f32(hitbox.w), as_f32(hitbox.h))),
                        ..default()
                    },
                    Transform::from_xyz(as_f32(hitbox.x + hitbox.w / 2.0), as_f32(-(hitbox.y + hitbox.h / 2.0)), 0.0),
                ));
            });
        let name = session.seat_name(seat);
        let shielded = player.tuning().shield.value > 0;
        commands.spawn((GameRoot, Overlay { seat }, Transform::default(), Visibility::default())).with_children(|overlay| {
            overlay.spawn((
                Text2d::new(name),
                TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(theme::TEXT),
                Transform::from_xyz(0.0, if shielded { 19.0 } else { 14.0 }, 0.0),
            ));
            // Mobiles with a shield show it as a thin bar over the HP bar, as GunBound does.
            if shielded {
                overlay.spawn((
                    Sprite {
                        color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                        custom_size: Some(Vec2::new(52.0, 5.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 6.0, 0.0),
                ));
                overlay.spawn((
                    ShieldFill,
                    Sprite {
                        color: theme::SHIELD,
                        custom_size: Some(Vec2::new(50.0, 3.0)),
                        ..default()
                    },
                    Anchor::CENTER_LEFT,
                    Transform::from_xyz(-25.0, 6.0, 0.1),
                ));
            }
            overlay.spawn((
                Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                    custom_size: Some(Vec2::new(52.0, 7.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            overlay.spawn((
                HpFill,
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(50.0, 5.0)),
                    ..default()
                },
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-25.0, 0.0, 0.1),
            ));
            overlay.spawn((
                BrittleTag,
                Text2d::new(brittle_label(shown.brittle)),
                TextFont {
                    font_size: bevy::text::FontSize::Px(11.0),
                    ..default()
                },
                TextColor(theme::FROST),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(30.0, 0.0, 0.1),
            ));
        });
    }
}

/// Swaps a placeholder for the rig's parts once the rig and its images are loaded.
fn dress_mobiles(
    game: Option<Res<Match>>,
    rigs: Res<Rigs>,
    server: Res<AssetServer>,
    views: Query<(Entity, &MobileView, &Children), Without<Dressed>>,
    placeholders: Query<Entity, With<Placeholder>>,
    mut commands: Commands,
) {
    let Some(game) = game else { return };
    for (entity, view, children) in &views {
        let Some(player) = game.predicted.players().get(usize::from(view.seat)) else { continue };
        let Some(rig) = rigs.get(player.mobile) else { continue };
        for child in children.iter() {
            if placeholders.contains(child) {
                commands.entity(child).despawn();
            }
        }
        let scale = 1.0 / art_scale_f32();
        commands.entity(entity).insert(Dressed).with_children(|root| {
            for part in &rig.parts {
                let image = server.load(lobshot_assets::part_path(player.mobile, &part.file));
                let z = f32::from(i16::try_from(part.z).unwrap_or(0)) * 0.01;
                let mut spawned = root.spawn((Sprite::from_image(image), Transform::from_xyz(as_f32(part.at.0), as_f32(-part.at.1), z).with_scale(Vec3::splat(scale))));
                if part.aim {
                    spawned.insert(CannonPart);
                }
                if part.spin {
                    spawned.insert(WheelPart);
                }
            }
            let rider = server.load(lobshot_assets::rider_path("pip"));
            let socket = rig.rider_socket;
            let z = f32::from(i16::try_from(socket.z).unwrap_or(0)) * 0.01;
            root.spawn((
                Sprite::from_image(rider),
                Transform::from_xyz(as_f32(socket.at.0), as_f32(-socket.at.1), z).with_scale(Vec3::splat(scale)),
            ));
        });
    }
}

/// The HP and shield bars' fills, and which each is.
type Fills<'w, 's> = Query<'w, 's, (&'static mut Sprite, Has<ShieldFill>), Or<(With<HpFill>, With<ShieldFill>)>>;

/// The rig parts that move on their own: the cannon and the wheels.
type MovingPart = (Or<(With<CannonPart>, With<WheelPart>)>, Without<MobileView>, Without<Overlay>);

/// Places each mobile and its overlay from its `Shown` state.
fn pose_mobiles(
    mut views: Query<(&MobileView, &Shown, &mut Transform, &mut Visibility, &Children)>,
    mut parts: Query<&mut Transform, MovingPart>,
    cannons: Query<(), With<CannonPart>>,
    mut overlays: Query<(&Overlay, &mut Transform, &mut Visibility, &Children), Without<MobileView>>,
    mut fills: Fills,
    game: Option<Res<Match>>,
) {
    let Some(game) = game else { return };
    let mut shown_by_seat = HashMap::new();
    for (view, shown, mut transform, mut visibility, children) in &mut views {
        shown_by_seat.insert(view.seat, *shown);
        let foot = world(shown.foot);
        transform.translation = foot.extend(depth::MOBILES + f32::from(view.seat) * 0.1);
        transform.rotation = Quat::from_rotation_z(as_f32(shown.tilt));
        transform.scale = Vec3::new(if shown.facing == Facing::Right { 1.0 } else { -1.0 }, 1.0, 1.0);
        *visibility = if shown.alive { Visibility::Inherited } else { Visibility::Hidden };
        for child in children.iter() {
            if let Ok(mut part) = parts.get_mut(child) {
                if cannons.contains(child) {
                    part.rotation = Quat::from_rotation_z(f32::from(shown.aim).to_radians());
                } else {
                    // Wheels roll with distance: one turn per 2πr, for a ~9 px wheel.
                    part.rotation = Quat::from_rotation_z(-foot.x / 9.0);
                }
            }
        }
    }
    for (overlay, mut transform, mut visibility, children) in &mut overlays {
        let Some(shown) = shown_by_seat.get(&overlay.seat) else { continue };
        // Above the rider's hat: Coilback with Pip stands about 95 px tall.
        transform.translation = (world(shown.foot) + Vec2::new(0.0, 104.0)).extend(depth::OVERLAYS);
        *visibility = if shown.alive { Visibility::Inherited } else { Visibility::Hidden };
        let Some(tuning) = game.predicted.players().get(usize::from(overlay.seat)).map(lobshot_sim::game::Player::tuning) else {
            continue;
        };
        for child in children.iter() {
            if let Ok((mut fill, shield)) = fills.get_mut(child) {
                let (now, max, height) = if shield { (shown.shield, tuning.shield.value, 3.0) } else { (shown.hp, tuning.hp.value, 5.0) };
                let fraction = (f64::from(now.max(0)) / f64::from(max.max(1))).clamp(0.0, 1.0);
                fill.custom_size = Some(Vec2::new(50.0 * as_f32(fraction), height));
            }
        }
    }
}

/// Between shots, mobiles settle on the predicted game: walks glide at walking pace (snapping
/// once close), and HP and life match the sim.
fn follow_game(game: Option<Res<Match>>, timeline: Res<Timeline>, time: Res<Time>, mut views: Query<(&MobileView, &mut Shown)>) {
    let Some(game) = game else { return };
    if timeline.busy() {
        return;
    }
    let step = f64::from(time.delta_secs()) * 90.0;
    for (view, mut shown) in &mut views {
        let Some(player) = game.predicted.players().get(usize::from(view.seat)) else { continue };
        let target = player.footing.world();
        let dx = target.x - shown.foot.x;
        if dx.abs() <= step || (target.y - shown.foot.y).abs() > 40.0 {
            shown.foot = target;
            shown.tilt = player.tilt;
        } else {
            shown.foot.x += step.copysign(dx);
            shown.foot.y = target.y;
        }
        shown.facing = player.facing;
        shown.hp = player.hp;
        shown.shield = player.shield;
        shown.alive = player.alive;
        shown.brittle = player.brittle;
    }
}

/// Keeps each brittle tag on what its mobile shows.
fn tag_brittle(overlays: Query<(&Overlay, &Children)>, views: Query<(&MobileView, &Shown)>, mut tags: Query<&mut Text2d, With<BrittleTag>>) {
    for (overlay, children) in &overlays {
        let Some((_, shown)) = views.iter().find(|(view, _)| view.seat == overlay.seat) else { continue };
        for child in children.iter() {
            if let Ok(mut tag) = tags.get_mut(child) {
                let label = brittle_label(shown.brittle);
                if tag.0 != label {
                    tag.0 = label;
                }
            }
        }
    }
}

/// A mine on the map (`lobshot_sim::mines`). Between scenes it sits where the predicted game has
/// it; during one, the playback's cues place and hide it as the scene shows it laid, crawling and
/// gone.
#[derive(Component)]
pub struct MineView {
    pub id: MineId,
}

/// How big a mine is drawn, px: its sprite's own size (`mine.svg` in a rig).
const MINE_SIZE: Vec2 = Vec2::new(18.0, 18.0);

/// The tin grey a mine is drawn in until its layer's sprite arrives.
const TIN: Color = Color::srgb_u8(0x9a, 0xa4, 0xad);

/// An armed mine (an enemy in sense: it crawls as its layer's turn comes up) glows red, as
/// GunBound's antenna turns red (SW-mob).
const ARMED: Color = Color::srgb_u8(0xff, 0x8a, 0x80);

/// A mine's tint: red when armed, else its own colors (or tin, drawn as a dot).
#[must_use]
pub fn mine_tint(armed: bool, dot: bool) -> Color {
    match (armed, dot) {
        (true, _) => ARMED,
        (false, true) => TIN,
        (false, false) => Color::WHITE,
    }
}

/// The picture `mobile`'s rig draws `id` with (`MobileRig::sprites`), once its rig has loaded.
pub fn shot_sprite(rigs: &Rigs, server: &AssetServer, mobile: MobileKind, id: &str) -> Option<Handle<Image>> {
    let sprite = rigs.get(mobile)?.sprite(id)?;
    Some(server.load(lobshot_assets::part_path(mobile, &sprite.file)))
}

/// A mine's sprite: its layer's `mine` picture, or a tin-grey dot until that has loaded.
pub fn mine_sprite(rigs: &Rigs, server: &AssetServer, dot: &Dot, mobile: MobileKind) -> Sprite {
    match shot_sprite(rigs, server, mobile, "mine") {
        Some(image) => Sprite {
            image,
            custom_size: Some(MINE_SIZE),
            ..default()
        },
        None => Sprite {
            image: dot.0.clone(),
            color: TIN,
            custom_size: Some(MINE_SIZE),
            ..default()
        },
    }
}

/// Where a mine is drawn: its middle, flipped the way it faces, just in front of the ground.
pub fn mine_transform(at: SimVec2, facing: Facing) -> Transform {
    Transform::from_translation(world(at).extend(depth::MOBILES - 1.0)).with_scale(Vec3::new(if facing == Facing::Right { 1.0 } else { -1.0 }, 1.0, 1.0))
}

/// Between scenes, the mines on screen match the predicted game's: new ones appear, gone ones go,
/// and each sits where it is.
fn follow_mines(
    game: Option<Res<Match>>,
    timeline: Res<Timeline>,
    rigs: Res<Rigs>,
    server: Res<AssetServer>,
    dot: Option<Res<Dot>>,
    mut views: Query<(Entity, &MineView, &mut Transform, &mut Visibility, &mut Sprite)>,
    mut commands: Commands,
) {
    let (Some(game), Some(dot)) = (game, dot) else { return };
    if timeline.busy() {
        return;
    }
    let mines = game.predicted.mines();
    for (entity, view, mut transform, mut visibility, mut sprite) in &mut views {
        let Some(mine) = mines.iter().find(|mine| mine.id == view.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        *transform = mine_transform(mine.middle(), mine.facing);
        *visibility = Visibility::Inherited;
        // Drawn as a dot before its layer's rig arrived: dress it now.
        if sprite.image == dot.0
            && let Some(layer) = game.predicted.players().get(usize::from(mine.seat))
        {
            *sprite = mine_sprite(&rigs, &server, &dot, layer.mobile);
        }
        sprite.color = mine_tint(lobshot_sim::mines::armed(&game.predicted, mine), sprite.image == dot.0);
    }
    for mine in mines {
        if views.iter().any(|(_, view, ..)| view.id == mine.id) {
            continue;
        }
        let Some(layer) = game.predicted.players().get(usize::from(mine.seat)) else { continue };
        commands.spawn((
            GameRoot,
            MineView { id: mine.id },
            mine_sprite(&rigs, &server, &dot, layer.mobile),
            mine_transform(mine.middle(), mine.facing),
        ));
    }
}

/// The current turn's seat, if the game is on.
#[must_use]
pub fn current_seat(game: &Match) -> Option<u8> {
    match game.predicted.phase() {
        Phase::Turn { seat } => Some(seat),
        Phase::Over { .. } => None,
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (build_map, slice_drawn, load_rigs, spawn_mobiles, dress_mobiles, follow_game, follow_mines, pose_mobiles, tag_brittle)
                .chain()
                .run_if(in_state(Screen::Game)),
        );
    }
}
