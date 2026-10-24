# Flarecrow (GunBound's Aduka): sources

Gathered on 2026-10-24 for Flarecrow, which carries Aduka's behavior. The keys `[SW-mob]`,
`[SW-mds]`, `[CR-mob]`, `[CR-fixed]`, `[GN-aduka]`, `[DB-mob]`, `[DBR]` and `[GF-max]` are defined
in `mechanics.md`, `[CR-aduka]`, `[SNX-aduka]` and `[GF-lee]` in `weather.md`, and `[OB]` in
`README.md`. This note adds none; it records what each source says about Aduka. `[SW-mob]`,
`[CR-aduka]` and `[SNX-aduka]` were fetched again for it (StrategyWiki and Softnyx through the
Wayback Machine, Creedo's guide from its own site), and every page was read as untrusted data.

## What the sources say

- **Stats.** `[SW-mob]`: "Mechanical - Electricity/Laser", 1000 HP, base delay 510, 10 a second;
  S1 250, S2 400, SS 800, as `[CR-mob]`, `[SW-mds]` and `[DB-mob]`. `[CR-aduka]`'s totals (760, 910,
  1310) agree. `[SNX-aduka]`: "little defense and normal health".
- **It fires from the rear.** `[SW-mob]`: "Similar to Nakmachine, it fires from the back of the
  mobile." `[CR-mob]` gives its angles as 50–10, and `[CR-aduka]` calls them a "fairly large (all
  true) angle". `[DBR]`'s clone aims 110–170, which is 10–70 backward (**CONTRADICTION** on the
  top of the range; GunBound's comes first).
- **Movement.** `[CR-aduka]`: "good climbing" but "short movement range"; `[SW-mob]`: "average
  mobility" (**CONTRADICTION**). Neither gives a number, so Flarecrow walks and climbs like most.
- **Weapon types.** `[CR-aduka]`: S1 is "an electrical type shot"; Thor's beam makes Aduka "a
  laser type creature so he inflicts the most damage to organic enemies and the least damage to
  shielded types". `[SNX-aduka]`: "the electric weapon, but it uses Thor's power as the main
  weapon". So S1 is Electric and the beams are Skyeye's Laser.

### S1: an electric ball and its aftershock

- `[SW-mob]`: "An electrical purple ball. Deals a weak aftershock." Damage "140-160 (includes
  aftershock)". `[DB-mob]` gives the same 140–160.
- `[CR-aduka]`: "if it hits it will do some damage for the direct hit and then some additional
  damage for the electrical 'splash'. The splash effect is large, so if Aduka misses completely the
  splash can still hit and even kill you. It also transfers from bot to bot, e.g: if Aduka shoots me
  and you're right next to me, and BOB is next to you...I will eat the normal damage, you will eat
  the splash damage only (about 70 points of damage), and then BOB will get some splash damage from
  you (much less, 15-30 points of damage). In close quarters maps like the cave, it's not unusual
  for a shot 1's splash effect to ripple down a line of metal bots". "It makes an unremarkable small
  hole if it misses." Typical damage 160.
- `[SNX-aduka]`: "Weapon 1 has electrical damage added to it, so it's good to make some additional
  damage and can give splash damage."
