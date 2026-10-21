//! End to end: spawn the real binary on an ephemeral port and talk HTTP and WebSocket to it.
//! Tests are named `e2e_*`, which nextest serializes and retries (see `.config/nextest.toml`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::{
    io::{BufRead as _, BufReader},
    net::SocketAddr,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use lobshot_protocol::{ClientMsg, RoomOptions, ServerMsg, Stage};
use lobshot_server::tests::{INDEX_HTML, create_room, http_get, write_bundle};
use lobshot_sim::game::Phase;

use support::{Client, play_out};

/// The running binary. Dropping it kills the process, so a failed assertion never leaks a server.
struct Server {
    child: Child,
    addr: SocketAddr,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts the binary on `127.0.0.1:0`, serving `dist` from inside a scratch working directory, and
/// reads the bound address from its `listening` log line.
fn start(dist: &Path, cwd: &Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lobshot"))
        .current_dir(cwd)
        .args(["--bind", "127.0.0.1:0"])
        .env("LOBSHOT_DIST", dist)
        .env("RUST_LOG", "info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(addr) = line.split("addr=").nth(1).and_then(|rest| rest.split_whitespace().next()) {
                let _ = tx.send(addr.to_owned());
            }
        }
    });
    let addr = rx.recv_timeout(Duration::from_secs(10)).expect("the server logs its address").parse().unwrap();
    Server { child, addr }
}

#[test]
fn e2e_serves_health_the_page_and_room_links() {
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let server = start(dist.path(), cwd.path());

    assert!(http_get(server.addr, "/healthz").unwrap().ends_with("ok"));
    for path in ["/", "/r/e2e"] {
        let response = http_get(server.addr, path).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{path}: {response}");
        assert!(response.ends_with(INDEX_HTML), "{path}: {response}");
    }
}

#[test]
fn e2e_starts_without_a_bundle() {
    let cwd = TempDir::new().unwrap();
    let server = start(&cwd.path().join("no-such-dist"), cwd.path());
    assert!(http_get(server.addr, "/healthz").unwrap().ends_with("ok"));
    assert!(http_get(server.addr, "/").unwrap().starts_with("HTTP/1.1 404"));
}

/// M2's acceptance test (PRD-0003 uat-002): two clients play a 1v1 on `flat` to the end against
/// the real binary. Each replays the log it receives with `lobshot_sim`, checks its state hash
/// against the server's after every event, and plays its turns with SearchBot.
#[tokio::test(flavor = "multi_thread")]
async fn e2e_game_two_clients_play_a_1v1_to_the_end() {
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let server = start(dist.path(), cwd.path());
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    // One client speaks postcard and the other JSON, so both forms carry a whole game.
    let mut alice = Client::connect(addr, &code, false).await;
    let (alice_id, _, view) = alice.hello("Alice", None).await;
    assert_eq!(view.master, alice_id);
    let mut bob = Client::connect(addr, &code, true).await;
    let (bob_id, _, _) = bob.hello("Bob", None).await;

    alice
        .send(&ClientMsg::Configure(RoomOptions {
            map: "flat".into(),
            team_size: 1,
            max_wind: 26,
            turn_seconds: 20,
            random_teams: false,
            sudden_death: None,
        }))
        .await;
    alice.send(&ClientMsg::Ready(true)).await;
    bob.send(&ClientMsg::Ready(true)).await;
    alice
        .recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.options.map == "flat" && view.members.iter().all(|m| m.ready)))
        .await;
    alice.send(&ClientMsg::Start).await;

    let ((alice_game, alice_room), (bob_game, bob_room)) = tokio::join!(play_out(&mut alice, alice_id, None, 1), play_out(&mut bob, bob_id, None, 2));

    let (alice_game, bob_game) = (alice_game.game().unwrap(), bob_game.game().unwrap());
    assert!(matches!(alice_game.phase(), Phase::Over { winner: Some(_) }), "{:?}", alice_game.phase());
    assert_eq!(alice_game.phase(), bob_game.phase());
    assert_eq!(alice_game.state_hash().unwrap(), bob_game.state_hash().unwrap());
    assert!(alice_game.turns() > 2, "a real game, not a forfeit");
    for room in [alice_room, bob_room] {
        assert_eq!(room.stage, Stage::Lobby);
        assert!(room.members.iter().all(|m| !m.ready && m.connected), "{room:?}");
    }
}
