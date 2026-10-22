# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each release opens with a sentence on
why the version moved the way it did.

## [Unreleased]

### Added

- Workspace scaffold: `lobshot-sim`, `lobshot-server` (the `lobshot` binary, serving the client
  bundle and `/r/{code}` room links), a wasm32 Bevy client, `lobshot-lab`, and `cargo xtask dist`.
- The design (`DESIGN.md`), research notes (`docs/research/`) and the art spike's first content
  (`assets/`).
- The deterministic game core in `lobshot-sim`: GunBound units and a sourced tuning table, a
  carvable terrain mask, closed-form ballistics with wind, movement, delay-based turn order, damage,
  Coilback's three shots, and event logs replayed to the same state hash natively and in wasm32.
- Online 1v1: rooms by link (`POST /api/rooms`, `/r/{code}`, `/ws/{code}`) with seating, ready,
  reconnect, spectators, kicks, chat and emotes; weather (Wind, Force, Mirror, Tornado); SearchBot
  on the server and in an offline practice mode; the Bevy client with the full HUD, hold-space power
  and walk prediction.
- `lobshot-protocol` (the WebSocket messages, and the pacing that keeps turn clocks fair) and
  `lobshot-assets` (the generated asset layout and manifests).
- `cargo xtask art`: SVG to pivot-centered PNGs, terrain masks and map manifests, deterministic.
  Coilback, Pip and Tidewall A are finished.
- A browser smoke test (`cargo make browser`) driving headless Chrome against the real server.
- Sound effects (Kenney, CC0): shots, blasts, hits, falls, your turn, the clock's last seconds, the
  result, buttons and chat.
- Full Solo (M3): teams up to 4v4, all 11 items and the six-slot loadout, SS once every four of
  your own turns, sudden death (Big Bomb, Double, SS Death), the dead players' slot machine,
  shields, and an end-of-game scoreboard of damage, kills and bonuses.
- Four more mobiles: Lanternwing, Hookclaw, Sparkhorn and Marblemole, each with its own three
  shots, art and fitted gravity; Random picks one at the start.
- Four more maps (Gablewick, Glintvein, Rimewake, Ashspine) and Tidewall B, each map with two
  variants.
- SearchBot uses items and every shot, and its levels differ by how they misread the wind and miss.
- The client's unit tests (`cargo make test-client`), on wasm32 in node.
- Deployed at https://lobshot.twitchax.com: one Fly machine (`fly.toml`, `cargo make deploy`).
- The lab (M4): `lobshot-lab balance` plays every pair of mobiles 1v1 between SearchBots on every
  map and reports the win-rate matrix, damage per delay, turns to kill and hit rates, with a
  35-65% gate (`cargo make balance-gate`, not yet in CI); `lobshot-lab play` and `lobshot-lab mcp`
  let an LLM agent play a seat; `docs/playtests/` holds the report format.
- Six more weathers (M5): Electricity calls the Thunder item's bolt where a shot through it lands;
  Drain (GunBound's Weakness) halves shots through it; Lock bars items, stops shields and speeds SS
  up; Balm (GunBound's Moon) heals everyone a little each turn; Random hides one of the column
  weathers until a shot passes through it; Skyeye's satellite beams down wherever a projectile
  lands, and levels up with the damage it deals. GunBound Classic's Meteor is Wind, which lobshot
  has. Each map rolls weather by its GunBound archetype's odds.

### Changed

- Protocol and log wire versions are 3: logs from M2 and M3 builds don't load.
- The server, practice mode and the lab roll and seat games through one referee (`sim::referee`):
  practice now rolls its wind up to the room's max.
- SearchBot walks: when it has no good shot from where it stands, it tries a few spots within its
  move budget and walks to the one with the best shot.

### Fixed

- A shot's power above four bars no longer decodes, so a client can't fire one.
- A shot fired from inside a weather column is caught by it: a Mirror sends it back, Force boosts
  it, a Tornado flings it up. It used to pass through.
