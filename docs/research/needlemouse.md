# Needlemouse: GunBound's Knight

What the sources say about Knight, the hidden GunBound mobile whose behavior Needlemouse
(`sim::tuning::needlemouse`) carries, and what lobshot picked where they are silent. Gathered on
2026-10-24; every page was read as untrusted data. Knight is A.Sate's behavior with swords, so it
shares Lensowl's satellite (`lensowl.md`). Keys from `mechanics.md` (`[SW-mob]`, `[SW-mds]`,
`[CR-mob]`, `[CR-fixed]`, `[DB-mob]`, `[DBR]`, `[GF-tiro]`, `[GF-max]`), `weather.md` (`[GF-lee]`,
`[GF-vgg]`, `[GF-cyber]`, `[GF-kurai]`, `[NAMU]`), `thistleback.md` (`[WP06-list]`), `lensowl.md`
(`[GF-asate]`, `[SW06-mob]`, `[GC-db]`, `[CR-1sd]`, `[CR-3bar]`) and `README.md` (`[OB]`) mean what
they mean there.

## What the sources say

- **Random only.** GunBound handed Knight out only through Random, at 5% `[WP06-list]`, "about a 1
  in 30 chance" `[GF-kurai]`, "MUCH rarer then Dragon" `[GF-lee]`. Lobshot makes it directly
  selectable (PRD-0006), and Random can still roll it.
- **Stats.** "Mechanical - Laser", HP 1000, base delay 550, shots 250 / 400 / 840 `[SW-mob]`;
  `[GC-db]` and `[CR-mob]` agree. DragonBound gives it 1500 HP `[DB-mob]` (**CONTRADICTION**). "the
  Gunbound's original Korean site lists Knight as a Metalic type mobile" `[GF-lee]`; Machine
  `[GF-cyber]`, `[NAMU]`; Creature `[GF-tiro]` (**CONTRADICTION**). `[GF-lee]`'s delay totals (850
  / 950 / 1310) don't add up from 550 (**CONTRADICTION**).
- **Angles.** 80–10, all of it true `[CR-mob]`, `[GC-db]`, `[OB]`: "the largest true angle range
  in the game (100%)" `[WP06-list]`. `[DBR]`'s clone aims 20–60.
- **Weight.** "Knight's shot weight is tied with lightning at 4th lightest" `[WP06-list]`;
  `[CR-1sd]` gives the two the same full-power angle for a screen (80.5) and `[CR-3bar]` the same
  powers. "Heavy" `[GC-db]`, "Average" `[GF-lee]`. `[DBR]`'s `ax` puts it with Aduka (65.5).
- **Movement.** "GREAT mobility" `[GF-tiro]`, 4 of 5 `[GF-lee]`, "average mobility" `[SW-mob]`;
  "이동속도가 아주 낮은 편" (moves quite slowly) `[NAMU]` (**CONTRADICTION**). `[OB]` gives Knight 100
  steps a turn, every other mobile 90.
- **Its satellite.** "a sword that shoots several mini-swords at the guide, wherever it lands"
  `[SW-mob]`; "It has a sword looking thing hovering over its head, much like A.Sate's satellite"
  `[GF-lee]`; the swords are "검 모양의 미사일" (sword-shaped missiles) from the sword floating over the
  mobile, "이 때문에 지붕형 지형과 돌풍 앞에선 고전하게 된다" (so it struggles under roofs and in
  tornadoes) `[NAMU]`; "Hard to hit an enemy whos hiding underground, or hiding right beside a
  loop" `[SNX-knight]`. `[GF-lee]` once saw swords pass through terrain, and calls that a
  hypothesis. They converge: "all the projectiles sent right at the tracer in a reverse cone
  fashion, so it does just as well at long range as in close range" `[GF-asate]`; "the sword shots
  do not 'home in' on the tracer shot, they just repeatedly pound the spot" `[GF-lee]`.
