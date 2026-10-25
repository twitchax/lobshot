# GunBound / DragonBound mechanics: public-source research for lobshot

Date: 2026-10-19. Scope: documented and player-known mechanics only. OpenBound's source is covered separately, in `openbound.md`.
Citations use short keys like `[DB-mob]`. Every key maps to a URL in the **Sources** section at the end.

## Source quality: read this first

- **The DragonBound official guide (`dragonbound.net/guide/*`) is the main DB source.** Its per-mobile text is copied almost word for word from StrategyWiki's *GunBound* mobiles page `[SW-mob]`, but many of the numbers were changed: HP, shields, base delay, Shot 2 delay, and some weapon types. That pattern points to deliberate DB-specific edits, so I treat the DB guide's numbers as DB values and the StrategyWiki numbers as GunBound values. Some GunBound-derived prose was never updated, though. The best example: the DB "How to Play" page still says "Dual will add 600… Teleport will add 150", while the DB Items page says Dual +550 and Teleport +100.
- **The GunBound sources come from 2003–2006 GBTH/GBWC-era fan material:** GameFAQs FAQs, Creedo's formula site, StrategyWiki, and a 2006 Wikipedia revision. Values differ between versions (Classic, Thor's Hammer, World Champion).
- **Clone code.** The GitHub project `DragonBoundR/DragonBound` `[DBR]` is a reverse-engineered DB server/client clone. It contains per-mobile physics constants and wind and sudden-death logic. This is clone code, not documentation. I cite it only where public documentation is silent, and I label those values **(clone, unverified)**. Treat them as hypotheses to check in play.
- Where sources disagree, the text says **CONTRADICTION**. Single-source or inferred claims are marked **(single source)** or **(inference)**.

---

## 1. Turn order and the delay system

### 1.1 Core model (GunBound, which DragonBound inherits)

- **Everyone starts at delay 0.** The starting order is random, with the constraint that teams alternate `[SW-delay]`.
- **The turn queue is a priority queue on accumulated delay.** After each turn the player's delay for that turn is added to their running total, and the lowest total goes next. Turns can be "lapped": a player can act twice before a slow player acts once `[SW-delay]`.
- **Delay for one turn = mobile base delay + weapon delay (S1/S2/SS) + item delay + time spent.** This four-factor breakdown is in `[WP06]` and `[AK]`. A worked example: Ice "Base Delay: 740 (490+250)" `[GF-ice snippet]`.
- **Time cost is 10 delay per second for every mobile except Turtle, which pays 12/s.** Sources: `[SW-delay]`, `[SW-mds]`, `[DB-play]`, `[DB-mob]`. A third-party DB summary also says 10/s (`dragonbound.net/guide/howtoplay`).
  - DB Electrico is listed at **8/s** `[DB-mob]`.
  - The timer is measured in 0.01 s increments `[WP06]`. StrategyWiki notes "most of the time, delay occurs in increments of 10" `[SW-delay]`.
- **Turn timer: 20 s.**
  - GunBound: `[SW-delay]`, `[GF-tiro]`, `[WP06]`.
  - DragonBound: "Turn Time: Remaining time of your turn (20 seconds maximum)" `[DB-game]`. Room masters can lower it; this option is rank-gated to Gold Battle Axe `[DB-room]`.
  - A Reddit comment complains about "5s turn to play" on DB `[RD-snip]` (single source, from a search snippet).
- **Passing the turn.**
  - GunBound: the "Base" column in `[SW-mds]` is defined as "the amount of delay incurred if the player passes". Those values are exactly the mobile base plus 200. For example, Armor's pass is 720 against a mobile base of 520 `[CR-mob]`, and every row in the two tables is consistent with this.
  - **CONTRADICTION:** `[WP06]` says forfeiting via the "skip turn" button costs no extra delay.
  - DragonBound: F8 is "Pass Turn" `[DB-ctrl]`. The DB pass cost is undocumented.
- **The turn-list display shows relative delay.** Your own row shows the delay of your last action, or of the currently selected action while it's your turn. Everyone else's row shows a ± delta relative to you, and the numbers update live as you switch shots, use items, or burn time `[SW-delay]`, `[DB-play]`, `[GF-delay]`. The DB turn list sits bottom-left and looks like "TesterPlayer 1130 / Haris Pilton +340" `[DB-img-delay]`.
- **Ties:** no public documentation found. **(clone, unverified)** `[DBR]` sorts by delay, then by `lastturn`, so whoever acted longest ago goes first.

### 1.2 Typical magnitudes (GunBound)

- A "fast" action costs about 800 and a "slow" one about 1500 `[SW-delay]`.
- S1 usually costs 720–800 total, S2 800–900+, and SS 1300+. Dual adds 600 on top `[GF-tiro]`.
- "Try to get below 800 delay on your first turn. 760 is ideal (with no avatar bonus)" `[GF-delay]`.
- "If your opponent's delay is +1600 or more, you can actually go 3 times in a row if you use two Weapon 1 attacks which usually take 790 delay or so" `[GF-delay]`.
- Avatar "Delay" and "Item Delay" stats subtract from normal and item delay respectively, but only on Avatar-ON servers `[WP06]`, `[DB-stats]`.

### 1.3 Per-mobile delay numbers

GunBound Classic totals `[SW-mds]`. Mobile base plus weapon delay comes from the Vexez table `[CR-mob]`; the totals equal those sums.

| Mobile | Pass | S1 | S2 | SS | (base / s1 / s2 / ss add-ons per [CR-mob]) |
|---|---|---|---|---|---|
| Armor | 720 | 770 | 960 | 1320 | 520 / 250 / 440 / 800 (+40 if an item is used, [SW-mds]) |
| Nak | 710 | 760 | 910 | 1310 | 520 / 250 / 400 / 800 (*CR-mob gives base 520; SW-mds implies 510*) |
| Mage | 700 | 750 | 900 | 1300 | 500 / 250 / 400 / 800 |
| Trico | 690 | 740 | 890 | 1290 | 490 / 250 / 400 / 800 |
| Bigfoot | 720 | 770 | 920 | 1320 | 520 / 250 / 400 / 800 |
| Boomer | 680 | 730 | 880 | 1280 | 480 / 250 / 400 / 800 |
| Raon | 700 | 750 | 900 | 1300 | 500 / 250 / 400 / 800 |
| Lightning | 700 | 750 | 800 | 1300 | 500 / 250 / 300 / 800 |
| JD | 690 | 740 | 830 | 1290 | 490 / 250 / 340 / 800 |
| A.Sate | 680 | 730 | 880 | 1280 | 480 / 250 / 400 / 800 |
| Ice | 690 | 740 | 890 | 1290 | 490 / 250 / 400 / 800 |
| Turtle | 690 | 740 | 890 | 1290 | 490 / 250 / 400 / 800 (12/s) |
| Grub | 690 | 740 | 890 | 1290 | 490 / 250 / 400 / 800 |
| Aduka | 710 | 760 | 910 | 1310 | 510 / 250 / 400 / 800 |
| Knight | n/a | | | | 550 / 250 / 400 / 840 [CR-mob] |
| Dragon | n/a | | | | 550 / 250 / 400 / 840 [CR-mob] |
| Kalsiddon (GBWC) | | | | | 510 / 250 / 460 / 880 [CR-bot] |
| J.Frog (GBWC) | | | | | 430 / 250 / 250 / 960 "(not sure)" [CR-bot] |

