# Bigfoot, for Tuskrack

What the sources say about GunBound's Bigfoot, the mobile whose behavior Tuskrack carries, and how
`sim::tuning::tuskrack` reads them. Gathered on 2026-10-22. Every key is defined in `mechanics.md`
(or, for `[OB]`, in `README.md`); this file adds no new sources, only what those say about Bigfoot
that the shared notes leave out. Fetched pages are quoted as data.

## What each source says

- **StrategyWiki** `[SW-mob]` (archive 20250919061317): Mechanical, Explosion, 1100 HP, base delay
  520, 10 delay a second. "Good attack. Excellent bunging power, high HP but lacks good defense.
  High mobility, but has poor climbing ability for finding higher angles."
  - Shot 1: "Fires four missiles, which are fairly weak and have a high spread over long distances.
    Great for bunging." 170-230, delay 250.
  - Shot 2: "Fires six grenades, which spread out soon after launch." 200-270, delay 400.
  - SS: "Fires nine missiles. Similar to Shot 1 but slightly more damage." 350-500, delay 800.
- **DragonBound's guide** `[DB-mob]` copies the same text and numbers (`mechanics.md`, 3.1).
- **Wikipedia, 2006** `[WP06]`: "Also known as NoobBot or NoobFoot because the wide radius of the
  missiles can hit even with poor aim. This bot has very small shot angle."
  - Shot 1: "Fires 4 blue missiles, excellent for destroying land."
  - Shot 2: "Fires a barrage of 6 tiny rectangular explosives that spread out into 3 loose pairs as
    they travel."
  - SS: "Fires 8 red missiles similar to its shot 1, but with no land damage on a direct hit."
- **Tirodalth's FAQ** `[GF-tiro]`: angle range "Very Low", true angle "VERY VERY low". Shot 1 "180
  or so. Shoots 4 missiles. The missiles are very spread out, so its hard to have all missiles hit
  one target (unless you're close). Terrible accuracy. Heavy projectile." Shot 2 "180 or so. Shoots
  many small explosive shells. Shells are very spread out, so it has poor accuracy too. Heavy
  Projectile." SS "300+. Shoots many red missiles. Heavy projectile, but nice damage." "Recommended
  that you are VERY close to your opponent (nearly kissing range)."
- **cactusjack23's damage guide** `[GF-max]`: Shot 1 "Shoots 4 rockets, each doing around 50 damage
  if you are using true angle... all 4 rockets split up once released from the cannon." Shot 2
  "Shoots 6 rockets. To maximize the damage, shotgun it to make sure all 6 rockets hit. This does
  around 200-300 damage." SS "Shoots 12 rockets, doing a total of around 400+ damage." "A bigfoot
  will have difficulty doing more than 100 damage per shot 1 with any distance futher than half the
  screen."
- **Vexez's table** `[CR-mob]`: 1100 HP, no shield; 520 / 250 / 400 / 800; aim 40-18 (true 35-23)
  for Shot 1 and 2, 35-23 for SS. **OpenBound** `[OB]` reads the same ranges.
- **cbchui's chart** `[CR-fixed]` gives Bigfoot's fixed power as "??".
- **OpenBound** `[OB]` (`openbound.md`, 3.7): Shot 1 is 4 shots at +3, -3, 0 and 0 degrees, 0.2 s
  apart; Shot 2 two waves of 3 at power -4, 0, +4 (of its 100), at 0.2 s and 0.4 s; SS 8 shots
  (the Shot 1 pattern twice), 0.125 s apart. Hitbox 46 x 40, the widest of its mobiles (Armor's is
  30 x 40).
- **The DragonBound clone** `[DBR]` (`src/game/gametypes.js`, clone, unverified): Bigfoot is
  `minang 20, maxang 45, ax 88, ay 0.74` (Armor: `ax 73.5, ay 0.74`). Its shots, as
  (`addtime` ms, angle, power) per projectile:
  - Shot 1, 4 projectiles of damage 40, crater `pala_bunge [39, 22]` (Armor's Shot 1: `[60, 42]`):
    (0, 0, 0), (100, +4, +10), (200, -2, -10), (300, +6, +20).
  - Shot 2, 6 of damage 44, crater `[38, 22]`: (0, 0, 0), (0, +5, +15), (0, +10, +30), then the
    same three at 100 ms.
  - SS, 8 of damage 42, crater `[39, 22]`: one every 50 ms, angles 0, +4, -2, +6, +1, +3, -1, +5.

## Contradictions, and what Tuskrack takes

- **The SS's missiles: nine** `[SW-mob]`, `[DB-mob]` **against eight** `[WP06]`, `[OB]`, `[DBR]`
  **and twelve** `[GF-max]`. GunBound's sources disagree, so DragonBound's nine (the fidelity rule).
- **Aim: 18-40** `[CR-mob]`, `[OB]` **against 20-45** `[DBR]`: GunBound's.
- **Damage per projectile.** The listed ranges are the whole shot landing, so each projectile carries
  an even share: S1 42-57, S2 33-45, SS 38-55. `[GF-max]`'s "around 50" a missile agrees with S1's.

## Shot weight

`[CR-fixed]` has no Bigfoot power, and `[GF-tiro]` calls every Bigfoot shot heavy. `[DBR]`'s `ax`
tracks the chart's powers as their square roots (`mechanics.md`, 2.3). Fitted by least squares
through the origin over the 13 mobiles both give, power = 0.289 x sqrt(`ax`) puts every one within
0.09 bar of its chart power (Turtle and Nak are the worst), and Bigfoot's 88, the heaviest, at
**2.71 bars**. Tuskrack's gravity is fitted to land 60 degrees one screen away at that power, as
the other mobiles' are fitted to theirs (`physics-fit.md`); `crates/sim/tests/formula.rs` holds the
data, the inference and both checks. Its `ay` equals Armor's, so Tuskrack's wind sensitivity stays
1.0.

## Movement

"High mobility, but has poor climbing ability" `[SW-mob]`, `[DB-mob]`, with no numbers anywhere
(`mechanics.md`, 6). OpenBound gives every mobile 90 steps, Knight 100, and one climb of 6 px a
step `[OB]`. Tuskrack's 120 steps and 2 px climb (slopes to about 63 degrees, against about 80) are
picks, and climbing is per mobile from here on (`MobileTuning::climb_px`); every other mobile keeps
OpenBound's 6.
