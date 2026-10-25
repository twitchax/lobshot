//! Shots fly a closed-form parabola under constant acceleration (gravity plus wind), sampled at
//! fixed ticks. Closed form from an integer tick count is the same bits everywhere, and every
//! consumer (server, client, bots) steps it identically. See `docs/research/physics-fit.md`.

use serde::{Deserialize, Serialize};

use crate::{
    tuning::{LAUNCH_SPEED_PER_BAR, WIND_X_PER_UNIT, WIND_Y_PER_UNIT},
    units::{TICK, Vec2, Wind, seconds},
};

/// A projectile's flight: where it started, how fast, and the constant acceleration on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub origin: Vec2,
    pub velocity: Vec2,
    pub accel: Vec2,
}

impl Flight {
    /// Position after `tick` ticks.
    #[must_use]
    pub fn at(&self, tick: u32) -> Vec2 {
        let t = f64::from(tick) * TICK;
        Vec2::new(
            self.origin.x + self.velocity.x * t + 0.5 * self.accel.x * t * t,
            self.origin.y + self.velocity.y * t + 0.5 * self.accel.y * t * t,
        )
    }

    /// The corners of a box holding every point of the flight from tick `from` to tick `to`, ticks
    /// between included: each axis's ends, and its turning point if it turns in between. Exact up to
    /// rounding, so a caller that needs certainty widens it a little.
    #[must_use]
    pub fn bounds(&self, from: u32, to: u32) -> (Vec2, Vec2) {
        let (t0, t1) = (f64::from(from) * TICK, f64::from(to) * TICK);
        let axis = |origin: f64, velocity: f64, accel: f64| {
            let at = |t: f64| origin + velocity * t + 0.5 * accel * t * t;
            let (a, b) = (at(t0), at(t1));
            let (mut low, mut high) = (a.min(b), a.max(b));
            if accel != 0.0 {
                let turn = -velocity / accel;
                if t0 < turn && turn < t1 {
                    low = low.min(at(turn));
                    high = high.max(at(turn));
                }
            }
            (low, high)
        };
        let (x, y) = (axis(self.origin.x, self.velocity.x, self.accel.x), axis(self.origin.y, self.velocity.y, self.accel.y));
        (Vec2::new(x.0, y.0), Vec2::new(x.1, y.1))
    }

    /// Velocity after `tick` ticks.
    #[must_use]
    pub fn velocity_at(&self, tick: u32) -> Vec2 {
        let t = f64::from(tick) * TICK;
        Vec2::new(self.velocity.x + self.accel.x * t, self.velocity.y + self.accel.y * t)
    }
}

/// Where a helix strand winding about `flight` is at the flight's `tick`: the flight's point,
/// pushed `amplitude · sin(2π · hertz · t + phase)` along its normal (the velocity turned a quarter
/// clockwise on screen), where `t` is the shot's elapsed time: `elapsed` ticks at the flight's tick
/// 0, plus `tick`. From elapsed tick `merge` on, if there is one, the strand runs on the path
/// itself (Spoutpot's S2 comes together). The sim collides strands here, and clients draw them
/// here.
#[must_use]
pub fn helix_point(flight: &Flight, tick: u32, elapsed: u32, amplitude: f64, hertz: f64, phase: f64, merge: Option<u32>) -> Vec2 {
    let at = flight.at(tick);
    if merge.is_some_and(|merge| elapsed + tick >= merge) {
        return at;
    }
    let velocity = flight.velocity_at(tick);
    let speed = velocity.length();
    if speed <= 0.0 {
        return at;
    }
    let normal = Vec2::new(-velocity.y / speed, velocity.x / speed);
    let t = seconds(elapsed + tick);
    at + normal * (amplitude * libm::sin(2.0 * core::f64::consts::PI * hertz * t + phase))
}

/// How an orbit's projectiles circle its path (`orbit_point`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    /// Turns per second, clockwise on screen whichever way the shot flies.
    pub hertz: f64,
    /// Seconds each orbiter takes to swing out from the path to its full radius.
    pub ramp: f64,
    /// The shot's elapsed tick its circling stops at (a tornado caught it), each orbiter holding
    /// its place about the path from then on; `u32::MAX` if it never stops.
    pub until: u32,
}

/// Where an orbiter circling `flight` is at the flight's `tick`: the flight's point, plus
/// `radius · (cos a, sin a)` on screen with `a = phase + 2π · hertz · s` (clockwise, since screen y
/// points down), where `s` is the shot's spinning time: `elapsed` ticks at the flight's tick 0,
/// plus `tick`, up to `spin.until`. The radius swings out from 0 over the first `spin.ramp`
/// seconds; an orbiter with no radius flies the path itself. The sim collides orbiters here, and
/// clients draw them here.
#[must_use]
pub fn orbit_point(flight: &Flight, tick: u32, elapsed: u32, spin: &Spin, radius: f64, phase: f64) -> Vec2 {
    let s = seconds(elapsed.saturating_add(tick).min(spin.until));
    let reach = if spin.ramp > 0.0 { radius * (s / spin.ramp).min(1.0) } else { radius };
    let angle = phase + 2.0 * core::f64::consts::PI * spin.hertz * s;
    flight.at(tick) + Vec2::new(libm::cos(angle), libm::sin(angle)) * reach
}

/// Launch speed for a power in bars (continuous, so bots and tests can search between steps).
#[must_use]
pub fn launch_speed(bars: f64) -> f64 {
    bars * LAUNCH_SPEED_PER_BAR.value
}

