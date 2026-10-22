//! Short-lived visuals that scenes leave behind: blast and dust puffs, rising numbers and words, an
//! item's icon popping over its user, and lightning bolts. Each fades on its own; playback decides
//! when they appear.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lobshot_sim::units::Vec2 as SimVec2;

use super::{GameRoot, world, world::depth};
use crate::model::Screen;

/// A soft round sprite for shells, strands, beacons and puffs.
#[derive(Resource)]
pub struct Dot(pub Handle<Image>);

/// Something that fades out over `life` seconds, rising `rise` px a second.
#[derive(Component)]
pub struct Fx {
    age: f32,
    life: f32,
    rise: f32,
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

/// A flash at a sim point `size` px across, fading as it grows: a blast's (orange), or a bounce's
/// dust.
pub fn puff(dot: &Dot, at: SimVec2, size: f32, color: Color, life: f32) -> impl Bundle {
    (
        GameRoot,
        Fx { age: 0.0, life, rise: 0.0 },
        Sprite {
            image: dot.0.clone(),
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(world(at).extend(depth::SHOTS + 1.0)),
    )
}

/// Words or a number rising from a point in world space (Bevy's, y up): damage, a heal, "SS ready".
pub fn rising(text: impl Into<String>, color: Color, size: f32, at: Vec2) -> impl Bundle {
    (
        GameRoot,
        Fx { age: 0.0, life: 1.1, rise: 45.0 },
        Text2d::new(text),
        TextFont {
            font_size: bevy::text::FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Transform::from_translation(at.extend(depth::OVERLAYS + 1.0)),
    )
}

/// An image rising from a point in world space, `size` world px: an item's icon over its user.
pub fn pop(image: Handle<Image>, size: Vec2, at: Vec2) -> impl Bundle {
    (
        GameRoot,
        Fx { age: 0.0, life: 1.4, rise: 30.0 },
        Sprite {
            image,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_translation(at.extend(depth::OVERLAYS + 1.0)),
    )
}

/// How long a bolt stays lit, in seconds.
const BOLT_SECONDS: f32 = 0.35;

/// Bolts of lightning lit now.
#[derive(Resource, Default)]
pub struct Bolts(Vec<Bolt>);

struct Bolt {
    from: SimVec2,
    to: SimVec2,
    left: f32,
    /// Picks its zigzag, so it holds its shape while it's lit.
    seed: u32,
    /// Skyeye's beam, at this level: straight, and whiter the higher it is (WP06).
    beam: Option<u8>,
}

impl Bolts {
    /// A bolt from `from` (the sky, or a side of the map) to where it struck.
    pub fn strike(&mut self, from: SimVec2, to: SimVec2) {
        // Seeded by where it runs, so bolts in a row zigzag differently.
        let bits = from.x.to_bits() ^ to.x.to_bits().rotate_left(21) ^ to.y.to_bits().rotate_left(42);
        let seed = u32::try_from(bits % u64::from(u32::MAX)).unwrap_or(1);
        self.0.push(Bolt {
            from,
            to,
            left: BOLT_SECONDS,
            seed,
            beam: None,
        });
    }

    /// Skyeye's beam from the satellite to where it struck, at `level`.
    pub fn beam(&mut self, from: SimVec2, to: SimVec2, level: u8) {
        self.0.push(Bolt {
            from,
            to,
            left: BOLT_SECONDS,
            seed: 0,
            beam: Some(level),
        });
    }
}

/// A beam's color at `level`: green at level 1, white by the top (WP06).
fn beam_color(level: u8, glow: f32) -> Color {
    let white = (f32::from(level.saturating_sub(1)) / 5.0).clamp(0.0, 1.0);
    Color::srgba(0.45 + 0.55 * white, 1.0, 0.5 + 0.5 * white, glow)
}

/// The bolt's zigzag: `SEGMENTS` steps from `from` to `to`, each joint pushed sideways by up to
/// `JAG` px (none at the ends), the same every frame for one seed.
fn zigzag(from: Vec2, to: Vec2, seed: u32) -> Vec<Vec2> {
    const SEGMENTS: u16 = 9;
    const JAG: f32 = 14.0;
    let across = (to - from).perp().normalize_or_zero();
    let mut state = seed | 1;
    (0..=SEGMENTS)
        .map(|step| {
            let t = f32::from(step) / f32::from(SEGMENTS);
            // xorshift32: a cheap, repeatable wobble.
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let wobble = if step == 0 || step == SEGMENTS {
                0.0
            } else {
                (f32::from(u16::try_from(state % 2001).unwrap_or(1000)) / 1000.0 - 1.0) * JAG
            };
            from.lerp(to, t) + across * wobble
        })
        .collect()
}

fn draw_bolts(mut bolts: ResMut<Bolts>, time: Res<Time>, mut gizmos: Gizmos) {
    let dt = time.delta_secs();
    bolts.0.retain_mut(|bolt| {
        bolt.left -= dt;
        bolt.left > 0.0
    });
    for bolt in &bolts.0 {
        let glow = (bolt.left / BOLT_SECONDS).clamp(0.0, 1.0);
        if let Some(level) = bolt.beam {
            let (from, to) = (world(bolt.from), world(bolt.to));
            // Three strokes side by side read as a fat beam.
            for offset in [-1.5, 0.0, 1.5] {
                gizmos.line_2d(from + Vec2::X * offset, to + Vec2::X * offset, beam_color(level, glow));
            }
            continue;
        }
        let points = zigzag(world(bolt.from), world(bolt.to), bolt.seed);
        gizmos.linestrip_2d(points.iter().copied(), Color::srgba(1.0, 0.95, 0.55, glow));
        // A second, offset stroke reads as a thicker bolt.
        gizmos.linestrip_2d(points.iter().map(|point| *point + Vec2::X), Color::srgba(1.0, 1.0, 1.0, glow));
    }
}

/// A fading effect: a puff, a rising number or word, or a popping icon.
type Fading = (Entity, &'static mut Fx, &'static mut Transform, Option<&'static mut Sprite>, Option<&'static mut TextColor>);

fn fade(mut effects: Query<Fading>, time: Res<Time>, mut commands: Commands) {
    for (entity, mut fx, mut transform, sprite, text) in &mut effects {
        fx.age += time.delta_secs();
        let left = 1.0 - (fx.age / fx.life).clamp(0.0, 1.0);
        transform.translation.y += fx.rise * time.delta_secs();
        if let Some(mut sprite) = sprite {
            sprite.color.set_alpha(left);
            // Puffs grow as they fade; icons rise instead.
            if fx.rise == 0.0 {
                transform.scale = Vec3::splat(1.0 + (1.0 - left) * 0.6);
            }
        }
        if let Some(mut text) = text {
            text.0.set_alpha(left);
        }
        if fx.age >= fx.life {
            commands.entity(entity).despawn();
        }
    }
}

fn reset(mut bolts: ResMut<Bolts>) {
    bolts.0.clear();
}

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bolts>()
            .add_systems(Startup, make_dot)
            .add_systems(OnExit(Screen::Game), reset)
            .add_systems(Update, (fade, draw_bolts).run_if(in_state(Screen::Game)));
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_bolt_runs_end_to_end_and_holds_its_shape() {
        let (from, to) = (Vec2::new(100.0, 400.0), Vec2::new(130.0, 0.0));
        let points = zigzag(from, to, 7);
        assert_eq!(points.first(), Some(&from));
        assert_eq!(points.last(), Some(&to));
        assert_eq!(points, zigzag(from, to, 7), "the same seed, the same bolt");
        assert_ne!(points, zigzag(from, to, 8));
        let straight = |point: &Vec2| (point.x - from.x - (to.x - from.x) * (point.y - from.y) / (to.y - from.y)).abs();
        assert!(points.iter().all(|point| straight(point) <= 15.0), "within the jag of the line");
    }
}
