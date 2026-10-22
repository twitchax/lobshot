# lobshot: design

Turn-based artillery in the browser, in the spirit of GunBound. Rust + Bevy, browser first. Every
decision below was settled with Aaron on 2026-10-19 over five rounds of questions. The research the
numbers come from is in `docs/research/`.

## Scope

- **Audience:** friends now, public later. The art bar is high from day one; abuse features
  (filters, bans) come later. Room creation and every message go through one rate-limited path, so
  those features are cheap to add.
- **Fidelity:** GunBound Classic + Thor's Hammer, true to GunBound: every mobile, all 11 items, all
  modes (Solo, Score, Tag, Jewel), every weather, all sudden-death types, the dead-player slot
  machine. Original names and art throughout.
- **Everything is available to everyone.** No accounts, ranks, gold, shop or avatar stats.
  Knight and Dragon are directly selectable (GunBound made them Random-only); Random stays.
  Cosmetics (riders) are all unlocked and remembered in `localStorage`.
- **Browser first.** The client is Bevy compiled to WASM; a native build can come later.

## Fidelity rules

- **Baseline:** GunBound Classic + Thor's Hammer as documented in Creedo's (Vexez) tables and
  StrategyWiki. Where those conflict or are silent, take DragonBound's documented value. Where every
  source is silent, pick a value and record the pick.
- **One table:** every number lives in `sim::tuning`, each tagged with its source key (keys are
  defined in `docs/research/`). Balance work edits one file.
- **GunBound units.** One screen is 800 px; the view is 4:3 (800×600 logical); the power bar runs
  0–4.0 bars in 0.05 steps. Gravity and wind constants are fitted per mobile so community aiming
  formulas hold: they become acceptance tests, within ±0.05 bar. The tables disagree with each
  other in places, so the tests are their consistent subset; `docs/research/physics-fit.md` lists
  what is in and out, and why. A wide "expand view" can come later.
- **Wind** changes only on the Wind weather. Max wind is a room option (0, 12, 26).

## IP and licensing

- Mechanics are free (17 USC §102(b)); expression is not (*Tetris Holding v. Xio*, 2012). Never use
  GunBound's names (product, mobiles, maps), sprites, sounds, map silhouettes or HUD art. Functional
  HUD conventions (wind dial, aim arc, power bar, turn list) are genre-standard and fine. "In the
  spirit of GunBound" in a description is a factual reference; never use it as a brand.
- Code identifiers and player-facing strings use lobshot names. GunBound names appear only in
  `sim::tuning` source tags and `docs/research/`. Items keep GunBound's plain words (Dual, Dual+,
  Teleport, Power Up, Thunder) or take plain lobshot ones (Sacrifice, Dig, Heal, Big Heal, Wind
  Flip, Swap).
- Never port OpenBound's code (GPLv3) or use its assets (ripped from GunBound). Its numbers and
  physics model are documented in `docs/research/openbound.md`; reimplement from those.
- **MIT for code, CC0 for `assets/`** (its own LICENSE file). Any future commission must release its
  work under CC0.
- The name also belongs to a 2023 remote-access malware family. Accepted.

## Architecture

- **One binary serves everything:** the client bundle, the WebSocket and the rooms. Same origin, so
  no mixed content and no CORS; TLS comes from whatever fronts it. Every URL is relative.
- **Self-hosting** is running the same binary or image. No in-game "host" button: a browser cannot
  accept connections.

Each crate lives in `crates/<name>` as package `lobshot-<name>`; the server's binary is `lobshot`.

