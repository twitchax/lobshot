//! Review sheets for the art director, in `target/art-preview/`: each mobile posed at several aims,
//! its exploded parts, every item icon and drop, and each map variant whole, at game scale and as
//! its mask. Mobiles are
//! composed from pivot-centered sprites exactly as the client places them (`DESIGN.md`, "Art"), so
//! what looks right here is right in the game.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use lobshot_assets::{MobileRig, mobile_slug};
use lobshot_sim::{
    base::{Res, Void},
    body::Pose,
    drops::DropKind,
    game::{MobileKind, Shot},
    items::Item,
    movement::{Footing, Landing, settle, tilt},
    terrain::Terrain,
    units::{Facing, Vec2},
};
use resvg::tiny_skia::{Color, FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

use super::{
    ART_SCALE,
    compose::{Mobile, Sprite},
    icons, in_parallel,
    map::BuiltMap,
    out::Out,
    raster::encode_png,
};

/// The game's view, in world pixels (`DESIGN.md`, "Client").
const VIEW: (f32, f32) = (800.0, 600.0);

/// Writes every review sheet.
pub fn build_previews(sources: &Path, out: &Out, rigs: &[(MobileKind, MobileRig)], riders: &[String], maps: &[BuiltMap]) -> Void {
    let rider = riders.first().context("no riders to pose")?;
    for (mobile, rig) in rigs {
        let slug = mobile_slug(*mobile);
        out.write(&format!("{slug}.png"), &encode_png(&poses(&Mobile::render(sources, *mobile, rig, rider, 5.0)?, *mobile)?)?)?;
        out.write(&format!("{slug}_parts.png"), &encode_png(&parts_sheet(sources, *mobile, rig, riders)?)?)?;
    }
    out.write("items.png", &encode_png(&icons_sheet(sources)?)?)?;
    let (mobile, rig) = rigs.first().context("no mobiles to place")?;
    let at_game_scale = Mobile::render(sources, *mobile, rig, rider, ART_SCALE)?;
    let variants: Vec<_> = maps.iter().flat_map(|map| map.variants.iter().zip(&map.info.variants).map(move |pair| (map, pair))).collect();
    in_parallel(&variants, |&(map, (built, variant))| {
        let scene = Scene {
            background: &map.background,
            terrain: &built.terrain,
            mask: &built.mask,
            spawns: &variant.spawns,
            mobile: &at_game_scale,
            kind: *mobile,
        };
        let name = format!("{}_{}", map.info.name, variant.id);
        out.write(&format!("{name}.png"), &encode_png(&scene.whole()?)?)?;
        out.write(&format!("{name}_view.png"), &encode_png(&scene.view()?)?)?;
        out.write(&format!("{name}_mask.png"), &encode_png(&scene.mask()?)?)
    })?;
    Ok(())
}

/// The mobile at its aim range's ends and middle, plus 0 to show the cannon clears the body, on a
/// plain ground line; the last pose carries the gameplay geometry from `sim::tuning`.
fn poses(mobile: &Mobile, kind: MobileKind) -> Res<Pixmap> {
    let tuning = kind.tuning();
    let aim = tuning.s1.aim.value;
    let aims = [0.0, f64::from(aim.min), f64::from(aim.true_min + aim.true_max) / 2.0, f64::from(aim.max)];
    let (cell, ground, scale) = (110.0_f32, 104.0_f32, 5.0_f32);
    #[allow(clippy::cast_precision_loss)]
    let width = cell * aims.len() as f32;
    let mut canvas = canvas(width * scale, 130.0 * scale)?;
    canvas.fill(Color::from_rgba8(0xd9, 0xe6, 0xef, 0xff));
    let view = Transform::from_scale(scale, scale);
    fill_rect(&mut canvas, Rect::from_xywh(0.0, ground, width, 130.0 - ground), view, (0xb9, 0xc7, 0xd2));
    for (i, &aim) in aims.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let x = cell * (i as f32 + 0.5);
        let pose = Pose {
            foot: Vec2::new(f64::from(x), f64::from(ground)),
            facing: Facing::Right,
            tilt: 0.0,
        };
        mobile.draw(&mut canvas, view, &pose, aim);
        if i + 1 == aims.len() {
            draw_geometry(&mut canvas, view, &pose, kind, aim);
        }
    }
    Ok(canvas)
}

