//! Terrain geometry: one hand-authored source (`land.ron`) for both a map variant's art and its
//! collision mask. Islands are chains of cubic Béziers; walkable tops are roughened along their
//! normals so silhouettes read as rock, not vector blobs, and decorations are dropped onto the
//! result. Everything here is placement: the shapes, colors and layers stay hand-written SVG in
//! the variant's `terrain.svg`.

use std::{fmt::Write as _, path::Path};

use anyhow::{Context as _, bail, ensure};
use lobshot_sim::{base::Res, terrain::Terrain};
use serde::Deserialize;

use super::{
    raster::{Svg, alpha},
    rng::Rng,
};

pub type Point = (f64, f64);

/// A map variant's land, as written in `land.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct Land {
    /// The world size in pixels.
    pub size: (u32, u32),
    pub islands: Vec<Island>,
    /// Decorations to place, each filling a `{{slot}}` in `terrain.svg`.
    #[serde(default)]
    pub dress: Vec<Dress>,
}

/// One landmass: a closed chain of edges from `start`, clockwise on screen, so walkable tops run
/// left to right and the outward side of every edge is on its left.
#[derive(Debug, Clone, Deserialize)]
pub struct Island {
    pub start: Point,
    pub edges: Vec<Edge>,
}

/// A cubic Bézier: two control points, then the end.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Edge {
    /// Walkable ground: roughened, capped with turf, dressed with tufts.
    Top(Point, Point, Point),
    /// Cliffs and undersides, kept smooth.
    Rock(Point, Point, Point),
}

/// Where an edge decoration may sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Along {
    /// Walkable tops.
    Top,
    /// Rock that faces down: the undersides of islands.
    Under,
    /// Rock that faces sideways: cliffs.
    Wall,
}

/// A decoration rule. Symbols are `id`s of elements in `terrain.svg`'s `<defs>`, drawn with
/// their origin at the attach point and "up" as `-y`; repeat an id to weight it.
#[derive(Debug, Clone, Deserialize)]
pub enum Dress {
    /// Along the outline, about every `every` pixels of it.
    Edge {
        slot: String,
        along: Along,
        symbols: Vec<String>,
        every: f64,
        scale: (f64, f64),
        /// Turn each symbol so its up is the surface's outward normal; otherwise it stays upright
        /// (hanging things).
        #[serde(default)]
        follow_normal: bool,
        /// Mirror half of them at random. Leave it off for symbols lit from one side.
        #[serde(default)]
        mirror: bool,
        seed: u64,
    },
    /// Scattered inside the land on a jittered grid, between `depth.0` and `depth.1` pixels below
    /// the surface above.
    Fill {
        slot: String,
        symbols: Vec<String>,
        /// Colors for symbols drawn in `currentColor`; empty leaves them as authored.
        #[serde(default)]
        tints: Vec<String>,
        spacing: f64,
        depth: (f64, f64),
        scale: (f64, f64),
        /// The largest turn either way, in degrees.
        turn: f64,
        #[serde(default)]
        mirror: bool,
        seed: u64,
    },
}

/// A point on the outline, after roughening, with its outward unit normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlinePoint {
    pub at: Point,
    pub normal: Point,
    pub top: bool,
}

/// The land, built: the one path both the art and the mask draw, and what placement needs.
#[derive(Debug, Clone)]
pub struct Shape {
    pub size: (u32, u32),
    /// SVG path data for every island.
    pub d: String,
    /// Each island flattened to a polygon (closing edge implied), for inside tests.
    pub polygons: Vec<Vec<Point>>,
    /// Points along the whole outline, about `SAMPLE_PX` apart.
    pub outline: Vec<OutlinePoint>,
}

/// Spacing of outline samples, in pixels: fine enough that roughening's shortest wave (about 20 px)
/// keeps its shape.
const SAMPLE_PX: f64 = 1.5;
/// Roughening fades in over this many pixels at each end of a run of tops, so it meets the smooth
/// rock without a step.
const ROUGH_FADE_PX: f64 = 16.0;

