//! Orbiters (Thistleback's S2): projectiles circling one path in the screen's plane, clockwise
//! whichever way the shot flies, each striking on its own; one with no radius flies the path
//! itself. They fly as helix strands do (`helix::weave`), placed by `ballistics::orbit_point`, which
//! clients draw with too. A tornado stops the circling (WP06-list): from then on each holds its
//! place about the path.

use super::helix;
use crate::{
    ballistics::{self, Flight, Spin},
    game::{Effect, Game, Impact, Leg, Orbiter},
    weather::Marks,
};

/// The shape of an orbit.
pub(super) struct Orbit<'o> {
    pub hertz: f64,
    /// Seconds to swing out to full radius.
    pub ramp: f64,
    /// Each orbiter's radius (px) and phase (radians clockwise from `+x`).
    pub orbiters: &'o [(f64, f64)],
}

/// Flies the orbiters about `flight`, landing each one (`land`, with its index, its impact, and
/// what the columns the path passed through did to it) as it strikes; a later one meets the
/// craters of the earlier. Returns the path (cut where the last orbiter stopped), how they spun,
/// the orbiters, and the landings' effects.
pub(super) fn fly(game: &mut Game, flight: Flight, orbit: &Orbit, land: impl FnMut(&mut Game, usize, &Impact, Marks, &mut Vec<Effect>)) -> (Vec<Leg>, Spin, Vec<Orbiter>, Vec<Effect>) {
    let path = helix::center(game, flight);
    let spin = Spin {
        hertz: orbit.hertz,
        ramp: orbit.ramp,
        until: tornado(&path.legs),
    };
    let place = |leg: &Leg, tick: u32, index: usize| {
        let (radius, phase) = orbit.orbiters[index];
        ballistics::orbit_point(&leg.flight, tick, leg.offset, &spin, radius, phase)
    };
    let (ends, effects) = helix::weave(game, &path, orbit.orbiters.len(), place, land);
    let orbiters = orbit.orbiters.iter().zip(&ends).map(|(&(radius, phase), &to)| Orbiter { radius, phase, to }).collect();
    (helix::cut(path.legs, &ends), spin, orbiters, effects)
}

/// The stage tick a tornado first caught the path, or `u32::MAX`: where its legs pause, since a
/// tornado is the only weather that holds a shot (a mirror turns it at once).
fn tornado(legs: &[Leg]) -> u32 {
    legs.windows(2)
        .find(|pair| pair[1].offset > pair[0].offset + pair[0].to)
        .map_or(u32::MAX, |pair| pair[0].offset + pair[0].to)
}
