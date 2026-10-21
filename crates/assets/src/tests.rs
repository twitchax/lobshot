//! Fixtures shared by unit tests here and in the crates that read assets. Always compiled (not
//! `#[cfg(test)]`) so other crates' tests can use it; keep it free of test-only dependencies.

use std::path::Path;

use anyhow::Context as _;

use lobshot_sim::{base::Void, terrain::Terrain};

use crate::{MapInfo, MapVariant, map_path, mask_path, to_ron};

/// Writes a drawn map into the generated assets at `root` the way `cargo xtask art` does: a
/// generated manifest with each variant's id, size, mask hash and spawns, and its collision mask.
///
/// # Errors
///
/// Fails if a file can't be written.
pub fn write_map_variants(root: &Path, name: &str, variants: &[(&str, &Terrain, &[i32])]) -> Void {
    let info = MapInfo {
        name: name.to_owned(),
        title: name.to_owned(),
        variants: variants
            .iter()
            .map(|&(id, mask, spawns)| MapVariant {
                id: id.to_owned(),
                spawns: spawns.to_vec(),
                size: Some((mask.width(), mask.height())),
                mask_hash: Some(mask.mask_hash()),
            })
            .collect(),
    };
    let mut files = vec![(map_path(name), to_ron(&info)?.into_bytes())];
    files.extend(variants.iter().map(|&(id, mask, _)| (mask_path(name, id), mask.encode_mask())));
    for (path, bytes) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().context("an asset path has a parent")?)?;
        std::fs::write(path, bytes)?;
    }
    Ok(())
}
