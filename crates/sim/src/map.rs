//! Which terrain a game is played on: a synthetic recipe (tests, bots' scratch games) or a drawn
//! map's mask, which the caller loads (sim does no I/O) through `MapAssets`.

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{base::Res, terrain::Terrain};

/// A map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapRef {
    /// Solid from row `ground` down.
    Flat { width: u32, height: u32, ground: u32 },
    /// Rolling hills: `ground` at the troughs, `amplitude` px higher at the crests, one crest every
    /// `period` px.
    Hills { width: u32, height: u32, ground: u32, amplitude: u32, period: u32 },
    /// A drawn map's variant, by name (`tidewall`, `A`). `mask_hash` pins the exact mask the game
    /// was set up on, so a client with stale assets fails loudly instead of desyncing.
    Asset { name: String, variant: String, mask_hash: u64 },
}

/// Loads drawn maps' masks: from disk on the server and in the lab, over HTTP in the client.
pub trait MapAssets {
    /// The terrain mask for `name`'s `variant`.
    ///
    /// # Errors
    ///
    /// Fails when the map or variant doesn't exist or can't be read.
    fn mask(&self, name: &str, variant: &str) -> Res<Terrain>;
}

/// For callers that only ever use synthetic maps.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAssets;

impl MapAssets for NoAssets {
    fn mask(&self, name: &str, variant: &str) -> Res<Terrain> {
        bail!("no map assets are loaded, so {name} {variant} is unavailable")
    }
}

impl MapRef {
    /// Builds the terrain, loading a drawn map's mask from `assets`.
    ///
    /// # Errors
    ///
    /// Fails on an out-of-range size or a zero period, a mask that can't be loaded, or a mask
    /// whose hash isn't the one the game was set up on.
    pub fn terrain(&self, assets: &dyn MapAssets) -> Res<Terrain> {
        match *self {
            Self::Flat { width, height, ground } => Terrain::from_fn(width, height, |_, y| y >= ground),
            Self::Hills { width, height, ground, amplitude, period } => {
                ensure!(period > 0, "hills need a nonzero period");
                let surface = |x: u32| -> f64 {
                    let phase = 2.0 * core::f64::consts::PI * f64::from(x) / f64::from(period);
                    f64::from(ground) - f64::from(amplitude) * (1.0 - libm::cos(phase)) / 2.0
                };
                Terrain::from_fn(width, height, |x, y| f64::from(y) >= surface(x))
            }
            Self::Asset { ref name, ref variant, mask_hash } => {
                let terrain = assets.mask(name, variant)?;
                let found = terrain.mask_hash();
                ensure!(found == mask_hash, "{name} {variant}'s mask hashes to {found:#018x}, but the game was set up on {mask_hash:#018x}");
                Ok(terrain)
            }
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    struct OneMap(Terrain);

    impl MapAssets for OneMap {
        fn mask(&self, name: &str, variant: &str) -> Res<Terrain> {
            ensure!(name == "pond" && variant == "A", "no {name} {variant}");
            Ok(self.0.clone())
        }
    }

    #[test]
    fn hills_rise_to_their_crests() {
        let hills = MapRef::Hills {
            width: 400,
            height: 300,
            ground: 250,
            amplitude: 60,
            period: 200,
        };
        let terrain = hills.terrain(&NoAssets).unwrap();
        assert_eq!(terrain.surface_at_or_below(0, 0), Some(250));
        assert_eq!(terrain.surface_at_or_below(100, 0), Some(190));
        let flat_period = MapRef::Hills {
            width: 10,
            height: 10,
            ground: 5,
            amplitude: 1,
            period: 0,
        };
        assert!(flat_period.terrain(&NoAssets).is_err());
    }

    #[test]
    fn asset_maps_load_through_the_caller_and_check_their_hash() {
        let pond = Terrain::from_fn(40, 30, |x, y| y > 20 || x < 3).unwrap();
        let assets = OneMap(pond.clone());
        let map = |variant: &str, mask_hash| MapRef::Asset {
            name: "pond".into(),
            variant: variant.into(),
            mask_hash,
        };

        assert_eq!(map("A", pond.mask_hash()).terrain(&assets).unwrap(), pond);
        assert!(map("A", pond.mask_hash() ^ 1).terrain(&assets).is_err(), "a stale mask");
        assert!(map("B", pond.mask_hash()).terrain(&assets).is_err(), "a missing variant");
        assert!(map("A", pond.mask_hash()).terrain(&NoAssets).is_err());
    }
}
