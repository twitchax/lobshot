//! Walking blobs (Sudsotter's shots). A blob sticks where it lands and walks along the edge of the
//! ground, keeping the ground on one side: counterclockwise around it (left, on level ground) or
//! clockwise (right). So it climbs walls, rounds corners and clings under ledges, and on a floating
//! island it goes round and round, until its fuse burns down.
//!
//! It walks in whole pixels, from one empty pixel to the next, always beside a solid one (its
//! floor): every step keeps it on the boundary between them. A step takes a set number of ticks,
//! more up and down walls and under a mobile. The path reaches the client as legs, one per straight
//! run at one speed, so it plays back like any flight. Weather columns do nothing to a walking blob
//! (CR-frog), and nothing carves the ground while it walks, so a walk is a pure function of the
//! ground and where the mobiles stand.

use crate::{
    ballistics::Flight,
    body::HitFrame,
    game::{Contact, Impact, Leg},
    terrain::Terrain,
    units::{TICK, Vec2, ticks},
};

/// Which way round the ground a blob walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sense {
    /// Ground on its left: left along level ground, down the near face of a drop.
    Counterclockwise,
    /// Ground on its right: right along level ground.
    Clockwise,
}

/// How a blob walks.
pub(super) struct Walk {
    /// Along level ground, px/s.
    pub speed: f64,
    /// Up and down walls, as a share of `speed`.
    pub climb_share: f64,
    /// Inside a mobile's hitbox, as a share of its speed there.
    pub under_mobile_share: f64,
    /// Ticks from landing until it goes off.
    pub fuse: u32,
}

/// Where a blob stood, and when: at `at` from stage tick `from` until `to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Stay {
    pub at: Vec2,
    pub from: u32,
    pub to: u32,
}

/// A whole walk.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Walked {
    /// Its path from where it stuck on, as legs.
    pub legs: Vec<Leg>,
    /// Every pixel it stood on, in order, the last until it went off.
    pub stays: Vec<Stay>,
    /// Where it goes off at its fuse: in a mobile, if it stands inside one's hitbox then, or on the
    /// ground; `Gone` if it walked off the map's sides or bottom first.
    pub end: Impact,
}

/// A pixel, or a step to one of its four neighbours.
type Cell = (i32, i32);

/// A quarter turn counterclockwise on screen (y down): east to north.
fn left((x, y): Cell) -> Cell {
    (y, -x)
}

/// A quarter turn clockwise on screen: east to south.
fn right((x, y): Cell) -> Cell {
    (-y, x)
}

fn plus((x, y): Cell, (dx, dy): Cell) -> Cell {
    (x + dx, y + dy)
}

fn center((x, y): Cell) -> Vec2 {
    Vec2::new(f64::from(x) + 0.5, f64::from(y) + 0.5)
}

/// The pixel a world point is in.
fn cell(at: Vec2) -> Cell {
    // A landing is a pixel or so from the ground, inside the map: in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let cell = (at.x.floor() as i32, at.y.floor() as i32);
    cell
}

/// One move to a neighbouring pixel, taking `cost` ticks.
#[derive(Debug, Clone, Copy)]
struct Move {
    from: Cell,
    step: Cell,
    cost: u32,
}

/// What one stride along the ground's edge does.
enum Stride {
    /// Turns to climb what is ahead, without moving.
    Turn,
    /// Steps on: one move, or two round a corner.
    Step(Move, Option<Move>),
    /// Walks off the map.
    Off,
}

/// The walk's state: where it stands, and the direction of the solid neighbour it keeps beside it.
struct Walker<'a> {
    terrain: &'a Terrain,
    frames: &'a [(u8, HitFrame)],
    rules: &'a Walk,
    sense: Sense,
    at: Cell,
    floor: Cell,
}

