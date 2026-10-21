//! Loading the generated assets (`lobshot_assets` layout, served under `assets/`): raw bytes for
//! masks and manifests, the `MapAssets` the sim needs to rebuild a drawn map's terrain, the
//! mobiles' rigs, and the images the screens keep coming back to.

use std::collections::HashMap;

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    ecs::system::SystemParam,
    prelude::*,
};
use lobshot_assets::MobileRig;
use lobshot_sim::{base::Res as SimRes, game::MobileKind, map::MapAssets, terrain::Terrain};

/// A file's bytes, for formats Bevy doesn't know (masks, RON manifests).
#[derive(Asset, TypePath, Debug)]
pub struct Blob(pub Vec<u8>);

#[derive(Default, TypePath)]
pub struct BlobLoader;

impl AssetLoader for BlobLoader {
    type Asset = Blob;
    type Error = std::io::Error;
    type Settings = ();

    async fn load(&self, reader: &mut dyn Reader, _settings: &(), _context: &mut LoadContext<'_>) -> Result<Blob, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Blob(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["bin", "ron"]
    }
}

/// Mask files requested so far, by map and variant.
#[derive(Resource, Default)]
pub struct MaskHandles(HashMap<(String, String), Handle<Blob>>);

/// Drawn maps' masks, loaded on demand.
#[derive(SystemParam)]
pub struct Masks<'w> {
    server: Res<'w, AssetServer>,
    blobs: Res<'w, Assets<Blob>>,
    handles: ResMut<'w, MaskHandles>,
}

impl Masks<'_> {
    /// Starts loading a mask (once).
    pub fn request(&mut self, name: &str, variant: &str) {
        let key = (name.to_owned(), variant.to_owned());
        if !self.handles.0.contains_key(&key) {
            let handle = self.server.load(lobshot_assets::mask_path(name, variant));
            self.handles.0.insert(key, handle);
        }
    }

    /// Whether a mask has arrived.
    #[must_use]
    pub fn ready(&self, name: &str, variant: &str) -> bool {
        self.handles.0.get(&(name.to_owned(), variant.to_owned())).is_some_and(|handle| self.blobs.contains(handle))
    }
}

impl MapAssets for Masks<'_> {
    fn mask(&self, name: &str, variant: &str) -> SimRes<Terrain> {
        let handle = self
            .handles
            .0
            .get(&(name.to_owned(), variant.to_owned()))
            .ok_or_else(|| anyhow::anyhow!("{name} {variant}'s mask was never requested"))?;
        let blob = self.blobs.get(handle).ok_or_else(|| anyhow::anyhow!("{name} {variant}'s mask hasn't loaded"))?;
        Terrain::decode_mask(&blob.0)
    }
}

/// Rigs by mobile: each manifest is requested once and parsed when it arrives, for the game's
/// mobiles and the lobby's portraits alike.
#[derive(Resource, Default)]
pub struct Rigs {
    manifests: HashMap<MobileKind, Handle<Blob>>,
    parsed: HashMap<MobileKind, MobileRig>,
}

impl Rigs {
    /// Starts loading a mobile's rig (once).
    pub fn request(&mut self, server: &AssetServer, mobile: MobileKind) {
        self.manifests.entry(mobile).or_insert_with(|| server.load(lobshot_assets::rig_path(mobile)));
    }

    /// A rig, once it has loaded.
    #[must_use]
    pub fn get(&self, mobile: MobileKind) -> Option<&MobileRig> {
        self.parsed.get(&mobile)
    }
}

fn parse_rigs(mut rigs: ResMut<Rigs>, blobs: Res<Assets<Blob>>) {
    if rigs.parsed.len() == rigs.manifests.len() {
        return;
    }
    let ready: Vec<(MobileKind, MobileRig)> = rigs
        .manifests
        .iter()
        .filter(|(kind, _)| !rigs.parsed.contains_key(*kind))
        .filter_map(|(kind, handle)| {
            let text = std::str::from_utf8(&blobs.get(handle)?.0).ok()?;
            match MobileRig::from_ron(text) {
                Ok(rig) => Some((*kind, rig)),
                Err(error) => {
                    error!("{kind:?}'s rig: {error:#}");
                    None
                }
            }
        })
        .collect();
    rigs.parsed.extend(ready);
}

/// Images the screens rebuild around (portraits, item icons, drop sprites), held for the page's
/// life: a lobby redrawn on every change would otherwise drop the last handle and fetch the file
/// again.
#[derive(Resource, Default)]
pub struct ImageCache(HashMap<String, Handle<Image>>);

impl ImageCache {
    /// The image at `path` (in the `lobshot_assets` layout), loading it the first time.
    pub fn get(&mut self, server: &AssetServer, path: String) -> Handle<Image> {
        self.0.entry(path).or_insert_with_key(|path| server.load(path.clone())).clone()
    }
}

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Blob>()
            .init_asset_loader::<BlobLoader>()
            .init_resource::<MaskHandles>()
            .init_resource::<Rigs>()
            .init_resource::<ImageCache>()
            .add_systems(Update, parse_rigs);
    }
}
