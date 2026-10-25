# Raon, for Tinbadger

GunBound's Raon (Raon Launcher), sourced, for lobshot's Tinbadger (PRD-0006 T-003). Pages were read
on 2026-10-24; quotes are verbatim fragments, typos kept. Keys already defined elsewhere are not
repeated: `[SW-mob]`, `[SW-mds]`, `[CR-mob]`, `[CR-fixed]`, `[DB-mob]`, `[DB-img-types]`, `[DBR]`,
`[WP06]`, `[GF-tiro]` and `[GF-max]` in `mechanics.md`; `[GF-lee]`, `[GF-vgg]`, `[GF-cyber]` and
`[NAMU]` in `weather.md`; `[GB-wiki]` and `[WP06-list]` in `thistleback.md`; `[GF-smash]` in
`flarecrow.md`; `[DB-live]` and `[OB]` in `README.md`.

Three sources are copies, not independent: DragonBound's guide copies StrategyWiki's text with its
own numbers, IGN's 2003 "Walkthrough" is `[GF-lee]`'s text, and Academic Kids copies an early
Wikipedia revision. OpenBound `[OB]` implements Raon in code (`RaonLauncher.cs`,
`RaonLauncherMine*.cs`, `RaonLauncherProjectile.cs`, `Parameter.cs` L375-397), with numbers of its
own: it is where most of lobshot's distances come from, as no GunBound source gives one.

## Numbers

| What | GunBound | Others |
|---|---|---|
| Type | Mechanical `[SW-mob]`, "Metallic" `[GF-lee]`, Machine `[GF-vgg]`, `[GF-cyber]` | DB Mech `[DB-mob]` |
| Weapon | Laser: `[GF-lee]`, `[GF-vgg]`, `[GF-cyber]`, `[GF-tiro]`, `[WP06]`, `[NAMU]`, `[GB-wiki]`; Explosion `[SW-mob]` alone (**CONTRADICTION**) | DB Laser `[DB-mob]`, `[DB-img-types]` |
| HP | 1000 `[SW-mob]`, `[GF-lee]`; 1050 `[CR-mob]` (**CONTRADICTION**) | DB 1000, OB 1000 |
| Delay (base / S1 / S2 / SS) | 500 / 250 / 400 / 800 `[SW-mob]`, `[SW-mds]`, `[CR-mob]`; the guides' totals 750 / 900 / 1300 agree; `[CR-blood]` has the SS at 1370 | DB the same |
| Damage S1 | 100-160 `[SW-mob]`; 50-60 a disc `[GF-max]`, 60 a disc `[GF-cyber]`; 140-180 `[GF-tiro]`; about 150 `[CR-blood]` | DB 100-160; OB 100 a disc |
| Damage S2 | 140-180 "per Raon mine" `[SW-mob]`; 130-150 each `[GF-max]`; 250 a bomb `[GF-cyber]`; the pair 275-360 `[CR-blood]`, `[CR-icy]`, `[GF-tiro]`, `[GF-lee]` | DB 140-180 per mine; OB 200 |
| Damage SS | none (`{{{ssdam}}}`, an unfilled template) `[SW-mob]`; 200-300 `[WP06]`; about 300 `[CR-tooya]`, `[CR-blood]`, `[GF-lee]`, `[GF-tiro]`; 330 `[GF-cyber]` | DB ??; OB 500 |
| Aim | 50-10, all true, every shot `[CR-mob]`; "True Angle Range: FULL" `[GF-tiro]`; GF-lee's widths 40 / 30 / 20 | OB 10-50; DBR 30-60 (clone) |
| Weight | 2.50 bars for 60 degrees at one screen `[CR-fixed]`, 2.5 for S2 and 2.6 for S1 `[CR-icy]`; "Heavier then average" `[GF-lee]`; "tied with A.Sate's for third heaviest" `[WP06-list]`; the SS lighter than S1 and S2 `[CR-tooya]`, `[CR-icy]` | DBR `ax` 73.5 |
| Moving | "low movement capacity but good at climbing" `[SW-mob]`; Movement 2/5, Climbing 2/5 `[GF-lee]` (**CONTRADICTION**); "great climbing ability" `[SNX-raon]` | OB 90 steps |

