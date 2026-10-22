# Playtest: three 1v1 games against SearchBot over the lab CLI (Hookclaw, Lanternwing, Sparkhorn)

- **Date:** 2026-10-21
- **Player:** an agent over the CLI
- **Build:** 2adc37e
- **Games:**
  - Hookclaw vs Marblemole (Normal), ashspine, seed 11: won on turn 17 (the view's counter), took no damage.
  - Lanternwing vs Coilback (Normal), glintvein, seed 5: won on turn 9, took no damage.
  - Sparkhorn vs Hookclaw (Hard), gablewick, seed 23: lost on turn 10.

## Findings

### No Marblemole shot can reach the other ashspine spawn over the central spire

- **Kind:** balance
- **Severity:** high
- **What happened:** In game 1 the Normal Marblemole fired nine S1s into the spire's face and never
  hurt me. To check, I took Marblemole's seat in the mirrored setup and traced every integer aim
  from 30 to 60 at every power from 1 to 80, for S1, S2 and SS. No shot reached Hookclaw. Every 60°
  shot hits the spire at about x=1017. Walking 90 px back to x=1185 didn't help either (powers in
  steps of 2). The wind stayed 15 toward 92° all game, so nothing changed. Hookclaw, which can aim
  up to 90°, hit with S1 at 80°, power 30 seven times in a row.
- **Replay:** `--mobile marblemole --against hookclaw --map ashspine --seed 11`, turn 0. The game I
  played was `--mobile hookclaw --against marblemole --map ashspine --seed 11`, bot turns 1 to 15.
- **Suggestion:** check the map's spawn pairs against each mobile's aim range. A lower spire, or a
  spawn farther back, would give 60°-capped mobiles a way over.

### A Mirror lets through shots fired from within about 60 px of it

- **Kind:** bug
- **Severity:** medium
- **What happened:** In game 2 (turn 3), a Mirror at x=1072 reflected every leftward shot I fired
  from x=1340, 268 px away, as the rules describe. In game 3 (turn 6) the Mirror stood at x=1580.
  Rightward S1s fired from x=1540 or x=1520 went straight through it and landed at x=1638 to 1900.
  The same traces from x=1500 or x=1480 were sent back. From x=1630, shooting left, a Dual S2 went
  through the Mirror and hit the enemy for 318. I fired that shot for real in a replay, and the
  result matched the trace.
- **Replay:** `--mobile sparkhorn --against hookclaw --map gablewick --seed 23 --level hard`. Play S2
  30°/16, then S2 25°/14, then Big Heal (slot 3) with S2 25°/14. That reaches turn 6 with the
  Mirror at x=1580. Trace `--shot s1 --aim 25 --power 40 --facing right` from x=1540, then walk
  left 40 px to x=1500 and run the same trace.

### `trace --facing` describes a shot the next `Shoot` won't fire

- **Kind:** interface
- **Severity:** medium
- **What happened:** In game 3, turn 6, I walked right, which turned me to face right. I then ran
  `trace --facing left` with Dual armed and got "Hits the enemy for 318". The `Shoot` action has
  no facing, so it fired right into the wall next to me: I took 89 damage and the enemy took none.
  That probably cost the game. Nothing in the trace mentions that it assumed a different facing,
  and the act's log ("You fired S2 at 32 degrees, power 19") doesn't say which way the shot went.
- **Replay:** game 3's setup and seed (above), turn 6: walk right 90 px, then compare
  `trace --shot s2 --aim 32 --power 19 --facing left` with
  `act '{"Shoot":{"shot":"S2","aim":32,"power":19}}'`.
- **Suggestion:** let `Shoot` take an optional facing, or have the trace and the act say which way
  they fire. A trace whose `--facing` differs from your current facing could also warn.

### Normal bots landed nothing; the Hard bot hit for 300 to 660 nearly every turn

- **Kind:** feel
- **Severity:** medium
- **What happened:** Across games 1 and 2, the Normal bots fired 12 shots and did no damage. Game
  1's misses are explained by the spire finding above. Coilback's three shots in game 2 (turns 2, 4
  and 7) all missed. Neither Normal bot ever walked, and Marblemole never used SS, Dual or Power Up.
  The Hard Hookclaw moved before my first turn: it walked, used Dual and fired S2. Eight hits took
  all 300 of my shield and 196 HP. After that it landed 400, 300 and 661 (an SS) on three of its
  four later turns.
- **Replay:** game 2 (`--mobile lanternwing --against coilback --map glintvein --seed 5`), turns
  2, 4 and 7. Game 3, the opening view's `happened`.

### Power Up didn't raise an S2 hit that came from a side bolt

