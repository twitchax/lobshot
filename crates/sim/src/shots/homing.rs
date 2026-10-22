//! Homing missiles (Herdhound's shots). A warhead that passes through mobiles flies to its apex
//! and opens there (`open`): a guide flies on, and missiles burst out of the opening point. Each
//! loops at a steady turn for a while, then homes on its guide, turning toward where the guide is
//! as fast as it may. Missiles are rockets at a steady speed, which neither gravity nor wind moves;
//! one carrying several (the SS's) splits as it stops looping, each half homing on a guide of its
//! own. The missiles fly together, a short steering step at a time, each step a leg under the
//! constant acceleration that turns its velocity by that step's turn, so the sim tracks them
//! through the weather like any shot and clients draw them from their legs.

use crate::{
    ballistics::Flight,
    game::{Contact, Course, Effect, Game, Impact, Leg, Senses, Track},
    units::{TICK, Vec2},
    weather::Marks,
};

/// Ticks per steering step: 1/64 s, so a loop at two turns a second is drawn in 32 chords.
pub(super) const STEP: u32 = 32;

/// How a warhead ended.
pub(super) enum Opening {
    /// It opened at its apex. Its path there; it carries on (as the guide) from `last` at its
    /// own tick `last_tick`.
    Opened(Track),
    /// It touched the ground before its apex, or left the map: its whole path.
    Dud(Track),
}

/// Flies a warhead, which passes through mobiles, to its apex: the first tick it is no longer
/// climbing, which is the opening tick if it is already falling (a downward shot opens at once).
pub(super) fn open(game: &Game, flight: Flight) -> Opening {
    let through = Senses { mobiles: false, terrain: true };
    let whole = game.track_sensing(flight, 0, 0, through);
    let Some(apex) = whole.legs.iter().find_map(apex).filter(|&tick| tick < whole.impact.tick) else {
        return Opening::Dud(whole);
    };
    let course = game.track_until(flight, 0, 0, through, apex);
    if course.flying { Opening::Opened(course.track) } else { Opening::Dud(course.track) }
}

/// The stage tick `leg` stops climbing at, if it does before it ends.
fn apex(leg: &Leg) -> Option<u32> {
    let climbing = |tick: u32| leg.flight.velocity_at(tick).y < 0.0;
    if !climbing(leg.from) {
        return Some(leg.offset + leg.from);
    }
    let lift = leg.flight.accel.y;
    if lift <= 0.0 {
        return None;
    }
    // Closed form, clamped to the leg, then on past any rounding.
    let estimate = (-leg.flight.velocity.y / (lift * TICK)).ceil().clamp(f64::from(leg.from), f64::from(leg.to) + 1.0);
    // Clamped to the leg's own ticks, a u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut tick = estimate as u32;
    while tick <= leg.to && climbing(tick) {
        tick += 1;
    }
    (tick <= leg.to).then_some(leg.offset + tick)
}

/// How the missiles fly.
pub(super) struct Herd {
    /// px/s, always.
    pub speed: f64,
    /// Steering steps they loop for once they burst out.
    pub loop_steps: u32,
    /// How fast they turn while looping, radians per second.
    pub loop_turn: f64,
    /// The most a homing missile turns, radians per second.
    pub homing_turn: f64,
    /// Ticks after bursting out that a missile which has hit nothing goes off where it is.
    pub lifetime: u32,
}

/// One missile as it bursts out of the warhead.
pub(super) struct Missile {
    /// Its heading off the warhead's, radians, upward (toward the sky as the shot flies forward):
    /// it loops that way round.
    pub heading: f64,
    /// What it becomes once it stops looping: `(heading off its own, guide)` for each missile it
    /// splits into, or one entry, itself homing on that guide (its heading unused).
    pub homes: Vec<(f64, usize)>,
}

/// A missile in flight.
struct Body {
    at: Vec2,
    velocity: Vec2,
    /// The stage tick it has flown to (past the current step's while a tornado holds it).
    time: u32,
    /// +1 loops counterclockwise on screen, -1 clockwise.
    spin: f64,
    marks: Marks,
    /// `(heading, guide, missile)`: the missiles it carries, numbered across the shot.
    carries: Vec<(f64, usize, usize)>,
    legs: Vec<Leg>,
    live: bool,
}

