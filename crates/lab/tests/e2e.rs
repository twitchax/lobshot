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

/// A plain agent's next move from its view: face the nearest enemy, then the S1 at 45 degrees
/// that `trace` says hurts it most (powers `step` apart), or a pass when none does.
fn next_move(view: &serde_json::Value, step: usize, mut trace: impl FnMut(u8) -> serde_json::Value) -> serde_json::Value {
    let players = view["players"].as_array().unwrap();
    let me = players.iter().find(|p| p["you"] == true).unwrap();
    let enemy = players
        .iter()
        .filter(|p| p["alive"] == true && p["team"] != me["team"])
        .min_by_key(|p| (p["x"].as_i64().unwrap() - me["x"].as_i64().unwrap()).abs())
        .unwrap();
    let toward = if enemy["x"].as_i64() > me["x"].as_i64() { "Right" } else { "Left" };
    if me["facing"] != toward {
        return serde_json::json!({ "Walk": { "facing": toward, "steps": 0 } });
    }
    let best = (0..=80_u8)
        .step_by(step)
        .filter_map(|power| {
            let traced = trace(power);
            let hurt: i64 = traced["damage"].as_array()?.iter().filter(|h| h["enemy"] == true).map(|h| h["amount"].as_i64().unwrap()).sum();
            (hurt > 0).then_some((hurt, power))
        })
        .max();
    best.map_or_else(|| serde_json::json!("Pass"), |(_, power)| serde_json::json!({ "Shoot": { "shot": "S1", "aim": 45, "power": power } }))
}

/// An MCP client of one `lobshot-lab mcp` process: JSON-RPC a line at a time.
struct Client {
    stdin: std::process::ChildStdin,
    stdout: std::io::BufReader<std::process::ChildStdout>,
    id: u32,
}

impl Client {
    fn request(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        use std::io::{BufRead as _, Write as _};

        self.id += 1;
        writeln!(self.stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params })).unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], self.id, "{reply}");
        reply["result"].clone()
    }

    /// A tool's reply, parsed as the JSON it carries.
    fn tool(&mut self, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
        let result = self.request("tools/call", &serde_json::json!({ "name": name, "arguments": arguments }));
        assert_eq!(result["isError"], false, "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}

/// uat-003: an agent plays a whole game against SearchBot through `lobshot-lab mcp`, speaking
/// MCP's JSON-RPC over the process's stdin and stdout as an MCP client does.
#[test]
fn e2e_agent_plays_a_full_game_over_mcp() {
    let dir = TempDir::new().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lobshot-lab"))
        .current_dir(dir.path())
        .arg("mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut client = Client {
        stdin: child.stdin.take().unwrap(),
        stdout: std::io::BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    let init = client.request(
        "initialize",
        &serde_json::json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "e2e", "version": "0" } }),
    );
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(client.request("tools/list", &serde_json::json!({}))["tools"].as_array().unwrap().len(), 5);

    let mut view = client.tool(
        "new_game",
        &serde_json::json!({ "mobile": "coilback", "against": ["sparkhorn"], "map": "flat", "level": "easy", "seed": 7 }),
    );
    for _ in 0..150 {
        if view["over"] == true {
            break;
        }
        let action = next_move(&view, 4, |power| client.tool("trace", &serde_json::json!({ "shot": "S1", "aim": 45, "power": power })));
        view = client.tool("act", &serde_json::json!({ "action": action }));
    }
    assert_eq!(view["over"], true, "{view}");
    assert!(view["status"].as_str().unwrap().starts_with("Game over"), "{view}");
    drop(client);
    assert!(child.wait().unwrap().success(), "it ends when its input does");
}

/// The same agent through `lobshot-lab play`, a process a step over a save file.
#[test]
fn e2e_agent_plays_a_full_game_over_the_cli() {
    let dir = TempDir::new().unwrap();
    let json = |args: &[&str]| -> serde_json::Value { serde_json::from_str(&stdout(&run_in(dir.path(), args))).unwrap() };
    let rules = stdout(&run_in(dir.path(), &["play", "rules"]));
    assert!(rules.contains("## Mobiles"));

    let mut view = json(&["play", "new", "--mobile", "coilback", "--against", "sparkhorn", "--map", "flat", "--level", "easy", "--seed", "7"]);
    assert!(dir.path().join("game.json").exists());
    for _ in 0..150 {
        if view["over"] == true {
            break;
        }
        let action = next_move(&view, 8, |power| json(&["play", "trace", "--shot", "s1", "--aim", "45", "--power", &power.to_string()]));
        view = json(&["play", "act", &action.to_string()]);
    }
    assert_eq!(view["over"], true, "{view}");
    let again = json(&["play", "view"]);
    assert_eq!(
        (again["over"].clone(), again["happened"].clone()),
        (serde_json::json!(true), serde_json::json!([])),
        "nothing new since the last look"
    );
    let refused = run_in(dir.path(), &["play", "act", "Pass"]);
    assert!(!refused.status.success() && String::from_utf8_lossy(&refused.stderr).contains("over"));
}

/// PRD-0007 T-001: `play new --mode score` seats a Score game, and the view counts each team's lives.
#[test]
fn e2e_play_starts_a_score_game_that_counts_lives() {
    let dir = TempDir::new().unwrap();
    let args = ["play", "new", "--map", "flat", "--team-size", "2", "--mode", "score", "--level", "easy", "--seed", "4"];
    let view: serde_json::Value = serde_json::from_str(&stdout(&run_in(dir.path(), &args))).unwrap();
    assert_eq!(view["mode"], "Score");
    // A life a player and one more, less any the bots' opening turns cost.
    let points = view["points"].as_array().unwrap();
    assert_eq!(points.iter().map(|p| p["team"].as_str().unwrap()).collect::<Vec<_>>(), ["A", "B"]);
    assert!(points.iter().all(|p| (1..=3).contains(&p["points"].as_i64().unwrap())), "{points:?}");
    assert_eq!(view["can_respawn"], false);
    let refused = run_in(dir.path(), &["play", "new", "--map", "flat", "--mode", "tag"]);
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("Tag mode isn't built yet"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
}

/// PRD-0007 T-003: `play new --mode jewel` seats a Jewel game, and the view shows the jewels on
/// the map and each team's points.
#[test]
fn e2e_play_starts_a_jewel_game_with_jewels_on_the_map() {
    let dir = TempDir::new().unwrap();
    let args = ["play", "new", "--map", "flat", "--mode", "jewel", "--level", "easy", "--seed", "4"];
    let view: serde_json::Value = serde_json::from_str(&stdout(&run_in(dir.path(), &args))).unwrap();
    assert_eq!(view["mode"], "Jewel");
    let points = view["points"].as_array().unwrap();
    assert_eq!(points.iter().map(|p| p["team"].as_str().unwrap()).collect::<Vec<_>>(), ["A", "B"]);
    let jewels = view["jewels"].as_array().unwrap();
    assert!(!jewels.is_empty(), "{view}");
    assert!(jewels.iter().all(|jewel| jewel["needs"].as_i64().unwrap() >= 100 && jewel["x"].as_i64().is_some()), "{jewels:?}");
}
