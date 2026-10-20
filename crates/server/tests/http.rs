//! The real router on a real socket, in-process: binds `127.0.0.1:0` through the fixture factory.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tempfile::TempDir;

use lobshot_server::tests::{INDEX_HTML, http_get, spawn_test_server, write_bundle};

#[tokio::test]
async fn serves_the_bundle_on_an_ephemeral_port() {
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let addr = spawn_test_server(dist.path()).await.unwrap();

    let response = tokio::task::spawn_blocking(move || http_get(addr, "/r/socket")).await.unwrap().unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with(INDEX_HTML), "{response}");
}
