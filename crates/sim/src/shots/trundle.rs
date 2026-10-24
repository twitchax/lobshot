//! Things that trundle along the ground the way a mobile walks (`movement::step`): Tinbadger's
//! wind-up beetles, the mines that crawl toward a target and the big one that marches ahead. Each
//! step is one column, climbing what a mobile could and dropping off what it couldn't; a wall
//! stops a crawler and turns a marcher round. The walk reaches the client as legs, one per run of
//! like steps, so it plays back like any flight. Nothing carves the ground while one walks, so a
//! walk is a pure function of the ground, where it starts and the mobiles it may touch.

use crate::{
    ballistics::Flight,
    game::{Contact, Impact, Leg},
    movement::{self, Footing, Landing, Step},
    terrain::Terrain,
    units::{Facing, TICK, Vec2, ticks},
};

/// How a walker walks.
pub(crate) struct Gait {
    /// Along the ground, px/s: a step a column, whatever it climbs or drops.
    pub speed: f64,
    /// The most a step may climb, px.
    pub climb: i32,
    /// How high its middle stands over the ground, px: what touches a mobile, and where it is drawn.
    pub height: f64,
}

/// Where a walker sets out from: the ground it stands on, the way it faces, and the stage tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Start {
    pub at: Footing,
    pub facing: Facing,
    pub tick: u32,
}

/// Where a walker heads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heading {
    /// Toward column `x`: it stops once there, or at a wall.
    Toward(i32),
    /// On ahead, turning round at walls.
    Ahead,
}

/// A whole walk.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Walked {
    /// Its path from where it started, as legs; a walker that never moved has one resting leg.
    pub legs: Vec<Leg>,
    /// Where it stopped, and why: in a mobile it touched (`Mobile`), out of steps or stopped
    /// (`Terrain`), or off the map's bottom (`Gone`). `at` is its middle.
    pub end: Impact,
    /// The ground it stands on at the end (where it was when it left the map, if it did).
    pub footing: Footing,
    pub facing: Facing,
}

impl Gait {
    /// A walker's middle, standing at `footing`.
    pub fn middle(&self, footing: Footing) -> Vec2 {
        footing.world() + Vec2::new(0.0, -self.height)
    }

    /// Ticks one step takes.
    fn step_ticks(&self) -> u32 {
        ticks(1.0 / self.speed).max(1)
    }
}

/// Walks from `start` for at most `steps` steps (a turn round at a wall counts as one), the way
/// `heading` says. `touch` names the mobile a walker's middle is inside, if any it may touch: it
/// stops there. It falls off edges as a mobile does, straight down (`fall_ticks`).
pub(crate) fn walk(terrain: &Terrain, start: Start, gait: &Gait, heading: Heading, steps: u32, touch: impl Fn(Vec2) -> Option<u8>) -> Walked {
    let step_ticks = gait.step_ticks();
    let mut runs = Runs::default();
    let Start { mut at, mut facing, tick: mut now } = start;
    let ended = |footing: Footing, facing: Facing, now: u32, contact: Contact, runs: Runs| {
        let middle = gait.middle(footing);
        Walked {
            legs: runs.finish(middle, now),
            end: Impact { tick: now, at: middle, contact },
            footing,
            facing,
        }
    };
    if let Some(seat) = touch(gait.middle(at)) {
        return ended(at, facing, now, Contact::Mobile(seat), runs);
    }
    for _ in 0..steps {
        let way = match heading {
            Heading::Toward(x) if x == at.x => break,
            Heading::Toward(x) => {
                if x > at.x {
                    Facing::Right
                } else {
                    Facing::Left
                }
            }
            Heading::Ahead => facing,
        };
        facing = way;
        let next = match movement::step(terrain, at, way, gait.climb) {
            Step::Walked(next) => {
                runs.go(gait.middle(at), gait.middle(next), now, step_ticks);
                now += step_ticks;
                next
            }
            Step::Blocked if heading == Heading::Ahead => {
                // It turns round where it stands, taking a step's time.
                facing = facing.flip();
                runs.wait(gait.middle(at), now, step_ticks);
                now += step_ticks;
                continue;
            }
            Step::Blocked => break,
            Step::Fell(Landing::On(landed)) => {
                // Over the edge, then straight down.
                let over = Footing { x: landed.x, y: at.y };
                runs.go(gait.middle(at), gait.middle(over), now, step_ticks);
                now += step_ticks;
                let drop = fall_ticks(gait, over, landed);
                runs.go(gait.middle(over), gait.middle(landed), now, drop);
                now += drop;
                landed
            }
            Step::Fell(Landing::OffMap) => {
                let x = at.x + way.sign();
                let gone = Footing {
                    x,
                    y: i32::try_from(terrain.height()).unwrap_or(i32::MAX),
                };
                let over = Footing { x, y: at.y };
                runs.go(gait.middle(at), gait.middle(over), now, step_ticks);
                now += step_ticks;
                let drop = fall_ticks(gait, over, gone);
                runs.go(gait.middle(over), gait.middle(gone), now, drop);
                now += drop;
                return ended(gone, facing, now, Contact::Gone, runs);
            }
        };
        at = next;
        if let Some(seat) = touch(gait.middle(at)) {
            return ended(at, facing, now, Contact::Mobile(seat), runs);
        }
    }
    ended(at, facing, now, Contact::Terrain, runs)
}

