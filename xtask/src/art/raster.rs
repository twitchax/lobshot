//! SVG to pixels. Every source goes through here, so the scale and the placement rules (a sprite's
//! origin at its center, a map's viewBox corner at its top-left) live in one place.

use std::path::Path;

use anyhow::{Context as _, bail, ensure};
use lobshot_sim::base::Res;
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};

/// A parsed SVG and its viewBox, in user units (one user unit is one world pixel).
pub struct Svg {
    tree: usvg::Tree,
    view: ViewBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl ViewBox {
    /// Whether the viewBox is `size` world pixels.
    pub fn matches(self, (width, height): (u32, u32)) -> bool {
        (f64::from(self.w) - f64::from(width)).abs() < 1e-3 && (f64::from(self.h) - f64::from(height)).abs() < 1e-3
    }
}

impl Svg {
    pub fn load(path: &Path) -> Res<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Parses SVG text. Sources draw in world pixels, so `width` and `height` must equal the
    /// viewBox's size: anything else would silently rescale the art against the rig and the mask.
    pub fn parse(text: &str) -> Res<Self> {
        let doc = usvg::roxmltree::Document::parse(text).context("malformed XML")?;
        let root = doc.root_element();
        let view = root.attribute("viewBox").context("the root <svg> has no viewBox")?;
        let numbers: Vec<f32> = view
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .context("reading the viewBox")?;
        let &[x, y, w, h] = numbers.as_slice() else {
            bail!("a viewBox needs four numbers, not {view:?}");
        };
        ensure!(w > 0.0 && h > 0.0, "the viewBox {view:?} is empty");
        for (name, expected) in [("width", w), ("height", h)] {
            let value: f32 = root
                .attribute(name)
                .with_context(|| format!("the root <svg> has no {name}"))?
                .parse()
                .with_context(|| format!("reading {name}"))?;
            ensure!(
                (value - expected).abs() < 1e-3,
                "{name} {value} differs from the viewBox's {expected}: one user unit must be one world pixel"
            );
        }

        let tree = usvg::Tree::from_str(text, &usvg::Options::default()).context("converting the SVG")?;
        Ok(Self { tree, view: ViewBox { x, y, w, h } })
    }

    pub fn view(&self) -> ViewBox {
        self.view
    }

    /// Renders at `scale` pixels per user unit with the SVG's origin at the image's center (a
    /// part's pivot, a rider's seat), so a sprite anchored at its center lands where the rig says.
    /// The image grows to hold the viewBox on both sides of the origin; its sides are even, so the
    /// center falls on a pixel corner, exactly.
    pub fn render_centered(&self, scale: f32) -> Res<Pixmap> {
        let ViewBox { x, y, w, h } = self.view;
        let half_w = (-x).max(x + w);
        let half_h = (-y).max(y + h);
        ensure!(half_w > 0.0 && half_h > 0.0, "the viewBox lies on an axis, so it has no extent around its origin");
        let (width, height) = (2 * pixels(half_w * scale)?, 2 * pixels(half_h * scale)?);
        // usvg maps the viewBox's corner to (0, 0); undo that, then put the origin at the center.
        #[allow(clippy::cast_precision_loss)] // Image sides are far below 2^24.
        let transform = Transform::from_translate(width as f32 / 2.0, height as f32 / 2.0).pre_scale(scale, scale).pre_translate(x, y);
        self.render(width, height, transform)
    }

    /// Renders the viewBox itself at `scale`: a map, whose world origin is its top-left corner.
    pub fn render_view(&self, scale: f32) -> Res<Pixmap> {
        let (width, height) = (pixels(self.view.w * scale)?, pixels(self.view.h * scale)?);
        self.render(width, height, Transform::from_scale(scale, scale))
    }

