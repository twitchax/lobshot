//! The output tree: every file the build writes goes through here, so it can report sizes and
//! clear out what a previous build left behind.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use anyhow::Context as _;
use lobshot_sim::base::{Res, Void};

/// Shared by the threads that render in parallel; each file is written whole, by one of them.
pub struct Out {
    root: PathBuf,
    /// Relative path to size, in path order.
    written: Mutex<BTreeMap<String, u64>>,
}

impl Out {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
            written: Mutex::new(BTreeMap::new()),
        }
    }

    /// Writes `bytes` at `path`, relative to the root (`maps/tidewall/map.ron`).
    pub fn write(&self, path: &str, bytes: &[u8]) -> Void {
        let full = self.root.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&full, bytes).with_context(|| format!("writing {}", full.display()))?;
        // A thread that panicked mid-insert can't leave a half-written entry behind.
        self.written.lock().unwrap_or_else(PoisonError::into_inner).insert(path.to_owned(), bytes.len() as u64);
        Ok(())
    }

    pub fn written(&self) -> BTreeMap<String, u64> {
        self.written.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Deletes files under the root that this build did not write: output of sources since removed.
    /// Files are replaced in place rather than the tree being cleared first, so a reader (trunk)
    /// never finds it missing.
    pub fn prune(&self) -> Res<Vec<String>> {
        let mut stale = Vec::new();
        self.prune_dir(&self.root, &self.written(), &mut stale)?;
        Ok(stale)
    }

    fn prune_dir(&self, dir: &Path, written: &BTreeMap<String, u64>, stale: &mut Vec<String>) -> Void {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in entries {
            let path = entry.with_context(|| format!("listing {}", dir.display()))?.path();
            if path.is_dir() {
                self.prune_dir(&path, written, stale)?;
                // A directory pruning emptied held only stale output; trunk would still copy it.
                if std::fs::read_dir(&path).with_context(|| format!("listing {}", path.display()))?.next().is_none() {
                    std::fs::remove_dir(&path).with_context(|| format!("removing {}", path.display()))?;
                }
                continue;
            }
            let relative = path.strip_prefix(&self.root).context("a path outside the output")?.to_string_lossy().replace('\\', "/");
            if !written.contains_key(&relative) {
                std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
                stale.push(relative);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn prunes_only_what_this_build_did_not_write() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("maps/old")).unwrap();
        std::fs::write(dir.path().join("maps/old/terrain.png"), b"stale").unwrap();

        let out = Out::new(dir.path());
        out.write("maps/new/map.ron", b"fresh").unwrap();
        assert_eq!(out.prune().unwrap(), ["maps/old/terrain.png"]);
        assert!(
            dir.path().join("maps/new/map.ron").exists() && !dir.path().join("maps/old").exists(),
            "stale files and the directories they leave empty go"
        );
        assert_eq!(out.written().get("maps/new/map.ron"), Some(&5));
    }
}
