//! Repo tooling, run as `cargo xtask <task>` (the alias lives in `.cargo/config.toml`). Tasks are
//! Rust rather than shell or Python, so they get the same lints and tests as the product.

mod art;
mod dist;

use anyhow::{Result, bail};

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  art           render the art sources in assets/ into assets/gen/ (review sheets in target/art-preview/)
  dist          art, then the release client bundle into dist/, precompressed, with sizes
  dist --dev    art, then a quick wasm-dev bundle into target/dev-dist/";

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("art") => art::run(),
        Some("dist") => dist::run(if std::env::args().any(|arg| arg == "--dev") { dist::Mode::Dev } else { dist::Mode::Release }),
        None | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => bail!("unknown task `{other}`; run `cargo xtask help`"),
    }
}