/// Hitbox (dashed red), each barrel's pivot (cyan) and muzzle (red), and ground probes (green).
fn draw_geometry(canvas: &mut Pixmap, view: Transform, pose: &Pose, kind: MobileKind, aim: f64) {
    let tuning = kind.tuning();
    let hitbox = tuning.hitbox.value;
    let corners = [
        (hitbox.x, hitbox.y),
        (hitbox.x + hitbox.w, hitbox.y),
        (hitbox.x + hitbox.w, hitbox.y + hitbox.h),
        (hitbox.x, hitbox.y + hitbox.h),
    ];
    let mut path = PathBuilder::new();
    for (i, &(x, y)) in corners.iter().enumerate() {
        let at = pose.to_world(Vec2::new(x, y));
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (at.x as f32, at.y as f32);
        if i == 0 { path.move_to(x, y) } else { path.line_to(x, y) }
    }
    path.close();
    if let Some(path) = path.finish() {
        let stroke = Stroke {
            width: 0.5,
            dash: resvg::tiny_skia::StrokeDash::new(vec![2.0, 1.0], 0.0),
            ..Stroke::default()
        };
        canvas.stroke_path(&path, &paint((0xff, 0x2d, 0x55)), &stroke, view, None);
    }
    #[allow(clippy::cast_possible_truncation)]
    let aim = aim.round() as i16;
    // Each barrel's pivot and muzzle: the cannon's, and a second barrel's if it has one.
    for shot in [Shot::S1, Shot::S2, Shot::Ss] {
        let (px, py) = tuning.barrel(shot).pivot;
        dot(canvas, view, pose.to_world(Vec2::new(px, py)), (0x00, 0xe0, 0xff));
        dot(canvas, view, pose.muzzle(tuning, shot, aim), (0xff, 0x2d, 0x55));
    }
    for probe in tuning.ground_probes.value {
        dot(canvas, view, pose.to_world(Vec2::new(probe, 0.0)), (0x7c, 0xff, 0x4f));
    }
}

/// Every part and rider on its own, at 4x, with its image bounds and its pivot (the image center).
fn parts_sheet(sources: &Path, mobile: MobileKind, rig: &MobileRig, riders: &[String]) -> Res<Pixmap> {
    let scale = 4.0_f32;
    let dir = sources.join("mobiles").join(mobile_slug(mobile));
    let mut sprites = Vec::new();
    let mut seen = Vec::new();
    for part in &rig.parts {
        if !seen.contains(&&part.file) {
            seen.push(&part.file);
            sprites.push(Sprite::render(&dir.join(&part.file), scale)?);
        }
    }
    for rider in riders {
        sprites.push(Sprite::render(&sources.join("riders").join(format!("{rider}.svg")), scale)?);
    }
    #[allow(clippy::cast_precision_loss)]
    let widths: Vec<f32> = sprites.iter().map(|s| s.pixmap.width() as f32).collect();
    #[allow(clippy::cast_precision_loss)]
    let tallest = sprites.iter().map(|s| s.pixmap.height()).max().unwrap_or(1) as f32;
    let gap = 24.0;
    let width = widths.iter().map(|w| w + gap).sum::<f32>() + gap;
    let mut canvas = canvas(width, tallest + 2.0 * gap)?;
    canvas.fill(Color::from_rgba8(0xe6, 0xed, 0xf3, 0xff));
    let mut x = gap;
    for (sprite, w) in sprites.iter().zip(&widths) {
        #[allow(clippy::cast_precision_loss)]
        let h = sprite.pixmap.height() as f32;
        let center = (x + w / 2.0, gap + tallest / 2.0);
        if let Some(bounds) = Rect::from_xywh(center.0 - w / 2.0, center.1 - h / 2.0, *w, h) {
            let stroke = Stroke {
                width: 1.0,
                dash: resvg::tiny_skia::StrokeDash::new(vec![4.0, 3.0], 0.0),
                ..Stroke::default()
            };
            canvas.stroke_path(&PathBuilder::from_rect(bounds), &paint((0x8a, 0xa0, 0xb4)), &stroke, Transform::identity(), None);
        }
        sprite.draw(&mut canvas, Transform::from_translate(center.0, center.1).pre_scale(scale, scale));
        let mut cross = PathBuilder::new();
        cross.move_to(center.0 - 8.0, center.1);
        cross.line_to(center.0 + 8.0, center.1);
        cross.move_to(center.0, center.1 - 8.0);
        cross.line_to(center.0, center.1 + 8.0);
        if let Some(cross) = cross.finish() {
            canvas.stroke_path(&cross, &paint((0xff, 0x2d, 0x55)), &Stroke { width: 1.5, ..Stroke::default() }, Transform::identity(), None);
        }
        x += w + gap;
    }
    Ok(canvas)
}

