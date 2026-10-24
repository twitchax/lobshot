//! The asset layout and manifests, in one place for the writer (`cargo xtask art`) and the readers
//! (the client draws them; the server and the lab load map masks and spawns through `maps`).
//!
//! Sources live in `assets/` (SVG, hand-written RON and Ogg sounds, CC0). `cargo xtask art`
//! renders them into `assets/gen/` (gitignored), which trunk copies into the bundle at `assets/`.
//! Paths below are relative to that root. Every rendered image is drawn at `ART_SCALE`, and its
//! center is its SVG's origin (a part's pivot, a rider's seat), so a sprite anchors at its center.

pub mod maps;
pub mod tests;

use std::collections::BTreeSet;

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use lobshot_sim::{base::Res, drops::DropKind, game::MobileKind, items::Item};

/// Rendered images have this many pixels per world pixel.
pub const ART_SCALE: f64 = 2.0;

/// A mobile's directory name, from its lobshot name (`coilback`).
#[must_use]
pub fn mobile_slug(mobile: MobileKind) -> String {
    mobile.tuning().name.to_lowercase()
}

/// A mobile's rig manifest.
#[must_use]
pub fn rig_path(mobile: MobileKind) -> String {
    format!("mobiles/{}/rig.ron", mobile_slug(mobile))
}

/// A rendered rig part, from the part's `file` in the rig (`body.svg` renders to `body.png`).
#[must_use]
pub fn part_path(mobile: MobileKind, file: &str) -> String {
    format!("mobiles/{}/{}", mobile_slug(mobile), png_name(file))
}

/// A rendered rider.
#[must_use]
pub fn rider_path(rider: &str) -> String {
    format!("riders/{rider}.png")
}

/// The drawn maps a room can pick, in the order the lobby offers them; `cargo xtask art` checks
/// this matches `assets/maps/`.
pub const MAPS: &[&str] = &[
    "tidewall",
    "gablewick",
    "rimewake",
    "glintvein",
    "ashspine",
    "clockrise",
    "tumblehill",
    "neonbay",
    "rootmere",
    "starmote",
];

/// The synthetic map the server builds in, for testing: one flat field. Not drawn, so not one of
/// `MAPS`; the lobby offers it after them.
pub const FLAT: &str = "flat";

/// A map's manifest.
#[must_use]
pub fn map_path(map: &str) -> String {
    format!("maps/{map}/map.ron")
}

/// A map's background, shared by its variants.
#[must_use]
pub fn background_path(map: &str) -> String {
    format!("maps/{map}/background.png")
}

/// A map variant's rendered terrain.
#[must_use]
pub fn terrain_path(map: &str, variant: &str) -> String {
    format!("maps/{map}/{variant}/terrain.png")
}

/// A map variant's collision mask, in `lobshot_sim::terrain::Terrain`'s mask format.
#[must_use]
pub fn mask_path(map: &str, variant: &str) -> String {
    format!("maps/{map}/{variant}/mask.bin")
}

/// An item's file name, from its player-facing name (`Dual+` is `dual_plus`, `Power Up` is
/// `power_up`).
#[must_use]
pub fn item_slug(item: Item) -> String {
    item.name().to_lowercase().replace('+', "_plus").replace(' ', "_")
}

/// An item's icon: square for a one-slot item, twice as wide for a two-slot one.
#[must_use]
pub fn item_path(item: Item) -> String {
    format!("items/{}.png", item_slug(item))
}

/// A slot-machine drop's sprite, drawn as it falls (`drops/dynamite.png`).
#[must_use]
pub fn drop_path(drop: DropKind) -> String {
    format!("drops/{}.png", format!("{drop:?}").to_lowercase())
}

/// A sound effect: one CC0 Ogg Vorbis file each, copied unchanged from `assets/sounds/`, whose
/// `CREDITS.md` says where each came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sound {
    /// A shot leaves the cannon.
    Fire,
    /// A shell explodes.
    Blast,
    /// A mobile takes damage.
    Hit,
    /// A mobile is destroyed.
    Death,
    /// A mobile lands after a fall.
    Land,
    /// Your turn begins.
    Turn,
    /// Each of your turn's last seconds.
    Tick,
    /// A button press.
    Click,
    /// A chat line arrives.
    Chat,
    /// Your team won.
    Win,
    /// Your team lost.
    Lose,
}

