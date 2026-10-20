# OpenBound (WickedPeanuts/OpenBound): research notes for lobshot

Source: https://github.com/WickedPeanuts/OpenBound. Read at the default branch `dev-2`,
commit `d11e8127`, from a shallow clone. Nothing was built or run.
All paths below are relative to the repo root. "L" means line numbers in that file.

---

## 1. What it is

| | |
|---|---|
| Language | C# (~2.0 MB of C#), .NET Core 3.x era |
| Engine | MonoGame (XNA API: `Microsoft.Xna.Framework`), desktop (Windows-first; WinForms launcher/patcher) |
| Components | `OpenBound/` (game client), `OpenBound Login Server`, `OpenBound Lobby Server`, `OpenBound Game Server`, `Openbound Network Object Library` (shared models + constants), `Avatar API` (ASP.NET donation shop with PayPal and Stripe), `OpenBound Game Launcher`, `OpenBound Patcher`, `OpenBound Asset Tools` (decoders for GunBound file formats), `OpenBound Management Tools` (Docker templates) |
| Storage | EF Core on SQL Server |
| Activity | Created on GitHub 2020-10-31 (README says it started 11/2018 on Azure DevOps). There are 46 commits on `dev-2`, and the last one is from **2021-05-03**. After the Oct 2020 import, almost every commit is patcher, launcher, or Docker work, not gameplay. Branch `dev-3` adds one commit (2024-09-22, "Switching lang demo"), which changes fonts and localization in 44 files and touches no mechanics. 14 stars, 5 forks, no issues. Effectively dormant. |

### Netcode model: a trusting relay with per-client simulation

- Transport is TCP with JSON (Newtonsoft) using short property keys. Request IDs are in
  `Openbound Network Object Library/Common/NetworkObjectParameters.cs` L38-91, and buffers are 4 KB.
- **The server owns:** turn order and delay bookkeeping (`Entity/MatchManager.cs`), the wind and weather
  RNG (`Entity/MatchMetadata.cs`), spawn assignment, the SS lock counter, the KO win check
  (`GameClientProvider.cs` L998-1023), and gold/XP rewards (L600-938). The server RNG is
  `new Random(RandomSeed=10)` (NetworkObjectParameters.cs L125-126). It is a fixed seed shared by the whole
  server process.
- **The clients own, and the server trusts:** their own position, crosshair angle, shot parameters, HP and
  shield, and deaths. `SyncMobile.Update` copies `CurrentHealth`/`CurrentShield`/`Position`/... straight
  from the client (`Entity/Sync/SyncMobile.cs` L70-82). Any client can report a death, and the first
  report wins (`GameClientProvider.cs` L940-969). The shooter also reports the damage stats used for
  rewards (L649 only checks that the sender is the shooter).
- **Wire payloads.** `SyncMobile` carries {OW player, MM {CH hp, CS shield}, PO int[2] pos relative to map
  top-left, CA crosshair angle int, SS shot type, FA facing, CM cause of death, SP SyncProjectile, DE delay,
  IA alive, SL SS-lock turns, UI used item, SI actions [LeftMovement, RightMovement, UnableToMove,
  ChargingShot]} (`SyncMobile.cs` L32-47). `SyncProjectile` = {ST shotType, CP cannon pos float[2],
  SA angle (radians), SS strength float} (`SyncProjectile.cs` L17-23). Mobile sync is sent on input
  edges (key press and release), not every frame (`Pawn/Mobile.cs` L316-395).
- **Shot flow.** The client sends `RequestShot(SyncMobile)`. The server adds delay, re-sorts, and
  broadcasts the same SyncMobile (`GameClientProvider.cs` L539-556). **Every client then simulates the
  projectile, terrain carving, and damage locally** (`Level/Scene/LevelScene.cs` L418-453). For a remote
  shooter, the client first busy-waits (`Thread.Sleep(100)` loop, L433) until the remote mobile has
  walked to its last reported position.
- **Turn advance.** Each client runs a background "turn requester" thread that polls once a second. When
  the local sim is done and nothing is falling, it asks the server for the next turn (`LevelScene.cs`
  L483-508), and the server answers with `MatchManager`. The client sleeps 3000 ms ("wind change"), then
  regenerates every mobile and grants the turn (L324-363).
- Nothing checks determinism: no checksums and no hashing. Floats are rounded in places,
  e.g. `Math.Round(Math.Cos(angle),3)` (`MobileAction/Projectile.cs` L154-161), to keep results
  similar across machines.
- The README admits desync problems: "A few mobile projectiles and mobile animations are unsynched"
  and "Weather animations are not fps proof. A laggy player can have different wind settings"
  (`OpenBound/README.md`, Known bugs).

### Completeness

