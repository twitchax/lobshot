//! Helix strands (Lanternwing's and Spoutpot's S2): projectiles winding about one path, which
//! touches nothing itself, each striking on its own, and perhaps coming together on it in the end.
//! OpenBound's shape (OB), reimplemented: strand positions come from `ballistics::helix_point`,
//! which clients draw with too. The strands' flight (`center`,
//! `weave`, `cut`) is shared with any shot whose projectiles fly about one path (`orbit`).

use crate::{
    ballistics::{self, Flight},
    game::{Contact, Effect, Game, Impact, Leg, Senses, Strand, Track},
    units::Vec2,
    weather::Marks,
};

/// The shape of a helix.
pub(super) struct Helix<'p> {
    pub amplitude: f64,
    pub hertz: f64,
    /// One strand per phase.
    pub phases: &'p [f64],
    /// The stage tick from which the strands run together on the path, if they ever do.
    pub merge: Option<u32>,
}

/// Flies the strands about `flight`, landing each one (`land`, with its index, its impact, and
/// what the columns the path passed through did to it) as it strikes; a later strand meets the craters of the
/// earlier. Returns the path (cut where the last strand stopped), the strands, and the landings'
/// effects.
pub(super) fn fly(game: &mut Game, flight: Flight, helix: &Helix, land: impl FnMut(&mut Game, usize, &Impact, Marks, &mut Vec<Effect>)) -> (Vec<Leg>, Vec<Strand>, Vec<Effect>) {
    let path = center(game, flight);
    let place = |leg: &Leg, tick: u32, index: usize| ballistics::helix_point(&leg.flight, tick, leg.offset, helix.amplitude, helix.hertz, helix.phases[index], helix.merge);
    let (ends, effects) = weave(game, &path, helix.phases.len(), place, land);
    let strands: Vec<Strand> = helix.phases.iter().zip(&ends).map(|(&phase, &to)| Strand { phase, to }).collect();
    (cut(path.legs, &ends), strands, effects)
}

/// The path that projectiles fly about: it only follows the weather, out of the map's sides or
/// bottom, and touches nothing itself.
pub(super) fn center(game: &Game, flight: Flight) -> Track {
    game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false })
}

/// Flies `count` projectiles about `path`, projectile `i` at `place(leg, tick, i)` along each of the
/// path's legs, landing each one (`land`, with its index, its impact, and what the columns the path
/// passed through did to it) as it strikes; a later one meets the craters of the earlier. Returns
/// the stage tick each stopped at (where it struck or left the map, or else the path's end), and
/// the landings' effects.
pub(super) fn weave(
    game: &mut Game,
    path: &Track,
    count: usize,
    place: impl Fn(&Leg, u32, usize) -> Vec2,
    mut land: impl FnMut(&mut Game, usize, &Impact, Marks, &mut Vec<Effect>),
) -> (Vec<u32>, Vec<Effect>) {
    let (width, height) = (f64::from(game.terrain().width()), f64::from(game.terrain().height()));
    let mut ends: Vec<Option<u32>> = vec![None; count];
    let mut effects = Vec::new();
    let mut frames = game.hit_frames();
    'path: for leg in &path.legs {
        for tick in leg.from..=leg.to {
            for (index, end) in ends.iter_mut().enumerate() {
                if end.is_some() {
                    continue;
                }
                let at = place(leg, tick, index);
                let contact = if at.x < 0.0 || at.x >= width || at.y >= height {
                    Contact::Gone
                } else if let Some(&(seat, _)) = frames.iter().find(|(_, frame)| frame.contains(at)) {
                    Contact::Mobile(seat)
                } else {
                    // Inside the map's sides and above its bottom, so the pixel is in i32 range.
                    #[allow(clippy::cast_possible_truncation)]
                    let solid = game.terrain().is_solid(at.x.floor() as i32, at.y.floor() as i32);
                    if !solid {
                        continue;
                    }
                    Contact::Terrain
                };
                let impact = Impact { tick: leg.offset + tick, at, contact };
                *end = Some(impact.tick);
                if contact != Contact::Gone {
                    land(game, index, &impact, path.marks, &mut effects);
                    frames = game.hit_frames();
                }
            }
            if ends.iter().all(Option::is_some) {
                break 'path;
            }
        }
    }
    (ends.into_iter().map(|end| end.unwrap_or(path.impact.tick)).collect(), effects)
}

/// The path's `legs`, cut where the last of the projectiles about it stopped (`ends`, stage ticks).
pub(super) fn cut(legs: Vec<Leg>, ends: &[u32]) -> Vec<Leg> {
    let last = ends.iter().copied().max().unwrap_or(0);
    legs.into_iter()
        .filter(|leg| leg.offset + leg.from <= last)
        .map(|leg| Leg { to: leg.to.min(last - leg.offset), ..leg })
        .collect()
}
