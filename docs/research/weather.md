# Weather (the moondisk)

GunBound's weathers, sourced, for M5. `mechanics.md` covers the cadence and the four weathers M2
shipped (Wind, Force, Mirror, Tornado); this file adds the rest, and the names that hide them. Raw
pages fetched for it sit outside the repo; the keys below say where each came from. Every quote is a
fragment; numbers are the sources' own.

## Source quality

GunBound Classic and Thor's Hammer guides (2002 to 2006) are the baseline, as everywhere in
`docs/research/`. `[OB]` is a fan reimplementation that documents its numbers in code, and `[DBR]`
a reverse-engineered DragonBound clone; both are **unverified** against the game. DragonBound's own
weather text `[DB-play]` copies `[SW-gp]`'s wording, so it adds names, not evidence.

## 1. One set of weathers, three sets of names

Classic StrategyWiki lists seven weathers by icon: Moon, Eclipse, Meteor, Thor, Force, Lightning
and Tornado `[SW-weather]`. They are the same weathers Thor's Hammer sources name differently.

| Classic `[SW-weather]` `[GF-tiro]` | Thor's Hammer `[WP06]` `[SW-gp]` `[OB]` | DragonBound `[DB-play]` | lobshot |
|---|---|---|---|
| Meteor | Wind | Wind | Wind (M2) |
| Lightning | Electricity | Thunder | Electricity |
| Eclipse (Item Lock) | Ignorance | Lock | Lock |
| Moon | Protection | (none) | Moon |
| Thor | Thor | Thor | Thor (with the Aduka behavior) |
| Force | Force | Force | Force (M2) |
| Tornado | Hurricane | Tornado | Tornado (M2) |
| (later) | Mirror | Mirror | Mirror (M2) |
| (later) | Weakness, Dark Force | Black Force | Weakness |
| (later) | Random, Joker Force | Random | Random |
| (later) | Land | (none) | none: "never been seen on any map" `[WP06]` |

- **Meteor is Wind.** The name comes from Wind's icon, a comet "with a red tail" `[WP06b]`; nothing
  falls. `[SW-weather]`: it changes the wind when it first appears, with no other effect.
- Mirror, Weakness, Random and Land are not in Classic or 2003 to 2006 sources; `[WP08]` (2008)
  counts eleven weather icons, these included. Land "was replaced by Thor" `[GF-cyber]`.

## 2. Placement, repeats and frequency

- Columns stand at random x `[GF-cyber]`, shown turns ahead by an arrow at the top `[GF-kurai]`.
  `[OB]` rolls x uniformly across the map.
- A repeat of Force, Tornado or Electricity adds a second column and keeps the first; two within
  about half a mobile's width "combine into a larger version" `[WP06b]`. `[OB]` affects a projectile
  at most once per weather type.