| Crate | Owns |
|---|---|
| `sim` | Pure, deterministic game rules, no Bevy: tuning table, terrain mask, trajectories, wind, weather, mobile shot behaviors, items, damage, delay turn order, movement, modes, and SearchBot. |
| `protocol` | The WebSocket messages between server and clients (browsers, bots, LLM agents): postcard with a version byte, or JSON. |
| `assets` | The generated-asset layout and manifests (rigs, maps), shared by `cargo xtask art` (writer) and the server and client (readers). |
| `server` | axum + tokio. Serves `dist/`, the WebSocket and the room registry. One tokio task per room owns that room's `sim`; messages arrive on an mpsc, and each connection has its own outbound channel (team chat is per-recipient). |
| `client` | Bevy, WASM first, built with trunk. Rendering, input, HUD, audio, local movement prediction. Networking via `ewebsock`. |
| `lab` | Headless games on the server's referee: SearchBot balance runs (`lobshot-lab balance`, with the win-rate gate), and the JSON and MCP interface for LLM agents (`play`, `mcp`). |
| `xtask` (at `xtask/`) | `cargo xtask dist` (the client bundle) and `cargo xtask art`. |

## Game model

- **Deterministic `sim`.** f64 with trig from the pure-Rust `libm` crate. `mul_add`, SIMD and
  fast-math are banned through clippy `disallowed_methods`. A CI test runs the same recorded game
  natively and under `wasm32` and compares state hashes.
- **The server owns randomness.** Wind rerolls, the weather queue, spawns, starting order, slot
  machine spins and Random picks are rolled by the server and enter the log as events. The sim
  says when a roll is owed (`Game::pending_roll`) and refuses moves until it arrives, so every log
  has its rolls at the same points. A client's
  `sim` is a pure function of the event log, and no client can read future wind from a seed.
- **One referee.** What each roll ranges over, how a game is seated (spawns, Random picks, turn
  order) and how a turn's time is stamped live in `sim::referee`; every host (the server, practice
  mode, the lab) brings its own dice to it, so a game plays by the same rules wherever it runs.
- **A game is its event log:** room options, accepted intents and server rolls. That is the replay
  format, the golden-test format and what the server broadcasts. No replay UI in v1.
- **Server authoritative.** The active player sends intents (move, angle, power, shot, item). The
  server validates them against its `sim` and broadcasts the accepted ones; every client replays
  them. Per-turn state hashes catch desync; a full snapshot repairs it.
- **Damage** hits a mobile's shield first, then its HP; shields regenerate at the start of their
  owner's turn.
- **Movement** is integer pixel stepping (climb, drop and step budget per mobile, in the tuning
  table). The active player's client predicts each step with the shared `sim` and streams inputs
  with a tick; the server replays and broadcasts them. Other players see the steps, smoothed.
- **Terrain** is a per-pixel mask, not a physics body. Explosions carve it; collision samples it;
  the client re-uploads only the dirty rect of the Bevy `Image`.
- **Shots** follow a closed-form trajectory under gravity and wind acceleration, sampled at fixed
  sub-steps. Each mobile's shots are behavior functions in `sim`.
