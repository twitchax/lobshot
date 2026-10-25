# Jewel mode: jewels for points, and no one hurt

Gathered on 2026-10-24 for PRD-0007 T-003. `sim::tuning::jewels` cites these keys; every key is
defined in `mechanics.md` (`[SW-modes]`, `[WP06]`, `[GF-tiro]`) or `weather.md` (`[GF-vgg]`,
`[GF-cyber]`, `[GF-kurai]`, `[GF-super]`, `[GF-lee]`, `[NAMU]`), so none is repeated here. `[SW-modes]`
was read again at its 2025-07-09 archive, the GameFAQs guides at their 2019-05-26 archives, `[NAMU]`
through its mirror, and `[WP06]` (oldid 55767667) for its Jewel and mobile entries. The fidelity
rule holds: GunBound Classic first, DragonBound where it is silent, a recorded pick where everyone
is.

## What the sources say

- **The objective.** "The objective is to destroy targets, called Jewels, for points. The first
  team to reach 100 points or exceed 100 points wins. (Note, it is 100 points regardless of the
  number of players.) Players do not take damage from attacks" `[SW-modes]`. "Players attack
  targets, known as 'Jewel' which give gold and points. Mobiles are invincible ... The first team
  to reach or exceed 100 points wins" `[WP06]`. First to 100 in `[GF-kurai]`, `[GF-cyber]`
  ("rack up at least 100 points by killing the Jewels"), `[GF-vgg]`, `[GF-super]` and `[NAMU]`
  ("100점 이상을 먼저 획득하면 승리": the first to 100 or more wins; "모빌간엔 데미지 판정은 없다":
  mobiles take no damage from each other). DragonBound's is the same (`mechanics.md` section 7).
- **The jewels.** "Jewels randomly drop from the sky, and are worth 5, 10, 25, or -5 points. The
  ones with -5 points become 5 points after sitting there a certain number of turns" `[SW-modes]`.
  Colors: "RED JEWELS: 25 points, 400 HP / YELLOW JEWELS: 10 points, 300 HP / BLUE JEWELS: 5
  points, 200 HP / BLACK JEWELS: -5 points, 200 HP. Black Jewels turn into Blue Jewels after a set
  amount of time" `[GF-cyber]`; black -5, blue 5, yellow 10, red 25, "the higher the score, the
  tougher" `[NAMU]`. "Shoot the little jewels scattered around the stage, worth either -5, 5, 10,
  or 25 points" `[GF-vgg]`; "weird little things that drop from the sky and have different point
  values" `[GF-super]`.
- **What one takes.** "It takes 100 damage for 5 or -5 points, 200 damage for 10 points, and 300
  damage for 25 points. The damage must be dealt in a single turn" `[SW-modes]`. "To kill a jewel,
  you must deplete it's HP in ONE turn, otherwise is is healed to the max" `[GF-cyber]`, whose HP
  are 200, 300, 400 and 200 (**CONTRADICTION**).
- **Who scores.** "The first team" `[SW-modes]`, `[WP06]`: a team's points, whoever on it shot.
- **Bunging.** "Land can still be destroyed, so it is possible to bunge enemies. If all players are
  bunged at the same time, the team loses. Otherwise, a bunged player can click to respawn like in
  Score. They will take two turns to respawn. Lives are not counted, so respawning is free"
  `[SW-modes]`. "When someone is bunged, they can drop back down at no penalty ... They also win if
  everybody on the other team is bunged" `[WP06]`. "If you aren't 1v1ing and you're bunged, you
  have to wait 2 turns to drop" `[GF-vgg]`. "If all of 1 teams mobiles are destroyed even if their
  points arent all the way gone, then the opposing team won" `[GF-kurai]`. The 2002 beta: "If you
  knock a competing tank off the field, they don't regenerate" `[GF-super]` (**CONTRADICTION**).
- **Items.** "Blood (the item) is very useful in this mode, as it has no extra delay, and normally
  drains you some of your health, which doesn't exist in this mode" `[GF-vgg]`; recommended items
  "Dual, Dual-Plus, BungeShot, Lightning" `[GF-cyber]`. Items are on.
- **The slot machine** belongs to Solo and Tag `[WP06]`; in Jewel the bunged drop back in instead.
- **Shots that meet jewels.** Nak's SS "explodes upon impact with a mobile, a jewel, or a
  mini-Raon"; Aduka's SS "triggers Thor's Hammer when it hits an enemy mobile ..., a jewel, or a
  mini-Raon"; J.D.'s S2 "sucks in surrounding mobiles, jewels and also mini-Raons" `[WP06]`.
- **Silent.** How many jewels fall, how often, where, and in what mix; whether the wind moves them;
  what becomes of one that falls into a pit or whose ground is blown away; how many turns a black
  one takes to turn blue; a turn or time limit.

## Rules as built, and the picks