impl Land {
    pub fn load(path: &Path) -> Res<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_ron(&text).with_context(|| format!("reading {}", path.display()))
    }

    pub fn from_ron(text: &str) -> Res<Self> {
        let land: Self = ron::from_str(text).context("malformed land")?;
        ensure!(land.size.0 > 0 && land.size.1 > 0, "the land has no size");
        ensure!(!land.islands.is_empty(), "the land has no islands");
        Ok(land)
    }

    /// Builds the outline: tops roughened, rock flattened for placement.
    pub fn shape(&self) -> Res<Shape> {
        let mut d = String::new();
        let mut polygons = Vec::with_capacity(self.islands.len());
        let mut outline = Vec::new();
        for (index, island) in self.islands.iter().enumerate() {
            let built = island.build().with_context(|| format!("island {index}"))?;
            d.push_str(&built.d);
            polygons.push(built.polygon);
            outline.extend(built.outline);
        }
        Ok(Shape { size: self.size, d, polygons, outline })
    }
}

struct BuiltIsland {
    d: String,
    polygon: Vec<Point>,
    outline: Vec<OutlinePoint>,
}

impl Island {
    fn build(&self) -> Res<BuiltIsland> {
        ensure!(self.edges.len() >= 2, "an island needs at least two edges");
        let mut d = format!("M {} ", fmt_point(self.start));
        let mut polygon = vec![self.start];
        let mut outline = Vec::new();
        let mut from = self.start;
        let mut edges = self.edges.iter().peekable();
        while let Some(&edge) = edges.next() {
            match edge {
                Edge::Top(..) => {
                    let mut run = vec![edge];
                    while let Some(&&next @ Edge::Top(..)) = edges.peek() {
                        run.push(next);
                        edges.next();
                    }
                    let points = rough_top(from, &run)?;
                    for point in &points[1..] {
                        let _ = write!(d, "L {} ", fmt_point(point.at));
                        polygon.push(point.at);
                    }
                    outline.extend(points);
                    from = run.last().map_or(from, |edge| edge.end());
                }
                Edge::Rock(c1, c2, to) => {
                    let _ = write!(d, "C {} {} {} ", fmt_point(c1), fmt_point(c2), fmt_point(to));
                    let samples = samples_for(from, c1, c2, to);
                    for i in 1..=samples {
                        let t = f64::from(i) / f64::from(samples);
                        let at = bezier(from, c1, c2, to, t);
                        polygon.push(at);
                        outline.push(OutlinePoint {
                            at,
                            normal: outward(tangent(from, c1, c2, to, t)),
                            top: false,
                        });
                    }
                    from = to;
                }
            }
        }
        ensure!(
            (from.0 - self.start.0).abs() < 1e-6 && (from.1 - self.start.1).abs() < 1e-6,
            "the edges end at {from:?}, not back at the start {:?}",
            self.start
        );
        // The closing point duplicates the start.
        polygon.pop();
        ensure!(signed_area(&polygon) > 0.0, "the island runs counterclockwise on screen; run it clockwise, with tops left to right");
        d.push_str("Z ");
        Ok(BuiltIsland { d, polygon, outline })
    }
}

impl Edge {
    fn end(self) -> Point {
        match self {
            Self::Top(_, _, to) | Self::Rock(_, _, to) => to,
        }
    }
}

