# J.Frog, for Sudsotter

GunBound's J.Frog, sourced, for lobshot's Sudsotter (PRD-0006 T-001). J.Frog came with GunBound
World Champion (Creedo calls it one of "the 2 new bots", with Kalsiddon), so it is missing from the
Classic tables (`[CR-mob]`, `[SW-mds]`, `[CR-fixed]`) and its sources are fewer. Pages were read on
2026-10-22; every quote is a fragment, and the numbers are the sources' own. Keys already defined
in `mechanics.md` (`SW-mob`, `DB-mob`, `WP06`, `CR-bot`, `DBR`), `weather.md` (`NAMU`) and
`README.md` (`OB`) are not repeated.

## Numbers

| What | GunBound | Others |
|---|---|---|
| Type | Bionic, Laser `[SW-mob]` `[SNX-frog]` | DB the same `[DB-mob]`; `[WP06]` lists J.Frog among the Hit types (**CONTRADICTION**) |
| HP | 930 `[SW-mob]` | DB 930 `[DB-mob]`, OB 930 `[OB]` |
| Delay (base / S1 / S2 / SS) | 500 / 250 / 250 / 800 `[SW-mob]` | DB the same `[DB-mob]`; `[CR-bot]` 430 / 250 / 250 / 960 "(not sure)" (**CONTRADICTION**) |
| Damage (S1 / S2 / SS) | 150-230 / 150-230 / 100-700 `[SW-mob]` | DB the same; `[CR-frog]` "typical" 150-210 / 150-210 / 400-500 |
| Aim | none found (OB marks it "NEED SOURCE") | DBR 30 to 60, marked "@TODO falta definir" (still to be settled) `[DBR]` |
| Weight | "very very light... lighter even than boomer's"; cbchui-style powers "from 1.95 (half screen shots) to 2.1 (1 full screen). I haven't confirmed if those numbers are perfect but they're close"; Banpao "between 2.5 and 2.6 bars of power (untested)" `[CR-frog]` | DBR gives Frog Grub's `ax`, 61 (clone, **CONTRADICTION**) |

- Creedo finds the delay "super low, the lowest in the game... like 670 or something" for a shot,
  but saw a display bug and "can't say for sure" `[CR-frog]`. 430 + 250 = 680 is `[CR-bot]`'s.
- StrategyWiki and DragonBound agree on every number, as they do across the roster (DB's guide copies
  StrategyWiki's text, `mechanics.md`).

## Shot 1 and Shot 2: the walking sponge

- **Through mobiles, then a walk.** "A green sponge that when it hits land (it goes through
  players), it starts rolling to the left for three seconds and then explodes" `[SW-mob]`. Creedo:
  the shots "travel through the air normally... then start 'walking' as soon as they hit the
  ground... they don't explode when they touch an enemy, instead they keep walking. After walking
  for about 3 seconds, they explode" `[CR-frog]`. `[WP06]`: "it has a set detonation time... won't
  detonate because a player hits the target directly". The official page: the Jelly "does not
  explode on impact, but explodes after a certain period" `[SNX-frog]`.
- **Left and right are the screen's, not the shooter's.** "Frog's shot 1 always walks left when it
  hits the floor, and shot 2 always walks right. So if you're facing right and use shot 1, then it
  will hit the ground and immediately start moving backwards (towards you)" `[CR-frog]`.
  `[WP06]`: S1 "starts crawling counterclockwise (left)", S2 "crawls clockwise (right)". The official
  page says to "select your weapon by calculating whether your enemy is on the right side or left
  side of the weapon's landing position" `[SNX-frog]` (it also speaks of a spin "to make it roll
  either forward or backwards"). DBR's walkers carry `dir: 1` and `dir: -1`. `[GBM-frog]`, a later
  version, makes S1 roll "away from the direction of the shot" and notes that the in-game text still
  says clockwise; that is a later change, not the baseline.
- **It climbs.** "It has unlimited climbing and can cling to the underside of ledges... it moves up
  and down a bit slower than it moves horizontally" `[CR-frog]`; "a moderately powered blob of jelly
  that climbs walls" `[WP06]`. Its walk "is based on time, not distance": "climbing up or down slopes
  means the shot has less horizontal walking to do before it explodes" `[CR-frog]`.
