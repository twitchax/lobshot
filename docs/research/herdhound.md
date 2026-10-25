# Herdhound: GunBound's Kalsiddon

What the sources say about Kalsiddon, the GunBound World Champion mobile whose behavior Herdhound
(`sim::tuning::herdhound`) carries, and what lobshot picked where they are silent. Gathered on
2026-10-22. Kalsiddon came late (GBWC), so the sources are thinner than for the Classic roster, and
OpenBound has it only as a data row (`openbound.md`). Keys from `mechanics.md` (`[SW-mob]`,
`[DB-mob]`, `[CR-bot]`, `[CR-mob]`, `[AK]`, `[OB]`, `[DBR]`) mean what they mean there.

## What the sources say

- **Stats.** Mechanical, Explosion, 1100 HP, base delay 515, S1 250, S2 460, SS 800 `[SW-mob]`;
  DragonBound copies the same `[DB-mob]`. Creedo's new-bot table gives 510 / 250 / 460 / 880
  `[CR-bot]`, and OpenBound's delay table 510 / 250 / 400 / 800 `[OB]`. **CONTRADICTION** on base
  delay and SS delay.
- **Weapon type.** `[SW-mob]` and `[DB-mob]` say Explosion. `[AK]` lists it as an "Electricity
  Machine", and Softnyx's own description calls it "a electrical mobile" whose "electric shock can
  only be given once" `[KAL-snx]`. **CONTRADICTION**; every other mobile's `[AK]` type matches
  `[DB-mob]`.
- **Every shot.** "First, you fire a big bomb. At the top of the big bomb's arc it will split up
  into smaller missiles. Even after the bomb splits, a little tracer (like a. sate's tracer) keeps
  flying along the shot's normal path and will hit the ground. The missiles that are released move
  in a weird circular pattern. They spin around in loops for a second before homing in on the
  tracer" `[CR-kal]`. "It splits into 2 missiles and a guide. After another second, the 2 missiles
  start following the guide and hit wherever the guide lands" `[SW-mob]`. "The warhead splits just
  when it begins to fall and the homing missiles then searches for its target" `[KAL-snx]`. The
  split missiles "circle around twice before straightening directly on top of the target"
  `[KAL-gbomb]`. "The missiles will have partial homing towards the tracer" `[KAL-dlh]`.
- **Too little air.** "If they don't have enough time in the air, ... their weird spinning
  animation causes them to crash into the ground or some other obstacle (like you) before they end
  up hitting the target" `[CR-kal]`. "Almost impossible to use effectively in close-ranged flat
  land" `[SW-mob]`. Shots need a high angle `[KAL-snx]`, `[KAL-gma]`.
- **The warhead.** "The big bombs pass through the target's body, they aren't solid until they hit
  land or blow up inside the enemy's body. If you shoot through their body into the land and the
  bombs don't get a chance to split, you do puny damage" `[CR-kal]`. "It can also destroy the
  terrain by hitting the terrain before the missiles splits" `[KAL-snx]`. The close-range trick is
  to open the shot inside or in front of the target `[CR-kal]`, `[KAL-dlh]`, `[CR-pb218]`.
- **Where they land.** "Landing the tracer at the bot's center doesn't guarantee a hit, because
  the missiles arrive at an angle" `[CR-pb218]`. Shots home "pretty well" even through a tornado
  `[CR-kal]`.
- **S2:** four missiles, 250-400 "on a complete hit, approximately 50-120 per missile" `[SW-mob]`.
- **SS:** "1 huge missile, that splits into 4 missiles. Then those 4 missiles split into 8
  missiles, each with their own guide, each guide following the main guide ... weak for an SS if
  not accompanied by a offense satellite ... requires pin-point precision" `[SW-mob]`; "8 missiles
  and 8 purple tracers" `[AK]`; "most of the time, the Kal SS doesn't hit each missile"
  `[KAL-gma]`.
- **Damage.** S1 150-220, S2 250-400, SS 300-650 `[SW-mob]`, `[DB-mob]`. Typical: S1 150-200, S2
  230-350, SS 400-500 `[CR-kal]`; S1 120-190, S2 220-340 `[KAL-gma]`.
- **Angles.** No source gives Kalsiddon's range. "Good angle range (sort of like nak's old angle
  range)", "you should have no trouble getting true angle with kal's good climbing" `[CR-kal]`.
  `[OB]` marks its aim "NEED SOURCE"; `[DBR]`'s Kalsiddon is an unfinished copy of its J.Frog (the
  same aim data, 30-60 overwritten with 30-110), so it is not used.
