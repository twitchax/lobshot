//! Where games are played: the built-in `flat` map, and drawn maps from the client bundle's
//! generated assets (`<dist>/assets`, which `cargo xtask art` writes and trunk copies). The server
//! reads a map's manifest when a game is set up, and its collision mask through `MapAssets`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use anyhow::{Context as _, ensure};

use lobshot_assets::{FLAT, MapInfo, map_path, mask_path};
use lobshot_sim::{
    base::{Res, Void},
    map::{MapAssets, MapRef},
    terrain::Terrain,
};

/// `flat` (`lobshot_assets::FLAT`): one field, two screens wide.
const FLAT_WIDTH: u32 = 1600;
const FLAT_MAP: MapRef = MapRef::Flat {
    width: FLAT_WIDTH,
    height: 900,
    ground: 700,
};
const FLAT_SPAWNS: [i32; 8] = [200, 1400, 500, 1100, 350, 1250, 650, 950];

/// A map ready to seat a game on.
#[derive(Debug, Clone, PartialEq)]
pub struct Arena {
    pub map: MapRef,
    pub width: u32,
    /// Columns mobiles may spawn in.
    pub spawns: Vec<i32>,
}

/// The maps a server can seat games on. Masks are cached once read: every game on a map starts
/// from the same one.
#[derive(Debug)]
pub struct Maps {
    root: PathBuf,
    masks: Mutex<HashMap<(String, String), Terrain>>,
}

impl Maps {
    /// Drawn maps come from `<dist>/assets`.
    #[must_use]
    pub fn new(dist: &Path) -> Self {
        Self {
            root: dist.join("assets"),
            masks: Mutex::new(HashMap::new()),
        }
    }

    /// The map a room named, one arena per variant (A, B), ready to seat a game on; a game is
    /// played on one of them, rolled at the start.
    ///
    /// # Errors
    ///
    /// Fails on a name that isn't a map, or a drawn map whose generated manifest is missing.
    pub fn arenas(&self, name: &str) -> Res<Vec<Arena>> {
        if name == FLAT {
            return Ok(vec![Arena {
                map: FLAT_MAP,
                width: FLAT_WIDTH,
                spawns: FLAT_SPAWNS.to_vec(),
            }]);
        }
        check_asset_name(name, "map")?;
        let path = self.root.join(map_path(name));
        let text = std::fs::read_to_string(&path).with_context(|| format!("there is no map {name:?}"))?;
        let info = MapInfo::from_ron(&text).with_context(|| format!("reading {}", path.display()))?;
        info.variants
            .iter()
            .map(|variant| {
                // The hand-written source manifests lack these; only `cargo xtask art`'s output has them.
                let (Some((width, _)), Some(mask_hash)) = (variant.size, variant.mask_hash) else {
                    anyhow::bail!("map {name}'s manifest is not generated: run `cargo xtask art`");
                };
                Ok(Arena {
                    map: MapRef::Asset {
                        name: name.to_owned(),
                        variant: variant.id.clone(),
                        mask_hash,
                    },
                    width,
                    spawns: variant.spawns.clone(),
                })
            })
            .collect()
    }
}

impl MapAssets for Maps {
    fn mask(&self, name: &str, variant: &str) -> Res<Terrain> {
        check_asset_name(name, "map")?;
        check_asset_name(variant, "variant")?;
        let key = (name.to_owned(), variant.to_owned());
        // The cache only ever holds whole masks, so one a panicking thread left behind is sound.
        if let Some(terrain) = self.masks.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            return Ok(terrain.clone());
        }
        let path = self.root.join(mask_path(name, variant));
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let terrain = Terrain::decode_mask(&bytes).with_context(|| format!("decoding {}", path.display()))?;
        self.masks.lock().unwrap_or_else(PoisonError::into_inner).insert(key, terrain.clone());
        Ok(terrain)
    }
}

