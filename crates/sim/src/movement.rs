//! Walking and falling on the pixel mask, in whole pixels. A mobile stands on a surface pixel
//! (solid, air above); each step moves one column, climbing up to the mobile's climb (most take
//! `CLIMB_PX`) or dropping up to `DROP_PX`, and anything steeper is a wall or an edge to fall from. Falling is instant: the sim
//! only needs where it ends, and the client animates it.

use serde::{Deserialize, Serialize};

use crate::{
    terrain::Terrain,
    tuning::{DROP_PX, TILT_SEARCH_PX},
    units::{Facing, Vec2},
};

/// A mobile's footing: the column it stands in and the surface row under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Footing {
    pub x: i32,
    pub y: i32,
}

impl Footing {
    /// The ground contact point in world coordinates: the top edge of the surface pixel's row, at
    /// the column's center.
    #[must_use]
    pub fn world(self) -> Vec2 {
        Vec2::new(f64::from(self.x) + 0.5, f64::from(self.y))
    }
}

/// Where a mobile ends up once nothing holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    On(Footing),
    /// It fell out of the bottom of the map.
    OffMap,
}

/// Drops a mobile in column `x` from row `y` straight down to the next surface. A mobile buried by
/// its spawn climbs out to the surface above it instead.
#[must_use]
pub fn settle(terrain: &Terrain, Footing { x, y }: Footing) -> Landing {
    if terrain.is_solid(x, y) {
        let mut top = y;
        while terrain.is_solid(x, top - 1) {
            top -= 1;
        }
        return Landing::On(Footing { x, y: top });
    }
    match terrain.surface_at_or_below(x, y) {
        Some(y) => Landing::On(Footing { x, y }),
        None => Landing::OffMap,
    }
}

/// The result of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Moved one column, onto this footing.
    Walked(Footing),
    /// A wall or the map's side: no move, no cost.
    Blocked,
    /// Walked off an edge and landed (or fell out of the map).
    Fell(Landing),
}

/// One 1 px step in `facing`, climbing at most `climb` px.
#[must_use]
pub fn step(terrain: &Terrain, from: Footing, facing: Facing, climb: i32) -> Step {
    let x = from.x + facing.sign();
    let width = i32::try_from(terrain.width()).unwrap_or(i32::MAX);
    if !(0..width).contains(&x) {
        return Step::Blocked;
    }
    let window = (from.y - climb)..=(from.y + DROP_PX.value);
    if let Some(y) = window.clone().find(|&y| terrain.is_surface(x, y)) {
        return Step::Walked(Footing { x, y });
    }
    if window.into_iter().all(|y| terrain.is_solid(x, y)) {
        return Step::Blocked;
    }
    Step::Fell(settle(terrain, Footing { x, y: from.y }))
}

/// What a mobile slid to an edge does there (`slide`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// Stops on the very edge.
    Holds,
    /// Goes over it, falls, and slides on from where it lands.
    Drops,
}

/// Slides a mobile from `from` up to `steps` 1 px steps in `direction`, by the walking rules
/// (`step`: up its `climb`, down `DROP_PX`, stopped by walls and the map's sides) but carried, not
/// walking, so it spends no move budget: pulled or pushed (Lodestoad's shots). At an edge it holds,
/// or drops (`edge`): it goes over, falls to the ground below and slides on from there, a step
/// spent on the fall. Returns the footings it passes through, a step each, with the point in the
/// air just over each edge it goes over, before where it lands. One that falls off the map ends at
/// that point, for `settle` to find no ground under.
#[must_use]
pub fn slide(terrain: &Terrain, from: Footing, direction: Facing, climb: i32, steps: u32, edge: Edge) -> Vec<Footing> {
    let mut path = Vec::new();
    let mut at = from;
    for _ in 0..steps {
        match step(terrain, at, direction, climb) {
            Step::Walked(footing) => at = footing,
            Step::Fell(landing) if edge == Edge::Drops => {
                path.push(Footing { x: at.x + direction.sign(), y: at.y });
                let Landing::On(footing) = landing else { break };
                at = footing;
            }
            Step::Blocked | Step::Fell(_) => break,
        }
        path.push(at);
    }
    path
}

