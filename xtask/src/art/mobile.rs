//! Mobiles and riders: every rig part and rider rendered pivot-centered, and each rig checked
//! against the gameplay geometry in `sim::tuning` before it ships.

use std::{collections::BTreeSet, path::Path};

use anyhow::{Context as _, ensure};
use lobshot_assets::{MobileRig, mobile_slug, part_path, rider_path, rig_path, to_ron};
use lobshot_sim::{
    base::{Res, Void},
    game::MobileKind,
    tuning::MobileTuning,
};

use super::{
    ART_SCALE,
    out::Out,
    raster::{Svg, encode_png},
};

/// Reads a mobile's rig from `assets/mobiles/<slug>/rig.ron` and checks it against its tuning.
pub fn load_rig(sources: &Path, mobile: MobileKind) -> Res<MobileRig> {
    let path = sources.join(rig_path(mobile));
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let rig = MobileRig::from_ron(&text).with_context(|| format!("reading {}", path.display()))?;
    check_rig(&rig, mobile.tuning()).with_context(|| format!("checking {}", path.display()))?;
    Ok(rig)
}

/// The art must agree with the gameplay geometry (`DESIGN.md`, "Art"): the cannon pivots where
/// the sim aims from, and its muzzle sits a barrel's length along it.
pub fn check_rig(rig: &MobileRig, tuning: &MobileTuning) -> Void {
    ensure!(rig.name == tuning.name, "the rig is named {:?}, but its tuning {:?}", rig.name, tuning.name);
    let mut ids = BTreeSet::new();
    for part in &rig.parts {
        ensure!(ids.insert(&part.id), "two parts are named {:?}", part.id);
        ensure!(!(part.spin && part.aim), "{} both spins and aims", part.id);
    }
    let aiming: Vec<&str> = rig.parts.iter().filter(|part| part.aim).map(|part| part.id.as_str()).collect();
    ensure!(aiming == [rig.cannon.part.as_str()], "exactly the cannon part must aim, but {aiming:?} do");

    let cannon = rig.parts.iter().find(|part| part.id == rig.cannon.part).context("the cannon names no part")?;
    let pivot = tuning.cannon_pivot.value;
    ensure!(close(cannon.at, pivot), "the cannon pivots at {:?}, but sim::tuning aims from {pivot:?}", cannon.at);
    let barrel = tuning.barrel_length.value;
    ensure!(
        close(rig.cannon.muzzle, (barrel, 0.0)),
        "the muzzle is at {:?}, but sim::tuning fires from ({barrel}, 0) along the barrel",
        rig.cannon.muzzle
    );
    Ok(())
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

/// Renders every mobile's parts and writes its rig beside them.
pub fn build_mobiles(sources: &Path, out: &Out) -> Res<Vec<(MobileKind, MobileRig)>> {
    let mobiles_dir = sources.join("mobiles");
    let known: BTreeSet<String> = MobileKind::ALL.iter().map(|&mobile| mobile_slug(mobile)).collect();
    for entry in std::fs::read_dir(&mobiles_dir).with_context(|| format!("listing {}", mobiles_dir.display()))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        ensure!(known.contains(&name), "assets/mobiles/{name} is no MobileKind; add the mobile to sim first");
    }

    let mut rigs = Vec::with_capacity(MobileKind::ALL.len());
    for mobile in MobileKind::ALL {
        let rig = load_rig(sources, mobile)?;
        let files: BTreeSet<&str> = rig.parts.iter().map(|part| part.file.as_str()).collect();
        for file in files {
            let path = sources.join("mobiles").join(mobile_slug(mobile)).join(file);
            let png = encode_png(&Svg::load(&path)?.render_centered(ART_SCALE)?)?;
            out.write(&part_path(mobile, file), &png)?;
        }
        // The validated rig, normalized: what was checked is exactly what ships.
        out.write(&rig_path(mobile), to_ron(&rig)?.as_bytes())?;
        rigs.push((mobile, rig));
    }
    Ok(rigs)
}

