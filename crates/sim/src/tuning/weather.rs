//! How often each weather comes up on each map. GunBound's room screen gave every map a weight from
//! 0 to 3 for each weather (GF-tiro); GF-vgg lists the 2004 weights, and each lobshot map takes the
//! weights of the GunBound map whose archetype it is built on (`DESIGN.md`, "Art").

use super::{DamageRange, Tuned, WeaponType, cited, fitted, picked};
use crate::{
    map::MapRef,
    weather::{Odds, WeatherKind},
};

// ── Skyeye (GunBound's Thor) ─────────────────────────────────────────────────

/// The damage Skyeye's beams must have dealt for each level past the first: level 2 from 201,
/// level 6 from 6801 (GF-lee, GF-vgg). CONTRADICTION: GF-tiro gives 250 / 750 / 1750 / 2500, OB
/// 800 / 1400 / 2000 / 2400 / 3000; CR-aduka says the designers raised them after Aduka's upgrade
/// and allows a seventh level.
pub const SKYEYE_LEVELS: Tuned<[u32; 5]> = cited([200, 700, 1800, 3900, 6800], &["GF-lee", "GF-vgg"]);

/// One beam's damage at each level (`DamageRange`: dead center, then at the hitbox's edge).
pub const SKYEYE_DAMAGE: Tuned<[DamageRange; 6]> = fitted(
    [
        DamageRange { min: 50, max: 100 },
        DamageRange { min: 58, max: 117 },
        DamageRange { min: 67, max: 133 },
        DamageRange { min: 83, max: 167 },
        DamageRange { min: 92, max: 183 },
        DamageRange { min: 100, max: 200 },
    ],
    &["CR-aduka", "GF-vgg"],
    "A weather beam deals 50 to 100 at first (CR-aduka); each level scales that by GF-vgg's average \
     for the level over level 1's (150, 175, 200, 250, 275, 300), which track Aduka's three-beam S2 \
     in CR-aduka (about 150, 180, then 220 to 250). CONTRADICTION: GF-tiro has about 200 by level \
     5, GF-max 350 at level 6, OB 40 + 20 a level.",
);

/// How far a beam's blast reaches and digs where it strikes.
pub const SKYEYE_BLAST: Tuned<f64> = cited(25.0, &["OB"]);

pub const SKYEYE_WEAPON: Tuned<WeaponType> = picked(WeaponType::Laser, "The sources call it Thor's laser (CR-aduka).");

/// How long after a projectile lands its beam strikes, in seconds.
pub const SKYEYE_DELAY_SECONDS: Tuned<f64> = picked(0.2, "Every shot is followed instantly by a hit (CR-aduka); a beat keeps the two readable.");

/// A GunBound map's 2004 weights, in GF-vgg's order: Force, Tornado, Electricity, Wind, Moon
/// (Balm), Item Lock (Lock) and Thor (Skyeye).
type Moondisk = [u8; 7];

/// Mine corridors (Glintvein A).
pub const METAMINE_A: Tuned<Moondisk> = cited([0, 2, 0, 3, 0, 2, 1], &["GF-vgg"]);
/// Mine corridors (Glintvein B).
pub const METAMINE_B: Tuned<Moondisk> = cited([2, 3, 0, 0, 0, 2, 1], &["GF-vgg"]);
/// Town rooftops (Gablewick).
pub const MIRAMO: Tuned<Moondisk> = cited([2, 1, 2, 1, 0, 0, 2], &["GF-vgg"]);
/// Floating islands (Tidewall A).
pub const NIRVANA_A: Tuned<Moondisk> = cited([0, 1, 2, 2, 1, 1, 1], &["GF-vgg"]);
/// Floating islands (Tidewall B).
pub const NIRVANA_B: Tuned<Moondisk> = cited([0, 0, 4, 1, 0, 0, 3], &["GF-vgg"]);
/// Long flat sea (Rimewake).
pub const SEA_OF_HERO: Tuned<Moondisk> = cited([2, 0, 2, 1, 0, 0, 3], &["GF-vgg"]);
/// Dragon's-back ridge (Ashspine).
pub const DRAGON: Tuned<Moondisk> = cited([3, 0, 2, 1, 1, 0, 1], &["GF-vgg"]);
/// Tall tower (Clockrise A).
pub const COZYTOWER_A: Tuned<Moondisk> = cited([2, 1, 1, 1, 0, 1, 2], &["GF-vgg"]);
/// Tall tower (Clockrise B).
pub const COZYTOWER_B: Tuned<Moondisk> = cited([2, 2, 1, 1, 0, 1, 1], &["GF-vgg"]);
/// City (Neonbay).
pub const METROPOLIS: Tuned<Moondisk> = cited([1, 1, 3, 1, 0, 1, 1], &["GF-vgg"]);
/// Practice slope (Tumblehill), and the generated practice and test fields.
pub const DUMMY_SLOPES: Tuned<Moondisk> = cited([2, 2, 0, 1, 1, 1, 1], &["GF-vgg"]);

