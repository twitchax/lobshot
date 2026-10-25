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
//! - Score, online: the browser plays a seat, walks into a pit and dies, clicks the map where it
//!   will drop back in, and comes back down there, alive, to play its turn.
//! - Tag, in practice: the title sets practice to Tag, and on its turn the browser presses T (F7's
//!   twin, which a browser may keep for itself): the
//!   mobile sitting out comes in, the other sits out, and the switch button goes grey till next
//!   turn.
//!
//! In each, each turn's clock starts only once the shot before it has shown.
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

use headless_chrome::{
    Browser, LaunchOptions,
    browser::tab::{Tab, point::Point},
    protocol::cdp::Input::{DispatchKeyEvent, DispatchKeyEventTypeOption},
};
use serde_json::{Value, json};
use tempfile::TempDir;

use lobshot_assets::{Sound, maps::Maps};
use lobshot_protocol::{ClientMsg, RoomOptions, ServerMsg, Stage};
use lobshot_server::tests::{create_room, write_map};
use lobshot_sim::{
    game::{MobileKind, Team},
    mode::Mode,
    terrain::Terrain,
};

use support::{Client, pass_out, play_out};

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
        mode: Mode::Solo,
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

/// PRD-0007 T-002: Tag in practice. A fresh profile picks Coilback, with Lanternwing second.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Chrome and a client bundle; run `cargo make browser`"]
async fn e2e_browser_tag_practice_switches_in_the_mobile_sitting_out() {
    let dist = bundle();
    let cwd = TempDir::new().unwrap();
    let server = start(&dist, cwd.path());
    let browser = launch();
    let tab = browser.new_tab().unwrap();
    let origin = format!("http://{}", server.addr);

    let state = tokio::task::spawn_blocking(move || {
        tab.navigate_to(&origin).unwrap().wait_until_navigated().unwrap();
        // The title steps practice through the modes: Solo, Score, Tag.
        click_until(&tab, "PracticeMode(Score)", "practice in Score", |state| spot(state, "PracticeMode(Tag)").is_some());
        click_until(&tab, "PracticeMode(Tag)", "practice in Tag", |state| spot(state, "PracticeMode(Solo)").is_some());
        let title = page_state(&tab).unwrap();
        assert_eq!((&title["picks"]["mobile"], &title["picks"]["second"]), (&json!("Coilback"), &json!("Lanternwing")), "{title}");
        click_until(&tab, "Practice", "a practice game", |state| state["screen"] == "Game");
        let mine = wait_for(&tab, "the player's turn, with a switch", |state| enabled(state, "Switch") == Some(true));
        assert_eq!((&mine["me"]["mobile"], &mine["me"]["sitting_out"]), (&json!("Coilback"), &json!("Lanternwing")), "{mine}");
        assert_eq!(mine["me"]["can_switch"], true, "{mine}");

        // T switches: Lanternwing comes in where the Coilback stood, and once a turn is all.
        tab.press_key("t").unwrap();
        wait_for(&tab, "the Lanternwing in play", |state| {
            clock_on_time(state);
            state["me"]["mobile"] == "Lanternwing" && state["me"]["sitting_out"] == "Coilback" && enabled(state, "Switch") == Some(false)
        })
    })
    .await
    .unwrap();
    assert_eq!(state["me"]["alive"], true, "{state}");
    assert_eq!(state["me"]["can_switch"], false, "{state}");
    drop(browser);
    drop(server);
}

/// Where the pit field's mobiles spawn: each with a shaft off the map `PIT_GAP` px to its right,
/// so a mobile that walks right falls to its death.
const PIT_SPAWNS: [i32; 4] = [200, 600, 1000, 1400];
const PIT_GAP: i32 = 10;
const PIT_WIDTH: i32 = 3;

/// How long a passing client (`support::pass_out`) waits on its turn before it passes: long enough
/// for the page to show a dead player's drop point coming down before it lands.
const PASS_AFTER: Duration = Duration::from_millis(1500);

