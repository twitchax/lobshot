# Augerdillo (GunBound's Nak): sources

Gathered on 2026-10-24 for Augerdillo, which carries Nak's behavior (StrategyWiki calls it
"Nakmachine"). The keys `[CR-mob]`, `[SW-mob]`, `[SW-mobC]`, `[SW-mds]`, `[DB-mob]`, `[CR-fixed]`,
`[CR-nak]`, `[GF-max]`, `[GF-tiro]`, `[WP06]` and `[DBR]` are defined in `mechanics.md`,
`[WP06-list]` in `thistleback.md`, `[GF-lee]` in `weather.md`, and `[OB]` in `README.md`; this note
adds the rest and records what each source says about Nak. Every page was read as untrusted data.
The 2003–05 guides measure delay and damage on GunBound's older scale, so their numbers differ from
the tables'.

## What the sources say

- **Stats.** `[SW-mob]` (re-read): Mechanical, Physical, 1100 HP, base delay 520 (10 a second),
  S1 100–150 / 250, S2 190–300 / 400, SS 350–500 / 800, "Very low mobility, but high climbing
  ability. Hard to master." `[DB-mob]` and `[CR-mob]` agree. `[SW-mds]`'s totals (pass 710, S1 760,
  S2 910, SS 1310) imply base 510, and `[OB]`'s delay table copies them (**CONTRADICTION**).
  `[DBR]`'s clone also says 520, HP 1100. Older totals: `[GF-exp]` 820 / 850 / 1320, `[GF-lee]` and
  `[GF-ant]` 770 / 920 / 1320.
- **Movement.** "low movement per turn" `[WP06-list]`; "Poor movement and climbing" `[GF-exp]`;
  `[GF-lee]` rates Movement 1/5, Climbing 2/5 (**CONTRADICTION** with `[SW-mob]`'s high climbing).
- **Angles.** `[CR-mob]`: S1 and S2 50–10 (true 45–15), SS 50–10 (all true). `[GF-max]`: "all of
  nak's angle is a true angle, and it has a 2% defence raise" (a later GunBound; `[OB]` likewise
  makes 10–50 all true). `[DBR]`: `minang: 110, maxang: 170` (10–70 backwards, **CONTRADICTION**).
- **It fires from the rear.** `[WP06-list]`: "This machine fires its bullets from the rear, like
  Aduka". `[GF-exp]`: "the only tank that fires out the back. This has absolutely no tactical
  effect since any tank can face either direction at will". `[DBR]`'s launch code moves the muzzle
  for Nak and Aduka 26 px behind the body's center, 31 px up.
- **Weight.** `[CR-fixed]`'s cbchui chart puts Nak heaviest: 2.70 bars for 60 degrees, one screen.
  `[DBR]`'s shot weight `ax` is 82. `[WP06-list]`: S1 is "an electric sphere that is very heavy,
  meaning that it is less affected by wind factors". `[DAT-nak]`: "Nak's weaponry are the heaviest
  in the game". S2 "has the same weight as the primary attack" `[GF-exp]`.
- **The Nak formula.** `[CR-nak]` (re-read): `Power = Sqrt (Distance/((47.87^2)*SIN(2*A)*((1/(111.615+W*SINX))+(1/610)))*4`,
  Distance "in UHA Method (1 screen = 9 parts)", A "Nak Shooting Angle", W and X the wind's power
  and angle. "The formula doesn't take horizontal wind into account yet." Vertical wind adds to the
  formula's gravity, so it also says how much the wind moves Nak's shots.
- **Aiming lore.** `[CR-ga1]`: "1 screen for nak shot 2 = 79 / 1 screen for nak shot 1 = 76";
  "Ngle 70shot 2 1/2 screen is about 2.1 full screen is about 3". `[CR-sage]`: "70 angle - 3.3 bars
  is full screen", "60 angle - 2.9 bars", "76 angle - Full is full screen". `[CR-fun]`: "exactly 1
  screen distance should be counted as 79 with almost full power". `[CR-nakwind]`: against the
  wind, "lower your angle by 2/3 of the wind".