- **Playable mobiles (10 + Random):** Armor, Bigfoot, Dragon, Ice, Knight, Lightning, Mage, Raon
  Launcher, Trico, Turtle (`Pawn/ActorBuilder.cs`). Aduka, A.Sate, Boomer, Grub, JD, JFrog,
  Kalsiddon, and Nak exist only as data rows (aim, delay, HP) with no behavior.
- **Weather:** Force, Tornado, Electricity, Weakness, Mirror, Random, and Thor are implemented. "Wind"
  re-rolls the wind. Protection and Ignorance exist only as enum values (the Protection logic is commented out).
- **Sudden death** (types BigBomb/Double/SS, at turn 40/56/72): enum plus HUD sprite only, **no logic.**
- **Game modes** Solo/Score/Tag/Jewel: UI only. The only win condition is knockout.
- **Items:** all 11 GunBound items are implemented (see §3.8).
- Code also exists for the lobby, rooms, chat, avatar shop, ranks, gold/XP, patcher, and Docker deploy.
  I read it but didn't run it. The README lists known bugs, such as "It is possible to freeze the match
  by passing the turn".

---

## 2. Licensing and asset provenance

**Code: GPLv3.** See `LICENCE.GPL` at the root, plus a `LICENCE.gpl` in every sub-project and a GPL
header in every source file. The README also requires a copyright line:
"Copyright (C) 2020, Carlos H.M.S." Porting the code verbatim would put lobshot under GPLv3.

**Assets: ripped from the original GunBound (Softnyx) clients. Not redrawn and not original.** Evidence:

1. Root `README.md` L27: "OpenBound Game requires an texture pack in order to be built. Since all
   the textures belongs to it's owners, I can not share it in here." The pack is handed out through
   Discord. `.gitignore` excludes `OpenBound/Content` except `Content.mgcb`.
2. `OpenBound/README.md` L5-9: "The following clients were **datamined** in order to acquire all
   necessary assets": MagicPunk ("Non-GB game made by the same company"), GunBound Beta, WC Season 1
   (NA/EU/SA/KR), and Season 2 (NA/SA). L38: "Assets that couldn't be extracted from the original
   game: Dragon's Fireball (S1/S2)". Because that sprite was missing, Dragon S1/S2 render as an
   invisible dummy with a particle trail (`UnitProjectiles/DragonProjectile.cs` L53-62).
3. Placeholder sounds also come from another commercial game: Knight's gallop and neigh are
   "extracted from HoMM3" (`OpenBound/README.md` L48-49).
4. Root README L79: the asset tool "GunboundImageFix" (now `OpenBound Asset Tools/`) is "a set of tools for
   converting from encrypted & proprietary formats to PNG spritesheets". `Utils/IMGCracker.cs`
   (GunBound `.img` sprite format, documented at L73-110: 565/4444 pixel modes, pivots, rider pivots),
   `Utils/EPACracker.cs` (`.epa` animation tables), and `Utils/XTFCracker.cs` (`.xtf` 4444 textures) are
   the decoders. A prebuilt `DLL/ImgTools.dll` of unknown origin ships too.
5. Credits section (root README L102-106): "Game client assets/Datamined values: ursoGb, Owaran,
   Alexwathever".
6. A little GunBound art is committed anyway. `OpenBound Game Launcher/Resources/LauncherFrame.jpg`
   (1120x700) appears to be official GunBound promotional art (Armor and Turtle mobiles, Halloween
   theme). There are also `GBIcon.ico` and 12 `RaonLauncherMine-*.png` frames, which look like
   extracted sprites. `OpenBound/ContentMetadata/Mobile/*/RiderPivot.json` are pivot tables extracted
   from the `.img` headers.
7. Names are Softnyx IP throughout: mobile names, map names (Adiumroot, Cozy Tower, Dragon, Dummy
   Slope, Dungeon, Four Heads, I Love You, Meta Mine, Metropolis, Miramo Town, Nirvana, Planet Lond,
   Sea of Hero, Stardust; internal aliases "Metamine/Facky", "SeaOfHero/Candy" at `LevelScene.cs`
   L217, L230), the rank ladder, and the item names.

**Takeaway:** lobshot cannot use any OpenBound asset, and should avoid the GunBound names too.

---

## 3. Mechanics as implemented

Coordinates are screen pixels with y pointing down. World (0,0) is the center of the map foreground
texture (`Level/Topography.cs` L148-156).

### 3.1 Projectile physics (`MobileAction/Projectile.cs`, `Physics/AcceleratedMovement.cs`, `Common/Parameter.cs`)

- **Power.** The power bar holds 0..100. Holding space adds +2 every time a frame timer passes 0.04 s.
  At 60 fps that is every 3 frames (0.05 s), so 0 to 100 takes **2.5 s**, and the shot fires
  automatically at 100. Constants are `Parameter.cs` L175-176 (`ShotStrenghtTimeLimit=0.04`,
  `ShotStrenghtBarStep=2`) and the bar logic is `Interface/ProgressBar.cs` L93-102. The result is 51
  discrete power values.
