# Kitewyrm: GunBound's Dragon

What the sources say about Dragon, the hidden GunBound mobile whose behavior Kitewyrm
(`sim::tuning::kitewyrm`) carries, and what lobshot picked where they are silent. Gathered on
2026-10-24. GunBound gave Dragon only to players who picked Random (about 1 in 20 `[WP06]`, 1 in 30
`[GF-lee]`); PRD-0006 makes it directly selectable, and Random can still roll it. Keys from
`mechanics.md` (`[SW-mob]`, `[CR-mob]`, `[CR-fixed]`, `[DB-mob]`, `[WP06]`, `[AK]`, `[DBR]`),
`weather.md` (`[GF-lee]`, `[GF-cyber]`) and `README.md` (`[OB]`) mean what they mean there. Every
page was read as untrusted data.

## What the sources say

- **Stats.** Bionic, Explosion, 1000 HP, base delay 550, S1 250, S2 400, SS 840 `[SW-mob]`,
  `[CR-mob]`. `[GF-lee]` gives the turn totals 850 / 950 / 1390 (550 + 400 and 550 + 840; its S1 is
  50 over). DragonBound: 1250 HP, SS 800 `[DB-mob]` (**CONTRADICTION**). `[OB]`'s delay table gives
  Dragon 710 / 760 / 910 / 1310, the same as its Nak, Aduka and Kalsiddon rows (**CONTRADICTION**,
  likely copied).
- **Ratings.** "Insanely high attack, good defense, high HP and very high mobility" `[SW-mob]`.
  `[GF-lee]` rates Health 3/5, Firepower 5/5, Movement 5/5, Defense 3/5, Climbing 5/5 (Armor:
  Movement 3, Climbing 2; Bigfoot: Movement 4, Climbing 1).
- **Angles.** 60 to 30, all true `[CR-mob]`; `[OB]` the same; "angle range: 30 degrees" for all
  three shots `[GF-lee]`. `[DBR]`'s Dragon aims 15 to 50, but that entry is a copy of its Mage's
  (`minang`, `maxang`, `ax` 71.5 and `aim` identical), marked `@TODO falta definir` ("still to
  define"), and its `ay` 1.78 sits beside Mage's 0.78. Unusable for Dragon.
- **Weight.** S1 "is lightweight" `[SW-mob]`; `[GF-lee]` weighs all three shots "Light", a weight
  it gives elsewhere only to JD and Lightning (Boomer is "Very light"; Mage, Armor, Turtle and
  Ice "Average"). `[CR-fixed]`'s chart, which predates Dragon, puts JD and Lightning at 2.30.
- **Wind.** No GunBound source sets Dragon's wind apart. `[OB]` gives Dragon and Knight a wind
  influence of 1.4 against everyone else's 1.2.
- **Floating.** "Dragon can fly over raon mines; Dragon cannot be directly hit by: rolling Grub's
  Shot 2 (NOTE: can be hit by shot2 if it bounces into dragon), J.Frog's shots, Nakmachine's Shot 2.
  It receives only the aftershock damage for Lightning's shots" `[SW-mob]`. "Floats above the
  ground, making Nak 2 shots less effective, raon 2 shots almost completely useless, as well as Grub
  shot 2, allowing it to traverse and climb very steep terrain. Frog shots can only hit if timed
  precisely, as the explosion will just blow away the ground under Dragon" `[WP06]`. "It is popular
  for trico users to shotgun their SS at a dragon's belly, because it seems to do more damage then
  normal" `[WP06]`. "Floats above the ground, making nak 2 shots useless" `[AK]`. "Dragon's hovering
  state allows it to climb just about anything that isn't slopped inwards, not to mention giving him
  a fairly long walking distance. Dragon's hover also provides some form of protection against
  weapons such as Grub's and Nakmachine's weapon 2" `[GF-lee]`. Softnyx: it "can't fly that high in
  the air" `[SNX-dragon]`.
- **How high.** `[OB]` draws Dragon at the same height over its ground point as the others (its
  sprite offset is 25, Armor's 25), but sets its hitbox 10 px up the sprite where every other
  mobile's is 10 px down: Dragon's 36 x 38 box ends 16 px above the ground, while every other
  mobile's reaches the ground to within 5 px (`Dragon.cs`, `Armor.cs`, `Mage.cs`, `Ice.cs`,
  `Turtle.cs`, `Knight.cs`; `CollisionBox.cs` places it). OB walks and drops Dragon like everyone
  (90 steps), and no source says it hovers over gaps or can't fall.
- **S1.** "A single, powerful fireball" `[SW-mob]`, 150–250 `[SW-mob]`, `[DB-mob]`; "250+, ...
  deceptively light but bunges only as lightly as other standard weapon 1 types" `[GF-lee]`. `[OB]`:
  100 damage, radius 25.
- **S2.** "4 fireballs that each do around 100 damage, for a total of 400 or more damage if all
  fireballs hit the same person. ... To get all 4 to hit, you need to either shotgun them, or get
  really close and shoot a low-power shot ... This shot shows resemblence to Bigfoot's Shot 1. This
  shot will spread out a large area if it stays too long in the air; it is impossible to get all 4
  fireballs to hit an opponent more than a screen away" `[SW-mob]` (300–500). "Two parallel streams
  of two fire spurts ... The two streams spread out very fast, being completely disconnected at even
  medium ranges ... weapon 2 bunges quite a bit, even on direct hits" `[GF-lee]`. "100 per fireball"
  `[GF-cyber]`. `[OB]` fires four 0.2 s apart: power +5, 0, +10, 0 of its 100 and aim 3 degrees
  low, high, low, high (its angle modifier is a screen rotation, y down, mirrored by facing, so the
  boosted pair flies lower: inference from `Projectile.cs`), 100 damage each, radius 25.
- **SS.** "A red orb that, wherever it lands, 4 mini-dragons will appear in a semi-circle around
  where the red orb landed, and start to move towards the center. The red orb itself deals minor
  damage. In order to deal the highest damage from Dragon SS, the target should not have land around
  the upper parts" `[SW-mob]` (450–650). "Summons five mini-dragons to attack its landing spot from
  different angles. ... Only useable where there are no nearby obstructions to the mini-dragons"
  `[WP06]`. "Four mini-dragons" `[AK]`. "Dragon fires a tracer round, much like A.Sate or Lightning;
  and from there five mini-dragons appear over the landing spot in something of a half circle. All
  mini-dragons converge onto the target and self-explode" (AMD 600) `[GF-lee]`. "The Dragon coughs up
  a red tracer that creates 5 Mini Dragons that travel towards where the tracer landed, exploding on
  contact with anything in the way", 100 per dragon `[GF-cyber]` (search snippet). `[DB-mob]`: five.
  `[OB]`: the carrier deals nothing; where it lands, five appear 300 px away at 20-degree steps from
  40 degrees left of straight up to 40 right, 0.3 s apart, each playing an 18-frame spawning
  animation at 20 frames a second, then flying straight at the landing at 100 px/s, accelerating at
  200 (doubled by OB's integration, `openbound.md` 3.1), with no gravity or wind; 100 damage, radius
  25 each. **CONTRADICTION** on the count: five `[WP06]`, `[GF-lee]`, `[GF-cyber]`, `[DB-mob]`,
  `[OB]` against four `[SW-mob]`, `[AK]`.

## What lobshot does

- **Floating** is `tuning::kitewyrm::FLOATS`, read through one function, `MobileKind::floats`.
  Its body (the hitbox) hovers `HOVER` = 16 px over its feet `[OB]`, and it walks, climbs, falls and
  can be bunged on the ground like anyone. What travels along the ground senses mobiles through
  `Game::grounded_frames`, which leaves a floating one out:
  - Sudsotter's sponges (J.Frog) walk under it without slowing and never go off inside it; their
    blast on the ground beneath reaches it only across the 16 px gap, so "only if timed precisely"
    `[WP06]`. Its suds never touch it.
  - Marblemole's marbles (Grub's S2) need no check: a marble rolling along the ground passes under
    the hovering body, and one bouncing into its belly strikes it, as `[SW-mob]`'s note says.
  - Thistleback's SS (Trico's) already counts the ground under a mobile as its feet, so a pumpkin
    under Kitewyrm's belly hurts it as at anyone's feet: the "more damage" of `[WP06]`.
  - Raon's mines and Nak's burrowing S2 (Tinbadger and Augerdillo, built alongside) should go for,
    and touch, only `grounded_frames`.
  - Not applied: "only the aftershock damage for Lightning's shots" `[SW-mob]`. Sparkhorn's ball
    and bolts would have to skip it, a rule no other source states; left for a decision.