- **S1.** `[SW-mob]`: "A regular round that deals damage upon impact."
- **S2.** `[SW-mob]`: "Digs into the ground upon contact, and returns to the surface where it
  detonates."
  - **Through mobiles until it digs in.** "it will pass right through tanks if it hasn't hit ground
    yet" `[GF-exp]`; "The shot 2 goes through bodies unless it has already gone underground"
    `[CR-sage]`. `[DBR]`'s drill has `canPlayerCollide: false`. "Many players avoid this by moving as
    close as possible to the enemy mobile and simply 'dropping' the shot on top of them"
    `[WP06-list]`.
  - **The bore.** "when it hits the ground it continues in the same direction horizonally, but it
    begins to curve upward as if gravity had been reversed. It detonates when it exits from the
    ground." `[GF-exp]`. It "resurfaces further in a reverse parabolic arc of its original path
    through the air, depending on the strength of firing. When it is underground, there is a strong
    'reverse gravity' that pulls it sharply to the surface where it explodes." `[WP06-list]`. "when
    it enters the ground at minus 84 degrees.. it will come up at 84 degrees" `[CR-ga2]`; "burrows
    through the ground whiling maintaing its speed horizontally before rising back up"
    `[DAT-nak]`.
  - **How far.** No source gives a number. "the faster the shot is going when it hits the ground,
    the further underground it will travel before resurfacing and exploding" `[WP06-list]`; "If it
    hits the ground at a shallow angle (near horizontal), it will travel a long way before it comes
    back up" `[GF-exp]`; "The higher the angle you use to shoot, the deeper Nak's shot 2 will go
    before curving upwards. However, it won't go as far." `[GF-ant]`.
  - **The wind.** "If the wind is strong enough, it will cause your nak bomb to start to reverse,
    meaning that the underground travel is also reversed" `[DAT-nak]`. Underground: "NOT affected
    by wind or tornado" `[GF-max]`; "YES wind DOES affect ur underground shots but not as much as
    above the ground", and an underground shot "will pass through the tornado without being
    affected" `[CR-ga1]` (**CONTRADICTION** on wind). `[DBR]`'s digger has no wind.
  - **Where it goes off.** "It detonates when it comes out of the ground, only." `[GF-exp]`;
    "explodes once it reachs the division between land and air after going in ground" `[GF-ant]`.
    Through thin land "it will hit the bottom side of the land and explode", and a bomb that
    "brushes" a wall goes off at once `[DAT-nak]`. "It is also possible to fire it directly
    underground" `[WP06-list]`. No source says what happens at the map's bottom.
  - **Land.** A crater where it comes up: "When it comes out the bottom side, it blows a big fat
    hole" `[GF-exp]`; "fairly large holes" `[GF-ant]`. No source mentions a tunnel, and `[DBR]` digs
    no crater where it enters.
  - **DragonBound's clone.** `[DBR]` (`src/game/gametypes.js`, the Nak entry; `src/game/world.js`,
    `adddigger` and `checkCollision`): where the drill strikes the map, a "digger" leaves from 5 time
    steps on along the flight, at the negated absolute angle of the flight there (down, the way it
    was going), with the drill's launch power (`power: "parent"`, an assignment that is always true)
    and weight -360 against the drill's 360, and completes, with its blast, at the first pixel that
    isn't ground.
- **Dragon.** "Dragon cannot be directly hit by: rolling Grub's Shot 2 ..., J.Frog's shots,
  Nakmachine's Shot 2" `[SW-mob]`; "making Nak 2 shots less effective" `[WP06-list]`; "(almost)
  immune to Nak's weapon 2" `[GF-tiro]`; "some form of protection" `[GF-lee]`.
- **SS.** `[SW-mob]`: "Shoots a mini-nak that ignores the land, going right through it. It explodes
  only when it hits a player (whether ally or enemy), which includes dead players (corpses). It is
  most often used for shotgun shooting, but its damage is not optimal. It won't do much damage
  unless you are lucky and you hit the mobile's 'weak spot'." `[WP06-list]`: "the same weight and
  close to same damage as the nak's shot 1, but travels through land as if it were air, and has a
  bit larger explosion radius ... a maximum damage of about 300." `[GF-ant]`: "It stops upon impact
  with any mobile, whether it's friend or foe, dead or alive. If it goes through without hitting
  anyone, it'll just drop out of the level through the bottom or the side." `[GF-cyb]`: until it
  hits "a corpse, player, or the edge of the screen". "SS also has some bunge capability when it
  hits someone" `[GF-ant]`. `[CR-ga1]`: S1 and SS are "the same ... same weight". `[CDW-pt]`, a
  Portuguese repost of an older guide, says it passes any obstacle "(isso nao inclue mobiles mortos)"
  (not dead mobiles) and is "mais pesado do que o tiro 1" (heavier than S1), both
  **CONTRADICTIONS**. `[DBR]`: `canMapCollide: false`, damage 250, crater 60 by 63; it hits allies,
  and the shooter only once it is 20 px out.
- **SS damage.** 350–500 `[SW-mob]`, `[DB-mob]`; the 2003–05 guides say about 300 at most
  (`[WP06-list]`, `[GF-exp]`), 250–300 `[GF-tiro]`, 200–250 `[DAT-nak]` (**CONTRADICTION**, an older
  scale). "this shot can do 400+ damage, if you are overlapping an enemy mobile ... This shot
  usually does 200+ damage." `[GF-max]`.