/// A run of tops, sampled about every `SAMPLE_PX` and pushed out along the normal by a sum of
/// incommensurate waves (the spike's recipe), faded to zero at the run's ends. The first point is
/// `from` itself and the last is the run's end, both unmoved.
fn rough_top(from: Point, run: &[Edge]) -> Res<Vec<OutlinePoint>> {
    let mut smooth: Vec<(Point, Point)> = vec![(from, outward(first_tangent(from, run[0])))];
    let mut start = from;
    for &edge in run {
        let Edge::Top(c1, c2, to) = edge else { bail!("a run of tops holds only tops") };
        ensure!(to.0 > start.0, "a top must run left to right (from {start:?} to {to:?}), so its outward side is up");
        let samples = samples_for(start, c1, c2, to);
        for i in 1..=samples {
            let t = f64::from(i) / f64::from(samples);
            smooth.push((bezier(start, c1, c2, to, t), outward(tangent(start, c1, c2, to, t))));
        }
        start = to;
    }

    let mut along = Vec::with_capacity(smooth.len());
    let mut total = 0.0;
    for (i, &(at, _)) in smooth.iter().enumerate() {
        if i > 0 {
            total += distance(smooth[i - 1].0, at);
        }
        along.push(total);
    }
    Ok(smooth
        .iter()
        .zip(&along)
        .map(|(&((x, y), normal), &s)| {
            let fade = (s.min(total - s) / ROUGH_FADE_PX).clamp(0.0, 1.0);
            let offset = fade * roughness(x);
            OutlinePoint {
                at: (x + offset * normal.0, y + offset * normal.1),
                normal,
                top: true,
            }
        })
        .collect())
}

/// How far the rough edge stands proud of the authored curve at `x`: a ripple of about 23 px whose
/// strength drifts with a 195 px wave, a 38 px wave, and one-sided 170 px knolls, all
/// incommensurate so no stretch of ground repeats another.
fn roughness(x: f64) -> f64 {
    let knoll = libm::sin(x / 27.0 + 2.0).max(0.0);
    let ripple = 0.8 * libm::sin(x / 3.7 + 0.4) * (0.55 + 0.45 * libm::sin(x / 31.0));
    1.2 * libm::sin(x / 6.1 + 1.1) + ripple + 2.0 * knoll * knoll * knoll
}

fn first_tangent(from: Point, edge: Edge) -> Point {
    match edge {
        Edge::Top(c1, c2, to) | Edge::Rock(c1, c2, to) => tangent(from, c1, c2, to, 0.0),
    }
}

/// Enough samples that neighbors are about `SAMPLE_PX` apart, judged by the control polygon.
fn samples_for(p0: Point, c1: Point, c2: Point, p3: Point) -> u32 {
    let length = distance(p0, c1) + distance(c1, c2) + distance(c2, p3);
    // The control polygon of an authored edge is a few hundred pixels at most.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let samples = (length / SAMPLE_PX).ceil().max(4.0) as u32;
    samples
}

fn bezier(p0: Point, c1: Point, c2: Point, p3: Point, t: f64) -> Point {
    let rest = 1.0 - t;
    let weights = [rest * rest * rest, 3.0 * rest * rest * t, 3.0 * rest * t * t, t * t * t];
    let blend = |pick: fn(Point) -> f64| weights[0] * pick(p0) + weights[1] * pick(c1) + weights[2] * pick(c2) + weights[3] * pick(p3);
    (blend(|p| p.0), blend(|p| p.1))
}

fn tangent(p0: Point, c1: Point, c2: Point, p3: Point, t: f64) -> Point {
    let rest = 1.0 - t;
    let weights = [3.0 * rest * rest, 6.0 * rest * t, 3.0 * t * t];
    let blend = |pick: fn(Point) -> f64| weights[0] * (pick(c1) - pick(p0)) + weights[1] * (pick(c2) - pick(c1)) + weights[2] * (pick(p3) - pick(c2));
    let raw = (blend(|p| p.0), blend(|p| p.1));
    // A control point on its end point leaves no tangent there; the chord's direction stands in.
    if raw.0.hypot(raw.1) < 1e-9 { (p3.0 - p0.0, p3.1 - p0.1) } else { raw }
}

/// The outward unit normal of a clockwise-on-screen outline: the tangent turned a quarter left.
fn outward((tx, ty): Point) -> Point {
    let length = tx.hypot(ty);
    if length < 1e-12 { (0.0, -1.0) } else { (ty / length, -tx / length) }
}