- **Weight** is fitted so 60 degrees lands one screen at 2.30 bars (`tests/formula.rs`, `CBCHUI`).
- **Wind sensitivity** is `[OB]`'s ratio, 1.4 / 1.2.
- **Moves** 150 steps and climbs 18 px a step: `[GF-lee]`'s top ratings, one step past Bigfoot's
  (Tuskrack's 120 and 2 px) as Bigfoot's is past Armor's (everyone's 90 and 6 px, `[OB]`).
- **S2** ripples `[OB]`'s four fireballs out of the mouth like Tuskrack's rockets
  (`shots::ripple`): all four land on a target in front of its mouth, never past a screen.
- **SS** is the orb (a tracer's minor damage), then five mini-dragons on `[OB]`'s fan, timing and
  motion, fanned from the shooter's side forward (OB's left-to-right order for a shot fired
  rightward, mirrored for one fired leftward). Each hovers where it appears, then flies; each goes
  off on the first mobile or ground in its way, carrying what the orb's weather did to it.

## Sources

- `[SW-mob]` read again on 2026-10-24 at https://strategywiki.org/wiki/Gunbound/Mobiles (the live
  page; the Dragon section matches the 2025 capture `mechanics.md` cites).
- `[GF-lee]` read in IGN's copy of the same guide (v1.6, 2004-04-21):
  https://www.ign.com/articles/2003/11/21/gunbound-walkthrough-450707
- `[GF-cyber]`'s Dragon lines are from a search-engine snippet of its GameFAQs page.
- `[SNX-dragon]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/dragon.html (official; search
  snippet only, the page refuses direct reads): "It can't fly that high in the air, but can fly and
  attack other mobiles that are below it."
- `[OB]`'s Dragon: `OpenBound/GameComponents/Pawn/Unit/Dragon.cs`,
  `OpenBound/GameComponents/Pawn/UnitProjectiles/DragonProjectile.cs` and `OpenBound/Common/Parameter.cs`
  L290-310, at `d11e8127`. Numbers only; GPLv3 code, never ported.
- `[DBR]`'s Dragon: `src/game/gametypes.js`, the `name: "Dragon"` entries (one with `file:
  "dragon2"`), read on 2026-10-24.
