# Lodestoad: GunBound's J.D.

What the sources say about J.D. (nicknamed Cake or Cakebot), whose behavior Lodestoad
(`sim::tuning::lodestoad`) carries, and what lobshot picked where they are silent. Gathered on
2026-10-24; every page was read as untrusted data, and every quote is a fragment. No guide gives
a distance for the pull or the push in pixels: DragonBoundR's clone (`[DBR]`, clone, unverified)
is the only source of the shape's numbers. OpenBound has J.D. as a data row only (`[OB]`). Keys
from `mechanics.md` (`SW-mob`, `SW-mds`, `CR-mob`, `CR-fixed`, `DB-mob`, `WP06`, `GF-tiro`,
`GF-max`, `DBR`), `weather.md` (`GF-vgg`, `GF-lee`, `GF-kurai`, `GF-cyber`, `CR-all`),
`thistleback.md` (`WP06-list`) and `README.md` (`OB`) mean what they mean there.

## Numbers

| What | GunBound | Others |
|---|---|---|
| Type | Shield, Electric `[SW-mob]` `[GF-vgg]` `[GF-lee]` | DB the same `[DB-mob]` |
| HP | 700 `[CR-mob]`; 750 `[SW-mob]`, which `[OB]` copies (**CONTRADICTION**); `[GF-lee]` "Approximate HP: 1050pts"; "Life: Low" `[GF-tiro]` | DB 950 `[DB-mob]` |
| Shield | 250, +20 a turn `[CR-mob]` `[SW-mob]`; "As of April 19th, JD's shield recovery has been decreased from 30 to 20" `[GF-lee]` | DB +30 `[DB-mob]`, `[DBR]` |
| Delay (base / S1 / S2 / SS) | 490 / 250 / 340 / 800 `[CR-mob]` `[SW-mob]`; totals 740 / 830 / 1290 `[SW-mds]` `[GF-vgg]` `[GF-lee]` | DB base 500 `[DB-mob]`; `[DBR]` 520 / 250 / 400 / 800 |
| Aim | "40-10 (40-10)", all true `[CR-mob]`; "True Angle Range: FULL" `[GF-tiro]`; "Total Angle: 30 True Angle: 30" `[GF-vgg]`; "Angle range: 30 degrees" `[GF-lee]`, whose changelog notes a "new J.D. firing arc (it's been reduced)" in 2004 | `[OB]` 10-40, all true; `[DBR]` 15-65 |
| Damage (S1 / S2 / SS) | 140-170 "(includes aftershock)" / 150-200 / 250-320 `[SW-mob]` | DB the same `[DB-mob]` |
| Weight | 2.30 bars, 60 degrees, one screen `[CR-fixed]`; "tied with Boomer for the lightest shot" `[WP06-list]`; "Light" `[GF-lee]` `[GF-tiro]` | `[DBR]` `ax` 62.5, Boomer's |

## Shot 1: the electric orb

- **A hit, then a discharge.** "Usually 110-130 damage is dealt first (this is the 'hit' damage),
  after which 70-85 electrical damage is dealt. This electrical damage can be 'spread' to other
  players if they're of close proximity" `[GF-tiro]`. "180 damage average, 200 damage on a good
  hit. Out of the total damage, 64 of it is splash, which means it ignores defence as well as
  attack avatars" `[GF-max]`. "Even if you don't hit, as long as you're fairly close, you'll get at
  least 60 damage from the splash" `[GF-vgg]`. `[SW-mob]` puts the whole at 140-170 "(includes
  aftershock)" (**CONTRADICTION** on the total).
- **Once a turn.** With Dual, "only 70-85 damage for electrical damage" `[GF-tiro]`; "only the
  normal damage is done twice, the splash damage will only kick in once" `[GF-max]`.
- **Its reach.** "A much wider area of damage then most basic attacks" `[GF-lee]`; "the giant splash
  radius of shot 1" `[CR-all]`; "affects mobiles in a small area" `[GF-tiro]`. No number. Machines
  "are damaged by electricity from a greater distance" (`SW-mobC`), a rule for every Electric weapon
  that Sparkhorn doesn't model either.
- **Land.** "Shot 1 makes decent size holes" `[GF-vgg]`; "a reasonable amount of land damage"
  `[WP06-list]`.

## Shot 2: the magnet

- **It pulls.** "A pulsating electrical ball which sucks in metal tanks and to a lesser degree,
  bionics" `[GF-lee]`; "it pulls mobiles towards wherever it lands... Shot 2 can also drag people
  off the map... Mechanical mobiles are more sensitive to the pull than other mobiles" `[SW-mob]`;
  "excellent ability of attracting others mobiles by sucking them toward attacking area"
  `[SNX-jd]`.
