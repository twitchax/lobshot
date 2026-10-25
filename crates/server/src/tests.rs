//! Fixtures shared by unit tests, integration tests and benches. Always compiled (not
//! `#[cfg(test)]`), so `tests/` and `benches/` can use it too; keep it free of test-only
//! dependencies.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail, ensure};

use lobshot_protocol::{Action, ServerMsg};
use lobshot_sim::{
    base::{Res, Void},
    bot::{self, Level},
    game::{Event, Game},
    log::Log,
    map::NoAssets,
    terrain::Terrain,
};

use crate::config::Config;

/// The page every fixture bundle serves; tests look for its canvas.
pub const INDEX_HTML: &str = "<!doctype html><title>lobshot</title><canvas id=\"lobshot\"></canvas>\n";

/// Writes a minimal client bundle into `dist`: the page plus a stand-in wasm file, named the way
/// trunk names the real one (minus the hash).
///
/// # Errors
///
/// Fails when the files cannot be written.
pub fn write_bundle(dist: &Path) -> Void {
    std::fs::write(dist.join("index.html"), INDEX_HTML)?;
    std::fs::write(dist.join("lobshot-client_bg.wasm"), b"\0asm\x01\0\0\0")?;
    Ok(())
}

/// Writes a drawn map into `dist`'s assets the way `cargo xtask art` does: a generated manifest
/// with variant `A` (its size and mask hash filled in) and its collision mask.
///
/// # Errors
///
/// Fails when the files cannot be written.
pub fn write_map(dist: &Path, name: &str, mask: &Terrain, spawns: &[i32]) -> Void {
    write_map_variants(dist, name, &[("A", mask, spawns)])
}

/// Like `write_map`, with each variant's id, mask and spawns.
///
/// # Errors
///
/// Fails if a file can't be written.
pub fn write_map_variants(dist: &Path, name: &str, variants: &[(&str, &Terrain, &[i32])]) -> Void {
    lobshot_assets::tests::write_map_variants(&dist.join("assets"), name, variants)
}

/// A server configuration for tests: loopback on any port, `dist` as the bundle, and turns that
/// start and bots that play at once.
#[must_use]
pub fn test_config(dist: &Path) -> Config {
    Config {
        bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        dist: PathBuf::from(dist),
        bot_delay_ms: 0,
        no_playback_wait: true,
        client_ip_header: None,
    }
}

/// Serves the real router on an ephemeral loopback port, in-process, and returns its address. The
/// server task lives as long as the test's runtime.
///
/// # Errors
///
/// Fails when no loopback port can be bound.
pub async fn spawn_test_server(dist: &Path) -> Res<SocketAddr> {
    spawn_server(test_config(dist)).await
}

/// `spawn_test_server` with a configuration of the test's own (`config.bind` is ignored).
///
/// # Errors
///
/// Fails when no loopback port can be bound.
pub async fn spawn_server(config: Config) -> Res<SocketAddr> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move { crate::server::serve(listener, &config).await });
    Ok(addr)
}