impl Walker<'_> {
    fn solid(&self, (x, y): Cell) -> bool {
        self.terrain.is_solid(x, y)
    }

    /// Off the map's sides or below its bottom (above its top is open air).
    fn off_map(&self, (x, y): Cell) -> bool {
        let width = i32::try_from(self.terrain.width()).unwrap_or(i32::MAX);
        let height = i32::try_from(self.terrain.height()).unwrap_or(i32::MAX);
        x < 0 || x >= width || y >= height
    }

    /// The ticks a move along `step` from `from` takes.
    fn cost(&self, from: Cell, step: Cell) -> u32 {
        let climb = if step.0 == 0 { self.rules.climb_share } else { 1.0 };
        let under = if self.frames.iter().any(|(_, frame)| frame.contains(center(from))) {
            self.rules.under_mobile_share
        } else {
            1.0
        };
        ticks(1.0 / (self.rules.speed * climb * under)).max(1)
    }

    fn step(&self, from: Cell, step: Cell) -> Move {
        Move { from, step, cost: self.cost(from, step) }
    }

    /// One stride along the edge of the ground. Solid ahead: turn to climb it. Open ahead with
    /// ground under it: step on. Open ahead over a drop: step on, and round the corner onto the face
    /// of the floor it just left.
    fn stride(&mut self) -> Stride {
        let ahead = match self.sense {
            Sense::Counterclockwise => right(self.floor),
            Sense::Clockwise => left(self.floor),
        };
        let next = plus(self.at, ahead);
        if self.solid(next) {
            self.floor = ahead;
            return Stride::Turn;
        }
        if self.off_map(next) {
            return Stride::Off;
        }
        let first = self.step(self.at, ahead);
        let corner = plus(next, self.floor);
        if self.solid(corner) {
            self.at = next;
            return Stride::Step(first, None);
        }
        if self.off_map(corner) {
            return Stride::Off;
        }
        let round = self.step(next, self.floor);
        self.at = corner;
        self.floor = (-ahead.0, -ahead.1);
        Stride::Step(first, Some(round))
    }
}

/// Where a blob that touched the ground at `into`, coming from the open point `open`, sticks: an
/// empty pixel, and the direction of a solid one beside it (the one it touched, where it can).
/// `None` if there is no ground beside it at all.
fn stick(terrain: &Terrain, open: Vec2, into: Vec2) -> Option<(Cell, Cell)> {
    let solid = |(x, y): Cell| terrain.is_solid(x, y);
    let mut at = cell(open);
    // A shot that starts inside the ground sticks to its surface, above.
    while solid(at) {
        at.1 -= 1;
    }
    let touched = cell(into);
    let toward = ((touched.0 - at.0).signum(), (touched.1 - at.1).signum());
    // Beside the touched pixel, or beside one between it and the touched pixel, below first.
    if let Some(floor) = [(0, toward.1), (toward.0, 0)].into_iter().find(|&step| step != (0, 0) && solid(plus(at, step))) {
        return Some((at, floor));
    }
    // Across a corner from it, with open pixels between: from the one below, it is beside.
    if toward.0 != 0 && toward.1 != 0 && solid(touched) {
        return Some((plus(at, (0, toward.1)), (toward.0, 0)));
    }
    [(0, 1), (-1, 0), (1, 0), (0, -1)].into_iter().find(|&step| solid(plus(at, step))).map(|floor| (at, floor))
}

/// Walks a blob that touched the ground at `into`, coming from `open`, at stage tick `landed`, the
/// `sense` way round, until its fuse burns down or it walks off the map. `frames` are the hitboxes
/// of the living mobiles it can touch (`Game::grounded_frames`: not a floating one's): it walks
/// through them, slower.
pub(super) fn walk(terrain: &Terrain, open: Vec2, into: Vec2, landed: u32, sense: Sense, rules: &Walk, frames: &[(u8, HitFrame)]) -> Walked {
    let fuse = landed + rules.fuse;
    let Some((at, floor)) = stick(terrain, open, into) else {
        // Nothing to stick to: it goes off where it is.
        let at = cell(open);
        return finish(&[], at, landed, fuse, frames);
    };
    let mut blob = Walker { terrain, frames, rules, sense, at, floor };
    let mut moves: Vec<(u32, Move)> = Vec::new();
    let (mut now, mut standing) = (landed, at);
    // Four turns in a row without a step: it is boxed into one pixel, and waits there.
    let mut turns = 0;
    while turns < 4 {
        let (first, round) = match blob.stride() {
            Stride::Turn => {
                turns += 1;
                continue;
            }
            Stride::Off => {
                let mut walked = finish(&moves, standing, now, now, frames);
                walked.end.contact = Contact::Gone;
                return walked;
            }
            Stride::Step(first, round) => (first, round),
        };
        turns = 0;
        for step in std::iter::once(first).chain(round) {
            if now + step.cost > fuse {
                return finish(&moves, standing, now, fuse, frames);
            }
            moves.push((now, step));
            now += step.cost;
            standing = plus(step.from, step.step);
        }
    }
    finish(&moves, standing, now, fuse, frames)
}