- **The weak spot.** No source says where it is. The clues: "overlapping an enemy mobile" for 400+
  `[GF-max]`; for S2, "hit EXACTLY in the middle of the opponent" `[GF-max]` and "the center of near
  the back of the center of the mobile for maxiumum damage" `[DAT-nak]`; a mobile's damage "depends
  on where a collision occurs relative to the hitbox" `[SW-mobC]`; a low Nak shot "hits them in
  their weak spot, doing the most dmg" `[CR-kex]`; aim the SS "at the top of the mobile but nothing
  higher than that or you will miss" `[DAT-nak]`.

## What lobshot built, and the picks

- **Rear fire.** Aim ranges past 90 aim over the back (`AimRange::backward`): `[CR-mob]`'s 50–10
  (true 45–15) is 130–170 (true 135–165) here, the SS 130–170 all true. The sim's launch needed no
  change; what reads an aim as an angle above level (the HUD, Up and Down, High Angle, the formula
  tests, the bot's facing, portraits) goes through `AimRange::elevation` or `backward`, written the
  same way as Flarecrow's.
- **Weight.** Gravity is fitted midway between `[CR-fixed]`'s 2.70 bars and `[CR-nak]`'s 2.62 for
  60 degrees over one screen, fired over the back, so both hold within 0.05 (as Herdhound is fitted
  between its two sources). Unfitted, the model holds `[CR-nak]` at 45 and 50 degrees from three
  quarters of a screen to one and a quarter, `[CR-ga1]`'s 2.1 bars for 70 degrees over half a
  screen, and lands a full-power shot one screen away at 78 degrees, within the guides' 76 to 79.
  `[CR-sage]`'s powers (2.9 for 60 degrees, 3.3 for 70, over a screen) are well above every other
  source and the fit, and are out. Wind sensitivity is fitted to `[CR-nak]`'s vertical wind term (14
  up-wind over one screen at 60 degrees): 0.67, so its heavy shot is less affected by wind, as
  `[WP06-list]` says. `[DBR]`'s wind coefficient (`ay` 0.867 against Armor's 0.74, clone) points
  the other way (**CONTRADICTION**).
- **Movement.** 60 steps a turn, two thirds of everyone's 90 `[OB]`, for "very low mobility", and a
  10 px climb per step (6 for the rest) for `[SW-mob]`'s "high climbing ability": both picks.