- **Angle.** World shot angle = body tilt + aim angle, truncated to integer degrees:
  `ToRadians((int)ToDegrees(CrosshairPointer.Rotation) - 90)` (`Interface/Crosshair.cs` L341-344).
  Aim is clamped to the mobile's "false" range; the "true" range only changes the HUD font (L227,
  L460-469).
- **Initial velocity:** `v0 = power * ProjectileMovementForceFactor(100) / mass`. Velocity components
  are `round(cos θ,3)` and `round(sin θ,3)` (Projectile.cs L154-155, L177-181). Mass is 14 (Armor,
  Bigfoot, Ice, Lightning, Mage, Turtle) or 10 (Dragon, Knight, Raon, Trico), so v0 at full power is
  **714 px/s** at mass 14 and **1000 px/s** at mass 10.
- **Acceleration:** `ax = cos(wA)*W*windInfluence`, `ay = 196 + sin(wA)*W*windInfluence`, with
  `ProjectileMovementGravity = 9.8*20 = 196` (Parameter.cs L134; Projectile.cs L179-180).
- **Integration** is closed form, not Euler: `x(t) = (v0 + a·t)·t` (`AcceleratedMovement.cs` L40-49).
  **Quirk:** this equals `v0·t + a·t²`, with no ½ factor, so the effective accelerations are doubled:
  **g_eff = 392 px/s²**, and wind_eff = 2·W·influence. Max wind (34 × 1.2 × 2 ≈ 82 px/s²) is about
  21% of gravity, or about 24% with influence 1.4.
- **Timestep.** The game uses MonoGame's fixed timestep at 60 fps (`Game1.cs` L49). Each frame advances
  the projectile `ProjectileMovementTotalTimeElapsed = 0.015` s of sim time in sub-steps of
  `ProjectileMovementTimeElapsedPerInteraction = 0.0005` s (nominally 30 per frame). It checks
  out-of-bounds and collision after every sub-step (Projectile.cs L273-278; Parameter.cs L136-137).
  Sim time therefore runs at 0.9× real time. At 1000 px/s each sub-step moves 0.5 px, so tunneling
  can't happen.
- **Worked numbers** (flat ground, no wind, 45°):
  - Mass 14, P=100: range ≈ 1300 px, flight ≈ 2.6 s sim (2.9 s real).
  - Mass 10, P=100: range ≈ 2550 px.
- **Bounds.** A projectile is destroyed when x leaves [0, width), when y ≥ map bottom, or when y is more
  than **300 px above the map top** (`ProjectilePlayableMapAreaYLimit = -300`, Parameter.cs L140;
  `Topography.cs` L160-167). There is no terrain collision above the map top (L181).
- **Per-projectile timers.** `SpawnTime` is a launch-animation delay before the projectile moves,
  e.g. 0.7 s for Ice S1/SS, Trico S1, and Raon. `InteractionTime` is the delay for a Dual second shot.
  `FreezeTime` is used by Knight swords.

### 3.2 Wind (`Entity/MatchMetadata.cs`, `NetworkObjectParameters.cs` L134-138)

- State is an integer force and an integer angle in degrees (full 360°). It is applied as a constant
  acceleration vector on both axes, scaled by each projectile's `windInfluence` (1.2 by default; 1.4 for
  Dragon and Knight; 0 for the Teleport beacon).
- The match starts with force 0 and angle 0. Wind only changes at a **round** boundary (TurnCount ==
  player count, L75-86):
  - `DisturbWind` (L148-156): 50% chance of force += U{-2..+1} (clamped ≥0) and angle += U{-3..+2}.
    Both deltas are biased downward by an integer-division off-by-one.
  - When the "Wind" weather comes up (L166-170): force = U[0,34] and angle = U[0,359]. The max is
    `Next(0,35)`, which is exclusive, so 35 is never rolled.
- Falling mobiles drift sideways with wind: an accumulator gets += windX/45 per frame, clamped to ±1,
  and moves 1 px when it saturates (`MobileAction/Motion/Movement.cs` L174-192). That is roughly
  30 px/s at max wind.
- The item ChangeWind does angle += 180 (`Mobile.cs` L490-493).

### 3.3 Terrain (`Level/Topography.cs`)

- **Representation.** A `bool[height][width]` mask comes from the foreground PNG, with solid where
  `alpha > 0` (L107-124). The PNG pixels themselves are kept for redraw. Maps are just two images per map
  (`Graphics/Maps/{map}/Background` and `Foreground{A|B|C}`; `LevelScene.cs` L204-208). Background
  animations are hard-coded per map.