/// The walk of `moves` (each with the stage tick it starts at), standing at `at` from `now` until
/// it goes off at `fuse`.
fn finish(moves: &[(u32, Move)], at: Cell, now: u32, fuse: u32, frames: &[(u8, HitFrame)]) -> Walked {
    let mut legs: Vec<Leg> = Vec::new();
    let mut stays: Vec<Stay> = Vec::with_capacity(moves.len() + 1);
    for &(start, step) in moves {
        stays.push(Stay {
            at: center(step.from),
            from: start,
            to: start + step.cost,
        });
        // A run of moves the same way at the same speed is one leg.
        let speed = 1.0 / (f64::from(step.cost) * TICK);
        let velocity = Vec2::new(f64::from(step.step.0) * speed, f64::from(step.step.1) * speed);
        match legs.last_mut() {
            Some(leg) if leg.flight.velocity == velocity && leg.offset + leg.to == start => leg.to += step.cost,
            _ => legs.push(Leg {
                flight: Flight {
                    origin: center(step.from),
                    velocity,
                    accel: Vec2::ZERO,
                },
                from: 0,
                to: step.cost,
                offset: start,
            }),
        }
    }
    let here = center(at);
    stays.push(Stay { at: here, from: now, to: fuse });
    legs.push(Leg {
        flight: Flight {
            origin: here,
            velocity: Vec2::ZERO,
            accel: Vec2::ZERO,
        },
        from: 0,
        to: fuse - now,
        offset: now,
    });
    let contact = frames.iter().find(|(_, frame)| frame.contains(here)).map_or(Contact::Terrain, |&(seat, _)| Contact::Mobile(seat));
    Walked {
        legs,
        stays,
        end: Impact { tick: fuse, at: here, contact },
    }
}

#[cfg(test)]
mod unit {
    // Exact float equality is the claim: a walk stands on pixel centers, and a run's speed is a
    // whole number of ticks a pixel, so its points come out exact.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::units::TICKS_PER_SECOND;

    const RULES: Walk = Walk {
        speed: 32.0,
        climb_share: 0.5,
        under_mobile_share: 0.5,
        fuse: 3 * TICKS_PER_SECOND,
    };

    /// Level ground from row 100 down, on a 400 x 200 map.
    fn flat() -> Terrain {
        Terrain::from_fn(400, 200, |_, y| y >= 100).unwrap()
    }

    /// Where a blob dropped onto `terrain` at x = 200.5 from above stands, every stay.
    fn dropped(terrain: &Terrain, x: f64, sense: Sense, rules: &Walk) -> Walked {
        let top = (0..200).find(|&y| terrain.is_solid(200, y)).unwrap();
        walk(terrain, Vec2::new(x, f64::from(top) - 0.2), Vec2::new(x, f64::from(top) + 0.2), 0, sense, rules, &[])
    }