/// Checks a name a client picked that becomes part of an asset path (a map, a rider): only plain
/// directory names, never `..` or `/`.
///
/// # Errors
///
/// Fails on anything but 1 to 32 ASCII letters, digits, `_` and `-`.
pub fn check_asset_name(name: &str, what: &str) -> Void {
    ensure!(
        !name.is_empty() && name.len() <= 32 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "{name:?} is not a {what} name"
    );
    Ok(())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use lobshot_sim::{
        game::{Game, MAX_SEATS, MobileKind, Seat, Setup, Team, check_spawn_spacing},
        items::Loadout,
        units::{Facing, Wind},
    };

    use super::*;
    use crate::tests::write_map_variants;

    fn pond() -> Terrain {
        Terrain::from_fn(400, 300, |_, y| y >= 200).unwrap()
    }

    #[test]
    fn flat_is_built_in() {
        let maps = Maps::new(Path::new("no-such-dist"));
        let [arena] = maps.arenas(FLAT).unwrap().try_into().unwrap();
        assert_eq!(arena.map, MapRef::Flat { width: 1600, height: 900, ground: 700 });
        assert_eq!(arena.width, 1600);
        assert_eq!(arena.spawns, [200, 1400, 500, 1100, 350, 1250, 650, 950]);
    }

    #[test]
    fn flat_fits_eight_of_any_mobile() {
        let maps = Maps::new(Path::new("no-such-dist"));
        let [arena] = maps.arenas(FLAT).unwrap().try_into().unwrap();
        assert_eq!(arena.spawns.len(), MAX_SEATS);
        // The rule `cargo xtask art` holds drawn maps' spawns to.
        check_spawn_spacing(&arena.spawns).unwrap();
        let (reach, _) = MobileKind::spawn_room();
        let (first, last) = (arena.spawns.iter().min().unwrap(), arena.spawns.iter().max().unwrap());
        assert!(first - reach >= 0 && last + reach <= i32::try_from(arena.width).unwrap(), "{:?}", arena.spawns);

        for mobile in MobileKind::ALL {
            let seats = (0..).zip(&arena.spawns).map(|(i, &spawn_x)| Seat {
                team: if i % 2 == 0 { Team::A } else { Team::B },
                mobile,
                spawn_x,
                facing: Facing::Right,
                items: Loadout::default(),
            });
            let setup = Setup {
                map: arena.map.clone(),
                seats: seats.collect(),
                order: (0..8).collect(),
                wind: Wind::default(),
                sudden_death: None,
            };
            Game::new(&setup, &maps).unwrap_or_else(|e| panic!("eight {mobile:?}: {e:#}"));
        }
    }

    /// The hand-written manifests in `assets/maps/`, which `cargo xtask art` copies into the bundle.
    #[test]
    fn every_map_the_lobby_offers_seats_eight_in_every_variant() {
        let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for &name in lobshot_assets::MAPS {
            let text = std::fs::read_to_string(sources.join(map_path(name))).unwrap();
            let info = MapInfo::from_ron(&text).unwrap();
            assert!(!info.variants.is_empty(), "{name}");
            for variant in &info.variants {
                assert!(variant.spawns.len() >= MAX_SEATS, "{name} {} spawns only {} players", variant.id, variant.spawns.len());
            }
        }
    }

    #[test]
    fn drawn_maps_load_every_variant_and_its_mask() {
        let dist = TempDir::new().unwrap();
        let island = Terrain::from_fn(400, 300, |x, y| y >= 150 && (100..300).contains(&x)).unwrap();
        write_map_variants(dist.path(), "pond", &[("A", &pond(), &[50, 350]), ("B", &island, &[150, 250])]).unwrap();
        let maps = Maps::new(dist.path());

        let [arena, b] = maps.arenas("pond").unwrap().try_into().unwrap();
        assert_eq!(
            b.map,
            MapRef::Asset {
                name: "pond".into(),
                variant: "B".into(),
                mask_hash: island.mask_hash()
            }
        );
        assert_eq!((b.spawns, b.map.terrain(&maps).unwrap()), (vec![150, 250], island));
        assert_eq!(
            arena,
            Arena {
                map: MapRef::Asset {
                    name: "pond".into(),
                    variant: "A".into(),
                    mask_hash: pond().mask_hash(),
                },
                width: 400,
                spawns: vec![50, 350],
            }
        );
        assert_eq!(arena.map.terrain(&maps).unwrap(), pond());

        // Cached: the file can go and the mask still loads.
        std::fs::remove_file(dist.path().join("assets").join(mask_path("pond", "A"))).unwrap();
        assert_eq!(maps.mask("pond", "A").unwrap(), pond());
    }

    #[test]
    fn missing_or_ungenerated_maps_are_refused() {
        let dist = TempDir::new().unwrap();
        let maps = Maps::new(dist.path());
        assert!(format!("{:#}", maps.arenas("tidewall").unwrap_err()).contains("no map"));

        let source = dist.path().join("assets").join(map_path("raw"));
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, r#"MapInfo(name: "raw", title: "Raw", variants: [(id: "A", spawns: [1, 2])])"#).unwrap();
        assert!(format!("{:#}", maps.arenas("raw").unwrap_err()).contains("cargo xtask art"));
        assert!(maps.mask("raw", "A").is_err());
    }

    #[test]
    fn names_never_leave_the_assets_directory() {
        let maps = Maps::new(Path::new("dist"));
        for name in ["", "..", "../secret", "a/b", "a\\b", "x.ron", &"long".repeat(9)] {
            assert!(maps.arenas(name).is_err(), "{name:?}");
            assert!(maps.mask(name, "A").is_err(), "{name:?}");
        }
        assert!(maps.mask("pond", "../A").is_err());
    }
}
