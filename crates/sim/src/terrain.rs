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

/// The side of a block in a terrain's block map (`Terrain::any_solid`), in pixels. A divisor of
/// 64, so a block's columns sit in one word of a mask row.
const BLOCK: u32 = 8;

/// A solid/air mask, one bit per pixel, row-major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Mask")]
pub struct Terrain {
    width: u32,
    height: u32,
    words_per_row: u32,
    bits: Vec<u64>,
    /// One bit per `BLOCK`-px square, row-major, set when any pixel in it is solid, so a flight
    /// through open sky can be ruled clear a block at a time instead of a pixel at a time. It is
    /// `bits` summarized, kept exact as they change: left out of the wire form (and so the state
    /// hash), and rebuilt on reading.
    #[serde(skip_serializing)]
    blocks: Vec<u64>,
}

/// A terrain's wire form: everything but its block map.
#[derive(Deserialize)]
struct Mask {
    width: u32,
    height: u32,
    words_per_row: u32,
    bits: Vec<u64>,
}

impl From<Mask> for Terrain {
    fn from(Mask { width, height, words_per_row, bits }: Mask) -> Self {
        let mut terrain = Self {
            width,
            height,
            words_per_row,
            bits,
            blocks: Vec::new(),
        };
        terrain.summarize();
        terrain
    }
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
            blocks: Vec::new(),
        };
        for y in 0..height {
            for x in 0..width {
                if solid(x, y) {
                    let (word, bit) = terrain.index(x, y);
                    terrain.bits[word] |= bit;
                }
            }
        }
        terrain.summarize();
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

    /// Blocks across a row of the block map, and the words that row takes.
    fn block_layout(&self) -> (u32, u32) {
        let across = self.width.div_ceil(BLOCK);
        (across, across.div_ceil(64))
    }

    /// Builds the block map from the mask.
    fn summarize(&mut self) {
        let (across, words) = self.block_layout();
        self.blocks = vec![0; (words * self.height.div_ceil(BLOCK)) as usize];
        self.resummarize(0, 0, across.saturating_sub(1), self.height.div_ceil(BLOCK).saturating_sub(1));
    }

    /// Recomputes blocks `bx0..=bx1` across and `by0..=by1` down from the mask.
    fn resummarize(&mut self, bx0: u32, by0: u32, bx1: u32, by1: u32) {
        let (_, words) = self.block_layout();
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let shift = (bx * BLOCK) % 64;
                let rows = by * BLOCK..((by + 1) * BLOCK).min(self.height);
                let solid = rows.into_iter().any(|y| (self.bits[(y * self.words_per_row + bx * BLOCK / 64) as usize] >> shift) & 0xFF != 0);
                let (word, bit) = ((by * words + bx / 64) as usize, 1_u64 << (bx % 64));
                if solid {
                    self.blocks[word] |= bit;
                } else {
                    self.blocks[word] &= !bit;
                }
            }
        }
    }

    /// Whether any pixel from (x0, y0) to (x1, y1), corners included, is solid. Outside the map is
    /// air. The block map answers first, so a stretch of open sky costs a few blocks; a small
    /// rectangle the blocks can't rule out is read pixel by pixel. A large one they can't is
    /// answered `true` unread: `false` is always certain, `true` only for a small one.
    #[must_use]
    pub fn any_solid(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        /// The most pixels it reads one by one.
        const READ: u32 = 256;
        let (Ok(width), Ok(height)) = (i32::try_from(self.width), i32::try_from(self.height)) else {
            return true;
        };
        if x1 < 0 || y1 < 0 || x0 >= width || y0 >= height || x0 > x1 || y0 > y1 {
            return false;
        }
        let clamp = |v: i32, side: u32| u32::try_from(v.max(0)).unwrap_or(0).min(side - 1);
        let (x0, x1, y0, y1) = (clamp(x0, self.width), clamp(x1, self.width), clamp(y0, self.height), clamp(y1, self.height));
        let (_, words) = self.block_layout();
        let flagged = (y0 / BLOCK..=y1 / BLOCK).any(|by| (x0 / BLOCK..=x1 / BLOCK).any(|bx| self.blocks[(by * words + bx / 64) as usize] & (1 << (bx % 64)) != 0));
        if !flagged {
            return false;
        }
        if (x1 - x0 + 1) * (y1 - y0 + 1) > READ {
            return true;
        }
        (y0..=y1).any(|y| {
            (x0..=x1).any(|x| {
                let (word, bit) = self.index(x, y);
                self.bits[word] & bit != 0
            })
        })
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
        let (ax, ay, bx, by) = changed?;
        self.resummarize(ax / BLOCK, ay / BLOCK, bx / BLOCK, by / BLOCK);
        Some(DirtyRect {
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

    /// Whether any pixel of the rectangle is solid, one by one.
    fn brute_any(terrain: &Terrain, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        (y0..=y1).any(|y| (x0..=x1).any(|x| terrain.is_solid(x, y)))
    }

    #[test]
    fn the_block_map_never_misses_a_solid_pixel_and_follows_carving() {
        // Specks, so most blocks are mixed, on a width that isn't a multiple of a block or a word.
        let mut terrain = Terrain::from_fn(203, 77, |x, y| (x * 31 + y * 17) % 97 == 0 || y >= 70).unwrap();
        let check = |terrain: &Terrain| {
            for (x0, y0) in [(-5, -5), (0, 0), (7, 3), (60, 40), (130, 60), (198, 72)] {
                for (w, h) in [(0, 0), (1, 1), (5, 9), (17, 3), (40, 40)] {
                    let (x1, y1) = (x0 + w, y0 + h);
                    let any = brute_any(terrain, x0, y0, x1, y1);
                    // Small rectangles are read exactly; large ones may only err toward solid.
                    if any || (w + 1) * (h + 1) <= 256 {
                        assert_eq!(terrain.any_solid(x0, y0, x1, y1), any, "({x0}, {y0}) to ({x1}, {y1})");
                    }
                }
            }
        };
        check(&terrain);
        assert!(!terrain.any_solid(-20, -20, -1, 50) && !terrain.any_solid(0, -30, 202, -1), "outside is air");
        assert!(!terrain.any_solid(250, 0, 260, 76) && !terrain.any_solid(0, 77, 202, 90));
        // Clear a whole block and the map says so; carving keeps it exact, so `Eq` still holds.
        for y in 0..BLOCK {
            for x in 0..BLOCK {
                terrain.carve(Vec2::new(f64::from(x) + 0.5, f64::from(y) + 0.5), 0.5);
            }
        }
        assert!(!terrain.any_solid(0, 0, 7, 7), "a carved-out block is air");
        check(&terrain);
        terrain.carve(Vec2::new(100.0, 72.0), 12.0);
        check(&terrain);
        assert_eq!(Terrain::decode_mask(&terrain.encode_mask()).unwrap(), terrain);
    }

    #[test]
    fn the_wire_form_leaves_the_block_map_out_and_reading_rebuilds_it() {
        let terrain = Terrain::from_fn(70, 20, |x, y| y >= 12 || x == 40).unwrap();
        let bytes = postcard::to_allocvec(&terrain).unwrap();
        let mut bare = postcard::to_allocvec(&(70_u32, 20_u32, 2_u32)).unwrap();
        bare.extend(postcard::to_allocvec(&terrain.bits).unwrap());
        assert_eq!(bytes, bare, "the state hash sees what it always did");
        assert_eq!(postcard::from_bytes::<Terrain>(&bytes).unwrap(), terrain);
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