impl Sound {
    pub const ALL: [Self; 11] = [
        Self::Fire,
        Self::Blast,
        Self::Hit,
        Self::Death,
        Self::Land,
        Self::Turn,
        Self::Tick,
        Self::Click,
        Self::Chat,
        Self::Win,
        Self::Lose,
    ];

    /// The file stem, and the name the browser smoke test reads (`blast`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Blast => "blast",
            Self::Hit => "hit",
            Self::Death => "death",
            Self::Land => "land",
            Self::Turn => "turn",
            Self::Tick => "tick",
            Self::Click => "click",
            Self::Chat => "chat",
            Self::Win => "win",
            Self::Lose => "lose",
        }
    }
}

/// A sound effect's file, the same in the sources (`assets/`) and the output.
#[must_use]
pub fn sound_path(sound: Sound) -> String {
    format!("sounds/{}.ogg", sound.name())
}

fn png_name(file: &str) -> String {
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    format!("{stem}.png")
}

/// A mobile's cutout rig: the art only. Gameplay geometry (hitbox, aim range, ground probes) lives
/// in `lobshot_sim::tuning`; `cargo xtask art` checks the rig's cannon (and second barrel) agrees
/// with it.
///
/// Body frame: origin at the center of the ground contact, `+x` forward, `y` down, world pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MobileRig {
    pub name: String,
    /// Drawn in `z` order, lowest first.
    pub parts: Vec<Part>,
    pub cannon: Cannon,
    /// A second barrel some shots leave instead of the cannon (Frostfin's tusk): the art of
    /// `MobileTuning::second_barrel`. Most rigs have none.
    #[serde(default)]
    pub second_barrel: Option<Cannon>,
    /// Where the rider sits.
    pub rider_socket: Socket,
    /// The source SVG of the mobile's own satellite (Lensowl's, Needlemouse's), next to the rig:
    /// drawn hovering over it, apart from the body, where `MobileKind::satellite` puts it, its
    /// pivot at the point it fires from. Most rigs have none.
    #[serde(default)]
    pub satellite: Option<String>,
    /// Pictures the mobile's shots are drawn with, in place of the plain shell: by `id`, `s1`, `s2`
    /// and `ss` for each shot's projectiles, and `mine` for what its mines look like on the map.
    /// Each renders centered on its SVG's origin, like a part. Most rigs have none.
    #[serde(default)]
    pub sprites: Vec<ShotSprite>,
}

/// One picture a mobile's shots are drawn with (`MobileRig::sprites`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShotSprite {
    pub id: String,
    /// The source SVG, next to the rig; the rendered PNG has the same stem.
    pub file: String,
}

/// One cutout part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    /// The source SVG, next to the rig; the rendered PNG has the same stem.
    pub file: String,
    /// Where the part's pivot sits in the body frame.
    pub at: (f64, f64),
    pub z: i32,
    /// Rolls as the mobile moves (wheels).
    #[serde(default)]
    pub spin: bool,
    /// Turns with the aim: the cannon, or a second barrel.
    #[serde(default)]
    pub aim: bool,
}

/// A barrel: which part aims, and where shots leave it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cannon {
    pub part: String,
    /// In the cannon part's frame at aim 0 (pointing `+x`).
    pub muzzle: (f64, f64),
}

/// A point to hang something on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Socket {
    pub at: (f64, f64),
    pub z: i32,
}

/// A map: its variants, and where mobiles may spawn on each.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapInfo {
    /// The directory name (`tidewall`).
    pub name: String,
    /// What players see (`Tidewall`).
    pub title: String,
    pub variants: Vec<MapVariant>,
}

/// One variant of a map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapVariant {
    /// `A` or `B`.
    pub id: String,
    /// Columns a mobile may spawn in; the server picks from these.
    pub spawns: Vec<i32>,
    /// World size in pixels, and the mask's hash. Absent in the hand-written source; `cargo
    /// xtask art` fills them in the generated manifest.
    #[serde(default)]
    pub size: Option<(u32, u32)>,
    #[serde(default)]
    pub mask_hash: Option<u64>,
}

