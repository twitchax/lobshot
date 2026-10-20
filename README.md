# lobshot

[![Build and Test](https://github.com/twitchax/lobshot/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/lobshot/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/lobshot/branch/main/graph/badge.svg)](https://codecov.io/gh/twitchax/lobshot)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Turn-based artillery in the browser, in the spirit of GunBound: chunky mobiles, wind, delay-based
turn order and terrain you blow apart. No accounts and no lobbies: start a game, send the link.

![Coilback on Tidewall, in the browser](docs/screenshots/tidewall.png)

Online 1v1 works: start a room, send the link, play Coilback on Tidewall against a friend or a
bot, with wind, weather, chat and spectators. "Practice vs bot" plays offline. More mobiles, maps,
items and team games come next: [`DESIGN.md`](DESIGN.md) has the plan and the milestones, and
`.prds/` tracks each one.

## Self-hosting

One binary serves the client, the WebSocket and the rooms, so self-hosting is running the image:

```bash
docker run --rm -p 8080:8080 ghcr.io/twitchax/lobshot:latest
```

Then open http://localhost:8080. Rooms live in memory: a restart ends the games in progress.

## Development

```bash
cargo make dev       # a quick client bundle, then serve it on http://127.0.0.1:8080
cargo make art       # render the SVG art in assets/ (review sheets land in target/art-preview/)
cargo make dist      # the release bundle in dist/, precompressed
cargo make ci        # fmt-check + clippy (native and wasm32) + nextest + wasm32 replays: the gate CI runs
cargo make browser   # the browser smoke test: the release bundle in headless Chrome
cargo make uat       # ci + the release bundle + the #[ignore]d slow suites, the browser test included
```

Needs the toolchain from `rust-toolchain.toml` (rustup installs it), plus trunk and wasmtime
(`cargo make tools`). The browser test needs Chrome or Chromium installed.

## License

Code: [MIT](LICENSE). Art and other files in `assets/`: [CC0](assets/LICENSE).

lobshot is not affiliated with GunBound, Softnyx or DragonBound.
