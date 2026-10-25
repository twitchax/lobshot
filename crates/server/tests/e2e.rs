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

use lobshot_assets::maps::Maps;
use lobshot_protocol::{Action, ClientMsg, RoomOptions, ServerMsg, Stage};
use lobshot_server::tests::{INDEX_HTML, create_room, http_get, write_bundle, write_map};
use lobshot_sim::{
    game::{Effect, Event, Game, Phase, SuddenDeath, SuddenDeathKind, Team},
    map::NoAssets,
    mode::Mode,
    terrain::Terrain,
    units::Facing,
};

use support::{Client, Follower, pass_out, play_out};

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
    start_with(dist, cwd, &[])
}

/// `start`, with more flags.
fn start_with(dist: &Path, cwd: &Path, flags: &[&str]) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lobshot"))
        .current_dir(cwd)
        .args(["--bind", "127.0.0.1:0"])
        .args(flags)
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
            mode: Mode::Solo,
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

/// M3's acceptance test (PRD-0004 uat-002): a 4v4 Solo game on `flat` with sudden death (Double
/// at 40), every seat a bot, plays to the end against the real binary. The host spectates and a
/// second spectator follows in JSON; each replays the log it receives and checks the server's
/// hash after every event. Bots play at once and turns start at once, since no one watches.
///
/// Sudden death is set up, and checked to start at turn 40 when the game gets that far, but not
/// required: whether it does is chance. Normal bots on `flat`, spawns shuffled, run a median 45
/// turns and pass turn 40 in about 80% of games (`lobshot_lab`'s `tool_length`, 128 seeds).
/// The sim's golden replays (`brawl_4v4`, `duel_big_bomb`, `duel_ss_death`) play each kind through.
#[tokio::test(flavor = "multi_thread")]
async fn e2e_solo_eight_bots_play_a_4v4_with_items_and_sudden_death() {
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let server = start_with(dist.path(), cwd.path(), &["--bot-delay-ms", "0", "--no-playback-wait"]);
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    let mut host = Client::connect(addr, &code, false).await;
    let (host_id, _, _) = host.hello("Host", None).await;
    host.send(&ClientMsg::Team(None)).await;
    let sudden_death = SuddenDeath { kind: SuddenDeathKind::Double, turn: 40 };
    host.send(&ClientMsg::Configure(RoomOptions {
        map: "flat".into(),
        team_size: 4,
        max_wind: 26,
        turn_seconds: 5,
        random_teams: false,
        sudden_death: Some(sudden_death),
        mode: Mode::Solo,
    }))
    .await;
    for team in [Team::A, Team::B].repeat(4) {
        host.send(&ClientMsg::AddBot { team }).await;
    }
    host.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.members.iter().filter(|m| m.bot).count() == 8)).await;

    let mut watcher = Client::connect(addr, &code, true).await;
    let (watcher_id, _, view) = watcher.hello("Watcher", None).await;
    assert_eq!(view.members.iter().find(|m| m.id == watcher_id).unwrap().team, None, "every seat is taken");
    host.send(&ClientMsg::Start).await;

    let ((host_replica, room), (watcher_replica, _)) = tokio::join!(play_out(&mut host, host_id, None, 0), play_out(&mut watcher, watcher_id, None, 0));

    let (game, log) = (host_replica.game().unwrap(), host_replica.log().unwrap());
    assert!(matches!(game.phase(), Phase::Over { .. }), "{:?}", game.phase());
    assert_eq!(game.state_hash().unwrap(), watcher_replica.game().unwrap().state_hash().unwrap());
    assert_eq!(log, watcher_replica.log().unwrap());
    assert_eq!(log.setup.seats.len(), 8);
    assert_eq!(log.setup.sudden_death, Some(sudden_death));
    assert_eq!(room.stage, Stage::Lobby);

    let items = log.events.iter().filter(|event| matches!(event, Event::UseItem { .. })).count();
    assert!(items > 0, "the bots used no items in {} turns", game.turns());

    // When the game outlasts turn 40, sudden death starts exactly then.
    let mut replay = Game::new(&log.setup, &NoAssets).unwrap();
    let mut started = None;
    for &event in &log.events {
        if replay.apply(event).unwrap().iter().any(|effect| matches!(effect, Effect::SuddenDeath { .. })) {
            started.get_or_insert(replay.turns());
        }
    }
    println!("the 4v4 ran {} turns; sudden death started at {started:?}", game.turns());
    if game.turns() > 40 {
        assert_eq!(started, Some(40));
    }
}

