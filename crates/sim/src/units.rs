//! GunBound's units. One screen is 800 px; the view is 800×600; power is 0 to 4.0 bars in
//! 0.05 steps; aim is in whole degrees; time is seconds, stepped in ticks of 1/2048 s. World
//! coordinates are pixels with `y` pointing down, matching the terrain image.

use std::ops::{Add, Mul, Neg, Sub};

use anyhow::ensure;
use serde::{Deserialize, Serialize};

use crate::base::Res;

/// One GunBound screen: the ruler every community aiming system measures in.
pub const SCREEN: f64 = 800.0;

/// Simulation time step, in seconds. A power of two, so `tick * TICK` is exact in f64.
pub const TICK: f64 = 1.0 / 2048.0;

/// Ticks per second.
pub const TICKS_PER_SECOND: u32 = 2048;

/// Converts degrees to radians with our own constant, so both targets round identically.
#[must_use]
pub fn to_radians(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

/// Converts radians to degrees with our own constant, so both targets round identically.
#[must_use]
pub fn to_degrees(radians: f64) -> f64 {
    radians * (180.0 / core::f64::consts::PI)
}

/// A shot's power: 0 to 4.0 bars in steps of 0.05, stored as twentieths of a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Power(u8);

impl Power {
    /// Full power: 4.0 bars.
    pub const MAX: Self = Self(80);

    /// Power from twentieths of a bar.
    ///
    /// # Errors
    ///
    /// Fails above 4.0 bars.
    pub fn from_twentieths(twentieths: u8) -> Res<Self> {
        ensure!(twentieths <= Self::MAX.0, "power {twentieths}/20 is above 4.0 bars");
        Ok(Self(twentieths))
    }

    /// The nearest representable power to `bars`, clamped to 0..=4.0.
    #[must_use]
    pub fn nearest(bars: f64) -> Self {
        let twentieths = (bars * 20.0).round().clamp(0.0, f64::from(Self::MAX.0));
        // In 0..=80 after the clamp, so the cast is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Self(twentieths as u8)
    }

    #[must_use]
    pub fn twentieths(self) -> u8 {
        self.0
    }

    #[must_use]
    pub fn bars(self) -> f64 {
        f64::from(self.0) / 20.0
    }
}

/// Which way a mobile faces; aim angles are measured from the facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    /// The sign of a step forward along x.
    #[must_use]
    pub fn sign(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }

    #[must_use]
    pub fn signum(self) -> f64 {
        f64::from(self.sign())
    }
}

/// The wind: a strength (GunBound's 0 to ~26 scale) and the direction it blows toward, in
/// degrees counterclockwise from screen-right (90 blows up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Wind {
    pub strength: u8,
    pub direction: u16,
}

impl Wind {
    /// Validates the direction (0..360).
    ///
    /// # Errors
    ///
    /// Fails when the direction is 360 or more.
    pub fn new(strength: u8, direction: u16) -> Res<Self> {
        ensure!(direction < 360, "wind direction {direction} is not in 0..360");
        Ok(Self { strength, direction })
    }

    /// The wind's horizontal and upward components, in wind units.
    #[must_use]
    pub fn components(self) -> (f64, f64) {
        let angle = to_radians(f64::from(self.direction));
        let strength = f64::from(self.strength);
        (strength * libm::cos(angle), strength * libm::sin(angle))
    }
}

/// Turn time in hundredths of a second, as the server measured it (GunBound's timer resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct Centis(pub u32);

/// A point or vector in world pixels, `y` down. Hand-rolled rather than glam so no SIMD or fused
/// operation can sneak into sim math.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        (self - other).length()
    }

    /// Rotates by `radians`, counterclockwise as seen on screen (`y` down).
    #[must_use]
    pub fn rotated(self, radians: f64) -> Self {
        let (sin, cos) = (libm::sin(radians), libm::cos(radians));
        Self::new(self.x * cos + self.y * sin, self.y * cos - self.x * sin)
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for Vec2 {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod unit {
    // Exact float equality is the point here: these values must be bit-exact on every target.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn power_steps_are_twentieths_of_a_bar() {
        assert_eq!(Power::nearest(2.45).twentieths(), 49);
        assert_eq!(Power::nearest(2.45).bars(), 2.45);
        assert_eq!(Power::nearest(9.0), Power::MAX);
        assert_eq!(Power::nearest(-1.0).twentieths(), 0);
        assert!(Power::from_twentieths(81).is_err());
    }

    #[test]
    fn ticks_are_exact_in_f64() {
        let t = f64::from(4_096_u32) * TICK;
        assert_eq!(t, 2.0);
        assert_eq!(f64::from(TICKS_PER_SECOND) * TICK, 1.0);
    }

    #[test]
    fn wind_components_point_where_the_wind_blows() {
        let (x, up) = Wind::new(10, 0).unwrap().components();
        assert_eq!((x, up), (10.0, 0.0));
        let (x, up) = Wind::new(10, 90).unwrap().components();
        assert!(x.abs() < 1e-12 && (up - 10.0).abs() < 1e-12);
        assert!(Wind::new(10, 360).is_err());
    }

    #[test]
    fn rotation_is_counterclockwise_on_screen() {
        // y points down, so rotating screen-right by 90 degrees counterclockwise points up (-y).
        let up = Vec2::new(1.0, 0.0).rotated(to_radians(90.0));
        assert!(up.x.abs() < 1e-12 && (up.y + 1.0).abs() < 1e-12, "{up:?}");
    }
}
