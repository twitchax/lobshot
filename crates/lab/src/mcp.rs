//! `lobshot-lab mcp`: an MCP server on stdin and stdout, so an agent (Claude Code, or any MCP
//! client) can play through `agent`. MCP's stdio transport is JSON-RPC 2.0, one message a line,
//! and a server with only tools needs four methods: `initialize`, `ping`, `tools/list` and
//! `tools/call`. That is written out plainly here, synchronous like the rest of the lab: the
//! dispatch is the `match` in `Server::answer`.

use std::io::{BufRead, Write};

use anyhow::Context as _;
use serde_json::{Value, json};

use lobshot_assets::maps::Maps;
use lobshot_protocol::Action;
use lobshot_sim::base::{Res, Void};

use crate::agent::{NewGame, Session, TraceRequest, rules};

/// The MCP revisions this server speaks, newest first. Tools are all it offers, and they work
/// the same in each.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What an agent reads first, in the `initialize` reply.
const INSTRUCTIONS: &str = "You play lobshot, a turn-based artillery game, from one seat against SearchBot. Read `rules` \
once. Start with `new_game`; it and `act` return what you see, with what happened since you last looked. On your turn, \
`trace` a few shots to see where they land, then `act`. Play to the end, then write up what felt off in the format of \
docs/playtests/README.md.";

/// JSON-RPC's codes for a method that doesn't exist, bad parameters, and unreadable JSON.
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const PARSE_ERROR: i64 = -32700;

/// One agent's server: the maps to seat games on, and its game, once it starts one.
pub struct Server {
    maps: Maps,
    session: Option<Session>,
}

impl Server {
    #[must_use]
    pub fn new(maps: Maps) -> Self {
        Self { maps, session: None }
    }

    /// Answers messages from `input` on `output`, a line each, until `input` ends.
    ///
    /// # Errors
    ///
    /// Fails when either stream does.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> Void {
        for line in input.lines() {
            let line = line.context("reading a message")?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle(&line) {
                writeln!(output, "{reply}").context("writing a reply")?;
                output.flush().context("writing a reply")?;
            }
        }
        Ok(())
    }

    /// The reply to one message; `None` for a notification, which gets none.
    pub fn handle(&mut self, message: &str) -> Option<Value> {
        let Ok(message) = serde_json::from_str::<Value>(message) else {
            return Some(failure(&Value::Null, PARSE_ERROR, "that isn't JSON"));
        };
        // A message without an id is a notification (`notifications/initialized` and the like).
        let id = message.get("id")?.clone();
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        Some(match self.answer(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, error)) => failure(&id, code, &error),
        })
    }

    fn answer(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str);
                let version = VERSIONS.iter().find(|&&v| Some(v) == asked).unwrap_or(&VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "lobshot-lab", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "a tool call names its tool".to_owned()))?;
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                // A tool that fails is still an answer: the agent reads why and tries again.
                Ok(match self.call(name, arguments) {
                    Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                    Err(error) => json!({ "content": [{ "type": "text", "text": format!("{error:#}") }], "isError": true }),
                })
            }
            other => Err((METHOD_NOT_FOUND, format!("there is no method {other:?}"))),
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Res<String> {
        match tool {
            "rules" => Ok(rules()),
            "new_game" => {
                let new: NewGame = serde_json::from_value(arguments).context("new_game's arguments")?;
                let session = self.session.insert(Session::new(&new, &self.maps)?);
                pretty(&session.view(&self.maps)?)
            }
            "view" => pretty(&started(&mut self.session)?.view(&self.maps)?),
            "trace" => {
                let request: TraceRequest = serde_json::from_value(arguments).context("trace's arguments")?;
                pretty(&started(&mut self.session)?.trace(&request)?)
            }
            "act" => {
                let action: Action = serde_json::from_value(arguments.get("action").cloned().unwrap_or(Value::Null)).context("act's action")?;
                started(&mut self.session)?.act(action)?;
                pretty(&started(&mut self.session)?.view(&self.maps)?)
            }
            other => anyhow::bail!("there is no tool {other:?}"),
        }
    }
}

fn started(session: &mut Option<Session>) -> Res<&mut Session> {
    session.as_mut().context("no game yet: start one with new_game")
}