**DragonBound per-mobile delays** are in the roster table in section 3. The DB guide lists them as a "Base Delay" plus separate "Shot N Delay" values, for example Armor 510 base, S1 250, S2 430, SS 800 `[DB-mob]`. I assume the totals are additive as in GunBound, which gives Armor 760/940/1310 **(inference)**.

### 1.4 GunBound vs DragonBound delay differences (documented)

- **Dual:** GB 600 `[SW-gp]`, `[WP06]`. DB +550 `[DB-items]`.
- **Dual+:** GB 250. DB +400.
- **Teleport:** GB 150 `[SW-gp]` or 100 `[WP06]`. DB +100.
- **DB has only three battle items:** Dual, Dual+, and Teleport `[DB-items]`, `[DB-game]`. GB has eleven (see section 5).
- **DB base delays mostly match GB**, but some shot delays differ:
  - Armor S2: DB 430 vs GB 440.
  - Lightning: DB S1 200 / S2 350 vs GB 250 / 300.
  - Grub S2: DB 400 vs StrategyWiki-GB 480.
  - Kalsiddon base: DB 515 vs GB 510.
  - Knight and Dragon SS: DB 840 and 800, vs GB 840 and 840 `[DB-mob]` vs `[CR-mob]`, `[SW-mob]`.

---

## 2. Shooting: angle, power, wind, gravity

### 2.1 Angles

- **Each mobile has a limited angle arc.** The brighter/darker green part of the aim arc is the **True Angle**, which deals full damage. The dimmer part is the **False Angle**, which deals reduced damage. Some mobiles have no false angle at all `[DB-play]`, `[SW-mobC]`, `[GF-tiro]`.
- **The arc appears to be relative to the mobile's body, which tilts with the terrain** (inference). The evidence is that climbing ability is described as mattering "for finding higher angles" (Bigfoot, `[DB-mob]`, `[SW-mob]`), and the DB HUD draws the arc rotated around the mobile `[DB-img-game]`.
- **Displayed angles are integers.** The HUD shows "Current Angle" plus "Previous Angle" from your last turn `[DB-game]`. Community formulas count in 1° steps `[CR-using]`.
- **GunBound angle ranges (true range in parentheses)** `[CR-mob]`. Each entry lists max–min for S1; S2 is the same as S1 unless noted.

| Mobile | S1 | SS / notes |
|---|---|---|
| Armor | 55–10 (40–25) | SS 50–10 |
| Mage | 50–15 (all true) | |
| Nak | 50–10 (45–15) | SS 50–10 |
| Trico | 50–10 (40–20) | SS 40–20 |
| Bigfoot | 40–18 (35–23) | SS 35–23 |
| Boomer | 90–10 (65–35) | |
| Raon | 50–10 | |
| Lightning | 40–18 (35–23) | |
| JD | 40–10 | |
| A.Sate | 60–20 | SS 55–25 |
| Ice | 70–20 (60–30) | SS 60–30 |
| Turtle | 50–25 | |
| Grub | 60–30 (55–35) | SS 60–30 |
| Aduka | 50–10 | |
| Knight | 80–10 | |
| Dragon | 60–30 | |

- **The GB Nak has since had all of its arc made true angle**, with a +2% defence buff `[GF-max]`.
- **DragonBound angle ranges (clone, unverified)** `[DBR]`, given as `minang–maxang`:
  - Armor 10–55, Ice 20–70, Lightning 18–40, Bigfoot 20–45, JD 15–65, A.Sate 20–60, Knight 20–60, Mage 15–50, Trico 10–60, Turtle 25–50, Boomer 10–90, Grub 30–60, Dragon 15–50, Barney 15–50, Raon 30–60, Frog 30–60, Electrico 25–60, Fox 20–60.
  - Nak and Aduka are 110–170 because they fire from the rear, which is equivalent to 10–70 backwards.
  - The clone also carries per-mobile true-angle data in an `aim` array, e.g. Armor `[52,33]`. Its exact semantics are unknown.

### 2.2 Power bar

- **GunBound.**
  - The power meter is 400 px wide and has 4 "bars" of 100 px each, marked by 3 triangles. Full power = 4.0 bars `[CR-using]`, `[WP06]` (slang entry "bar"), `[GF-tiro]`.
  - With slice shooting (hold Space, release), the smallest practical step is 0.05 bar (5 px). Missing by 0.02–0.03 bar is usually fine; missing by 0.05 often misses the shot `[CR-using]`.
  - Slice power can only go up. Dragshot (mouse) can go both ways but jitters the longer you drag. A short-lived "Sniper" mode snapped to your last power `[GF-tiro]`, `[AK]`, `[WP06]`.
  - A draggable "clip" marker on the bar lets you remember a power `[GF-tiro]`.
- **DragonBound.**
  - There are two shooting modes, Slice (Space) and Drag (mouse drag right), plus touch modes: "TouchShot Exact" lets you tap the bar directly `[DB-ctrl]`.
  - The HUD shows an hourglass-shaped marker at the previous shot's power `[DB-img-game]`, `[DBMOS a.png]`.
  - A 2013 DB HUD screenshot shows the bar divided into **8 numbered segments (1–8)** `[DBMOS a.png]`. DB community fixed-power guides quote powers on that 8-scale, e.g. "the power should always be 4.5 bars… 60° reaches a full screen" `[DBMOS]`. A DB ruler overlay labels distances from 0.8 to 4.8 at 60° `[DBMOS regla.png]`.
  - **CONTRADICTION / ambiguity:** other DB charts use GunBound-style 0–4 numbers. A "DragonBound World's" fixed-angle table gives 70° = 0.9 … 2.8 across about 1 screen `[DBMOS 2nd image]`, and La República quotes "2.8 at base 70" `[LR]`. The current guide screenshot shows only faint tick marks. **Measure in the browser.** Either way, the underlying quantity is continuous.

### 2.3 Range, gravity and per-mobile "weight"

- **Maximum distance for any power is at 45°** `[CR-using]`, `[CR-turtle snippet]`. Angles above and below 45° roughly mirror each other.
- **Range grows faster than linearly with power.** "If 1.5 markers covered 10 metres, 3 markers… around 30 metres" `[GF-tiro]`. The Nak formula community-fits Power ∝ sqrt(Distance) `[CR-nak]`, i.e. **R ∝ P²·sin2θ / g**, a vacuum parabola.
  - The tables roughly fit this up to about 1 screen. Beyond 1 screen, the required power rises faster than √distance; the Aduka 60° series, for example, goes 2.40 → 3.60 for 1 → 1.5 screens `[GN-aduka]`. **(inference:** there may be drag, launch-offset effects, or a non-linear power→velocity mapping.) One amateur write-up claims GB "air resistance is proportional to time" `[JIM]` (low confidence).
