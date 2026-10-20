//! `cargo xtask dist`: build the client bundle the server serves.
//!
//! First `cargo xtask art`, since the bundle copies `assets/gen/`. Then trunk builds
//! `crates/client` to wasm (with wasm-opt) into `dist/`, and every compressible file, at any depth,
//! gets `.br` and `.gz` siblings, which the server picks by `Accept-Encoding`: brotli for HTTPS,
//! gzip for plain-HTTP self-hosts (browsers only offer br on secure origins). Compressing once at
//! build time keeps the shared-cpu Fly machine from compressing a multi-MB wasm per request.
//!
//! `--dev` builds the quick `wasm-dev` bundle into `target/dev-dist/` instead: no wasm-opt, no
//! compression, rebuilt in about 30 s.

use std::{
    fs::File,
    io::{BufReader, BufWriter, Write as _},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, bail};
use flate2::{Compression, write::GzEncoder};

/// Extensions worth compressing: code, pages, and the art manifests and masks (a mask shrinks about
/// 40x). Images and audio are already compressed.
const COMPRESSIBLE: [&str; 8] = ["wasm", "js", "html", "css", "svg", "json", "ron", "bin"];

/// Which bundle to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `dist/`: optimized, wasm-opt'd and precompressed. What the image ships.
    Release,
    /// `target/dev-dist/`: quick to build, for `cargo make dev`.
    Dev,
}

/// One precompressed file, for the size report.
#[derive(Debug)]
pub struct Compressed {
    pub path: PathBuf,
    pub raw: u64,
    pub brotli: u64,
    pub gzip: u64,
}

pub fn run(mode: Mode) -> Result<()> {
    crate::art::run()?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let (dist, args): (PathBuf, &[&str]) = match mode {
        Mode::Release => (root.join("dist"), &["build", "--release", "--cargo-profile", "wasm-release", "--dist"]),
        Mode::Dev => (root.join("target/dev-dist"), &["build", "--cargo-profile", "wasm-dev", "--dist"]),
    };

    let status = Command::new("trunk")
        .current_dir(root.join("crates/client"))
        .args(args)
        .arg(&dist)
        .arg("index.html")
        .status()
        .context("running trunk; install it with `cargo make install-trunk`")?;
    if !status.success() {
        bail!("trunk build failed ({status})");
    }

    if mode == Mode::Release {
        let report = precompress(&dist)?;
        print_report(&dist, &report);
        println!("{:>12}  over the wire: everything, brotli where precompressed", mib(wire_size(&dist)?));
    } else {
        println!("the dev bundle is in {}", dist.strip_prefix(&root).unwrap_or(&dist).display());
    }
    Ok(())
}

/// Writes `.br` and `.gz` siblings for every compressible file in `dir`, at any depth.
pub fn precompress(dir: &Path) -> Result<Vec<Compressed>> {
    let mut report = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let compressible = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| COMPRESSIBLE.contains(&ext));
            if !compressible {
                continue;
            }
            let raw = std::fs::metadata(&path)?.len();
            let brotli = write_brotli(&path).with_context(|| format!("brotli-compressing {}", path.display()))?;
            let gzip = write_gzip(&path).with_context(|| format!("gzip-compressing {}", path.display()))?;
            report.push(Compressed { path, raw, brotli, gzip });
        }
    }
    report.sort_by_key(|file| std::cmp::Reverse(file.raw));
    Ok(report)
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn write_brotli(path: &Path) -> Result<u64> {
    let out = sibling(path, ".br");
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        // The largest window browsers accept (16 MiB): it finds repeats across a whole wasm.
        lgwin: 24,
        ..Default::default()
    };
    let mut writer = BufWriter::new(File::create(&out)?);
    brotli::BrotliCompress(&mut BufReader::new(File::open(path)?), &mut writer, &params)?;
    writer.flush()?;
    Ok(std::fs::metadata(&out)?.len())
}

fn write_gzip(path: &Path) -> Result<u64> {
    let out = sibling(path, ".gz");
    let mut encoder = GzEncoder::new(BufWriter::new(File::create(&out)?), Compression::best());
    std::io::copy(&mut BufReader::new(File::open(path)?), &mut encoder)?;
    encoder.finish()?.flush()?;
    Ok(std::fs::metadata(&out)?.len())
}