| Question | Answer | Source |
|---|---|---|
| How a team wins | the first to 100 points, whatever its size | cited `[SW-modes]`, `[WP06]`, `[GF-kurai]`, `[GF-cyber]`, `[GF-vgg]`, `[GF-super]`, `[NAMU]` |
| How a team loses otherwise | no one of it standing: everyone bunged at once (or gone). A bunge in 1v1 ends the game | cited `[SW-modes]`, `[WP06]`, `[GF-kurai]`, `[GF-vgg]` |
| Damage to players | none, from anything: every hurt goes through `Game::hurt`, Lanternwing's strip through `Game::strip`, and both ask `Mode::harms`. Shots, mines, bolts, beams, lasers and drops still carve land | cited `[SW-modes]`, `[WP06]`, `[NAMU]` |
| Values | blue Sapphire 5, yellow Topaz 10, red Ruby 25, black Onyx -5 | cited `[SW-modes]`, `[GF-cyber]`, `[NAMU]`, `[GF-vgg]`; the names are ours, by GunBound's colors |
| Damage needed | 100, 200, 300, and 100 for the Onyx | cited `[SW-modes]` over `[GF-cyber]`'s 200 / 300 / 400 / 200 |
| In one turn | a jewel's damage adds up from its turn's start (its mines' crawl included) to the next turn's start, over every volley of a Dual or Double, and starts again from nothing | cited `[SW-modes]`, `[GF-cyber]`; the turn boundary is ours |
| Who scores | the team of the seat whose turn it is (its mines, crawling as its turn starts, score for it) | pick: the sources say only "the team" |
| A team's points | never below 0: an Onyx taken at 0 costs nothing | pick |
| The Onyx | turns blue (a Sapphire, 5 points) once it has sat 2 rounds (a turn a seat each) | cited that it turns `[SW-modes]`, `[GF-cyber]`; the 2 rounds are a pick |
| A jewel's size | a 32 px square sitting on the ground: half a mobile's height | pick: "weird little things" `[GF-super]`; big enough that a fair shot takes one |
| How damage reaches a jewel | as it would a mobile with no armor (no weapon strong or weak against it): every blast (`Game::explode_by`) and bolt (`bolt::zap`) in reach, direct when the blast goes off inside the jewel. Projectiles fly through jewels; the shots the sources name stop at one (below) | pick: the sources give jewels HP and say they take "damage", nothing about armor |
| What only mobiles take | splash, Flarecrow's aftershock, Marblemole's SS passing through and Sudsotter's suds: they touch mobiles, not jewels; their blasts still reach jewels | pick |
| Augerdillo's SS | goes off on the first mine or jewel it meets before a mobile, at its middle | cited `[WP06]` |
| Flarecrow's SS | calls a beam onto each jewel it passes through, as onto each enemy and mine | cited `[WP06]` |
| Tinbadger's SS | the big beetle goes off when it walks into a jewel, as into a mine | pick, after `[WP06]`'s Nak and Aduka: what meets a mini-Raon meets a jewel |
| J.D.'s S2 (Lodestoad) | drags jewels in reach to the ground under its center (`jewels::drag`) | cited `[WP06]` |
| How many, and when | 4 as the game starts; at each turn's end one more while the map has fewer than 8 | pick: "scattered around the stage" `[GF-vgg]`, "randomly drop" `[SW-modes]`; 8 leaves cheap ones in reach while the dear ones wait for an SS or a Dual |
| Where, and what | any column of the map, rolled through `sim::referee` (`Roll::Jewel`); the mix 6 : 2 : 1 : 1 Sapphire : Topaz : Ruby : Onyx | cited "randomly drop from the sky" `[SW-modes]`, `[GF-super]`; the mix is a pick, weighted to the cheap ones so a game flows: an untaken Topaz or Ruby sits on the map until a strong turn takes it |
| The fall | straight down from the sky at once, through mobiles, to the first ground; the wind doesn't move it | pick |
| A jewel that misses the ground | lost, down the pit or off the map | pick |
| Its ground blown away | it falls as a mobile does, and one with none left is gone | pick, after the mines (`tinbadger.md`) |
| The bunged | pick a drop point, as in Score, and drop in at the end of the 2nd turn after the pick's, whole; free | cited `[SW-modes]`, `[WP06]`, `[GF-vgg]`; counting from the turn after the pick's is Score's (`modes-score.md`) |
| Items | on; Sacrifice costs nothing, since there is no health to lose | cited `[GF-vgg]`, `[GF-cyber]` |
| The slot machine | off: the bunged drop back in instead | cited `[WP06]` |
| Turn or time limit | none beyond the room's sudden death, which plays as in any mode | pick: the sources name none |

## Contradictions resolved

- **The damage a jewel needs.** `[SW-modes]` gives 100 / 200 / 300 (and 100 for -5); `[GF-cyber]`
  gives HP of 200 / 300 / 400 (and 200). Lobshot follows `[SW-modes]`, the fidelity rule's first
  source; both agree a black one takes what a blue one does.
- **Whether the bunged come back.** `[GF-super]` (the 2002 beta) says they don't; `[SW-modes]`,
  `[WP06]` and `[GF-vgg]` say they drop back in after two turns. Lobshot follows the later three.

## SearchBot in Jewel

Not a source question, but the picks behind the bots (`sim::bot`): shots hurt no one, so a bot
searches a shot at the two jewels worth points that take least (nearest first) and the nearest
dearer one, from where it stands, and takes the best shot that takes one (a jewel's point is worth
30 damage, so a Sapphire's 5 make a strong shot), trying Dual or Power Up on it. When it can take
none, it digs: it shoots the ground at the nearest enemy's feet, walking for a better spot as it
would for a shot at an enemy, valuing each pixel the enemy falls (and a bunge as a kill). It never
aims at an Onyx while it costs points. A Normal bot takes a jewel about one turn in eight on open
ground, so a bot-only game to 100 runs a few hundred turns; the server's end-to-end test plays a
1v1 on a thin shelf, where the digging ends it sooner.

