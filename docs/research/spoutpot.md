# Spoutpot (GunBound's Turtle): sources

Gathered on 2026-10-22 for Spoutpot, which carries Turtle's behavior. The roster tables in
`mechanics.md` (§1.3, §2.1, §3.1, §3.2) and OpenBound's Turtle (`openbound.md` §3.6, §3.7) cover
its stats; this note adds the two sources that describe how its shots behave in detail.

## StrategyWiki's Turtle entry `[SW-mob]`

Read through the archived page (20250919061317). GunBound Classic values:

- Bionic, Physical. 950 HP. Base delay 490, and 12 delay a second of turn time, "the only mobile
  in the game" to pay more than 10. "Highest defense in Gunbound but poor mobility."
- S1: "A stream of water. Deals moderate damage." 130-160, delay 250.
- S2: "Two streams of water revolving around each other. It is very hard to get both streams of
  water to hit the opponent, unless you use a high angle or shotgun them. After about 3 seconds
  after shooting, the 2 streams of water will come together so they will both hit the opponent."
  220-280, delay **480** (CR-mob, SW-mds, CR-turtle and OB all give 400: **CONTRADICTION**).
- SS: "A ball of water that after a few seconds breaks into 6 smaller balls of water, which spread
  out as they go through the air." Aim it to split "directly in front of your target": "The point
  where SS will split is just a little earlier than the merging point of Shot2." All six landing
  deal "around 400-600". Delay 800.

## Creedo's Turtle guide `[CR-turtle]`

https://creedo.gbgl-hq.com/turtleguide/turtleguide2.php through `turtleguide30.php` (page 28 is the
`[CR-turtle snippet]` in `mechanics.md`). A GunBound Classic / Thor's Hammer player's guide, with
aiming tables for every Turtle shot. Its facts, by page:

- **2 (weapons).** S1: "a small round hole", typical damage 150, delay 740 in all. S2: "two
  streams of water that spiral through the air ... come from Turtle's cannon clustered together,
  then split apart, then come together again, and they keep repeating this fast spiral pattern 5
  or 6 times. After enough time in the air (about 2.5 seconds), the streams come together one last
  time and stay together", typical 260, delay 890 in all. SS: "one big water blob. After about
  2.15 seconds in the air, the blob splits into 6 smaller bombs. The bombs immediately fan out in
  a spread pattern" (Contra's spread gun), typical 550+, delay 1290 in all.
- **3 (strengths).** "Turtle's shot is the exact same weight" as Armor's (page 17 repeats it).
  **CONTRADICTION** with `[CR-fixed]`, which puts Turtle at 2.39 bars and Armor at 2.45. "Tiny,
  slow walking range." "Your shot's explosions are tiny ... Turtle's near-misses do very small
  damage." "Very little bunging ability", and the shot "doesn't blast through land easily".
- **4 (shooting).** "The wide spirals ... are so far apart that at their widest point, they will
  completely wrap around an enemy's body without touching him." A solid S2 does 240-300. "About
  2.2 bars of power is the minimum to get the shot to converge" at a high angle.
- **5 (shotgun).** Point blank at full power both streams hit before they part: "the shot starts
  spreading apart as soon as it leaves your cannon. The more power you use, the further the shot
  goes before it splits." That reaches "about 1/4th of a screen".
- **6 (short forks).** A fork lands as the streams cross: one spin in well under a second, two
  spins in "about twice as much airtime, although it will still be in the air less than a second".
- **7 (S1).** "1 makes a bigger explosion when it misses than shot 2 does."
- **19 (skybomb SS).** "The shot splits open after 2.2 seconds in the air", ideally at the top of
  its arc: "The way it spreads is to fan out in the direction the shot is moving ... if it's moving
  downward, it fans out horizontally (which is bad). If it's moving forward but has no downward
  momentum, it spreads out vertically (which is good)." A patch made the six balls fall "more
  quickly than before", so the SS "sort of changes weight in the middle of the shot ... the path
  of the shot looks a bit lopsided". Skybomb landmarks (angle, bars, distance): 89 3.67 56 px;
  88 3.68 112; 87 3.68 167; 86 3.69 220; 85 3.70 274; 84 3.70 328; 83 3.72 387; 82 3.74 446;
  81 3.77 503; 80 3.79 560; 79 3.82 617; 78 3.85 676; 77 3.88 735; 76 3.91 795 (about 1 screen);
  75 3.94 ~860; 74 3.97 ~933; 73 4.0 ~1009; 72 4.0 ~1088. Too early or too late, the SS is "a
  weak, lousy hit"; a spread one does "low 300's".
