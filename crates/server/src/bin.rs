//! The `lobshot` binary: set up tracing, read the config, run the server. Everything worth testing
//! lives in `lobshot_server`; this file stays thin.

// `cargo llvm-cov` on nightly sets `coverage_nightly`; elsewhere this is inert, so the source builds
// on stable. Enable the feature only in a crate root that uses `#[coverage(off)]`: an unused
// feature fails the `unused` lint group on current nightlies.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::io::IsTerminal as _;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use lobshot_server::{config::Config, server};
use lobshot_sim::base::Void;

#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> Void {
    // `RUST_LOG` overrides the default level; color only when a human is watching.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();

    server::run(&Config::parse()).await
}
