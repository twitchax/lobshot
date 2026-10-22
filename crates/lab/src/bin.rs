//! The `lobshot-lab` binary: SearchBot balance runs, and replaying any game they played.

// `cargo llvm-cov` on nightly sets `coverage_nightly`; elsewhere this is inert, so the source builds
// on stable. Enable the feature only in a crate root that uses `#[coverage(off)]`: an unused
// feature fails the `unused` lint group on current nightlies.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    fmt,
    io::{IsTerminal as _, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use anyhow::{Context as _, ensure};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use lobshot_assets::{MAPS, maps::Maps};
use lobshot_lab::{
    balance::{self, LevelName, Spec},
    report::{BAND, Band, Report},
};
use lobshot_sim::{base::Void, game::MobileKind};

/// Headless lobshot: bot-vs-bot balance runs and the interface for LLM agents.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Play every pair of mobiles against each other, 1v1 between SearchBot seats, and report the
    /// win-rate matrix, damage per unit of delay and turns to kill.
    Balance(BalanceArgs),
    /// Replay one game of a balance run by its id (as `--logs` names it), printing its log as JSON.
    Game {
        /// mobile.mobile.map.variant.level.seed, e.g. `coilback.hookclaw.tidewall.A.normal.17`.
        id: Spec,
        #[command(flatten)]
        assets: AssetsArg,
    },
}

#[derive(Debug, clap::Args)]
struct AssetsArg {
    /// The generated assets (`cargo make art` writes them).
    #[arg(long, default_value = "assets/gen")]
    assets: PathBuf,
}

#[derive(Debug, clap::Args)]
struct BalanceArgs {
    /// Games per matchup (a pair of mobiles, or a mirror).
    #[arg(long, default_value_t = 200)]
    games: u32,
    /// How well the bots play: easy, normal or hard.
    #[arg(long, default_value = "normal")]
    level: LevelName,
    /// Maps to play, every variant of each (`flat` is built in and needs no assets).
    #[arg(long, value_delimiter = ',', default_values_t = MAPS.iter().map(|&m| m.to_owned()))]
    maps: Vec<String>,
    /// Mobiles to pair up; all of them by default.
    #[arg(long, value_delimiter = ',', value_parser = balance::mobile)]
    mobiles: Vec<MobileKind>,
    /// Where every game's rolls start from; the same seed plays the same run.
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Threads to play on; every core by default.
    #[arg(long)]
    threads: Option<usize>,
    /// Print the report as JSON instead of text.
    #[arg(long)]
    json: bool,
    /// Fail when a matchup's win rate is outside `--band`.
    #[arg(long)]
    gate: bool,
    /// The gate's band, in percent.
    #[arg(long, default_value_t = BandArg(BAND))]
    band: BandArg,
    /// Write every game's log here as `<id>.json`.
    #[arg(long)]
    logs: Option<PathBuf>,
    #[command(flatten)]
    assets: AssetsArg,
}

/// `LOW..HIGH`, in percent.
#[derive(Debug, Clone, Copy)]
struct BandArg(Band);

impl fmt::Display for BandArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.0.low * 100.0, self.0.high * 100.0)
    }
}

impl FromStr for BandArg {
    type Err = anyhow::Error;

    fn from_str(band: &str) -> anyhow::Result<Self> {
        let (low, high) = band.split_once("..").with_context(|| format!("{band:?} isn't LOW..HIGH"))?;
        let (low, high): (f64, f64) = (low.parse().context("the band's low end")?, high.parse().context("the band's high end")?);
        ensure!((0.0..=high).contains(&low) && high <= 100.0, "{band:?} isn't a band within 0..100");
        Ok(Self(Band { low: low / 100.0, high: high / 100.0 }))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> ExitCode {
    // Logs go to stderr so stdout stays clean for reports; `RUST_LOG` overrides the default level.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();

    let result = match Cli::parse().command {
        Command::Balance(args) => balance(&args),
        Command::Game { id, assets } => game(&id, &assets.assets),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn balance(args: &BalanceArgs) -> Void {
    let maps = Maps::new(&args.assets.assets);
    let arenas = balance::arenas(&maps, &args.maps).with_context(|| format!("loading maps from {} (`cargo make art` builds them)", args.assets.assets.display()))?;
    let mobiles = if args.mobiles.is_empty() { MobileKind::ALL.to_vec() } else { args.mobiles.clone() };
    let specs = balance::specs(&mobiles, &arenas, args.games, args.level.0, args.seed);
    let threads = args.threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
    let total = specs.len();
    let tty = std::io::stderr().is_terminal();
    let progress = move |done: usize| {
        if tty && (done % 50 == 0 || done == total) {
            eprint!("\r{done}/{total} games");
            let _ = std::io::stderr().flush();
        }
    };
    let played = balance::run(&specs, &maps, threads, &progress)?;
    if tty {
        eprintln!();
    }
    if let Some(dir) = &args.logs {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        for (outcome, log) in &played {
            let path = dir.join(format!("{}.json", outcome.spec));
            std::fs::write(&path, log.to_json()?).with_context(|| format!("writing {}", path.display()))?;
        }
    }
    let outcomes: Vec<_> = played.into_iter().map(|(outcome, _)| outcome).collect();
    let report = Report::new(&outcomes, &mobiles, args.level.0);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).context("writing the report")?);
    } else {
        print!("{}", report.to_text(args.band.0));
    }
    let outside = report.outside(args.band.0).len();
    ensure!(!args.gate || outside == 0, "the balance gate failed: {outside} matchups outside the band");
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn game(id: &Spec, assets: &Path) -> Void {
    let (_, log) = balance::play(id, &Maps::new(assets))?;
    println!("{}", log.to_json()?);
    Ok(())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn bands_parse_in_percent() {
        let BandArg(band) = "35..65".parse().unwrap();
        assert_eq!((band.low, band.high), (0.35, 0.65));
        assert_eq!(BandArg(BAND).to_string(), "35..65", "DESIGN.md's band");
        for bad in ["35", "65..35", "a..b", "0..101", "-1..50"] {
            assert!(bad.parse::<BandArg>().is_err(), "{bad:?}");
        }
    }
}
