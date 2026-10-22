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
- Spoutpot (M5), a hippo with a teapot water tank and hose cannons: S1 a water shot, S2 two
  streams that braid and join, SS a blob that bursts into six balls in flight.
- Tuskrack (M5), a walrus on a tank-sled with a fireworks rack: S1 four rockets rippling off the
  rack, S2 a bundle of grenades that bursts into three pairs, SS nine rockets. Climbing is now per
  mobile, and Tuskrack climbs poorly.
- Sudsotter (M5), an otter in a bathtub buggy: sponges that fly through mobiles, stick, and walk
  the ground (S1 left, S2 right), climbing walls, before they go off; SS suds that hurt whoever
  they touch as they walk. The weather leaves its shots' damage alone.
- Frostfin (M5), a narwhal on an ice sled with the most HP of any mobile: S1 an icicle from the
  cannon on its back that digs a wide hole; S2 and SS ice from its tusk, a second, lower barrel,
  each hit leaving its target 5% more brittle to Physical damage, up to 50%, all game.
- Clockrise (M5), a toy clocktower town on the tall-tower archetype: A one cuckoo-clock tower
  whose storeys float over open works, B twin towers joined by a high bridge.
- Tumblehill (M5), a sunny training ground on the practice-slope archetype: A two broad hills
  over a valley, B a long slope down to a pond.
- Herdhound (M5), a sheepdog mech: each shot is a warhead that opens at the top of its arc into a
  whistle that flies on and missiles that loop, then home on it (S1 two, S2 four, SS four that
  split into eight, each with its own whistle).
- Neonbay (M5), a harbor city at night on the city archetype: A downtown towers with rooftop neon,
  B the docks, with container stacks and a gantry crane over the slip.
- Rootmere (M5), a giant tree's roots over a marsh at dusk on the root-tangle archetype: A low
  arches over wide hollows, B a higher lattice on stilts with a bridge that breaks.
- Thistleback (M5), a hedgehog gardener with a wheelbarrow catapult: S1 a heavy cabbage, S2 a
  cabbage with two sprouts circling it, SS a pumpkin that bursts nine times in a ring and hurts
  most when it lands at a target's feet.
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
