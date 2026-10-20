# DragonBound: a live game + the official guide (2026-10-19)

Sources: a DragonBound account in a browser (auto-dropped into the tutorial room 103, "BOSS 1 vs bot",
map Meta Mine), and https://dragonbound.net/guide/* (howtoplay, controls, game, mobiles, items, stats).
Numbers are facts from the guide, summarized; descriptions are paraphrased.

## Could not play
Automated keyboard input never reached the game (synthetic JS KeyboardEvents and DevTools-protocol
key presses, arrows and ArrowUp; the angle readout never moved). Mouse clicks work in menus. Turns timed out.
Untried: Drag shooting mode via mouse drag, W/S keys. Leaving a Boss game is free.

## Observed in the tutorial game
- Turn timer: 20 s max; room options offer 5 / 10 / 20.
- Timed-out turn showed my delay as 960 = Armor base 510 + Shot1 250 + 20 s x 10/s. So a timeout is
  charged like a full-length Shot 1 turn.
- Turn list shows your own delay, and everyone else's relative to you (+810, -150). Top row moves next.
- Bot's Armor Shot 1 hit me for 147 (guide range 120-165). Bonus toasts in chat: "Good Shot Bonus +50G",
  "Shot Bonus +25G".
- Weather: a 5-slot queue across the top; tornado pillars on the map; "Random" weather slot.
- Room: 4 slots per team (up to 4v4), teams A/B, modes include BOSS and NORMAL (guide also names
  Score and Jewel), map picker, turn time, avatars on/off, max wind (was 0), random teams,
  item loadout panel (Dual, Dual+, Teleport; items take 1-2 cells). "Share Game Replay" exists, so
  games are recorded as replayable logs.
- Mobile picker (17 + Random): Armor, Ice, Aduka, Lightning, Bigfoot, J.D, A.Sate, Nak, Trico, Mage,
  Turtle, Boomer, Grub, Randomizer, Raon Launcher, Random, Frog, Kalsiddon. Random-only: Knight,
  Dragon, Barney, Electrico. Fox is rentable. DragonBound reuses GunBound's mobile names and looks.

## HUD (layout as seen)
- Top: ESC button (left), weather queue, gold counter (center), item slots F1-F6 (right), F8 pass,
  big red turn timer under it.
- Wind meter: dial near top center, number in the middle, arrow for direction.
- Around the active mobile: circular aim gauge; bright-green arc = True Angle (full damage),
  dim green = False Angle (reduced damage); red needle = current angle. Turn-timer bubble above head.
- Bottom: turn list (left), weapon buttons 1 / 2 / SS, angle readout with previous angle under it,
  shooting-mode toggle (Slice / Drag), power bar with ticks at 1/4, 1/2, 3/4 and a marker for last
  turn's power, movement bar, pass button.
- Art: chibi characters riding chunky vehicle "mobiles", thick dark outlines on terrain, painted
  layered backgrounds (Meta Mine: steampunk mine, gears, lava pit at bottom), seasonal UI theme
  (Halloween right now).

## Rules from the guide
- Turn: move (left/right or A/D, limited by a movement bar), angle (up/down or W/S), pick S1/S2/SS
  (Tab), optional item (F1-F6), shoot (Space hold-release in Slice; mouse drag in Drag; touch the bar
  for exact power on touch). Right-click-drag pans the camera.
- Delay: lowest goes next. Each mobile has a base delay (480-550) plus the shot's delay
  (S1 ~200-250, S2 ~340-480, SS 800-840) plus 10 per second of turn time (Turtle 12, Electrico 8).
  Items add delay. Guide contradicts itself: Items page says Dual +550, Dual+ +400, Teleport +100;
  How-to-play page says Dual +600, Teleport +150.
- Weather changes every round of turns: Force (+dmg), Black Force (-dmg), Lock (no items, shields
  stop regenerating), Mirror (reflects, adds force), Random, Thor (satellite bolt at impact),
  Thunder (bolt at impact), Tornado (shot orbits then exits), Wind (changes wind).
- Damage types: mobile armor is Steel/Shield/Bio; weapons are Hit/Laser/Electric/Explosion.
  Hit +15% vs Shield, -15% vs Steel; Laser +15% vs Bio, -15% vs Shield; Electric +15% vs Steel,
  -15% vs Bio; Explosion neutral.
