//! Maps: the shared background, and per variant the terrain art, the collision mask and the
//! generated manifest with each variant's size and mask hash.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail, ensure};
use lobshot_assets::{MapInfo, background_path, map_path, mask_path, terrain_path, to_ron};
use lobshot_sim::{
    base::{Res, Void},
    game::{MobileKind, check_spawn_spacing},
    movement::{Footing, Landing, settle},
    terrain::Terrain,
    tuning::TILT_SEARCH_PX,
};

use resvg::tiny_skia::Pixmap;

use super::{
    ART_SCALE,
    land::{Land, Shape},
    out::Out,
    raster::{Svg, encode_png},
};

/// A variant, built: its land, mask and rendered terrain, kept for the review sheets.
pub struct BuiltVariant {
    pub shape: Shape,
    pub mask: Terrain,
    pub terrain: Pixmap,
}

/// A map, built: the generated manifest, with its variants in the same order.
pub struct BuiltMap {
    pub info: MapInfo,
    pub background: Pixmap,
    pub variants: Vec<BuiltVariant>,
}

/// Builds every map in `assets/maps/`, in name order.
pub fn build_maps(sources: &Path, out: &mut Out) -> Res<Vec<BuiltMap>> {
    let dir = sources.join("maps");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    names.iter().map(|name| build_map(&dir.join(name), out).with_context(|| format!("building map {name}"))).collect()
}

/// One map: `map.ron` (hand-written: names, variants, spawns), `background.svg`, `terrain.svg` (the
/// art, with slots for placement, shared by the variants), and per variant `<id>/land.ron` (the
/// geometry, and where the art's set pieces go). A variant may draw its own `<id>/terrain.svg`.
pub fn build_map(dir: &Path, out: &mut Out) -> Res<BuiltMap> {
    let mut info = load_info(dir)?;

    let background_svg = Svg::load(&dir.join("background.svg"))?;
    let background = background_svg.render_view(ART_SCALE)?;
    out.write(&background_path(&info.name), &encode_png(&background)?)?;

    let mut variants = Vec::with_capacity(info.variants.len());
    for variant in &mut info.variants {
        let built = build_variant(dir, &info.name, &variant.id, out).with_context(|| format!("variant {}", variant.id))?;
        let view = background_svg.view();
        let (width, height) = built.shape.size;
        ensure!(
            view.matches((width, height)) && (view.x, view.y) == (0.0, 0.0),
            "the background's viewBox must be the map, 0 0 {width} {height}, so it lines up with every variant"
        );
        check_spawns(&built.mask, &variant.spawns).with_context(|| format!("variant {}'s spawns", variant.id))?;
        variant.size = Some(built.shape.size);
        variant.mask_hash = Some(built.mask.mask_hash());
        variants.push(built);
    }
    out.write(&map_path(&info.name), to_ron(&info)?.as_bytes())?;
    Ok(BuiltMap { info, background, variants })
}

/// Reads a map's hand-written manifest, which must name its own directory and leave the generated
/// fields to the build.
pub fn load_info(dir: &Path) -> Res<MapInfo> {
    let path = dir.join("map.ron");
    let info = MapInfo::from_ron(&std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?)?;
    let dir_name = dir.file_name().map(|name| name.to_string_lossy());
    ensure!(dir_name.as_deref() == Some(info.name.as_str()), "{} names the map {:?}", path.display(), info.name);
    for variant in &info.variants {
        ensure!(
            variant.size.is_none() && variant.mask_hash.is_none(),
            "variant {}'s size and mask_hash are generated; leave them out",
            variant.id
        );
    }
    Ok(info)
}

/// A variant's land, built from its `land.ron`.
pub fn load_shape(dir: &Path, variant: &str) -> Res<(Land, Shape)> {
    let land = Land::load(&dir.join(variant).join("land.ron"))?;
    let shape = land.shape()?;
    Ok((land, shape))
}