- **Carving** (`CreateErosion`, L52-102): every pixel with dist ≤ R becomes transparent and non-solid.
  A 3 px ring outside it (`BlastBlackmaskRadius=3`) gets RGB×0.4 (scorch). R is
  `ExplosionRadius × (1 + Dig/100)` (Projectile.cs L439). The function returns the number of pixels
  removed, which only drives particles. There is no falling sand or terrain gravity.
- **Bug:** the texture index is `h * CollidableForegroundMatrix.Length + w` (L76), which uses height where
  it needs width. That is only correct for square foregrounds. I couldn't verify map sizes because the
  textures aren't in the repo; spawn points reach about 1850×1790.
- **Collision** samples one point (the projectile center, truncated to int) against the mask
  (L175-184). Mobiles use a rotated-rectangle point test (`Collision/CollisionBox.cs` L99-121).

### 3.4 Movement (`MobileAction/Motion/Movement.cs`, `Parameter.cs` L120-131)

- **Walking.** The first 0.1 s of holding the key does nothing (`TankMovementSidewaysDelay`). After that
  the mobile moves **1 px per frame (60 px/s)** (`TankMovementSpeed=1`; L114-123).
- **Budget.** Each 1 px step costs one step. `MaximumStepsPerTurn = 90` for every mobile except Knight,
  which gets 100. That is about 1.5 s of walking per turn. Movement adds **no delay**. The HUD movement
  bar is 100/steps per step.
- **Slopes** (L78-104). For the next column it scans from **6 px up (`TankMovementMaxYStepping`) to 5 px
  down**:
  - It steps to the top solid pixel under free space.
  - If all 12 cells are solid, the move is a wall and the mobile is "unable to move".
  - If there is no ground within 5 px down, the mobile steps to +6 and starts falling.

  That allows climbing slopes up to roughly 80°.
- **Body tilt** (`UpdateAngle`, L231-340): atan2 between the surface points sampled ±12 px horizontally
  (`RotationCalculationOffsetX=12`) within ±25 px vertically. Tilt is forced to 0 while falling. The aim
  is relative to the body.
- **Falling** (`ApplyGravity`, L143-226): the fall starts after a 0.05 s delay, then the mobile drops
  `GravitySpeed` px per frame, starting at 3 and adding 0.15 per frame. That is 180 px/s initially,
  accelerating about 540 px/s². There is **no fall damage** anywhere in the code.
- **Death off the map:** when the mobile's top passes the map bottom (or goes 300 px above the top),
  `RequestDeath(Bungee)` fires (L145-149).
- **Bug:** the side-edge check uses `MapHeight` for X (L91). Walking past the edge keeps your height,
  then you fall out.
- The player can't shoot while falling (the shot is cancelled, `Mobile.cs` L632-636), and can't use
  items while falling (`HUD.cs` L470).

### 3.5 Turn order and delay (`Entity/MatchManager.cs`, `Entity/MobileMetadata.cs`)

- **Initial order.** Both teams are shuffled and a coin flip picks which team starts. Then the list
  alternates A, B, A, B (MatchManager.cs L40-73). Everyone starts at **delay 0**.
- **On shot:** `delay += GetDelay(shotType)`, i.e. S1/S2/SS from the table below,
  × `(1 - AttackDelay/100)` from avatar stats (L82; MobileMetadata.cs L416-428). Then
  `SyncMobileList.OrderBy(Delay)`.
- **Next player** = the first alive in that list (L34). **Tie-break:** LINQ `OrderBy` is a stable sort,
  so a tied player keeps their previous relative order. The player who just shot was already ahead of
  the others in that list, so on a tie the shooter goes first again.
- **Pass or timeout:** +200 (`TurnSkipDelayCost`). The turn timer is 30 s (`TimePerPlayerTurn`;
  `HUD.cs` L568, L605).
- **Item:** `delay += itemCost × (1 - ItemDelay/100)` (MatchManager.cs L115-119).
- **Movement:** no delay.
- The `DelayPreset.Base` column exists but is **never read**.
- **Delay table** (MobileMetadata.cs L293-310; the comments look copied from a community table). "Base"
  is unused.

| Mobile | Base | S1 | S2 | SS |
|---|---|---|---|---|
| Aduka | 710 | 760 | 910 | 1310 |
| Armor | 720 | 770 | 960 | 1320 |
| A.Sate | 680 | 730 | 880 | 1280 |
| Bigfoot | 720 | 770 | 920 | 1320 |
| Boomer | 680 | 730 | 880 | 1280 |
| Dragon | 710 | 760 | 910 | 1310 |
| Grub | 690 | 740 | 890 | 1290 |
| Ice | 690 | 740 | 890 | 1290 |
| JD | 690 | 740 | 830 | 1290 |
| JFrog | 690 | 740 | 890 | 1290 |
| Kalsiddon | 710 | 760 | 910 | 1310 |
| Knight | 690 | 740 | 890 | 1290 |
| Lightning | 700 | 750 | 800 | 1300 |
| Mage | 700 | 750 | 900 | 1300 |
| Nak | 710 | 760 | 910 | 1310 |
| Raon | 700 | 750 | 900 | 1300 |
| Trico | 690 | 740 | 890 | 1290 |
| Turtle | 690 | 740 | 890 | 1290 |

