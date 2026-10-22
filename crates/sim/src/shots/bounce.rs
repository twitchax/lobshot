//! Bouncing balls (Marblemole's S2). Each flies until it touches something: off the ground and the
//! map's sides it bounces, losing speed, until it is slow enough to rest; its fuse lights at its
//! first bounce, and it goes off when the fuse burns down, or at once on touching a mobile. The
//! balls fly together, event by event in tick order, so one going off carves the ground the others
//! bounce on, and a resting ball whose ground is blown away falls.

use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Leg, Track},
    terrain::Terrain,
    units::Vec2,
    weather::Marks,
};

/// How far around a bounce to look for the ground's slope, px.
const NORMAL_RADIUS: i32 = 3;

/// How balls bounce.
pub(super) struct Bounce {
    /// The share of its speed into a surface a ball keeps, bouncing back out.
    pub restitution: f64,
    /// The share of its speed along a surface a ball keeps through a bounce.
    pub friction: f64,
    /// Below this speed after a bounce (px/s), a ball rests until its fuse.
    pub rest_speed: f64,
    /// Ticks from a ball's first bounce until it goes off.
    pub fuse: u32,
    /// After this many bounces a ball rests where it is.
    pub max_bounces: u8,
}

#[derive(Debug, Clone, Copy)]
enum Motion {
    /// Flying `flight` from its own tick `from`; its tick 0 was stage tick `offset`.
    Flying {
        flight: Flight,
        from: u32,
        offset: u32,
    },
    /// Still at `at` since stage tick `since`.
    Resting {
        at: Vec2,
        since: u32,
    },
    Done,
}

struct Ball {
    legs: Vec<Leg>,
    motion: Motion,
    /// The stage tick it goes off at, once lit.
    fuse: Option<u32>,
    bounces: u8,
    /// What the columns it passed through did to it.
    marks: Marks,
    /// Gravity and wind on it, for falling again from rest.
    accel: Vec2,
}

/// What a ball does next.
enum Next {
    /// Its fuse burns down; the flight it is on, if it is flying.
    Fuse(Option<Track>),
    /// It touches something.
    Contact(Track),
}

/// Flies the balls, landing each one (`land`, with its index, where it went off, and what the
/// columns it passed through did to it) as it goes off. Returns each ball's path, by index, and the landings' and
/// bounces' effects in tick order.
pub(super) fn roll(game: &mut Game, flights: &[Flight], bounce: &Bounce, mut land: impl FnMut(&mut Game, usize, &Impact, Marks, &mut Vec<Effect>)) -> (Vec<Vec<Leg>>, Vec<Effect>) {
    let mut balls: Vec<Ball> = flights
        .iter()
        .map(|&flight| Ball {
            legs: Vec::new(),
            motion: Motion::Flying { flight, from: 0, offset: 0 },
            fuse: None,
            bounces: 0,
            marks: Marks::default(),
            accel: flight.accel,
        })
        .collect();
    let mut effects = Vec::new();
    // Each ball's next event against the ground as it stands, cleared whenever a blast changes it.
    let mut upcoming: Vec<Option<(u32, Next)>> = balls.iter().map(|_| None).collect();
    let most = balls.len() * (usize::from(bounce.max_bounces) + 4);
    for _ in 0..most {
        for (ball, next) in balls.iter().zip(upcoming.iter_mut()) {
            if next.is_none() {
                *next = next_event(game, ball);
            }
        }
        let first = upcoming.iter().enumerate().filter_map(|(index, next)| next.as_ref().map(|(tick, _)| (*tick, index))).min();
        let Some((tick, index)) = first else { break };
        let Some((_, next)) = upcoming[index].take() else { break };
        let ball = &mut balls[index];
        let went_off = match next {
            Next::Fuse(track) => Some(go_off(ball, tick, track)),
            Next::Contact(track) => touch(game, ball, &track, bounce, &mut effects),
        };
        if let Some(impact) = went_off {
            land(game, index, &impact, ball.marks, &mut effects);
            for next in &mut upcoming {
                *next = None;
            }
            for ball in &mut balls {
                unsettle(game.terrain(), ball, tick);
            }
        }
    }
    (balls.into_iter().map(|ball| ball.legs).collect(), effects)
}