- **The community "generic formula" is** `Power(bars) = sqrt(Distance / (47.87² · sin(2A) · 1/(G + W·sinX))) · 4`, where:
  - G is a per-mobile gravity (Nak G = 111.615 in the formula's units),
  - W is the wind strength,
  - X is the wind angle.
  - The **vertical wind component therefore acts as an addition to gravity**. The formula ignores horizontal wind `[CR-nak]`. A forum snippet says Mauvecow measured "gravity is 98 for most bots" `[CR-pb458]` (search snippet only).
- **Per-mobile shot weight is the strongest player-visible evidence of per-mobile gravity.** Creedo's cbchui fixed-power chart lists the power needed so that **75° lands at ½ screen and 60° at 1 screen** in zero wind `[CR-fixed]`:
  - Boomer 2.25; JD ("Cake") 2.30; Lightning 2.30; Ice 2.33; Grub 2.33; Aduka 2.35; Mage 2.38; Turtle 2.39; Armor 2.45; A.Sate 2.50; Raon 2.50; Trico 2.65; Nak 2.70; Bigfoot "??".
- **(clone, unverified)** DB per-mobile coefficients `ax` / `ay` appear in `[DBR]`. They match the format DB aimbots use, e.g. `{"a":73.5,"b":0.74,"name":"Armor"}`. The clone's own comments are unsure what they mean ("resistencia viento??? / gravedad ???").
  - `ax`: Boomer/Ice/JD 62.5, Grub/Frog 61, Lightning 65, Aduka/Knight 65.5, Mage/Dragon 71.5, Armor/Turtle/Raon 73.5, A.Sate 76, Nak 82, Trico 84, Bigfoot 88.
  - `ay`: mostly 0.61–0.87. Boomer **1.395**; Dragon and Barney **1.78**.
  - **(inference)** `ax` behaves like gravity. sqrt(ax ratio) closely matches the GB power ratios above: Armor/Boomer sqrt(73.5/62.5) = 1.084 vs 2.45/2.25 = 1.089; Trico/Boomer 1.159 vs 1.178. `ay` behaves like wind sensitivity, which fits Boomer being "extremely sensitive to even the smallest amount of wind" `[WP06]`.
- **Screen unit.** One GunBound screen = 800 px wide, regardless of monitor. Distances are measured in "SD" (screen distance), from a mobile half off one edge to a mobile half off the other `[CR-using]`. The DB battlefield is a 4:3 box by default, with an optional "Expand View" `[DB-game]`.

### 2.4 Wind

- **Range.**
  - GunBound: up to about 26. Creedo's windchart has rules "for 25/26 lower 2 more angles", and Philoburn's chart has wind buckets up to 26 `[CR-wind]`, `[CR-philo]`.
  - DragonBound: the room master picks **Max Wind 0 / 12 / 26 / 50** `[DB-room]`. Higher max wind pays more GP: 0 = 0%, 12 = +5%, 26 = +10%, 50 = +15% `[GL]`.
  - A 2013 DB tutorial says DB wind numbers are on **twice the GunBound scale**: "divide the wind by two (DB wind 48 = GB wind 24)" before applying GB charts `[DBMOS]` (single source).
- **Direction.** The GB HUD is a circular dial with an arrow and a magnitude number `[GF-tiro]`. Players decompose wind into x/y components with unit-circle factors at 30/45/60° `[TAMBAY]`.
  - Community wind charts use about 16 sectors, but that reflects the charts, not engine granularity `[CR-wind]`, `[CR-philo]`.
  - **(clone, unverified)** `[DBR]` rolls the wind angle as an integer 0–359°.
- **When wind changes.**
  - GunBound: the **Wind satellite on the moondisk** changes speed and direction once, and the wind "stays the same until the next time Wind comes up" `[WP06]`. Meteor weather also randomises wind `[SW-weather]`. The Change Wind item reverses it (horizontal and vertical) `[SW-gp]`.
  - DragonBound: the "Wind" weather "change[s] the strength and direction of the wind" `[DB-play]`.
  - **(clone, unverified)** `[DBR]` additionally re-rolls wind *strength* every ~3 turns and the *angle* only on the Wind weather.
  - **Unresolved:** whether DB rerolls wind every turn. Check in the browser.
- **How strongly wind affects shots (GunBound community factors).**
  - **Horizontal wind:** adjust angle by about wind × 0.5 for the 2.4/2.5-bar fixed-power systems, i.e. "÷2" on the side spokes `[CR-wind]`, `[CR-philo]`.
  - **Diagonal factors** on Philoburn's chart: ×0.7 (tailwind, up-diagonal), ×0.6, ×0.45, ×0.25, ×0.1 (near down-tail); ×0.4, ×0.5, ×0.6, ×0.55, ×0.05 on the headwind side `[CR-philo]`.
  - **Vertical wind:** change power, not angle. Subtract 0.1 bar per 7 wind of up-wind and add 0.1 bar per 7 wind of down-wind `[CR-wind]`, `[GN-aduka]`.
  - **Boomer:** "in 10 wind towards the enemy… about .45 bars of power less… angle 70 1 screen shot (normally 2.8 bars) will be 2.35 bars" `[UC-boomer]` (search snippet).
  - **DB:** for base 70 (fixed-angle-ish), new angle = 70 ± wind/2, plus about +0.1 power per 10 wind of headwind `[LR]`. A separate DB rule halves the wind and then applies GB charts `[DBMOS]`.
  - "Even… if the wind is 1 different from your last turn it can alter your shot" `[TRIPOD snippet]`.
- **(inference) Model consistent with all of the above:** wind is a constant acceleration vector (strength × direction × per-mobile sensitivity) added to gravity for the whole flight. This explains:
  - vertical wind behaving exactly like a gravity change `[CR-nak]`,
  - horizontal wind ≈ a linear angle shift per wind unit,
  - and the long, curling Boomer hooks and backshots (high sensitivity).

### 2.5 Community aiming systems (these encode the physics)

- **2.4 fixed power.** Angle = 90 − distance ± wind·factor, with 30 parts per screen and power 2.4 fixed (slightly less up close, slightly more far away). "70, 2.4 will hit… 2/3 screen". 2.4 "cannot shoot past 1 screen" `[CR-using]`.
- **cbchui fixed power.** 30 parts per screen; 60° = 1 screen, 90° = yourself; per-mobile powers as listed in section 2.3 `[CR-fixed]`.
- **Banpao / "3-bar".** About 3.0 bars fixed, "angle 60 will land exactly 1.5 screens away" (use 3.05 far, 2.95 near) `[CR-using]`.
- **BJSL.** Power controls distance and angle compensates wind. For example, BJSL 80 hits ½ SD at 80° with 2.9 bars for Armor or Turtle, and cannot reach 1 SD with heavy shots `[CR-using]`.
- **Fixed-angle tables (GunBound PH, Aduka, 0 wind)** `[GN-aduka]`:
  - 60°: ¼ → 1.10, ½ → 1.60, ¾ → 2.10, 1 → 2.40, 1¼ → 3.10, 1½ → 3.60.
  - 70°: 1.35 / 2.00 / 2.45 / 2.85 / 3.25 / 3.65.
  - 80°: 1.70 / 2.40 / 3.30 / 3.80.
  - "Super high": ½ screen = 85° at 3.80; 1 screen = 80° at 3.80.
- **Plain throw:** 45° with 2.25 bars ≈ 1 screen; 1.33 bars ≈ ½ screen `[GF-tiro]`.
- **Backshot (Boomer):** 60 − wind·index + distance at 2.5 bars. Boomer's maximum backshot distance occurs at 66° `[CR-using]`.
- **DB fixed power (8-scale):** 4.5 bars, 60° = 1 screen (4.6 to be exact), 75° = ½ screen, 1 bar ≈ 4 angles; 32–35° at 4.5 also reaches 1 screen `[DBMOS]`.

---

## 3. Mobile roster

### 3.1 DragonBound roster (official guide numbers) `[DB-mob]`

Damage figures are "base damage, direct hit, no modifiers" `[DB-mob]`. Types are listed as Armor class | Weapon type. Delays are listed as base / S1 / S2 / SS.

| Mobile | Type | HP (shield, regen/turn) | Delays | S1 dmg | S2 dmg | SS dmg |
|---|---|---|---|---|---|---|
| Armor | Mech \| Explosion | 1000 | 510 / 250 / 430 / 800 | 120–165 | 230–290 | 350–500 |
| Mage | Shield \| Laser | 730 (220, +30) | 500 / 250 / 400 / 800 | 100–160 | 190–280 | 350–500 |
| Nak | Mech \| Physical | 1100 | 520 / 250 / 400 / 800 | 100–150 | 190–300 | 350–500 |
| Trico | Bionic \| Explosion | 1050 | 490 / 250 / 400 / 800 | 100–150 | 170–310 | 250–500 |
| Bigfoot | Mech \| Explosion | 1100 | 520 / 250 / 400 / 800 | 170–230 | 200–270 | 350–500 |
| Boomer | Bionic \| Physical | 1100 | 480 / 250 / 400 / 800 | 120–160 | 230–290 | 350–900 |
| Raon Launcher | Mech \| Laser | 1000 | 500 / 250 / 400 / 800 | 100–160 | 140–180 per mine | ?? |
| Lightning | Shield \| Electric | 750 (300, +20) | 500 / **200** / 350 / 800 | 150–250 | 150–300 | 150–200 per target |
| J.D. | Shield \| Electric | 950 (250, +30) | 500 / 250 / 340 / 800 | 140–170 | 150–200 | 250–320 |
| A.Sate | Shield \| Laser | 800 (210, +35) | 480 / 250 / 400 / 800 | 100–160 | 200–280 | 400–550 |
| Ice | Bionic \| Physical | 1200 (highest) | 490 / 250 / 400 / 800 | 120–160 | 150–170 | 150–170 |
| Turtle | Bionic \| Physical | 950 | 490 / 250 / 480 / 800 (12/s) | 130–160 | 220–280 | 400–600 |
| Grub | Bionic \| Electric | 1000 | 490 / 250 / 400 / 800 | 80–130 | 180–220 | 400–1000 |
| Aduka | Mech \| Electric/Laser | 1000 | 510 / 250 / 400 / 800 | 140–160 | 140–500 (Thor) | 80–250 |
| Kalsiddon | Mech \| Explosion | 1100 | 515 / 250 / 460 / 800 | 150–220 | 250–400 | 300–650 |
| J.Frog | Bionic \| Laser | 930 | 500 / 250 / 250 / 800 | 150–230 | 150–230 | 100–700 |
| Knight (Random only) | Mech \| Laser | 1500 | 550 / 250 / 400 / 840 | 200–250 | 230–290 | 400–550 |
| Dragon (Random only) | Bionic \| Explosion | 1250 | 550 / 250 / 400 / 800 | 150–250 | 300–500 | 450–650 |
| Barney (Random only) | Bionic \| Explosion | ?? | ?? | ?? | ?? | ?? |
| Electrico (Random only) | Bionic \| S1–S2 Electric, SS Explosion | ?? | ?? (8/s) | ?? | ?? | ?? |
| Fox (rentable Ex-Item) | Bionic \| Explosion | ?? | ?? | ?? | heal shot | team heal |
| Randomizer | Mech | ?? | ?? | random | random | random |

**Shot behavior** (from the same page; GunBound behavior is the same unless noted):

- **Armor.**
  - S1: one heavy round with a wide blast; good for bunging.
  - S2: two warheads. The first detonates on contact and the second continues to the next contact. "The second warhead pierces defense if the first hits."
  - SS: a shell that expands after 2.5 s and then hits much harder. GB sources give the transform time as 3 s `[WP06]` or 1.8 s `[GF-tiro]` (**CONTRADICTION**).
- **Mage.**
  - S1: a laser bolt with good bunge.
  - S2: two laser beams that wiggle around each other.
  - SS: a large-radius blue ball that strips shields and does heavy damage on a direct hit.
- **Nak.** Fires from the rear.
  - S1: a regular round.
  - S2: digs into the ground, resurfaces, then detonates.
  - SS: a mini-Nak that passes through land and explodes only on a player (ally or enemy). Damage depends on hitting a "weak spot".
- **Trico.**
  - S1: a heavy cabbage ball.
  - S2: three balls, two orbiting a centre one.
  - SS: a fireball with a big multi-explosion and heavy land removal. It is best when it hits the enemy's feet.
- **Bigfoot.**
  - S1: 4 missiles with wide spread; great for bunge.
  - S2: 6 grenades that spread soon after launch.
  - SS: 9 missiles.
- **Boomer.**
  - S1: a light boomerang that is very wind-dependent ("hooks" and "backshots").
  - S2: 4 boomerangs that travel clustered.
  - SS: turns golden after a few seconds in flight, for much more damage. If it doesn't transform, it does S1 damage `[SW-mob]`.
- **Raon.**
  - S1: 3 spinning discs that remove a lot of land.
  - S2: 2 mines that arm when an enemy is near, crawl toward it on the launcher's next turn, and are more attracted to Mechanical mobiles.
  - SS: a big Raon that walks in the facing direction, turns at walls it can't climb, and explodes after 5–7 s or on contact.
- **Lightning.**
  - S1: a ball that calls a vertical bolt. Land above the target blocks it.
  - S2: two diagonal bolts in a V shape (45° each side `[WP06]`).
  - SS: a large radius in which everyone gets a 170-damage bolt. GB S1 has an about 70–80 "aftershock" and always at least 76 splash `[SW-mob]`, `[WP06]`.
- **J.D.**
  - S1: an electric orb.
  - S2: an orb that *pulls* mobiles toward the impact point, with Mechanical mobiles pulled more. It can drag people off the map.
  - SS: a pyramid that *pushes* mobiles away, with more damage near the centre. The DB guide says it "can push people off a map". **CONTRADICTION:** GB `[WP06]` says it "cannot push them off cliffs (it leaves them on the very edge)", and that Creature types are pushed less.
- **A.Sate.**
  - S1: a tracer; then a satellite above A.Sate fires an inaccurate laser at it. Obstacles in the path intercept the laser.
  - S2: the satellite rises 3× higher and fires 3 lasers.
  - SS: the satellite moves above the landing point and sweeps 5 lasers left to right.
- **Ice.**
  - S1: an icicle from the back cannon; removes a lot of land.
  - S2: an ice ball from the *trunk*, a lower muzzle that can jam into walls. Each hit permanently lowers the target's Physical/Hit defence by 5%, capped at 50%.
  - SS: like S2 but a snowflake. GB patched the SS reduction from 20 down to 5 `[WP06]`.
- **Turtle.**
  - S1: a water stream.
  - S2: two streams that orbit each other and converge after about 3 s.
  - SS: splits into 6 balls after about 3 s (3.25 s per `[WP06]`). It takes 400–600 if all of them land.
- **Grub.**
  - S1: weakest shot in the game.
  - S2: 4 bouncing, rolling orbs that explode about 3 s after first contact or on a player, and bounce off the map edges.
  - SS: a ball that goes through players and does damage the whole time it touches one (up to 1000+ in a pit with Force).
- **Aduka.** Fires from the rear.
  - S2: 3 mini tracers; the map satellite **Thor** fires a beam at each. Thor's damage grows as Thor levels up.
  - SS: a land-ignoring mini-Aduka. Every enemy it passes gets a Thor beam (multi-hit through a tornado).
- **Kalsiddon.**
  - S1: a missile that splits at the apex into 2 missiles plus a guide. The missiles home on the guide.
  - S2: 4 missiles.
  - SS: 4 → 8 missiles, each with its own guide.
- **J.Frog.** Shots pass through players.
  - S1: a sponge that rolls left for 3 s and then explodes.
  - S2: the same, rolling right.
  - SS: a wall-climbing blob that does damage the whole time it is in contact. It explodes with little damage on a direct hit.
- **Knight.** A.Sate-like, using a sword that fires mini-swords. S2 = 5 swords from twice as high.
- **Dragon.**
  - S1: a light fireball.
  - S2: 4 fireballs at about 100 each.
  - SS: summons 5 mini-dragons in a semicircle that converge (4 per `[SW-mob]`).
  - Dragon floats, so it ignores rolling shots, Nak S2, and Raon mines `[SW-mob]`.
- **DB-only mobiles.**
  - **Barney:** 4, 6, or 9 meteorites, Bigfoot-like and very bunge-heavy. It has seasonal skin variants.
  - **Electrico:** S1 = 4 electric spheres; S2 = Trico-like 3 orbiting spheres that each call a lightning bolt; SS = a big meteorite.
  - **Fox:** a support mobile. S2 heals and raises defence for allies it touches; SS heals and buffs the whole team in a radius.
  - **Randomizer:** fires random other mobiles' bullets.
  - Barney, Dragon, Knight, and Electrico are obtainable only via Random. Fox is rented as an Ex-Item for up to 1 month `[DB-mob]`.

### 3.2 GunBound values that differ from DB

From `[SW-mob]` (GunBound Classic/WC) and `[CR-mob]` (Vexez):

| Mobile | GunBound HP (shield, regen) | Notable GB differences |
|---|---|---|
| Armor | 1000 | base 520–525; S2 440 (CR) or 480 (SW) |
| Mage | 730 (220, +30) [CR] or 760 (220, +20) [SW] | |
| Nak | 1100 | |
| Trico | 1050 [CR] or 1100 [SW] | |
| Bigfoot | 1100 | |
| Boomer | 1000 | DB lists 1100 |
| Raon | 1050 [CR] or 1000 [SW] | GB weapon type Explosion [SW]; DB says Laser [DB-mob], [DB-img-types] |
| Lightning | 750 (300, +20) [CR] or 760 (220, +20) [SW] | S1 250 / S2 300 |
| JD | 700 (250, +20) [CR] or 750 (250, +20) [SW] | DB 950 HP, +30 regen |
| A.Sate | 750 (200, +30) [CR] or 760 (220, +20) [SW] | |
| Turtle | 960 [CR] or 950 [SW] | |
| Grub | 1000 | S2 480 [SW] |
| Knight | 1000 | DB 1500 |
| Dragon | 1000 | DB 1250 |

**GB defence ratings (single source, meaning unclear)** `[CR-def]`:

- Turtle 90.50% (162/179)
- Armor, J.Frog, Nak 70.39%
- Kalsiddon 60.34%
- Aduka, Grub, Mage, Trico 40.22%
- A.Sate, Boomer, Ice 30.17%

---

## 4. Damage model

- **Base damage is a per-shot range** ("120–165"), and the DB guide's damage is "base damage, direct hit" `[DB-mob]`. **What the hit location and distance do:**
  - "Each mobile has a differently shaped hitbox. The amount of damage a mobile takes depends on where a collision occurs relative to the hitbox" `[SW-mobC]`.
  - Adweek describes DB as "damage according to the power of the weapon and how accurately the shot hit them" `[ADWEEK]`.
  - Nak SS needs to hit a "weak spot" `[DB-mob]`.
  - The rider avatar is *not* part of the hitbox. "Haircut/headshot" means a shot that slices the top of the avatar but doesn't register `[WP06]`.
  - JD SS and Trico SS damage depend on distance from the centre or foot placement `[DB-mob]`.
  - **No public formula** for falloff with distance was found.
- **Splash.** Some shots add a separate splash component, for example JD about 64 and Lightning a constant 76.
  - Splash ignores defence and attack stats `[GF-max]`.
  - Splash applies only once per turn even with Dual, so a dual of a 100 + 64 shot ≠ 164 × 2 `[GF-max]`.
  - Splash is "not affected by dual or dual+" `[WP06]`.
- **Armour class × weapon type.**
  - DB: Hit is +15% vs Shield and −15% vs Mechanical; Laser is +15% vs Bio and −15% vs Shield; Electric is +15% vs Mechanical and −15% vs Bio; Fire/Explosion is ±0 `[DB-mob]`.
  - GB: the same matrix, but "the difference is known to be +4/−4 percentage points" `[SW-mobC]` (**CONTRADICTION** in magnitude; possibly a different game version).
  - Also: Bionic mobiles gain more from healing; Mechanical mobiles take electricity from farther away `[SW-mobC]`.
- **True vs false angle.**
  - DB: the false angle deals "reduced damage" with no number given `[DB-play]`.
  - GB: the false angle deals **70%** of true-angle damage `[GF-tiro]`. Another source says the true angle "makes your bot do 33% more damage" `[GF-max]`, and another says false angle "reduces damage by 1.5x" `[WP06]`.
  - So all GB sources put false-angle damage at about 0.67–0.75× **(CONTRADICTION in exact value)**.
- **Other damage multipliers (GB).**
  - Force weather: +33% `[GF-tiro]`. Another source says the bonus is proportional to the time spent inside the beam `[WP06]`, `[SW-gp]`.
  - Power Up and Blood: +33% `[SW-gp]`.
  - Backshots and boomer shots: +20% damage `[GF-max]`.
  - The ice defence shred stacks up to 50% (section 3).
  - DB has **Force** (+damage) and **Black Force** (−damage); Mirror reflects the shot and adds a Force effect `[DB-play]`.
- **Thor (GB).** Levels rise with Thor's cumulative damage: Lv2 at 250, Lv3 at 750, Lv4 at 1750, Lv5 at 2500. At Lv5 it does about 200+ per beam `[GF-tiro]`. The DB HUD shows Thor as "Lv. 1 / 0" `[DB-img-game]`.
- **Crits.** No critical-hit mechanic is documented in either game. "Dual" is an item (two shots in one turn), not a crit.
- **Bonuses, DragonBound** `[DB-play]`:

| Bonus | Condition | Reward |
|---|---|---|
| Double Kill | 2 enemies killed at once | +500G 5GP |
| Triple Kill | 3 enemies killed at once | +999G 7GP |
| Bunge Shot | someone falls off the map | +200G 1GP |
| Ending | a kill | +400G 2GP |
| Excellent Shot | ≥400 damage to enemies | +100G |
| Good Shot | 200 damage | +50G |
| Shot | 100 damage | +25G |
| Hurricane | through a tornado, ≥50 damage | +20G |
| Mirror | via a mirror, ≥50 damage | +20G |
| High Angle | ≥70° and 2 s of flight, ≥50 damage | +50G |
| Ultra High Angle | ≥70° and 3 s of flight | +100G |
| Boomer Shot | passes the target, returns, ≥50 damage | +20G |
| Back Shot | fired backwards, comes forward | +25G |
| 1000 / 2000 / 3000 total damage | per game | +100G 1GP / +200G 2GP / +300G 4GP |
| Team-damage penalty | over 50 to an ally | −250G −1GP |
| Team-kill penalty | | −2000G −4GP |
| Suicide | | −5000G −5GP |

  - **CONTRADICTION:** the Spanish DB guide says High Angle needs 2.5 s and Ultra needs 4 s `[DB-es-play]`. Those match the GB values `[SW-gp]`.
- **Bonuses, GunBound** `[SW-gp]`:
  - Shot of God (2+ kills): 500G 4GP. Bunge: 100G 1GP. Ending: 100G 1GP.
  - Excellent (500+): 50G. Good (250–499): 20G. Shot (150–249): 10G.
  - Hurricane: 20G. High Angle (≥70°, 2.5 s): 15G. Ultra (≥70°, 4 s): 30G. Boomer shot: 20G. Back shot: 25G.
  - Team kill: −50G −1GP.
- **Falling off the map ("bunge").** Destroying the ground under a mobile until it drops through the bottom of the map kills it instantly, and counts as a kill and a bonus `[SW-modes]`, `[DB-play]`, `[WP06]`. The DB "Dig" stat enlarges craters for bunging `[DB-stats]`. **No source documents fall damage** from dropping a shorter distance.

---

## 5. Items

### 5.1 DragonBound `[DB-items]`

| Item | Effect | Delay |
|---|---|---|
| Dual | Fire twice in one turn with the same weapon | +550 (the How-to-Play page says 600, inherited from GB) |
| Dual+ | Fire twice with different weapons | +400 |
| Teleport | Move yourself to another spot on the map | +100 (the How-to-Play page says 150) |

- Items are used with F1–F6 `[DB-ctrl]`.
- Lock weather blocks items and stops shield regeneration `[DB-play]`.
- The room master can ban Dual+ and Teleport, and can also ban Shot 1. These bans are rank-gated `[DB-room]`.
- The room screen shows 6 item boxes, and the item icons are double-width `[DB-img-room]`. That *suggests* a 6-slot budget with 2-slot items. **(inference, verify)**
- DB Ex-Items such as Power User and Power Potion are account items, not battle items (section 7).

### 5.2 GunBound

From `[SW-gp]`; slots and the second delay value from `[WP06]` and `[AK]`. The loadout is 6 slots ("up to 6 basic items, or 3 large items").

| Item | Slots | Delay | Effect |
|---|---|---|---|
| Dual | 2 | 600 | Selected weapon (S1 or S2, not SS) fires twice |
| Dual+ | 2 | 250 | Selected weapon, then the other non-SS weapon |
| Blood | 1 | 0 | −8% of your HP; next attack +33% |
| Power Up | 1 | 150 | Next attack +33% |
| Bunge Shot | 1 | 50 | +25% land damage |
| Energy Up | 1 | 100 | Small heal; Bionic +5% extra |
| Energy Up 2 | 2 | 300 | +25% of max HP (Bionic 30%) |
| Change Wind | 1 | 150 | Reverse wind (horizontal and vertical) |
| Thunder / Thunderbolt | 2 | 200 [SW] or 100 [WP06] | A bolt strikes where each projectile lands |
| Teleport | 2 | 150 [SW] or 100 [WP06] | Shot teleports you to its landing point; no damage; ends the turn |
| Team Teleport | 2 | 50 | Swap places with your lowest-HP teammate; ends the turn |

- Armor pays +40 extra delay whenever it uses an item `[SW-mds]`.
- In Solo and Tag, dead players spin a slot machine and drop hazards on the living. Drops take 4 turns to fall. Options include Dynamite (50–60 damage, wind-affected), Hammer (~50 damage), Lightning (76–80 damage, not wind-affected), Treasure chests, and Wind Null / Wind Reverse `[WP06]`.

---

## 6. Movement

- **Each mobile has a per-turn movement budget**, shown as a Move bar `[DB-game]`, `[GF-tiro]`. "Certain mobiles have more move than others" `[GF-tiro]`.
  - Mobiles also differ in **climbing ability**: Nak "very low mobility, but high climbing ability"; Bigfoot "high mobility, but poor climbing" `[DB-mob]`.
  - Dragon floats and can climb very steep terrain `[WP06]`.
  - **No numbers** for movement range or maximum slope were found.
- **Movement has no separate delay cost** in any source. It costs delay only through time spent, at 10/s `[GF-delay]` ("the delay from moving and adjusting your angle every turn really adds up").
- **You can walk off edges**: "move… left or right (and thus fall from an edge if they go too far)" `[SW-gp]`.
- **Knockback is shot-specific:**
  - JD S2 pulls toward the impact (and can drag people off the map) and JD SS pushes away `[DB-mob]` (see the contradiction in section 3).
  - Mobiles otherwise fall when the terrain under them is removed. No general explosion knockback is documented.
- **Respawn drops** in Score mode are slightly wind-affected `[SW-modes]`, `[GF-tiro]`.
- **Controls.** DB uses ←/→ or A/D to move, ↑/↓ or W/S to aim, Tab to cycle S1/S2/SS, and right-drag to pan the camera across the whole map `[DB-ctrl]`.

---

## 7. Modes, match flow and progression (DragonBound)

- **Room sizes:** 1v1, 2v2, 3v3, 4v4 `[DB-room]`, `[DB-lobby]`. Battle Royale is free-for-all with 3–8 players `[DB-lobby]`. Boss mode is "4 vs BOT" `[DB-img-room]`.
- **Modes** `[DB-lobby]`, `[DB-room]`:
  - **Normal:** kill the other team.
  - **Score:** limited revives; "the first team to reach 0 points or run out of players alive… loses".
    - GB detail: team lives = players per team + 1, and respawn comes 4 turns after you click a drop point `[SW-modes]`.
  - **Jewel:** shoot falling jewels for points; first to 100 wins. Bullets don't hurt players; only bunge does.
    - GB detail: jewels are worth 5, 10, 25, or −5 points, need 100, 200, or 300 damage *in one turn*, and bunged players respawn after 2 turns `[SW-modes]`.
  - **Tag:** pick 2 mobiles. In DB, when the first is defeated you switch to the backup at half HP.
    **Unconfirmed (2026-10-25):** DragonBound's archived room guide lists no Tag mode at all (Normal,
    Score, Boss, Same), and no DB source found backs this line; see `modes-tag.md`.
    - **Difference vs GB Classic:** in GB you may switch once per turn, but "if the current mobile's HP is reduced to zero, the player is out, regardless of the other mobile" `[SW-modes]`.
  - **Battle Royale (DB-only):** last one standing wins. Kills pay out immediately, and placing in the top half records a Win.
  - **Boss (DB-only):** escalating bots. Beating bots unlocks the ability to *create* rooms in each mode: Normal after 1, Score after 3, Jewel after 5, Tag after 7, BR after 9.
