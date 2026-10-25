//! Jewels on the map (Jewel mode, `lobshot_sim::jewels`): gems in GunBound's colors, each with its
//! worth written over it. Between scenes they sit where the predicted game has them; during one,
//! the playback's cues (`Cue`, `stage`) show them land, take damage, fall and go.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lobshot_sim::{
    game::Team,
    jewels::{Jewel, JewelId, JewelKind},
    tuning::jewels::SIZE,
    units::Vec2 as SimVec2,
};

use super::{
    GameRoot, Timeline,
    fx::{Dot, puff, rising},
    world,
    world::depth,
};
use crate::{
    model::{Match, Screen},
    ui::{team_color, theme},
};

/// A jewel on screen.
#[derive(Component)]
pub struct JewelView {
    pub id: JewelId,
    pub kind: JewelKind,
}

/// A gem: a diamond with a lighter upper facet, tinted by kind.
#[derive(Resource)]
pub struct Gem(pub Handle<Image>);

/// A jewel's color, GunBound's: blue, yellow, red and black (NAMU, GF-cyber).
#[must_use]
pub fn color(kind: JewelKind) -> Color {
    match kind {
        JewelKind::Sapphire => Color::srgb_u8(0x4f, 0x8c, 0xff),
        JewelKind::Topaz => Color::srgb_u8(0xff, 0xd2, 0x3f),
        JewelKind::Ruby => Color::srgb_u8(0xe8, 0x41, 0x3c),
        JewelKind::Onyx => Color::srgb_u8(0x3a, 0x38, 0x48),
    }
}

/// What it's worth, as written over it: "+5", or "-5" for an Onyx.
#[must_use]
pub fn worth(kind: JewelKind) -> String {
    format!("{:+}", kind.points())
}

/// How big a jewel is drawn: its size in the sim. Tens of pixels, exact in f32.
#[allow(clippy::cast_possible_truncation)]
fn size() -> Vec2 {
    Vec2::splat(SIZE.value as f32)
}

/// A jewel's sprite, or a dot in its color until the gem is made.
fn sprite(gem: Option<&Gem>, dot: &Dot, kind: JewelKind) -> Sprite {
    Sprite {
        image: gem.map_or_else(|| dot.0.clone(), |gem| gem.0.clone()),
        color: color(kind),
        custom_size: Some(size()),
        ..default()
    }
}

/// Where a jewel is drawn: its middle, just behind the mobiles.
fn transform(at: SimVec2) -> Transform {
    Transform::from_translation(world(at).extend(depth::MOBILES - 2.0))
}

/// A jewel's view, with its worth over it.
fn spawn(commands: &mut Commands, gem: Option<&Gem>, dot: &Dot, jewel: &Jewel) {
    let label_y = size().y * 0.5 + 9.0;
    commands
        .spawn((GameRoot, JewelView { id: jewel.id, kind: jewel.kind }, sprite(gem, dot, jewel.kind), transform(jewel.middle())))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new(worth(jewel.kind)),
                TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(theme::TEXT),
                Transform::from_xyz(0.0, label_y, 1.0),
            ));
        });
}