fn build_variant(dir: &Path, map: &str, variant: &str, out: &mut Out) -> Res<BuiltVariant> {
    let (land, shape) = load_shape(dir, variant)?;
    let mask = shape.mask()?;

    let template_path = template_path(dir, variant)?;
    let template = std::fs::read_to_string(&template_path).with_context(|| format!("reading {}", template_path.display()))?;
    let art = Svg::parse(&shape.dress(&template, &land.dress)?).with_context(|| format!("parsing {} once dressed", template_path.display()))?;
    let view = art.view();
    ensure!(view.matches(shape.size), "terrain.svg is {}x{}, but the land is {:?}", view.w, view.h, shape.size);
    let terrain = art.render_view(ART_SCALE)?;
    out.write(&terrain_path(map, variant), &encode_png(&terrain)?)?;
    out.write(&mask_path(map, variant), &mask.encode_mask())?;
    Ok(BuiltVariant { shape, mask, terrain })
}

/// A variant's terrain art: its own `terrain.svg` if it draws one, else the map's.
pub fn template_path(dir: &Path, variant: &str) -> Res<PathBuf> {
    [dir.join(variant).join("terrain.svg"), dir.join("terrain.svg")]
        .into_iter()
        .find(|path| path.is_file())
        .with_context(|| format!("{} has no terrain.svg for variant {variant}", dir.display()))
}

