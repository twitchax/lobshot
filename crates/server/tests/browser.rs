//! The browser smoke tests (PRD-0003 uat-003): the real client, built by trunk, in headless Chrome
//! against the real binary. The client publishes what it's doing, and where its buttons are, as
//! `window.lobshot` (`crates/client/src/web.rs`), which the tests read.
//!
//! - Online, as a spectator: a protocol client hosts (on the `flat` map, against a server bot) and
//!   plays its turns with SearchBot, and the browser, joining first while both seats are taken,
//!   watches. The page loads, the canvas renders, the socket connects, every sound loads, and the
//!   client follows the game to its second turn, sounding the shots it shows.
//! - Practice: the browser clicks "Practice vs bot" and lets its turn run out, which chimes and
//!   ticks.
//! - Practice, with items: on its turn the browser arms Power Up with its key, then fires SS, and
//!   the page shows the item used and SS locked.
//!
//! In both, each turn's clock starts only once the shot before it has shown.
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

use headless_chrome::{Browser, LaunchOptions, browser::tab::point::Point};
use serde_json::Value;
use tempfile::TempDir;

use lobshot_assets::Sound;
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

/// Headless Chrome, rendering WebGL2 in software.
fn launch() -> Browser {
    Browser::new(
        LaunchOptions::default_builder()
            .headless(true)
            // No `--disable-gpu`: Bevy needs WebGL2, which SwiftShader provides without a GPU.
            .enable_gpu(true)
            // Audio runs before any click, so every sound the page plays is decoded and mixed.
            .args(vec![
                OsStr::new("--use-angle=swiftshader"),
                OsStr::new("--enable-unsafe-swiftshader"),
                OsStr::new("--autoplay-policy=no-user-gesture-required"),
            ])
            .window_size(Some((1024, 768)))
            .build()
            .unwrap(),
    )
    .expect("Chrome launches")
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

/// Where the page's button for `action` (`Practice`) is, from `window.lobshot.buttons`.
fn spot(state: &Value, action: &str) -> Option<Point> {
    let button = state["buttons"].as_array()?.iter().find(|button| button["action"] == action)?;
    Some(Point {
        x: button["x"].as_f64()?,
        y: button["y"].as_f64()?,
    })
}

/// Clicks the button for `action` until `done` accepts the page's state. Bevy presses only a
/// hovered button, and hover follows the pointer a frame late, so a first click can miss.
fn click_until(tab: &headless_chrome::Tab, action: &str, what: &str, mut done: impl FnMut(&Value) -> bool) -> Value {
    let state = wait_for(tab, &format!("a {action} button"), |state| spot(state, action).is_some());
    let point = spot(&state, action).unwrap();
    tab.move_mouse_to_point(point).unwrap();
    let started = Instant::now();
    loop {
        tab.click_point(point).unwrap();
        for _ in 0..10 {
            if let Some(state) = page_state(tab).filter(|state| done(state)) {
                return state;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(started.elapsed() < PATIENCE, "clicking {action} never reached {what}; last state {:?}", page_state(tab));
    }
}

/// Whether the page's buttons offer `action`, enabled.
fn enabled(state: &Value, action: &str) -> Option<bool> {
    state["buttons"].as_array()?.iter().find(|button| button["action"] == action)?["enabled"].as_bool()
}

/// The turn clock starts each turn once, when the shot before it has shown, so it never counts
/// more turns started than the game has played, plus the first.
fn clock_on_time(state: &Value) {
    if let (Some(turns), Some(started)) = (state["turns"].as_i64(), state["started"].as_i64()) {
        assert!(started <= turns + 1, "the clock started a turn before its shot had shown: {state}");
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
        sudden_death: None,
    }))
    .await;
    host.send(&ClientMsg::AddBot { team: Team::B }).await;

    // The browser joins next, as a spectator: both seats are taken.
    let browser = launch();
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

    // The page follows: it reaches the game screen, sees the second turn, and sounds a shot fired
    // and landing, its clock on time all along.
    let state = tokio::task::spawn_blocking(move || {
        wait_for(&tab, "the second turn", |state| {
            clock_on_time(state);
            state["screen"] == "Game" && state["turns"].as_i64().is_some_and(|turns| turns >= 2)
        });
        wait_for(&tab, "a shot's sounds", |state| {
            clock_on_time(state);
            ["fire", "blast"].iter().all(|sound| state["sounds"]["played"][sound].as_u64() >= Some(1))
        })
    })
    .await
    .unwrap();
    assert!(state["seat"].is_null(), "the browser watches without a seat: {state}");
    assert_eq!(state["sounds"]["loaded"].as_u64(), Some(Sound::ALL.len() as u64), "every sound loads: {state}");
    game.abort();
    drop(browser);
    drop(server);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Chrome and a client bundle; run `cargo make browser`"]
async fn e2e_browser_practice_turns_start_after_the_shot_before() {
    let dist = bundle();
    let cwd = TempDir::new().unwrap();
    let server = start(&dist, cwd.path());
    let browser = launch();
    let tab = browser.new_tab().unwrap();
    let origin = format!("http://{}", server.addr);

    // Practice is refereed in the page, so its turns pass on the page's own schedule. The player
    // never acts, so their turn runs out: it chimes, then ticks through its last seconds.
    let state = tokio::task::spawn_blocking(move || {
        tab.navigate_to(&origin).unwrap().wait_until_navigated().unwrap();
        click_until(&tab, "Practice", "a practice game", |state| state["screen"] == "Game");
        wait_for(&tab, "the player's turn running out, and the next", |state| {
            clock_on_time(state);
            state["turns"].as_i64() >= Some(2) && state["sounds"]["played"]["tick"].as_u64() >= Some(5)
        })
    })
    .await
    .unwrap();
    assert_eq!(state["seat"], 0, "the player sits first: {state}");
    let played = &state["sounds"]["played"];
    assert!(played["click"].as_u64() >= Some(1) && played["turn"].as_u64() >= Some(1), "{state}");
    drop(browser);
    drop(server);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Chrome and a client bundle; run `cargo make browser`"]
async fn e2e_browser_practice_arms_an_item_and_locks_ss() {
    let dist = bundle();
    let cwd = TempDir::new().unwrap();
    let server = start(&dist, cwd.path());
    let browser = launch();
    let tab = browser.new_tab().unwrap();
    let origin = format!("http://{}", server.addr);

    // A fresh profile brings the starter loadout: Dual, Power Up, Heal, Wind Flip, Dig. Its items
    // are usable only on its own turn, which the enabled item buttons show.
    let state = tokio::task::spawn_blocking(move || {
        tab.navigate_to(&origin).unwrap().wait_until_navigated().unwrap();
        click_until(&tab, "Practice", "a practice game", |state| state["screen"] == "Game");
        let mine = wait_for(&tab, "the player's turn", |state| enabled(state, "Item(1)") == Some(true));
        assert_eq!(mine["me"]["items"][1], "PowerUp", "{mine}");
        assert_eq!(mine["me"]["ss"], "ready", "{mine}");

        // 5 uses the second item, Power Up: it's armed, and no other item can join it this turn.
        tab.press_key("5").unwrap();
        let armed = wait_for(&tab, "Power Up armed", |state| state["me"]["armed"] == "PowerUp");
        assert!(armed["me"]["items"][1].is_null(), "used up: {armed}");
        wait_for(&tab, "the other items disabled", |state| enabled(state, "Item(0)") == Some(false));

        // 3 picks SS and a tap of Space fires it, at once: SS locks for the next own turns.
        tab.press_key("3").unwrap();
        tab.press_key(" ").unwrap();
        wait_for(&tab, "SS locked", |state| {
            clock_on_time(state);
            state["me"]["ss_lock"].as_u64() > Some(0) && state["me"]["ss"] == "locked" && enabled(state, "Shot(Ss)") == Some(false)
        })
    })
    .await
    .unwrap();
    assert_eq!(state["seat"], 0, "{state}");
    drop(browser);
    drop(server);
}