/// Every item icon, then every drop, in a row: at 1x and at game scale on a dark panel like the
/// HUD's, and at game scale on a light one.
fn icons_sheet(sources: &Path) -> Res<Pixmap> {
    let paths: Vec<PathBuf> = Item::ALL
        .iter()
        .map(|&item| icons::item_source(sources, item))
        .chain(DropKind::ALL.iter().map(|&drop| icons::drop_source(sources, drop)))
        .collect();
    let bands = [(1.0, (0x2a, 0x24, 0x38)), (ART_SCALE, (0x3a, 0x34, 0x50)), (ART_SCALE, (0xe6, 0xed, 0xf3))];
    let mut rows = Vec::with_capacity(bands.len());
    for (scale, color) in bands {
        let sprites = paths.iter().map(|path| Sprite::render(path, scale)).collect::<Res<Vec<_>>>()?;
        rows.push((scale, color, sprites));
    }
    #[allow(clippy::cast_precision_loss)] // Sprite sides are far below 2^24.
    let side = |n: u32| n as f32;
    let gap = 12.0;
    let width = rows
        .iter()
        .map(|(_, _, sprites)| sprites.iter().map(|s| side(s.pixmap.width()) + gap).sum::<f32>() + gap)
        .fold(0.0, f32::max);
    let heights: Vec<f32> = rows
        .iter()
        .map(|(_, _, sprites)| side(sprites.iter().map(|s| s.pixmap.height()).max().unwrap_or(1)) + 2.0 * gap)
        .collect();
    let mut canvas = canvas(width, heights.iter().sum())?;
    let mut top = 0.0;
    for ((scale, color, sprites), &height) in rows.iter().zip(&heights) {
        fill_rect(&mut canvas, Rect::from_xywh(0.0, top, width, height), Transform::identity(), *color);
        let mut x = gap;
        for sprite in sprites {
            let w = side(sprite.pixmap.width());
            sprite.draw(&mut canvas, Transform::from_translate(x + w / 2.0, top + height / 2.0).pre_scale(*scale, *scale));
            x += w + gap;
        }
        top += height;
    }
    Ok(canvas)
}

/// A map variant with mobiles standing on every spawn, as the sim would settle them.
struct Scene<'a> {
    background: &'a Pixmap,
    terrain: &'a Pixmap,
    mask: &'a Terrain,
    spawns: &'a [i32],
    mobile: &'a Mobile,
    kind: MobileKind,
}