/// PRD-0007 uat-002, Score (T-001): a 2v2 Score game on `flat`, every seat a bot, plays to the end
/// against the real binary. A spectator replays the log and checks the server's hash after every
/// event. Deaths cost lives, dead bots pick where to drop back in, and the game ends when a team
/// is out of lives or has no one standing.
#[tokio::test(flavor = "multi_thread")]
async fn e2e_modes_score_four_bots_play_a_2v2_to_the_end() {
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let server = start_with(dist.path(), cwd.path(), &["--bot-delay-ms", "0", "--no-playback-wait"]);
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    let mut host = Client::connect(addr, &code, true).await;
    let (host_id, _, _) = host.hello("Host", None).await;
    host.send(&ClientMsg::Team(None)).await;
    host.send(&ClientMsg::Configure(RoomOptions {
        map: "flat".into(),
        team_size: 2,
        max_wind: 26,
        turn_seconds: 5,
        random_teams: false,
        sudden_death: None,
        mode: Mode::Score,
    }))
    .await;
    for team in [Team::A, Team::B].repeat(2) {
        host.send(&ClientMsg::AddBot { team }).await;
    }
    host.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.members.iter().filter(|m| m.bot).count() == 4 && view.options.mode == Mode::Score))
        .await;
    host.send(&ClientMsg::Start).await;

    let (replica, room) = play_out(&mut host, host_id, None, 0).await;
    let (game, log) = (replica.game().unwrap(), replica.log().unwrap());
    assert_eq!(room.stage, Stage::Lobby);
    assert_eq!(log.setup.mode, Mode::Score);
    let Phase::Over { winner: Some(winner) } = game.phase() else { panic!("{:?}", game.phase()) };
    let loser = if winner == Team::A { Team::B } else { Team::A };
    let standing = game.players().iter().any(|p| p.alive && p.team == loser);
    assert!(
        game.points(loser) == Some(0) || !standing,
        "team {loser:?} lost with {:?} lives and someone standing",
        game.points(loser)
    );

    // Replayed: lives went, and the dead dropped back in where they picked.
    let mut replay = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut lives, mut landed) = (0, 0);
    for &event in &log.events {
        for effect in replay.apply(event).unwrap() {
            match effect {
                Effect::Points { .. } => lives += 1,
                Effect::Respawned { to: Some(_), .. } => landed += 1,
                _ => {}
            }
        }
    }
    let picks = log.events.iter().filter(|event| matches!(event, Event::Respawn { .. })).count();
    println!("the Score 2v2 ran {} turns: {lives} lives lost, {picks} drop points picked, {landed} landed", game.turns());
    assert!(lives >= 2 && picks >= 1, "{lives} lives lost, {picks} picks");
}

/// PRD-0007 uat-002, Tag (T-002): a 2v2 Tag game on `flat`, every seat a bot with two mobiles,
/// plays to the end against the real binary. A spectator replays the log and checks the server's
/// hash after every event. The bots switch when it helps, and a player is out when the mobile in
/// play falls, whatever the other has left.
#[tokio::test(flavor = "multi_thread")]
async fn e2e_modes_tag_four_bots_play_a_2v2_to_the_end() {
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    let server = start_with(dist.path(), cwd.path(), &["--bot-delay-ms", "0", "--no-playback-wait"]);
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    let mut host = Client::connect(addr, &code, true).await;
    let (host_id, _, _) = host.hello("Host", None).await;
    host.send(&ClientMsg::Team(None)).await;
    host.send(&ClientMsg::Configure(RoomOptions {
        map: "flat".into(),
        team_size: 2,
        max_wind: 26,
        turn_seconds: 5,
        random_teams: false,
        sudden_death: None,
        mode: Mode::Tag,
    }))
    .await;
    for team in [Team::A, Team::B].repeat(2) {
        host.send(&ClientMsg::AddBot { team }).await;
    }
    host.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.members.iter().filter(|m| m.bot).count() == 4 && view.options.mode == Mode::Tag))
        .await;
    host.send(&ClientMsg::Start).await;

    let (replica, room) = play_out(&mut host, host_id, None, 0).await;
    let (game, log) = (replica.game().unwrap(), replica.log().unwrap());
    assert_eq!(room.stage, Stage::Lobby);
    assert_eq!(log.setup.mode, Mode::Tag);
    assert!(log.setup.seats.iter().all(|seat| seat.second.is_some()), "every seat brings two");
    let Phase::Over { winner: Some(winner) } = game.phase() else { panic!("{:?}", game.phase()) };
    let loser = if winner == Team::A { Team::B } else { Team::A };
    assert!(game.players().iter().all(|p| !(p.alive && p.team == loser)), "team {loser:?} has no one standing");

    // Replayed: the bots switched, and players went out with a mobile left sitting out.
    let mut replay = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut switches, mut out_with_one_left) = (0, 0);
    for &event in &log.events {
        for effect in replay.apply(event).unwrap() {
            match effect {
                Effect::Switched { .. } => switches += 1,
                Effect::Died { seat, .. } if replay.bench(seat).is_some_and(|bench| bench.hp > 0) => out_with_one_left += 1,
                _ => {}
            }
        }
    }
    println!("the Tag 2v2 ran {} turns: {switches} switches, {out_with_one_left} out with a mobile left", game.turns());
    assert!(switches >= 1 && out_with_one_left >= 2, "{switches} switches, {out_with_one_left} out with a mobile left");
}