- **Rounds.** A round is N turns, where N is the number of players (`OriginalTeamSize` = both teams,
  RoomMetadata.cs L180). Each round enqueues one weather event and may disturb the wind. The wind is only
  fully re-rolled when the event is "Wind" (§3.2).
- **Pacing:** 2000 ms before the first turn and 3000 ms between turns (`Parameter.cs` L65-66).

### 3.6 Mobiles: stats, hitboxes, aim

HP is base HP × (1 + avatar Health/100). Regen is applied to **every mobile at every turn change**,
× (1 + avatar Regeneration/100) (`Mobile.cs` L573-596, `LevelScene.cs` L350-356). Damage hits the
shield first, then HP (`Mobile.cs` L788-798).

The data (MobileMetadata.cs L363-380, unit ctors in `Pawn/Unit/*.cs`) is below.

**Aim ranges** use the format "true min-max / false (clamp) min-max" in degrees, sourced from
creedo.gbgl-hq.com per L105:

| Mobile | HP | Shield | HP regen/turn | Shield regen/turn | Hitbox w×h (offset) | Steps | Aim S1 | Aim S2 | Aim SS |
|---|---|---|---|---|---|---|---|---|---|
| Armor | 1000 | 0 | 0 | 0 | 30×40 (0,10) | 90 | 25-40/10-55 | 25-40/10-50 | 10-50/10-50 |
| Bigfoot | 1100 | 0 | 0 | 0 | 46×40 (0,10) | 90 | 18-40/18-40 | 18-40/18-40 | 23-35/23-35 |
| Dragon | 1000 | 0 | 10 | 0 | 36×38 (0,-10) | 90 | 30-60/30-60 | same | same |
| Ice | 1200 | 0 | 10 | 0 | 40×33 (0,10) | 90 | 30-60/20-70 | 30-60/20-70 | 30-60/30-60 |
| Knight | 1000 | 0 | 0 | 0 | 45×30 (0,10) | **100** | 10-80/10-80 | same | same |
| Lightning | 760 | 220 | 0 | 20 | 41×30 (0,10) | 90 | 23-35/18-40 | 23-35/18-40 | 23-35/23-35 |
| Mage | 760 | 220 | 0 | 20 | 38×33 (0,10) | 90 | 15-50/15-50 | same | same |
| Raon Launcher | 1000 | 0 | 0 | 0 | 30×38 (0,0) | 90 | 10-50/10-50 | same | same |
| Trico | 1100 | 0 | 10 | 0 | 40×33 (0,10) | 90 | 20-40/10-50 | 20-40/10-50 | 20-40/20-40 |
| Turtle | 950 | 0 | 10 | 0 | 38×33 (0,10) | 90 | 25-50/25-50 | same | same |

The data-only mobiles have these HP values: Aduka 1000, A.Sate 760+220 shield, Boomer 1000, Grub 1000,
JD 750+250 shield, JFrog 930, Kalsiddon 1100, Nak 1100.

The "MobileStatus" Attack/Defence/Energy/Mobility/Delay bars and the attack archetypes
(Electrical/Laser/Fire) are **display-only**. The one exception is the armor archetype: Bionic gets +3%
from EnergyUp items and +8% from Blood (`Mobile.cs` L552-560). The values are sourced from strategywiki
(L313).

### 3.7 Shot catalogue (`Pawn/UnitProjectiles/*.cs`, constants in `Parameter.cs` L245-507)

Explosion radius is **25 px for almost everything**, which suggests the values were guessed rather than
datamined.

