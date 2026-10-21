//! Single-source guards. The toolchain is pinned in exactly one place, `rust-toolchain.toml`; these
//! tests fail when a workflow or Dockerfile grows its own pin. A "keep in sync" comment is not
//! enough: conclave's and razel's rotted (ironpad's fix, generalized).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A missing file reads as empty: not every variant has a Dockerfile or release workflow.
fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_default()
}

const PINNABLE: [&str; 3] = [".github/workflows/build.yml", ".github/workflows/release.yml", "Dockerfile"];

#[test]
fn toolchain_is_pinned_only_in_rust_toolchain_toml() {
    let toml = read("rust-toolchain.toml");
    let channel = toml
        .lines()
        .find_map(|line| line.trim().strip_prefix("channel = "))
        .expect("rust-toolchain.toml sets a channel")
        .trim_matches('"');

    for file in PINNABLE {
        let text = read(file);
        assert!(!text.contains(channel), "{file} repeats the toolchain pin `{channel}`; read rust-toolchain.toml instead");
        assert!(!text.contains("nightly-20"), "{file} pins a dated nightly; only rust-toolchain.toml may");
    }
}

#[test]
fn dockerfile_cooks_dependencies_on_the_pinned_toolchain() {
    let dockerfile = read("Dockerfile");
    if dockerfile.is_empty() {
        return;
    }
    let copy = dockerfile.find("COPY rust-toolchain.toml").expect("Dockerfile copies rust-toolchain.toml");
    let cook = dockerfile.find("cargo chef cook").expect("Dockerfile cooks dependencies with cargo-chef");
    assert!(copy < cook, "copy rust-toolchain.toml before `cargo chef cook`, or the dependency layer builds on the wrong toolchain");
}

/// The `doc-valid-idents` line of a clippy.toml.
fn doc_idents(rel: &str) -> String {
    read(rel).lines().find(|line| line.starts_with("doc-valid-idents")).unwrap_or_default().to_owned()
}

#[test]
fn sim_clippy_toml_repeats_the_root_doc_idents() {
    // Clippy reads only the nearest clippy.toml, so sim's (which adds the determinism bans) must
    // repeat the root's list.
    let root = doc_idents("clippy.toml");
    assert!(!root.is_empty(), "clippy.toml sets doc-valid-idents");
    assert_eq!(doc_idents("crates/sim/clippy.toml"), root, "crates/sim/clippy.toml drifted from clippy.toml");
}

/// The value of the first `key = value` line in `text`, unquoted.
fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| line.trim().strip_prefix(key)?.trim_start().strip_prefix('=')).map(|value| value.trim().trim_matches('"'))
}

#[test]
fn fly_toml_points_at_the_containers_port_and_health_route() {
    // The image's port lives in the Dockerfile; fly.toml must route to it and check a route the
    // server answers, or every deploy fails its health check.
    let fly = read("fly.toml");
    let port = setting(&fly, "internal_port").expect("fly.toml sets internal_port");
    let dockerfile = read("Dockerfile");
    assert!(dockerfile.contains(&format!("LOBSHOT_BIND=0.0.0.0:{port}")), "the image listens on {port}");
    assert!(dockerfile.contains(&format!("EXPOSE {port}")), "the image exposes {port}");
    let health = setting(&fly, "path").expect("fly.toml checks a path");
    assert!(read("crates/server/src/server.rs").contains(&format!("\"{health}\"")), "the server routes {health}");
    assert_eq!(setting(&fly, "LOBSHOT_CLIENT_IP_HEADER"), Some("Fly-Client-IP"), "rate limits key on Fly's client-IP header");
}