/// Every spawn column must hold every mobile: ground under the whole hitbox, and air for the
/// hitbox's height above it, so nothing spawns under an overhang, hanging off an edge, or on top
/// of a roof that runs off the top of the map.
pub fn check_spawns(mask: &Terrain, spawns: &[i32]) -> Void {
    ensure!(!spawns.is_empty(), "no spawns");
    check_spawn_spacing(spawns)?;
    let (wide, height) = MobileKind::spawn_room();

    let search = TILT_SEARCH_PX.value;
    for &x in spawns {
        let Landing::On(foot) = settle(mask, Footing { x, y: 0 }) else {
            bail!("spawn {x} has no ground under it");
        };
        // Either facing puts the hitbox's long side on either side of the column.
        for column in (x - wide)..=(x + wide) {
            let Some(ground) = ((foot.y - search)..=(foot.y + search)).find(|&y| mask.is_surface(column, y)) else {
                bail!("spawn {x} hangs over an edge: no ground in column {column} within {search} px of {}", foot.y);
            };
            // Off the map is air, so the roof test alone would pass a mobile standing on land that
            // runs off the top: the only surface a spawn dropped from row 0 can find there.
            ensure!(ground > 0, "spawn {x} stands on land that runs off the top of the map, in column {column}");
            if let Some(roof) = ((ground - height)..ground).find(|&y| mask.is_solid(column, y)) {
                bail!("spawn {x} is under an overhang: column {column} is solid at {roof}, within {height} px of the ground at {ground}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::art::sources;

    fn flat_with(extra: impl Fn(u32, u32) -> bool) -> Terrain {
        Terrain::from_fn(600, 300, |x, y| y >= 200 || extra(x, y)).unwrap()
    }

    #[test]
    fn spawns_need_ground_under_and_air_over_the_whole_mobile() {
        check_spawns(&flat_with(|_, _| false), &[100, 300, 500]).unwrap();
        let ledge = flat_with(|x, y| (280..330).contains(&x) && (150..160).contains(&y));
        assert!(check_spawns(&ledge, &[100]).is_ok());
        assert!(check_spawns(&ledge, &[300]).is_err(), "a spawn onto the ledge hangs over its ends");
        assert!(check_spawns(&ledge, &[350]).is_err(), "a spawn beside the ledge is under it");
        let pit = Terrain::from_fn(600, 300, |x, y| y >= 200 && !(320..340).contains(&x)).unwrap();
        assert!(check_spawns(&pit, &[300]).is_err(), "a hole under part of the hitbox");
        assert!(check_spawns(&flat_with(|_, _| false), &[100, 140]).is_err(), "two mobiles on one spot");
    }

    #[test]
    fn spawns_never_stand_on_a_roof_that_runs_off_the_top() {
        // A mine's roof: solid from the top row down to 40 over the west half.
        let roofed = flat_with(|x, y| x < 300 && y < 40);
        assert!(check_spawns(&roofed, &[450]).is_ok(), "open to the sky");
        assert!(check_spawns(&roofed, &[320]).is_ok(), "beside the roof, under its edge but far below it");
        let error = check_spawns(&roofed, &[150]).unwrap_err();
        assert!(format!("{error:#}").contains("top of the map"), "{error:#}");
        assert!(check_spawns(&roofed, &[290]).is_err(), "half on the roof");
    }

    #[test]
    fn variants_share_the_maps_terrain_art_unless_they_draw_their_own() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("A")).unwrap();
        std::fs::create_dir_all(dir.path().join("B")).unwrap();
        assert!(template_path(dir.path(), "A").is_err(), "none at all");
        std::fs::write(dir.path().join("terrain.svg"), "").unwrap();
        std::fs::write(dir.path().join("B/terrain.svg"), "").unwrap();
        assert_eq!(template_path(dir.path(), "A").unwrap(), dir.path().join("terrain.svg"));
        assert_eq!(template_path(dir.path(), "B").unwrap(), dir.path().join("B/terrain.svg"));
    }

    #[test]
    fn every_maps_spawns_are_standable() {
        let maps = sources().join("maps");
        for entry in std::fs::read_dir(&maps).unwrap() {
            let dir = entry.unwrap().path();
            let info = load_info(&dir).unwrap();
            for variant in &info.variants {
                let (_, shape) = load_shape(&dir, &variant.id).unwrap();
                check_spawns(&shape.mask().unwrap(), &variant.spawns).unwrap_or_else(|e| panic!("{} {}: {e:#}", info.name, variant.id));
            }
        }
    }

    #[test]
    fn the_lobby_offers_exactly_the_drawn_maps() {
        let mut drawn: Vec<String> = std::fs::read_dir(sources().join("maps"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        drawn.sort();
        let mut offered: Vec<String> = lobshot_assets::MAPS.iter().map(|&name| name.to_owned()).collect();
        offered.sort();
        assert_eq!(offered, drawn, "lobshot_assets::MAPS lists every map in assets/maps/, and only those");
    }

    /// The mask is the land path and nothing else: every pixel agrees with an exact inside test on
    /// the geometry, except where the outline passes within a pixel of the pixel's center (coverage
    /// rounding). Decorations or a drifted renderer would show up as solid pixels far from it.
    #[test]
    fn every_mask_is_exactly_its_land() {
        let maps = sources().join("maps");
        for entry in std::fs::read_dir(&maps).unwrap() {
            let dir = entry.unwrap().path();
            for variant in load_info(&dir).unwrap().variants {
                let (_, shape) = load_shape(&dir, &variant.id).unwrap();
                let mask = shape.mask().unwrap();
                let segments: Vec<((f64, f64), (f64, f64))> = shape
                    .polygons
                    .iter()
                    .flat_map(|polygon| polygon.iter().zip(polygon.iter().cycle().skip(1)).map(|(&a, &b)| (a, b)))
                    .collect();
                let mut near_outline = 0;
                for y in 0..mask.height() {
                    let row = f64::from(y) + 0.5;
                    let mut crossings: Vec<f64> = segments
                        .iter()
                        .filter(|((_, ay), (_, by))| (*ay <= row) != (*by <= row))
                        .map(|((ax, ay), (bx, by))| ax + (row - ay) * (bx - ax) / (by - ay))
                        .collect();
                    crossings.sort_by(f64::total_cmp);
                    for x in 0..mask.width() {
                        let column = f64::from(x) + 0.5;
                        let inside = crossings.iter().filter(|&&at| at < column).count() % 2 == 1;
                        if inside == mask.is_solid(i32::try_from(x).unwrap(), i32::try_from(y).unwrap()) {
                            continue;
                        }
                        let gap = segments.iter().map(|&(a, b)| distance_to_segment((column, row), a, b)).fold(f64::INFINITY, f64::min);
                        assert!(
                            gap < 1.0,
                            "{} {}: pixel ({x}, {y}) is {} in the mask but {gap:.2} px from the outline",
                            info_name(&dir),
                            variant.id,
                            if inside { "air" } else { "solid" }
                        );
                        near_outline += 1;
                    }
                }
                let outline: f64 = segments.iter().map(|&(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum();
                assert!(f64::from(near_outline) < outline / 4.0, "{near_outline} pixels disagree along {outline:.0} px of outline");
            }
        }
    }

    fn info_name(dir: &Path) -> String {
        dir.file_name().unwrap().to_string_lossy().into_owned()
    }

    fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx * dx + dy * dy;
        let t = if length == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0) };
        (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
    }
}