/// The pit field: 1600 by 600, ground from y = 400, and a shaft beside each spawn.
fn pit_field() -> Terrain {
    let shaft = |x: i32| PIT_SPAWNS.iter().any(|&spawn| (spawn + PIT_GAP..spawn + PIT_GAP + PIT_WIDTH).contains(&x));
    Terrain::from_fn(1600, 600, |x, y| y >= 400 && !shaft(i32::try_from(x).unwrap())).unwrap()
}

/// Copies a bundle, so a test can add a map to it without touching the one it came from.
fn copy_bundle(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_bundle(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Holds an arrow key down for `held`: Bevy walks only while it sees the key down, which a press and
/// release between two of headless Chrome's slow frames never shows.
fn hold_arrow_right(tab: &Tab, held: Duration) {
    let event = |kind| DispatchKeyEvent {
        Type: kind,
        key: Some("ArrowRight".to_owned()),
        code: Some("ArrowRight".to_owned()),
        windows_virtual_key_code: Some(39),
        native_virtual_key_code: Some(39),
        modifiers: None,
        timestamp: None,
        text: None,
        unmodified_text: None,
        key_identifier: None,
        auto_repeat: None,
        is_keypad: None,
        is_system_key: None,
        location: None,
        commands: None,
    };
    tab.call_method(event(DispatchKeyEventTypeOption::RawKeyDown)).unwrap();
    std::thread::sleep(held);
    tab.call_method(event(DispatchKeyEventTypeOption::KeyUp)).unwrap();
}

/// The map column under a point on the page, once the page reports one.
fn map_column(tab: &Tab, point: Point) -> Option<i64> {
    tab.move_mouse_to_point(point).unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if let Some(x) = page_state(tab).and_then(|state| state["map_x"].as_i64()) {
            return Some(x);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Clicks the map at `point` until the page reports the drop point picked, and returns its column.
/// A click can land on a frame that misses it, as with buttons.
fn pick(tab: &Tab, point: Point) -> i64 {
    for _ in 0..10 {
        tab.click_point(point).unwrap();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            if let Some(x) = page_state(tab).and_then(|state| state["me"]["drops_at"].as_i64()) {
                return x;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    panic!("clicking the map never picked a drop point; last state {:?}", page_state(tab));
}

/// PRD-0007 T-001: a dead player in Score clicks the map to drop back in. Online, not in practice:
/// practice's hills have no pit to fall into, and its bots take minutes to kill a player that
/// doesn't fight back, longer than a test may run. Here two protocol clients, one on each team,
/// pass every turn, and the browser plays the third seat on a field with a shaft beside each
/// spawn: it walks into its shaft on its first turn, picks a drop point by clicking the map, and
/// drops back in there (no wind), alive and whole, to play its turn; a drop that falls through a
/// shaft costs nothing, and it picks again.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Chrome and a client bundle; run `cargo make browser`"]
async fn e2e_browser_score_a_dead_player_clicks_the_map_to_drop_back_in() {
    let dist = TempDir::new().unwrap();
    copy_bundle(&bundle(), dist.path());
    write_map(dist.path(), "pits", &pit_field(), &PIT_SPAWNS).unwrap();
    let cwd = TempDir::new().unwrap();
    let server = start(dist.path(), cwd.path());
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    // The host configures Score on the pit field, in no wind; the browser joins team B, and the
    // host moves over beside it, leaving team A to a rival.
    let mut host = Client::connect(addr, &code, false).await;
    let (host_id, _, _) = host.hello("Host", None).await;
    host.send(&ClientMsg::Configure(RoomOptions {
        map: "pits".into(),
        team_size: 2,
        max_wind: 0,
        turn_seconds: 10,
        random_teams: false,
        sudden_death: None,
        mode: Mode::Score,
    }))
    .await;
    let browser = launch();
    let tab = browser.new_tab().unwrap();
    let origin = format!("http://{addr}");
    let room_url = format!("{origin}/r/{code}");
    let tab = tokio::task::spawn_blocking(move || {
        tab.navigate_to(&origin).unwrap().wait_until_navigated().unwrap();
        // Coilback walks into a pit as anything does; a floating mobile might not.
        tab.evaluate("localStorage.setItem('lobshot.name', 'Diver'); localStorage.setItem('lobshot.mobile', 'Coilback')", false)
            .unwrap();
        tab.navigate_to(&room_url).unwrap().wait_until_navigated().unwrap();
        wait_for(&tab, "the lobby", |state| state["screen"] == "Lobby");
        click_until(&tab, "Ready(true)", "ready", |state| spot(state, "Ready(false)").is_some());
        tab
    })
    .await
    .unwrap();
    host.send(&ClientMsg::Team(Some(Team::B))).await;
    let mut rival = Client::connect(addr, &code, false).await;
    let (rival_id, _, _) = rival.hello("Rival", None).await;
    rival.send(&ClientMsg::Team(Some(Team::A))).await;
    rival.send(&ClientMsg::Ready(true)).await;
    host.send(&ClientMsg::Ready(true)).await;
    host.recv_until(
        |msg| matches!(msg, ServerMsg::Room(view) if view.members.len() == 3 && view.members.iter().all(|m| m.ready) && view.members.iter().filter(|m| m.team == Some(Team::B)).count() == 2),
    )
    .await;
    host.send(&ClientMsg::Start).await;
    let maps = || Maps::new(&dist.path().join("assets"));
    let passing = [tokio::spawn(pass_out(host, host_id, maps(), PASS_AFTER)), tokio::spawn(pass_out(rival, rival_id, maps(), PASS_AFTER))];

    let state = tokio::task::spawn_blocking(move || -> (i64, Value) {
        // On its turn, into the pit; again on its next, should a walk go unseen.
        for _ in 0..3 {
            let now = wait_for(&tab, "the player's turn", |state| {
                clock_on_time(state);
                state["screen"] == "Game" && (enabled(state, "Item(0)") == Some(true) || state["me"]["alive"] == false)
            });
            if now["me"]["alive"] == false {
                break;
            }
            hold_arrow_right(&tab, Duration::from_millis(1500));
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(5) && page_state(&tab).is_some_and(|state| state["me"]["alive"] == true) {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let dead = wait_for(&tab, "the player dead, free to pick a drop point", |state| state["me"]["can_respawn"] == true);
        assert_eq!(dead["me"]["alive"], false, "{dead}");

        // A click on the map, away from the shafts as the pointer reads it, picks where it drops
        // back in. The camera glides between turns, so the column picked may still be a shaft's:
        // then it falls through, at no cost, and picks again.
        let clear = |x: i64| PIT_SPAWNS.iter().all(|&spawn| (x - i64::from(spawn + PIT_GAP)).abs() > 40);
        for _ in 0..3 {
            let point = [Point { x: 512.0, y: 300.0 }, Point { x: 300.0, y: 300.0 }, Point { x: 720.0, y: 300.0 }]
                .into_iter()
                .find(|&point| map_column(&tab, point).is_some_and(clear))
                .expect("a spot on the map clear of the shafts");
            let x = pick(&tab, point);
            // It drops in there, alive and whole, and plays its turn; or it fell through.
            let after = wait_for(&tab, "the player back on its turn, or free to pick again", |state| {
                clock_on_time(state);
                (state["me"]["alive"] == true && enabled(state, "Item(0)") == Some(true)) || state["me"]["can_respawn"] == true
            });
            if after["me"]["alive"] == true {
                return (x, after);
            }
        }
        panic!("three drops in a row fell through the map");
    })
    .await
    .unwrap();
    let (x, back) = state;
    assert_eq!(back["me"]["x"].as_i64(), Some(x), "straight down where it clicked, in no wind: {back}");
    assert_eq!(back["me"]["hp"].as_i64(), Some(i64::from(MobileKind::Coilback.tuning().hp.value)), "whole again: {back}");
    assert_eq!(back["me"]["drops_at"], Value::Null, "{back}");
    assert_eq!(back["me"]["can_respawn"], false, "{back}");
    for task in passing {
        task.abort();
    }
    drop(browser);
    drop(server);
}