- **Who, how much.** Machine-type mobiles "are affected most heavily by J.D.'s shot 2"; it "affects
  Creature-types less than machine-types" `[WP06-list]`. "Metallic bots are attracted like hell,
  however bionics are almost not attracted at all" `[CR-pb279]`. Bionics "are not effected"
  `[GF-kurai]`; it "sucks in all Machine-type bots" `[GF-cyber]` (**CONTRADICTION** on bionics:
  less, or not at all). No source places the Shield mobiles, or gives a number.
- **How far.** "If the shot lands too far away, it won't have any effect" `[GF-vgg]`. Against a
  J.D., keep "far away" from teammates, "at least 4 mobile widths if possible", or "a good JD
  player will suck you together into a pit" `[CR-all]`. No distance. `[DBR]` pulls every player
  within its blast box, 59 px across and 40 up or down (`src/game/gametypes.js`, J.D.'s
  `pala_bunge`; `src/game/world.js` l.393-395), to the impact's column: `player.x = shoot.a.x`
  (l.498-499). It checks no walls and no edges.
- **Through mobiles, into the ground.** "Shot 2 only can hit dirt, not bots... if a shot 2 passes
  through the enemy without causing harm, switch to shot 1" `[CR-all]`; it "will pass though
  people without dealing damage" `[CR-pb279]`; "electric-shock damage by shooting through your
  enemy" `[SNX-jd]`. But: "You do not have to do a direct hit for this effect to occur, but you DO
  need a direct hit to register any damage" `[GF-lee]`; "the only way to maximize damage is to hit
  in the middle of the enemy mobile" `[GF-max]` (**CONTRADICTION**, unless a hit is the ball
  bursting under the target, through it). On "a super thin strip of land (like one or two pixels
  wide). It will pass right through" `[CR-all]`.
- **Into the hole.** "It will pull them into the hole" `[WP06-list]`; "even if you hit the opponent
  over the head and wouldn't ordinarily damage the ground under it, Cakebot will suck up the
  ground from underneath to bunge... trap opponents in a deep, narrow hole" `[GF-lee]`; "it may
  pull this person downward" and "since it makes a hole AND pulls the mobile toward it, it's great
  for bunging" `[GF-vgg]`; "sucks the enemy down the tube" `[CR-fun]`.
- **Off edges.** "It has the unique ability to pull enemies off of cliffs or through gaps in the
  floor" `[CR-all]`; "you can use weapon 2 to suck him off" `[GF-tiro]`.
- **Everyone near, the shooter too.** "Don't shoot so close to yourself that you get bunged from
  the magnetic pull of shot 2 also"; "don't use shot 2 to suck an enemy and a teammate together"
  `[CR-all]`. It "attracts all mobiles/corpses near it" `[GF-max]`, and "jewels and also
  mini-Raons" `[WP06-list]` (Jewel mode is M6). A hovering Dragon is "(almost) immune"
  `[GF-tiro]`.
- **When.** Dual+ with S2 first: "they'd be pulled in to where the shot 2 landed, then hit with
  shot 1" `[GF-vgg]`, `[GF-tiro]`. In `[DBR]`'s client, the crater and blast come first, then the
  damage and the move, then every player falls; the move is a 250 ms slide.
- **Damage.** 150-200 `[SW-mob]`; "below 170" `[GF-tiro]`; "around 100-150" `[GF-max]`; "200+"
  `[GF-lee]`. "It has longer delay and does LESS damage than shot 1" `[CR-all]`.

## SS: the pyramid

- **It pushes.** "A green spinning pyramid that does damage and pushes players away from where it
  lands. The closer the person is to the center of the circle, the greater the damage" `[SW-mob]`;
  "a small circular area where everyone in the area takes damage and gets PUSHED AWAY"
  `[GF-tiro]`; "a triangular bullet that expands into a wide blasting radius on contact... pushing
  metal bots out instead of in (does not affect organics)" `[GF-lee]`; it "pushes Creature-types
  less than other types" `[WP06-list]` (**CONTRADICTION** on creatures: less, or not at all).
