# Raon, for Tinbadger

GunBound's Raon (Raon Launcher), sourced, for lobshot's Tinbadger (PRD-0006 T-003). Pages were read
on 2026-10-24; quotes are verbatim fragments. Keys already defined elsewhere are not repeated:
`[SW-mob]`, `[SW-mds]`, `[CR-mob]`, `[CR-fixed]`, `[DB-mob]`, `[DB-img-types]`, `[DBR]` and `[WP06]`
in `mechanics.md`, `[GF-lee]` in `weather.md`, `[DB-live]` and `[OB]` in `README.md`.

Two GunBound sources describe Raon's shots in any detail: StrategyWiki `[SW-mob]` (read through
web.archive.org, 20250919061317, page oldid 904011) and Lee T. HaXXXor's Mobile Guide `[GF-lee]`
(v1.6, 2004-04-21; read through its IGN mirror,
https://www.ign.com/articles/2003/11/21/gunbound-walkthrough-450707). OpenBound `[OB]` implements
Raon in code (`RaonLauncher.cs`, `RaonLauncherMine*.cs`, `RaonLauncherProjectile.cs`,
`Parameter.cs` L375-397), with its own numbers. DragonBound's guide `[DB-mob]` copies
StrategyWiki's text with its own numbers.

## Numbers

| What | GunBound | Others |
|---|---|---|
| Type | Mechanical `[SW-mob]`, "Metallic" `[GF-lee]` | DB Mech `[DB-mob]` |
| Weapon | Explosion `[SW-mob]`; Laser, every shot `[GF-lee]` (**CONTRADICTION**) | DB Laser `[DB-mob]`, `[DB-img-types]` |
| HP | 1000 `[SW-mob]`, "Approximate HP: 1000HP" `[GF-lee]`; 1050 `[CR-mob]` (**CONTRADICTION**) | DB 1000, OB 1000 |
| Delay (base / S1 / S2 / SS) | 500 / 250 / 400 / 800 `[SW-mob]`, `[SW-mds]`, `[CR-mob]`; GF-lee's totals 750 / 900 / 1300 agree | DB the same |
| Damage (S1 / S2 / SS) | 100-160 / 140-180 "(per Raon mine)" / none given (`{{{ssdam}}}`, an unfilled template) `[SW-mob]`; GF-lee's "AMD" 180 / 275+ / 300+ | DB 100-160 / 140-180 per mine / ??; OB 100 a disc, 200 a mine, 500 the SS |
| Aim | 50-10 `[CR-mob]`; GF-lee's angle ranges 40 / 30 / 20 degrees (its "range" is a width: Trico's 40 is CR-mob's 50-10) | OB 10-50 for all; DBR 30-60 (clone, "@TODO falta definir") |
| Weight | 2.50 bars for 60 degrees at one screen `[CR-fixed]`; "Heavier then average" `[GF-lee]` | DBR `ax` 73.5, Armor's |
| Moving | "High in defense, low movement capacity but good at climbing" `[SW-mob]`; Movement 2/5, Climbing 2/5 `[GF-lee]` (**CONTRADICTION**) | OB 90 steps, every mobile's |

- lobshot takes 1000 HP (SW-mob, GF-lee, DB and OB over CR-mob), Laser (GunBound's two sources
  disagree, so DragonBound's, which sides with GF-lee) and 10-50, all of it true (CR-mob gives no
  true range; OB keeps it all true).
- No source gives the SS a damage range. lobshot picks 300-500: GF-lee's "300+" at the bottom, OB's
  500 at the top.
- The movement contradiction leaves Tinbadger with every mobile's climb and move budget.

## Shot 1: the discs

- "Three discs spinning around each other. They remove a lot of land, so it can be good for
  bunging" `[SW-mob]`. GF-lee: "It fires a stream of mini mines at the opponent. I personally find
  it to be much stronger then it's written AMD... I consider it to be the best weapon 1 in the
  game!"
- OB braids four discs about an invisible carrier, 12 px off it at 1 Hz, phases a quarter turn
  apart, 100 damage r15 each. lobshot braids GunBound's three on OB's helix, a third of a turn
  apart, each its own hit, the 100-160 split between them, with craters wider than their reach.

## Shot 2: the mines

The heart of Raon, and the first thing in lobshot that outlives its turn.

- **Laid.** "Shoots 2 Raon mines" `[SW-mob]`; "fires two Raons (a smaller form of itself) in a
  hard to control arc. Unless they land directly on their target (in which case they immediately
  explode), the Raon stands alert, waiting for a target to come nearby" `[GF-lee]`. OB fires the two
  at power ±3 (of 100) and has them fly through mobiles to the ground (**CONTRADICTION** with
  GF-lee's direct hit; lobshot takes GF-lee's).
- **Armed and asleep.** "If they are close to an enemy player, a mine's antenna will turn red and
  start spinning" `[SW-mob]`; "If their eyes are open, it means that the Raon will rush the opponent
  on your next turn and explode. If their eyes are closed, then they enter a sleep state"
  `[GF-lee]`. OB arms a mine within 90 px of the nearest mobile.
- **When it moves.** "The mine will slowly progress toward the enemy it is targeting when the
  original launcher's turn comes, and detonate upon contact" `[SW-mob]`; GF-lee thanks a reader
  "for the interesting bit about Raons needing energy from its master meaning raons can only move
  on your own turn". OB gives each mine its own turn, one after another, just before Raon's (its
  `GrantTurn` daisy chain), up to 90 steps. DragonBound: "S2 homing mines that crawl on your turn"
  `[DB-live]`. PRD-0006's note, "mines take their own steps in the turn order", is OB's way of
  saying the same: they step before their layer acts, not on everyone's turn.