/// The ground's slope under a mobile, from the surface heights under its two probes (radians,
/// counterclockwise on screen). A probe that finds no ground nearby reads the mobile's own height.
#[must_use]
pub fn tilt(terrain: &Terrain, at: Footing, probes: [f64; 2]) -> f64 {
    let search = TILT_SEARCH_PX.value;
    let height = |offset: f64| -> (f64, f64) {
        // Probe offsets are a few dozen pixels; rounding to a column is exact in i32.
        #[allow(clippy::cast_possible_truncation)]
        let x = at.x + offset.round() as i32;
        let y = ((at.y - search)..=(at.y + search)).find(|&y| terrain.is_surface(x, y)).unwrap_or(at.y);
        (f64::from(x), f64::from(y))
    };
    let (left_x, left_y) = height(probes[0].min(probes[1]));
    let (right_x, right_y) = height(probes[0].max(probes[1]));
    if right_x <= left_x {
        return 0.0;
    }
    // y points down, so a higher right side (smaller y) is a positive, counterclockwise slope.
    libm::atan2(left_y - right_y, right_x - left_x)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{tuning::CLIMB_PX, units::to_degrees};

    const CLIMB: i32 = CLIMB_PX.value;

    /// Ground at `ground(x)`, solid below.
    fn terrain(width: u32, height: u32, ground: impl Fn(u32) -> u32) -> Terrain {
        Terrain::from_fn(width, height, |x, y| y >= ground(x)).unwrap()
    }

    #[test]
    fn settles_onto_the_ground_below_or_out_of_the_map() {
        let flat = terrain(50, 100, |_| 60);
        assert_eq!(settle(&flat, Footing { x: 10, y: 0 }), Landing::On(Footing { x: 10, y: 60 }));
        assert_eq!(settle(&flat, Footing { x: 10, y: 80 }), Landing::On(Footing { x: 10, y: 60 }), "climbs out when buried");
        let pit = terrain(50, 100, |x| if x == 10 { 100 } else { 60 });
        assert_eq!(settle(&pit, Footing { x: 10, y: 0 }), Landing::OffMap);
    }

    #[test]
    fn walks_climbs_and_drops_within_the_limits() {
        // A 6 px step up at x = 11, then a 5 px drop at x = 12.
        let ground = terrain(50, 100, |x| match x {
            11 => 54,
            12.. => 59,
            _ => 60,
        });
        let start = Footing { x: 10, y: 60 };
        assert_eq!(step(&ground, start, Facing::Right, CLIMB), Step::Walked(Footing { x: 11, y: 54 }));
        assert_eq!(step(&ground, Footing { x: 11, y: 54 }, Facing::Right, CLIMB), Step::Walked(Footing { x: 12, y: 59 }));
        assert_eq!(step(&ground, start, Facing::Left, CLIMB), Step::Walked(Footing { x: 9, y: 60 }));
    }

    #[test]
    fn a_poor_climber_is_walled_by_a_step_another_takes() {
        // A 3 px step up at x = 11.
        let ground = terrain(50, 100, |x| if x >= 11 { 57 } else { 60 });
        let start = Footing { x: 10, y: 60 };
        assert_eq!(step(&ground, start, Facing::Right, 3), Step::Walked(Footing { x: 11, y: 57 }));
        assert_eq!(step(&ground, start, Facing::Right, 2), Step::Blocked);
    }

    #[test]
    fn a_wall_blocks_and_an_edge_drops() {
        let wall = terrain(50, 100, |x| if x == 11 { 53 } else { 60 });
        assert_eq!(step(&wall, Footing { x: 10, y: 60 }, Facing::Right, CLIMB), Step::Blocked);

        let cliff = terrain(50, 100, |x| if x >= 11 { 90 } else { 60 });
        assert_eq!(step(&cliff, Footing { x: 10, y: 60 }, Facing::Right, CLIMB), Step::Fell(Landing::On(Footing { x: 11, y: 90 })));

        let edge = terrain(50, 100, |_| 60);
        assert_eq!(step(&edge, Footing { x: 49, y: 60 }, Facing::Right, CLIMB), Step::Blocked, "the map's side is a wall");
    }

    #[test]
    fn a_slide_follows_the_ground_and_stops_at_a_wall() {
        // A 6 px step up at x = 13, a 5 px step down at x = 15, a wall at x = 20.
        let ground = terrain(50, 100, |x| match x {
            13 | 14 => 54,
            15..=19 => 59,
            20 => 30,
            _ => 60,
        });
        let path = slide(&ground, Footing { x: 10, y: 60 }, Facing::Right, CLIMB, 40, Edge::Drops);
        let expected: Vec<Footing> = [(11, 60), (12, 60), (13, 54), (14, 54), (15, 59), (16, 59), (17, 59), (18, 59), (19, 59)]
            .map(|(x, y)| Footing { x, y })
            .to_vec();
        assert_eq!(path, expected);
        assert_eq!(slide(&ground, Footing { x: 10, y: 60 }, Facing::Right, CLIMB, 2, Edge::Drops).len(), 2, "no farther than its steps");
        assert_eq!(
            slide(&ground, Footing { x: 10, y: 60 }, Facing::Right, 5, 40, Edge::Drops).len(),
            2,
            "a poor climber stops at the step up"
        );
    }

    #[test]
    fn a_slide_holds_at_an_edge_or_drops_over_it_and_slides_on() {
        let cliff = terrain(50, 100, |x| if x >= 13 { 90 } else { 60 });
        let start = Footing { x: 10, y: 60 };
        let held = slide(&cliff, start, Facing::Right, CLIMB, 10, Edge::Holds);
        assert_eq!(held.last(), Some(&Footing { x: 12, y: 60 }), "on the very edge");
        let dropped = slide(&cliff, start, Facing::Right, CLIMB, 5, Edge::Drops);
        let expected: Vec<Footing> = [(11, 60), (12, 60), (13, 60), (13, 90), (14, 90), (15, 90)].map(|(x, y)| Footing { x, y }).to_vec();
        assert_eq!(dropped, expected, "over the edge, in the air, then down and on");
        assert_eq!(slide(&cliff, start, Facing::Left, CLIMB, 0, Edge::Drops), [], "no steps, no slide");

        let pit = terrain(50, 100, |x| if x == 13 { 100 } else { 60 });
        let fallen = slide(&pit, start, Facing::Right, CLIMB, 10, Edge::Drops);
        assert_eq!(fallen.last(), Some(&Footing { x: 13, y: 60 }), "over the pit, with no ground below");
        assert_eq!(settle(&pit, Footing { x: 13, y: 60 }), Landing::OffMap);
    }

    #[test]
    fn tilt_follows_the_slope() {
        // Rising one pixel per column to the right: 45 degrees.
        let slope = terrain(100, 200, |x| 150 - x);
        let degrees = to_degrees(tilt(&slope, Footing { x: 50, y: 100 }, [-18.0, 18.0]));
        assert!((degrees - 45.0).abs() < 1e-9, "{degrees}");
        let flat = terrain(100, 200, |_| 150);
        assert!(tilt(&flat, Footing { x: 50, y: 150 }, [-18.0, 18.0]).abs() < 1e-12);
    }
}
