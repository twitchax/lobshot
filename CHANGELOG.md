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