/// Flies the missiles out of `opened` (`Opening::Opened`), each homing on its guide among
/// `guides`, and lands each (`land`, with its number across the shot, its impact, and what the
/// columns the warhead and it passed through did to it) as it strikes, in tick order: a later one
/// meets the craters, and the deaths, of the earlier. `forward` is the shot's way, for a warhead
/// that opened standing still. Returns each missile's path in the order they flew (those that
/// split first, then their halves) and the landings' effects.
pub(super) fn fly(
    game: &mut Game,
    herd: &Herd,
    opened: &Track,
    guides: &[Track],
    missiles: &[Missile],
    forward: Vec2,
    mut land: impl FnMut(&mut Game, usize, &Impact, Marks, &mut Vec<Effect>),
) -> (Vec<Vec<Leg>>, Vec<Effect>) {
    let (last, tick) = (opened.last, opened.last_tick);
    let born = last.offset + tick;
    let moving = last.flight.velocity_at(tick);
    let way = if moving.length() > 0.0 { moving * (1.0 / moving.length()) } else { forward };
    // Headings are upward off the way it flies: counterclockwise on screen when it flies right.
    let side = if way.x < 0.0 { -1.0 } else { 1.0 };
    let mut count = 0;
    let mut bodies: Vec<Body> = missiles
        .iter()
        .map(|missile| {
            let carries = missile
                .homes
                .iter()
                .map(|&(heading, guide)| {
                    count += 1;
                    (side * heading, guide, count - 1)
                })
                .collect();
            Body {
                at: last.flight.at(tick),
                velocity: way.rotated(side * missile.heading) * herd.speed,
                time: born,
                spin: if missile.heading < 0.0 { -side } else { side },
                marks: opened.marks,
                carries,
                legs: Vec::new(),
                live: true,
            }
        })
        .collect();

    let loop_end = born + herd.loop_steps * STEP;
    let deadline = born.saturating_add(herd.lifetime);
    let mut effects = Vec::new();
    let mut window = born;
    while bodies.iter().any(|body| body.live) {
        let end = window + STEP;
        split(&mut bodies, loop_end);
        let mut steps: Vec<Option<Course>> = bodies.iter().map(|body| step(game, herd, body, guides, loop_end, end)).collect();
        // The earliest strike first; it changes the ground and the mobiles, so the others that
        // strike this step fly it again.
        while let Some((_, index)) = steps
            .iter()
            .enumerate()
            .filter_map(|(index, course)| course.as_ref().filter(|course| !course.flying).map(|course| (course.track.impact.tick, index)))
            .min()
        {
            let Some(course) = steps[index].take() else { break };
            let body = &mut bodies[index];
            body.legs.extend_from_slice(&course.track.legs);
            body.live = false;
            let marks = body.marks | course.track.marks;
            for &(_, _, missile) in &body.carries {
                land(game, missile, &course.track.impact, marks, &mut effects);
            }
            for (body, course) in bodies.iter().zip(steps.iter_mut()) {
                if course.as_ref().is_some_and(|course| !course.flying) {
                    *course = step(game, herd, body, guides, loop_end, end);
                }
            }
        }
        for (body, course) in bodies.iter_mut().zip(steps) {
            if let Some(course) = course {
                carry_on(body, &course, herd.speed);
            }
        }
        for body in bodies.iter_mut().filter(|body| body.live && body.time >= deadline) {
            body.live = false;
            let impact = Impact {
                tick: body.time,
                at: body.at,
                contact: Contact::Terrain,
            };
            for &(_, _, missile) in &body.carries {
                land(game, missile, &impact, body.marks, &mut effects);
            }
        }
        window = end;
    }
    (bodies.into_iter().map(|body| body.legs).collect(), effects)
}

/// Splits each missile done looping that carries several into its halves, which fly on from
/// where it is.
fn split(bodies: &mut Vec<Body>, loop_end: u32) {
    let mut halves = Vec::new();
    for body in bodies.iter_mut().filter(|body| body.live && body.carries.len() > 1 && body.time >= loop_end) {
        body.live = false;
        halves.extend(body.carries.iter().map(|&(heading, guide, missile)| Body {
            at: body.at,
            velocity: body.velocity.rotated(heading),
            time: body.time,
            spin: body.spin,
            marks: body.marks,
            carries: vec![(0.0, guide, missile)],
            legs: Vec::new(),
            live: true,
        }));
    }
    bodies.append(&mut halves);
}