- **As built:** a small blast (18 px) of 70–90 Electric, then the aftershock: a flat 70 on every
  mobile within 50 px of the landing, the one struck included, and from each mobile it hits a flat
  22 to every mobile within 60 px of that one's center, and on down the line. It is splash
  (`[GF-max]`): flat, once a turn on each mobile, Dual or not. A direct hit totals 140–160 against a
  neutral armor. **Picks:** the 70–90 split (so the total is `[SW-mob]`'s), both reaches, and 22
  (the middle of `[CR-aduka]`'s 15–30). `[CR-aduka]`'s "metal bots" suggests Mechanical mobiles
  conduct farther; like Sparkhorn's splash, Flarecrow's ignores armor class.

### S2: three tracer flares, three beams

- `[SW-mob]`: "It fires three mini-adukas tracers. Wherever the tracers landed, Thor immediately
  fires 3 beams directly at where the mini-adukas landed. If there is land or another player in
  between the mini-adukas and Thor, Thor will hit the land or other player instead." Damage
  140–500.
- `[CR-aduka]`: "Aduka's shot 2 fires a 3-pack of ghostly Aduka images. Whenever those images hit
  something (an enemy, a friend, or just dirt) then Thor will lash out with a laser strike. This
  happens even when the Thor weather effect is not active. Each of the 3 images causes 1 Thor
  strike". "Since Aduka's shot 2 is entirely Thor damage, with no base damage of its own, you will
  do crappy damage to start with (about 150)", "180 or so" at level 2, "220-250" at level 3. On how
  the three fly: "the first tracer destroys land on the way towards the enemy, then the 2nd tracer
  travels past that point. It too will destroy some land, and by the time the third tracer hits
  something, you hopefully are landing the shot near or on your enemy", and "if you land in front
  the shot gradually drills its way closer and closer to the target". A beam meant for a tracer
  behind a mobile passes through that mobile; "Thor hardly moves the enemy".
- `[GF-lee]` (in `weather.md`): Aduka's S2 and SS call beams with or without the weather and get no
  extra ones during it.
- `[CR-aduka]`: "No added damage from force or Thor (since you're already using Thor to attack)";
  `[SNX-aduka]`: "thor's beam does not get stat bonuses from avatar customs".
- **As built:** three flares leave down one path, 0.3 s apart, and land in turn. Each calls one
  Skyeye beam (`shots::skyeye::beam`, the same beam as the weather's, at Skyeye's level) a beat
  after it lands, toward where it landed: the first thing in the beam's way takes it, and its crater
  lets the next flare fly on. The flares hurt and carve nothing; Force, Drain and items leave the
  beams alone, as they do the weather's; Thunder or Electricity still calls its bolt where each
  lands. Every beam's damage feeds the one satellite's XP. **Pick:** the 0.3 s between flares, a
  beat longer than a beam's delay, so each beam lands before the next flare arrives.

### SS: a mini-cart that passes through everything

- `[SW-mob]`: "Similar to Nak's SS, it is a mini-aduka that ignores all land. It is different from
  Nak's SS because it doesn't explode as soon as it hits someone. Instead, whenever it hits a player
  on the opposite team, Thor shoots a beam right at it, and it continues to fly through the map. It
  doesn't do much damage, but it can hit many opponents at the same time". Under a hurricane "the
  Aduka SS can hit him twice (even 3-4 times)". Damage 80–250, the same as `[DB-mob]`.
- `[CR-aduka]`: "it will travel through land as if the land didn't even exist. Unlike Nak's shot,
  it doesn't stop and explode when it hits something solid... it just keeps on going." "When the
  shot successfully passes through an enemy's body, Thor will fire a heavy hitting laser attack at
  that enemy (I think even at a low level it's like 200-250 damage for a single strike, which is
  much more powerful than a normal hit). If the shot passes through a teammate's body, I think it
  doesn't do any damage." Typical damage "250-470 (? total guess?)".
- **As built:** the SS flies through land and mobiles until it leaves the map. Each time it passes
  into a living enemy, Skyeye beams at that enemy's center a beat later, at 1.6 times its level's
  damage. A teammate it passes takes nothing. A mirror or a tornado that sends it back through an
  enemy earns a second beam. **Fit:** 1.6, from `[SW-mob]`'s own ranges: its SS 80–250 for one beam
  over its S2's 140–500 for three is 80 over 47 (1.7) at level 1 and 250 over 167 (1.5) at the top.
  **CONTRADICTION:** `[CR-aduka]` guesses 200–250 a beam even at a low level. Lobshot's tornado
  flings a shot up and on rather than spinning it round, so it seldom brings the SS back through
  the same enemy as GunBound's did.

### Shot weight

`[CR-fixed]`'s cbchui chart and `[CR-aduka]`'s own fixed-power rule give 2.35 bars for 60 degrees,
one screen; `[CR-aduka]` adds "for 1 full screen I use a tiny bit more power, like 2.4 bars", and
`[GN-aduka]`'s table says 2.40. Flarecrow is fitted midway, at 2.375, so both hold (`tool_fit` in
`crates/sim/tests/formula.rs`); its gravity is 591.5 px/s², from its drawn mortar.

`[CR-aduka]`'s fixed-angle table (credited to the KNAT guild) and `[GN-aduka]`'s are nearly the
same table. The acceptance set (`formula_flarecrow_holds_adukas_power_tables`, within 0.05 bar):

| Entry | Source | Says | Model |
|---|---|---|---|
| 60°, ½ screen | `[GN-aduka]`, `[CR-aduka]` | 1.60 | 1.615 |
| 80°, ¾ screen | `[GN-aduka]`, `[CR-aduka]` | 3.30 | 3.285 |
| 80°, 1 screen | `[GN-aduka]` | 3.80 | 3.825 |
| "3 bar formula": 1 screen = 70° at 2.75 | `[CR-aduka]` | 2.75 | 2.773 |

And `[CR-aduka]`'s full-power high angles, within a degree
(`formula_flarecrow_full_power_lobs_land_where_creedo_says`): 87° lands ⅓ screen, 84° ⅔ and 81° one
screen. The model gives exactly those three.

Left out, and why:

| Entry | Says | Model | Why it's out |
|---|---|---|---|
| 70°, ¼ to 1 screen (both tables) | 1.35, 2.00, 2.45, 2.85 | 1.25, 1.90, 2.38, 2.77 | The whole 70° column runs 0.08 to 0.10 bar heavier than the 60° and 80° columns of the same table; `[CR-aduka]`'s own 3-bar rule puts one screen at 70° on 2.75, which holds. |
| 60°, ¼ and ¾ screen (both tables) | 1.10, 2.10 | 1.05, 2.03 | Just past the tolerance; the ½ and 1-screen entries between them hold. |
| 80°, ¼ and ½ screen (`[GN-aduka]`) | 1.70, 2.40 | 1.76, 2.64 | Short 80° entries disagree between sources by up to 0.3 bar (`physics-fit.md`, BJSL). |
| 80°, ¼, ½ and 1 screen (`[CR-aduka]`) | 1.95, 2.70, 3.90 | 1.76, 2.64, 3.83 | As above: `[CR-aduka]` and `[GN-aduka]` disagree on these by 0.1 to 0.3. |
| 85°, ½ screen (`[GN-aduka]`) | 3.80 | 3.72 | A "super high" shot whose angle steps are wider than the tolerance. |
| 60°, 1¼ and 1½ screens (`[GN-aduka]`) | 3.10, 3.60 | 2.68, 2.95 | Contradicted by `[CR-aduka]`'s 3-bar rule (1½ screens at 2.85) and by Banpao (3.0); see `physics-fit.md`. |
| 70°, 1¼ and 1½ screens (`[GN-aduka]`) | 3.25, 3.65 | 3.12, 3.43 | As above. |
| 3-bar ½ screen = 80° at 2.55, 1½ screens = 60° at 2.85 (`[CR-aduka]`) | 2.55, 2.85 | 2.64, 2.95 | A rule of thumb that "may need to adjust your power level" between angles. |
| 81° "unfull", about 3.8, just short of one screen (`[CR-aduka]`) | 3.8 | 4.03 at one screen | Not at a stated distance. |

## More sources, and what they contradict

A second pass read the GameFAQs guides, Wikipedia's 2006 text and a few later guides. Keys
`[GF-lee]`, `[GF-vgg]`, `[GF-cyber]`, `[CR-all]` and `[NAMU]` are defined in `weather.md`,
`[GF-tiro]`, `[WP06]`, `[AK]` and `[SW-weather]` in `mechanics.md`, and `[GB-wiki]` in
`thistleback.md`. None changes a number in `sim::tuning`; they are here for the record.

- **Two Adukas.** Softnyx's April 2004 patch turned S2 from two markers into three tracers and made
  the SS pass through everything (`[GF-lee]`: "THE ONLY CHANGE THAT OCCURRED HERE IS THAT YOU SHOOT
  THREE INSTEAD OF TWO"; `[CR-aduka]`, `[GF-cyber]`). `[GF-tiro]` (March 2004) describes the old one,
  so it is set aside for S2 and the SS. `[SW-mob]` and `[DB-mob]` share their Aduka text word for
  word: one source, not two.
- **S1 damage (CONTRADICTION).** 140–160 with its aftershock `[SW-mob]`; typically 160 `[CR-aduka]`;
  175 `[GF-lee]`; "150 damage on hit. Out of the 150, 64 damage is splash" `[GF-max]`; "130 + 82
  Splash" `[GF-cyber]`; 150 `[BR-tut]`. `[GF-lee]` gives it "practically non-existent" bunge.
- **Delays (CONTRADICTION).** 760 / 910 / 1310 (`[CR-aduka]`, `[GF-vgg]`, `[OB]`) against 820 / 960
  / 1320 (`[GF-lee]`, `[NEO-ramza]`).
- **The tracers.** "The tracer shots don't spread and follow themselves much like Boomerlauncher's
  multiple boomers: if the first tracer shot hits the second will land at the same spot too" and
  "do not deal any damage by themselves" `[GF-lee]`. `[DBR]` launches three identical tracers 0.2 s
  apart.
- **Force on S2 (CONTRADICTION).** `[GF-lee]` (2004-06-04): "corrected the workings of Aduka shot 2
  (it does get status effects)". `[CR-aduka]`: "shot 2 doesn't get any help from the force effect";
  `[SW-weather]`: "shots which do not deal damage by themselves will be unaffected by Force". Lobshot
  follows Creedo and StrategyWiki: the beams are Skyeye's, whatever the flare flew through.
- **Thor's damage by level (CONTRADICTION).** S2 about 150, 180, then 220–250 `[CR-aduka]`; a
  level 1 beam 100+ up to 600+ at level 6 `[GF-lee]` (uncertain, its author says); averages 150+ to
  300+ `[GF-vgg]`; "an average max of 350 per hit" at level 6 `[GF-max]`; about 250 a shot
  `[GF-smash]`. Skyeye's table (`tuning::weather::SKYEYE_DAMAGE`) is the weather's, shared.
- **The SS.** It "continues to go, until flying/dropping out of the level" `[GF-vgg]`, "each time
  that it passes through a mobile, it triggers Thor to fire a single laser" `[GF-lee]`, and it
  "bypasses teammates" `[GF-vgg]`; `[GF-cyber]` dissents ("fire at any player the shot hits"). An
  arc that crosses an enemy going up and again coming down hits twice, "una de subida y otra de
  bajada" `[TP-aduka]` (single source): lobshot's SS does the same, a beam for each entry. Its damage:
  80–250 `[SW-mob]`, 100–200 `[WP06]`, 150 to 230 by level `[BR-tut]`. `[GF-lee]` gives the SS 30
  degrees of angle, not 40 (**CONTRADICTION**; `[CR-mob]` and `[GF-vgg]` give 40).
- **Rear fire is cosmetic** for players: "This has absolutely no tactical effect since any tank can
  face either direction at will, but it does look cool" (`[GF-nak]`, of Nak). Guides quote angles
  as elevations toward the target (`[CR-aduka]`), as lobshot's HUD does.
- **One satellite for everyone.** "If anyone else is using aduka, your shots get stronger at a
  faster rate, even if the other aduka is on the enemy's team" `[CR-all]`; it hangs "in the air at
  the map centre" `[NAMU]`, in a "fixed position" `[GB-wiki]`.

### Sources added by this pass

- `[GF-nak]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/25277 (exploder's Nakmachine
  FAQ, 2003-08-25; archive 20190526114140)
- `[GF-smash]` https://gamefaqs.gamespot.com/pc/582632-gunbound/faqs/31866 (Smashnuke's A.Sate
  FAQ v1.2, 2004–2005; archive 20190526114147)
- `[NEO-ramza]` https://www.neoseeker.com/gunbound/faqs/83681-e.html (RamzaBeoulve259's FAQ v0.92,
  2003–04; archive 20180316021125)
- `[TP-aduka]` https://www.teamperu.com/foros/temas/aduka.1290/ ("GUIA DEL ADUKA", Team Peru forum,
  2006, Spanish)
- `[BR-tut]` http://tutoriaison-games.blogspot.com/2011/01/tutorial-completo-sobre-aduka.html (a
  Brazilian Aduka tutorial, 2011; low quality)