fn distance(a: Point, b: Point) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// The shoelace area: positive for a polygon that runs clockwise on screen (y down).
fn signed_area(polygon: &[Point]) -> f64 {
    let n = polygon.len();
    (0..n).map(|i| polygon[i].0 * polygon[(i + 1) % n].1 - polygon[(i + 1) % n].0 * polygon[i].1).sum::<f64>() / 2.0
}

fn fmt_point((x, y): Point) -> String {
    format!("{x:.2},{y:.2}")
}

impl Shape {
    /// Whether a point is inside the land.
    pub fn contains(&self, (x, y): Point) -> bool {
        self.polygons.iter().any(|polygon| crossings_above(polygon, (x, y)).0 % 2 == 1)
    }

    /// How far a point lies below the nearest outline directly above it (`None` if nothing is).
    pub fn depth(&self, (x, y): Point) -> Option<f64> {
        self.polygons.iter().filter_map(|polygon| crossings_above(polygon, (x, y)).1).map(|above| y - above).reduce(f64::min)
    }

    /// The collision mask, from the land path alone: drawn at one pixel per world pixel and solid
    /// where at least half a pixel is covered, so edges are crisp and decorations never count.
    pub fn mask(&self) -> Res<Terrain> {
        let (width, height) = self.size;
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><path d="{}" fill="#000"/></svg>"##,
            self.d
        );
        let pixmap = Svg::parse(&svg).context("drawing the mask")?.render_view(1.0)?;
        ensure!((pixmap.width(), pixmap.height()) == self.size, "the mask drew at {}x{}", pixmap.width(), pixmap.height());
        let alpha = alpha(&pixmap);
        Terrain::from_fn(width, height, |x, y| alpha[(y * width + x) as usize] >= 128)
    }

    /// Fills `terrain.svg`'s `{{land}}` and dress slots.
    pub fn dress(&self, template: &str, dress: &[Dress]) -> Res<String> {
        let mut svg = fill_slot(template, "land", &self.d)?;
        for rule in dress {
            let (slot, uses) = match rule {
                Dress::Edge { slot, .. } => (slot, self.dress_edge(rule)?),
                Dress::Fill { slot, .. } => (slot, self.dress_fill(rule)?),
            };
            svg = fill_slot(&svg, slot, &uses)?;
        }
        if let Some(at) = svg.find("{{") {
            let slot: String = svg[at..].chars().take_while(|&c| c != '}').collect();
            bail!("terrain.svg has a slot no dress rule fills: {slot}}}}}");
        }
        Ok(svg)
    }

    fn dress_edge(&self, rule: &Dress) -> Res<String> {
        let Dress::Edge {
            along,
            symbols,
            every,
            scale,
            follow_normal,
            mirror,
            seed,
            ..
        } = rule
        else {
            bail!("not an edge rule")
        };
        ensure!(!symbols.is_empty() && *every > 0.0, "an edge rule needs symbols and a positive spacing");
        let mut rng = Rng::new(*seed);
        let mut uses = String::new();
        let mut walked = 0.0;
        let mut next = every * rng.unit();
        let width = f64::from(self.size.0);
        for pair in self.outline.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            walked += distance(a.at, b.at);
            if walked < next {
                continue;
            }
            next = walked + every * rng.range(0.6, 1.4);
            let (nx, ny) = b.normal;
            let fits = match along {
                Along::Top => b.top,
                Along::Under => !b.top && ny > 0.55,
                Along::Wall => !b.top && nx.abs() > 0.75,
            };
            if !fits || !(0.0..=width).contains(&b.at.0) {
                continue;
            }
            let symbol = rng.pick(symbols);
            let size = rng.range(scale.0, scale.1);
            let flip = if rng.unit() < 0.5 && *mirror { -1.0 } else { 1.0 };
            let turn = if *follow_normal { libm::atan2(nx, -ny).to_degrees() } else { 0.0 };
            let _ = writeln!(
                uses,
                r##"<use href="#{symbol}" transform="translate({}) rotate({turn:.1}) scale({:.2} {size:.2})"/>"##,
                fmt_point(b.at).replace(',', " "),
                flip * size
            );
        }
        Ok(uses)
    }

    fn dress_fill(&self, rule: &Dress) -> Res<String> {
        let Dress::Fill {
            symbols,
            tints,
            spacing,
            depth,
            scale,
            turn,
            mirror,
            seed,
            ..
        } = rule
        else {
            bail!("not a fill rule")
        };
        ensure!(!symbols.is_empty() && *spacing > 0.0, "a fill rule needs symbols and a positive spacing");
        let mut rng = Rng::new(*seed);
        let mut uses = String::new();
        let (width, height) = (f64::from(self.size.0), f64::from(self.size.1));
        let mut y = 0.0;
        while y < height {
            let mut x = 0.0;
            while x < width {
                let at = (x + spacing * rng.unit(), y + spacing * rng.unit());
                // Draw every number for the cell before deciding, so one rejected cell never
                // shifts the rest of the field.
                let (symbol, size, angle, flip) = (rng.pick(symbols), rng.range(scale.0, scale.1), rng.range(-turn, *turn), rng.unit() < 0.5);
                let tint = if tints.is_empty() { None } else { Some(rng.pick(tints)) };
                x += spacing;
                let deep_enough = self.depth(at).is_some_and(|below| (depth.0..=depth.1).contains(&below));
                if !self.contains(at) || !deep_enough {
                    continue;
                }
                let color = tint.map_or_else(String::new, |tint| format!(r#" color="{tint}""#));
                let sx = if flip && *mirror { -size } else { size };
                let _ = writeln!(
                    uses,
                    r##"<use href="#{symbol}"{color} transform="translate({}) rotate({angle:.1}) scale({sx:.2} {size:.2})"/>"##,
                    fmt_point(at).replace(',', " ")
                );
            }
            y += spacing;
        }
        Ok(uses)
    }
}

/// Crossings of the vertical ray from `p` upward with the polygon's edges: how many, and the
/// lowest one (the outline directly above).
fn crossings_above(polygon: &[Point], (x, y): Point) -> (usize, Option<f64>) {
    let mut count = 0;
    let mut nearest: Option<f64> = None;
    for (i, &from) in polygon.iter().enumerate() {
        let to = polygon[(i + 1) % polygon.len()];
        // Half-open in x, so a vertex on the ray counts once.
        if (from.0 <= x) == (to.0 <= x) {
            continue;
        }
        let at = from.1 + (x - from.0) * (to.1 - from.1) / (to.0 - from.0);
        if at <= y {
            count += 1;
            nearest = Some(nearest.map_or(at, |lowest: f64| lowest.max(at)));
        }
    }
    (count, nearest)
}

fn fill_slot(svg: &str, slot: &str, with: &str) -> Res<String> {
    let marker = format!("{{{{{slot}}}}}");
    ensure!(svg.matches(&marker).count() == 1, "terrain.svg must hold {marker} exactly once");
    Ok(svg.replace(&marker, with))
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    /// A 200x120 world with one slab: a top from (20, 60) to (180, 60), then rock down and back.
    fn slab() -> Land {
        Land::from_ron(
            r"Land(
                size: (200, 120),
                islands: [(
                    start: (20.0, 60.0),
                    edges: [
                        Top((70.0, 60.0), (130.0, 60.0), (180.0, 60.0)),
                        Rock((185.0, 80.0), (170.0, 100.0), (150.0, 100.0)),
                        Rock((110.0, 100.0), (60.0, 100.0), (40.0, 100.0)),
                        Rock((25.0, 100.0), (15.0, 80.0), (20.0, 60.0)),
                    ],
                )],
            )",
        )
        .unwrap()
    }

    #[test]
    fn roughens_tops_but_pins_their_ends_and_leaves_rock_smooth() {
        let shape = slab().shape().unwrap();
        let tops: Vec<_> = shape.outline.iter().filter(|p| p.top).collect();
        assert_eq!(tops.first().unwrap().at, (20.0, 60.0));
        assert_eq!(tops.last().unwrap().at, (180.0, 60.0));
        assert!(tops.iter().any(|p| (p.at.1 - 60.0).abs() > 1.0), "the middle is rough");
        assert!(tops.iter().all(|p| (p.at.1 - 60.0).abs() < 6.0), "but only a few pixels");
        assert!(shape.d.contains("C 185.00,80.00 170.00,100.00 150.00,100.00"), "rock stays a curve: {}", shape.d);
    }

    #[test]
    fn normals_point_out_of_the_land() {
        let shape = slab().shape().unwrap();
        for point in &shape.outline {
            let out = (point.at.0 + 3.0 * point.normal.0, point.at.1 + 3.0 * point.normal.1);
            let inside = (point.at.0 - 3.0 * point.normal.0, point.at.1 - 3.0 * point.normal.1);
            assert!(!shape.contains(out) && shape.contains(inside), "{point:?}");
        }
    }

    #[test]
    fn rejects_islands_that_run_the_wrong_way_or_do_not_close() {
        let mut land = slab();
        land.islands[0].edges.pop();
        assert!(land.shape().is_err(), "open");

        let backwards = Land::from_ron(
            r"Land(size: (100, 100), islands: [(start: (80.0, 50.0), edges: [
                Rock((60.0, 50.0), (40.0, 50.0), (20.0, 50.0)),
                Rock((20.0, 70.0), (80.0, 70.0), (80.0, 50.0)),
            ])])",
        )
        .unwrap();
        assert!(backwards.shape().is_err(), "counterclockwise");
    }

    #[test]
    fn depth_is_measured_from_the_outline_above() {
        let shape = slab().shape().unwrap();
        let depth = shape.depth((100.0, 90.0)).unwrap();
        assert!((25.0..35.0).contains(&depth), "{depth}");
        assert_eq!(shape.depth((100.0, 10.0)), None);
    }

    #[test]
    fn dress_fills_every_slot_and_rejects_strays() {
        let mut land = slab();
        land.dress = ron::from_str(
            r##"[
                Edge(slot: "tufts", along: Top, symbols: ["tuft"], every: 10.0, scale: (1.0, 1.0), follow_normal: true, seed: 1),
                Fill(slot: "stones", symbols: ["stone"], tints: ["#c96"], spacing: 8.0, depth: (6.0, 100.0), scale: (0.5, 1.0), turn: 20.0, seed: 2),
            ]"##,
        )
        .unwrap();
        let shape = land.shape().unwrap();
        let template = r#"<path d="{{land}}"/><g>{{tufts}}</g><g>{{stones}}</g>"#;
        let svg = shape.dress(template, &land.dress).unwrap();
        assert!(svg.matches("#tuft").count() > 8 && svg.matches("#stone").count() > 20, "{svg}");
        assert!(svg.contains(r##"color="#c96""##));
        assert_eq!(svg, shape.dress(template, &land.dress).unwrap(), "placement is seeded");

        assert!(shape.dress(r"{{land}}{{tufts}}", &land.dress).is_err(), "a rule with no slot");
        assert!(shape.dress(r"{{land}}{{tufts}}{{stones}}{{moss}}", &land.dress).is_err(), "a slot with no rule");
    }

    #[test]
    fn the_mask_is_the_land_at_one_pixel_per_world_pixel() {
        let shape = slab().shape().unwrap();
        let mask = shape.mask().unwrap();
        assert_eq!((mask.width(), mask.height()), (200, 120));
        assert!(mask.is_solid(100, 80) && !mask.is_solid(100, 40) && !mask.is_solid(5, 80));
    }
}
