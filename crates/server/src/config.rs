//! Runtime configuration. Every flag also reads a `LOBSHOT_*` environment variable, so the same
//! binary configures from a shell, a container or a unit file without a config file.

use std::{net::SocketAddr, path::PathBuf};

use axum::http::HeaderName;
use clap::Parser;

/// Turn-based artillery in the browser, in the spirit of GunBound.
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Address to listen on.
    #[arg(long, env = "LOBSHOT_BIND", default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,

    /// The client bundle to serve: the directory `cargo make dist` writes.
    #[arg(long, env = "LOBSHOT_DIST", default_value = "dist")]
    pub dist: PathBuf,

    /// How long a bot thinks before it plays, in milliseconds, so its turn reads as a turn.
    #[arg(long, env = "LOBSHOT_BOT_DELAY_MS", default_value_t = 1200)]
    pub bot_delay_ms: u64,

    /// Start each turn's clock at once, instead of after clients have shown the last shot. For
    /// tests with no one watching.
    #[arg(long, env = "LOBSHOT_NO_PLAYBACK_WAIT")]
    pub no_playback_wait: bool,

    /// The request header a fronting proxy puts the client's IP in (`Fly-Client-IP` on Fly).
    /// Without it, rate limits key on the socket's peer address.
    #[arg(long, env = "LOBSHOT_CLIENT_IP_HEADER")]
    pub client_ip_header: Option<HeaderName>,
}