/// A minimal blocking HTTP/1.1 GET, so tests need no HTTP client dependency. Returns the raw
/// response: status line, headers and body.
///
/// # Errors
///
/// Fails when the connection or the read fails.
pub fn http_get(addr: SocketAddr, path: &str) -> Res<String> {
    http(addr, &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"))
}

/// `http_get`'s POST, with an empty body.
///
/// # Errors
///
/// Fails when the connection or the read fails.
pub fn http_post(addr: SocketAddr, path: &str) -> Res<String> {
    http(addr, &format!("POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"))
}

fn http(addr: SocketAddr, request: &str) -> Res<String> {
    use std::io::{Read as _, Write as _};

    let mut stream = std::net::TcpStream::connect(addr)?;
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

/// Opens a room over HTTP and returns its code.
///
/// # Errors
///
/// Fails unless the server answers `201` with a code.
pub fn create_room(addr: SocketAddr) -> Res<String> {
    let response = http_post(addr, "/api/rooms")?;
    ensure!(response.starts_with("HTTP/1.1 201"), "creating a room: {response}");
    let body = response.split("\r\n\r\n").nth(1).context("a response body")?;
    let json: serde_json::Value = serde_json::from_str(body).context("a JSON body")?;
    Ok(json["code"].as_str().context("a code")?.to_owned())
}

/// A client's copy of a room's game, rebuilt only from what the server sends, the way every real
/// client keeps one: replay `Game { log }`, then apply each `Event` and check the server's hash.
/// Plays on `flat` and other synthetic maps.
#[derive(Debug, Clone, Default)]
pub struct Replica {
    game: Option<Game>,
    /// Everything applied so far: the log as this client has it.
    log: Option<Log>,
    /// The turn (`Game::turns`) this client already planned.
    planned: Option<i64>,
    /// How long the log was when this client last picked a drop point.
    picked: Option<usize>,
}

impl Replica {
    /// Takes in one server message; anything but the game is ignored.
    ///
    /// # Errors
    ///
    /// Fails when the log won't replay, an event arrives out of order or is refused, or the state
    /// after an event hashes differently from the server's: a desync.
    pub fn absorb(&mut self, msg: &ServerMsg) -> Void {
        match msg {
            ServerMsg::Game { log } => {
                self.game = Some(log.replay(&NoAssets)?);
                self.log = Some(log.clone());
                self.planned = None;
                self.picked = None;
            }
            ServerMsg::Event { index, event, hash } => {
                let (Some(game), Some(log)) = (self.game.as_mut(), self.log.as_mut()) else {
                    bail!("an event before the game");
                };
                ensure!(*index as usize == log.events.len(), "event {index} arrived, but {} was next", log.events.len());
                game.apply(*event).with_context(|| format!("applying event {index}: {event:?}"))?;
                log.events.push(*event);
                let mine = game.state_hash()?;
                ensure!(mine == *hash, "desync after event {index}: {mine:#018x} here, {hash:#018x} on the server");
            }
            _ => {}
        }
        Ok(())
    }

    #[must_use]
    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }

    /// The game's setup and every event applied since.
    #[must_use]
    pub fn log(&self) -> Option<&Log> {
        self.log.as_ref()
    }

    /// What `seat` does with its turn, as protocol actions: SearchBot's plan, once per turn.
    /// `None` when it isn't `seat`'s turn, a roll is owed, or this turn was already planned.
    pub fn plan(&mut self, seat: u8, level: Level, seed: u64) -> Option<Vec<Action>> {
        let game = self.game.as_ref()?;
        if self.planned == Some(game.turns()) {
            return None;
        }
        let events = bot::plan(game, seat, level, seed)?;
        self.planned = Some(game.turns());
        Some(events.into_iter().filter_map(action).collect())
    }

    /// Where dead `seat` drops back in (Score), as SearchBot picks it: once until the log grows.
    /// `None` when it can't pick now.
    pub fn respawn(&mut self, seat: u8) -> Option<Action> {
        let (game, log) = (self.game.as_ref()?, self.log.as_ref()?);
        if self.picked == Some(log.events.len()) {
            return None;
        }
        let pick = bot::respawn(game, seat).and_then(action)?;
        self.picked = Some(log.events.len());
        Some(pick)
    }
}

/// The action a player sends for a planned event (the server stamps the times).
#[must_use]
pub fn action(event: Event) -> Option<Action> {
    match event {
        Event::Walk { facing, steps, .. } => Some(Action::Walk { facing, steps }),
        Event::Shoot { shot, aim, power, .. } => Some(Action::Shoot { shot, aim, power }),
        Event::Pass { .. } => Some(Action::Pass),
        Event::UseItem { slot, .. } => Some(Action::Item { slot }),
        Event::Spin { .. } => Some(Action::Spin),
        Event::Respawn { x, .. } => Some(Action::Respawn { x }),
        Event::Switch { .. } => Some(Action::Switch),
        Event::Forfeit { .. } | Event::Wind(_) | Event::Weather(_) | Event::Drop(_) | Event::Jewel(_) => None,
    }
}