- **Room options** `[DB-room]`, `[DB-img-room]`:
  - mode, team size, map, Avatars ON/OFF, Max Wind (0/12/26/50), Random Teams, Turn Time, and weapon/item bans;
  - mobile bans (right-click a mobile to disable it).
- **How a game ends:** the last team with living members wins `[DB-what]`.
  - Leaving costs −1500G −5GP and +1 Loss if you're alive, only +1 Loss if you're dead, and always the full price in Score and Jewel. Leavers forfeit all earnings `[DB-play]`.
- **Weather / moondisk.**
  - DB: "The Weather changes every round of turns", shown at the bottom right `[DB-play]`. The DB weathers are Force, Black Force, Lock, Mirror, Random, Thor, Thunder, Tornado, and Wind.
  - GB: an effect lasts as many turns as there are players (6 turns in 3v3) `[SW-weather]`. A snippet gives a matching cadence: "1 vs 1: every two turns… 4 vs 4: every eight turns" `[GAMICUS snippet]`. Repeated Force, Tornado, or Electricity beams persist until a different satellite comes up `[WP06]`.
- **Sudden death.**
  - **GunBound:** room options are Big Bomb (bigger craters; SS disabled), Double (every shot is dual; SS disabled), SS Death (SS every turn), and No Death. Items are disabled once sudden death starts. It begins after a set number of turns `[WP06]`, `[AK]`, `[GF-tiro]`, `[GGH-room snippet]`. A private-server changelog mentions "Double Death 40 turns by default" `[RD-origins snippet]`.
  - **DragonBound:** not documented in the official guide. **(clone, unverified)** `[DBR]` triggers at turn 40: everyone gets Dual, then each turn drains HP by 5 × (turns past 39).
