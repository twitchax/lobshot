//! Loading the generated assets (`lobshot_assets` layout, served under `assets/`): raw bytes for
//! masks and manifests, and the `MapAssets` the sim needs to rebuild a drawn map's terrain.

use std::collections::HashMap;

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    ecs::system::SystemParam,
    prelude::*,
};
use lobshot_sim::{base::Res as SimRes, map::MapAssets, terrain::Terrain};

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

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Blob>().init_asset_loader::<BlobLoader>().init_resource::<MaskHandles>();
    }
}