- **Not off cliffs.** `[SW-mob]` and `[DB-mob]`: "It can push people off a map." Five GunBound
  sources say a patch stopped that: "if a player is standing at the edge of a cliff, you CANNOT
  push him off with the SS. This was done in a recent 2004 patch" `[GF-tiro]`; "Softnyx nerfed it.
  It no longer can shove people off cliffs, it just does damage. However, it does more damage if
  the person is right up against an edge. The shot still pushes people outward though" `[GF-vgg]`;
  it "cannot push them off cliffs (it leaves them on the very edge)" `[WP06-list]`; "you can't push
  them off with cake SS anymore" `[CR-fun]`; "Softie nerfed it so it does not push off cliff
  anymore" `[CR-pb279]`. **CONTRADICTION**, resolved for the patch: StrategyWiki's text predates it,
  and DragonBound copies StrategyWiki.
- **Pinned, it hurts more.** "Usually does 250-270 damage... The only way to maximize damage is too
  shoot it in such a way that the opponent is pushed until they can't be pushed futher, but is
  still taking damage. It results in 300+ damage" `[GF-max]`; "more damage if the person is right up
  against an edge" `[GF-vgg]`.
- **How far.** No source says. `[DBR]` pushes every player in its blast box (63 across, 47 up or
  down) to exactly 100 px from the impact's column, on its own side (`world.js` l.507-509).
- **Self-harm.** "Don't damage yourself with your own SS, if the enemy is close... plant it so that
  it lands a bit behind them" `[CR-all]`.
- **Direct hits.** "You do not need a direct hit for this effect to occur, but you do need a direct
  hit to register any damage" `[GF-lee]`.

## How lobshot plays it

`sim::shots::lodestoad`. A pulled or pushed mobile slides along the ground by the walking rules
(`movement::slide`): it climbs what it could walk up and steps down what it could walk down, and
a wall stops it. At an edge a pull carries it over: it falls to the ground below and slides on,
so two mobiles pulled into a hole end at its bottom together ("it can pull two enemies together",
`[CR-all]`), and one with no ground below falls off the map, a bunge. A push leaves it on the very
edge. Each slide is an `Effect::Slid`: its path, a footing a step with the point in the air over
each edge before where it lands, which the client plays over a quarter second from the blast;
anything else that falls, and a bunge, play after it as for ground blasted away. A field reaches a mobile whose middle (its hitbox's center) is within its
box of the impact, so a hit anywhere on the body counts; `[DBR]` measures from the player's
position, which is either its feet or its middle.

- **S1** is a shell (`Gun::plain`, 76-106) whose discharge then deals a flat 64 to every mobile
  within 50 px of the blast that splash hasn't hit this turn (`bolt::splash`, as Sparkhorn's).