- **SS availability.**
  - GB: "it can be used every 4 shots by the player, and will recharge twice as fast during Item Locks" `[WP06]`, also "1 SS every 4 turns" `[GF-tiro]`. **CONTRADICTION:** another source says "usually 9 turns, including turns by other players" `[AK]`.
  - DB: **undocumented**. Check in the browser.
- **Ranks (DB).** There are 26 GP-based ranks `[DB-ranks]`.
  - Fixed thresholds: Chicken (≤1,000) → Wood Hammer 1,100 → … → Battle Axe 6,900.
  - Percentile-gated ranks: Battle Axe+ (80%) … Gold Wand (0.1%), Platinum Wand (0.01%), and Blue / Red / Silver Dragon (top 16 / 4 / 1).
  - GM and Mod are staff ranks.
  - Rank-ups grant avatar rewards.
- **Gold and avatars.** Gold is earned in battle and buys avatars; Cash is premium currency `[DB-shop]`.
  - **Avatars change stats** when Avatars are ON. The Head, Body, Glass, and Flag slots give stats; Background and Foreground are cosmetic only; set bonuses exist `[DB-ava]`.
  - The 8 stats are Popularity (gold), Delay, Attack, Defense, Life, Item Delay, Dig, and Shield `[DB-stats]`. The cap is 50 per stat, 55 with Power User, and the Power Potion gives +5 to all stats for 1 hour `[DB-ava]`.
  - Avatar-OFF servers ignore everything except Popularity `[DB-srv]`. GB used the same stat cap of 50, and negative values were possible `[WP06]`.