/// Renders every rider in `assets/riders/`, seat point centered. Returns their names.
pub fn build_riders(sources: &Path, out: &Out) -> Res<Vec<String>> {
    let dir = sources.join("riders");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "svg") {
            let name = path.file_stem().context("a rider file with no name")?.to_string_lossy().into_owned();
            names.push(name);
        }
    }
    // Directory order is the file system's; sort so the build is the same everywhere.
    names.sort();
    for name in &names {
        let png = encode_png(&Svg::load(&dir.join(format!("{name}.svg")))?.render_centered(ART_SCALE)?)?;
        out.write(&rider_path(name), &png)?;
    }
    Ok(names)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::art::{raster::alpha, sources};

    #[test]
    fn every_rig_parses_and_agrees_with_sim_tuning() {
        for mobile in MobileKind::ALL {
            let rig = load_rig(&sources(), mobile).unwrap();
            let cannon = rig.parts.iter().find(|part| part.id == rig.cannon.part).unwrap();
            assert_eq!(cannon.at, mobile.tuning().cannon_pivot.value);
            assert!((rig.cannon.muzzle.0.hypot(rig.cannon.muzzle.1) - mobile.tuning().barrel_length.value).abs() < 1e-9);
        }
    }

    #[test]
    fn a_rig_that_drifts_from_tuning_is_refused() {
        let tuning = MobileKind::Coilback.tuning();
        let mut rig = load_rig(&sources(), MobileKind::Coilback).unwrap();
        check_rig(&rig, tuning).unwrap();

        let pivot = rig.parts.iter_mut().find(|part| part.aim).unwrap();
        pivot.at.0 += 1.0;
        assert!(check_rig(&rig, tuning).is_err(), "pivot");

        let mut rig = load_rig(&sources(), MobileKind::Coilback).unwrap();
        rig.cannon.muzzle.0 += 2.0;
        assert!(check_rig(&rig, tuning).is_err(), "barrel length");

        let mut rig = load_rig(&sources(), MobileKind::Coilback).unwrap();
        rig.parts[0].aim = true;
        assert!(check_rig(&rig, tuning).is_err(), "a second aiming part");
    }

    /// A wheel is round about its axle, so if the image's center is the pivot, its drawn pixels
    /// are centered too. Every mobile's spinning parts, since a wheel off its axle wobbles.
    #[test]
    fn parts_render_pivot_centered() {
        let spinning = MobileKind::ALL.iter().flat_map(|&mobile| {
            let rig = load_rig(&sources(), mobile).unwrap();
            rig.parts.into_iter().filter(|part| part.spin).map(move |part| (mobile, part))
        });
        for (mobile, part) in spinning {
            let path = sources().join("mobiles").join(mobile_slug(mobile)).join(&part.file);
            let pixmap = Svg::load(&path).unwrap().render_centered(ART_SCALE).unwrap();
            let (w, h) = (pixmap.width(), pixmap.height());
            assert!(w % 2 == 0 && h % 2 == 0, "{} is {w}x{h}: an odd side puts the center inside a pixel", part.file);
            let alpha = alpha(&pixmap);
            let (mut sx, mut sy, mut total) = (0.0, 0.0, 0.0);
            for (y, row) in (0..h).zip(alpha.chunks(w as usize)) {
                for (x, &a) in (0..w).zip(row) {
                    sx += (f64::from(x) + 0.5) * f64::from(a);
                    sy += (f64::from(y) + 0.5) * f64::from(a);
                    total += f64::from(a);
                }
            }
            let (cx, cy) = (sx / total - f64::from(w) / 2.0, sy / total - f64::from(h) / 2.0);
            assert!(cx.abs() < 1.0 && cy.abs() < 1.0, "{}'s coverage is centered {cx:.2},{cy:.2} px off its pivot", part.file);
        }
    }
}