    #[test]
    fn on_level_ground_counterclockwise_is_left_and_clockwise_right() {
        let terrain = flat();
        let left = dropped(&terrain, 200.5, Sense::Counterclockwise, &RULES);
        let right = dropped(&terrain, 200.5, Sense::Clockwise, &RULES);
        // 64 ticks a pixel: 96 pixels in 3 s.
        assert_eq!(left.end.at, Vec2::new(200.5 - 96.0, 99.5));
        assert_eq!(right.end.at, Vec2::new(200.5 + 96.0, 99.5));
        assert_eq!(
            left.end,
            Impact {
                tick: 3 * TICKS_PER_SECOND,
                at: left.end.at,
                contact: Contact::Terrain
            }
        );
        assert!(left.stays.iter().all(|stay| stay.at.y == 99.5), "it stays on the ground");
    }

    #[test]
    fn its_legs_are_one_run_and_a_rest_and_play_back_where_it_stood() {
        let walked = dropped(&flat(), 200.5, Sense::Clockwise, &RULES);
        assert_eq!(walked.legs.len(), 2, "{:?}", walked.legs);
        let run = walked.legs[0];
        assert!(run.flight.accel == Vec2::ZERO && run.flight.velocity.y == 0.0);
        assert!(run.flight.at(run.to).distance(walked.end.at) < 1e-9, "{:?}", run.flight.at(run.to));
        let rest = walked.legs[1];
        assert_eq!((rest.offset, rest.offset + rest.to), (run.offset + run.to, walked.end.tick));
        for stay in &walked.stays[..walked.stays.len() - 1] {
            let at = run.flight.at(stay.from - run.offset);
            assert!(at.distance(stay.at) < 1e-9, "{at:?} vs {stay:?}");
        }
    }

    #[test]
    fn it_climbs_a_wall_slower_and_walks_on_over_the_top() {
        // A 20 px step up, 30 px to the right of the landing.
        let terrain = Terrain::from_fn(400, 200, |x, y| y >= 100 || (x >= 230 && y >= 80)).unwrap();
        let walked = dropped(&terrain, 200.5, Sense::Clockwise, &RULES);
        let up: Vec<&Stay> = walked.stays.iter().filter(|stay| stay.at.x == 229.5 && stay.to - stay.from == 128).collect();
        assert_eq!(up.len(), 20, "up the face pixel by pixel, at half speed");
        assert!(up.iter().all(|stay| (80.0..100.0).contains(&stay.at.y)), "{up:?}");
        // 29 px to the wall, 20 up and 1 round the top corner: 50 moves, 3 s at 64 a level one.
        let level = (3 * TICKS_PER_SECOND - 29 * 64 - 20 * 128 - 64) / 64;
        assert_eq!(walked.end.at, Vec2::new(230.5 + f64::from(level), 79.5));
    }

    #[test]
    fn counterclockwise_it_goes_down_a_drop_and_under_a_ledge() {
        // A shelf from x = 150 to 250 at rows 100-109, nothing under it: walking left off its end,
        // the blob rounds the corner, goes down the end face and back right along its underside.
        let terrain = Terrain::from_fn(400, 200, |x, y| (150..250).contains(&x) && (100..110).contains(&y)).unwrap();
        let fast = Walk { fuse: 10 * TICKS_PER_SECOND, ..RULES };
        let walked = dropped(&terrain, 160.5, Sense::Counterclockwise, &fast);
        let underneath = walked.stays.iter().find(|stay| stay.at.y == 110.5).unwrap();
        assert!(underneath.at.x > 149.0, "{underneath:?}");
        let after = walked.stays.iter().skip_while(|stay| stay.at.y != 110.5).nth(5).unwrap();
        assert!(after.at.x > underneath.at.x, "it clings under the ledge, heading right: {after:?}");
    }

    #[test]
    fn on_an_island_it_goes_round_and_round() {
        let terrain = Terrain::from_fn(400, 200, |x, y| (190..210).contains(&x) && (100..110).contains(&y)).unwrap();
        let long = Walk { fuse: 20 * TICKS_PER_SECOND, ..RULES };
        let walked = dropped(&terrain, 200.5, Sense::Clockwise, &long);
        let start = walked.stays[0].at;
        let returns = walked.stays.iter().filter(|stay| stay.at == start).count();
        assert!(returns >= 2, "{returns}");
        assert_eq!(walked.end.contact, Contact::Terrain);
    }