---

## 8. Art direction

This section is based on official DB guide screenshots and is descriptive. The images I looked at: `[DB-img-game]` (game1.jpg, game2_en.jpg, howtoplay1.jpg), `[DB-img-mob]` (the mobiles1.jpg select grid), `[DB-img-types]`, `[DB-img-room]`, `[DBMOS 1.png]`, and `[DB-2013-maps]`.

### 8.1 Visual style

- **Characters.** Each mobile is a small vehicle or creature, about 40–60 px tall at the native 4:3 resolution. A **chibi rider avatar** sits on top, with a head about half the avatar's height.
  - Sprites are 2D raster with **dark, fairly heavy outlines (about 2 px)**, saturated flat colors, and simple 2–3 tone cel shading with small specular highlights.
  - Proportions are toy-like and rounded: big wheels, chunky cannons, cute animal bodies (mammoth, triceratops, turtle, frog, mole-truck).
  - Overall "anime-influenced" `[DB-what]`, like Korean MMO cute-mech.
- **Maps** are built in two layers.
  - **Background:** a soft painted scene, low contrast, with warm gradient skies (golden or peach in the samples) and distant hills or architecture.
  - **Foreground:** destructible terrain with crisp silhouettes. The terrain is a busy, ornate, high-saturation texture: striped onion domes, green tentacle-like bands studded with chrome-and-orange orbs, cyan metal pipes with bolts, and grass caps on top edges.
  - Terrain reads clearly against the background because of value contrast and outline.
  - Maps are wide: about 2+ screens, with right-drag panning.
  - DB 2013 supported **custom maps from a PNG, where non-transparent pixels = ground** `[DB-2013-maps]`. In other words, terrain is a bitmap alpha mask.