- **Weather** is GunBound's moondisk: five forecasts, the front one coming up every N turns (N =
  players). Each map rolls them by the weights of the GunBound map whose archetype it is built on
  (GF-vgg, `sim::tuning::weather`); Mirror, Drain and Random, which came later, weigh 1. Force, Drain, Electricity, Mirror and Tornado stand as full-height columns where the
  server rolled them; repeats of a kind stack, a new kind clears them. A shot that starts inside a
  column is caught by it too. Force adds 33% damage (GF-tiro); Drain (GunBound's Weakness) halves
  it (WP08); Electricity calls the Thunder item's bolt down where the shot lands, once per
  projectile (the item is the weather, GF-tiro); Mirror reverses a shot's sideways motion and wind
  (OB); Tornado holds a shot a second, then flings it back up (a pick: sources only say it orbits,
  then exits). Wind rerolls the wind and clears the columns; GunBound Classic calls it Meteor
  (`docs/research/weather.md`). Lock and Balm (GunBound's Item Lock and Moon) clear the columns
  and hold over the whole map until the next weather: under Lock no one may use an item, shields
  stop regenerating and the SS lock counts down twice a turn (WP06); under Balm each player heals
  3% of max HP as their turn starts, Bionic mobiles 6% (NAMU, WP06). A Random column (80 px, OB)
  hides one of the five column weathers, rolled with its forecast: a shot passing through meets
  what it hides, and the column shows it from then on. SearchBot and the lab's trace plan as if a
  hidden column weren't there (`Game::as_seen`). Skyeye (GunBound's Thor) is a satellite over
  the middle of the map, as high as bolts start: while it is up, each projectile that lands calls
  its beam toward the landing, and the first mobile or ground in the beam's way takes it (radius 25,
  OB). It levels up with the damage its beams deal, all game (GF-lee and GF-vgg's thresholds); a
  level 1 beam deals 50 to 100 (CR-aduka), scaled by GF-vgg's averages to 100 to 200 at level 6.
- **Turn order** is GunBound's delay system: base + shot + item + 10 per second of turn time
  (Turtle 12). The lowest accumulated delay moves next.

## Protocol

- `POST /api/rooms` creates a room and answers `{"code"}`; `GET /ws/{code}` is its WebSocket.
- postcard with a leading version byte; a JSON form of the same serde types for debugging and LLM
  agents (the server answers in the form the client last used). The log format is a long-term
  commitment. JSON numbers lose precision past 2^53 in JavaScript, so a JS agent should treat
  state hashes as opaque.
- The game travels as its log: `Game { log }` on start or on (re)joining, then each accepted
  `Event` with the state hash after it, so a client that diverged knows at once.
- **Pacing:** the sim resolves a shot at once, but clients show it over seconds (flight, blasts,
  falls). Both follow one schedule (`lobshot_protocol::pacing`), and the server starts the next
  turn's clock, and a bot's think delay, only once clients have finished showing it, so watching a
  shot never eats into the next player's turn. An event plays in stages (a Dual's two volleys, a
  drop landing), each once the one before it is done, and scenes play one after another, so a
  dead player's spin mid-turn delays what follows it, never the turn already running.
- Heartbeat every ~20 s: a player lining up a shot sends nothing, and proxies drop idle sockets.

## Rooms

- **No lobbies.** No matchmaking, accounts, server list or database. "New game" mints an
  unguessable code; the link `/r/<code>` is the invitation.
- **Joining:** type a name once; name, rider and reconnect token live in `localStorage`. The
  pre-game screen (mobile, team, loadout, ready) is inside the room.
- **Room master:** the creator, then the oldest remaining player. Sets mode, map, team size (1v1 to
  4v4), max wind, turn time, sudden-death type and turn, random teams. Adds bots. Starts the game
  once everyone is ready.
- **Seating:** a newcomer takes a seat on the smaller team if it has room, and otherwise watches;
  everyone seated, the master too, readies before the start.
- **Late joiners** spectate. A reconnect token rejoins its seat.
- **Kicks:** the master kicks; the token is revoked and the seat freed. Mid-game the mobile dies, as
  a leaver's would. The player can rejoin through the link as a spectator, or as a new player before
  the next game. No IP bans.
- **Chat:** All and Team, plus emotes. Spectators see All.
- In memory only. An empty room dies after 10 minutes. A restart or deploy drops games in progress.

## Bots and the lab

- `sim` defines a controller: given what a player can see, return intents. Bots see only what a
  player sees, with no oracle wind.
- **Human:** a remote player on the server.
- **SearchBot:** searches angle, power, shot and item in `sim`, scored by expected damage. When
  nothing good reaches from where it stands, it searches a few spots within its move budget too
  and walks to the best. Its levels differ by how badly they read the wind and miss the aim and power they found
  (`sim::tuning::bot`): Hard is exact, Normal hurts an enemy with about two shots in three. It
  fills seats in real rooms. Runs on `spawn_blocking` with a ~1 s budget per
  turn so a room of bots cannot starve the others.