    #[test]
    fn it_walks_off_the_side_of_the_map() {
        let walked = dropped(&flat(), 30.5, Sense::Counterclockwise, &RULES);
        assert_eq!(walked.end.contact, Contact::Gone);
        assert!(walked.end.tick < 3 * TICKS_PER_SECOND);
    }

    #[test]
    fn it_slows_under_a_mobile() {
        use crate::{body::Pose, tuning::Rect, units::Facing};
        let pose = Pose {
            foot: Vec2::new(240.5, 100.0),
            facing: Facing::Left,
            tilt: 0.0,
        };
        // The hitbox spans x 230.5 to 250.5: 21 pixel centers.
        let frames = [(1, HitFrame::new(&pose, Rect { x: -10.0, y: -40.0, w: 20.0, h: 40.0 }))];
        let terrain = flat();
        let walked = walk(&terrain, Vec2::new(200.5, 99.8), Vec2::new(200.5, 100.2), 0, Sense::Clockwise, &RULES, &frames);
        let costs: Vec<u32> = walked.stays.iter().filter(|stay| (230.0..251.0).contains(&stay.at.x)).map(|stay| stay.to - stay.from).collect();
        assert!(costs.len() == 21 && costs.iter().all(|&cost| cost == 128), "{costs:?}");
        // 30 px to the mobile, 21 under it at half speed, then on: 3 s is 96 level pixels.
        assert_eq!(walked.end.at.x, 200.5 + 30.0 + 21.0 + f64::from(96 - 30 - 2 * 21));
    }

    #[test]
    fn it_goes_off_inside_a_mobile_it_stops_under() {
        use crate::{body::Pose, tuning::Rect, units::Facing};
        let pose = Pose {
            foot: Vec2::new(280.5, 100.0),
            facing: Facing::Left,
            tilt: 0.0,
        };
        let frames = [(3, HitFrame::new(&pose, Rect { x: -30.0, y: -40.0, w: 60.0, h: 40.0 }))];
        let walked = walk(&flat(), Vec2::new(200.5, 99.8), Vec2::new(200.5, 100.2), 0, Sense::Clockwise, &RULES, &frames);
        assert_eq!(walked.end.contact, Contact::Mobile(3));
    }

    #[test]
    fn it_sticks_beside_the_pixel_it_touched() {
        let terrain = flat();
        assert_eq!(stick(&terrain, Vec2::new(10.5, 99.9), Vec2::new(10.5, 100.1)), Some(((10, 99), (0, 1))));
        // Across a corner, down and right, into a wall: it sticks beside the wall.
        let wall = Terrain::from_fn(400, 200, |x, _| x >= 50).unwrap();
        assert_eq!(stick(&wall, Vec2::new(49.9, 20.9), Vec2::new(50.1, 21.1)), Some(((49, 20), (1, 0))));
        // Onto a corner with open pixels either side between: it sticks to the one below, beside it.
        let corner = Terrain::from_fn(400, 200, |x, y| x >= 50 && y >= 21).unwrap();
        assert_eq!(stick(&corner, Vec2::new(49.9, 20.9), Vec2::new(50.1, 21.1)), Some(((49, 21), (1, 0))));
        // Started inside the ground: it sticks on top.
        assert_eq!(stick(&terrain, Vec2::new(10.5, 120.5), Vec2::new(10.5, 120.6)), Some(((10, 99), (0, 1))));
        assert_eq!(stick(&Terrain::from_fn(400, 200, |_, _| false).unwrap(), Vec2::new(10.5, 20.5), Vec2::new(10.5, 21.5)), None);
    }

    #[test]
    fn boxed_in_it_waits() {
        let terrain = Terrain::from_fn(400, 200, |x, y| !(x == 200 && y == 50)).unwrap();
        let walked = walk(&terrain, Vec2::new(200.5, 50.5), Vec2::new(200.5, 51.5), 0, Sense::Clockwise, &RULES, &[]);
        assert_eq!(walked.end.at, Vec2::new(200.5, 50.5));
        assert_eq!(walked.legs.len(), 1);
    }
}