fn next_event(game: &Game, ball: &Ball) -> Option<(u32, Next)> {
    match ball.motion {
        Motion::Done => None,
        // A ball only rests after a bounce, so its fuse is lit.
        Motion::Resting { .. } => ball.fuse.map(|fuse| (fuse, Next::Fuse(None))),
        Motion::Flying { flight, from, offset } => {
            let track = game.track(flight, from, offset);
            Some(match ball.fuse {
                Some(fuse) if fuse <= track.impact.tick => (fuse, Next::Fuse(Some(track))),
                _ => (track.impact.tick, Next::Contact(track)),
            })
        }
    }
}

/// The ball's fuse burns down at `tick`: it goes off where it is.
fn go_off(ball: &mut Ball, tick: u32, track: Option<Track>) -> Impact {
    let at = match (ball.motion, track) {
        (Motion::Resting { at, since }, _) => {
            ball.legs.push(rest(at, since, tick));
            at
        }
        (Motion::Flying { flight, from, .. }, Some(track)) => {
            let mut at = flight.at(from);
            for leg in track.legs.into_iter().filter(|leg| leg.offset + leg.from <= tick) {
                let leg = Leg { to: leg.to.min(tick - leg.offset), ..leg };
                at = leg.flight.at(leg.to);
                ball.legs.push(leg);
            }
            at
        }
        (Motion::Flying { flight, from, .. }, None) => flight.at(from),
        (Motion::Done, _) => Vec2::ZERO,
    };
    ball.motion = Motion::Done;
    Impact { tick, at, contact: Contact::Terrain }
}

/// The ball touches what `track` ran into: a mobile sets it off; the ground or a side of the map
/// bounces it; the map's bottom loses it. Returns where it went off, if it did.
fn touch(game: &Game, ball: &mut Ball, track: &Track, bounce: &Bounce, effects: &mut Vec<Effect>) -> Option<Impact> {
    ball.marks |= track.marks;
    ball.legs.extend_from_slice(&track.legs);
    let impact = track.impact;
    let (last, tick) = (track.last, track.last_tick);
    let velocity = last.flight.velocity_at(tick);
    // Where it was a tick before the touch: in the open, to bounce from.
    let start = last.flight.at(tick.saturating_sub(1).max(last.from));
    let width = f64::from(game.terrain().width());
    let out = match impact.contact {
        Contact::Mobile(_) => {
            ball.motion = Motion::Done;
            return Some(impact);
        }
        Contact::Terrain => reflect(velocity, surface_normal(game.terrain(), impact.at), bounce),
        Contact::Gone if impact.at.y < f64::from(game.terrain().height()) && (impact.at.x < 0.0 || impact.at.x >= width) => Vec2::new(-velocity.x * bounce.restitution, velocity.y),
        Contact::Gone | Contact::Column(_) => {
            ball.motion = Motion::Done;
            return None;
        }
    };
    effects.push(Effect::Bounced {
        at: Vec2::new(impact.at.x.clamp(0.0, width), impact.at.y),
        tick: impact.tick,
    });
    ball.bounces += 1;
    ball.fuse.get_or_insert(impact.tick + bounce.fuse);
    ball.motion = if out.length() < bounce.rest_speed || ball.bounces >= bounce.max_bounces {
        Motion::Resting { at: start, since: impact.tick }
    } else {
        Motion::Flying {
            flight: Flight {
                origin: start,
                velocity: out,
                accel: last.flight.accel,
            },
            from: 0,
            offset: impact.tick,
        }
    };
    None
}