/// How long a walker takes to drop from `over` to `landed`: as fast as it walks, four times over.
fn fall_ticks(gait: &Gait, over: Footing, landed: Footing) -> u32 {
    let height = f64::from((landed.y - over.y).max(0));
    ticks(height / (4.0 * gait.speed)).max(1)
}

/// A walk's legs, built a move at a time: moves the same way at the same speed run on in one leg.
#[derive(Default)]
struct Runs {
    legs: Vec<Leg>,
}

impl Runs {
    /// Moves from `from` to `to` over `span` ticks, starting at stage tick `now`.
    fn go(&mut self, from: Vec2, to: Vec2, now: u32, span: u32) {
        let velocity = if span == 0 { Vec2::ZERO } else { (to - from) * (1.0 / (f64::from(span) * TICK)) };
        match self.legs.last_mut() {
            Some(leg) if leg.flight.velocity == velocity && leg.offset + leg.to == now => leg.to += span,
            _ => self.legs.push(Leg {
                flight: Flight {
                    origin: from,
                    velocity,
                    accel: Vec2::ZERO,
                },
                from: 0,
                to: span,
                offset: now,
            }),
        }
    }

    /// Stays at `at` for `span` ticks.
    fn wait(&mut self, at: Vec2, now: u32, span: u32) {
        self.go(at, at, now, span);
    }

    /// The legs, ending with a rest at `at` from `now`.
    fn finish(mut self, at: Vec2, now: u32) -> Vec<Leg> {
        self.wait(at, now, 0);
        self.legs
    }
}

#[cfg(test)]
mod unit {
    // Exact float equality is the claim: walkers stand on pixel columns, and their legs are built
    // from those points.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::units::TICKS_PER_SECOND;

    const GAIT: Gait = Gait { speed: 64.0, climb: 6, height: 4.0 };

    /// Level ground from row 100 down, on a 400 x 200 map.
    fn flat() -> Terrain {
        Terrain::from_fn(400, 200, |_, y| y >= 100).unwrap()
    }

    fn at(x: i32) -> Footing {
        Footing { x, y: 100 }
    }

    /// Where a walk is at stage tick `tick`, by its legs.
    fn played(legs: &[Leg], tick: u32) -> Vec2 {
        let leg = legs.iter().rfind(|leg| leg.offset <= tick).unwrap();
        leg.flight.at((tick - leg.offset).min(leg.to))
    }

    #[test]
    fn a_crawler_walks_toward_its_column_and_stops_there() {
        let walked = walk(
            &flat(),
            Start {
                at: at(100),
                facing: Facing::Left,
                tick: 0,
            },
            &GAIT,
            Heading::Toward(130),
            90,
            |_| None,
        );
        assert_eq!(walked.footing, at(130));
        assert_eq!(walked.facing, Facing::Right);
        assert_eq!(walked.end.contact, Contact::Terrain);
        // 30 columns at 32 ticks each.
        assert_eq!(walked.end.tick, 30 * 32);
        assert_eq!(played(&walked.legs, 15 * 32), GAIT.middle(at(115)));
        assert_eq!(played(&walked.legs, walked.end.tick), walked.end.at);
    }