- **S1.** One round, 100–150, a blast of 20 (DBR's crater for it is about Armor's S2 warhead's).
- **S2.** The drill flies through mobiles until it strikes the ground (`[GF-exp]`, `[CR-sage]`,
  `[DBR]`). There it bores on: down at the angle and speed it struck, the way it was heading, under
  its gravity turned upward, so through level ground it bores a mirror of its arc, deeper and
  farther the faster it struck (`[WP06-list]`, `[GF-exp]`, `[CR-ga2]`); one the wind turned back
  bores back. `[DBR]` bores at the launch speed (**CONTRADICTION**; GunBound first). It goes off,
  190–300 with a blast of 20, at the first open pixel: on the surface, under a ledge, or in a mobile
  standing there (a direct hit). One whose bore leaves the map's sides or bottom is lost (a pick).
  No wind reaches it underground (`[GF-max]`, `[DBR]`), nor any weather column (`[CR-ga1]` for
  tornadoes). It digs no tunnel.
- **A floating mobile** (Kitewyrm, Dragon's behavior) can't be struck by the S2: the bore looks for
  mobiles only through `Game::grounded_frames`, and Kitewyrm's body hovers 16 px over the ground,
  where a drill coming up through the ground under it never is. Its blast (20) still reaches across
  the gap, so one coming up right under Kitewyrm grazes it for about a fifth of the least it would
  deal a mobile standing there: "(almost) immune" `[GF-tiro]`, "less effective" `[WP06-list]`. The
  hover alone gives the closer reading; nothing else special-cases it.
- **SS.** A mini armadillo that flies through the ground as through air and goes off in the first
  living mobile it meets, either side's, the shooter's own included; one that meets none leaves the
  map. Lobshot clears the dead from the map, so corpses don't stop it, and there are no jewels or
  mines yet (M6, T-003). Its blast is 25, "a bit larger" than S1's `[WP06-list]`, and carves a
  crater. Damage 350–500 `[SW-mob]`, `[DB-mob]`. The weak spot is a pick: the hitbox's center, the
  ordinary direct hit (`damage::damage`, the most dead center, the least at a corner). The SS goes
  off where it first touches a mobile, at the edge of its body, so a shot from afar never reaches
  the center; one fired while overlapping the target, point blank, goes off inside it, near the
  center, for close to 500 (`[GF-max]`'s 400+).

## The Nak formula against the fit

Bars to pass through a Coilback's center `screens` away, zero wind, flat ground, fired over the
back, with the fitted gravity, beside `[CR-nak]`'s. The acceptance set
(`crates/sim/tests/formula.rs`) is 60 degrees at one screen and Augerdillo's steepest angles, 45
and 50 degrees, from three quarters of a screen to one and a quarter; each holds within 0.05. The
formula is a level-ground parabola from the ground, while lobshot's shot leaves the mortar about 70
px up and is aimed at a center 29 px up, which matters most for flat and short shots: those are
out.

| Angle | 1/4 | 1/2 | 3/4 | 1 | 1 1/4 | 1 1/2 |
|---|---|---|---|---|---|---|
| 15 | 1.27 / 1.72 | 2.15 / 2.43 | 2.79 / 2.98 | 3.32 / 3.44 | 3.78 / 3.85 | 4.19 / 4.22 |
| 30 | 1.06 / 1.31 | 1.72 / 1.85 | 2.20 / 2.27 | 2.59 / 2.62 | 2.93 / 2.93 | 3.24 / 3.20 |
| 45 | 1.05 / 1.22 | 1.65 / 1.72 | **2.08 / 2.11** | **2.44 / 2.43** | **2.76 / 2.72** | 3.04 / 2.98 |
| 50 | 1.08 / 1.23 | 1.67 / 1.74 | **2.11 / 2.12** | **2.47 / 2.45** | **2.79 / 2.74** | 3.07 / 3.01 |
| 60 | 1.19 / 1.31 | 1.81 / 1.85 | 2.28 / 2.27 | **2.66 / 2.62** | 2.99 / 2.93 | 3.29 / 3.20 |
| 70 | 1.43 / 1.52 | 2.14 / 2.15 | 2.67 / 2.63 | 3.11 / 3.04 | 3.50 / 3.40 | 3.84 / 3.72 |

`tool_fit` prints this table after it refits the gravity.

## Sources

- `[GF-exp]` exploder, "Nakmachine FAQ", 2003-08-25:
  https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/25277 (read via the Wayback Machine,
  20190526114140 capture). A Nak player's own guide; the best on its mechanics.
- `[GF-ant]` Anthropofobe, FAQ v1.2, 2004-08-16: https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31328
  (Wayback, 20190526114145).
- `[GF-cyb]` Cyberquad, bots and items guide, 2004-07-26:
  https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/28187 (Wayback, 20190526114146).
- `[DAT-nak]` Dataslycer's Nak guide, with notes by Creedo (about 2004):
  https://www.oocities.org/gunbound_canada/nak.html
- `[CR-ga1]`, `[CR-ga2]` guardiangel's Nak guide, parts 1 and 2:
  https://creedo.gbgl-hq.com/guardiangel_nak_guide_pt1.php and
  https://creedo.gbgl-hq.com/guardiangel_nak_guide_pt2.php (informal).
- `[CR-sage]` https://creedo.gbgl-hq.com/sage_tonwhy_nak_guide.php (informal).
- `[CR-fun]` https://creedo.gbgl-hq.com/gbfunstuff.php and `[CR-nakwind]`
  https://creedo.gbgl-hq.com/nak_easy_opposite_wind_adjustment_formula.php (Creedo's site).
- `[CR-kex]` Creedo's GunBound site, Kexeric's Armor guide: https://creedo.gbgl-hq.com/kexeric_armor_guide.php
  (undated).
- `[CDW-pt]` "Ponto Fracos Dos Móbiles [Gunbound]", cheatsdaweb forum, 9 May 2012:
  https://cheatsdaweb.forumeiros.com/t111-ponto-fracos-dos-mobiles-gunbound (a machine-translated
  repost of an English guide; low confidence).
- `[SW-mob]` read again at its 2025-09-19 archive, section "Nakmachine"; `[WP06-list]` at its
  2006-09-13 archive, sections "NakMachine (Hit Attack)" and "Dragon"; `[GF-max]`, `[GF-tiro]` and
  `[GF-lee]` at their 2019 Wayback captures.
- `[DBR]` read at `5e42cb4` on 2026-10-24: `src/game/gametypes.js` (the Nak entry, lines 2394–2466),
  `src/game/world.js` (`adddigger` at 1262, `checkCollision` at 861, damage at 384–422),
  `src/game/game.js` (the rear muzzle, 448–480), `src/game/lib/shoot.js`.