/// The acceleration on a shot: the shooter's gravity, plus wind scaled by the same gravity (so the
/// community's wind rules hold for every mobile) and by the mobile's wind sensitivity.
#[must_use]
pub fn acceleration(gravity: f64, wind: Wind, sensitivity: f64) -> Vec2 {
    let (across, up) = wind.components();
    Vec2::new(across * WIND_X_PER_UNIT.value * gravity * sensitivity, gravity - up * WIND_Y_PER_UNIT.value * gravity * sensitivity)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::units::{TICKS_PER_SECOND, to_radians};

    #[test]
    fn a_vacuum_shot_lands_where_the_range_formula_says() {
        let (speed, angle, g) = (735.0, to_radians(60.0), 585.0);
        let flight = Flight {
            origin: Vec2::ZERO,
            velocity: Vec2::new(speed * libm::cos(angle), -speed * libm::sin(angle)),
            accel: Vec2::new(0.0, g),
        };
        let range = speed * speed * libm::sin(2.0 * angle) / g;
        let air_time = 2.0 * speed * libm::sin(angle) / g;
        // Sample the tick nearest landing.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let tick = (air_time * f64::from(TICKS_PER_SECOND)).round() as u32;
        let landing = flight.at(tick);
        assert!((landing.x - range).abs() < 0.5, "{landing:?} vs {range}");
        assert!(landing.y.abs() < 0.5, "{landing:?}");
    }

    #[test]
    fn bounds_hold_every_tick_and_the_top_of_the_arc() {
        let flight = Flight {
            origin: Vec2::new(100.0, 500.0),
            velocity: Vec2::new(-250.0, -600.0),
            accel: Vec2::new(40.0, 620.0),
        };
        for (from, to) in [(0, 0), (0, 64), (1500, 2100), (900, 4000), (0, 8000)] {
            let (low, high) = flight.bounds(from, to);
            let ticks: Vec<Vec2> = (from..=to).map(|tick| flight.at(tick)).collect();
            for at in &ticks {
                assert!(
                    at.x >= low.x - 1e-9 && at.x <= high.x + 1e-9 && at.y >= low.y - 1e-9 && at.y <= high.y + 1e-9,
                    "{at:?} outside {low:?}..{high:?}"
                );
            }
            // Tight: each side is within a tick's travel of some tick.
            let near = |side: f64, of: fn(&Vec2) -> f64| ticks.iter().any(|at| (of(at) - side).abs() < 1.0);
            assert!(near(low.x, |v| v.x) && near(high.x, |v| v.x) && near(low.y, |v| v.y) && near(high.y, |v| v.y), "{from}..{to}");
        }
    }

    #[test]
    fn helix_strands_run_on_the_path_once_they_merge() {
        let flight = Flight {
            origin: Vec2::ZERO,
            velocity: Vec2::new(300.0, -300.0),
            accel: Vec2::new(0.0, 600.0),
        };
        let quarter = TICKS_PER_SECOND / 4;
        // At a quarter turn a strand is at its widest; from the merge on it is on the path.
        let wide = helix_point(&flight, quarter, 0, 20.0, 1.0, 0.0, Some(2 * quarter));
        assert!((wide.distance(flight.at(quarter)) - 20.0).abs() < 1e-9, "{wide:?}");
        assert_eq!(helix_point(&flight, quarter, quarter, 20.0, 1.0, 0.0, Some(2 * quarter)), flight.at(quarter));
        assert_eq!(helix_point(&flight, 3 * quarter, 0, 20.0, 1.0, 0.0, Some(2 * quarter)), flight.at(3 * quarter));
        assert_ne!(helix_point(&flight, 3 * quarter, 0, 20.0, 1.0, 0.0, None), flight.at(3 * quarter));
    }

    #[test]
    fn an_orbiter_swings_out_and_circles_its_path_clockwise_until_it_stops() {
        let flight = Flight {
            origin: Vec2::ZERO,
            velocity: Vec2::new(300.0, -300.0),
            accel: Vec2::new(0.0, 600.0),
        };
        let spin = Spin { hertz: 0.5, ramp: 0.5, until: u32::MAX };
        let offset = |tick: u32, spin: &Spin| orbit_point(&flight, tick, 0, spin, 35.0, 0.0) - flight.at(tick);
        let second = TICKS_PER_SECOND;
        assert_eq!(offset(0, &spin).length(), 0.0, "it leaves on the path");
        assert!((offset(second / 4, &spin).length() - 17.5).abs() < 1e-9, "half way out at half the ramp");
        // A quarter turn at 0.5 Hz takes half a second: from left of the path (y down) to above it.
        let (a, b) = (offset(second, &spin), offset(second * 3 / 2, &spin));
        assert!((a - Vec2::new(-35.0, 0.0)).length() < 1e-9 && (b - Vec2::new(0.0, -35.0)).length() < 1e-9, "{a:?} {b:?}");
        assert!(a.x * b.y - a.y * b.x > 0.0, "clockwise on screen");
        let stopped = Spin { until: second, ..spin };
        assert!((offset(2 * second, &stopped) - offset(second, &stopped)).length() < 1e-9, "it holds its place once stopped");
        assert_eq!(orbit_point(&flight, 777, 0, &spin, 0.0, 1.0), flight.at(777), "no radius flies the path");
    }

    #[test]
    fn up_wind_lightens_gravity_and_side_wind_pushes_sideways() {
        let calm = acceleration(600.0, Wind::default(), 1.0);
        assert_eq!(calm, Vec2::new(0.0, 600.0));
        let up = acceleration(600.0, Wind::new(10, 90).unwrap(), 1.0);
        assert!(up.y < 600.0 && up.x.abs() < 1e-9);
        let right = acceleration(600.0, Wind::new(10, 0).unwrap(), 2.0);
        assert!(right.x > 0.0 && (right.y - 600.0).abs() < 1e-9);
        assert!((right.x - 2.0 * acceleration(600.0, Wind::new(10, 0).unwrap(), 1.0).x).abs() < 1e-9);
    }
}
