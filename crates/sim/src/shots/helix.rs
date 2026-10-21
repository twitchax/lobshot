//! Helix strands (Lanternwing's S2): projectiles winding about one path, which touches nothing
//! itself, each striking on its own. OpenBound's shape (OB), reimplemented: strand positions come
//! from `ballistics::helix_point`, which clients draw with too.

use crate::{
    ballistics::{self, Flight},
    game::{Contact, Effect, Game, Impact, Leg, Senses, Strand},
};

/// The shape of a helix.
pub(super) struct Helix<'p> {
    pub amplitude: f64,
    pub hertz: f64,
    /// One strand per phase.
    pub phases: &'p [f64],
}

/// Flies the strands about `flight`, landing each one (`land`, with its index, its impact, and
/// whether the path passed through Force) as it strikes; a later strand meets the craters of the
/// earlier. Returns the path (cut where the last strand stopped), the strands, and the landings'
/// effects.
pub(super) fn fly(game: &mut Game, flight: Flight, helix: &Helix, mut land: impl FnMut(&mut Game, usize, &Impact, bool, &mut Vec<Effect>)) -> (Vec<Leg>, Vec<Strand>, Vec<Effect>) {
    // The path itself only follows the weather, out of the map's sides or bottom.
    let path = game.track_sensing(flight, 0, 0, Senses { mobiles: false, terrain: false });
    let (width, height) = (f64::from(game.terrain().width()), f64::from(game.terrain().height()));
    let mut ends: Vec<Option<u32>> = vec![None; helix.phases.len()];
    let mut effects = Vec::new();
    let mut frames = game.hit_frames();
    'path: for leg in &path.legs {
        for tick in leg.from..=leg.to {
            for (index, &phase) in helix.phases.iter().enumerate() {
                if ends[index].is_some() {
                    continue;
                }
                let at = ballistics::helix_point(&leg.flight, tick, leg.offset, helix.amplitude, helix.hertz, phase);
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
                ends[index] = Some(impact.tick);
                if contact != Contact::Gone {
                    land(game, index, &impact, path.forced, &mut effects);
                    frames = game.hit_frames();
                }
            }
            if ends.iter().all(Option::is_some) {
                break 'path;
            }
        }
    }
    let strands: Vec<Strand> = helix
        .phases
        .iter()
        .zip(&ends)
        .map(|(&phase, end)| Strand {
            phase,
            to: end.unwrap_or(path.impact.tick),
        })
        .collect();
    let last = strands.iter().map(|strand| strand.to).max().unwrap_or(0);
    let legs = path
        .legs
        .into_iter()
        .filter(|leg| leg.offset + leg.from <= last)
        .map(|leg| Leg { to: leg.to.min(last - leg.offset), ..leg })
        .collect();
    (legs, strands, effects)
}