- **Frequency is per map.** The room screen shows each map's weights, 0 to 3 `[GF-tiro]`. The 2004
  weights, in the order Force / Tornado / Electricity / Wind / Moon / Lock / Thor `[GF-vgg]`:

  | Map (archetype) | F | T | E | W | Moon | Lock | Thor |
  |---|---|---|---|---|---|---|---|
  | Metamine A (mine corridors) | 0 | 2 | 0 | 3 | 0 | 2 | 1 |
  | Metamine B | 2 | 3 | 0 | 0 | 0 | 2 | 1 |
  | Miramo (town rooftops) | 2 | 1 | 2 | 1 | 0 | 0 | 2 |
  | Nirvana A (floating islands) | 0 | 1 | 2 | 2 | 1 | 1 | 1 |
  | Nirvana B | 0 | 0 | 4 | 1 | 0 | 0 | 3 |
  | Metropolis (city) | 1 | 1 | 3 | 1 | 0 | 1 | 1 |
  | Sea of Hero (long flat sea) | 2 | 0 | 2 | 1 | 0 | 0 | 3 |
  | Adiumroot (root tangle) | 1 | 3 | 1 | 0 | 1 | 1 | 1 |
  | Dragon (dragon's-back ridge) | 3 | 0 | 2 | 1 | 1 | 0 | 1 |
  | Cozytower A (tall tower) | 2 | 1 | 1 | 1 | 0 | 1 | 2 |
  | Cozytower B | 2 | 2 | 1 | 1 | 0 | 1 | 1 |
  | Dummy Slopes (practice slope) | 2 | 2 | 0 | 1 | 1 | 1 | 1 |
  | Stardust (sparse starfield) | 1 | 2 | 2 | 1 | 0 | 1 | 1 |

  On Dragon, Electricity and Force "always appear in the center of the level" `[GF-vgg]`.

## 3. Electricity (Lightning, Thunder)

- **The same as the Thunder item.** The item "works the same" `[GF-tiro]`; it is "the equivalent of
  sending your shot through the electricity" `[GF-vgg]`. `[OB]` implements the item as an invisible
  Electricity column on the shooter. So one bolt serves both.
- A full-height column. A shot that crosses it is electrified; where it lands, a bolt "strikes from
  above, stopping when it hits land", leaves a small hole, and shocks those nearby `[SW-weather]`.
  One bolt per projectile: four projectiles, four bolts `[GF-tiro]`.
- **Damage: no GunBound number.** "A less damaging version" of Lightning's first shot `[WP06]`.
  `[OB]`: a flat 30 within 30 px of the bolt, crater 10; `[DBR]`: 75. The closest GunBound numbers
  are the slot machine's bolt, 76 to 80 `[WP06]`, and Lightning's 76 splash `[GF-max]`.
- Width: `[OB]` 40 px, like Force. GunBound is silent.
- J.Frog's shots ignore weathers that change damage, Thor and Electricity included `[WP06]`.

## 4. Lock (Eclipse, Ignorance)

- No one may use an item; "there is no exception to this rule" `[WP06]`. Also `[SW-weather]`,
  `[GF-tiro]`, `[DB-play]`. SS stays usable `[GF-vgg]`.
- Shields stop regenerating `[WP06]`, `[SW-gp]`, `[DB-play]`. (`[OB]` keeps regenerating them.)
- SS recharges faster: "twice as fast during Item Locks" `[WP06]`; also `[SW-gp]`, `[GF-cyber]`.
- It lasts one moondisk slot, like any weather. Sources say nothing of items armed before it began.

## 5. Moon (Protection)

- A heal while it is up: at the start of each player's turn, with Bionic mobiles recovering
  "significantly more" `[SW-weather]`; "a small increase in energy for each turn", Bionic double
  `[WP06]`; 3% of HP a turn, Bionic double `[NAMU]`.
- **CONTRADICTION:** `[OB]` heals once, when it activates, a flat 35 HP (70 Bionic).
- Sources are silent on shields.

## 6. Weakness (Dark Force, Black Force)

- A column that weakens the shots passing through it: "decreases the damage of your shot by 50%"
  `[WP08]`; "reduces shot damage" `[DB-play]`.
- **CONTRADICTION:** `[OB]` divides by 1.2 and subtracts 10, the inverse of its Force. `[DBR]`
  halves.

## 7. Random (Joker Force)

- A column whose kind is hidden until a shot passes through it, then it becomes that weather where
  it stands `[DB-play]`.
- `[OB]`: 80 px wide; the server rolls the hidden kind ahead from Force, Tornado, Electricity,
  Weakness and Mirror; it never becomes Wind, Thor, Lock or Moon.

## 8. Thor

- While Thor is up, every projectile that lands calls a beam from the satellite to where it landed;
  whatever is in the way takes it, and it digs `[SW-weather]`, `[CR-aduka]`. One beam per
  projectile `[GF-tiro]`.
- The satellite hovers high above the map on every level `[SW-weather]`, `[CR-aduka]`, off screen
  `[GF-tiro]`. GunBound sources don't say whether it moves; `[OB]` moves it to a random point on
  each activation.
- **Levels:** Thor gains experience from the damage its own beams deal, and keeps it all game
  `[GF-lee]`, `[GF-max]`. Aduka's beams feed the same pool `[GF-max]`, `[CR-aduka]`.
  - **CONTRADICTION**, thresholds: 250 / 750 / 1750 / 2500 `[GF-tiro]`; 200 / 700 / 1800 / 3900 /
    6800 `[GF-lee]`, `[GF-vgg]`; 800 / 1400 / 2000 / 2400 / 3000 `[OB]`. The designers raised them
    after Aduka's upgrade `[CR-aduka]`.
  - **CONTRADICTION**, top level: 6 in most sources; 7 `[CR-aduka]`.
  - **CONTRADICTION**, beam damage: 50 to 100 at first `[CR-aduka]`; about 200 by level 5
    `[GF-tiro]`; 150 to 300 by level `[GF-vgg]`; `[OB]` 40 + 20 a level.
- Aduka's S2 and SS call beams with or without the weather, and get no extra ones during it
  `[GF-lee]`.

## Sources

Keys already defined in `mechanics.md` (`SW-weather`, `SW-gp`, `WP06`, `GF-tiro`, `GF-max`,
`DB-play`, `DBR`) and `README.md` (`OB`) are not repeated.

- `[WP06b]` https://en.wikipedia.org/w/index.php?oldid=87291062 (Gunbound article, 2006-11-12)
- `[WP08]` https://en.wikipedia.org/w/index.php?oldid=233913400 (2008-08-24; the eleven icons first
  appear in oldid 205454934, 2008-04-14)
- `[GAMICUS]` https://gamicus.fandom.com/wiki/GunBound (archive 20250919165951; copies `[WP08]`)
- `[GF-vgg]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31328 (vgamegirl v1.2,
  2004-08-15; archive 20190526114145)
- `[GF-lee]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/25616 (Mobile Guide v1.6,
  2004-04-21; archive 20190526114145)
- `[GF-cyber]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/28187 (Cyberquad v1.1,
  2004-07-26; archive 20190526114146)
- `[GF-kurai]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/26935 (Beginner's guide
  v0.90; archive 20190526114143)
- `[GF-ever]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/30567 (Lightning guide,
  2005-05-14; archive 20190526114146)
- `[GF-super]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/19598 (v.72, 2002-10-04;
  archive 20190526114146): Force at +25% in the 2002 beta
- `[CR-aduka]` https://creedo.gbgl-hq.com/aduka.doc (Creedo Aduka Guide v0.6)
- `[CR-all]` https://creedo.gbgl-hq.com/gunbound_all.doc
- `[NAMU]` https://namu.wiki/w/건바운드 (read through the mirror https://tcatmon.com/wiki/건바운드)
- `[SNX-aduka]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/aduka.html (official; archive
  20181029092209)
- `[SNX-items]` http://gunbound.softnyx.net/GameInfo/Contents/GameItem.aspx (official; archive
  20211016214959): Thunderbolt, 2 slots, delay 100
- `[SNX-beginner]` http://gunbound.softnyx.net/GameInfo/BeginnersGuide.aspx (official; archive
  20161218063647)