    #[test]
    fn a_crawler_runs_out_of_steps() {
        let walked = walk(
            &flat(),
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 64,
            },
            &GAIT,
            Heading::Toward(300),
            90,
            |_| None,
        );
        assert_eq!(walked.footing, at(190));
        assert_eq!(walked.end.tick, 64 + 90 * 32);
        assert_eq!(walked.legs[0].offset, 64, "it starts at its stage tick");
    }

    #[test]
    fn a_wall_stops_a_crawler_and_turns_a_marcher() {
        // A 20 px wall at x = 150.
        let terrain = Terrain::from_fn(400, 200, |x, y| y >= 100 || (x >= 150 && y >= 80)).unwrap();
        let crawled = walk(
            &terrain,
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Toward(300),
            90,
            |_| None,
        );
        assert_eq!(crawled.footing, at(149));
        let marched = walk(
            &terrain,
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Ahead,
            90,
            |_| None,
        );
        assert_eq!(marched.facing, Facing::Left);
        // 49 steps to the wall, one to turn, and 40 back.
        assert_eq!(marched.footing, at(109));
        assert_eq!(marched.end.tick, 90 * 32);
    }

    #[test]
    fn it_climbs_what_a_mobile_climbs() {
        let terrain = Terrain::from_fn(400, 200, |x, y| y >= 100 || (x >= 150 && y >= 95)).unwrap();
        let walked = walk(
            &terrain,
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Toward(200),
            200,
            |_| None,
        );
        assert_eq!(walked.footing, Footing { x: 200, y: 95 });
    }

    #[test]
    fn it_stops_inside_a_mobile_it_touches() {
        let walked = walk(
            &flat(),
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Ahead,
            90,
            |middle| (middle.x > 140.0).then_some(3),
        );
        assert_eq!(walked.end.contact, Contact::Mobile(3));
        assert_eq!(walked.footing, at(140));
    }

    #[test]
    fn it_drops_off_a_ledge_and_walks_on() {
        // A 30 px drop at x = 150.
        let terrain = Terrain::from_fn(400, 200, |x, y| if x >= 150 { y >= 130 } else { y >= 100 }).unwrap();
        let walked = walk(
            &terrain,
            Start {
                at: at(140),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Toward(170),
            90,
            |_| None,
        );
        assert_eq!(walked.footing, Footing { x: 170, y: 130 });
        let path: Vec<f64> = (0..walked.end.tick).step_by(8).map(|tick| played(&walked.legs, tick).y).collect();
        assert!(path.windows(2).all(|w| w[1] >= w[0]), "only down: {path:?}");
    }

    #[test]
    fn it_walks_off_the_bottom_of_the_map() {
        let terrain = Terrain::from_fn(400, 200, |x, y| x < 150 && y >= 100).unwrap();
        let walked = walk(
            &terrain,
            Start {
                at: at(140),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Ahead,
            90,
            |_| None,
        );
        assert_eq!(walked.end.contact, Contact::Gone);
        assert!(walked.end.at.y >= 190.0, "{:?}", walked.end);
        assert!(walked.end.tick < 90 * 32 + TICKS_PER_SECOND);
    }

    #[test]
    fn boxed_in_a_marcher_turns_round_until_its_steps_run_out() {
        let terrain = Terrain::from_fn(400, 200, |x, y| y >= 100 || x == 99 || x == 101).unwrap();
        let walked = walk(
            &terrain,
            Start {
                at: at(100),
                facing: Facing::Right,
                tick: 0,
            },
            &GAIT,
            Heading::Ahead,
            10,
            |_| None,
        );
        assert_eq!(walked.footing, at(100));
        assert_eq!(walked.end.tick, 10 * 32);
        assert!(walked.legs.iter().all(|leg| leg.flight.velocity == Vec2::ZERO));
    }
}
