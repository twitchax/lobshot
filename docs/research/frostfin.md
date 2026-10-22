# Frostfin (GunBound's Ice)

The sources behind Frostfin, the narwhal on an ice sled that carries GunBound Ice's behavior,
gathered on 2026-10-22 for PRD-0006 T-001. `mechanics.md` §3 holds the roster tables this builds on;
this file adds the Ice-specific pages and what they say. Raw pages fetched for it sit outside the
repo; the keys below say where each came from. Quotes are fragments; numbers are the sources' own.

## Source quality

- `[SW-mob]` (StrategyWiki's GunBound mobiles page) and `[CR-mob]` (Vexez's table) are the
  baseline, as everywhere in `docs/research/`. `[DB-mob]` copies `[SW-mob]`'s text.
- `[GF-ice]` is a whole Ice guide (Tolerance0, v2.1, December 2007): the most detail, and the
  latest. It reports the patch that weakened the SS, so it describes a later game than Classic.
- `[WP06]` (May 2006) and `[GF-lee]` (a 2003 guide) are Thor's Hammer era and earlier. `[GF-lee]`
  disagrees with every later source on three points (below); it is the oldest and loses each.
- `[OB]` implements Ice's shred in code with its own numbers; unverified against the game.

## Stats

| | Value | Sources |
|---|---|---|
| Type | Bionic, Physical ("Hit") | `[SW-mob]`, `[WP06]`, `[DB-mob]` |
| HP | 1200, the most of any mobile; no shield | `[SW-mob]`, `[CR-mob]`, `[GF-ice]`, `[OB]`. `[GF-lee]`: "Approximate HP: 1175" |
| Delay | base 490, S1 250, S2 400, SS 800, 10 a second | `[CR-mob]`, `[SW-mds]`, `[SW-mob]`, `[GF-ice]` |
| Aim | S1 and S2 70 to 20 (true 60 to 30); SS 60 to 30, all true | `[CR-mob]`, `[GF-ice]`, `[OB]` |
| Damage | S1 120 to 160; S2 150 to 170 "(base damage)"; SS 150 to 170 "(base damage)" | `[SW-mob]`, `[DB-mob]` |
| Average damage | S1 150, S2 190, SS 250 | `[GF-ice]`; `[GF-lee]` gives 180, 200+ and 250 |
| Shot weight | "Light" for all three | `[GF-ice]`; `[GF-lee]` says "Average" |
| Fixed power | 2.33 bars lands 60 degrees one screen | `[CR-fixed]` |

**CONTRADICTION:** the SS's damage. `[SW-mob]` gives it S2's range and says it "does a little more
damage"; `[WP06]`, "slightly more damage than Shot 2"; `[GF-ice]` and `[GF-lee]` average it at 250
against S2's 190 to 200.

## The shots

- **S1, an icicle from the cannon.** "A pointed large icicle that breaks when it hits land or a
  player. It removes a lot of land ... released from the cannon on the back" `[SW-mob]`. It "creates
  large, wide, oval holes" `[WP06]` and "rivals Trico's shot 1 in terms of total explosion area"
  `[GF-ice]`.
- **S2, a ball of ice from the trunk.** "Released from the trunk of the mammoth, making the point of
  release slightly lower. This sometimes affects the trajectory. Also, if you are leaning up on a
  wall, Shot 2 may get jammed in the wall even if you can do Shot 1 perfectly well" `[SW-mob]`.
  "A slightly lower firing position than its shot 1 (the trunk instead of the cannon)" `[WP06]`.
  "It is really only a problem when you're trying to fire out of a hole or a cliff, because
  otherwise the shot hits the exact same place as Shot 1" `[GF-ice]`. The ice "crumbles on impact"
  and "doesn't do that much bunge" `[GF-lee]`. **CONTRADICTION:** `[GF-lee]` puts the S2's firing
  point higher than S1's.
- **SS, a snowflake from the trunk.** "Exactly like shot 2, except it is a snowflake" `[SW-mob]`.
  It "turns into an enormous snowflake on contact with the ground or a mobile" `[WP06]`, "expands
  into a wide blasting radius on contact" and "uses the same firing point as weapon 2" `[GF-lee]`.
  `[GF-ice]`: "basically Shot 2 with an extra 400 delay"; "Super Shot should almost never be used".

## The defence shred

