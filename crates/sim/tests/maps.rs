//! Every drawn map in `assets/maps/` has its weather odds in `sim::tuning::weather`, so a new map
//! can't quietly roll the practice slope's.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use lobshot_sim::tuning::weather::moondisk;

#[test]
fn every_drawn_map_variant_has_weather_odds() {
    let maps = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps");
    let mut checked = 0;
    for entry in std::fs::read_dir(&maps).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_str().unwrap().to_owned();
        let manifest = std::fs::read_to_string(dir.join("map.ron")).unwrap();
        for variant in manifest.split("(id: \"").skip(1).filter_map(|rest| rest.split_once('"').map(|(id, _)| id)) {
            assert!(moondisk(&name, variant).is_some(), "{name} {variant} has no weather odds in sim::tuning::weather");
            checked += 1;
        }
    }
    assert!(checked >= 10, "found only {checked} variants; is the manifest format still `(id: \"A\", ...)`?");
}
