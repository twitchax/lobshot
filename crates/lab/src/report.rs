//! A balance run's report: the win-rate matrix, damage per unit of delay, turns to kill, hit rates
//! and the first mover's edge, as plain text for people and JSON for tools. And the gate: `DESIGN.md`
//! ("Bots and the lab") holds every matchup to a 35-65% win rate once the roster is big enough.

use std::fmt::Write as _;

use serde::Serialize;

use lobshot_sim::{bot::Level, game::MobileKind};

use crate::balance::{Outcome, Side, level_name};

/// The gate's band, as win rates: no matchup may be won more than 65% of the time by either side.
pub const BAND: Band = Band { low: 0.35, high: 0.65 };

/// The shots, in `Side::shots` order.
const SHOTS: [&str; 3] = ["S1", "S2", "SS"];

/// A range of win rates, inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Band {
    pub low: f64,
    pub high: f64,
}

/// A share of games, with its 95% margin of error.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rate {
    /// Wins, a draw counting half.
    pub wins: f64,
    pub games: u32,
    pub rate: f64,
    /// Half the width of the 95% interval around `rate` (normal approximation).
    pub margin: f64,
}

impl Rate {
    #[must_use]
    pub fn new(wins: f64, games: u32) -> Self {
        let n = f64::from(games.max(1));
        let rate = wins / n;
        Self {
            wins,
            games,
            rate,
            margin: 1.96 * (rate * (1.0 - rate) / n).sqrt(),
        }
    }
}

/// A rate over one level's games, in a run that played more than one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AtLevel {
    pub level: Level,
    pub rate: Rate,
}

/// Two different mobiles' games against each other.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Matchup {
    pub mobiles: [MobileKind; 2],
    pub draws: u32,
    /// How often `mobiles[0]` won, over every level's games.
    pub first: Rate,
    /// The same, level by level.
    pub by_level: Vec<AtLevel>,
    pub mean_turns: f64,
}

/// One mobile over the whole run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MobileReport {
    pub mobile: MobileKind,
    /// Against the other mobiles (mirrors left out).
    pub wins: Rate,
    /// The same, level by level.
    pub wins_by_level: Vec<AtLevel>,
    /// HP taken off enemies per 1000 delay its turns cost.
    pub damage_per_1000_delay: f64,
    /// Its own turns to win a game it won.
    pub turns_to_kill: Option<f64>,
    /// Shots that hurt an enemy.
    pub hit_rate: f64,
    /// S1, S2, SS.
    pub shots: [ShotReport; 3],
}

/// One mobile's use of one shot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ShotReport {
    /// The share of its turns that fired this shot.
    pub share: f64,
    pub damage_per_1000_delay: Option<f64>,
}

/// One arena over the whole run: how long its games ran, and how often a shot hurt an enemy. A
/// map where SearchBot rarely finds a shot drags its games out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArenaReport {
    /// "map variant".
    pub arena: String,
    pub games: u32,
    pub mean_turns: f64,
    pub hit_rate: f64,
    pub draws: u32,
}

/// A whole balance run. A run may play its games at several levels; every number pools them, and
/// the win rates are given level by level too.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The levels its games were played at, easiest first.
    pub levels: Vec<Level>,
    pub games: usize,
    pub arenas: Vec<ArenaReport>,
    pub mobiles: Vec<MobileReport>,
    pub matchups: Vec<Matchup>,
    /// How often the seat that moved first won a decisive game.
    pub first_mover: Rate,
    pub draws: u32,
    pub mean_turns: f64,
}

