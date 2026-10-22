//! How often each weather comes up on each map. GunBound's room screen gave every map a weight from
//! 0 to 3 for each weather (GF-tiro); GF-vgg lists the 2004 weights, and each lobshot map takes the
//! weights of the GunBound map whose archetype it is built on (`DESIGN.md`, "Art").

use super::{Tuned, cited, picked};
use crate::{
    map::MapRef,
    weather::{Odds, WeatherKind},
};

/// A GunBound map's 2004 weights, in GF-vgg's order: Force, Tornado, Electricity, Wind, Moon
/// (Balm), Item Lock (Lock) and Thor. Thor's weight waits for the Thor weather.
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
/// Practice slope: the generated practice and test fields.
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
    let [force, tornado, electricity, wind, balm, lock, _thor] = row;
    Odds::new(|kind| match kind {
        WeatherKind::Wind => wind,
        WeatherKind::Force => force,
        WeatherKind::Tornado => tornado,
        WeatherKind::Electricity => electricity,
        WeatherKind::Lock => lock,
        WeatherKind::Balm => balm,
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
        assert_eq!(moondisk("nowhere", "A"), None);
    }
}