- **Palette.** UI chrome is glossy cobalt or steel blue with bevels and gradients. Accents are red, orange, and yellow (the SS button, the timer, angle digits). Game art is warm and saturated. Team colors are red (A) and blue (B) `[DB-img-room]`.

### 8.2 HUD layout (DB game screen, numbered items from `[DB-game]`)

- **Top centre: wind dial.** A round metal gauge with a ring arrow and green digital digits ("00").
- **Top right: turn timer.** Huge red or orange brush-stroke numerals (20 → 0).
- **Above the active mobile:**
  - an **aim arc**, a translucent dark semicircle around the mobile with a green wedge for the true angle and a red needle for the current angle;
  - a **blue speech-bubble tag** showing the current angle number;
  - below the mobile, a green HP bar (plus a blue shield bar), the rank icon, and the name.
- **Sky:** the Thor satellite, with "Lv. N" and its damage counter in red.
- **Bottom-left overlay: Turn List.** A dark translucent panel with a gold header, rows with order index, rank icon, name in team color, and delay or relative delay.
- **Bottom HUD strip** (about 90 px), left to right:
  - WEAPON panel: buttons 1 and 2 in blue and SS in red. ESC (options) and F8 PASS sit under it.
  - Angle readout capsule showing "LAST nn" and the current angle in big yellow italic digits.
  - 3 item slots.
  - Weather (moondisk) strip with 5 round slots showing upcoming weathers.
  - A long **power bar**: a dark track, red fill, tick dividers, and an hourglass-shaped **previous-power marker**.
  - A thin **Move bar** underneath.
  - A gold counter at the bottom right.
  - A Drag/Slice mode toggle icon at the bar's left.