fn pretty(value: &impl serde::Serialize) -> Res<String> {
    serde_json::to_string_pretty(value).context("writing the reply")
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The tools, with their input schemas.
fn tools() -> Value {
    let shot = json!({ "type": "string", "enum": ["S1", "S2", "Ss"] });
    let facing = json!({ "type": "string", "enum": ["Left", "Right"] });
    let maps = format!("{}, or flat; tidewall when left out.", lobshot_assets::MAPS.join(", "));
    json!([
        {
            "name": "rules",
            "description": "How lobshot plays: coordinates, actions, delay, wind and weather, and every mobile's shots and every item.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "new_game",
            "description": "Start a game (replacing any game in progress) and see it. You are seat 0 on team A; every other seat is SearchBot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mobile": { "type": "string", "description": "Your mobile, e.g. coilback; random when left out." },
                    "against": { "type": "array", "items": { "type": "string" }, "description": "The other team's mobiles, one a seat; random when left out." },
                    "map": { "type": "string", "description": maps },
                    "team_size": { "type": "integer", "minimum": 1, "maximum": 4, "description": "Players a team; 1 when left out." },
                    "mode": { "type": "string", "enum": ["solo", "score", "tag", "jewel"], "description": "What the game is played for: solo (last team standing), score (team lives, and the dead drop back in), tag (two mobiles each, switching once a turn) or jewel (jewels for points, and no one is hurt); solo when left out." },
                    "second": { "type": "string", "description": "Your second mobile in Tag, e.g. hookclaw; random when left out." },
                    "level": { "type": "string", "enum": ["easy", "normal", "hard"], "description": "How well the bots play; normal when left out." },
                    "seed": { "type": "integer", "minimum": 0, "description": "Replays a game: the same seed and setup play the same game." },
                },
            },
        },
        {
            "name": "view",
            "description": "What you see now, and what happened since you last looked.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "trace",
            "description": "Where a shot would land if you fired it now, the way you face, and whom it would hurt, in the wind as it is. Changes nothing. To shoot the other way, turn first with a Walk of 0 steps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": shot,
                    "aim": { "type": "integer", "description": "Degrees above level toward where you face; within the shot's range." },
                    "power": { "type": "integer", "minimum": 0, "maximum": 80, "description": "Twentieths of a bar: 0 to 80." },
                    "item": { "type": "integer", "minimum": 0, "description": "Arm the item in this loadout slot first." },
                },
                "required": ["shot", "aim", "power"],
            },
        },
        {
            "name": "act",
            "description": "Your move: walk, use an item, switch mobiles (Tag), shoot or pass; or, dead in Score or Jewel, pick where to drop back in. The bots then play until your next turn (or your next pick), and you see the result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "description": "One of {\"Walk\": {\"facing\": \"Left\", \"steps\": 40}}, {\"Item\": {\"slot\": 0}}, {\"Shoot\": {\"shot\": \"S1\", \"aim\": 45, \"power\": 48}}, \"Pass\", in Tag \"Switch\", or, dead in Score or Jewel, {\"Respawn\": {\"x\": 600}}.",
                        "oneOf": [
                            { "type": "object", "properties": { "Walk": { "type": "object", "properties": { "facing": facing, "steps": { "type": "integer", "minimum": 0 } }, "required": ["facing", "steps"] } }, "required": ["Walk"] },
                            { "type": "object", "properties": { "Item": { "type": "object", "properties": { "slot": { "type": "integer", "minimum": 0 } }, "required": ["slot"] } }, "required": ["Item"] },
                            { "type": "object", "properties": { "Shoot": { "type": "object", "properties": { "shot": shot, "aim": { "type": "integer" }, "power": { "type": "integer", "minimum": 0, "maximum": 80 } }, "required": ["shot", "aim", "power"] } }, "required": ["Shoot"] },
                            { "const": "Pass" },
                            { "const": "Switch" },
                            { "type": "object", "properties": { "Respawn": { "type": "object", "properties": { "x": { "type": "integer", "minimum": 0 } }, "required": ["x"] } }, "required": ["Respawn"] },
                        ],
                    },
                },
                "required": ["action"],
            },
        },
    ])
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;

    fn server() -> Server {
        Server::new(Maps::new(Path::new("no-assets")))
    }

    fn call(server: &mut Server, id: u32, tool: &str, arguments: &Value) -> Value {
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
        let reply = server.handle(&message.to_string()).unwrap();
        assert_eq!(reply["id"], id);
        reply["result"].clone()
    }

    fn text(result: &Value) -> &str {
        result["content"][0]["text"].as_str().unwrap()
    }

    #[test]
    fn initialize_agrees_on_a_version_and_offers_tools() {
        let mut server = server();
        let reply = server
            .handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#)
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(reply["result"]["serverInfo"]["name"], "lobshot-lab");
        assert!(reply["result"]["capabilities"]["tools"].is_object());
        let unknown = server.handle(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).unwrap();
        assert_eq!(unknown["result"]["protocolVersion"], VERSIONS[0], "the newest it knows");
        assert_eq!(server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        assert_eq!(server.handle(r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#).unwrap()["result"], json!({}));
    }

    #[test]
    fn every_tool_is_listed_with_a_schema() {
        let reply = server().handle(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["rules", "new_game", "view", "trace", "act"]);
        assert!(tools.iter().all(|t| t["inputSchema"]["type"] == "object" && t["description"].is_string()));
        let maps = tools[1]["inputSchema"]["properties"]["map"]["description"].as_str().unwrap();
        assert!(lobshot_assets::MAPS.iter().all(|map| maps.contains(map)), "new_game names every map: {maps}");
    }

    #[test]
    fn tools_play_a_game() {
        let mut server = server();
        let rules = call(&mut server, 1, "rules", &json!({}));
        assert!(text(&rules).starts_with("# lobshot, for agents"));

        let started = call(&mut server, 2, "new_game", &json!({ "mobile": "coilback", "map": "flat", "seed": 4, "level": "easy" }));
        assert_eq!(started["isError"], false);
        let view: Value = serde_json::from_str(text(&started)).unwrap();
        assert_eq!(view["your_turn"], true);

        let traced = call(&mut server, 3, "trace", &json!({ "shot": "S1", "aim": 45, "power": 40 }));
        let trace: Value = serde_json::from_str(text(&traced)).unwrap();
        assert!(trace["summary"].is_string());

        let acted = call(&mut server, 4, "act", &json!({ "action": "Pass" }));
        let after: Value = serde_json::from_str(text(&acted)).unwrap();
        assert!(after["happened"][0].as_str().unwrap().starts_with("You passed."), "{after}");
    }

    #[test]
    fn mistakes_come_back_as_tool_errors_the_agent_can_read() {
        let mut server = server();
        let early = call(&mut server, 1, "view", &json!({}));
        assert_eq!(early["isError"], true);
        assert!(text(&early).contains("new_game"));
        // A mobile that fires forward, whatever Random would roll as the roster grows.
        call(&mut server, 2, "new_game", &json!({ "mobile": "coilback", "map": "flat", "seed": 1 }));
        let wide = call(&mut server, 3, "act", &json!({ "action": { "Shoot": { "shot": "S1", "aim": 170, "power": 40 } } }));
        assert_eq!(wide["isError"], true);
        assert!(text(&wide).contains("can't aim"), "{}", text(&wide));
        let strong = call(&mut server, 4, "act", &json!({ "action": { "Shoot": { "shot": "S1", "aim": 45, "power": 200 } } }));
        assert_eq!(strong["isError"], true);
        assert_eq!(call(&mut server, 5, "nope", &json!({}))["isError"], true);
    }

    #[test]
    fn protocol_errors_follow_json_rpc() {
        let mut server = server();
        assert_eq!(server.handle("{not json").unwrap()["error"]["code"], PARSE_ERROR);
        assert_eq!(server.handle(r#"{"jsonrpc":"2.0","id":9,"method":"resources/list"}"#).unwrap()["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(server.handle(r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{}}"#).unwrap()["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn it_serves_a_line_a_message() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            "\n\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
            "\n"
        );
        let mut output = Vec::new();
        server().serve(input.as_bytes(), &mut output).unwrap();
        let lines: Vec<Value> = String::from_utf8(output).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.iter().map(|l| l["id"].clone()).collect::<Vec<_>>(), [json!(1), json!(2)]);
    }
}