- **S1.** Three swords `[NAMU]` ("several" `[SW-mob]`, "three or so" `[GF-lee]`, "Three sets of
  three" `[GF-cyber]`). "A little inaccurate like A.Sate Shot1" `[SW06-mob]`; "less accurate than
  A.Sate's Shot 1" `[SW-mob]`. Damage 200–250 `[SW-mob]`, `[DB-mob]`; 180–200 `[GC-db]`.
- **S2.** "Shoots twice as high as Shot1 with 5 mini-swords at the guide. It is more accurate than
  A.Sate's Shot 2" `[SW-mob]`; five `[NAMU]`; "about six", from over the landing `[GF-lee]`
  (**CONTRADICTION**). Damage 230–290 `[SW-mob]`, `[DB-mob]`; 250–300 `[SW06-mob]`. "VERY little
  bunging power" `[GF-lee]`.
- **SS.** "Just like A.Sate's SS, the sword moves right above where the guide lands and shoots many
  mini-swords down" `[SW-mob]`; "칼이 탄착점 위로 이동하여 7발을 발사한다" (the sword moves over the impact
  and fires 7) `[NAMU]`; "a barrage of burning swords are shot down vertically" `[WP06-list]`;
  "A.Sate's SS with more power but only 3/4 of it's bunging" `[GF-lee]`. Damage 400–550 `[SW-mob]`,
  `[DB-mob]`.
- **OpenBound's Knight** `[OB]`: S1 a carrier that does no damage; where it lands, 3 swords leave
  the satellite at the owner + (0, -150), 15 px apart across their line, and fly straight to the
  impact at 800 px/s, 80 damage each in 25 px; S2 the same with 5 from (0, -300); SS 7 from the
  impact + (0, -300) - (45, 0) · cos(60° · i), leaving 0.4 + i / 7.5 s after it lands. Wind
  influence 1.4 for Knight and Dragon, 1.2 for the rest.
- **Weather.** "A.Sate and Knight's shots fired through a hurricane will also circle around it
  before going on" `[SW-gp]`; see `[NAMU]` above.

## What lobshot does, and the picks

- **Stats:** GunBound's 1000 HP over DragonBound's 1500; Mechanical, Laser; 550 / 250 / 400 / 840;
  aim 10–80, all true; 100 steps (`[OB]`, with `[GF-tiro]`'s "GREAT mobility").
- **Gravity** fitted to Lightning's cbchui power, 2.30 bars `[CR-fixed]`, as every source that
  weighs Knight ties it with Lightning. Wind sensitivity 1.0: OB's 1.4 against 1.2 is a remake's
  number no GunBound source backs.
- **The pincushion** is Lensowl's satellite (`shots::satellite`): 150 px over Needlemouse's feet
  for S1, twice that for S2 and while the SS is picked `[OB]`, `[SW-mob]`.
- **Pins** are projectiles: each flies straight at the tracer at 800 px/s, untouched by gravity and
  wind, and lands like any shot on the first mobile or ground in its way (`[SNX-knight]`, `[NAMU]`;
  not `[GF-lee]`'s hypothesis), meeting the weather on the way. One still flying after 3 s is gone
  (a pick: a tornado or mirror can send a straight-flying pin off forever). Each has `[OB]`'s 25 px
  reach but a 12 px crater (a pick: Knight barely bunges, `[GF-lee]`).
- **S1:** three pins, 15 px apart across their line, converging on the tracer, aimed as Lensowl's
  S1 laser is (13 px high when they fly leftward, `[SW06-mob]`). 67–83 each: 200–250 split three
  ways (`[OB]`'s 80).
- **S2:** five pins from twice as high, straight at the tracer. 46–58 each: 230–290 split five ways.
- **SS:** the cushion glides over the tracer, 300 px up, and sweeps back and forth across it as its
  seven pins leave, each flying at the tracer, on `[OB]`'s timing. 57–79 each: 400–550 split seven
  ways.

## Sources

- `[SNX-knight]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/knight.html (Softnyx, official;
  archive 20200224035019)
