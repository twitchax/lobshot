# Tag mode: two mobiles, one switch a turn

Gathered on 2026-10-24 for PRD-0007 T-002. `sim::tuning::modes` and `sim::tag` cite these keys;
every key is defined in `mechanics.md`, `weather.md` or `frostfin.md` (Sources), so none is
repeated here. The guides were read through the Wayback Machine at the archive dates those files
give. The fidelity rule holds: GunBound Classic first, DragonBound where it is silent, a recorded
pick where everyone is.

## What the sources say

- **Two mobiles, one life.** "Similarly to Solo, if you die, you are out. However, each player can
  choose two mobiles" `[SW-modes]`; "Again, you only have one life, but this time, you have another
  mobile you can switch to" `[GF-vgg]`; "Everyone has only one life. However, you get to change to
  a reserve mobile" `[GF-tiro]`; "You select 2 mobiles ... die once and your out" `[GF-kurai]`.
- **The second on half HP.** "The first one has full HP, and the second one has half HP"
  `[SW-modes]`; "The second bot will only have half life" `[GF-vgg]`; "1 will have full health and 1
  will have half hp" `[GF-kurai]`; "a second tank that has half as much life" `[GF-super]`; "태그형
  모빌은 체력이 절반인 상태로 시작된다" (the tag mobile starts on half HP) `[NAMU]`.
- **Once a turn, on your turn.** "Once per turn, you may choose to switch between the two mobiles"
  `[SW-modes]`; "you can keep changing the mobile indefinitely, but only once a turn, and only
  during your turn" `[GF-tiro]`; "After you switch bots during your turn, you can't switch back, so
  you're stuck until your turn again" `[GF-vgg]`; "You can switch tanks during your turn by hitting
  F7" `[GF-super]`.
- **Separate HP.** "Each mobile has its own HP bar, which are damaged or healed separately"
  `[SW-modes]`.
- **Out when the one in play falls.** "If the current mobile's HP is reduced to zero, the player is
  out, regardless of how much HP the other mobile has" `[SW-modes]`; "If either of your tanks die,
  you're out" `[GF-super]`. A bunge is a kill like any other: "An alternative way to kill a player
  is by destroying the ground beneath them" `[SW-modes]`.
- **Frostfin's brittleness is per mobile.** "During Tag games, the defense reduction only applies to
  the bot that was hit by Shot 2/SS; each bot has a separate counter" `[GF-ice]`.
- **The dead spin.** "Slots, like in solo, come into play here, as well as corpses left on the
  field" `[GF-vgg]`; "Tag also has a 'jackpot system'" `[GF-tiro]`; `[GF-kurai]` and `[WP06]` agree.
- **The key and the HUD.** F7 switches `[GF-vgg]`, `[GF-kurai]`, `[GF-super]`, `[GF-tiro]`,
  `[GF-cyber]`, `[NAMU]`. The bottom right of the screen "shows the tank you can tag too and also
  has the button for doing so" `[GF-super]`; "Tag shows your other bot and it's HP" `[GF-vgg]`.
- **The second is hidden.** "The first click you make will be the bot everyone sees you on. The
  second bot click will be your tag bot, which people won't see until you tag" `[GF-vgg]`.
- **Random.** In Tag a Random pick lands on a hidden mobile far more often than in other modes:
  "히든 모빌이 선택될 확률이 타 대전보다는 훨씬 높다" `[NAMU]`. Nothing else is said about Random.
- **DragonBound differs.** Its Tag switches to the backup, at half HP, only when the first is
  defeated (`mechanics.md` section 7, `[DB-lobby]`). PRD-0007 builds GunBound Classic's rule.

Nothing found, in any of those or in `[WP06]`, `[GF-lee]` or `[OB]` (whose modes are UI only),
says what a switch costs in delay, where the mobile coming in stands, what a mobile sitting out
keeps besides its HP, or how items are shared.

## Rules as built, and the picks