impl Report {
    #[must_use]
    pub fn new(outcomes: &[Outcome], mobiles: &[MobileKind]) -> Self {
        let decisive: Vec<&Outcome> = outcomes.iter().filter(|o| o.winner.is_some()).collect();
        let first_wins = decisive.iter().filter(|o| o.winner == Some(o.first)).count();
        let levels: Vec<Level> = [Level::Easy, Level::Normal, Level::Hard]
            .into_iter()
            .filter(|&level| outcomes.iter().any(|o| o.spec.level == level))
            .collect();
        Self {
            games: outcomes.len(),
            arenas: arena_reports(outcomes),
            mobiles: mobiles.iter().map(|&mobile| mobile_report(outcomes, mobile, &levels)).collect(),
            matchups: matchups(outcomes, mobiles, &levels),
            levels,
            first_mover: Rate::new(count(first_wins), u32::try_from(decisive.len()).unwrap_or(u32::MAX)),
            draws: u32::try_from(outcomes.len() - decisive.len()).unwrap_or(u32::MAX),
            mean_turns: mean(outcomes.iter().map(|o| float(o.turns))).unwrap_or(0.0),
        }
    }

    /// The matchups won outside `band` by either side.
    #[must_use]
    pub fn outside(&self, band: Band) -> Vec<&Matchup> {
        let within = |rate: f64| (band.low..=band.high).contains(&rate);
        self.matchups.iter().filter(|m| !within(m.first.rate) || !within(1.0 - m.first.rate)).collect()
    }