- **Items.** "Duals don't have more than the usual 'average' dual delay (he's not like armor)"
  `[CR-kal]`. `[KAL-dlh]` claims "an innate -20 item delay stat" (single source, unconfirmed).
- **Power.** In cbchui-style 30-part shooting, "kal will use power levels ranging from 2.55 (half
  screen shots) to 2.65 (1 full screen)" (the author calls them close but unconfirmed) `[CR-kal]`;
  "BF 2" (Bigfoot 2, Kalsiddon's nickname `[AK]`) "uses 2.6 power with 30 sd's, or 2.65 for angles
  74 to 66 and 2.7 for angles 66 to 60" `[CR-kal30]`. Banpao-style: 3.1 for half a screen or
  closer, 3.15 for 3/4, 3.2 for 1, 3.25 for 1.5 screens; fixed 70 (untested): 2.25, 2.75, 3.2 and
  3.9 for 1/2, 3/4, 1 and 1.5 screens `[CR-kal]`.

## How lobshot plays it

`sim::shots::herdhound` over the `homing` engine. Each shot is a warhead under Herdhound's gravity
and the wind that passes through mobiles. It opens at its apex, the first tick it is no longer
climbing. A guide flies on along the same flight, and the missiles burst out of the opening point.
They loop at a steady turn for `LOOP_SECONDS`, then each turns toward where its guide is, as hard as
`HOMING_TURN` allows. Missiles are rockets at a steady `MISSILE_SPEED`, which neither gravity nor
wind moves. Each flies 1/64 s steering steps, each a leg under the constant acceleration that turns
its velocity by that step's turn. So `Game::track_until` follows them through the weather like any
shot (Force, Drain and Electricity mark them; Mirror and Tornado turn them, and the homing turns them
back), and clients draw them from their legs.

- **A warhead that opens inside a mobile** looses its missiles inside it: they strike at once, the
  close-range trick. One that touches the ground first is a dud: `DUD_DAMAGE` and a full crater.
- **Loops** are about 57 px across. An apex lower than that over the ground, or over a mobile
  (the shooter included), crashes them, as close, flat shots do.
- **The SS** opens into four that loop, then split into two each. Each half homes on its own guide:
  eight guides leave the opening point beside the main one, fanned sideways by `SS_GUIDE_SPREAD`. They
  land spread about where the shot would, wider the longer they fall.
- **Missiles inherit the warhead's marks**: a warhead through Force boosts every missile.

## Picks and resolved contradictions

| What | Value | Why |
|---|---|---|
| Weapon type | Explosion | `[SW-mob]`, `[DB-mob]` over `[AK]` and `[KAL-snx]`'s Electric: StrategyWiki is the primary GunBound source, and DragonBound agrees. Electric is one line to change. |
| Base delay, SS delay | 515, 880 | `[SW-mob]` and `[CR-bot]` disagree (515/800 vs 510/880). The base is DragonBound's `[DB-mob]`, which matches `[SW-mob]`; the SS was too, at 800, until balance (PRD-0006 T-010) took `[CR-bot]`'s 880. |
| Item surcharge | 0 | `[CR-kal]`'s "average" dual delay; `[KAL-dlh]`'s -20 is a single unconfirmed claim. |
| Aim | 10-50, true 15-45; SS all true | Nak's GunBound range `[CR-mob]`, which `[CR-kal]` likens Kalsiddon's to. |
| Gravity | 748.5 px/s² | Fitted so 60 degrees lands one screen at 2.675 bars, midway between `[CR-kal]`'s 2.65 and `[CR-kal30]`'s 2.7; both hold within 0.05 (`tests/formula.rs`). |
| Wind sensitivity | 1.0 | No source sets it apart. |
| Loop | 1 s, two turns | `[SW-mob]`, `[CR-kal]`; two turns `[KAL-gbomb]` (search snippet only). |
| Missile speed | 450 px/s | Loops about 72 px across, so close, flat shots crash; a missile crosses a screen in under two seconds. 360 until balance (PRD-0006 T-010). |
| Homing turn | 3 rad/s | "Partial homing towards the tracer" (`[KAL-dlh]`): a missile comes about in a second after its loops and comes in on the guide at an angle (`[CR-pb218]`), scattering about a guide that lands beside its target. 6 until balance (PRD-0006 T-010), when missiles landed on the guide every time and Herdhound led at both levels. |
| Missile lifetime | 6 s | A guard: a missile that has hit nothing goes off where it is. |
| Missile damage | S1 75-110, S2 63-100, SS 38-81 each | `[SW-mob]`'s totals split evenly. |
| Dud | 40-60, 28 px crater | "Puny damage" `[CR-kal]`, but it carves terrain `[KAL-snx]`. |
| Burst headings, SS split and guide spread | `tuning::herdhound` | Picked for a readable flower of loops and an SS that lands three or four of eight on a dead-center target, the "pin-point precision" its full damage needs `[SW-mob]`. |
| Electric shock | none | `[KAL-snx]` mentions a once-a-turn electric shock but gives no number, and lobshot follows `[SW-mob]`'s Explosion. |

## Power formulas left out

At the fitted gravity (Herdhound's own muzzle and a Coilback target, as in `tests/formula.rs`):

| Entry | Says | Model |
|---|---|---|
| Banpao 80 degrees, 1/2 screen | 3.1 | 2.99 |
| Banpao 75 degrees, 3/4 screen | 3.15 | 3.06 |
| Banpao 70 degrees, 1 screen | 3.2 | 3.13 |
| Banpao 60 degrees, 1.5 screens | 3.25 | 3.32 |
| Fixed 70, 1/2 screen | 2.25 | 2.15 |
| Fixed 70, 3/4 screen | 2.75 | 2.68 |
| Fixed 70, 1.5 screens | 3.9 | 3.87 |
| cbchui 75 degrees, 1/2 screen | 2.55 | 2.45 |

The same pattern as Coilback's left-out entries (`physics-fit.md`): the fixed-power systems are
linear rules over a curve, and `[CR-kal]` flags its own figures as unconfirmed and its fixed-70 ones
as untested. Only the one-screen figures, which two authors give, are fitted.

## Sources

Fetched 2026-10-22. Every page is a fan or vendor page; treat its numbers as player-reported.

- `[CR-kal]` https://creedo.gbgl-hq.com/basic_frog_and_kal.php (CreeDo, "Basic J.Frog and
  Kalsiddon use"; notes that Kalsiddon was later "MAJORLY nerfed")
- `[CR-kal30]` http://creedo.gbgl-hq.com/gbcw_new_mobile_30part_formulas.php ("Frog/Kal 30 parts",
  credited to demonchaos on GBGL; Kalsiddon appears as "BF 2")
- `[CR-pb218]` https://creedo.proboards.com/thread/218/kalsiddon-high-angles (forum thread,
  2005)
- `[KAL-snx]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/kalsiddon.html (Softnyx's official
  page; access denied, read through its search snippet and the English copy at
  https://forum.gamer.com.tw/G2.php?bsn=7804&sn=105)
- `[KAL-gbomb]` https://www.giantbomb.com/games/3030-15697/ and
  https://giantbomb.com/wiki/Games/Gunbound (Giant Bomb's GunBound wiki; 403, read through search
  snippets only)
- `[KAL-dlh]` https://www.dlh.net/en/cheats/31951/gunbound.html (a player's guide)
- `[KAL-gma]` https://www.deviantart.com/goldmageace/art/Gunbound-FAQ-101336660 (GoldMageAce's
  GunBound FAQ)
