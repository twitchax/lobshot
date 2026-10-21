//! Item icons and slot-machine drops: each rendered at `ART_SCALE` with its SVG's origin at the
//! image's center, after a check that it has the shape the client lays it out in. The client sizes
//! an icon as wide as the slots its item fills, and a drop as a square.

use std::{
    collections::BTreeSet,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};
use lobshot_assets::{drop_path, item_path};
use lobshot_sim::{base::Void, drops::DropKind, items::Item};

use super::{
    ART_SCALE,
    out::Out,
    raster::{Svg, ViewBox, encode_png},
};

/// An icon's height in world pixels, and its width per slot its item fills.
pub const ICON_SIDE: u32 = 40;

/// The sides a drop sprite may have, in world pixels.
const DROP_SIDES: RangeInclusive<f32> = 24.0..=32.0;

/// An item icon's source: its rendered path in the layout, as SVG under `sources`.
pub fn item_source(sources: &Path, item: Item) -> PathBuf {
    sources.join(item_path(item)).with_extension("svg")
}

/// A drop sprite's source, the same way.
pub fn drop_source(sources: &Path, drop: DropKind) -> PathBuf {
    sources.join(drop_path(drop)).with_extension("svg")
}

/// Renders every item icon and drop sprite, refusing a missing one, a misshapen one, or a source
/// that belongs to no item or drop.
pub fn build_icons(sources: &Path, out: &mut Out) -> Void {
    refuse_strays(&sources.join("items"), Item::ALL.iter().map(|&item| item_source(sources, item)), "Item")?;
    refuse_strays(&sources.join("drops"), DropKind::ALL.iter().map(|&drop| drop_source(sources, drop)), "DropKind")?;
    for item in Item::ALL {
        let path = item_source(sources, item);
        let svg = Svg::load(&path)?;
        check_icon(svg.view(), item.slots()).with_context(|| format!("checking {}", path.display()))?;
        out.write(&item_path(item), &encode_png(&svg.render_centered(ART_SCALE)?)?)?;
    }
    for drop in DropKind::ALL {
        let path = drop_source(sources, drop);
        let svg = Svg::load(&path)?;
        check_drop(svg.view()).with_context(|| format!("checking {}", path.display()))?;
        out.write(&drop_path(drop), &encode_png(&svg.render_centered(ART_SCALE)?)?)?;
    }
    Ok(())
}

/// Every file in `dir` must be one of `expected`: art for an item sim doesn't have would never ship.
fn refuse_strays(dir: &Path, expected: impl Iterator<Item = PathBuf>, kind: &str) -> Void {
    let expected: BTreeSet<PathBuf> = expected.collect();
    for entry in std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
        let path = entry?.path();
        ensure!(expected.contains(&path), "{} is no {kind}; add it to sim first", path.display());
    }
    Ok(())
}

/// One slot is a square `ICON_SIDE` on a side; a two-slot item's icon is twice as wide.
fn check_icon(view: ViewBox, slots: u8) -> Void {
    ensure!(centered(view), "the viewBox must be centered on the origin, so the image is exactly the icon");
    let size = (ICON_SIDE * u32::from(slots), ICON_SIDE);
    ensure!(view.matches(size), "the icon is {}x{}, but a {slots}-slot item's is {}x{}", view.w, view.h, size.0, size.1);
    Ok(())
}

/// A drop is square (the client shows it in a square reel) and between 24 and 32 on a side.
fn check_drop(view: ViewBox) -> Void {
    ensure!(centered(view), "the viewBox must be centered on the origin, so the image is exactly the sprite");
    ensure!((view.w - view.h).abs() < 1e-3, "a drop is square, not {}x{}", view.w, view.h);
    ensure!(DROP_SIDES.contains(&view.w), "a drop is {DROP_SIDES:?} on a side, not {}", view.w);
    Ok(())
}

fn centered(view: ViewBox) -> bool {
    (view.x + view.w / 2.0).abs() < 1e-3 && (view.y + view.h / 2.0).abs() < 1e-3
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;
    use resvg::tiny_skia::Pixmap;
    use tempfile::TempDir;

    use super::*;
    use crate::art::sources;

    fn view(x: f32, y: f32, w: f32, h: f32) -> ViewBox {
        ViewBox { x, y, w, h }
    }

    #[test]
    fn every_item_and_drop_ships_its_art() {
        let dir = TempDir::new().unwrap();
        build_icons(&sources(), &mut Out::new(dir.path())).unwrap();
        let png = |path: &str| Pixmap::decode_png(&std::fs::read(dir.path().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
        let side = ICON_SIDE * 2;
        for item in Item::ALL {
            let icon = png(&item_path(item));
            assert_eq!((icon.width(), icon.height()), (side * u32::from(item.slots()), side), "{item:?}: as wide as its slots, at ART_SCALE");
        }
        for drop in DropKind::ALL {
            let sprite = png(&drop_path(drop));
            assert_eq!(sprite.width(), sprite.height(), "{drop:?}");
            assert!((48..=64).contains(&sprite.width()), "{drop:?} is {} px", sprite.width());
        }
    }

    #[test]
    fn an_icon_is_as_wide_as_its_slots() {
        check_icon(view(-20.0, -20.0, 40.0, 40.0), 1).unwrap();
        check_icon(view(-40.0, -20.0, 80.0, 40.0), 2).unwrap();
        assert!(check_icon(view(-20.0, -20.0, 40.0, 40.0), 2).is_err(), "a two-slot item drawn square");
        assert!(check_icon(view(-40.0, -20.0, 80.0, 40.0), 1).is_err(), "a one-slot item drawn wide");
        assert!(check_icon(view(0.0, 0.0, 40.0, 40.0), 1).is_err(), "off center: render_centered would pad it");
    }

    #[test]
    fn a_drop_is_a_drop_sized_square() {
        check_drop(view(-14.0, -14.0, 28.0, 28.0)).unwrap();
        assert!(check_drop(view(-14.0, -10.0, 28.0, 20.0)).is_err(), "not square");
        assert!(check_drop(view(-30.0, -30.0, 60.0, 60.0)).is_err(), "too big");
        assert!(check_drop(view(0.0, 0.0, 28.0, 28.0)).is_err(), "off center");
    }

    #[test]
    fn stray_or_missing_art_is_refused() {
        let copy = || {
            let dir = TempDir::new().unwrap();
            for sub in ["items", "drops"] {
                std::fs::create_dir_all(dir.path().join(sub)).unwrap();
                for entry in std::fs::read_dir(sources().join(sub)).unwrap() {
                    let path = entry.unwrap().path();
                    std::fs::copy(&path, dir.path().join(sub).join(path.file_name().unwrap())).unwrap();
                }
            }
            dir
        };
        let out = TempDir::new().unwrap();

        let stray = copy();
        std::fs::write(stray.path().join("items/jetpack.svg"), "").unwrap();
        let error = build_icons(stray.path(), &mut Out::new(out.path())).unwrap_err();
        assert!(format!("{error:#}").contains("jetpack.svg is no Item"), "{error:#}");

        let missing = copy();
        std::fs::remove_file(drop_source(missing.path(), DropKind::Chest)).unwrap();
        assert!(build_icons(missing.path(), &mut Out::new(out.path())).is_err(), "no chest");
    }
}
