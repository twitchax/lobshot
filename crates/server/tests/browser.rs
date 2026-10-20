//! The browser smoke test (PRD-0003 uat-003): the real client, built by trunk, in headless Chrome
//! against the real binary. The page loads, the canvas renders, the socket connects, and the
//! client follows a game to its second turn.
//!
//! The browser is a spectator, so the test needn't click Bevy's canvas UI: a protocol client hosts
//! (on the `flat` map, against a server bot) and plays its turns with SearchBot, and the browser,
//! joining first while both seats are taken, watches. The client publishes what it's doing as
//! `window.lobshot` (`crates/client/src/web.rs`), which the test reads.
//!
//! Ignored by default: it needs Chrome and a client bundle. `cargo make browser` builds the
//! release bundle and runs it; `LOBSHOT_BROWSER_DIST` points it at another bundle (the quicker
//! `wasm-dev` one, say).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::{
    ffi::OsStr,
    io::{BufRead as _, BufReader},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

use headless_chrome::{Browser, LaunchOptions};
use serde_json::Value;
use tempfile::TempDir;

use lobshot_protocol::{ClientMsg, RoomOptions, ServerMsg, Stage};
use lobshot_server::tests::create_room;
use lobshot_sim::game::Team;

use support::{Client, play_out};

/// How long the page may take to load and the game to get going: the wasm is several MB, and
/// software rendering in a headless browser is slow.
const PATIENCE: Duration = Duration::from_secs(180);

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

fn start(dist: &Path, cwd: &Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lobshot"))
        .current_dir(cwd)
        .args(["--bind", "127.0.0.1:0"])
        .env("LOBSHOT_DIST", dist)
        .env("LOBSHOT_BOT_DELAY_MS", "300")
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

/// The bundle to serve: `LOBSHOT_BROWSER_DIST`, or the repo's `dist/` from `cargo make dist`.
fn bundle() -> PathBuf {
    let dist = std::env::var_os("LOBSHOT_BROWSER_DIST").map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist"), PathBuf::from);
    assert!(
        dist.join("index.html").is_file(),
        "no client bundle at {}; run `cargo make dist` (or `cargo make browser`)",
        dist.display()
    );
    dist
}

/// What the page says it's doing, from `window.lobshot`.
fn page_state(tab: &headless_chrome::Tab) -> Option<Value> {
    let answer = tab.evaluate("JSON.stringify(window.lobshot || null)", false).ok()?;
    serde_json::from_str(answer.value?.as_str()?).ok()
}

/// Polls the page until `done` accepts its state, or fails after `PATIENCE`.
fn wait_for(tab: &headless_chrome::Tab, what: &str, mut done: impl FnMut(&Value) -> bool) -> Value {
    let started = Instant::now();
    loop {
        if let Some(state) = page_state(tab).filter(|state| done(state)) {
            return state;
        }
        assert!(started.elapsed() < PATIENCE, "the page never reached {what}; last state {:?}", page_state(tab));
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Chrome and a client bundle; run `cargo make browser`"]
async fn e2e_browser_spectator_follows_a_game() {
    let dist = bundle();
    let cwd = TempDir::new().unwrap();
    let server = start(&dist, cwd.path());
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    // The host takes team A, a server bot team B, on the synthetic map.
    let mut host = Client::connect(addr, &code, false).await;
    let (host_id, _, _) = host.hello("Host", None).await;
    host.send(&ClientMsg::Configure(RoomOptions {
        map: "flat".into(),
        team_size: 1,
        max_wind: 12,
        turn_seconds: 20,
        random_teams: false,
    }))
    .await;
    host.send(&ClientMsg::AddBot { team: Team::B }).await;

    // The browser joins next, as a spectator: both seats are taken.
    let browser = Browser::new(
        LaunchOptions::default_builder()
            .headless(true)
            // No `--disable-gpu`: Bevy needs WebGL2, which SwiftShader provides without a GPU.
            .enable_gpu(true)
            .args(vec![OsStr::new("--use-angle=swiftshader"), OsStr::new("--enable-unsafe-swiftshader")])
            .window_size(Some((1024, 768)))
            .build()
            .unwrap(),
    )
    .expect("Chrome launches");
    let tab = browser.new_tab().unwrap();
    let origin = format!("http://{addr}");
    let room_url = format!("{origin}/r/{code}");
    let tab = tokio::task::spawn_blocking(move || {
        tab.navigate_to(&origin).unwrap().wait_until_navigated().unwrap();
        tab.evaluate("localStorage.setItem('lobshot.name', 'Watcher')", false).unwrap();
        tab.navigate_to(&room_url).unwrap().wait_until_navigated().unwrap();
        wait_for(&tab, "the lobby", |state| state["screen"] == "Lobby");
        tab
    })
    .await
    .unwrap();

    // Once the browser is in the room, start, and play the host's turns until the game ends.
    host.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.members.len() == 3)).await;
    host.send(&ClientMsg::Ready(true)).await;
    host.send(&ClientMsg::Start).await;
    host.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if matches!(view.stage, Stage::Playing { .. }))).await;
    let game = tokio::spawn(async move { play_out(&mut host, host_id, None, 1).await });

    // The page follows: it reaches the game screen and sees the second turn.
    let state = tokio::task::spawn_blocking(move || wait_for(&tab, "the second turn", |state| state["screen"] == "Game" && state["turns"].as_i64().is_some_and(|turns| turns >= 2)))
        .await
        .unwrap();
    assert!(state["seat"].is_null(), "the browser watches without a seat: {state}");
    game.abort();
    drop(browser);
    drop(server);
}