| Question | Answer | Source |
|---|---|---|
| Mobiles | two each: the first in play, the second sitting out | cited `[SW-modes]`, `[GF-vgg]`, `[GF-tiro]`, `[GF-kurai]` |
| The second's HP at the start | half its max (`TAG_SECOND_HP_PERCENT`, 50), rounding down | cited `[SW-modes]`, `[GF-vgg]`, `[GF-kurai]`, `[GF-super]`, `[NAMU]` |
| The second's shield at the start | full | pick: the sources speak of HP ("life") alone, and a shield regenerates in play anyway |
| When a player may switch | on its own turn, once, at any point before it shoots or passes | cited `[SW-modes]`, `[GF-tiro]`, `[GF-vgg]`; any point in the turn is a pick (no source restricts it) |
| What a switch costs | nothing: no delay of its own, and the turn goes on | pick: no source gives a cost, and "you're stuck until your turn again" `[GF-vgg]` says the turn is played out with the new mobile. The turn's delay is the mobile that ends it's (its base, its shot) |
| Moves after a switch | the steps walked this turn count against the new mobile's budget | pick: otherwise a switch would buy a second walk |
| Where the switched-in mobile stands | where the other stood, facing its way, tilted to the ground for its own build | pick: the sources are silent; a tag happens in place |
| What each mobile keeps on the bench | its own HP, shield, SS lock and brittleness | HP cited `[SW-modes]`; brittleness cited `[GF-ice]`; shield and SS lock are a pick, after them: each mobile is its own |
| What changes on the bench | nothing: no shield regeneration, SS countdown or Balm heal; nothing on the map can touch it | pick: the turn-start effects belong to the mobile taking the turn, and the bench is off the map |
| The loadout | one per player, shared by both mobiles; an item used before a switch rides on the turn's shot | pick: GunBound picks items once a room `[GF-vgg]` ("You have 6 item slots"), and no source gives a mobile its own |
| Out | when the mobile in play dies, blasted or bunged, whatever the other has left | cited `[SW-modes]`, `[GF-super]` |
| The dead | spin the slot machine, as in Solo | cited `[GF-vgg]`, `[GF-tiro]`, `[GF-kurai]`, `[WP06]` |
| Random picks | each of the two rolls on its own over every mobile, so both may be the same; any two picks may be the same mobile | pick: lobshot has no hidden mobiles, so `[NAMU]`'s odds don't apply, and no source forbids a pair |
| The second, seen by others | everyone sees both from the start | pick: every client replays the same log, which carries the setup, so lobshot can't hide it as `[GF-vgg]` describes |
| The key | F7, and T for a browser that keeps F7 for itself | cited `[GF-vgg]`, `[GF-super]`, `[GF-tiro]`; T is ours |

### Mobiles with state of their own

| Mobile | When it switches | Why |
|---|---|---|
| Tinbadger | its mines stay on the map and keep crawling as its seat's turn comes up; they go only when its player is out | pick: a mine belongs to its layer's seat (`tinbadger.md`), and switching out isn't dying |
| Lensowl, Needlemouse | the satellite hovers over the mobile in play: it leaves with its mobile and comes back with it | pick: a satellite is part of its mobile, and nothing about it persists between turns |
| Kitewyrm | it floats while in play; a grounded mobile switched in where it floated stands on the ground there, and sets off any mine at its sweet spot, as a mobile walking onto one does | pick, after `[GF-lee]` ("Running over a Raon explodes them immediately") and Score's drop onto a mine |

## Contradictions resolved

- **"Either" or "current".** `[GF-super]` (the 2002 beta) has a player out "if either of your
  tanks die"; `[SW-modes]` says the current one. They agree in play: the mobile sitting out can't be
  touched, so only the one in play can die.
- **GunBound Classic against DragonBound.** DragonBound switches to the backup at half HP when the
  first falls; GunBound Classic lets a player switch once a turn and puts them out when the one in
  play falls. PRD-0007's principle builds GunBound Classic's.