- **Whom it hunts.** "Raons are more attracted to Mechanical mobiles than Bionics and Shields"
  `[SW-mob]`; "The Raon alertness range seems to be wider for metal tanks compared to bionics...
  Raons are MUCH more likely to be attracted to non-bionic tank then bionics" `[GF-lee]` (non-bionic
  takes in Shield mobiles too: **CONTRADICTION**, lobshot takes SW-mob's Mechanical). No source says
  how much more: lobshot counts a Mechanical enemy half again as near (`MECHANICAL_PULL`).
- **What sets it off.** Contact: "detonate upon contact" `[SW-mob]`; "Running over a Raon explodes
  them immediately" `[GF-lee]`. Neither limits it to enemies; OB's mine checks whichever mobile is
  nearest, of any team. lobshot sets one off on touching any mobile.
- **What clears it.** "Shooting them will eliminate the threat. Electrical splash damage will also
  eliminate them" `[GF-lee]`. OB sets a mine off when a blast reaches it, and has a tornado kill it
  (**CONTRADICTION**: lobshot smashes a mine in a blast's or a bolt's reach, without its going
  off). Tornadoes are left out: lobshot's weather columns act on what flies, and Sudsotter's walking
  sponges already ignore them (CR-frog).
- **How long it lives.** No GunBound source says. OB keeps a mine until it goes off or leaves the
  map, and removes every mine of a Raon that dies (`RaonLauncher.Die`); lobshot does the same.
- "Shooting Raons at ultra close range is a gamble, but one that can offer huge returns (600+ when
  using duals)" `[GF-lee]`: four mines landing on one target.
- DBR (clone, unverified) walks a mine at most 30, its second shell 100 ms behind the first.
- Dragon "can fly over raon mines" `[SW-mob]`: `sim::mines::senses` is where a floating mobile is
  left out.

## SS: the big Raon

- "A big Raon. When it lands on the ground, it'll start walking the direction it was shot at. When
  it bumps into a wall that is too steep to climb, it will turn around the other way. After about
  5-7 seconds, it will explode automatically. If it walks into a player, it will explode
  regardless of ally, enemy, or corpse" `[SW-mob]`.
- GF-lee: "a bigger Raon which starts walking furiously about, not stopping until it finds an
  opponent and blows up into it. It usually targets the Mobile closest to its landing spot, but
  it'll just run off on its own if it doesn't find anything (warning: it doesn't climb obstacles
  so well). Usually lasts for 3 seconds before blowing up" (**CONTRADICTION**: a homing walk, and
  3 s). lobshot takes SW-mob's, which OB agrees with: OB walks it the way Raon faces, flips it at
  walls, and blows it up on touching any mobile or after 360 steps, 6 s at its pixel a frame; 500
  damage r30.
- The 5-7 s fuse is a range, which would need a roll; lobshot picks its middle, OB's 6 s, so the
  player can plan the walk. Corpses don't stop it in lobshot: a dead mobile has no hitbox here.

## Fitting its weight

Tinbadger's gravity is fitted like the Classic mobiles' (`physics-fit.md`, `tool_fit`), to
`[CR-fixed]`'s 2.50 bars: 654.6 px/s², between Coilback's 622 (2.45) and Thistleback's 748 (2.65).
DBR gives Raon Armor's `ax`, 73.5, which would make it 2.45 (**CONTRADICTION**; the chart comes
first).