- lobshot takes 1000 HP, Laser (every GunBound source but SW-mob, the official text's Shield
  weakness, and GF-vgg's "4% Less Damage to Shields"), 10-50 all true, and every mobile's climb and
  move budget.
- One weight for all three shots: lobshot's gravity is per mobile, so the SS isn't lighter.
- S1's damage is SW-mob's 100-160 split among the three discs, though the guides find it hits
  harder.

## Shot 1: the discs

- "Three discs spinning around each other. They remove a lot of land" `[SW-mob]`; "three spinning
  blade disks that intertwine with each other, like the Mage's shot 2" `[GF-cyber]`; "circle around
  each other" `[GF-max]`; a helix of gears `[WP06]`, `[GB-wiki]`.
- "Shot 1 creates a deep hole" `[GF-vgg]`; "It eats land like crazy, in a straight, narrow hole ...
  It don't destroy much land with a direct hit" `[CR-blood]`.
- OB braids four discs 12 px off an invisible carrier at 1 Hz. lobshot braids GunBound's three on
  OB's helix, a third of a turn apart.

## Shot 2: the mines

- **Laid.** Two shells at different speeds (`[CR-blood]`, `[CR-tooya]`), so their spread depends on
  the angle, not the wind: none at 0 or 90 degrees, most at 45 (`[CR-icy]`, `[GF-vgg]`,
  `[CR-blood]`). They fly through mobiles: "the mines have to hit the ground first in order to
  explode" `[GF-vgg]`; "the babies have to hit ground to blow" `[CR-blood]`; OB's shells collide
  with the ground only.
- **Off at once.** "Unless they land directly on their target (in which case they immediately
  explode)" `[GF-lee]`. Where on the target: "raon mines blow when they walk into the enemy's
  center, what I like to call 'the sweetspot'" `[CR-blood]`; "they don't explode until they hit a
  certain point on the mobile, located in the center of it" `[GF-vgg]`; "seldom explode on
  contact, unless they hit exactly in the middle of a mobile" `[GF-max]`. lobshot: within 6 px of
  the hitbox's middle column.
- **Armed or asleep.** "If they are close to an enemy player, a mine's antenna will turn red and
  start spinning" `[SW-mob]`; eyes open or closed `[GF-lee]`; red for armed in most sources, green
  in `[WP06]` (after a 2006 edit) and `[GB-wiki]`. Range: "an approximate 1 inch radius" on screen
  `[GF-max]`, OB's 90 px. Out of it, a mobile can walk away `[GB-wiki]`.
- **When it moves.** On its layer's turn, before it acts: "when the original launcher's turn comes"
  `[SW-mob]`; "on your next turn" `[GF-lee]`, `[GF-tiro]`, `[GF-cyber]`, `[CR-icy]`; "at the
  beginning of one's next turn" `[WP-early]`; "Raon's ability can only be used when it receives
  energy from its master" `[SNX-raon]`; "mines always crawl directly inbetween turns" `[CR-blood]`.
  It may take "a few 'walkings'" to arrive `[GF-vgg]`; "every turn they move closer" `[GB-wiki]`.
  Outliers: "every so many turns" `[NAMU]`; GF-lee once says "the next player's turn", and once
  "only move on your own turn". PRD-0006's note, "mines take their own steps in the turn order", is
  OB's way of saying the same: each mine gets a step of its own just before its layer's turn.
- **How far.** No GunBound source gives a number, only that they "slowly progress" `[SW-mob]` and
  climb poorly `[GF-smash]`. lobshot crawls one up to OB's 90 px at OB's 60 px/s; DBR walks one at
  most 30 (**CONTRADICTION**).
- **Whom it hunts.** The nearest mobile, the layer's own side and the layer included:
  "(including the player)" `[WP06]`; "impossible to distinguish our allies from the dead"
  `[SNX-raon]`; it reacts to allies, yourself and corpses `[NAMU]`; "friend or foe" `[GB-wiki]`;
  "the mines may turn on you if you're close enough" `[GF-vgg]`. SW-mob's "enemy" is the outlier
  (**CONTRADICTION**). lobshot has no corpses to hunt.
- **What draws it.** "Raons are more attracted to Mechanical mobiles than Bionics and Shields"
  `[SW-mob]`; "wider for metal tanks" `[GF-lee]`; mechanical mobiles are sensed from far away,
  Bionics least `[NAMU]`; Shields "a normal attraction" `[WP06]`; "greatly reduced for bionic
  mobiles" `[GF-max]`. lobshot senses a Mechanical mobile 1.5 times as far as a Shield one, a Bionic
  one 0.6 times (picks: no source gives a number).
- **What sets it off.** Reaching a mobile's middle; a mobile running over it: "Running over a Raon
  explodes them immediately" `[GF-lee]`. Shots: "Shooting them will eliminate the threat"
  `[GF-lee]`; Dual+ with S1 second "to blow up the mines. This will deal 300+ damage easily"
  `[GF-tiro]`; "making both the mines explode with the following shot 1, causing more damage, such
  as 450" `[CR-icy]`; "Thor or lightning guarantee your shot to explode on the enemy" `[CR-tooya]`.
  Bolts and Thor: "Lightning bolts destroy mines more effectively than any other weapon in the
  game" `[GF-tiro]`; "Electrical splash damage will also eliminate them" `[GF-lee]`; "Thor kills
  the raon babies" `[GF-vgg]`. Other mobiles' specials: J.D.'s S2 pulls them in, Nak's and Aduka's
  SS hit them `[WP06]`; the big Raon goes off on them `[CR-icy]`. lobshot sets a mine off, with its
  own blast, when any blast, beam or bolt reaches it (`sim::mines::set_off`).