| Mobile | S1 | S2 | SS |
|---|---|---|---|
| Armor (m14, wind 1.2) | 150 dmg | 200 dmg, then the projectile *continues* through its own crater and explodes again for 80 dmg r20 (destroy suppressed on first hit; ArmorProjectile.cs L88-109) | 150 dmg; after 2.0 s of flight it pauses for an "opening" animation, then resumes with +50 dmg (L160-189) |
| Bigfoot (m14) | 4 shots at +3°,-3°,0°,0°, staggered 0.2 s, 90 dmg each (BigfootProjectile.cs L28-36) | 2 waves × 3 shots with power -4/0/+4, at 0.2 s and 0.4 s, 70 dmg | 8 shots (±3°,0,0 pattern ×2) staggered 0.125 s, 90 dmg |
| Dragon (m10, wind 1.4) | 1 shot, 100 dmg (invisible + particle trail) | 4 shots: power +5/0/+10/0, angle ∓3°, staggered 0.2 s, 100 dmg | carrier (0 dmg); on impact, 5 blades spawn 300 px away at -40,-20,0,+20,+40° from straight up, staggered 0.3 s, and accelerate into the impact point (v0 100, a 200), 100 dmg r25 each (L144-207). README says the stagger was changed from the original. |
| Ice (m14) | 180 dmg (0.7 s spawn delay) | 220 dmg, and each hit lowers the target's **Defense by 5 (cumulative cap -35, lasts the whole match)** (IceProjectile.cs L90-108) | 320 dmg |
| Knight (m10, wind 1.4) | carrier (0 dmg); on impact, **3 swords** launch from the satellite at owner+(0,-150), spaced 15 px perpendicular, flying straight at **800 px/s** to the impact point, 80 dmg r25 each (KnightProjectile.cs L46-73, L209-213) | same with **5 swords** from owner+(0,-300) | 7 swords from impact+(0,-300)-(45,0)·cos(60°·i), spawning 0.4 + i/7.5 s, 80 dmg each |
| Lightning (m14) | 95 dmg + a "bolt": ray-march upward from the impact in 3 px steps to the **highest solid point in that column**, explode there (95 dmg r25), plus 55 shock dmg to mobiles within 25 px of the ray above it (BeamDummyProjectile.cs L100-119, LightningProjectile.cs L45-58) | 75 dmg + 4 diagonal bolts (45/135/225/315°), each 100 dmg + 35 shock | 200 dmg; then every mobile within 100 px gets a vertical bolt at its position (95 + 55 shock) |
| Mage (m14) | 150 dmg (straight) | 2 strands on a helix (±15 px amplitude, 1 Hz, phase 0/π), **each a separate 200 dmg r25 hit** (MageProjectile.cs L160-180) | 200 dmg r25 (**bug: uses S2 constants; `ProjectileMageSSBaseDamage=300` unused**) + adds the target's current shield to damage, and strips shields of mobiles within 100 px (L212-238) |
| Raon (m10) | 4 sub-projectiles braided around an invisible carrier (12 px sinusoid, 1 Hz, phases 90° apart), 100 dmg **r15** each (RaonLauncherProjectile.cs L33-39, L159-189) | 2 shots (power ±3); each lands as a **mine**. On Raon's later turns each mine walks toward the nearest mobile if it is within 90 px (≤90 steps/turn) and explodes on contact for 200 dmg r15. Mines are killed by tornado and set off by any explosion damage (RaonLauncherMineS2.cs; RaonLauncher.cs L62-88) | lands as a crawler mine that walks forward, flips at walls, and explodes on contact or after 360 steps: **500 dmg r30** |
| Trico (m10) | 200 dmg | 3 projectiles orbiting a carrier at (35,0),(0,0),(-35,0), rotating 0.5 rev/s with offsets ramping in at 2/s, 180 dmg each (TricoProjectile.cs L186-211) | on impact: 1 + 8 explosions, 0.1 s apart, at 20 px around the impact covering all 8 compass directions, 120 dmg r25 each (L271-307) |
| Turtle (m14) | 150 dmg | 2 helix strands (±20 px, 1.5 Hz); after 2 s the amplitude decays at 0.8/s so the strands converge; 200 dmg each (TurtleProjectile.cs L171-195) | 50 dmg if it hits early; otherwise after 1.5 s it splits into **6 bubbles** spread -7.5..+7.5° in 3° steps, at **power/3**, 60 dmg r15 each (L32-49, L232-261) |

### 3.8 Damage, death, items, SS

- **Damage** (Projectile.cs L409-427):
  1. Gate: the explosion center must be within `ExplosionRadius` of the closest point on the target's
     rotated hitbox.
  2. `dmg = BaseDamage × ExplosionRadius / dist(explosion, hitbox center)`. This is inverse-distance
     and **unclamped**: it exceeds BaseDamage whenever the blast is closer than R to the center, and
     divides by about 0 for a dead-center hit.
  3. `× (1 + (attacker.Attack − target.Defense)/100)` from avatar attributes.
  4. Force/Weakness/Mirror weather modify `BaseDamage` in flight (×1.2+10, ÷1.2−10, ×1.1+5;
     Parameter.cs L518-529).
  5. Power-Up (+30) and Blood (+33) add to Attack for the turn.
  6. A projectile touching two mobiles in one sub-step calls `Explode()` once per mobile
     (L378-388, a bug).
- **Death:** HP ≤ 0 gives `CausaMortis.NoHealth`. Leaving the map gives `Bungee`. A bungee kill is
  credited to the current turn owner (`Mobile.cs` L397-411).