- **Kind:** bug
- **Severity:** low
- **What happened:** In game 3, turn 8, a Tornado stood at x=1403 between us. Traced from x=1630, S2
  at 40°, power 14 hit the enemy for 76 with or without Power Up armed. The ball landed at x=1439,
  and the damage came from a blast at x=1341. On the same turn, Power Up raised SS from 160 to 213.
  Earlier it raised a direct S2 hit from 197 to 237. The rules say Power Up makes "this turn's
  shot" hit harder.
- **Replay:** game 3, turn 8 (the Tornado stands at x=1403): `trace --shot s2 --aim 40 --power 14`,
  with and without `--item 1`.

### The log says "You's shield"

- **Kind:** interface
- **Severity:** low
- **What happened:** Every hit on my shield in game 3 read "You's shield took 62.", starting with
  the opening view.
- **Replay:** game 3, the opening view's `happened`.

### The trace summary can point at the wrong blast, and calls leaving the map "landing"

- **Kind:** interface
- **Severity:** low
- **What happened:** In game 1, turn 0, an S2 trace said "Hits the enemy for 42. Lands near x=1037,
  y=348." That point is a boomerang hitting the spire on my side; the hit came from a different end,
  at (724, 449). Shots that leave the map read "Lands near x=0, y=-423" or "x=1900, y=212".
- **Replay:** game 1's setup, turn 0: `trace --shot s2 --aim 78 --power 28`.
- **Suggestion:** name the landing point that did the damage, and say "leaves the map" for exits.

### Heal amounts aren't stated, and Big Heal varied between mobiles with the same HP

- **Kind:** interface
- **Severity:** low
- **What happened:** The rules say only "a small heal" and "a big heal". Big Heal healed 300 for
  Marblemole (game 1, turn 6) and Hookclaw (game 3, turn 9), but 250 for Coilback (game 2, turn 7).
  All three have 1000 HP. It healed 187 for my 750-HP Sparkhorn (game 3, turn 4), and Heal healed
  169 both times I saw it.
- **Replay:** the three games' setups and seeds, at those turns.

### A few gaps in the view

- **Kind:** interface
- **Severity:** low
- **What happened:**
  - The picture draws weather columns as `|`, which no legend mentions, and every weather kind
    uses the same glyph.
  - On glintvein, `surface` reads 0 across x=750 to 1250, the top of the overhang. The floor I
    stood on there is at y≈720.
  - A pass's delay isn't listed anywhere. Mine cost 712 (game 2, turn 3).
  - The log gives where opponents aimed but not where their shots landed.
- **Replay:** game 2's setup and seed, turns 0 to 3.

## Notes

- The trace matched the result exactly on every shot I fired: 15 shots across the three games, plus
  the replay. Replays were deterministic: I replayed game 3 to turn 6 five times with identical
  results.
- Errors were clear. For example: "seat 0 has no item at slot 0" (slots keep their numbers after an
  item is used), and "seat 0 has used an item this turn".
- Game 1 felt static. No Wind weather came in 17 turns and nobody moved, so one aim and power hit
  seven times.
- In game 2, Lanternwing's only window from the spawn was aim 26° to 28° at power 74 to 80. After
  the wind changed on turn 4, no shot hit until I walked 45 px.
- Force columns boosted damage as described: S1 went from 115 to 153.
- Next time: try a Tornado with the shooter close to it, to see whether it behaves like the Mirror.
  Run a balance sweep of the ashspine spawns for every mobile with an aim cap of 60° or less. Play
  Easy against Hard on the same seed to compare the difficulty levels.

## Follow-up

What changed after this playtest, and what is still open.

- **Fixed in the lab:**
  - `trace` has no `--facing` any more: it fires the way you face, as `Shoot` does, and reports
    which way that is. Turning is a free 0-step `Walk`.
  - The trace's summary names the point nearest the nearest enemy, with its distance, and says
    "Leaves the map" when every flight ends off it.
  - The log reads "Your shield", says which way each shot went and where it burst.
  - `map.surfaces` lists every floor in a column, so under an overhang the floor comes after the
    roof.
  - The picture has a legend, and weather columns read F, M and T.
  - Your options show your base delay and a pass's.
  - The rules give heal shares, Power Up, Sacrifice and Dig numbers, each mobile's armor, and that
    slots keep their numbers.
- **Fixed in the sim:**
  - A Bionic mobile's Heal healed 169 of 1000 HP instead of 170, a float rounding error
    (2214a2d). Big Heal's 250 against 300 is by design: Bionic mobiles heal 5 points more, and
    Coilback isn't Bionic.
  - A shot whose muzzle stood inside a weather column passed through it: columns only caught
    shots flying into them. A shot is now caught by any column it starts in, as in OB.
- **By design:** the 76 that Power Up didn't raise was the bolt's splash, which is flat whatever
  the armor, weather or items (GF-max). The rules now say so, under Sparkhorn and Power Up.
- **Open, for the game:**
  - The ashspine spire against mobiles whose aim stops at 60°.
  - Normal bots that never walk.
