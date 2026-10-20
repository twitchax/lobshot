//! End to end: spawn the real binary as a user would, inside a scratch directory. Tests here are
//! named `e2e_*`, which nextest serializes and retries (see `.config/nextest.toml`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Command, Output};

use tempfile::TempDir;

fn run(args: &[&str]) -> Output {
    let dir = TempDir::new().unwrap();
    Command::new(env!("CARGO_BIN_EXE_lobshot-lab")).current_dir(dir.path()).args(args).output().unwrap()
}

#[test]
fn e2e_reports_its_version() {
    let out = run(&["--version"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("lobshot-lab "));
}

#[test]
fn e2e_rejects_unknown_arguments() {
    let out = run(&["--no-such-flag"]);
    assert!(!out.status.success());
}