- **What it does.** Each S2 that hurts a mobile "decreases its resistance to Physical attacks by
  5%. You can reduce up to 50%" `[SW-mob]`. "Any damage results in 5-point reduction in defense that
  lasts for the duration of the game (even after respawn in Score mode). Maximum defense reduction
  is limited to 50 points on a single mobile" `[WP06]`. `[GF-ice]`: "reduces an enemies HIT TYPE
  defense by 5% each round", maximum "-50% DEF".
- **How it scales damage.** `[GF-ice]`'s worked example: three S2 hits, "their defense is reduced by
  15%. This means that your base damage 150 shot 1 should do around 175": a multiplier of about
  1.15, applied to the shot's damage.
- **Whose shots.** Only Physical ones: "Defense reduction only applies to Hit-type attacks, meaning
  Hit-types such as the NakMachine, Boomer, J.Frog, Turtle, and other Ice will deal more damage
  while other mobiles will still deal the same damage" `[WP06]`. Teammates' too: "This DOES cause
  your team mates, as well as enemies, to do more damage (assuming they are hit type)" `[GF-ice]`.
- **How long.** The whole game, Score respawns included `[WP06]`, `[GF-ice]`, `[GF-lee]`; per
  mobile, so in Tag "each bot has a separate counter" `[GF-ice]`.
- **What players see.** "A red -XX DEF counter is displayed to the right of their mobile that
  tracks the level of defense they have been reduced" `[GF-ice]`; ice "sticking and crumbling off the
  opponent's mobile" `[GF-lee]`.
- **The SS's share.** Patched down: "Traditionally, the SS did -20% defense reduction ... It also
  used to have a radius that would reduce defense for any mobiles in the area of effect. Now ... It
  only reduces 5% defense, and only if a mobile is directly damaged by it" `[GF-ice]`. `[WP06]`:
  "only decreases the defense of anything in the snowflake's radius by 5 instead of the previous 20".
  **CONTRADICTION:** whether the patched SS frosts everyone its blast hurts (`[WP06]`) or only a
  direct hit (`[GF-ice]`).
- **CONTRADICTIONS on the numbers:** `[OB]` lowers defence by 5 a hit but caps it at 35; `[GF-lee]`
  caps it at 20, and says the SS weakens a mobile to Electric attacks instead.

## The 70-degree power table

`[GF-ice]`'s "70 BJSL" table: the power that lands a 70-degree shot at each distance in zero wind,
in screens. Next to it, lobshot's Frostfin as fitted to `[CR-fixed]` (gravity 558.4 from the
cannon), from each barrel, against a Coilback target (`tool_fit` in `crates/sim/tests/formula.rs`
prints these).

| Distance | 1/8 | 1/4 | 1/2 | 3/4 | 1 | 1 1/2 | 2 |
|---|---|---|---|---|---|---|---|
| `[GF-ice]` | 0.90 | 1.35 | 1.90 | 2.30 | 2.70 | 3.30 | 3.90 |
| S1, the cannon | 0.83 | 1.28 | 1.89 | 2.34 | 2.72 | 3.36 | 3.89 |
| S2, the tusk | 0.72 | 1.22 | 1.85 | 2.31 | 2.70 | 3.34 | 3.87 |

- **In** (`formula_frostfin_seventy_degree_table`): 3/4, 1 and 2 screens, from either barrel, each
  within 0.05 bar. The table doesn't say which shot it was made with; the guide calls S2 "the bread
  and butter shot", and says S1 lands in the same place.
- **Out:** 1/2 and 1 1/2 screens, each within 0.05 from one barrel and just past it from the other
  (the tusk needs 0.055 less at 1/2, the cannon 0.06 more at 1 1/2); and 1/8 and 1/4 screen,
  where the model needs 0.07 to 0.18 less than the table, where the muzzle's offset from the mobile is a large share of the distance and the
  mobile's drawn geometry is lobshot's own, not GunBound's.
- The tusk lands about where the cannon does in the open, as `[GF-ice]` says: at 60 degrees one
  screen, both need `[CR-fixed]`'s 2.33 bars within 0.05 (`formula_frostfin_tusk_lands_where_its_cannon_does`).

## Sources

- `[GF-ice]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31845 (Tolerance0, "Ice/Mammoth
  Guide" v2.1, 2007-12-20; read via archive 20190526114147). The same FAQ as `mechanics.md`'s
  `GF-ice snippet`, read whole.
- `[SW-mob]` (`mechanics.md`), read again via archive 20250919061317 for its Ice section.
- `[WP06]` (`mechanics.md`), its Ice entry.
- `[GF-lee]` (`weather.md`), its section "f. Ice".
