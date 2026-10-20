# Research

The sources behind lobshot's rules and look, gathered on 2026-10-19. `sim::tuning` tags every
number with a source key (`[SW-mds]`, `[CR-mob]`, `[DB-mob]`, ...); the keys are defined in the
Sources section at the end of `mechanics.md`.

| File | What it holds |
|---|---|
| [`mechanics.md`](mechanics.md) | GunBound and DragonBound mechanics from public sources: delay, angles, power, wind, the roster, damage, items, movement, modes, art direction. Marks every contradiction and single-source claim. |
| [`openbound.md`](openbound.md) | How OpenBound (a C# GunBound remake) implements physics, movement, turn order and damage. GPLv3 code and ripped assets: use the numbers, never the code. |
| [`dragonbound-live.md`](dragonbound-live.md) | A live DragonBound game and its official guide: HUD layout, room options, timeout cost, the full mobile table. |
| [`physics-fit.md`](physics-fit.md) | How the ballistics were fitted to the community aiming formulas, which formulas are acceptance tests, and why the rest are not. |
| [`art-spike/`](art-spike/) | The SVG art spike that settled the art pipeline. Its sources are the first content in `assets/`. |

The fidelity rule (`DESIGN.md`, "Fidelity rules"): GunBound Classic + Thor's Hammer as documented
by Creedo's (Vexez) tables and StrategyWiki first, DragonBound's documented values where those
conflict or are silent, and a recorded pick where everything is silent.

Keys for the notes without a Sources section of their own:

- `[OB]` [`openbound.md`](openbound.md): OpenBound's code, read at `dev-2` `d11e8127`.
- `[DB-live]` [`dragonbound-live.md`](dragonbound-live.md): the live DragonBound game.
