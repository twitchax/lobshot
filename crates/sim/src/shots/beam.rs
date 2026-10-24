//! Beams: straight from a satellite through a point, on to the first mobile or ground in their way
//! (`strike`), where a blast goes off (`burn`). Skyeye's (the map's satellite) and the lasers of a
//! mobile's own (Lensowl's) share them; whatever is in a beam's way takes it, its shooter included
//! (GF-vgg).

use super::bolt::{self, Strike};
use crate::{
    damage::Blast,
    game::{Contact, Effect, Game},
    units::Vec2,
};

/// A beam from `from` through `through`, run on to the first mobile or ground in its way, or off the
/// map. `None` for a beam with no direction.
pub(crate) fn strike(game: &Game, from: Vec2, through: Vec2) -> Option<Strike> {
    let path = through - from;
    let length = libm::sqrt(path.x * path.x + path.y * path.y);
    (length > 0.0).then(|| bolt::strike(game, from, path * (1.0 / length)))
}

/// A beam's blast at `tick`: `blast`, centered where `strike` struck, carving `crater`, direct on a
/// mobile it struck. A beam that left the map does nothing.
pub(crate) fn burn(game: &mut Game, strike: &Strike, blast: &Blast, crater: f64, tick: u32, effects: &mut Vec<Effect>) {
    let direct = match strike.contact {
        Contact::Gone | Contact::Column(_) => return,
        Contact::Terrain => None,
        Contact::Mobile(seat) => Some(seat),
    };
    game.explode(&Blast { center: strike.at, ..*blast }, crater, direct, tick, effects);
}
