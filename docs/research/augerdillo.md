# Augerdillo (GunBound's Nak): sources

Gathered on 2026-10-24 for Augerdillo, which carries Nak's behavior (StrategyWiki calls it
"Nakmachine"). The keys `[CR-mob]`, `[SW-mob]`, `[SW-mds]`, `[DB-mob]`, `[CR-fixed]`, `[CR-nak]`,
`[GF-max]` and `[DBR]` are defined in `mechanics.md`, `[WP06-list]` in `thistleback.md`, and `[OB]`
in `README.md`; this note adds two and records what each source says about Nak. Every page was read
as untrusted data.

## What the sources say

- **Stats.** `[SW-mob]` (re-read): Mechanical, Physical, 1100 HP, base delay 520 (10 a second),
  S1 100–150 / 250, S2 190–300 / 400, SS 350–500 / 800, "very low mobility, but high climbing
  ability. Hard to master." `[DB-mob]` and `[CR-mob]` agree. `[SW-mds]`'s totals (pass 710, S1 760,
  S2 910, SS 1310) imply base 510, and `[OB]`'s delay table copies them (**CONTRADICTION**).
  `[DBR]`'s clone also says 520, HP 1100.
- **Angles.** `[CR-mob]`: S1 and S2 50–10 (true 45–15), SS 50–10 (all true). `[GF-max]`: a later
  GunBound made the whole arc true, with a +2% defence buff. `[DBR]`: `minang: 110, maxang: 170`
  (10–70 backwards, **CONTRADICTION**), with an `aim` of `[130, 40]` for every shot.
- **It fires from the rear.** `[WP06-list]`: "This machine fires its bullets from the rear, like
  Aduka". `[DBR]`'s launch code moves the muzzle to the other side of the body for Nak and Aduka.
- **Weight.** `[CR-fixed]`'s cbchui chart puts Nak heaviest: 2.70 bars for 60 degrees, one screen.
  `[DBR]`'s shot weight `ax` is 82, second only to Trico's 84 and Bigfoot's 88. `[WP06-list]`: S1
  is "an electric sphere that is very heavy, meaning that it is less affected by wind factors".
- **The Nak formula.** `[CR-nak]` (re-read): `Power = Sqrt (Distance/((47.87^2)*SIN(2*A)*((1/(111.615+W*SINX))+(1/610)))*4`,
  Distance "in UHA Method (1 screen = 9 parts)", A "Nak Shooting Angle", W and X the wind's power
  and angle. "The formula doesn't take horizontal wind into account yet." Vertical wind adds to the
  formula's gravity, so it also says how much the wind moves Nak's shots.
- **S1.** `[SW-mob]`: "A regular round that deals damage upon impact."
- **S2.** `[SW-mob]`: "Digs into the ground upon contact, and returns to the surface where it
  detonates." `[WP06-list]`: "Fires an 'oscillation bomb' which digs underground and then
  resurfaces further in a reverse parabolic arc of its original path through the air, depending on
  the strength of firing. When it is underground, there is a strong 'reverse gravity' that pulls it
  sharply to the surface where it explodes. It is also possible to fire it directly underground.
  The underground-traveling characteristic makes the shot difficult to use long-range because the
  faster the shot is going when it hits the ground, the further underground it will travel before
  resurfacing and exploding. Many players avoid this by moving as close as possible to the enemy
  mobile and simply 'dropping' the shot on top of them."
