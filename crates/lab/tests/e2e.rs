//! End to end: spawn the real binary as a user would, inside a scratch directory. Tests here are
//! named `e2e_*`, which nextest serializes and retries (see `.config/nextest.toml`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

fn run_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lobshot-lab")).current_dir(dir).args(args).output().unwrap()
}

fn run(args: &[&str]) -> Output {
    run_in(TempDir::new().unwrap().path(), args)
}

fn stdout(out: &Output) -> String {
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout.clone()).unwrap()
}

/// A quick run on the built-in map, which needs no assets.
const QUICK: &[&str] = &["balance", "--games", "2", "--maps", "flat", "--mobiles", "coilback,hookclaw", "--level", "hard"];

#[test]
fn e2e_reports_its_version() {
    assert!(stdout(&run(&["--version"])).starts_with("lobshot-lab "));
}

#[test]
fn e2e_rejects_unknown_arguments() {
    let out = run(&["--no-such-flag"]);
    assert!(!out.status.success());
}

#[test]
fn e2e_balance_prints_the_matrix_for_every_pair() {
    let text = stdout(&run(QUICK));
    // Two games for each mirror and two for the pair.
    assert!(text.starts_with("Balance: 6 games, 1v1, Hard bots, on 1 arena (flat A)."), "{text}");
    for row in ["Coilback", "Hookclaw"] {
        assert!(text.lines().any(|line| line.starts_with(row) && line.contains('%')), "{text}");
    }
    assert!(text.contains("By arena:\n"), "{text}");
}

#[test]
fn e2e_balance_reports_json_for_tools() {
    let mut args = QUICK.to_vec();
    args.push("--json");
    let report: serde_json::Value = serde_json::from_str(&stdout(&run(&args))).unwrap();
    assert_eq!(report["games"], 6);
    assert_eq!(report["matchups"].as_array().unwrap().len(), 1);
    assert_eq!(report["mobiles"][1]["mobile"], "Hookclaw");
}

#[test]
fn e2e_the_balance_gate_fails_a_matchup_outside_its_band() {
    let mut args = QUICK.to_vec();
    args.extend(["--gate", "--band", "0..1"]);
    let out = run(&args);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("the balance gate failed: 1 matchups outside the band"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Outside 0-1%:"), "the report still prints");
}

#[test]
fn e2e_any_logged_game_replays_by_its_id() {
    let dir = TempDir::new().unwrap();
    let mut args = QUICK.to_vec();
    args.extend(["--logs", "logs"]);
    stdout(&run_in(dir.path(), &args));
    let mut logs: Vec<_> = std::fs::read_dir(dir.path().join("logs")).unwrap().map(|entry| entry.unwrap().path()).collect();
    logs.sort();
    assert_eq!(logs.len(), 6);
    let log = &logs[0];
    let id = log.file_stem().unwrap().to_str().unwrap();
    let replayed = stdout(&run_in(dir.path(), &["game", id]));
    assert_eq!(replayed.trim_end(), std::fs::read_to_string(log).unwrap());
}

#[test]
fn e2e_balance_without_built_maps_says_how_to_build_them() {
    let out = run(&["balance", "--games", "2"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cargo make art"), "{}", String::from_utf8_lossy(&out.stderr));
}
