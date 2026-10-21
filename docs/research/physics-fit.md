# Fitting the ballistics

How lobshot's shot physics was matched to GunBound's community aiming formulas, which formulas
are acceptance tests (`crates/sim/tests/formula.rs`), and why the rest are not. Source keys are
defined in `mechanics.md`.

## The model

- **A vacuum parabola under constant acceleration,** sampled at ticks of 1/2048 s:
  `p(t) = origin + v·t + a·t²/2`. This is the shape OpenBound implements (`[OB]`) and the
  community's "generic formula" assumes (`[CR-nak]`).
- **Power maps linearly to launch speed,** 300 px/s per bar, the same for every mobile. This only
  sets flight time: an 80°, one-screen lob flies 3.8 s, close to GunBound's 4 s Ultra High Angle
  threshold (`[SW-gp]`).
- **Shot weight is per-mobile gravity.** Coilback (Armor) is fitted so 60° lands one screen at
  2.45 bars, Creedo's cbchui fixed-power value (`[CR-fixed]`). The result is 622 px/s². Every
  other mobile is fitted to its own value on the same chart: Lanternwing (Mage) 2.38, Hookclaw
  (Boomer) 2.25, Sparkhorn (Lightning) 2.30, Marblemole (Grub) 2.33, with each mobile's own
  muzzle and hitbox.
- **Wind is a constant acceleration,** a fraction of the shooter's gravity per wind unit, times the
  mobile's sensitivity. Vertical wind therefore acts exactly like a change of gravity, as `[CR-nak]`
  says.
- **Geometry counts.** Shots leave the muzzle (about 73 px above the ground for Coilback at 60°),
  and "lands at one screen" means passing through the target's hitbox center 800 px away.

## Wind: two rules, two factors

The community has two well-sourced wind rules:

- **Vertical:** 0.1 bar less power per 7 units of up-wind, and more for down-wind (`[CR-wind]`,
  `[GN-aduka]`).
- **Side:** at fixed power, move the angle half a degree per unit (`[CR-wind]`, `[CR-philo]`).

If wind pushed equally in every direction, the vertical rule would make the side rule a full degree
per unit, twice what players use. Each rule has two sources, so lobshot honors both: the
vertical factor (0.0114 of gravity per unit) is about 1.9 times the horizontal one (0.0062).
OpenBound's single factor matches the side rule (`[OB]`).

## Acceptance tests

Each must hold within 0.05 bar, or one degree for the angle rule. Only the first and the two wind
anchors were fitted; the rest are independent checks.

| Formula | Source | Says | Model |
|---|---|---|---|
| 60°, one screen (fitted) | `[CR-fixed]` | 2.45 | 2.45 |
| 60°, one screen, each other mobile (fitted) | `[CR-fixed]` | 2.25 to 2.38 | within 0.05 |
| Boomer's hook: 10 wind behind a 70°, one-screen shot saves 0.45 bar (Hookclaw's wind sensitivity, fitted) | `[UC-boomer]` | 0.45 | within 0.05 |
| 45°, one screen | `[GF-tiro]` | 2.25 | within 0.05 |
| 60°, 1.5 screens (Banpao) | `[CR-using]` | 3.0 | within 0.05 |
| 7 / 14 up-wind, 7 / 14 down-wind at 60° (7 up fitted) | `[CR-wind]`, `[GN-aduka]` | ∓0.1 per 7 | within 0.05 |
| 4 / 10 / 16 tail and head wind at 2.45 (10 fitted) | `[CR-wind]`, `[CR-philo]` | ±½° per unit | within 1° |

## Left out, and why

The model's numbers below are for Coilback; the Aduka tables are for a mobile about 2% lighter,
so scale their powers by 1.02 before comparing.

| Entry | Says | Model | Why it's out |
|---|---|---|---|
| cbchui 75°, ½ screen | 2.45 | 2.27 | The fixed-power systems are linear rules ("30 parts per screen") that approximate the curve; they admit "slightly less power up close" (`[CR-using]`). |
| 2.4 system 70°, ⅔ screen | 2.4 | 2.31 | The same approximation. |
| BJSL 80°, ½ screen (Armor) | 2.9 | 2.76 | Contradicted by the Aduka table at the same geometry (2.40, about 2.45 scaled): short 80° entries disagree with each other by 0.5 bar. |
| Aduka 80°, ½ screen | 2.40 | 2.76 | As above. |
| Aduka 60°, 1¼ and 1½ screens | 3.10, 3.60 | 2.76, 3.03 | Contradicted by Banpao, which puts 1.5 screens at 3.0 bars. The table's own ≤1-screen entries fit the model. |
| Plain throw 45°, ½ screen | 1.33 | 1.52 | A rough "≈" from the same FAQ whose one-screen value is in the set. |
| Boomer 70°, one screen, calm | 2.8 | 2.64 (Hookclaw) | `[UC-boomer]`'s calm power contradicts `[CR-fixed]`'s, which sets Hookclaw's gravity; its 0.45-bar wind saving is kept instead. |

The entries the model does match, unfitted, include the Aduka 60° ¼-screen (1.10 vs 1.11) and 70°
one-screen (2.85 vs 2.87) values, and the 80° one-screen super-high shot (3.80 vs 3.96; 3.88
scaled).

## Re-fitting

`cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit` fits Coilback's gravity
and the wind factors and prints them, with the model's numbers for every entry above; copy those
into `sim::tuning`. It rewrites the other mobiles' `GRAVITY` (and Hookclaw's wind sensitivity) in
place, so refit after a drawing moves a cannon, then re-record the golden replays (`tool_record`):
the new mobiles play in them. The acceptance tests then say whether everything still holds.
