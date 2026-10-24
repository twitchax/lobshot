//! Skyeye's beam (GunBound's Thor): from the satellite toward a point, striking the first mobile or
//! ground in its way, whoever that is (GF-vgg: dual under it and the beams may hit you). It digs and
//! hurts where it strikes (`beam`), and the damage it deals levels the satellite up.

use super::beam;
use crate::{
    damage::Blast,
    game::{Effect, Game},
    tuning::weather::{SKYEYE_BLAST, SKYEYE_DAMAGE, SKYEYE_WEAPON},
    units::Vec2,
    weather::Skyeye,
};

/// Fires a beam at `target` at `tick`.
pub(crate) fn beam(game: &mut Game, target: Vec2, tick: u32, effects: &mut Vec<Effect>) {
    let Some(strike) = beam::strike(game, Skyeye::at(game.terrain().width()), target) else { return };
    let level = game.weather().skyeye.level();
    effects.push(Effect::Beam {
        from: strike.from,
        to: strike.at,
        tick,
        level,
    });
    let blast = Blast {
        center: strike.at,
        radius: SKYEYE_BLAST.value,
        damage: SKYEYE_DAMAGE.value[usize::from(level - 1).min(SKYEYE_DAMAGE.value.len() - 1)],
        weapon: SKYEYE_WEAPON.value,
        false_angle: false,
        boost: 1.0,
    };
    let before = effects.len();
    beam::burn(game, &strike, &blast, SKYEYE_BLAST.value, tick, effects);
    let dealt: u32 = effects[before..]
        .iter()
        .map(|effect| match *effect {
            Effect::Damaged { amount, .. } | Effect::Shielded { amount, .. } => u32::try_from(amount).unwrap_or(0),
            _ => 0,
        })
        .sum();
    let skyeye = game.skyeye_mut();
    skyeye.xp = skyeye.xp.saturating_add(dealt);
    let now = skyeye.level();
    if now > level {
        effects.push(Effect::SkyeyeLevel { level: now });
    }
}