- Shield mobiles have a blue shield bar that regenerates per round (Mage 220 +30, Lightning 300 +20,
  JD 250 +30, A.Sate 210 +35).
- Bonuses (gold/GP): double kill, triple kill, bunge (knock off map), kill, 100/200/400 damage tiers,
  hurricane, mirror, high angle (>=70 deg and 2 s or 3 s flight), boomerang, back shot, damage totals.
  Penalties for team damage, team kill, suicide. Leaving mid-game costs gold/GP and a loss.
- Avatar stats (when Avatars ON): popularity, speed/delay, attack, defense, life, item delay, dig
  (bigger craters), shield regen.

## Mobile table (base delay; S1 / S2 / SS damage and delay)
| Mobile | Type | HP (shield) | Base | S1 dmg / delay | S2 dmg / delay | SS dmg / delay | Gimmick |
|---|---|---|---|---|---|---|---|
| Armor | Steel / Explosion | 1000 | 510 | 120-165 / 250 | 230-290 / 430 | 350-500 / 800 | S2 two warheads; SS expands after 2.5 s |
| Mage | Shield / Laser | 730 (220, +30) | 500 | 100-160 / 250 | 190-280 / 400 | 350-500 / 800 | S2 twin wiggling beams; SS strips shields |
| Nak | Steel / Hit | 1100 | 520 | 100-150 / 250 | 190-300 / 400 | 350-500 / 800 | S2 digs and resurfaces; SS ignores land |
| Trico | Bio / Explosion | 1050 | 490 | 100-150 / 250 | 170-310 / 400 | 250-500 / 800 | S2 three orbiting balls |
| Bigfoot | Steel / Explosion | 1100 | 520 | 170-230 / 250 | 200-270 / 400 | 350-500 / 800 | spread missiles (4 / 6 / 9) |
| Boomer | Bio / Hit | 1100 | 480 | 120-160 / 250 | 230-290 / 400 | 350-900 / 800 | boomerangs, wind-heavy; SS turns gold |
| Raon | Steel / Laser | 1000 | 500 | 100-160 / 250 | 140-180 x2 / 400 | ? / 800 | S2 homing mines that crawl on your turn; SS walker bomb |
| Lightning | Shield / Electric | 750 (300, +20) | 500 | 150-250 / 200 | 150-300 / 350 | 150-200 per target / 800 | bolts from the sky at impact |
| J.D. | Shield / Electric | 950 (250, +30) | 500 | 140-170 / 250 | 150-200 / 340 | 250-320 / 800 | S2 pulls mobiles; SS pushes |
| A.Sate | Shield / Laser | 800 (210, +35) | 480 | 100-160 / 250 | 200-280 / 400 | 400-550 / 800 | tracer, then satellite lasers |
| Ice | Bio / Hit | 1200 | 490 | 120-160 / 250 | 150-170 / 400 | 150-170 / 800 | S2/SS shred physical defense 5% per hit |
| Turtle | Bio / Hit | 950 | 490 | 130-160 / 250 | 220-280 / 480 | 400-600 / 800 | 12 delay/s; twin streams; SS splits into 6 |
| Grub | Bio / Electric | 1000 | 490 | 80-130 / 250 | 180-220 / 400 | 400-1000 / 800 | bouncing timed orbs; SS damage over contact |
| Aduka | Steel / Electric-Laser | 1000 | 510 | 140-160 / 250 | 140-500 / 400 | 80-250 / 800 | Thor satellite that levels up with damage |
| Kalsiddon | Steel / Explosion | 1100 | 515 | 150-220 / 250 | 250-400 / 460 | 300-650 / 800 | missiles split at apex and home on a guide |
| J.Frog | Bio / Laser | 930 | 500 | 150-230 / 250 | 150-230 / 250 | 100-700 / 800 | rolling sponge (left / right) |
| Knight* | Steel / Laser | 1500 | 550 | 200-250 / 250 | 230-290 / 400 | 400-550 / 840 | A.Sate variant with swords |
| Dragon* | Bio / Explosion | 1250 | 550 | 150-250 / 250 | 300-500 / 400 | 450-650 / 800 | SS mini-dragons converge |
(* random-only. Barney, Electrico, Fox, Randomizer: stats not published.)
