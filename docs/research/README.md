# Research

The sources behind lobshot's rules and look, gathered on 2026-10-19. `sim::tuning` tags every
number with a source key (`[SW-mds]`, `[CR-mob]`, `[DB-mob]`, ...); the keys are defined in the
Sources sections at the end of `mechanics.md`, `weather.md` and each mobile's own note.

| File | What it holds |
|---|---|
| [`mechanics.md`](mechanics.md) | GunBound and DragonBound mechanics from public sources: delay, angles, power, wind, the roster, damage, items, movement, modes, art direction. Marks every contradiction and single-source claim. |
| [`weather.md`](weather.md) | Every GunBound weather under its three sets of names (Meteor is Wind), the per-map weather weights, and the rules for Electricity, Lock, Moon, Weakness, Random and Thor. Gathered on 2026-10-21. |
| [`spoutpot.md`](spoutpot.md) | Turtle (Spoutpot): StrategyWiki's entry and Creedo's Turtle guide, with its fork, skybomb and timebomb tables. |
| [`tuskrack.md`](tuskrack.md) | Bigfoot (Tuskrack): its spreads, ripples and bursts, and the inferred shot weight. |
| [`frostfin.md`](frostfin.md) | Ice (Frostfin): GF-ice's guide, the two muzzles, the defence shred and its 70-degree power table. |
| [`herdhound.md`](herdhound.md) | Kalsiddon (Herdhound): its guides and homing missiles, from Creedo's and the official guides. |
| [`thistleback.md`](thistleback.md) | Trico (Thistleback): its orbiting S2 and nine-blast SS, from the GunBound wiki and OB. |
| [`sudsotter.md`](sudsotter.md) | J.Frog (Sudsotter): Creedo's frog guide, its walking shots and its weather-proofing. |
| [`kitewyrm.md`](kitewyrm.md) | Dragon (Kitewyrm): what floating means, its mini-dragons, and why DBR's Dragon entry is unusable. |
| [`flarecrow.md`](flarecrow.md) | Aduka (Flarecrow): rear fire, its aftershock chain, flares calling Thor, and the GN-aduka power fit. |
| [`augerdillo.md`](augerdillo.md) | Nak (Augerdillo): the S2's mirrored bore, the SS that only stops in a mobile, its weak spot, and the community Nak formula fit. |
| [`lensowl.md`](lensowl.md) | A.Sate (Lensowl): the tracer, its satellite's heights and lasers, the S1's side bias and the SS sweep. |
| [`needlemouse.md`](needlemouse.md) | Knight (Needlemouse): its sword counts, where the swords come from, and its GunBound and DragonBound stats. |
| [`tinbadger.md`](tinbadger.md) | Raon (Tinbadger): when its mines crawl, whom they hunt, what sets them off, the walking SS, and every contradiction between the guides. |
| [`modes-score.md`](modes-score.md) | Score mode: team lives, what a death costs, and how the dead drop back in, with every pick. Gathered on 2026-10-24. |
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
