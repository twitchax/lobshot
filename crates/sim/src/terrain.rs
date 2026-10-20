//! Terrain as a per-pixel mask, not a physics body: explosions carve it, collision samples it.
//! Anything outside the map is air, so a mobile whose ground is gone falls out the bottom.

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use crate::{base::Res, hash::fnv1a, units::Vec2};

/// The largest map side we accept, in pixels. GunBound maps reach about 1850 px (OB).
const MAX_SIDE: u32 = 8192;

/// The mask file's magic bytes and version (`mask.bin`, written by `cargo xtask art`).
const MASK_MAGIC: &[u8; 4] = b"LSMK";
const MASK_VERSION: u8 = 1;

/// A rectangle of pixels that changed, for the client to re-upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A solid/air mask, one bit per pixel, row-major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terrain {
    width: u32,
    height: u32,
    words_per_row: u32,
    bits: Vec<u64>,
}

impl Terrain {
    /// A mask where `solid(x, y)` decides each pixel.
    ///
    /// # Errors
    ///
    /// Fails when a side is zero or larger than 8192.
    pub fn from_fn(width: u32, height: u32, solid: impl Fn(u32, u32) -> bool) -> Res<Self> {
        ensure!((1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height), "terrain {width}x{height} is out of range");
        let words_per_row = width.div_ceil(64);
        let mut terrain = Self {
            width,
            height,
            words_per_row,
            bits: vec![0; (words_per_row * height) as usize],
        };
        for y in 0..height {
            for x in 0..width {
                if solid(x, y) {
                    let (word, bit) = terrain.index(x, y);
                    terrain.bits[word] |= bit;
                }
            }
        }
        Ok(terrain)
    }

    /// A mask from an image's alpha channel (one byte per pixel, row-major): solid where alpha > 0.
    ///
    /// # Errors
    ///
    /// Fails when the size is out of range or `alpha` is not `width * height` bytes.
    pub fn from_alpha(width: u32, height: u32, alpha: &[u8]) -> Res<Self> {
        ensure!(alpha.len() == width as usize * height as usize, "alpha has {} bytes for a {width}x{height} mask", alpha.len());
        Self::from_fn(width, height, |x, y| alpha[(y * width + x) as usize] > 0)
    }

    /// The mask file format: `LSMK`, a version byte, width and height as little-endian u32, then
    /// each row's bits, least significant bit first, padded to whole bytes.
    #[must_use]
    pub fn encode_mask(&self) -> Vec<u8> {
        let row_bytes = self.width.div_ceil(8) as usize;
        let mut bytes = Vec::with_capacity(13 + row_bytes * self.height as usize);
        bytes.extend_from_slice(MASK_MAGIC);
        bytes.push(MASK_VERSION);
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        for y in 0..self.height {
            let row = &self.bits[(y * self.words_per_row) as usize..((y + 1) * self.words_per_row) as usize];
            bytes.extend(row.iter().flat_map(|word| word.to_le_bytes()).take(row_bytes));
        }
        bytes
    }