/// The weight of Mirror, Drain and Random, which came after the 2004 table.
pub const LATER: Tuned<u8> = picked(
    1,
    "The 2004 table predates Mirror, Weakness and Random (WP08); DragonBound rolls them alongside the \
     rest without saying how often, so each takes the lowest weight a map gives a weather it has.",
);

/// The archetype's weights for a drawn map, by name and variant; `None` for a map lobshot doesn't
/// know.
#[must_use]
pub fn moondisk(name: &str, variant: &str) -> Option<Moondisk> {
    let tuned = match (name, variant) {
        ("glintvein", "B") => METAMINE_B,
        ("glintvein", _) => METAMINE_A,
        ("gablewick", _) => MIRAMO,
        ("tidewall", "B") => NIRVANA_B,
        ("tidewall", _) => NIRVANA_A,
        ("rimewake", _) => SEA_OF_HERO,
        ("ashspine", _) => DRAGON,
        ("clockrise", "B") => COZYTOWER_B,
        ("clockrise", _) => COZYTOWER_A,
        ("tumblehill", _) => DUMMY_SLOPES,
        ("neonbay", _) => METROPOLIS,
        _ => return None,
    };
    Some(tuned.value)
}

/// The weather odds on `map`. A drawn map lobshot doesn't know (a test's) and the generated fields
/// take the practice slope's.
#[must_use]
pub fn odds(map: &MapRef) -> Odds {
    let row = match map {
        MapRef::Asset { name, variant, .. } => moondisk(name, variant).unwrap_or(DUMMY_SLOPES.value),
        MapRef::Flat { .. } | MapRef::Hills { .. } => DUMMY_SLOPES.value,
    };
    let [force, tornado, electricity, wind, balm, lock, skyeye] = row;
    Odds::new(|kind| match kind {
        WeatherKind::Wind => wind,
        WeatherKind::Force => force,
        WeatherKind::Tornado => tornado,
        WeatherKind::Electricity => electricity,
        WeatherKind::Lock => lock,
        WeatherKind::Balm => balm,
        WeatherKind::Skyeye => skyeye,
        WeatherKind::Mirror | WeatherKind::Drain | WeatherKind::Random => LATER.value,
    })
}

#[cfg(test)]
mod unit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn each_map_takes_its_archetypes_weights() {
        let ashspine = odds(&MapRef::Asset {
            name: "ashspine".to_owned(),
            variant: "A".to_owned(),
            mask_hash: 0,
        });
        assert_eq!((ashspine.weight(WeatherKind::Force), ashspine.weight(WeatherKind::Tornado)), (3, 0));
        assert_eq!(ashspine.weight(WeatherKind::Mirror), LATER.value);
        let flat = odds(&MapRef::Flat { width: 800, height: 600, ground: 500 });
        assert_eq!(flat.weight(WeatherKind::Electricity), 0, "the practice slope has none");
        assert_eq!(moondisk("glintvein", "B").map(|row| row[3]), Some(0), "no Wind in Metamine B");
        assert_eq!(moondisk("clockrise", "A").map(|row| (row[1], row[6])), Some((1, 2)), "Cozytower A: Thor over Tornado");
        assert_eq!(moondisk("clockrise", "B").map(|row| (row[1], row[6])), Some((2, 1)), "Cozytower B: Tornado over Thor");
        assert_eq!(moondisk("tumblehill", "B"), Some(DUMMY_SLOPES.value), "the drawn practice slope rolls the practice fields' odds");
        assert_eq!(moondisk("nowhere", "A"), None);
    }
}
