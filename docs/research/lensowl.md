# Lensowl: GunBound's A.Sate

What the sources say about A.Sate, the Classic and Thor's Hammer mobile whose behavior Lensowl
(`sim::tuning::lensowl`) carries, and what lobshot picked where they are silent. Gathered on
2026-10-24; every page was read as untrusted data. OpenBound has A.Sate only as a data row
(`openbound.md`). Keys from `mechanics.md` (`[SW-mob]`, `[SW-mds]`, `[CR-mob]`, `[CR-fixed]`,
`[DB-mob]`, `[DBR]`, `[GF-tiro]`, `[GF-max]`), `weather.md` (`[GF-lee]`, `[GF-vgg]`, `[NAMU]`),
`thistleback.md` (`[WP06-list]`, `[GB-wiki]`) and `README.md` (`[OB]`) mean what they mean there.

## What the sources say

- **Stats.** HP 750, shield 200, +30 a turn `[CR-mob]`, `[GC-db]` (about 750 / 200 / 25
  `[GF-asate]`); 760 / 220 / +20 `[SW-mob]`; 800 / 210 / +35 `[DB-mob]` (**CONTRADICTION**).
  Shield armor, Laser weapon `[SW-mob]`, `[DB-mob]`. Base delay 480 `[CR-mob]`, `[GC-db]`,
  `[DB-mob]`, or 490 `[SW-mob]` (**CONTRADICTION**); every source's totals are 730 / 880 / 1280
  `[SW-mds]`, which add up from 480. Shots 250 / 400 / 800.
- **Angles.** 60–20, SS 55–25, no false range `[CR-mob]`, `[GC-db]`; the SS is "Shot 2's angle with
  5 degrees max and min taken off" `[GF-asate]`. `[DBR]`'s clone aims 20–60.
- **Weight.** cbchui: 2.50 bars for one screen at 60 degrees `[CR-fixed]`; CreeDo's own fixed-power
  method for sate fires 2.5 bars at 90 minus the distance, "60 = 1 screen" `[CR-sate]`. At 60
  degrees `[CR-60]` needs 2.55 for a screen (**CONTRADICTION**). The descriptions disagree too:
  third heaviest `[WP06-list]` (`[CR-1sd]` gives it Raon's full-power angle), average `[GF-lee]`,
  light `[GF-tiro]`, `[WP06-gbart]`, and `[CR-3bar]` groups it with the lightest.
- **The tracer.** "Tracers themselves do not do any damage, similar to the Aduka attack"
  `[WP06-list]`; "The tracer shot deals no damage" `[GF-lee]`.
- **The satellite.** "a floating satellite which is above the A.Sate shoots an inaccurate laser at
  the guide. If there is land or a player directly in between the satellite and where the guide
  landed, the laser will hit the land" `[SW06-mob]`; "the satellite points to that point and
  shoots concentrated light … When the ground stand in the way of its aiming point and the
  satellite, It's almost impossible to shoot" `[SNX-asate]`; "the land blocks the shot 1's laser,
  which fires at a low flat angle" and "If a shot lands past them but they're in the line of fire
  between you and the tracer, they still can get hit" `[CR-sate]`. "The disk is pretty low above
  the bot's head" `[WP06-gbart]`. It sits inside terrain if the ground overhangs: "you could have
  your satellite in the earth" `[GF-asate]`. No source says whether it can be hit.
- **Its heights.** "For weapon 1 and 2, the satellite stays directly above you … for SS, it hovers
  above (or below) where the tracer landed"; "weapon 1's satellite is the lowest, weapon 2's is the
  highest, and the SS' is in the middle … the bot, weapon 1's, SS' and weapon 2's satellite are all
  evenly spaced apart" `[GF-asate2]`. S2's is "3 times as high as shot 1" `[SW-mob]` or "about
  double" `[GF-vgg]` (**CONTRADICTION**); "the Satellite is lowered by around 2 mobile lengthes" for
  the SS `[GF-max]`; "switch between weapons to see your satellite bob up and down!" `[GF-lee]`.
- **S1's inaccuracy is a bias, not a roll.** "When shooting from the right side of the opponent,
  the laser tends to shoot about half a centimeter above the guide. When shooting on the left, it
  tends to shoot in a more accurate manner" `[SW06-mob]`; "Landing the tracer directly on the enemy
  mobile may cause the laser to swoop past the mobiles head, not doing any damage" `[GF-max]`. No
  source says it is random. Damage 100–160 `[SW-mob]`, `[DB-mob]`; "A near miss does no damage
  also" `[CR-sate]`.
- **S2.** Three lasers from the higher satellite `[SW-mob]`. "one lead laser and two tail lasers
  which shoot to the left of the lead laser … the lead laser shoots directly at the guide"
  `[SW06-mob]`; "Sate's Shot 2 lasers always rain down from left to right" `[CR-sate]`; "The shots
  always hit to the left of the tracer and move to it" `[GF-asate]`. Their spread: "Imagine a line
  going from the satellite straight down to Sate, and a line going from the tracer to the
  satellite. The smaller this angle is, the closer the lasers will bunch together … your shooting
  angle does nothing to affect this" `[GF-asate]`; "At long range, your laser spread is so wide
  that half your shot will miss" `[CR-sate]`. Bunched, "the lasers will make a perfect vertical
  narrow hole" `[CR-sate]`. Damage 200–280 `[DB-mob]` (240–280 `[SW06-mob]`); "each doing around
  80-90 damage" `[GF-max]`. Giant Bomb's six lasers `[GB-wiki]` is alone.
- **SS.** "the satellite moves right above wherever the guide landed … It is NOT shot straight
  down, the satellite starts from the left side and shoots a laser, moves approximately half a
  centimeter to the right and shoots another one. This continues until the satellite has released
  5 lasers to the target" `[SW06-mob]`, `[SW-mob]`. "The satellite fires 6 lasers straight down,
  moving slightly to the left and right … the satellite only moves horizontally, so firing on
  someone on a high elevation from a low elevation causes screwiness" `[GF-asate]`. "One is shot
  straight down and the other four are shot at slight angles" `[WP06-list]`. Six `[GF-asate]`,
  `[GF-lee]`, `[GF-tiro]`, `[GF-max]`, seven `[NAMU]` (**CONTRADICTION** on the count and on the
  fan). It leaves "a clean, narrow, vertical hole" `[GF-asate2]`. Damage 400–550 `[SW-mob]`,
  `[DB-mob]`; "Usually 75 per laser" `[GF-asate]`.
- **Weather.** The tracer and the laser are both bent by a tornado `[SNX-asate]`, `[WP06-list]`,
  `[NAMU]`. For Force, "both your tracer AND the laser must pass through" `[CR-sate]`; the SS "is
  denied any weather effects unless the opponent is actually INSIDE the weather or the side to side
  movement causes the satellite to move into the weather, regardless of wether the tracer went
  through one" `[GF-asate]`. Thor follows each laser: "You still get 6 thor lasers" `[GF-asate]`;
  "Sate gets a good damage bonus from lightning, better than almost anyone else" `[CR-sate]`.
- **Floating.** `[WP06-list]` counts A.Sate among the floating mobiles (J.Frog's shots don't slow
  under it). Lobshot doesn't model that for Lanternwing (Mage) either.

## What lobshot does, and the picks

- **Stats:** `[DB-mob]`'s 800 / 210 / +35, since `[CR-mob]` and `[SW-mob]` disagree (the fidelity
  rule). Base delay 480. Aim 20–60 (SS 25–55), all true.