impl Scene<'_> {
    /// The whole map at one pixel per world pixel.
    fn whole(&self) -> Res<Pixmap> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.mask.width() as f32, self.mask.height() as f32);
        self.draw((0.0, 0.0), (w, h), 1.0, self.spawns)
    }

    /// The game's 800x600 view at game scale, around the two spawns nearest the map's middle.
    fn view(&self) -> Res<Pixmap> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.mask.width() as f32, self.mask.height() as f32);
        let mut spawns = self.spawns.to_vec();
        spawns.sort_by_key(|&x| (2 * x - i32::try_from(self.mask.width()).unwrap_or(0)).abs());
        spawns.truncate(2);
        #[allow(clippy::cast_precision_loss)]
        let middle = spawns.iter().sum::<i32>() as f32 / spawns.len().max(1) as f32;
        let ground = spawns.iter().filter_map(|&x| self.stand(x)).map(|pose| pose.foot.y).fold(0.0, f64::max);
        #[allow(clippy::cast_possible_truncation)]
        let ground = ground as f32;
        let left = (middle - VIEW.0 / 2.0).clamp(0.0, w - VIEW.0);
        let top = (ground - VIEW.1 * 0.62).clamp(0.0, h - VIEW.1);
        self.draw((left, top), VIEW, ART_SCALE, &spawns)
    }

    fn draw(&self, origin: (f32, f32), size: (f32, f32), scale: f32, spawns: &[i32]) -> Res<Pixmap> {
        let mut canvas = canvas(size.0 * scale, size.1 * scale)?;
        let view = Transform::from_scale(scale, scale).pre_translate(-origin.0, -origin.1);
        // The map art as the client draws it: the rendered images, scaled into the world.
        let to_world = view.pre_scale(1.0 / ART_SCALE, 1.0 / ART_SCALE);
        let paint = PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        };
        canvas.draw_pixmap(0, 0, self.background.as_ref(), &paint, to_world, None);
        canvas.draw_pixmap(0, 0, self.terrain.as_ref(), &paint, to_world, None);
        #[allow(clippy::cast_precision_loss)]
        let middle = self.mask.width() as f32 / 2.0;
        for (i, &x) in spawns.iter().enumerate() {
            let Some(mut pose) = self.stand(x) else { continue };
            #[allow(clippy::cast_precision_loss)]
            let facing_right = (x as f32) < middle;
            pose.facing = if facing_right { Facing::Right } else { Facing::Left };
            // A spread of aims, so a sheet shows the cannon's range.
            let aim = [20.0, 35.0, 50.0][i % 3];
            self.mobile.draw(&mut canvas, view, &pose, aim);
        }
        Ok(canvas)
    }

    /// Where a mobile dropped in column `x` comes to rest, leaning with the ground.
    fn stand(&self, x: i32) -> Option<Pose> {
        let Landing::On(foot) = settle(self.mask, Footing { x, y: 0 }) else { return None };
        let lean = tilt(self.mask, foot, self.kind.tuning().ground_probes.value);
        Some(Pose {
            foot: foot.world(),
            facing: Facing::Right,
            tilt: lean,
        })
    }

    /// The collision mask: solid dark, air light, each spawn column marked.
    fn mask(&self) -> Res<Pixmap> {
        let (w, h) = (self.mask.width(), self.mask.height());
        let mut canvas = Pixmap::new(w, h).context("a mask image")?;
        for (i, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let solid = self.mask.is_solid(x as i32, y as i32);
            let shade = if solid { 0x2a } else { 0xf2 };
            *pixel = resvg::tiny_skia::ColorU8::from_rgba(shade, shade, shade.saturating_add(8), 0xff).premultiply();
        }
        for &x in self.spawns {
            #[allow(clippy::cast_precision_loss)]
            if let Some(line) = Rect::from_xywh(x as f32 - 1.0, 0.0, 2.0, h as f32) {
                canvas.fill_rect(line, &paint((0xff, 0x2d, 0x55)), Transform::identity(), None);
            }
        }
        Ok(canvas)
    }
}

fn canvas(w: f32, h: f32) -> Res<Pixmap> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (w, h) = (w.ceil() as u32, h.ceil() as u32);
    Pixmap::new(w, h).with_context(|| format!("a {w}x{h} sheet"))
}

fn paint((r, g, b): (u8, u8, u8)) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 0xff);
    paint.anti_alias = true;
    paint
}

fn fill_rect(canvas: &mut Pixmap, rect: Option<Rect>, view: Transform, color: (u8, u8, u8)) {
    if let Some(rect) = rect {
        canvas.fill_rect(rect, &paint(color), view, None);
    }
}

fn dot(canvas: &mut Pixmap, view: Transform, at: Vec2, color: (u8, u8, u8)) {
    #[allow(clippy::cast_possible_truncation)]
    if let Some(circle) = PathBuilder::from_circle(at.x as f32, at.y as f32, 1.2) {
        canvas.fill_path(&circle, &paint(color), FillRule::Winding, view, None);
    }
}
