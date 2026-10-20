//! Composing a mobile from its pivot-centered sprites, the way the client does: each part's image
//! center on its `at`, turned by aim or spin, the whole body leaned and mirrored by its pose. The
//! review sheets and the art tests both pose mobiles through here.

use std::{collections::BTreeMap, path::Path};

use lobshot_assets::{MobileRig, mobile_slug};
use lobshot_sim::{base::Res, body::Pose, game::MobileKind};
use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

use super::raster::Svg;

/// An image whose center is its pivot, drawn at `scale` pixels per world pixel.
pub struct Sprite {
    pub pixmap: Pixmap,
    pub scale: f32,
}

impl Sprite {
    pub fn render(path: &Path, scale: f32) -> Res<Self> {
        Ok(Self {
            pixmap: Svg::load(path)?.render_centered(scale)?,
            scale,
        })
    }

    /// Draws with the pivot at `at`'s origin.
    pub fn draw(&self, canvas: &mut Pixmap, at: Transform) {
        #[allow(clippy::cast_precision_loss)] // Sprite sides are far below 2^24.
        let (w, h) = (self.pixmap.width() as f32, self.pixmap.height() as f32);
        let transform = at.pre_scale(1.0 / self.scale, 1.0 / self.scale).pre_translate(-w / 2.0, -h / 2.0);
        let paint = PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        };
        canvas.draw_pixmap(0, 0, self.pixmap.as_ref(), &paint, transform, None);
    }
}

/// A mobile's parts and a rider, ready to pose.
pub struct Mobile {
    rig: MobileRig,
    parts: BTreeMap<String, Sprite>,
    rider: Sprite,
}

impl Mobile {
    pub fn render(sources: &Path, mobile: MobileKind, rig: &MobileRig, rider: &str, scale: f32) -> Res<Self> {
        let dir = sources.join("mobiles").join(mobile_slug(mobile));
        let mut parts = BTreeMap::new();
        for part in &rig.parts {
            if !parts.contains_key(&part.file) {
                parts.insert(part.file.clone(), Sprite::render(&dir.join(&part.file), scale)?);
            }
        }
        let rider = Sprite::render(&sources.join("riders").join(format!("{rider}.svg")), scale)?;
        Ok(Self { rig: rig.clone(), parts, rider })
    }

    /// Draws the mobile standing at `pose` with its cannon at `aim` degrees, through `view` (world
    /// to canvas). The body frame maps to the world as `sim::body::Pose::to_world` does.
    pub fn draw(&self, canvas: &mut Pixmap, view: Transform, pose: &Pose, aim: f64) {
        self.draw_only(canvas, view, pose, aim, |_| true);
    }

    /// Draws only the parts whose ids `keep` accepts; the rider's id is `rider`.
    pub fn draw_only(&self, canvas: &mut Pixmap, view: Transform, pose: &Pose, aim: f64, keep: impl Fn(&str) -> bool) {
        let (sin, cos) = (libm::sin(pose.tilt), libm::cos(pose.tilt));
        #[allow(clippy::cast_possible_truncation)] // Preview geometry: f32 is plenty.
        let body = view
            .pre_concat(Transform::from_row(cos as f32, -sin as f32, sin as f32, cos as f32, pose.foot.x as f32, pose.foot.y as f32))
            .pre_scale(pose.facing.signum() as f32, 1.0);
        let mut layers: Vec<Layer> = self
            .rig
            .parts
            .iter()
            .filter(|part| keep(&part.id))
            .filter_map(|part| {
                Some(Layer {
                    z: part.z,
                    sprite: self.parts.get(&part.file)?,
                    at: part.at,
                    turn: if part.aim { -aim } else { 0.0 },
                })
            })
            .collect();
        if keep("rider") {
            layers.push(Layer {
                z: self.rig.rider_socket.z,
                sprite: &self.rider,
                at: self.rig.rider_socket.at,
                turn: 0.0,
            });
        }
        // Stable, so parts on one z keep the rig's order.
        layers.sort_by_key(|layer| layer.z);
        for layer in layers {
            #[allow(clippy::cast_possible_truncation)]
            layer.sprite.draw(canvas, body.pre_translate(layer.at.0 as f32, layer.at.1 as f32).pre_rotate(layer.turn as f32));
        }
    }
}

/// One sprite of a posed mobile.
struct Layer<'a> {
    z: i32,
    sprite: &'a Sprite,
    /// The pivot in the body frame.
    at: (f64, f64),
    /// Degrees, clockwise on screen.
    turn: f64,
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use lobshot_sim::units::{Facing, Vec2};

    use super::*;
    use crate::art::{mobile::load_rig, raster::alpha, sources};

    /// The spike's cannon covered the snail's antennae at aim 0. Past its ball joint, the barrel may
    /// not cross anything else on the mobile, from 0 up to the highest aim any shot allows.
    #[test]
    fn the_cannon_clears_the_body_at_every_aim() {
        // The ball joint and the turret cup it sits in overlap the barrel by design.
        const JOINT: f64 = 11.0;
        for kind in MobileKind::ALL {
            let rig = load_rig(&sources(), kind).unwrap();
            let mobile = Mobile::render(&sources(), kind, &rig, "pip", 2.0).unwrap();
            let tuning = kind.tuning();
            let top = [tuning.s1.aim, tuning.s2.aim, tuning.ss.aim].iter().map(|aim| aim.value.max).max().unwrap();
            let pose = Pose {
                foot: Vec2::new(100.0, 140.0),
                facing: Facing::Right,
                tilt: 0.0,
            };
            let pivot = pose.to_world(Vec2::new(tuning.cannon_pivot.value.0, tuning.cannon_pivot.value.1));
            let view = Transform::from_scale(2.0, 2.0);
            for aim in (0..=top).step_by(5) {
                let aim = f64::from(aim);
                let mut cannon = Pixmap::new(400, 300).unwrap();
                mobile.draw_only(&mut cannon, view, &pose, aim, |id| id == rig.cannon.part);
                let mut rest = Pixmap::new(400, 300).unwrap();
                mobile.draw_only(&mut rest, view, &pose, aim, |id| id != rig.cannon.part && id != "rider");
                let crossed = alpha(&cannon)
                    .iter()
                    .zip(alpha(&rest))
                    .enumerate()
                    .filter(|&(i, (&a, b))| {
                        // Pixel centers, back in world pixels (the canvas is at 2x).
                        let pixel = |n: usize| f64::from(u32::try_from(n).unwrap()) * 0.5 + 0.25;
                        let world = Vec2::new(pixel(i % 400), pixel(i / 400));
                        a > 0 && b > 0 && world.distance(pivot) > JOINT
                    })
                    .count();
                assert_eq!(crossed, 0, "{} at aim {aim}: the barrel crosses {crossed} px of the body", rig.name);
            }
        }
    }
}