- **Chat** sits bottom-left above the HUD, with All/Team, a spectator eye toggle, and a mic toggle `[DB-game]`.
- **No minimap** is visible in any DB guide screenshot. You look around with right-drag camera panning `[DB-ctrl]`.
- **GunBound HUD (2004, Thor's Hammer era).** Same building blocks: a circular wind meter with arrow and number, items along the top, weather/moondisk at the top-left and later bottom-left/right, the power bar at the bottom with 3 light-blue triangle markers and a draggable clip marker, the delay list at the bottom-left, and the 20 s timer at the top-right `[GF-tiro]`, `[WP06]`.

### 8.3 Genre-generic vs distinctive

- **Genre-generic (Scorched Earth / Worms lineage; safe to use):**
  - angle + power + wind aiming;
  - destructible bitmap terrain and falling off the map;
  - turn order, HP bars, turn timer, wind indicator, power bar, team colors, damage numbers.
- **Distinctive to GunBound/DragonBound (mechanics are fine to emulate; avoid copying the art, names, and trade dress):**
  - mechanics/UX: the **delay number in the turn list** (relative ± display); the **true-angle wedge** on the aim arc; S1/S2/SS buttons; the moondisk weather strip showing upcoming weather; the previous-angle and previous-power markers; a sky satellite (Thor) that levels up; rank-icon progression (axes → wands → dragons);
  - visual: a chibi avatar riding a cute mech; the giant brush-stroke timer; the glossy blue Korean-MMO chrome.
- **IP:** the mobile names and designs (Armor, Nak, Trico, Boomer, Aduka, A.Sate…) and the map names (Metamine, Adiumroot, Miramo Town, Cozy Tower, Nirvana, Stardust, Sea of Hero, Dummy Slope, Dragon, Metropolis `[SW-maps]`) are Softnyx GunBound IP, which DB reuses. For lobshot, use original names and silhouettes.

---

## 9. Open questions to check in play

1. **Wind:** does DB re-roll wind every turn, every N turns, or only on the Wind weather? Is the direction continuous (0–359°)?
2. **Power bar:** what does DB's numeric scale look like now (8 segments vs 4)? What is the displayed precision?
3. **SS:** how is DB's SS gated (cooldown in own turns, a gauge, or always available)?
4. **Sudden death:** does DB have it? When does it start, and what does it do?
5. **Passing:** what delay does DB charge for a pass (base + 200 as in GB, or base only)?
6. **Turn order:** in DB, is the first turn random with teams alternating, and how are ties broken?
7. **Damage falloff:** measure damage vs impact offset with one mobile on flat ground, and measure the false-angle multiplier.
8. **Movement:** measure the move budget in px per mobile and the maximum climbable slope. Check whether falling short of the map bottom causes damage.
9. **Items:** what is the DB item slot budget (6 slots with Dual = 2?)?
10. **Clone check:** verify the `[DBR]` constants (angle ranges, `ax`/`ay`) against observed trajectories.

---

## Sources

DragonBound official guide (fetched 2026-10-19 unless noted):

- `[DB-what]` https://dragonbound.net/guide/what_is_dragonbound
- `[DB-play]` https://dragonbound.net/guide/howtoplay
- `[DB-es-play]` https://dragonbound.net/guia/comojugar (Spanish version)
- `[DB-ctrl]` https://dragonbound.net/guide/controls
- `[DB-mob]` https://dragonbound.net/guide/mobiles (Spanish: https://dragonbound.net/guia/moviles, same numbers)
- `[DB-items]` https://dragonbound.net/guide/items (Spanish: https://dragonbound.net/guia/items, same numbers)
- `[DB-game]` https://dragonbound.net/guide/game
- `[DB-room]` https://dragonbound.net/guide/room
- `[DB-lobby]` https://dragonbound.net/guide/lobby
- `[DB-srv]` https://dragonbound.net/guide/servers
- `[DB-stats]` https://dragonbound.net/guide/stats
- `[DB-ava]` https://dragonbound.net/guide/avatars
- `[DB-ranks]` https://dragonbound.net/guide/ranks
- `[DB-shop]` https://dragonbound.net/guide/shop

DragonBound official guide images:

- `[DB-img-game]` https://dragonbound.net/static/images/guide/game1.jpg, …/game2_en.jpg, …/howtoplay1.jpg
- `[DB-img-mob]` https://dragonbound.net/static/images/guide/mobiles1.jpg
- `[DB-img-types]` https://dragonbound.net/static/images/guide/mobiles_types.png
- `[DB-img-room]` https://dragonbound.net/static/images/guide/roomoption2.jpg and …/room_desc_en.jpg
- `[DB-img-delay]` https://dragonbound.net/static/images/guide/delaylist2_en.png
- `[DB-2013-maps]` http://web.archive.org/web/2013/http://dragonbound.net/static/images/custom_maps.png

DragonBound community and press:

- `[GL]` https://gamelytic.com/dragonbound-control-and-game-play-mode-guide/
- `[LR]` https://larepublica.pe/videojuegos/2020/04/21/dragonbound-truco-de-tiros-perfectos-con-viento-a-favor-y-viento-en-contra-para-ganar-partidas-fotos-video
- `[DBMOS]` https://dbmosjugar.blogspot.com/2013/05/tutorial-guia-de-tiros-perfectos-con.html (images a.png, regla.png, 1.png, 14142_…png on blogger.googleusercontent.com, linked from the post)
- `[ADWEEK]` https://adweek.com/digital/dragonbound-review
- `[RD-snip]` https://www.reddit.com/r/gunbound/comments/s13qd3/dragonboundnet_is_pretty_sweet/ (comment text seen only in a search-engine snippet)
- `[DBR]` https://github.com/DragonBoundR/DragonBound (`src/game/gametypes.js`, `src/game/game.js`). Reverse-engineered clone; **unverified**.

GunBound:

- `[SW-delay]` https://strategywiki.org/wiki/GunBound_Classic/Delay (read via http://web.archive.org/web/20250709125724/https://strategywiki.org/wiki/GunBound_Classic/Delay)
- `[SW-mds]` https://strategywiki.org/wiki/GunBound_Classic/Mobile_Delay_Stats (archive 20250709125724)
- `[SW-mobC]` https://strategywiki.org/wiki/GunBound_Classic/Mobiles (archive 20250709125724)
- `[SW-modes]` https://strategywiki.org/wiki/GunBound_Classic/Game_Modes (archive 20250709125724)
- `[SW-weather]` https://strategywiki.org/wiki/GunBound_Classic/Weather_Effects (archive 20250709125724)
- `[SW-maps]` https://strategywiki.org/wiki/GunBound_Classic/Maps (archive 20250717024932)
- `[SW-gp]` https://strategywiki.org/wiki/Gunbound/Gameplay (archive 20250821213712; the same content is at /Gunbound/Gameplay_Mechanics, archive 20191115102347)
- `[SW-mob]` https://strategywiki.org/wiki/Gunbound/Mobiles (archive 20250919061317)
- `[GF-delay]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/27330 (psion0011 Delay Guide v1.1, 2003–04; archive 20190526114147)
- `[GF-tiro]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/28494 (Tirodalth FAQ v0.91, 2004; archive 20190526114141)
- `[GF-max]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/32773 (cactusjack23 Max Damage Guide; archive 20190526114146)
- `[GF-ice snippet]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31845 (search snippet: "Base Delay : 740 (490+250)")
- `[WP06]` https://en.wikipedia.org/w/index.php?oldid=55767667 (Gunbound article, 29 May 2006 revision)
- `[AK]` https://academickids.com/encyclopedia/index.php/Gunbound
- `[CR-using]` https://creedo.gbgl-hq.com/using_formula.php
- `[CR-mob]` http://creedo.gbgl-hq.com/mobiles.htm (table by Vexez)
- `[CR-fixed]` http://creedo.gbgl-hq.com/fixed.htm and http://creedo.gbgl-hq.com/fixed_chart.jpg
- `[CR-wind]` http://creedo.gbgl-hq.com/windchart.jpg
- `[CR-philo]` http://creedo.gbgl-hq.com/gunboundwtfchart.jpg (Lord Philoburn)
- `[CR-def]` http://creedo.gbgl-hq.com/mobile_defense.php
- `[CR-bot]` http://creedo.gbgl-hq.com/new_bot_delay.php
- `[CR-nak]` https://creedo.gbgl-hq.com/nak_shot_formula.php
- `[CR-pb458]` https://creedo.proboards.com/thread/458/mauvecows-physics-formula (search snippet only)
- `[CR-turtle snippet]` https://creedo.gbgl-hq.com/turtleguide/turtleguide28.php (search snippet)
- `[CR-sirla]` http://creedo.gbgl-hq.com/sirlamort_wind_info.php
- `[GN-aduka]` https://gitlingninja.blogspot.com/2016/07/aduka-basics-guide-2016.html (GunBound PH)
- `[TAMBAY]` http://tambaygbth.weebly.com/wind.html
- `[JIM]` https://jeffreyim.wordpress.com/2010/10/06/mechanics-of-gunbound/
- `[UC-boomer]` https://www.unknowncheats.me/wiki/Gunbound:Boomer_Guide (search snippet)
- `[TRIPOD snippet]` https://gunboundcheatshacks.tripod.com/gunboundcheats3.html (search snippet)
- `[GAMICUS snippet]` https://gamicus.fandom.com/wiki/GunBound (search snippet; the page itself is Cloudflare-blocked)
- `[GGH-room snippet]` https://gunboundggh.com/guides/room/EN (search snippet)
- `[RD-origins snippet]` https://www.reddit.com/r/gunbound/comments/b6wvl0/about_gunbound_origins/ (search snippet)
