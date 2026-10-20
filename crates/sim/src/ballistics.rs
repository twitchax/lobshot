//! Shots fly a closed-form parabola under constant acceleration (gravity plus wind), sampled at
//! fixed ticks. Closed form from an integer tick count is the same bits everywhere, and every
//! consumer (server, client, bots) steps it identically. See `docs/research/physics-fit.md`.

use serde::{Deserialize, Serialize};

use crate::{
    tuning::{LAUNCH_SPEED_PER_BAR, WIND_X_PER_UNIT, WIND_Y_PER_UNIT},
    units::{TICK, Vec2, Wind},
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

    /// Velocity after `tick` ticks.
    #[must_use]
    pub fn velocity_at(&self, tick: u32) -> Vec2 {
        let t = f64::from(tick) * TICK;
        Vec2::new(self.velocity.x + self.accel.x * t, self.velocity.y + self.accel.y * t)
    }
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