impl MapInfo {
    /// Reads a manifest.
    ///
    /// # Errors
    ///
    /// Fails on malformed RON or a map with no variants.
    pub fn from_ron(text: &str) -> Res<Self> {
        let info: Self = ron::from_str(text).context("reading a map manifest")?;
        ensure!(!info.variants.is_empty(), "map {} has no variants", info.name);
        Ok(info)
    }

    #[must_use]
    pub fn variant(&self, id: &str) -> Option<&MapVariant> {
        self.variants.iter().find(|v| v.id == id)
    }
}

impl MobileRig {
    /// Reads a manifest.
    ///
    /// # Errors
    ///
    /// Fails on malformed RON, or a cannon or second barrel that names no part.
    pub fn from_ron(text: &str) -> Res<Self> {
        let rig: Self = ron::from_str(text).context("reading a rig")?;
        for barrel in rig.barrels() {
            ensure!(rig.parts.iter().any(|p| p.id == barrel.part), "{}'s barrel names no part {:?}", rig.name, barrel.part);
        }
        Ok(rig)
    }

    /// Every source SVG it draws from, each once: its parts', its satellite's if it has one, and
    /// its shots' sprites.
    #[must_use]
    pub fn files(&self) -> BTreeSet<&str> {
        self.parts
            .iter()
            .map(|part| part.file.as_str())
            .chain(self.satellite.as_deref())
            .chain(self.sprites.iter().map(|sprite| sprite.file.as_str()))
            .collect()
    }

    /// The cannon, then the second barrel if there is one.
    pub fn barrels(&self) -> impl Iterator<Item = &Cannon> {
        std::iter::once(&self.cannon).chain(&self.second_barrel)
    }

    /// The sprite named `id`, if the rig draws one.
    #[must_use]
    pub fn sprite(&self, id: &str) -> Option<&ShotSprite> {
        self.sprites.iter().find(|sprite| sprite.id == id)
    }
}