    fn render(&self, width: u32, height: u32, transform: Transform) -> Res<Pixmap> {
        let mut pixmap = Pixmap::new(width, height).with_context(|| format!("a {width}x{height} image"))?;
        resvg::render(&self.tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }
}

/// A length in pixels, rounded up to a whole pixel.
fn pixels(length: f32) -> Res<u32> {
    ensure!(length.is_finite() && length > 0.0 && length < 16384.0, "{length} px is not a usable image side");
    // Checked above: positive and well inside u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rounded = length.ceil() as u32;
    Ok(rounded)
}

/// Encodes a pixmap as an 8-bit RGBA PNG at the png crate's highest compression. The encoder is
/// deterministic, so the same pixels give the same bytes.
pub fn encode_png(pixmap: &Pixmap) -> Res<Vec<u8>> {
    let rgba = pixmap.clone().take_demultiplied();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().context("writing a PNG header")?;
    writer.write_image_data(&rgba).context("writing PNG data")?;
    writer.finish().context("finishing a PNG")?;
    Ok(bytes)
}

/// The alpha channel, one byte per pixel, row-major.
pub fn alpha(pixmap: &Pixmap) -> Vec<u8> {
    pixmap.pixels().iter().map(|p| p.alpha()).collect()
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    /// The bounding box of pixels with any coverage: (left, top, right, bottom), exclusive.
    pub fn opaque_bounds(pixmap: &Pixmap) -> (u32, u32, u32, u32) {
        let alpha = alpha(pixmap);
        let w = pixmap.width();
        let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
        for (i, &a) in alpha.iter().enumerate() {
            if a > 0 {
                let (x, y) = (u32::try_from(i).unwrap() % w, u32::try_from(i).unwrap() / w);
                (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1));
            }
        }
        (left, top, right, bottom)
    }

    #[test]
    fn centered_renders_put_the_origin_at_the_center_of_the_image() {
        // A disc around the origin in a viewBox that is far from centered on it.
        let svg = Svg::parse(r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="24" viewBox="-4 -20 40 24"><circle r="3" fill="#000"/></svg>"##).unwrap();
        let pixmap = svg.render_centered(2.0).unwrap();
        // x spans -4..36, so the image holds -36..36; y spans -20..4, so it holds -20..20.
        assert_eq!((pixmap.width(), pixmap.height()), (144, 80));
        let (left, top, right, bottom) = opaque_bounds(&pixmap);
        assert_eq!((left + right, top + bottom), (144, 80), "the disc is centered");
        assert_eq!(right - left, 12, "3 units of radius at 2x");
    }

    #[test]
    fn view_renders_map_the_viewbox_corner_to_the_top_left() {
        let svg = Svg::parse(r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="6" viewBox="0 0 10 6"><rect x="2" y="1" width="3" height="2" fill="#000"/></svg>"##).unwrap();
        let pixmap = svg.render_view(2.0).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (20, 12));
        assert_eq!(opaque_bounds(&pixmap), (4, 2, 10, 6));
    }

    #[test]
    fn rejects_sources_that_would_rescale_the_art() {
        let scaled = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="12" viewBox="0 0 10 6"/>"#;
        assert!(Svg::parse(scaled).is_err());
        assert!(Svg::parse(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="6"/>"#).is_err(), "no viewBox");
        assert!(Svg::parse(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="6" viewBox="0 0 10"/>"#).is_err());
    }

    #[test]
    fn pngs_round_trip_and_are_deterministic() {
        let svg = Svg::parse(r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="6" viewBox="-5 -3 10 6"><circle r="2" fill="#c04" opacity="0.5"/></svg>"##).unwrap();
        let pixmap = svg.render_centered(3.0).unwrap();
        let png = encode_png(&pixmap).unwrap();
        assert_eq!(png, encode_png(&svg.render_centered(3.0).unwrap()).unwrap());
        let decoded = Pixmap::decode_png(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (pixmap.width(), pixmap.height()));
        assert_eq!(alpha(&decoded), alpha(&pixmap));
    }
}