    /// The report for people.
    #[must_use]
    pub fn to_text(&self, band: Band) -> String {
        let mut text = String::new();
        let mut line = |s: &str| {
            text.push_str(s);
            text.push('\n');
        };
        let names: Vec<&str> = self.mobiles.iter().map(|m| m.mobile.tuning().name).collect();
        line(&format!(
            "Balance: {}, 1v1, {} bots, on {} ({}).",
            plural(self.games, "game"),
            self.levels.iter().map(|&level| capitalized(level_name(level))).collect::<Vec<_>>().join(" and "),
            plural(self.arenas.len(), "arena"),
            self.arenas.iter().map(|a| a.arena.as_str()).collect::<Vec<_>>().join(", ")
        ));
        line("");
        line("Win rate of the row against the column (a draw counts half), with its 95% margin:");
        let mut header = format!("{:<12}", "");
        for name in &names {
            let _ = write!(header, " {name:>12}");
        }
        line(&header);
        for row in &self.mobiles {
            let mut cells = format!("{:<12}", row.mobile.tuning().name);
            for column in &self.mobiles {
                let cell = self
                    .against(row.mobile, column.mobile)
                    .map_or_else(|| "-".to_owned(), |rate| format!("{:.0}% ±{:.0}", rate.rate * 100.0, rate.margin * 100.0));
                let _ = write!(cells, " {cell:>12}");
            }
            line(&cells);
        }
        line("");
        if self.levels.len() > 1 {
            line(&format!(
                "Every number pools the levels' games; wins by level: {}.",
                self.mobiles
                    .iter()
                    .map(|m| format!("{} {}", m.mobile.tuning().name, by_level(&m.wins_by_level)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            line("");
        }
        line("By mobile (win rate leaves mirrors out; damage per 1000 delay counts every turn it cost):");
        line(&format!(
            "{:<12} {:>6} {:>10} {:>14} {:>9}   {}",
            "",
            "wins",
            "dmg/1000",
            "turns to kill",
            "hit rate",
            SHOTS.map(|shot| format!("{shot}: share, dmg/1000")).join("   ")
        ));
        for m in &self.mobiles {
            let shots: Vec<String> = m
                .shots
                .iter()
                .map(|s| format!("{:>3.0}%, {:>4}", s.share * 100.0, s.damage_per_1000_delay.map_or_else(|| "-".to_owned(), |d| format!("{d:.0}"))))
                .collect();
            line(&format!(
                "{:<12} {:>5.0}% {:>10.0} {:>14} {:>8.0}%   {}",
                m.mobile.tuning().name,
                m.wins.rate * 100.0,
                m.damage_per_1000_delay,
                m.turns_to_kill.map_or_else(|| "-".to_owned(), |t| format!("{t:.1}")),
                m.hit_rate * 100.0,
                shots.join("      ")
            ));
        }
        line("");
        line("By arena:");
        line(&format!("{:<14} {:>6} {:>7} {:>9} {:>6}", "", "games", "turns", "hit rate", "draws"));
        for a in &self.arenas {
            line(&format!("{:<14} {:>6} {:>7.1} {:>8.0}% {:>6}", a.arena, a.games, a.mean_turns, a.hit_rate * 100.0, a.draws));
        }
        line("");
        line(&format!(
            "The first to move won {:.0}% ±{:.0} of decisive games. Games ran {:.1} turns on average; {} drawn.",
            self.first_mover.rate * 100.0,
            self.first_mover.margin * 100.0,
            self.mean_turns,
            plural(usize::try_from(self.draws).unwrap_or(usize::MAX), "game"),
        ));
        let outside = self.outside(band);
        if outside.is_empty() {
            line(&format!("Every matchup is within {:.0}-{:.0}%.", band.low * 100.0, band.high * 100.0));
        } else {
            line(&format!("Outside {:.0}-{:.0}%:", band.low * 100.0, band.high * 100.0));
            for m in outside {
                let [a, b] = m.mobiles.map(|mobile| mobile.tuning().name);
                let levels = if m.by_level.len() > 1 { format!(" ({})", by_level(&m.by_level)) } else { String::new() };
                line(&format!(
                    "  {a} beats {b} {:.0}% ±{:.0} of {} games{levels}",
                    m.first.rate * 100.0,
                    m.first.margin * 100.0,
                    m.first.games
                ));
            }
        }
        text
    }

    /// How often `row` beat `column`; `None` for a mirror or a pair the run didn't play.
    #[must_use]
    pub fn against(&self, row: MobileKind, column: MobileKind) -> Option<Rate> {
        self.matchups.iter().find_map(|m| match m.mobiles {
            [a, b] if a == row && b == column => Some(m.first),
            [a, b] if a == column && b == row => Some(Rate::new(f64::from(m.first.games) - m.first.wins, m.first.games)),
            _ => None,
        })
    }
}

fn arena_reports(outcomes: &[Outcome]) -> Vec<ArenaReport> {
    let mut names: Vec<String> = outcomes.iter().map(arena_name).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|arena| {
            let games: Vec<&Outcome> = outcomes.iter().filter(|o| arena_name(o) == arena).collect();
            let sides = || games.iter().flat_map(|o| o.sides.iter());
            let shots: f64 = sides().map(|s| s.shots.iter().map(|u| f64::from(u.uses)).sum::<f64>()).sum();
            ArenaReport {
                games: u32::try_from(games.len()).unwrap_or(u32::MAX),
                mean_turns: mean(games.iter().map(|o| float(o.turns))).unwrap_or(0.0),
                hit_rate: ratio(sides().map(|s| f64::from(s.hits)).sum(), shots).unwrap_or(0.0),
                draws: u32::try_from(games.iter().filter(|o| o.winner.is_none()).count()).unwrap_or(u32::MAX),
                arena,
            }
        })
        .collect()
}

fn arena_name(outcome: &Outcome) -> String {
    format!("{} {}", outcome.spec.map, outcome.spec.variant)
}

/// Rates level by level, as `Normal 61%, Hard 48%`.
fn by_level(rates: &[AtLevel]) -> String {
    rates
        .iter()
        .map(|at| format!("{} {:.0}%", capitalized(level_name(at.level)), at.rate.rate * 100.0))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How often `mobile` won `games`, pooled and level by level.
fn wins(games: &[&Outcome], mobile: MobileKind, levels: &[Level]) -> (Rate, Vec<AtLevel>) {
    let rate = |games: &mut dyn Iterator<Item = &&Outcome>| {
        let (wins, n) = games.fold((0.0, 0_u32), |(wins, n), o| (wins + score(o, mobile), n + 1));
        Rate::new(wins, n)
    };
    let by_level = levels
        .iter()
        .map(|&level| AtLevel {
            level,
            rate: rate(&mut games.iter().filter(|o| o.spec.level == level)),
        })
        .collect();
    (rate(&mut games.iter()), by_level)
}

fn matchups(outcomes: &[Outcome], mobiles: &[MobileKind], levels: &[Level]) -> Vec<Matchup> {
    let mut all = Vec::new();
    for (i, &a) in mobiles.iter().enumerate() {
        for &b in &mobiles[i + 1..] {
            let games: Vec<&Outcome> = outcomes.iter().filter(|o| o.spec.mobiles == [a, b] || o.spec.mobiles == [b, a]).collect();
            if games.is_empty() {
                continue;
            }
            let (first, by_level) = wins(&games, a, levels);
            all.push(Matchup {
                mobiles: [a, b],
                draws: u32::try_from(games.iter().filter(|o| o.winner.is_none()).count()).unwrap_or(u32::MAX),
                first,
                by_level,
                mean_turns: mean(games.iter().map(|o| float(o.turns))).unwrap_or(0.0),
            });
        }
    }
    all
}

fn mobile_report(outcomes: &[Outcome], mobile: MobileKind, levels: &[Level]) -> MobileReport {
    let sides: Vec<(&Outcome, usize)> = outcomes.iter().flat_map(|o| (0..2).filter(move |&i| o.spec.mobiles[i] == mobile).map(move |i| (o, i))).collect();
    let versus: Vec<&Outcome> = outcomes.iter().filter(|o| o.spec.mobiles.contains(&mobile) && o.spec.mobiles[0] != o.spec.mobiles[1]).collect();
    let total = |f: fn(&Side) -> f64| -> f64 { sides.iter().map(|&(o, i)| f(&o.sides[i])).sum() };
    let (turns, damage, delay, hits) = (total(|s| f64::from(s.turns)), total(|s| float(s.damage)), total(|s| unsigned(s.delay)), total(|s| f64::from(s.hits)));
    let shots_fired: f64 = sides.iter().map(|&(o, i)| o.sides[i].shots.iter().map(|u| f64::from(u.uses)).sum::<f64>()).sum();
    let shots = [0, 1, 2].map(|shot| {
        let (uses, damage, delay) = sides.iter().fold((0.0, 0.0, 0.0), |(u, d, l), &(o, i)| {
            let usage = o.sides[i].shots[shot];
            (u + f64::from(usage.uses), d + float(usage.damage), l + unsigned(usage.delay))
        });
        ShotReport {
            share: ratio(uses, turns).unwrap_or(0.0),
            damage_per_1000_delay: ratio(damage * 1000.0, delay).filter(|_| uses > 0.0),
        }
    });
    let (wins, wins_by_level) = wins(&versus, mobile, levels);
    MobileReport {
        mobile,
        wins,
        wins_by_level,
        damage_per_1000_delay: ratio(damage * 1000.0, delay).unwrap_or(0.0),
        turns_to_kill: mean(sides.iter().filter(|&&(o, i)| o.winner == Some(seat(i))).map(|&(o, i)| f64::from(o.sides[i].turns))),
        hit_rate: ratio(hits, shots_fired).unwrap_or(0.0),
        shots,
    }
}

/// What a game was worth to `mobile`'s side: 1 a win, ½ a draw. In a mirror, to seat 0.
fn score(outcome: &Outcome, mobile: MobileKind) -> f64 {
    let mine = seat(usize::from(outcome.spec.mobiles[0] != mobile));
    match outcome.winner {
        None => 0.5,
        Some(winner) if winner == mine => 1.0,
        Some(_) => 0.0,
    }
}

fn seat(i: usize) -> u8 {
    u8::try_from(i).unwrap_or(u8::MAX)
}

/// A game's or a side's count as a float: none comes near `i32::MAX`.
fn float(n: i64) -> f64 {
    f64::from(i32::try_from(n).unwrap_or(i32::MAX))
}

fn unsigned(n: u64) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    (denominator > 0.0).then(|| numerator / denominator)
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0.0), |(sum, n), v| (sum + v, n + 1.0));
    ratio(sum, n)
}

fn plural(n: usize, what: &str) -> String {
    if n == 1 { format!("1 {what}") } else { format!("{n} {what}s") }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::balance::{Spec, Usage};

    const A: MobileKind = MobileKind::Coilback;
    const B: MobileKind = MobileKind::Hookclaw;

    fn side(turns: u32, damage: i64, delay: u64) -> Side {
        let mut shots = [Usage::default(); 3];
        shots[0] = Usage { uses: turns, damage, delay };
        Side {
            turns,
            damage,
            delay,
            hits: u32::from(damage > 0),
            shots,
        }
    }

    /// A game between `mobiles` that `winner` (a seat) won, `first` moving first.
    fn game(mobiles: [MobileKind; 2], winner: Option<u8>, first: u8) -> Outcome {
        Outcome {
            spec: Spec {
                mobiles,
                map: "flat".into(),
                variant: "A".into(),
                level: Level::Normal,
                seed: 1,
            },
            winner,
            turns: 10,
            first,
            sides: [side(5, 1000, 4000), side(5, 500, 5000)],
        }
    }

    #[test]
    fn the_matrix_counts_both_seatings_and_draws_half() {
        let outcomes = [game([A, B], Some(0), 0), game([B, A], Some(1), 0), game([A, B], None, 1), game([B, A], Some(0), 1)];
        let report = Report::new(&outcomes, &[A, B]);
        // A won games 1 and 2, drew game 3 and lost game 4: 2.5 of 4.
        assert_eq!(report.against(A, B).unwrap().rate, 0.625);
        assert_eq!(report.against(B, A).unwrap().rate, 0.375);
        assert_eq!(report.against(A, A), None);
        assert_eq!(report.draws, 1);
        assert_eq!(report.matchups[0].draws, 1);
        // Decisive games 1, 2 and 4: the first mover (seats 0, 0 and 1) won only game 1.
        assert_eq!((report.first_mover.wins, report.first_mover.games), (1.0, 3));
    }

    #[test]
    fn per_mobile_numbers_follow_their_seat() {
        let outcomes = [game([A, B], Some(0), 0), game([B, A], Some(0), 0)];
        let report = Report::new(&outcomes, &[A, B]);
        let a = &report.mobiles[0];
        // A sat in seat 0 (1000 damage over 4000 delay) and seat 1 (500 over 5000).
        assert_eq!(a.damage_per_1000_delay, 1500.0 * 1000.0 / 9000.0);
        assert_eq!(a.turns_to_kill, Some(5.0), "it won the first game in its five turns");
        assert_eq!((a.wins.wins, a.wins.games), (1.0, 2));
        assert_eq!(a.shots[0].share, 1.0);
        assert_eq!(a.shots[1].damage_per_1000_delay, None, "never fired");
        assert_eq!(a.hit_rate, 2.0 / 10.0);
    }

    #[test]
    fn mirrors_feed_their_mobile_but_not_its_win_rate_or_the_gate() {
        let outcomes = [game([A, A], Some(0), 0), game([A, B], Some(1), 0)];
        let report = Report::new(&outcomes, &[A, B]);
        assert_eq!(report.mobiles[0].wins.games, 1);
        assert_eq!(report.matchups.len(), 1);
        assert_eq!(report.mobiles[0].turns_to_kill, Some(5.0), "the mirror's win counts");
    }

    #[test]
    fn the_gate_flags_lopsided_matchups_either_way() {
        let lopsided: Vec<Outcome> = (0..10).map(|i| game(if i % 2 == 0 { [A, B] } else { [B, A] }, Some(u8::from(i % 2 == 1)), 0)).collect();
        let report = Report::new(&lopsided, &[A, B]);
        assert_eq!(report.against(A, B).unwrap().rate, 1.0);
        assert_eq!(report.outside(BAND).len(), 1);
        let text = report.to_text(BAND);
        assert!(text.contains("Outside 35-65%:\n  Coilback beats Hookclaw 100%"), "{text}");

        // Either side's rate counts: in a band off center, the loser's rate can fall outside it.
        let lopsided_low = Band { low: 0.0, high: 0.01 };
        assert_eq!(report.outside(lopsided_low).len(), 1, "Hookclaw's 0% is in it, Coilback's 100% isn't");

        let even = [game([A, B], Some(0), 0), game([B, A], Some(0), 0)];
        let report = Report::new(&even, &[A, B]);
        assert_eq!(report.outside(BAND), Vec::<&Matchup>::new());
        assert!(report.to_text(BAND).contains("Every matchup is within 35-65%."));
    }

    #[test]
    fn the_text_has_a_row_and_a_column_per_mobile() {
        let report = Report::new(&[game([A, B], Some(0), 0)], &[A, B]);
        let text = report.to_text(BAND);
        assert!(text.starts_with("Balance: 1 game, 1v1, Normal bots, on 1 arena (flat A)."), "{text}");
        let header = text.lines().find(|l| l.trim_start().starts_with("Coilback")).unwrap();
        assert!(header.contains("Hookclaw"), "{text}");
        assert!(text.lines().any(|l| l.starts_with("Hookclaw") && l.contains("0% ±0")), "{text}");
    }

    #[test]
    fn arenas_report_their_length_hits_and_draws() {
        let mut elsewhere = game([A, B], None, 0);
        elsewhere.spec.map = "tidewall".into();
        elsewhere.turns = 30;
        let report = Report::new(&[game([A, B], Some(0), 0), game([B, A], Some(0), 0), elsewhere], &[A, B]);
        let names: Vec<&str> = report.arenas.iter().map(|a| a.arena.as_str()).collect();
        assert_eq!(names, ["flat A", "tidewall A"]);
        let (flat, tidewall) = (&report.arenas[0], &report.arenas[1]);
        assert_eq!((flat.games, flat.mean_turns, flat.draws), (2, 10.0, 0));
        assert_eq!((tidewall.games, tidewall.mean_turns, tidewall.draws), (1, 30.0, 1));
        assert_eq!(flat.hit_rate, 4.0 / 20.0, "one hit a side a game, in five shots each");
        assert!(report.to_text(BAND).contains("\ntidewall A          1    30.0       20%      1\n"), "{}", report.to_text(BAND));
    }

    #[test]
    fn a_run_at_two_levels_pools_them_and_gives_each_level_its_rate() {
        let hard = |mut outcome: Outcome| {
            outcome.spec.level = Level::Hard;
            outcome
        };
        // Normal: A wins both. Hard: B wins both.
        let outcomes = [game([A, B], Some(0), 0), game([B, A], Some(1), 0), hard(game([A, B], Some(1), 0)), hard(game([B, A], Some(0), 0))];
        let report = Report::new(&outcomes, &[A, B]);
        assert_eq!(report.levels, [Level::Normal, Level::Hard]);
        let matchup = &report.matchups[0];
        assert_eq!((matchup.first.rate, matchup.first.games), (0.5, 4), "pooled");
        let by_level: Vec<(Level, f64)> = matchup.by_level.iter().map(|at| (at.level, at.rate.rate)).collect();
        assert_eq!(by_level, [(Level::Normal, 1.0), (Level::Hard, 0.0)]);
        assert_eq!(report.mobiles[1].wins_by_level[1].rate.rate, 1.0, "B wins at Hard");

        let text = report.to_text(Band { low: 0.6, high: 1.0 });
        assert!(text.starts_with("Balance: 4 games, 1v1, Normal and Hard bots,"), "{text}");
        assert!(text.contains("wins by level: Coilback Normal 100%, Hard 0%, Hookclaw Normal 0%, Hard 100%."), "{text}");
        assert!(text.contains("Coilback beats Hookclaw 50% ±49 of 4 games (Normal 100%, Hard 0%)"), "{text}");
    }

    #[test]
    fn margins_shrink_with_more_games() {
        assert!(Rate::new(50.0, 100).margin > Rate::new(500.0, 1000).margin);
        assert_eq!(Rate::new(0.0, 0).rate, 0.0, "no games, no division by zero");
    }
}