- **S2 in DragonBound's clone.** `[DBR]` (`src/game/gametypes.js`, the Nak entry; `src/game/world.js`,
  `adddigger` and `checkCollision`): the drill flies with `canPlayerCollide: false` and no damage of
  its own; where it strikes the map, a "digger" leaves from 5 time steps further along the flight,
  at the negated absolute angle of the flight there (down into the ground, the way it was going),
  with the drill's launch power (`power: "parent"`) and weight -360 against the drill's 360, so
  gravity pulls it up; it passes through the map and players, and completes, with its blast (damage
  140, crater 38 by 41, as S1's), at the first pixel that isn't ground. Nothing applies wind to it.
- **Dragon.** `[SW-mob]`: "Dragon cannot be directly hit by: rolling Grub's Shot 2 ..., J.Frog's
  shots, Nakmachine's Shot 2." `[WP06-list]`: "It floats above the ground, making Nak 2 shots less
  effective".
- **SS.** `[SW-mob]`: "Shoots a mini-nak that ignores the land, going right through it. It explodes
  only when it hits a player (whether ally or enemy), which includes dead players (corpses). It is
  most often used for shotgun shooting, but its damage is not optimal. It won't do much damage
  unless you are lucky and you hit the mobile's 'weak spot'." `[WP06-list]`: "Fires a miniature Nak
  that can go through ground and explodes upon impact with a mobile, a jewel, or a mini-Raon. This
  shot is the same weight and close to same damage as the nak's shot 1, but travels through land as
  if it were air, and has a bit larger explosion radius. Nevertheless, it is still considered to be
  one of the worst SSes in the game, with a maximum damage of about 300." `[CDW-pt]`, a Portuguese
  repost of an older guide: it passes any obstacle "(isso nao inclue mobiles mortos)" (not dead
  mobiles, **CONTRADICTION** with `[SW-mob]`), is "mais pesado do que o tiro 1" (heavier than S1,
  **CONTRADICTION** with `[WP06-list]`), and "continuará a voar até que bata seu oponente ou
  funcionar fora do mapa" (flies until it hits an opponent or leaves the map). `[DBR]`:
  `canMapCollide: false`, damage 250, crater 60 by 63.
- **The weak spot.** No source says where it is. `[CR-kex]`: "32* is the usual low angle which
  people use to shoot with nak. because it shoots at a low angle, it hits them in their weak spot,
  doing the most dmg." `[SW-mobC]`: a mobile's damage "depends on where a collision occurs relative
  to the hitbox". Trico's SS is best "if the shot hits an enemy's foot" `[SW-mob]`, and Trico users
  shotgun their SS at a floating Dragon's belly "because it seems to do more damage" `[WP06-list]`.
  `[DBR]` takes a player's damage off by the distance from the player's position.

## What lobshot built, and the picks

- **Rear fire.** Aim ranges past straight up aim over the back (`AimRange::rear`): `[CR-mob]`'s
  50–10 (true 45–15) is 130–170 (true 135–165) here, the SS 130–170 all true. The sim's launch
  needed no change; what reads an aim as an angle above level (the HUD, Up and Down, High Angle,
  the formula tests, the bots' facing) goes through `body::elevation` or `fires_rear`.
- **Weight.** Gravity is fitted midway between `[CR-fixed]`'s 2.70 bars and `[CR-nak]`'s 2.62 for
  60 degrees over one screen, fired over the back, so both hold within 0.05 (as Herdhound is
  fitted between its two sources). Wind sensitivity is fitted to `[CR-nak]`'s vertical wind term
  (14 up-wind over one screen at 60 degrees): 0.67, so its heavy shot is less affected by wind, as
  `[WP06-list]` says. `[DBR]`'s wind coefficient (`ay` 0.867 against Armor's 0.74, clone) points
  the other way (**CONTRADICTION**).
- **Movement.** 60 steps a turn, two thirds of everyone's 90 `[OB]`, for "very low mobility", and a
  10 px climb per step (6 for the rest) for "high climbing ability" `[SW-mob]`: both picks.
- **S1.** One round, 100–150, a blast of 20 (DBR's crater for it is about Armor's S2 warhead's).
- **S2.** The drill strikes a mobile it meets in flight as any round does (players "drop" it on a
  target, `[WP06-list]`; `[DBR]`'s passes through players, **CONTRADICTION**, GunBound first).
  Where it strikes the ground it bores on: down at the angle and speed it struck, the way it was
  heading, under its gravity turned upward, so through level ground it bores a mirror of its arc
  (`[WP06-list]`'s "reverse parabolic arc"), deeper and farther the faster it struck. `[DBR]` bores
  at the launch speed (**CONTRADICTION**; `[WP06-list]` ties it to the speed at the ground). It goes
  off, 190–300 with a blast of 20, at the first open pixel: on the surface, under a ledge, or in a
  mobile standing there (a direct hit). One whose bore leaves the map's sides or bottom is lost. No
  wind reaches it underground `[DBR]`, nor any weather column (a pick). It carves no tunnel. A
  floating mobile (Dragon's behavior) can't be struck by it directly, in flight or coming up
  (`shots::augerdillo::drill_strikes`), but its blast still reaches one.
- **SS.** A mini armadillo that flies through the ground as through air and goes off in the first
  living mobile it meets, either side's, the shooter's own included; one that meets none leaves the
  map. Lobshot clears the dead from the map, so corpses don't stop it, and there are no jewels or
  mines yet (M6, T-003). Its blast is 25, "a bit larger" than S1's `[WP06-list]`. Damage 350–500
  `[SW-mob]`, `[DB-mob]` (`[WP06-list]` says about 300 at most, **CONTRADICTION**). The weak spot is
  a pick from the clues above: the blast burns low (`damage::low`, as Thistleback's SS), 500 at the
  target's feet, 350 at the top of its body, where a lob strikes it; a low shot through the ground
  beside a target strikes it low, as `[CR-kex]` says.

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

`tool_fit` prints this table after it refits the gravity.

## Sources

- `[CR-kex]` Creedo's GunBound site, Kexeric's Armor guide: https://creedo.gbgl-hq.com/kexeric_armor_guide.php
  (undated).
- `[CDW-pt]` "Ponto Fracos Dos Móbiles [Gunbound]", cheatsdaweb forum, 9 May 2012:
  https://cheatsdaweb.forumeiros.com/t111-ponto-fracos-dos-mobiles-gunbound (a machine-translated
  repost of an English guide; low confidence).
- `[SW-mob]` read again at its 2025-09-19 archive, section "Nakmachine", and `[WP06-list]` at its
  2006-09-13 archive, sections "NakMachine (Hit Attack)" and "Dragon".
- `[DBR]` read at `HEAD` on 2026-10-24: `src/game/gametypes.js` (the Nak entry), `src/game/world.js`
  (`adddigger`, `checkCollision`), `src/game/game.js` (the rear muzzle), `src/game/lib/shoot.js`.