- **Tornadoes.** "If the mines are in the center of one at all, they get sucked up into oblivion,
  whether they crawl into it or not" `[CR-blood]`; also `[WP-early]`, `[NAMU]`.
- **How many.** "Up to eight Raons out at a time" `[GB-wiki]`; "a total of 8 out" `[GF-vgg]`;
  "babies(8)" `[CR-blood]`. Per player or per game is unclear; lobshot keeps eight a layer, its
  oldest going to make room (picked).
- **How long, and its layer's death.** No source gives a lifetime; the pit strategies
  (`[GF-vgg]`, `[CR-blood]`) imply one lasts until something sets it off. No GunBound source says
  what a dead layer's mines do; OB removes them (`RaonLauncher.Die`), and lobshot does the same.
- **Land.** Contested: "do some land damage also" `[GF-vgg]`; "they do not do land damage"
  `[CR-tooya]` (**CONTRADICTION**). lobshot digs OB's small crater.
- Dragon "can fly over raon mines" `[SW-mob]`; "you can't baby shotgun dragon" `[CR-blood]`.

## SS: the big Raon

- **Which way.** The way it was flying: "it starts walking in whatever direction it flys through
  the air. (IMPORTANT NOTE: If you shoot High angle SS against hook wind, you may suffer from
  watching your little clone walk in the opposite direction that you shot it...)" `[CR-tooya]`;
  "decided on which way the raon was shot from when it lands" `[GF-max]`; "the direction it was
  shot at" `[SW-mob]`; "the direction it's shot in" `[GF-vgg]`. The way its launcher faced:
  `[GF-cyber]`, `[WP06]` (**CONTRADICTION**). GF-lee alone has it home on the mobile nearest its
  landing spot.
- **Walls and edges.** It turns round at a wall "too steep to climb" (`[SW-mob]`, `[GF-vgg]`,
  `[GF-tiro]`, `[CR-icy]`); it can fall off a cliff or out of the map (`[GF-tiro]`, `[GF-cyber]`).
- **Fuse.** About 3 s `[GF-lee]`, `[CR-blood]`; 5 s `[WP-early]`; 5-6 s `[GF-vgg]`, `[CR-icy]`;
  5-7 s `[SW-mob]`. lobshot picks 6 s, OB's 360 steps at a pixel a frame.
- **What sets it off.** "If it walks into a player, it will explode regardless of ally, enemy, or
  corpse" `[SW-mob]`; "A target is not just a player, it can be another Raon mine or just a dead
  mobile" `[CR-icy]`. A floating mobile it passes under, as mines pass under it.
- **Damage.** It falls the longer it walks (`[GF-vgg]`, `[GF-max]`), or "could miss by a mile, and
  still do max damage" `[CR-tooya]` (**CONTRADICTION**: lobshot keeps it flat).

## Fitting its weight

Tinbadger's gravity is fitted like the Classic mobiles' (`physics-fit.md`, `tool_fit`), to
`[CR-fixed]`'s 2.50 bars: 659.0 px/s², between Coilback's 622 (2.45) and Thistleback's 748 (2.65).
It first shipped as 654.6, stale on arrival: the commit that added it already refits to 659.0, so no
later change moved it (most likely the fit was taken before its drawn geometry settled). Both land
2.50 within the chart test's 0.05 bars, so only a refit catches it
(`fitted_gravities_are_what_tool_fit_writes`, now).
DBR gives Raon Armor's `ax`, 73.5, which would make it 2.45 (**CONTRADICTION**; the chart comes
first). The full-power angles agree that it is heavy: 79 degrees for one screen and 84 for half
(`[GF-vgg]`, `[CR-blood]`, `[CR-tooya]`).

## Sources

- `[SNX-raon]` http://gunbound.softnyx.net/GameInfo/Mobileinfo/raon.html (official; archive
  20190719033329); the 2004 site (http://gunbound.net/gunbound/gun_Mobile.asp, archive
  20040215160120) has the same text
- `[WP-early]` Earlier revisions of Wikipedia's Gunbound article, read through the API: 7147589
  (2004-10-28), 10710057 (2005-03-02), 11671054 (2005-03-30), 19441419 (2005-07-23), 23246077
  (2005-09-14), 34507955 (2006-01-09), 44461928 (2006-03-19) and 46265242 (2006-03-31)
- `[CR-blood]` Bloodbug's Raon guide: https://creedo.gbgl-hq.com/raon_bloodbug.php (archive
  20060617105824), first at http://www.freewebtown.com/raoners/raonerguide.html (archive
  20041015045307)
- `[CR-tooya]` https://creedo.gbgl-hq.com/tooya_raon.php (archive 20060617105759)
- `[CR-icy]` https://creedo.gbgl-hq.com/icytalent_raon_guide.php (archive 20060617105912)
- `[GF-lee]` was read through its IGN mirror,
  https://www.ign.com/articles/2003/11/21/gunbound-walkthrough-450707; `[SW-mob]` through
  web.archive.org, 20250919061317, page oldid 904011.
