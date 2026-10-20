//! `cargo xtask art`: render the art sources in `assets/` (SVG and hand-written RON, CC0) into
//! `assets/gen/`, in the layout `lobshot_assets` defines, plus review sheets in
//! `target/art-preview/`. The same sources always give byte-identical output. The sheets stay out of
//! `assets/gen/` because trunk ships all of it to every player.
//!
//! - Mobiles: each rig part at `ART_SCALE`, pivot-centered, and the checked rig beside them.
//! - Riders: each at `ART_SCALE`, seat-centered.
//! - Maps: the background, and per variant the terrain art and the collision mask, both from one
//!   land geometry (`land.rs`), plus the generated manifest with sizes and mask hashes.

mod compose;
mod land;
mod map;
mod mobile;
mod out;
mod preview;
mod raster;
mod rng;

use std::path::{Path, PathBuf};

use lobshot_assets::MobileRig;
use lobshot_sim::{
    base::{Res, Void},
    game::MobileKind,
};

use self::out::Out;

/// `lobshot_assets::ART_SCALE` as resvg takes it.
#[allow(clippy::cast_possible_truncation)] // 2.0 is exact in f32.
const ART_SCALE: f32 = lobshot_assets::ART_SCALE as f32;

/// The art sources, `assets/` at the repo root.
pub fn sources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("assets")
}

/// Where the review sheets go, from the repo root: under `target/`, so they never reach the client
/// bundle.
const PREVIEWS: &str = "target/art-preview";

pub fn run() -> Void {
    let sources = sources();
    let built = build(&sources, &sources.join("gen"))?;
    let sheets = write_previews(&sources, &Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(PREVIEWS), &built)?;
    print_report(&built.written, &sheets);
    Ok(())
}

/// A build's output, and what the review sheets are drawn from.
pub struct Built {
    /// Each written file, relative to the output directory, and its size.
    pub written: Vec<(String, u64)>,
    rigs: Vec<(MobileKind, MobileRig)>,
    riders: Vec<String>,
    maps: Vec<map::BuiltMap>,
}

/// Renders everything in `sources` into `out_dir`, and removes whatever an earlier build left
/// there that this one did not write.
pub fn build(sources: &Path, out_dir: &Path) -> Res<Built> {
    let mut out = Out::new(out_dir);
    let rigs = mobile::build_mobiles(sources, &mut out)?;
    let riders = mobile::build_riders(sources, &mut out)?;
    let maps = map::build_maps(sources, &mut out)?;
    out.prune()?;
    Ok(Built { written: sizes(&out), rigs, riders, maps })
}

/// Writes the review sheets for a build into `dir`, replacing the last set.
pub fn write_previews(sources: &Path, dir: &Path, built: &Built) -> Res<Vec<(String, u64)>> {
    let mut out = Out::new(dir);
    preview::build_previews(sources, &mut out, &built.rigs, &built.riders, &built.maps)?;
    out.prune()?;
    Ok(sizes(&out))
}

fn sizes(out: &Out) -> Vec<(String, u64)> {
    out.written().iter().map(|(path, &size)| (path.clone(), size)).collect()
}

/// Sizes per game asset and in total (the first-load goal, `DESIGN.md` "Performance goals",
/// counts what ships), then where the review sheets are.
fn print_report(assets: &[(String, u64)], sheets: &[(String, u64)]) {
    for (path, size) in assets {
        println!("{:>10}  assets/gen/{path}", kib(*size));
    }
    println!("{:>10}  total, {} files", kib(assets.iter().map(|(_, size)| size).sum()), assets.len());
    println!("review sheets in {PREVIEWS}/: {}", sheets.iter().map(|(path, _)| path.as_str()).collect::<Vec<_>>().join(", "));
}