/// `velocity` off a surface with outward `normal`: the part into it reversed and damped, the part
/// along it slowed.
fn reflect(velocity: Vec2, normal: Vec2, bounce: &Bounce) -> Vec2 {
    let into = velocity.x * normal.x + velocity.y * normal.y;
    // A slope read from a few pixels can face the wrong way; then it bounces straight back.
    let (normal, into) = if into < 0.0 {
        (normal, into)
    } else {
        let speed = velocity.length();
        if speed <= 0.0 {
            return Vec2::ZERO;
        }
        (Vec2::new(-velocity.x / speed, -velocity.y / speed), -speed)
    };
    let head_on = normal * into;
    let along = velocity - head_on;
    along * bounce.friction - head_on * bounce.restitution
}

/// The ground's outward normal near `at`: away from the solid pixels around it.
fn surface_normal(terrain: &Terrain, at: Vec2) -> Vec2 {
    // A touch is inside the map, so its pixel is in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (at.x.floor() as i32, at.y.floor() as i32);
    let mut away = Vec2::ZERO;
    for dy in -NORMAL_RADIUS..=NORMAL_RADIUS {
        for dx in -NORMAL_RADIUS..=NORMAL_RADIUS {
            if dx * dx + dy * dy <= NORMAL_RADIUS * NORMAL_RADIUS && terrain.is_solid(x + dx, y + dy) {
                away = away - Vec2::new(f64::from(dx), f64::from(dy));
            }
        }
    }
    let length = away.length();
    if length > 0.0 { Vec2::new(away.x / length, away.y / length) } else { Vec2::new(0.0, -1.0) }
}

/// A leg standing still at `at` from stage tick `since` to `until`.
fn rest(at: Vec2, since: u32, until: u32) -> Leg {
    Leg {
        flight: Flight {
            origin: at,
            velocity: Vec2::ZERO,
            accel: Vec2::ZERO,
        },
        from: 0,
        to: until - since,
        offset: since,
    }
}

/// A resting ball whose ground a blast at `tick` took away falls from where it was.
fn unsettle(terrain: &Terrain, ball: &mut Ball, tick: u32) {
    let Motion::Resting { at, since } = ball.motion else { return };
    // Resting is inside the map, so its pixel is in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (at.x.floor() as i32, at.y.floor() as i32);
    if (1..=2).any(|below| terrain.is_solid(x, y + below)) {
        return;
    }
    ball.legs.push(rest(at, since, tick));
    ball.motion = Motion::Flying {
        flight: Flight {
            origin: at,
            velocity: Vec2::ZERO,
            accel: ball.accel,
        },
        from: 0,
        offset: tick,
    };
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const LIVELY: Bounce = Bounce {
        restitution: 0.5,
        friction: 0.8,
        rest_speed: 40.0,
        fuse: 6144,
        max_bounces: 16,
    };

    fn close(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn a_bounce_reverses_and_damps_the_speed_into_the_ground_and_slows_the_rest() {
        let up = Vec2::new(0.0, -1.0);
        assert!(close(reflect(Vec2::new(100.0, 200.0), up, &LIVELY), Vec2::new(80.0, -100.0)));
        // A normal facing the wrong way bounces it straight back.
        assert!(close(reflect(Vec2::new(0.0, 200.0), Vec2::new(0.0, 1.0), &LIVELY), Vec2::new(0.0, -100.0)));
    }

    #[test]
    fn the_ground_faces_away_from_its_solid_pixels() {
        let flat = Terrain::from_fn(100, 100, |_, y| y >= 50).unwrap();
        assert!(close(surface_normal(&flat, Vec2::new(40.5, 50.5)), Vec2::new(0.0, -1.0)));
        let wall = Terrain::from_fn(100, 100, |x, _| x >= 50).unwrap();
        assert!(close(surface_normal(&wall, Vec2::new(50.5, 40.5)), Vec2::new(-1.0, 0.0)));
    }
}