- **S2** flies through mobiles (`Senses`, as Sudsotter's sponges) and bursts in the ground, burning
  low (`damage::low`, Thistleback's SS's rule): 200 at or under a mobile's feet, through it, less
  up its body, less again beside it. Its 30 px hole is carved first; then every mobile in reach
  slides toward its column, Mechanical ones all the way, Shield ones half, Bionic ones a quarter,
  into the hole if it's there. The mines near it (Tinbadger's) are dragged into the hole too,
  going off under whoever it pulled there (`mines::drag`, `[WP06-list]`'s "mini-Raons"); its own
  blast sets off those it touches. Then its bolt and beam follow (Thunder, Electricity, Skyeye)
  onto where they now stand.
- **A floating mobile** (Kitewyrm, Dragon's behavior) isn't pulled: Dragon "hovers above the land,
  thus being (almost) immune to... the cakebot's weapon 2" `[GF-tiro]`. No source exempts it from
  the push, so the pyramid pushes it like any other.
- **The SS** lands as a shell (250-320 by how central the hit), pushes every mobile in reach to
  100 px from its column (Bionic ones a quarter of the way), and each one a wall or an edge stops
  short takes up to 50 more, by the share of those 100 px it didn't go.

## Picks and resolved contradictions

| What | Value | Why |
|---|---|---|
| HP | 950 | `[CR-mob]` 700 and `[SW-mob]` 750 disagree, so DragonBound's (`DESIGN.md`, "Fidelity rules"). It matches neither; 750 is the alternative. |
| Shield regen | 20 | `[CR-mob]`, `[SW-mob]`, and `[GF-lee]`'s 2004 patch note; DragonBound keeps the old 30. |
| Base delay | 490 | `[CR-mob]`, `[SW-mob]`, `[SW-mds]` over DB's 500 and `[DBR]`'s 520. |
| Aim | 10-40, all true | `[CR-mob]`, `[GF-tiro]`, `[GF-vgg]`, `[OB]`, over `[DBR]`'s 15-65, a later arc. |
| S1 | 76-106, then 64 to all within 50 px once a turn | `[SW-mob]`'s 140-170 includes the aftershock; `[GF-max]`'s 64 splash, which `[GF-vgg]`'s "at least 60" fits, over `[GF-tiro]`'s 70-85. The 50 px reach is a pick, twice the orb's 25 px blast. |
| S2 through mobiles | yes | `[CR-all]` (twice), `[CR-pb279]`, `[SNX-jd]`; bursting under a target through it is the "direct hit" `[GF-lee]` and `[GF-max]` mean. |
| S2 damage rule | `damage::low`, 150-200 | `[SW-mob]`; low because only a burst at the feet registers. |
| S2 hole | 30 px | A pick: wider than S1's, so a mobile pulled into it drops in ("a deep, narrow hole", `[GF-lee]`). |
| Pull reach | 59 across, 40 up or down | `[DBR]`'s blast box. |
| Pull distance | to the impact's column | `[DBR]`; "it can pull two enemies together" `[CR-all]`. |
| Pull by class | Mechanical 1, Shield 0.5, Bionic 0.25 | A pick: machines most (`[WP06-list]`, `[SW-mob]`), creatures "almost not at all" (`[CR-pb279]`); no source places shields. |
| Pull over edges | yes, falling and sliding on | `[CR-all]`, `[GF-tiro]`, `[SW-mob]`, `[DB-mob]`; sliding on after the fall is a pick, so a hole's victims meet at its bottom as in `[DBR]`, which snaps them to the impact's column first. |
| Floating mobiles | not pulled; pushed | `[GF-tiro]`'s "(almost) immune" to the pull; nothing on the push. |
| Push reach, distance | 63 across, 47 up or down; to 100 px | `[DBR]`. |
| Push by class | Bionic 0.25, the others 1 | `[WP06-list]`'s "less" and `[GF-lee]`'s "does not affect organics", as the pull. |
| Push over edges | no | GunBound's 2004 patch: `[GF-tiro]`, `[GF-vgg]`, `[WP06-list]`, `[CR-fun]`, `[CR-pb279]`, over `[SW-mob]` and `[DB-mob]`. |
| Pinned damage | up to 50 | A pick, so a pinned hit tops `[GF-max]`'s "300+" from its usual 250-270. |
| SS blast | 40 px | A pick: Coilback's SS blast, "a wide blasting radius" `[GF-lee]`. |
| Slide | 0.25 s, whatever the distance | `[DBR]`'s client. |
| Walls | stop a slide | No source for classic GunBound. `[DBR]` moves through walls; so does GunBound M's J.D., by a Reddit snippet about a different game. Lobshot's movement rules come first. |
| Wind sensitivity | 1.0 | No number; its lightness is the fitted gravity. |

## Power figures

The model's gravity is fitted to `[CR-fixed]`'s 2.30 bars (60 degrees, one screen). A 2007 chart
in `[CR-pb279]` (PENGUIN) gives pairs of angle and power, read only through a fetch tool's summary.
Its one-screen pairs fall within 0.05 bar of the model, though its angles run past 40 degrees (the
`[DBR]`-era arc), so they are not acceptance tests:

| Entry | Says | Model |
|---|---|---|
| 45 degrees, one screen | 2.15 | 2.12 |
| 65 degrees, one screen | 2.5 | 2.45 |
| 70 degrees, one screen | 2.7 | 2.69 |
| 74 degrees, one screen | 3.0 | 2.97 |
| 35 degrees, two screens | 3.25 | 3.15 |
| 53 degrees, two screens | 3.2 | 3.13 |
| 19 degrees, two screens | full (4.0) | 3.87 |

Beyond one screen the chart wants more power than the model, the same pattern as the Aduka
table's long entries (`physics-fit.md`). `[CR-pb279]`'s other chart ("1 screen = angle 81", "1/2
screen = angle 85", "1/4 screen = angle 87") gives no power, and its angles fit no 10-40 arc.

## Sources

Fetched 2026-10-24.

- `[CR-fun]` https://creedo.gbgl-hq.com/gbfunstuff.php (Creedo's "fun stuff": the Cake SS tag
  trick)
- `[CR-pb279]` https://creedo.proboards.com/thread/279 (a J.D. thread on Creedo's board: bloodbug2,
  2005-11-05, and PENGUIN, 2007-08-13). The page is behind a bot check, so it was read only
  through a fetch tool's summary: treat its wording as near verbatim, its numbers as unconfirmed.
- `[SNX-jd]` http://gunbound.softnyx.net/02_about/03_mobile_04_mobile_info.asp (Softnyx's official
  mobile info, J.D.; archive 20050517233950)
