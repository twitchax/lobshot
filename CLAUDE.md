# lobshot: project instructions

Turn-based artillery in the browser, in the spirit of GunBound.

The global conventions in `~/CLAUDE.md` and the `bootstrap-rust-project` skill's `conventions.md`
apply in full. This file records only what is specific to lobshot; **where it is silent,
defer to `~/CLAUDE.md`.** Every design decision, and why, is in `DESIGN.md`; the sources behind
the numbers are in `docs/research/`.

## Architecture

A Cargo workspace; shared metadata, dependency versions and lints live once in the root
`Cargo.toml`.

| Path | Package | What |
|---|---|---|
| `crates/sim` | `lobshot-sim` | Pure, deterministic game rules, weather and SearchBot. No Bevy, no I/O, no clocks. Shares the `base` error aliases with every crate. |
| `crates/protocol` | `lobshot-protocol` | The WebSocket messages (postcard or JSON). The contract between server and clients. |
| `crates/assets` | `lobshot-assets` | The generated-asset layout and manifests (rigs, maps): written by xtask, read by server and client. |
| `crates/server` | `lobshot-server` | axum. The `lobshot` binary: serves `dist/`, `/r/{code}`, `POST /api/rooms`, `/ws/{code}`; one task per room (`room/` is a plain state machine, `room/task.rs` its async shell). |
| `crates/client` | `lobshot-client` | Bevy, **wasm32 only**, built by trunk. Outside `default-members`. Screens, the game world, HUD, input with walk prediction, and practice mode. |
| `crates/lab` | `lobshot-lab` | Headless games on `sim::referee`: SearchBot balance runs (`balance`, `game <id>`), and LLM agents' way in (`play`, `mcp`). |
| `xtask/` | `xtask` | `cargo xtask art` (SVG to PNG, masks, map manifests, sounds) and `cargo xtask dist` (trunk, then `.br`/`.gz` siblings). |
| `assets/` | | CC0 sources: art (SVG, RON) and sounds (Ogg Vorbis). `assets/gen/` is the build output (gitignored). |

## Commands

```bash
cargo make ci           # checks + the balance gate: the gate CI runs, as side-by-side jobs
cargo make checks       # fmt-check + clippy (native and wasm32, -D warnings) + nextest + client tests + determinism
cargo make test         # nextest (native crates)
cargo make test-client  # the client's unit tests: wasm32, in node, under wasm-bindgen's runner
cargo make determinism  # sim's tests on wasm32 under wasmtime: replays must match native bit for bit
cargo make art          # render assets/ into assets/gen/ (art, plus the sounds, checked)
cargo make dist         # the release client bundle in dist/ (art first)
cargo make dev          # a quick wasm-dev bundle, then the server on http://127.0.0.1:8080
cargo make browser      # the browser smoke tests: release bundle, headless Chrome, real server
cargo make uat          # ci + the #[ignore]d slow suites: what PRD UATs point at
cargo make codecov      # coverage.lcov
cargo make balance      # a balance run in release: every pair of mobiles, 1v1, every map (flags pass through)
cargo make balance-gate # Normal and Hard pooled, 200 games a matchup, failing outside 35-65%: part of ci
```

## Conventions

- **Toolchain:** pinned in `rust-toolchain.toml` (a dated nightly, plus the wasm32 target). Nothing
  else may pin a toolchain; `repo_sync` tests enforce it.
- **Lints:** the `[workspace.lints]` table in `Cargo.toml` (`all` + `pedantic` + `unused` +
  `unwrap_used` / `expect_used` denied). No new `allow`s beyond a narrow, commented one, and never
  for `too_many_*` or `needless_pass_by_value`: refactor instead. The one exception is
  `lobshot-client`, which allows `needless_pass_by_value` at its crate root because Bevy systems
  must take their parameters by value. `clippy.toml` lists the proper names `doc_markdown` accepts.
- **The client is wasm32 only.** Native tasks pass `--exclude lobshot-client`; `clippy-wasm` lints
  it (and `sim`, which must build for wasm32 too) for the browser. Its unit tests import
  `wasm_bindgen_test` as `test` (a plain `#[test]` never runs on wasm32) and run under
  `cargo make test-client`. Keep rules out of it: what the sim or protocol can decide (loadout
  legality, option values, SS state) lives there, with native tests. Try client changes in
  practice mode (title, "Practice vs bot"): it needs no server.
- **`sim` stays deterministic** (`DESIGN.md`, "Game model"): f64 with `libm`, no `mul_add`, no SIMD,
  no fast-math, no hash-order iteration, and randomness only from rolled events. What a roll
  ranges over, how a game is seated and how a turn is timed live in `sim::referee`, which every
  host (server, practice, lab) calls with its own dice: never put such a rule in one host.
  `crates/sim/clippy.toml` enforces the bans; the golden replays and `cargo make determinism` prove
  it. A change that moves a golden hash changes how recorded games play out: re-record with
  `tool_record` only when that is intended.
- **Every gameplay number lives in `sim::tuning`** with its source (`cited`, `fitted` or `picked`);
  cited keys must exist in `docs/research/` (`tests/sources.rs`).
- **Names:** code identifiers and player-facing strings use lobshot names (Coilback, Tidewall).
  GunBound names appear only in `sim::tuning` (source tags, `behaves_like`), in tests that check
  against GunBound sources, and in `docs/research/`.
- **Errors:** `anyhow` through the `Res` / `Void` aliases in `lobshot_sim::base`, with `.context()`
  on every `?` that crosses an I/O or parse boundary.
- **Tests:** unit tests in-module, integration and e2e in `tests/` (one subsystem per file, `e2e_*`
  names for tests that spawn the binary), fixtures from each crate's `tests` factory module. Every
  behavioral change ships a test.
- **PRDs** live in `.prds/`, one per milestone; reference task IDs in commits, and never mark a UAT
  verified without a real, passing test.
- **No AI attribution** anywhere: no `Co-Authored-By` trailers, session links or "generated with"
  footers in commits, PRs, changelogs or docs.
