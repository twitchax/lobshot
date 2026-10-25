# Score mode: team lives and respawns

Gathered on 2026-10-24 for PRD-0007 T-001. `sim::tuning::modes` cites these keys; every key is
defined in `mechanics.md` or `weather.md` (Sources), so none is repeated here. The fidelity rule
holds: GunBound Classic first, DragonBound where it is silent, a recorded pick where everyone is.

## What the sources say

- **Lives.** "Each team has a number of lives, equal to one more than the number of players. So a
  3v3 game would have 4 lives on each team" `[SW-modes]`. The same in `[GF-tiro]` ("Your TEAM has
  X lives + 1"), `[GF-vgg]` and `[GF-kurai]` ("2v2 has 3, 3v3 has 4, 4v4 has 5").
- **What a death costs.** "Whenever a player is killed, their team loses a life" `[SW-modes]`;
  "Every time someone on your team dies, one life is exhausted" `[GF-tiro]`, `[GF-vgg]`. Killing
  and bunging both count: "kill or bunge the other team to bring its lives to 0" `[NAMU]`.
- **How a team loses.** "A team wins when they reduce the number of lives on the other team to
  zero, or eliminate all enemy players at the same time" `[SW-modes]`; "if no one on your team is on
  the field (all of you are waiting to drop), you lose the game" `[GF-vgg]`; DragonBound: "the
  first team to reach 0 points or run out of players alive… loses" (`mechanics.md` section 7).
  So 1v1 Score plays as Solo: "even with lives left, a team whose players are all dead loses, so
  1:1 is no different from Solo" `[NAMU]`.
- **Respawning.** "After a player is killed, they may click on the map at any time. That location
  will be shown to all players, along with a counter showing the number of turns until that player
  respawns. At this time, the player will drop from the sky at that location. Their landing is
  affected by Wind to a small degree" `[SW-modes]`; "you'll respawn somewhere around
  there (depending on the wind)" `[GF-tiro]`.
- **When.** "After clicking, it takes 4 turns, starting from the turn of the next player in play,
  to respawn" `[SW-modes]`. Four turns in `[GF-vgg]` ("you drop 4 turns after you pick your spawn
  spot"), `[GF-kurai]`, `[GF-super]` and `[NAMU]`.
- **Waiting is allowed.** "It may be advantageous to wait until the right moment to respawn,
  instead of doing so as soon as possible" `[SW-modes]`.
- **Turn on return.** "On a positive note, it will be your turn exactly 2 turns after you drop in"
  `[GF-super]` (the 2002 beta). `[SW-modes]` says only that the place in the turn list "depends on
  their Delay", deferring to its Delay page, which never comes back to it `[SW-delay]`.
- **A drop that misses.** "If you drop, but fail to land on any land, and fall straight through, it
  doesn't cost the team a life, but it does take you another 4 turns to fall again" `[GF-vgg]`.
- **No slot machine.** "No slot machine here" `[GF-kurai]`; "Slots and corpses obviously don't come
  into effect here" `[GF-vgg]`. `[WP06]` gives the slot machine to Solo and Tag only.
- **What carries over.** Ice's defence decrease "will carry on between the different lives"
  `[GF-lee]`: a mobile comes back as the same mobile, not a fresh one.

## Rules as built, and the picks

| Question | Answer | Source |
|---|---|---|
| Team lives | players on the team + 1 (each team counts its own) | cited `[SW-modes]`, `[GF-tiro]`, `[GF-vgg]`, `[GF-kurai]` |
| What a death costs | one life, however it died (blasted, bunged or forfeited) | cited `[SW-modes]`, `[GF-tiro]`, `[NAMU]`; a forfeit counting is a pick: a leaver's mobile dies, as in Solo |
| How a team loses | no lives left, or no one standing (all dead or waiting to drop) | cited `[SW-modes]`, `[GF-vgg]`, `[NAMU]` |
| Both teams out at once | a draw, as in Solo | pick: `[SW-modes]` thinks "the team which made the final move loses" but says the evidence is unclear |
| Who may come back | the dead of a team with lives left, unless they forfeited | cited `[SW-modes]`; a forfeiter staying out is a pick: they left the game |
| The last life | yes: a dead player may drop back in while the team has a life left; the death that takes the last one ends the game | cited `[SW-modes]` (a life goes on each death) |
| When it drops | at the end of the 4th turn after the turn it was picked in | cited `[SW-modes]`, `[GF-vgg]`, `[GF-kurai]`, `[GF-super]`, `[NAMU]` |
| The drop | from the sky at the picked column, the wind pushing it a quarter as hard as a shot | wind-affected: cited `[SW-modes]`, `[GF-tiro]`; the quarter is a pick ("to a small degree") |
| A drop that misses the ground | no life lost; the player picks again and waits the 4 turns again | cited `[GF-vgg]` |
| HP and shield on return | full | pick: the sources are silent; a life is a whole mobile |
| Its turn on return | the second turn to start after it lands is its own: its delay is set level with whoever moves next, and it loses the tie | cited `[GF-super]` ("your turn exactly 2 turns after you drop in"); ties going to others is ours |
| Items and SS on return | as they were: items used stay used, the rest stay, and an SS lock stands | pick, after `[GF-lee]` (a mobile's state carries between lives); GunBound brings one loadout a game |
| Brittleness (Frostfin) | carries over | cited `[GF-lee]` |
| A player who never picks | stays out; the game neither waits for them nor picks for them, and the turn clock never runs for the dead | cited `[SW-modes]` ("at any time", and waiting may pay) |
| The slot machine | off: the dead pick a drop point instead | cited `[GF-kurai]`, `[GF-vgg]`, `[WP06]` |
| Dropping in onto a mine | sets it off, as running over one does | pick, after `[GF-lee]` ("running over a Raon explodes them immediately"): a mobile landing on one meets it as one walking onto it does |
| A dead layer's mines | go with its death, as in every mode, and don't come back when it drops back in | pick: Tinbadger's rule (`tinbadger.md`) holds; a mine moves on its layer's turns, and a new life lays its own |

## Contradictions resolved

- **When the count starts.** `[GF-vgg]` and `[GF-kurai]` say "4 turns after you pick"; `[SW-modes]`
  says the 4 start "from the turn of the next player in play". Lobshot follows `[SW-modes]`, the
  most specific: the turn a point is picked in doesn't count.
- **Its turn on return.** `[GF-super]` gives a fixed "2 turns after you drop in"; `[SW-modes]` says
  it "depends on their Delay" without saying how. Lobshot sets the returning mobile's delay level
  with the next mover's, which puts its turn second after it lands, and still lets delay decide
  ties and every turn after.
