//! The `lobshot-lab` binary: headless games in `lobshot_sim`, for balance runs (M4) and LLM agents.
//! It has no commands until the sim can play a game; M0 only wires the crate in.

// `cargo llvm-cov` on nightly sets `coverage_nightly`; elsewhere this is inert, so the source builds
// on stable. Enable the feature only in a crate root that uses `#[coverage(off)]`: an unused
// feature fails the `unused` lint group on current nightlies.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::io::IsTerminal as _;

use clap::Parser;
use tracing_subscriber::EnvFilter;

/// Headless lobshot: bot-vs-bot balance runs and the interface for LLM agents.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() {
    // Logs go to stderr so stdout stays clean for reports; `RUST_LOG` overrides the default level.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();

    let Cli {} = Cli::parse();
    tracing::info!("nothing to run yet");
}