fn kib(bytes: u64) -> String {
    // Asset sizes stay far below 2^52 bytes, where an f64 stops being exact.
    #[allow(clippy::cast_precision_loss)]
    let kib = bytes as f64 / 1024.0;
    format!("{kib:.1} KiB")
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use lobshot_assets::{MapInfo, MobileRig, mask_path, rig_path, terrain_path};
    use lobshot_sim::{game::MobileKind, terrain::Terrain};
    use pretty_assertions::assert_eq;
    use resvg::tiny_skia::Pixmap;
    use tempfile::TempDir;

    use super::*;

    /// The real mobiles and riders beside a small map, so a full build takes a moment.
    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        let assets = dir.path();
        for (from, to) in [("mobiles/coilback", "mobiles/coilback"), ("riders", "riders")] {
            std::fs::create_dir_all(assets.join(to)).unwrap();
            for entry in std::fs::read_dir(sources().join(from)).unwrap() {
                let path = entry.unwrap().path();
                std::fs::copy(&path, assets.join(to).join(path.file_name().unwrap())).unwrap();
            }
        }
        let map = assets.join("maps/pond");
        std::fs::create_dir_all(map.join("A")).unwrap();
        std::fs::write(map.join("map.ron"), r#"MapInfo(name: "pond", title: "Pond", variants: [(id: "A", spawns: [100])])"#).unwrap();
        std::fs::write(
            map.join("background.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="120" viewBox="0 0 200 120"><rect width="200" height="120" fill="#9d88c2"/></svg>"##,
        )
        .unwrap();
        std::fs::write(
            map.join("A/land.ron"),
            r#"Land(
                size: (200, 120),
                islands: [(start: (20.0, 60.0), edges: [
                    Top((70.0, 60.0), (130.0, 60.0), (180.0, 60.0)),
                    Rock((185.0, 80.0), (170.0, 110.0), (150.0, 110.0)),
                    Rock((110.0, 110.0), (60.0, 110.0), (40.0, 110.0)),
                    Rock((25.0, 110.0), (15.0, 80.0), (20.0, 60.0)),
                ])],
                dress: [Edge(slot: "tufts", along: Top, symbols: ["tuft"], every: 9.0, scale: (0.8, 1.2), follow_normal: true, mirror: true, seed: 3)],
            )"#,
        )
        .unwrap();
        std::fs::write(
            map.join("A/terrain.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="120" viewBox="0 0 200 120">
                <defs><path id="tuft" d="M -3,0 L 0,-6 L 3,0 Z" fill="#6cc247"/></defs>
                <path d="{{land}}" fill="#d98f5c" stroke="#2a1a2e" stroke-width="3"/>
                <g>{{tufts}}</g>
            </svg>"##,
        )
        .unwrap();
        dir
    }

    #[test]
    fn builds_the_layout_its_readers_expect() {
        let sources = fixture();
        let out = TempDir::new().unwrap();
        let written = build(sources.path(), out.path()).unwrap().written;
        let read = |path: &str| std::fs::read(out.path().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));

        let rig = MobileRig::from_ron(&String::from_utf8(read(&rig_path(MobileKind::Coilback))).unwrap()).unwrap();
        for part in &rig.parts {
            let png = Pixmap::decode_png(&read(&lobshot_assets::part_path(MobileKind::Coilback, &part.file))).unwrap();
            assert!(png.width() % 2 == 0 && png.height() % 2 == 0, "{}: the pivot must fall on a pixel corner", part.file);
        }
        Pixmap::decode_png(&read(&lobshot_assets::rider_path("pip"))).unwrap();

        let info = MapInfo::from_ron(&String::from_utf8(read(&lobshot_assets::map_path("pond"))).unwrap()).unwrap();
        let variant = info.variant("A").unwrap();
        let mask = Terrain::decode_mask(&read(&mask_path("pond", "A"))).unwrap();
        assert_eq!(variant.size, Some((200, 120)));
        assert_eq!((mask.width(), mask.height()), (200, 120));
        assert_eq!(variant.mask_hash, Some(mask.mask_hash()), "the manifest pins the mask it ships with");
        let terrain = Pixmap::decode_png(&read(&terrain_path("pond", "A"))).unwrap();
        assert_eq!((terrain.width(), terrain.height()), (400, 240), "terrain at ART_SCALE");
        assert!(written.iter().all(|(path, _)| !path.starts_with("preview")), "review sheets never ship");
    }

    #[test]
    fn the_same_sources_give_the_same_bytes() {
        let sources = fixture();
        let (first, second) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let written = build(sources.path(), first.path()).unwrap().written;
        assert_eq!(build(sources.path(), second.path()).unwrap().written, written);
        for (path, _) in &written {
            assert!(
                std::fs::read(first.path().join(path)).unwrap() == std::fs::read(second.path().join(path)).unwrap(),
                "{path} differs between builds"
            );
        }
    }

    #[test]
    #[ignore = "renders Tidewall twice at full size: minutes unless resvg is optimized in the dev profile"]
    fn the_real_assets_build_byte_identically() {
        let (first, second) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let written = build(&sources(), first.path()).unwrap().written;
        assert_eq!(build(&sources(), second.path()).unwrap().written, written);
        for (path, _) in &written {
            assert!(
                std::fs::read(first.path().join(path)).unwrap() == std::fs::read(second.path().join(path)).unwrap(),
                "{path} differs between builds"
            );
        }
    }
}
