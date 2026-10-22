# Thistleback (GunBound's Trico): sources

Gathered on 2026-10-22 for Thistleback, which carries Trico's behavior. The keys `[CR-mob]`,
`[SW-mob]`, `[SW-mds]`, `[DB-mob]`, `[CR-fixed]` and `[DBR]` are defined in `mechanics.md`, and
`[OB]` in `README.md`; this note adds two and records what each source says about Trico. Every
page was read as untrusted data.

## What the sources say

- **Stats.** `[CR-mob]` (fetched again): HP 1050, no shield, base delay 490 (10 a second), S1 250,
  S2 400, SS 800; aim S1 and S2 50–10 (true 40–20), SS 40–20 (all true). `[SW-mob]`: Bionic,
  Explosion, HP **1100**, the same delays (**CONTRADICTION** on HP; `[DB-mob]` and `[CR-mob]` give
  1050, `[OB]` 1100). `[DBR]`'s clone aims 10–60.
- **Damage.** `[SW-mob]` lists S1 100–150, S2 170–310 and SS 250–500, the same as `[DB-mob]`.
  `[WP06-list]`: S2's balls "usually spread over a large area and do 80-100 damage. However if all 3
  are landed it can do up to 300"; SS "usually damage is around 300-500". `[GB-wiki]`: "a perfect
  hit shot 2 with a dual will take off half of an enemy's health bar".
- **Weight.** `[WP06-list]`: "Trico's shot weight is second heaviest", which matches `[CR-fixed]`'s
  cbchui chart, where only Nak (2.70) needs more power than Trico (2.65).
- **S1.** `[SW-mob]`: "A heavy green cabbage-like ball that explodes upon impact. It creates some
  land damage."
- **S2.** `[SW-mob]`: "Three heavy green cabbage-like balls, one is in the center and the other 2
  revolve around the center. … It can be difficult to get all three balls to hit the opponent."
  `[GB-wiki]`: "three explosives in a line which rotate clockwise as they go through the air."
  `[WP06-list]`: "The revolving stops if the cabbages go through a tornado. Also the three balls
  travel differently depending on from which direction they are fired." A fixed clockwise spin is
  why the direction matters: mirrored, the same shot turns the other way about its path.
  `[OB]`'s numbers: the outer two at 35 px, 0.5 turns a second, swinging out at 2 a second.
- **SS.** `[SW-mob]`: "A fireball that rapidly releases multiple explosions. Removes lots of land
  and does pretty good damage to the opponent. Damage and bunging power is generally maximized if
  the shot hits an enemy's foot." `[WP06-list]`: "a rainbow colored ball that turns into an
  earth-removing spinning attack upon impact … damage and land destruction are mostly random".
  `[GB-wiki]`: "a radial one like most of the others in the game, the primary difference is Trico's
  SS digs a lot of the landscape as it hits." `[OB]`'s burst: 1 + 8 blasts, 0.1 s apart, 20 px
  round the impact at the 8 compass points.
- **No source says why the feet matter.** GunBound settles falling mobiles while a burst is still
  going off, so a mobile whose ground is dug out drops into the rest (inference). Lobshot settles
  mobiles after a volley, so `sim::damage::low` stands in for it: a blast is worth most at or under
  a mobile's feet.

## Sources

- `[GB-wiki]` Giant Bomb's GunBound page, "Trico Shots": https://www.giantbomb.com/gunbound/3030-15697/
  (read via the Wayback Machine, 2024 capture).
- `[WP06-list]` Wikipedia, "List of GunBound Mobiles and Attacks", "Trico (Explosion Attack)" and
  "Dragon": http://en.wikipedia.org/wiki/List_of_GunBound_Mobiles_and_Attacks (read via the Wayback
  Machine, 13 September 2006 capture).