- **SS:** there is **no damage-charged gauge**. Using SS sets `SSLockRemainingTurns += SSCooldownTimer(3)+1`.
  The lock goes down by 1 at the start of each of your own turns, and SS re-enables at 0. That means 3
  own turns locked and SS available again on the 4th (MatchManager.cs L90-92; SyncMobile.cs
  L100-106; NetworkObjectParameters.cs L285). The value -1 is reserved for "sudden death", which is
  never set.
- **Items** (`Entity/Item.cs`; effects `Mobile.cs` L443-527, L870-903; values NetworkObjectParameters.cs
  L208-232). The delay column is the cost added to your delay:

| Item | Slots | Delay | Effect |
|---|---|---|---|
| Dual | 2 | 600 | shoot the same shot twice (2nd after 1 s + spawn time) |
| Dual+ | 2 | 450 | shot + the other of S1/S2 |
| Thunder | 2 | 200 | invisible giant Electricity weather on you: your shot spawns a bolt on impact |
| Energy Up 2 | 2 | 350 | +30% max HP (+3% Bionic) |
| Team Teleport | 2 | 50 | swap position with lowest-HP living ally |
| Teleport | 2 | 150 | fire a beacon (mass 10, no wind) and teleport to impact |
| Blood | 1 | 0 | +33 Attack this turn, costs 8% base HP (min 1) |
| Bunge Shot | 1 | 80 | +25 Dig (crater ×1.25) this turn |
| Power Up | 1 | 150 | +30 Attack this turn |
| Change Wind | 1 | 120 | wind angle +180° |
| Energy Up 1 | 1 | 100 | +12% HP (+3% Bionic) |

- **Weather** (`WeatherEffect/*.cs`). Each weather is a vertical column about 40 px wide (70 for
  Tornado, 80 for Random), and overlapping columns of the same type merge.
  - Tornado: inside the column the projectile flies straight at its current speed (min 25), mirrors
    its X, then exits and re-launches ballistically along its new heading.
  - Mirror: negates the x-velocity and x-wind.
  - Electricity: a bolt on explosion, 0 base + 30 shock, r30.
  - Thor: a satellite beams the impact point for 40 + 20×level dmg. Thor levels with damage dealt;
    XP thresholds are 800/1400/2000/2400/3000.
  - Random: becomes one of the five basic weathers on first touch.
  - The spawn weights are per-map integer arrays. The `Map.cs` L38-40 comment admits they are invented
    for most maps.
- **Rewards** are GunBound's named bonuses (GameClientProvider.cs L709-813): high angle (70-120° and
  flight >2.5 s), ultra high (>4 s), boomer shot, back shot, tornado/mirror shot, and excellent/good/simple
  shot (>500/>250/>150 enemy dmg). There is a dead branch: the 2000-dmg check repeats the 1000 condition
  (L786 vs L792).

### 3.9 Map format (`Entity/Map.cs`)

There is no map file format. Maps are a hard-coded C# list (L43-95) of
`(id, name, type A/B/C, weather weights[10], spawn points int[,] in foreground-pixel coords,
ground-particle frame count/pivot)`, plus `Background.png` and `Foreground{A,B,C}.png` from the
texture pack. The 14 GunBound maps have 1-3 variants each. Spawns are picked at random from the list
(RoomMetadata.cs L198-208). Coordinates go up to about 1850×1790 px. The original game is described as
800×600 at 30 fps (`OpenBound/README.md` L78-79).

---

## 4. What the code records about original GunBound

- `Entity/MobileMetadata.cs` L21-28: GunBound file naming for shot assets: `n`=S1, `nl`=S1 SFX,
  `s`=S2, `sl`, `p`=SS, `pl`.
- Same file: the aim true/false ranges (L105-288, from creedo.gbgl-hq.com/mobiles.htm; JFrog and
  Kalsiddon marked "NEED SOURCE"), the full **delay table** for 18 mobiles (L291-311), the
  HP/shield/regen table (L363-380), and status archetypes (L313, from strategywiki.org/wiki/Gunbound).
  These are the most valuable "faithful" numbers in the repo. Note that **projectile damage, radii,
  masses, and wind influence are not sourced** and look tuned.
- `NetworkObjectParameters.cs` L168-204: the GunBound rank ladder (Chick → Wood Hammer → … → Dragon,
  plus Champion/World Champion/VIP/GM) with XP thresholds. Item delay costs and the gold bonus
  table are at L206-282.
- `Entity/Map.cs`: GunBound map roster and A/B/C variants. Weather weights are explicitly made up
  where "No information available".
- `Entity/RoomMetadata.cs` L20-55: room options (Solo/Score/Tag/Jewel; sudden death
  NoDeath/BigBomb/Double/SS at 40/56/72 turns; 1v1-4v4; Basic/Attack slot modes).
