# Playtests

Qualitative findings from agents (or people) playing lobshot through the lab (PRD-0005 T-006).
The balance runs say how often a mobile wins; a playtest says why a game felt wrong: a shot that
misbehaves, a map spot with no way out, a rule the view made hard to follow. One file a session,
named `YYYY-MM-DD-<topic>.md`.

## How to play one

Build the maps once (`cargo make art`), then either:

- **MCP:** add the server to an MCP client, e.g. Claude Code:
  `claude mcp add lobshot -- cargo run --release -p lobshot-lab -- mcp`, run from the repo root.
  Its tools are `rules`, `new_game`, `view`, `trace` and `act`.
- **CLI:** `cargo run --release -p lobshot-lab -- play rules`, then `play new`, `play view`,
  `play trace` and `play act`, each a process over the save file `game.json`.

Read `rules` first. Play whole games, and note the seed and setup of each (every view reports the
seed): `play new` with the same setup and seed plays the same game, so a finding can be replayed.

## The format

```markdown
# Playtest: <what was played, in a line>

- **Date:** YYYY-MM-DD
- **Player:** <how it was played: an agent over MCP or the CLI, or a person>
- **Build:** <commit>
- **Games:** one line each: setup and seed, and how it ended.

## Findings

Most important first. Each:

### <the finding, as a claim>

- **Kind:** bug, balance, feel, or interface (the lab's view or tools).
- **Severity:** high (breaks a game), medium (skews one), or low (a rough edge).
- **What happened:** one to three sentences, with turn numbers.
- **Replay:** the game's setup and seed, and the turn.
- **Suggestion:** optional.

## Notes

Anything else worth a line: what worked well, what to try next time.
```

Keep findings to what was seen, not guessed at: "Hookclaw's S2 cluster missed a target it
straddled, turn 12", not "Hookclaw is weak". A balance claim needs a balance run behind it
(`cargo make balance`).