/// Between scenes, the jewels on screen match the predicted game's: new ones appear, gone ones go,
/// each sits where it is, and one that changed (an Onyx turned blue) is drawn anew.
fn follow(
    game: Option<Res<Match>>,
    timeline: Res<Timeline>,
    gem: Option<Res<Gem>>,
    dot: Option<Res<Dot>>,
    mut views: Query<(Entity, &JewelView, &mut Transform, &mut Visibility)>,
    mut commands: Commands,
) {
    let (Some(game), Some(dot)) = (game, dot) else { return };
    if timeline.busy() {
        return;
    }
    let jewels = game.predicted.jewels();
    let mut shown = Vec::new();
    for (entity, view, mut transform, mut visibility) in &mut views {
        match jewels.iter().find(|jewel| jewel.id == view.id && jewel.kind == view.kind) {
            Some(jewel) => {
                *transform = self::transform(jewel.middle());
                *visibility = Visibility::Inherited;
                shown.push(jewel.id);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for jewel in jewels.iter().filter(|jewel| !shown.contains(&jewel.id)) {
        spawn(&mut commands, gem.as_deref(), &dot, jewel);
    }
}

/// What a scene does to a jewel on screen, on its own schedule beside the other cues.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cue {
    /// It lands: it appears where it sits.
    Landed(Jewel),
    /// It took damage: the amount rises from it.
    Hit { id: JewelId, amount: i32 },
    /// `team` took it: its points rise from it in the team's color, and it goes.
    Taken { id: JewelId, team: Team },
    /// It shows at `at` (its middle): where it fell to.
    At { id: JewelId, at: SimVec2 },
    /// It is gone, off the map.
    Hide(JewelId),
}

/// The jewels on screen, as playback moves them.
pub type Parts<'w, 's> = Query<'w, 's, (&'static JewelView, &'static mut Transform, &'static mut Visibility), (Without<super::world::MobileView>, Without<super::world::MineView>)>;

/// Plays a jewel's cue on the jewels on screen; a landing that isn't on screen yet is spawned.
pub fn stage(cue: Cue, gem: Option<&Gem>, dot: &Dot, jewels: &mut Parts, commands: &mut Commands) {
    let middle = |jewels: &Parts, id: JewelId| jewels.iter().find(|(view, ..)| view.id == id).map(|(view, transform, _)| (view.kind, transform.translation.truncate()));
    let hide = |jewels: &mut Parts, id: JewelId| {
        for (_, _, mut visibility) in jewels.iter_mut().filter(|(view, ..)| view.id == id) {
            *visibility = Visibility::Hidden;
        }
    };
    match cue {
        Cue::Landed(jewel) => {
            if !jewels.iter().any(|(view, ..)| view.id == jewel.id) {
                spawn(commands, gem, dot, &jewel);
            }
        }
        Cue::Hit { id, amount } => {
            if let Some((_, at)) = middle(jewels, id) {
                commands.spawn(rising(amount.to_string(), theme::ACCENT, 18.0, at + Vec2::new(0.0, 24.0)));
            }
        }
        Cue::Taken { id, team } => {
            if let Some((kind, at)) = middle(jewels, id) {
                commands.spawn(rising(worth(kind), team_color(team), 24.0, at + Vec2::new(0.0, 40.0)));
                commands.spawn(puff(dot, SimVec2::new(f64::from(at.x), f64::from(-at.y)), 48.0, color(kind), 0.5));
            }
            hide(jewels, id);
        }
        Cue::At { id, at } => {
            for (_, mut transform, _) in jewels.iter_mut().filter(|(view, ..)| view.id == id) {
                transform.translation = world(at).extend(transform.translation.z);
            }
        }
        Cue::Hide(id) => hide(jewels, id),
    }
}

/// Draws the gem: a diamond, its upper facet lighter, with a soft edge.
fn make_gem(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    const SIDE: u32 = 32;
    let half = f64::from(SIDE) / 2.0;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (f64::from(x) + 0.5 - half, f64::from(y) + 0.5 - half);
            // A diamond's edge is where |dx| + |dy| reaches the half side.
            let edge = (dx.abs() + dy.abs()) / half;
            let shade = if dy < -half / 4.0 { 255.0 } else { 200.0 };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (alpha, shade) = (((1.0 - edge).clamp(0.0, 0.1) * 10.0 * 255.0) as u8, shade as u8);
            data.extend_from_slice(&[shade, shade, shade, alpha]);
        }
    }
    let image = Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    commands.insert_resource(Gem(images.add(image)));
}

pub struct JewelsPlugin;

impl Plugin for JewelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_gem).add_systems(Update, follow.run_if(in_state(Screen::Game)));
    }
}

#[cfg(test)]
mod unit {
    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn each_kind_has_its_own_color_and_says_what_it_is_worth() {
        let colors: Vec<Color> = JewelKind::ALL.iter().map(|&kind| color(kind)).collect();
        for (i, a) in colors.iter().enumerate() {
            assert!(colors[i + 1..].iter().all(|b| a != b), "{colors:?}");
        }
        assert_eq!(JewelKind::ALL.map(worth), ["+5", "+10", "+25", "-5"]);
    }
}