- `OpenBound/README.md` §General.7: Lightning was changed to "work like the season 2 GB" (the main
  projectile does a fraction of the damage and the discharge does the rest), and Dragon SS was changed.
  The README also lists which mobiles have no movement sound in the original.
- `OpenBound Asset Tools/Utils/IMGCracker.cs` L73-110: reverse-engineered GunBound `.img` sprite
  container (header, color modes 565/1-bit/4444, pivots, rider pivots).
- `Movement.cs` L26-29 admits that movement does *not* match GunBound: "The method collisions are now
  calculated must be re-created to mimic original GB movement. Right now im out of ideas."

---

## 5. What lobshot should borrow vs. ignore

### Borrow (as numbers and ideas; reimplement, don't transliterate GPL code)

- **The data tables with external provenance:** the per-mobile S1/S2/SS delay table, aim true/false
  ranges, HP/shield/regen values, item delay costs, a 200 delay for pass or timeout, a 30 s turn timer,
  and a 3-own-turn SS lock. They drop straight into a `sim` crate as const tables.
- **The turn model:** accumulated delay, then a stable sort ascending, with alternating-team initial
  order. Make the tie-break explicit in lobshot rather than relying on sort stability, and decide
  whether a tie favors the shooter, as it does here.
- **The physics shape:** power 0-100 in steps of 2 (2.5 s fill), v0 = P·100/mass, constant gravity plus
  a 2D wind vector (force × per-projectile influence), and a **closed-form trajectory sampled at fixed
  sub-steps**. Closed form from integer tick counts is very deterministic-friendly. Fold the t² quirk
  into the constants if you want this feel: g_eff ≈ 392 px/s², wind up to about 82 px/s².
- **Terrain:** a 1-bit solid mask from alpha, circular carve with a Dig multiplier, and a cosmetic scorch
  ring. Point sampling of the projectile center is fine with 0.5 px steps.
- **Movement algorithm:** per-pixel stepping, climb ≤6 px and drop ≤5 px per pixel, otherwise fall; a
  step budget per turn; body tilt from surface sampling at ±12 px; aim relative to body; falling
  accelerates per frame; falling drifts with wind; bungee death. Tune the budget (90 px feels short) and
  check climb limits against GunBound footage.
- **Shot-behavior archetypes** as a reusable vocabulary for lobshot mobiles:
  - staggered fans (angle and power offsets)
  - helix and braid strands (each strand a separate hit)
  - orbiters
  - drill-through second explosion
  - mid-air transform or split after N seconds, with child power ÷3
  - delayed "strike from above" summons aimed at the impact (Knight/Dragon)
  - multi-blast rings (Trico SS)
  - upward-raycast lightning to the top surface, with side shock damage
  - persistent walking mines with their own turn phase
  - debuffs that persist across turns (Ice S2 −Defense)
- **Weather as vertical columns:** damage multipliers, a mirror that flips X, a tornado that re-launches.
  This is simple and readable, and works for a server-authoritative sim.

### Ignore or do differently

- **Netcode.** OpenBound trusts the client (HP, positions, deaths), has every client simulate
  independently with no state hash, uses thread sleeps and polling for turn sync, and shares one fixed
  `Random(10)` across all matches. Lobshot's plan (axum server running the pure `sim` crate
  authoritatively, clients sending inputs only, server broadcasting results or a seeded event log) is
  strictly better. At minimum, add a per-turn state hash if clients also simulate.
- **Damage formula.** `Base·R/dist` is unclamped and explodes near the center, and a multi-mobile overlap
  can call `Explode()` twice. Use a clamped falloff (e.g. full damage at the core, linear to the edge)
  and dedupe hits per projectile.
- **Damage and radius values.** Nearly everything is r25 and the masses are 10 or 14. These aren't
  GunBound data, so don't treat them as ground truth.
- **Ceiling.** The -300 px ceiling deletes very high shots. Verify against GunBound before copying;
  high-angle play there relies on shots leaving the screen.
- **Known bugs not to replicate:**
  - erosion index using height instead of width (`Topography.cs` L76)
  - edge check using MapHeight (`Movement.cs` L91)
  - Mage SS using S2 constants
  - biased wind disturbance (U{-2..1}, U{-3..2})
  - wind max 34 instead of 35
  - unused `DelayPreset.Base`
  - dead 2000-damage reward branch
- **Assets and names.** Everything visual and audio is ripped Softnyx (plus HoMM3) material. Mobile,
  map, item, and rank names are GunBound IP. Lobshot needs original art, audio, and names.
- **Code reuse.** It's GPLv3. Copying code (vs. re-deriving behavior from this write-up) would put
  lobshot under GPLv3.