/// Writes a manifest back out (the generated copies).
///
/// # Errors
///
/// Fails only if serialization does.
pub fn to_ron<T: Serialize>(value: &T) -> Res<String> {
    ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).context("writing RON")
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn paths_follow_the_layout() {
        assert_eq!(rig_path(MobileKind::Coilback), "mobiles/coilback/rig.ron");
        assert_eq!(part_path(MobileKind::Coilback, "wheel_big.svg"), "mobiles/coilback/wheel_big.png");
        assert_eq!(mask_path("tidewall", "A"), "maps/tidewall/A/mask.bin");
        assert_eq!(rider_path("pip"), "riders/pip.png");
        assert_eq!(sound_path(Sound::Blast), "sounds/blast.ogg");
        assert_eq!(item_path(Item::DualPlus), "items/dual_plus.png");
        assert_eq!(item_path(Item::BigHeal), "items/big_heal.png");
        assert_eq!(drop_path(DropKind::Dynamite), "drops/dynamite.png");
    }

    #[test]
    fn the_built_in_map_is_none_of_the_drawn_ones() {
        assert!(!MAPS.contains(&FLAT), "{FLAT} names the server's synthetic field, which has no art");
    }

    #[test]
    fn every_item_and_drop_has_its_own_file() {
        let mut paths: Vec<String> = Item::ALL.iter().map(|&item| item_path(item)).chain(DropKind::ALL.iter().map(|&drop| drop_path(drop))).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), Item::ALL.len() + DropKind::ALL.len());
    }

    #[test]
    fn every_sound_has_its_own_file() {
        let mut paths: Vec<String> = Sound::ALL.iter().map(|&sound| sound_path(sound)).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), Sound::ALL.len());
    }

    #[test]
    fn reads_a_hand_written_map_and_round_trips_a_generated_one() {
        let source = r#"MapInfo(name: "pond", title: "Pond", variants: [(id: "A", spawns: [100, 700])])"#;
        let mut info = MapInfo::from_ron(source).unwrap();
        assert_eq!(info.variant("A").unwrap().size, None);
        info.variants[0].size = Some((800, 600));
        info.variants[0].mask_hash = Some(42);
        assert_eq!(MapInfo::from_ron(&to_ron(&info).unwrap()).unwrap(), info);
        assert!(MapInfo::from_ron(r#"MapInfo(name: "x", title: "X", variants: [])"#).is_err());
    }

    #[test]
    fn reads_a_rig() {
        let source = r#"MobileRig(
            name: "Coilback",
            parts: [(id: "body", file: "body.svg", at: (0.0, 0.0), z: 0), (id: "cannon", file: "cannon.svg", at: (7.0, -48.0), z: 3, aim: true)],
            cannon: (part: "cannon", muzzle: (28.8, 0.0)),
            rider_socket: (at: (-12.0, -56.0), z: 4),
        )"#;
        let rig = MobileRig::from_ron(source).unwrap();
        assert!(rig.parts[1].aim && !rig.parts[0].spin);
        assert_eq!(rig.second_barrel, None, "a rig without one has no second barrel");
        assert_eq!(rig.satellite, None, "nor a satellite");
        assert_eq!(rig.files(), BTreeSet::from(["body.svg", "cannon.svg"]));
        assert!(MobileRig::from_ron(&source.replace(r#"part: "cannon""#, r#"part: "nope""#)).is_err());
    }

    #[test]
    fn reads_a_rig_with_a_satellite() {
        let source = r#"MobileRig(
            name: "Lensowl",
            parts: [(id: "body", file: "body.svg", at: (0.0, 0.0), z: 0), (id: "cannon", file: "cannon.svg", at: (14.0, -36.0), z: 3, aim: true)],
            cannon: (part: "cannon", muzzle: (30.0, 0.0)),
            rider_socket: (at: (-12.0, -50.0), z: 4),
            satellite: Some("satellite.svg"),
        )"#;
        let rig = MobileRig::from_ron(source).unwrap();
        assert_eq!(rig.files(), BTreeSet::from(["body.svg", "cannon.svg", "satellite.svg"]));
        assert_eq!(MobileRig::from_ron(&to_ron(&rig).unwrap()).unwrap(), rig, "round trips");
    }

    #[test]
    fn reads_a_rig_with_shot_sprites() {
        let source = r#"MobileRig(
            name: "Tinbadger",
            parts: [(id: "body", file: "body.svg", at: (0.0, 0.0), z: 0), (id: "cannon", file: "cannon.svg", at: (30.0, -38.0), z: 3, aim: true)],
            cannon: (part: "cannon", muzzle: (24.0, 0.0)),
            rider_socket: (at: (14.0, -36.0), z: 4),
            sprites: [(id: "s2", file: "mine.svg"), (id: "mine", file: "mine.svg"), (id: "ss", file: "beetle.svg")],
        )"#;
        let rig = MobileRig::from_ron(source).unwrap();
        assert_eq!(rig.files(), BTreeSet::from(["beetle.svg", "body.svg", "cannon.svg", "mine.svg"]), "each file once");
        assert_eq!(rig.sprite("mine").map(|sprite| sprite.file.as_str()), Some("mine.svg"));
        assert_eq!(rig.sprite("s1"), None);
        assert_eq!(MobileRig::from_ron(&to_ron(&rig).unwrap()).unwrap(), rig, "round trips");
    }

    #[test]
    fn reads_a_rig_with_a_second_barrel() {
        let source = r#"MobileRig(
            name: "Frostfin",
            parts: [(id: "body", file: "body.svg", at: (0.0, 0.0), z: 0), (id: "cannon", file: "cannon.svg", at: (5.0, -47.0), z: 3, aim: true), (id: "tusk", file: "tusk.svg", at: (26.0, -26.0), z: 2, aim: true)],
            cannon: (part: "cannon", muzzle: (25.0, 0.0)),
            second_barrel: Some((part: "tusk", muzzle: (30.0, 0.0))),
            rider_socket: (at: (-16.0, -44.0), z: 4),
        )"#;
        let rig = MobileRig::from_ron(source).unwrap();
        assert_eq!(rig.barrels().map(|barrel| barrel.part.as_str()).collect::<Vec<_>>(), ["cannon", "tusk"]);
        assert!(MobileRig::from_ron(&source.replace(r#"part: "tusk", muzzle"#, r#"part: "nope", muzzle"#)).is_err());
        assert_eq!(MobileRig::from_ron(&to_ron(&rig).unwrap()).unwrap(), rig, "round trips");
    }
}