- **21-22 (timebomb SS).** Time the split "a split second before" the target, so the six balls hit
  "while they're still clustered close together". Landmarks (angle, bars, screens): 84 1.8 1/8,
  78 1.85 1/4, 71 1.9 3/8, 66 2.0 1/2, 60 2.05 5/8, 56 2.15 3/4, 52 2.3 7/8, 48 2.4 1; past one
  screen 43 2.66 1.25, 38 3.0 1.5, 32 3.28 1.75, 30 3.65 2. "Power controls airtime and angle
  controls distance."
- **26 (SS tips).** "When shot 2 spins through the air, it spirals 6 times. The middle of the final
  spiral is the point where the SS will open (just before the shot stops spinning and starts to
  come together)." With an Electricity column "you get a hit for every ball in the SS". "It's
  better if the timebomb opens and gets a crappy half hit than if it doesn't open at all."
- **27 (long forks).** A long fork hits "with the final (fifth) crossing of shot 2", in about 2 s
  of air ("~1.9 seconds", page 29). Landmarks (angle, bars, screens): 82 1.55 1/8, 73 1.61 1/4,
  65 1.69 3/8, 59 1.81 1/2, 53 1.92 5/8, 47 2.08 3/4, 43 2.25 7/8, 40 2.4 1. A shot 2 "that barely
  comes together after about 2.3 seconds" is a fake fork.
- **29 (FAQ).** "If you can feel shots pretty well with angle 60, your shot 2 will have enough
  airtime for 3/4 and 1 screen shots, but not enough airtime for 1/2 screen. Even with angle 70,
  the airtime is barely enough for a half screen shot."

The guide's angles run to 90: Turtle reaches them on slopes (page 3: "Easy to get a high angle
without a lot of slope, because it's placed so high"; "On flat land it can be difficult or
impossible to make a nice shot 2 for full damage"). Its aim arc is 25-50 above the body
(`[CR-mob]`, `[OB]`).

## How the times were read into lobshot

GunBound sources disagree on the times: S2 comes together after about 3 s (SW-mob, copied by
DB-mob) or about 2.5 s (CR-turtle); the SS opens after a few seconds (SW-mob), about 3 s (DB-mob),
3.25 s (WP06), 2.15-2.2 s (CR-turtle), or 1.5 s (OB). **CONTRADICTION.** CR-turtle's are the only
ones that keep SW-mob's own order (the SS opens a little before S2 comes together), and the only
ones with aiming tables behind them, so lobshot takes CR-turtle's shape and fits its clock to
those tables, as `physics-fit.md` fits gravity to the formulas. `cargo test -p lobshot-sim --test
formula -- --ignored --nocapture tool_fit` does the fits and prints every entry below.

- **Its crossings are one clock.** CR-turtle's three times keep one rhythm: the fifth crossing
  (forks) at about 1.9 s, the middle of the sixth spiral (the SS opening) at about 2.15 s, and the
  sixth crossing (coming together for good) at about 2.3 s: about 0.38 s a crossing.
- **The fork table sets the clock.** In lobshot's ballistics every long-fork landmark comes down
  through its target's height after the same airtime, 1.61 to 1.65 s, so that is the fifth
  crossing: S2 turns at 1.534 Hz (OB's own is 1.5 Hz), the SS opens at 5.5 crossings (1.79 s), and
  the streams come together for good at the sixth (1.96 s). lobshot's shots fly a GunBound
  landmark's angle and power a little faster than GunBound's do, so its seconds come out about a
  sixth shorter than the guide's.
- **The skybomb table sets the weight change.** Each SS ball keeps 0.464 of the blob's speed as it
  opens, fitted so the one-screen skybomb (76 degrees, 3.91 bars) lands 795 px out. The other
  sixteen land within 10 px or 2% of the table, but for the two longest (73 and 72 degrees: 3% and
  5% short).

Checks, not fits (`crates/sim/tests/formula.rs`):

- Every fork landmark lands within 0.06 crossings of the fifth.
- Every timebomb landmark (the eight up to 1 screen) comes down 0.07 to 0.13 s after the SS opens:
  "a split second" after.
- CR-turtle's FAQ: at 60 degrees S2 comes together at 1 screen (2.16 s of air) and not at 1/2
  (1.52 s); at 70 degrees it barely does at 1/2 (1.91 s, against 1.96 s).

Left out:

- The FAQ's 60 degrees at 3/4 screen, which it says has the airtime to come together: lobshot gives
  it 1.87 s, 0.09 s short.
- The fork and timebomb tables' distances. With their own powers, lobshot's shots land 15 to 25%
  long (the fork at 40 degrees, 2.4 bars, lands 931 px out, not 800), as `physics-fit.md` finds
  for the fixed-power tables away from 60 degrees. The airtimes, which are what those tables are
  for ("power controls airtime and angle controls distance"), hold.