- **ExternalAgent:** an LLM playing one seat against SearchBot, headless in the lab, for qualitative
  playtesting (`docs/playtests/`). It sees what a player sees (players, wind, weather, the terrain as
  heights and a text picture), may trace where a shot would land in the wind as it is (what
  SearchBot's search knows, nothing more), and acts with the wire's own `Action`. Its turns are
  charged the bot think time, so a slow model plays on a bot's clock. It plays through
  `lobshot-lab play` (a step a process over a save file) or `lobshot-lab mcp` (the same as MCP
  tools over stdio, for any MCP client; the lab holds no API key). The MCP server is a plain
  JSON-RPC loop of four methods, synchronous like the rest of the lab.
- **`lab`** hosts games exactly as the server does (`sim::referee`, the bot loadout, the default
  room rules, the server's bot think time) and runs thousands of 1v1 games between SearchBots:
  every pair of mobiles on every map variant, each seed played twice with the seats swapped. It
  reports the win-rate matrix, damage per 1000 delay (by shot too), turns to kill, hit rates, a
  by-arena table and the first mover's edge, as text or JSON; any game replays by its id. Results
  depend on the bots' level (Normal misreads the wind, which costs the most wind-sensitive mobiles
  most), so read Normal and Hard together. Once the roster passes ~5 mobiles, CI fails any matchup
  outside a 35–65% win-rate band (`cargo make balance-gate`).

## Client

- **Bevy 0.20, pinned,** `default-features = false`, 2D features only, WebGL2. Upgrade only when an
  ecosystem crate forces it.
- **Bevy UI for everything,** plus a thin DOM `<input>` overlay for chat and name entry, because
  IME, paste and phone keyboards are poor inside a canvas.
- **Input:** keyboard and mouse. Hold-space shooting is required, for difficulty. The HUD is laid out
  so touch can come later.
- **HUD** uses GunBound's functional layout: wind dial top center, aim arc with the true-angle
  wedge, power bar with last turn's marker, turn list with relative delays bottom left, S1/S2/SS,
  item slots, weather queue, movement bar.
- **View:** GunBound's 800×600, letterboxed to 4:3 in any window, so a "screen" stays a ruler. The
  camera follows the active mobile, and a shell in flight.
- **Practice:** a game against SearchBot with no server, the client refereeing itself through
  `sim::referee` (rolls, turn times, the bot's turns). Also how the client is tested without a server.
- **Dev loop:** `cargo make dev` builds a `wasm-dev` bundle (Bevy optimized, no debug info; ~30 s
  rebuilds) and serves it. The release bundle (`cargo make dist`) takes minutes.
- **Audio:** CC0 sound effects (Kenney, Freesound's CC0 filter) through Bevy's audio, Ogg Vorbis
  only. `assets/sounds/` holds one file per `lobshot_assets::Sound`, credited in its `CREDITS.md`;
  `cargo xtask art` checks each decodes and ships it. Shots, blasts, hits and falls sound on the
  scene's pacing schedule, with what they show. The page resumes audio on the first click or key
  press, since browsers start it suspended. Music later, CC0 or AI.

## Art

- **Style:** original designs in the GunBound family. Chibi riders on chunky, toy-like mechs; thick
  dark outlines; flat 2–3 tone cel shading; soft backgrounds behind high-contrast terrain.
- **Pipeline: SVG throughout,** written as text by an LLM agent and art-directed by Aaron. A shared palette and
  stroke widths keep the roster on-model; team colors and recolors are palette swaps. Backgrounds
  are the designated upgrade slot (AI raster or commission) if a painterly look is ever wanted,
  since nothing depends on them.
- **`cargo xtask art`** rasterizes SVG with the `resvg` crate at 2× into gitignored build output.
  Golden replay tests pin each collision mask's hash, so a renderer change that moves collision is
  caught.
- **Mobiles are cutout rigs:** part SVGs plus a RON file (pivots, draw order, which parts spin or
  aim, the rider socket). Bevy animates them: wheels spin, the cannon follows the angle, the body
  tilts to the terrain, it squashes on firing. Gameplay geometry (hitbox, muzzle offset, aim range,
  ground probes) belongs to `sim::tuning`; a test checks the rig agrees.
- **Riders** are cosmetic and swappable. Neither they nor thin decoration that sticks out (Coilback's
  antennae) is hittable: the hitbox is the body, so the target size stays the balance number.
- **Maps:** 10 original maps, each built on one GunBound map's archetype (mine corridors, root
  tangle, town rooftops, tall tower, floating islands, sparse starfield, long flat sea, practice
  slope, dragon's-back ridge, city), each with A/B variants. A map is terrain geometry that produces
  both the art and the collision mask (the mask comes from the land path only), a background SVG,
  and a RON file (spawn points; Skyeye needs none, as it hovers over the middle).
- **Effects** (explosions, trails, particles, weather) are Bevy code, not art files.
- **The spike's output is the first content:** Coilback (a snail-shell bell-mortar) carries Armor's
  behavior, Tidewall is the floating-islands map, Pip is the first rider. The spike's record is in
  `docs/research/art-spike/`; M2 fixed its flaws and replaced its scripts with `cargo xtask art`,
  whose review sheets land in `target/art-preview/`. Expect ~1–2 hours of agent time per
  additional mobile, plus review.
- **M3 added** Lanternwing (a moth mech, Mage's behavior), Hookclaw (a hermit crab, Boomer's),
  Sparkhorn (a rhino beetle, Lightning's) and Marblemole (a mole in a mine cart, Grub's), the item
  icons and slot-machine drops, and four maps: Gablewick (town rooftops), Glintvein (mine
  corridors), Rimewake (long flat sea) and Ashspine (dragon's-back ridge), plus Tidewall B.
- **M5 adds**, from Aaron's concepts: Spoutpot (a hippo with a teapot water tank, Turtle's
  behavior) and Tuskrack (a walrus on a tank-sled with a fireworks rack, Bigfoot's).

## Testing

- Unit tests in each crate. The client builds only for the browser, so its tests run on wasm32 in
  node under wasm-bindgen's test runner (`cargo make test-client`).
- Physics: the GunBound-formula acceptance tests.
- Replays: golden tests where a recorded log must reproduce the same final state hash, plus the
  native-vs-wasm hash test.
- Server: end-to-end tests that spawn the real binary in a `TempDir` and drive full games with fake
  WebSocket clients and bots.
- Browser: smoke tests online as a spectator (page loads, canvas renders, socket connects, a game
  reaches turn 2) and in practice (clicks into a game, the turn runs out; an item armed, SS
  locked), driving the installed Chrome over CDP from Rust (`headless_chrome`): no browser
  download. The client publishes its state, and where its buttons are, as `window.lobshot` for
  them. The first two check that each turn's clock starts only once the shot before it has shown.
- Balance: the `lab` win-rate gate, once the roster passes ~5 mobiles.

## Performance goals

Measured and reported in each PRD. They are goals, not CI gates; going over one is a conversation,
not a failure.

- Latest desktop Chrome, Firefox and Safari. Phones best effort until touch lands.
- First load (WASM plus the first map) around 10 MB or less with brotli; other maps load lazily.
- 60 fps on a five-year-old laptop.
- One shared-cpu Fly machine.

## Deployment

At the end of M3, when a complete Solo game exists. `fly.toml` holds the app's settings.

- **Container:** multi-stage Docker. Build stage: cargo-chef dependency caching, `cargo xtask art`,
  `cargo xtask dist` (trunk with `wasm-opt`, then brotli and gzip siblings the server picks by
  `Accept-Encoding`), the server binary. Runtime stage: a slim image
  with the binary and `dist/`. Self-hosting is `docker run -p 8080:8080`.
- **Exactly one Fly machine,** app `twitchax-lobshot`. Rooms live in memory, so two machines would
  split a room's players. `fly launch` creates two by default: deploy with `--ha=false` or
  `fly scale count 1`. Scale-out later means `fly-replay` keyed by room code.
- **DNS:** `twitchax.com` is on Cloudflare. `fly certs add lobshot.twitchax.com`, then a
  **DNS-only (grey cloud)** CNAME to `twitchax-lobshot.fly.dev`. The orange-cloud proxy breaks Fly's
  Let's Encrypt issuance unless the `_acme-challenge` CNAME Fly prints is also added.
- Open sockets keep Fly's auto-stop from killing a live game; with nobody connected the machine
  stops, so cost rounds to near zero.
- **Deploys kill live games.** Deploy when nobody is playing.

## Milestones

One PRD per milestone in `.prds/`.

- **M0 Bootstrap.** The reshaped workspace; CI green; Docker builds; the server serves an empty Bevy
  canvas built by trunk. Research and the art spike move into the repo.
- **M1 `sim` core.** Tuning table and GunBound units; terrain mask and carving; trajectory with
  gravity and wind plus the formula acceptance tests; delay turn order; pixel movement; damage
  (hitbox, ranges, armor against weapon type, true and false angle); bunge; the event log, replay
  golden tests and the cross-target hash test; Coilback's S1, S2 and SS (Armor's).
- **M2 Online 1v1.** Rooms by link and the protocol; rendering with the full HUD and hold-space
  power; movement prediction; the weather engine (Wind, Tornado, Force, Mirror); chat, spectators,
  reconnect, kick; SearchBot v0; `cargo xtask art` with Coilback, Pip and Tidewall; sound effects.
- **M3 Full Solo.** 2v2 to 4v4; all 11 items and the 6-slot loadout; SS gating; all four
  sudden-death types; the slot machine; five starter mobiles (Armor, Mage, Boomer, Lightning and
  Grub behaviors); the bonus scoreboard; five maps. **Deployed** to `lobshot.twitchax.com`.
- **M4 Lab.** Headless balance runs, the win-rate matrix and the CI band gate; the JSON/MCP
  interface for LLM agents.
- **M5 Full roster.** The remaining 13 mobiles (Raon's mines, Aduka with Thor, JD's pull and push,
  Nak's digging, Knight, Dragon and the rest); the remaining weathers; the remaining five maps.
- **M6 Modes.** Score, Tag and Jewel.

## Risks

- **Movement feel** on pixel terrain (climbing, sliding, falling off edges) is the main feel risk.
  The numbers come from OpenBound's reading of GunBound; test it early in M2.
- **Content is the long pole,** not the code: 18 mobiles, 20 map variants, balance.
- **Bevy churn:** a breaking release roughly every four months.

## Considered and rejected

- **lightyear / bevy_replicon:** built for real-time games. Turn-based needs an event log, not
  replicated ECS state.
- **Cloudflare Durable Objects:** the server would have to build for two hosts or drop the
  self-host binary. A Fly container instead.
- **WebRTC peer-to-peer with a browser host (matchbox):** still needs signaling and TURN, and the
  game dies when the host closes the tab.
- **Macroquad:** a smaller bundle, but a hand-rolled UI, scene and effect timing, and a thin
  ecosystem.
- **Fixed-point math:** deterministic by construction, but much uglier, especially trig. f64 with
  `libm` and a cross-target test instead.
- **Shipping trajectory keyframes from a non-deterministic server:** gives up replays, golden tests
  and the bots' search.
- **HTML screens over the canvas:** two UI systems and two styles.
- **AI raster art throughout:** loses clean cutout parts and roster consistency.
- **Progression** (ranks, gold, avatar stats): conflicts with no accounts and no database.
- **Discord integration:** dropped.

## Prior art

- [DragonBound](https://dragonbound.net/): the browser GunBound clone this is reacting to.
- [OpenBound](https://github.com/WickedPeanuts/OpenBound): a C# GunBound remake; numbers only (see
  above).
- [DTerrain](https://github.com/Ideefixze/DTerrain/) and [Iron Warriors](https://github.com/WildRikku/gdtvjam24):
  destructible terrain in a jam artillery game.
- [castle-game](https://rust-digger.code-maven.com/crates/castle-game): Rust pixel-terrain game.