- **Gravity** fitted to `[CR-fixed]`'s 2.50 bars, which `[CR-sate]` repeats.
- **The satellite** hovers straight over Lensowl's feet, whatever the slope: 130 px for S1, about
  two of its lengths (`[GF-max]`, `[GF-asate2]`), three times that for S2 (`[SW-mob]`), twice for
  the SS (`[GF-asate2]`). It takes 0.25 s to reach a shot's height as the shot fires (a pick); the
  client shows it at the picked shot's height during its player's turn (`[GF-lee]`). Nothing hits
  it, and it may sit inside a hill.
- **Every laser** runs from the satellite through its aim point to the first mobile or ground in
  its way (`shots::beam`, Skyeye's too), its shooter included, and bursts there: a narrow blast
  (15 px for S1 and S2, 18 for the SS) that is also its crater. It fires 0.2 s after the tracer
  lands, then 0.15 s apart (picks).
- **S1's bias**, as `[SW06-mob]` gives it: aimed 13 px (half a centimeter on an 800 px, 30 cm screen)
  above the tracer when the satellite is right of it, at the tracer otherwise. Deterministic, as
  the sources describe a bias, not chance.
- **S2:** the lead aims at the tracer and fires last; the two before it lean 1.5 and 3 degrees left
  on screen about the satellite (a pick: 14 px apart fired straight down, about 28 half a screen off
  and 53 a screen off, where one of three lands on a mobile). 67–93 a laser: `[DB-mob]`'s 200–280
  split three ways, near `[GF-max]`'s 80–90.
- **SS:** five lasers (`[SW-mob]`, `[WP06-list]`) straight down (`[SW06-mob]`, `[GF-asate]`), 13 px
  apart, left to right, after the satellite glides 0.6 s sideways over the landing at its SS height
  over Lensowl's feet (`[GF-asate]`: it only moves sideways). 80–110 a laser: 400–550 split five
  ways.
- **Weather:** the tracer is a projectile, bent and marked like any. A laser is marked by the Force,
  Drain and Electricity columns it crosses, and only those (`[GF-asate]`; `[CR-sate]` wants the
  tracer's too, which flies the same stretch of sky almost always). No column bends a beam (a
  pick: Skyeye's beams aren't bent either). Thunder, Electricity's bolt and Skyeye's beam follow
  each laser that lands.

## Sources

- `[GF-asate]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31866 (Smashnuke, Asate FAQ,
  2004-08-18; archive 20190526114147)
- `[GF-asate2]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/35112 (chloromethane, A.sate
  Supplementary Guide v1.22, 2005-03; archive 20190526114145)
- `[CR-sate]` https://creedo.gbgl-hq.com/sate_quikki_guide_creedo.php (CreeDo, "Quick and useful
  info for sate", undated)
- `[SW06-mob]` StrategyWiki's Gunbound/Mobiles before its "ADVANCED" notes were cut (2007-06-09):
  https://strategywiki.org/w/index.php?oldid=122540 (2007-03-25) and
  https://strategywiki.org/w/index.php?oldid=89762 (2006-12-13)
- `[WP06-gbart]` https://en.wikipedia.org/w/index.php?oldid=54461974 (the Gunbound article,
  2006-05-22, before its mobile list was split out)
- `[SNX-asate]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/aSate.html (Softnyx, official;
  archive 20180325101656)
- `[GC-db]` http://www.geocities.com/gunbound_canada/database.html (Gunbound Mobile Database;
  archive 20070520053328)
- `[CR-60]` https://creedo.gbgl-hq.com/fixed_60.php (KNAT and Vexez, powers at 60 degrees)
- `[CR-1sd]` https://creedo.gbgl-hq.com/1sd_high_angles.php (full-power angles for one screen,
  credited to chill)
- `[CR-3bar]` https://creedo.gbgl-hq.com/3_0formula.htm (Vexez, translated from KNAT)