/// One body's steering step to stage tick `end`, if it is flying and due: a steady turn while it
/// loops, then as hard toward its guide as it may.
fn step(game: &Game, herd: &Herd, body: &Body, guides: &[Track], loop_end: u32, end: u32) -> Option<Course> {
    if !body.live || body.time >= end {
        return None;
    }
    let seconds = f64::from(end - body.time) * TICK;
    let turn = if body.time < loop_end {
        body.spin * herd.loop_turn * seconds
    } else {
        let most = herd.homing_turn * seconds;
        let target = body.carries.first().and_then(|&(_, guide, _)| guides.get(guide)).map_or(body.at, |guide| guide_at(guide, body.time));
        angle(body.velocity, target - body.at).clamp(-most, most)
    };
    let turned = body.velocity.rotated(turn);
    let flight = Flight {
        origin: body.at,
        velocity: body.velocity,
        accel: (turned - body.velocity) * (1.0 / seconds),
    };
    Some(game.track_until(flight, 0, body.time, Senses::ALL, end))
}

/// Moves a body on to where its step left it, flying at its steady speed (a tornado's fling or a
/// leg's chord can leave it a little off).
fn carry_on(body: &mut Body, course: &Course, speed: f64) {
    let track = &course.track;
    body.legs.extend_from_slice(&track.legs);
    body.marks |= track.marks;
    body.at = track.last.flight.at(track.last_tick);
    let velocity = track.last.flight.velocity_at(track.last_tick);
    let length = velocity.length();
    if length > 0.0 {
        body.velocity = velocity * (speed / length);
    }
    body.time = track.last.offset + track.last_tick;
}

/// Where a guide is at stage tick `tick`: on its path, held where a tornado holds it, and where it
/// stopped once it has.
pub(super) fn guide_at(guide: &Track, tick: u32) -> Vec2 {
    let mut at = guide.legs.first().map_or(guide.impact.at, |leg| leg.flight.at(leg.from));
    for leg in &guide.legs {
        if tick < leg.offset + leg.from {
            return at;
        }
        if tick <= leg.offset + leg.to {
            return leg.flight.at(tick - leg.offset);
        }
        at = leg.flight.at(leg.to);
    }
    guide.impact.at
}

/// The angle from `from` to `to`, radians counterclockwise on screen (`y` down), in -π..=π.
fn angle(from: Vec2, to: Vec2) -> f64 {
    libm::atan2(from.y * to.x - from.x * to.y, from.x * to.x + from.y * to.y)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::units::to_radians;

    #[test]
    fn angles_count_counterclockwise_on_screen() {
        let right = Vec2::new(1.0, 0.0);
        assert!((angle(right, Vec2::new(0.0, -1.0)) - to_radians(90.0)).abs() < 1e-12, "up is a quarter turn counterclockwise");
        assert!((angle(right, Vec2::new(0.0, 1.0)) + to_radians(90.0)).abs() < 1e-12);
        // The same sense `Vec2::rotated` turns by.
        let turned = right.rotated(0.4);
        assert!((angle(right, turned) - 0.4).abs() < 1e-12);
    }

    fn lob(velocity: Vec2) -> Leg {
        Leg {
            flight: Flight {
                origin: Vec2::ZERO,
                velocity,
                accel: Vec2::new(0.0, 600.0),
            },
            from: 0,
            to: 4096,
            offset: 100,
        }
    }

    #[test]
    fn a_lob_tops_out_where_it_stops_climbing() {
        let leg = lob(Vec2::new(200.0, -300.0));
        let tick = apex(&leg).unwrap() - leg.offset;
        assert!(leg.flight.velocity_at(tick).y >= 0.0 && leg.flight.velocity_at(tick - 1).y < 0.0);
        // 300 px/s up under 600 px/s² stops climbing half a second in.
        assert_eq!(tick, 1024);
        assert_eq!(apex(&lob(Vec2::new(200.0, 50.0))), Some(100), "falling already: it opens at once");
        assert_eq!(apex(&lob(Vec2::new(0.0, -5000.0))), None, "still climbing when the leg ends");
    }

    #[test]
    fn a_guide_holds_where_a_tornado_holds_it_and_rests_where_it_stopped() {
        let first = Leg {
            to: 1000,
            offset: 0,
            ..lob(Vec2::new(100.0, 0.0))
        };
        let second = Leg {
            from: 0,
            to: 500,
            offset: 3000,
            ..lob(Vec2::new(-100.0, -50.0))
        };
        let guide = Track {
            legs: vec![first, second],
            impact: Impact {
                tick: 3500,
                at: second.flight.at(500),
                contact: Contact::Terrain,
            },
            marks: Marks::default(),
            last: second,
            last_tick: 500,
        };
        assert_eq!(guide_at(&guide, 0), first.flight.at(0));
        assert_eq!(guide_at(&guide, 400), first.flight.at(400));
        assert_eq!(guide_at(&guide, 2000), first.flight.at(1000), "held");
        assert_eq!(guide_at(&guide, 3200), second.flight.at(200));
        assert_eq!(guide_at(&guide, 9000), guide.impact.at);
    }
}