- **How far.** "If the land is pretty flat, I plant the shot about 1/7th of a screen away from the
  intended target - about 4 parts out of a 30-part screen" `[CR-frog]`: about 107 px in 3 s.
- **Under a mobile** "the rolling shot tends to slow down, so it handles easier than it looks"
  `[SW-mob]`.
- **The blast** "makes a pretty nice hole like mage shot 1" `[CR-frog]`.
- **Weather, in flight and after.** "Once your shot 1 or 2 hits the ground, it ignores tornados. It
  moves through the nado like all other shots do when it's airborne" `[CR-frog]`; "it will be
  affected by hurricane if the shot has not landed yet... but when it is crawling it walks right
  through the hurricane" `[WP06]`.
- **Duals.** "You want the first shot to blow up a little late because it will make a hole and
  'slow down' the second shot" `[CR-frog]`.

## SS: the walking suds

- **Pops on a direct hit.** "Don't hit the opponent with the SS, or it will explode on contact,
  doing little damage" `[SW-mob]`.
- **Hurts by touch.** "Is like the grub's SS because the entire time it is touching a player, it
  does damage to that player" `[SW-mob]`; "does damage as long as it is in contact within about
  1/2-1cm of the mobile" `[WP06]`; "continuously emits energy causing constant damage to all mobiles
  around it" `[SNX-frog]`; "when it passes through the enemy it does damage" `[CR-frog]`.
- **Which way.** "If you shoot SS left, but wind blows right, the bubble will eventually fly
  backwards and roll to the RIGHT, not the left" `[SW-mob]`; "its direction is determined by either
  how hard players shoot it and in which direction, also it is determined by which way the wind is
  blowing" `[WP06]`; "walks in the direction it's fired" `[CR-frog]`; it "will roll forward"
  `[SNX-frog]`; it goes forward "regardless of direction" (방향에 상관없이) `[NAMU]`.
- **How long.** "It just keeps walking for a long time" `[CR-frog]`. No source gives a time.
- **Where it shines.** "Try to hit an enemy on a floating speck of land. Then the SS will walk in
  circles around that speck and pass through the target several times for very high damage"; walk
  it through two weak enemies for a double kill; "Otherwise, screw it. It's nearly useless"
  `[CR-frog]`. "It is best to shoot a tiny piece of land that your opponent is on" `[SW-mob]`;
  "especially effective on mobiles on small floating terrains" `[SNX-frog]`. `[WP06]`: it "isn't
  powerful at all unless... the player's opponent is stuck on a very small piece of land".

## Weather and damage (CONTRADICTION)

- `[WP06]`: J.Frog "is not affected by Thor, Thunder, or any of the other weathers that affect
  damage".
- `[CR-frog]`: "Weather affects the shot when it blows up, not when it lands, so don't be scared to
  use force/lightning/thor with frog."
- `weather.md` §3 takes `[WP06]`'s side; lobshot does too (`sim::tuning::sudsotter::WEATHER_PROOF`),
  for Force, Drain and Electricity's bolt. Mirror and Tornado still turn its shots in flight, which
  both sources describe.

## Fitting its weight

Sudsotter's gravity is fitted like the Classic mobiles' (`physics-fit.md`), to `[CR-frog]`'s
cbchui-style figure: 60 degrees lands one screen at 2.1 bars, which gives 466.6 px/s² (Coilback's
is 622). `[CR-frog]`'s other two figures then hold unfitted, and are acceptance tests
(`crates/sim/tests/formula.rs`): 75 degrees, half a screen, 1.95 bars (the model needs 1.91), and
Banpao's 60 degrees, a screen and a half, "between 2.5 and 2.6" (2.61). DBR's `ax` would make it
as heavy as Grub's 2.33 (**CONTRADICTION**), which none of Creedo's figures fit.

## Sources

- `[CR-frog]` https://creedo.gbgl-hq.com/basic_frog_and_kal.php (Creedo, "Basic J.Frog and
  Kalsiddon use"; GunBound World Champion era)
- `[SNX-frog]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/jFrog.html (official; archive
  20200224035000)
- `[GBM-frog]` https://sites.google.com/view/gbmessentials/home/mobiles-guide/j-frog (GBM
  Essentials; a later version of the game, its in-game text checked 2019-02-17)