    /// Reads the mask file format.
    ///
    /// # Errors
    ///
    /// Fails on the wrong magic or version, an out-of-range size, or a short file.
    pub fn decode_mask(bytes: &[u8]) -> Res<Self> {
        let header = bytes.get(..13).context("a mask needs a 13-byte header")?;
        ensure!(&header[..4] == MASK_MAGIC, "not a lobshot mask");
        ensure!(header[4] == MASK_VERSION, "mask version {}, but this build reads {MASK_VERSION}", header[4]);
        let read = |at: usize| u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]);
        let (width, height) = (read(5), read(9));
        let row_bytes = width.div_ceil(8) as usize;
        let body = &bytes[13..];
        ensure!(
            body.len() == row_bytes * height as usize,
            "a {width}x{height} mask needs {} bytes of rows, not {}",
            row_bytes * height as usize,
            body.len()
        );
        Self::from_fn(width, height, |x, y| body[y as usize * row_bytes + x as usize / 8] & (1 << (x % 8)) != 0)
    }

    /// A hash of the mask file form, so a game can name the exact terrain it was set up on.
    #[must_use]
    pub fn mask_hash(&self) -> u64 {
        fnv1a(&self.encode_mask())
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    fn index(&self, x: u32, y: u32) -> (usize, u64) {
        ((y * self.words_per_row + x / 64) as usize, 1 << (x % 64))
    }

    /// Whether the pixel at (x, y) is solid. Outside the map is air.
    #[must_use]
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return false;
        };
        if x >= self.width || y >= self.height {
            return false;
        }
        let (word, bit) = self.index(x, y);
        self.bits[word] & bit != 0
    }

    /// Whether row `y` of column `x` is a surface: solid, with air directly above.
    #[must_use]
    pub fn is_surface(&self, x: i32, y: i32) -> bool {
        self.is_solid(x, y) && !self.is_solid(x, y - 1)
    }

    /// The first surface in column `x` from row `from` down to the bottom, if any.
    #[must_use]
    pub fn surface_at_or_below(&self, x: i32, from: i32) -> Option<i32> {
        let bottom = i32::try_from(self.height).unwrap_or(i32::MAX);
        (from.max(0)..bottom).find(|&y| self.is_surface(x, y))
    }

    /// Clears every pixel whose center lies within `radius` of `center`, and returns the rectangle
    /// that changed (`None` when nothing did).
    pub fn carve(&mut self, center: Vec2, radius: f64) -> Option<DirtyRect> {
        if radius <= 0.0 {
            return None;
        }
        let clamp = |v: f64, side: u32| -> u32 {
            // Clamped into 0..=side first, so the cast is exact.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = v.clamp(0.0, f64::from(side)) as u32;
            v
        };
        let (x0, x1) = (clamp((center.x - radius).floor(), self.width), clamp((center.x + radius).ceil(), self.width));
        let (y0, y1) = (clamp((center.y - radius).floor(), self.height), clamp((center.y + radius).ceil(), self.height));

        let mut changed: Option<(u32, u32, u32, u32)> = None;
        for y in y0..y1 {
            let dy = f64::from(y) + 0.5 - center.y;
            for x in x0..x1 {
                let dx = f64::from(x) + 0.5 - center.x;
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let (word, bit) = self.index(x, y);
                if self.bits[word] & bit != 0 {
                    self.bits[word] &= !bit;
                    changed = Some(match changed {
                        None => (x, y, x, y),
                        Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x), by.max(y)),
                    });
                }
            }
        }
        changed.map(|(ax, ay, bx, by)| DirtyRect {
            x: ax,
            y: ay,
            w: bx - ax + 1,
            h: by - ay + 1,
        })
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    fn flat(width: u32, height: u32, ground: u32) -> Terrain {
        Terrain::from_fn(width, height, |_, y| y >= ground).unwrap()
    }

    #[test]
    fn samples_solid_and_treats_outside_as_air() {
        let terrain = flat(100, 50, 30);
        assert!(terrain.is_solid(0, 30) && terrain.is_solid(99, 49));
        assert!(!terrain.is_solid(50, 29));
        assert!(!terrain.is_solid(-1, 40) && !terrain.is_solid(100, 40) && !terrain.is_solid(10, 50) && !terrain.is_solid(10, -5));
    }

    #[test]
    fn widths_that_are_not_a_multiple_of_64_keep_columns_apart() {
        let terrain = Terrain::from_fn(70, 3, |x, _| x == 64 || x == 69).unwrap();
        let solid: Vec<i32> = (0..70).filter(|&x| terrain.is_solid(x, 1)).collect();
        assert_eq!(solid, [64, 69]);
    }

    #[test]
    fn finds_surfaces() {
        let terrain = flat(10, 20, 12);
        assert_eq!(terrain.surface_at_or_below(3, 0), Some(12));
        assert_eq!(terrain.surface_at_or_below(3, 13), None);
        assert!(terrain.is_surface(3, 12) && !terrain.is_surface(3, 13));
    }

    #[test]
    fn carves_a_disc_and_reports_what_changed() {
        let mut terrain = flat(100, 100, 50);
        let dirty = terrain.carve(Vec2::new(50.0, 50.0), 10.0).unwrap();
        assert!(!terrain.is_solid(50, 55) && !terrain.is_solid(50, 59));
        assert!(terrain.is_solid(50, 61) && terrain.is_solid(61, 51));
        // Only the lower half was solid, so only it changed.
        assert_eq!(dirty, DirtyRect { x: 40, y: 50, w: 20, h: 10 });
        assert_eq!(terrain.carve(Vec2::new(50.0, 20.0), 10.0), None, "carving air changes nothing");
    }

    #[test]
    fn carving_at_the_map_edge_stays_in_bounds() {
        let mut terrain = flat(20, 20, 0);
        let dirty = terrain.carve(Vec2::new(0.0, 0.0), 5.0).unwrap();
        assert_eq!((dirty.x, dirty.y), (0, 0));
        assert!(!terrain.is_solid(0, 0) && terrain.is_solid(19, 19));
    }

    #[test]
    fn the_mask_format_round_trips_and_rejects_damage() {
        let terrain = Terrain::from_fn(70, 5, |x, y| (x * 7 + y * 3) % 5 == 0).unwrap();
        let bytes = terrain.encode_mask();
        assert_eq!(bytes.len(), 13 + 9 * 5, "rows pad to whole bytes");
        assert_eq!(Terrain::decode_mask(&bytes).unwrap(), terrain);
        assert_eq!(terrain.mask_hash(), fnv1a(&bytes));

        assert!(Terrain::decode_mask(&bytes[..20]).is_err(), "short");
        let mut wrong = bytes.clone();
        wrong[0] = b'X';
        assert!(Terrain::decode_mask(&wrong).is_err(), "magic");
        let mut wrong = bytes;
        wrong[4] = MASK_VERSION + 1;
        assert!(Terrain::decode_mask(&wrong).is_err(), "version");
    }

    #[test]
    fn reads_alpha_masks() {
        let terrain = Terrain::from_alpha(2, 2, &[0, 255, 1, 0]).unwrap();
        assert!(!terrain.is_solid(0, 0) && terrain.is_solid(1, 0) && terrain.is_solid(0, 1));
        assert!(Terrain::from_alpha(2, 2, &[0; 3]).is_err());
        assert!(Terrain::from_fn(0, 5, |_, _| true).is_err());
    }
}