/// The whole bundle as a browser with brotli fetches it: each file's `.br` sibling where there is
/// one, the file itself where not (images). The first-load goal (`DESIGN.md`, "Performance
/// goals") is measured against it.
pub fn wire_size(dir: &Path) -> Result<u64> {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "br" || ext == "gz") {
                continue;
            }
            let brotli = sibling(&path, ".br");
            total += std::fs::metadata(if brotli.is_file() { &brotli } else { &path })?.len();
        }
    }
    Ok(total)
}

/// Prints per-file and total sizes of what was compressed.
fn print_report(dist: &Path, report: &[Compressed]) {
    println!("{:>12} {:>12} {:>12}  file", "raw", "brotli", "gzip");
    for file in report {
        let name = file.path.strip_prefix(dist).unwrap_or(&file.path).display();
        println!("{:>12} {:>12} {:>12}  {name}", mib(file.raw), mib(file.brotli), mib(file.gzip));
    }
    let total = |size: fn(&Compressed) -> u64| report.iter().map(size).sum::<u64>();
    println!("{:>12} {:>12} {:>12}  total", mib(total(|f| f.raw)), mib(total(|f| f.brotli)), mib(total(|f| f.gzip)));
}

fn mib(bytes: u64) -> String {
    // Sizes stay far below 2^52 bytes, where an f64 stops being exact.
    #[allow(clippy::cast_precision_loss)]
    let mib = bytes as f64 / (1024.0 * 1024.0);
    format!("{mib:.2} MiB")
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::io::Read as _;

    use flate2::read::GzDecoder;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn precompresses_bundle_files_and_skips_the_rest() {
        let dir = TempDir::new().unwrap();
        let wasm = "(module) ".repeat(4096);
        std::fs::write(dir.path().join("app_bg.wasm"), &wasm).unwrap();
        std::fs::write(dir.path().join("index.html"), "<canvas></canvas>").unwrap();
        std::fs::write(dir.path().join("sky.png"), b"already compressed").unwrap();

        let report = precompress(dir.path()).unwrap();

        let names: Vec<_> = report.iter().map(|f| f.path.file_name().unwrap().to_owned()).collect();
        assert_eq!(names, ["app_bg.wasm", "index.html"]);
        assert!(!dir.path().join("sky.png.br").exists());
        assert!(report[0].brotli < report[0].raw && report[0].gzip < report[0].raw);

        let mut unbrotli = String::new();
        brotli::Decompressor::new(File::open(dir.path().join("app_bg.wasm.br")).unwrap(), 4096)
            .read_to_string(&mut unbrotli)
            .unwrap();
        assert_eq!(unbrotli, wasm);

        let mut ungzip = String::new();
        GzDecoder::new(File::open(dir.path().join("app_bg.wasm.gz")).unwrap()).read_to_string(&mut ungzip).unwrap();
        assert_eq!(ungzip, wasm);
    }

    #[test]
    fn reaches_into_folders_and_compresses_masks_and_manifests() {
        let dir = TempDir::new().unwrap();
        let map = dir.path().join("assets/maps/pond/A");
        std::fs::create_dir_all(&map).unwrap();
        std::fs::write(map.join("mask.bin"), vec![0_u8; 4096]).unwrap();
        std::fs::write(map.join("terrain.png"), b"already compressed").unwrap();
        std::fs::write(dir.path().join("assets/maps/pond/map.ron"), "MapInfo(name: \"pond\")").unwrap();

        let report = precompress(dir.path()).unwrap();

        assert_eq!(report.len(), 2, "{report:?}");
        assert!(map.join("mask.bin.br").exists() && map.join("mask.bin.gz").exists());
        assert!(dir.path().join("assets/maps/pond/map.ron.br").exists());
        assert!(!map.join("terrain.png.br").exists());
    }

    #[test]
    fn the_wire_size_counts_brotli_where_it_exists_and_the_file_where_not() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("app_bg.wasm"), "(module) ".repeat(4096)).unwrap();
        std::fs::write(dir.path().join("assets/sky.png"), [7_u8; 1000]).unwrap();
        let report = precompress(dir.path()).unwrap();
        assert_eq!(wire_size(dir.path()).unwrap(), report[0].brotli + 1000);
    }

    #[test]
    fn rerunning_does_not_compress_the_siblings() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("index.html"), "<canvas></canvas>").unwrap();
        precompress(dir.path()).unwrap();
        let report = precompress(dir.path()).unwrap();
        assert_eq!(report.len(), 1);
        assert!(!dir.path().join("index.html.br.br").exists());
    }
}