/// PRD-0007 T-001: a human player in Score dies, sends `Action::Respawn` over its socket, and its
/// mobile drops back in where it picked, against the real binary. Its teammate and a rival are
/// protocol clients that pass every turn; it walks into a shaft beside its spawn on its first turn.
#[tokio::test(flavor = "multi_thread")]
async fn e2e_modes_score_a_human_picks_a_drop_point_and_drops_back_in() {
    const SPAWNS: [i32; 4] = [100, 500, 900, 1300];
    const DROP_X: i32 = 700;
    let cwd = TempDir::new().unwrap();
    let dist = TempDir::new().unwrap();
    write_bundle(dist.path()).unwrap();
    // Ground from y = 400, with a shaft off the map 10 px right of each spawn.
    let shaft = |x: i32| SPAWNS.iter().any(|&spawn| (spawn + 10..spawn + 13).contains(&x));
    let pits = Terrain::from_fn(1600, 600, |x, y| y >= 400 && !shaft(i32::try_from(x).unwrap())).unwrap();
    write_map(dist.path(), "pits", &pits, &SPAWNS).unwrap();
    let server = start_with(dist.path(), cwd.path(), &["--bot-delay-ms", "0", "--no-playback-wait"]);
    let addr = server.addr;
    let code = tokio::task::spawn_blocking(move || create_room(addr)).await.unwrap().unwrap();

    // The diver hosts on team A, its mate joins it, and a rival takes team B.
    let mut diver = Client::connect(addr, &code, true).await;
    let (diver_id, _, _) = diver.hello("Diver", None).await;
    diver
        .send(&ClientMsg::Configure(RoomOptions {
            map: "pits".into(),
            team_size: 2,
            max_wind: 0,
            turn_seconds: 20,
            random_teams: false,
            sudden_death: None,
            mode: Mode::Score,
        }))
        .await;
    diver.send(&ClientMsg::Mobile(Some(lobshot_sim::game::MobileKind::Coilback))).await;
    let mut mate = Client::connect(addr, &code, false).await;
    let (mate_id, _, _) = mate.hello("Mate", None).await;
    mate.send(&ClientMsg::Team(Some(Team::A))).await;
    let mut rival = Client::connect(addr, &code, false).await;
    let (rival_id, _, _) = rival.hello("Rival", None).await;
    rival.send(&ClientMsg::Team(Some(Team::B))).await;
    for client in [&mut diver, &mut mate, &mut rival] {
        client.send(&ClientMsg::Ready(true)).await;
    }
    diver
        .recv_until(
            |msg| matches!(msg, ServerMsg::Room(view) if view.members.len() == 3 && view.members.iter().all(|m| m.ready) && view.members.iter().filter(|m| m.team == Some(Team::A)).count() == 2),
        )
        .await;
    diver.send(&ClientMsg::Start).await;
    let maps = || Maps::new(&dist.path().join("assets"));
    let passing = [
        tokio::spawn(pass_out(mate, mate_id, maps(), Duration::ZERO)),
        tokio::spawn(pass_out(rival, rival_id, maps(), Duration::ZERO)),
    ];

    // Into the pit on its turn, a pick once dead, and its turn again once it is back.
    let mut follower = Follower::new(diver_id, maps());
    let (mut walked, mut picked) = (false, false);
    let (game, seat) = loop {
        follower.absorb(diver.recv().await);
        let (Some(game), Some(seat)) = (follower.game.as_ref(), follower.seat) else { continue };
        if !walked && follower.my_turn().is_some() {
            walked = true;
            diver.send(&ClientMsg::Act(Action::Walk { facing: Facing::Right, steps: 30 })).await;
        } else if !picked && game.can_respawn(seat) {
            picked = true;
            diver.send(&ClientMsg::Act(Action::Respawn { x: DROP_X })).await;
        } else if picked && follower.my_turn().is_some() {
            break (game.clone(), seat);
        }
    };
    for task in passing {
        task.abort();
    }

    let me = game.players()[usize::from(seat)];
    assert!(me.alive, "{me:?}");
    assert_eq!(me.footing.x, DROP_X, "straight down where it picked, in no wind");
    assert_eq!(me.hp, me.tuning().hp.value, "whole again");
    assert_eq!(game.points(Team::A), Some(2), "two players and one more, less its death");
    assert_eq!(game.points(Team::B), Some(2), "one player and one more");
    assert_eq!(game.respawns(), []);
}
