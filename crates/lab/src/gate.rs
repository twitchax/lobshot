//! The balance gate (`DESIGN.md`, "Bots and the lab"): every matchup within the band, or on the
//! list of matchups known to sit outside it (`KNOWN`), no worse than when it was measured.
//!
//! A known matchup is one the lab measured outside the band at the gate's settings (`cargo make
//! balance-gate`) that nothing within the mobiles' sources brought in (PRD-0006 T-010). The gate
//! names each one every run, holds it to its measured rate within that rate's 95% margin, and
//! notes one that has come back inside the band, to drop from the list. It never skips a matchup
//! silently: one outside the band and not on the list fails it.

use lobshot_sim::game::MobileKind::{
    self, Augerdillo, Coilback, Flarecrow, Frostfin, Herdhound, Hookclaw, Kitewyrm, Lanternwing, Lensowl, Lodestoad, Marblemole, Needlemouse, Sparkhorn, Spoutpot, Sudsotter, Thistleback, Tinbadger,
    Tuskrack,
};

use crate::report::{Band, Matchup, Rate, Report};

/// A matchup known to sit outside the band: how often `winner` beat `loser` when measured, in
/// percent, and why it stays out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Known {
    pub winner: MobileKind,
    pub loser: MobileKind,
    pub percent: f64,
    pub why: &'static str,
}

const SPARKHORN: &str = "Sparkhorn fires at 18 to 40 degrees (CR-mob): its balls strike the side of a hitbox, far from its center, \
                         so its direct hits deal near their minimum, and it wins 17% at Hard against the field.";
const FLOATS: &str = "Kitewyrm floats, and J.Frog's walking shots can't hit a Dragon (SW-mob, WP06): only a sponge's blast timed \
                      under it reaches across the 16 px gap (OB).";
const FLARECROW: &str = "Flarecrow's S2 is three of Skyeye's beams, from level 1 (SW-mob's 140-500, CR-aduka): less a hit than the \
                         roster's middle, and Dual adds less to beams than to shells. It wins 34% at Hard against the field.";
const COILBACK: &str = "Coilback is Armor, GunBound's reference shot, every number cited (CR-mob, DB-mob): its winner's shots \
                        forgive a misread wind as its shells don't, and it wins 38% at Normal against the field.";
const NEAR: &str = "Within 6 points of the band after T-010's tuning: no further pick or contradiction in either mobile was found \
                    to bring it in.";
const HARD: &str = "Lopsided at Hard, where exact aim leaves the race to damage per delay (Lanternwing over Sudsotter 83%, \
                    Marblemole over Tinbadger 87%), near the band at Normal: T-010 found nothing within either mobile's sources \
                    to bring it in.";

/// The matchups known to sit outside 35-65% at the gate's settings, as `cargo make balance-gate`
/// measured them at PRD-0006 T-010 (Normal and Hard pooled, 200 games a matchup).
pub const KNOWN: &[Known] = &[
    known(Lanternwing, Sparkhorn, 84.0, SPARKHORN),
    known(Marblemole, Sparkhorn, 84.0, SPARKHORN),
    known(Kitewyrm, Sparkhorn, 81.0, SPARKHORN),
    known(Lensowl, Sparkhorn, 78.0, SPARKHORN),
    known(Lodestoad, Sparkhorn, 78.0, SPARKHORN),
    known(Frostfin, Sparkhorn, 77.5, SPARKHORN),
    known(Augerdillo, Sparkhorn, 76.8, SPARKHORN),
    known(Needlemouse, Sparkhorn, 75.0, SPARKHORN),
    known(Herdhound, Sparkhorn, 71.8, SPARKHORN),
    known(Tuskrack, Sparkhorn, 71.5, SPARKHORN),
    known(Hookclaw, Sparkhorn, 69.8, SPARKHORN),
    known(Thistleback, Sparkhorn, 69.0, SPARKHORN),
    known(Spoutpot, Sparkhorn, 67.5, SPARKHORN),
    known(Coilback, Sparkhorn, 67.0, SPARKHORN),
    known(Kitewyrm, Sudsotter, 93.5, FLOATS),
    known(Marblemole, Flarecrow, 75.5, FLARECROW),
    known(Herdhound, Flarecrow, 74.0, FLARECROW),
    known(Lanternwing, Flarecrow, 70.0, FLARECROW),
    known(Kitewyrm, Flarecrow, 67.0, FLARECROW),
    known(Lodestoad, Flarecrow, 66.5, FLARECROW),
    known(Tuskrack, Flarecrow, 66.0, FLARECROW),
    known(Needlemouse, Flarecrow, 65.5, FLARECROW),
    known(Tinbadger, Flarecrow, 65.5, FLARECROW),
    known(Marblemole, Coilback, 78.5, COILBACK),
    known(Lodestoad, Coilback, 73.5, COILBACK),
    known(Herdhound, Coilback, 69.0, COILBACK),
    known(Kitewyrm, Coilback, 65.5, COILBACK),
    known(Augerdillo, Coilback, 65.5, COILBACK),
    known(Lanternwing, Sudsotter, 75.5, HARD),
    known(Marblemole, Tinbadger, 75.0, HARD),
    known(Kitewyrm, Tinbadger, 70.5, NEAR),
    known(Tuskrack, Lensowl, 68.5, NEAR),
    known(Kitewyrm, Frostfin, 68.0, NEAR),
    known(Tuskrack, Frostfin, 67.5, NEAR),
    known(Herdhound, Sudsotter, 67.5, NEAR),
    known(Augerdillo, Lensowl, 66.5, NEAR),
    known(Marblemole, Lodestoad, 66.5, NEAR),
    known(Augerdillo, Lanternwing, 66.0, NEAR),
];

const fn known(winner: MobileKind, loser: MobileKind, percent: f64, why: &'static str) -> Known {
    Known { winner, loser, percent, why }
}

/// What the gate makes of a report.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict<'r> {
    /// Outside the band and not known, or known and worse than measured: each fails the gate.
    pub failures: Vec<(&'r Matchup, Option<&'static Known>)>,
    /// Known, and no worse than measured.
    pub known: Vec<(&'r Matchup, &'static Known)>,
    /// Known, but now within the band: to drop from `KNOWN`.
    pub back: Vec<(&'r Matchup, &'static Known)>,
}

impl Verdict<'_> {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// The verdict for people, a line a matchup.
    #[must_use]
    pub fn to_text(&self, band: Band) -> String {
        let mut text = String::new();
        let mut line = |s: String| {
            text.push_str(&s);
            text.push('\n');
        };
        let (low, high) = (band.low * 100.0, band.high * 100.0);
        line(format!(
            "The balance gate ({low:.0}-{high:.0}%): {} known outside and no worse, {} back inside, {} failing.",
            self.known.len(),
            self.back.len(),
            self.failures.len()
        ));
        for (matchup, known) in &self.failures {
            let (winner, loser, rate) = winning(matchup);
            line(match known {
                Some(known) => format!(
                    "  FAIL {} beats {} {:.0}% ±{:.0}: known at {:.0}%, now worse",
                    winner.tuning().name,
                    loser.tuning().name,
                    rate.rate * 100.0,
                    rate.margin * 100.0,
                    known.percent
                ),
                None => format!(
                    "  FAIL {} beats {} {:.0}% ±{:.0}: outside the band",
                    winner.tuning().name,
                    loser.tuning().name,
                    rate.rate * 100.0,
                    rate.margin * 100.0
                ),
            });
        }
        for (matchup, known) in &self.back {
            let (winner, loser, rate) = winning(matchup);
            line(format!(
                "  back {} beats {} {:.0}%: known at {:.0}%, now inside; drop it from `gate::KNOWN`",
                winner.tuning().name,
                loser.tuning().name,
                rate.rate * 100.0,
                known.percent
            ));
        }
        for (matchup, known) in &self.known {
            let (winner, loser, rate) = winning(matchup);
            line(format!(
                "  known {} beats {} {:.0}% (measured {:.0}%): {}",
                winner.tuning().name,
                loser.tuning().name,
                rate.rate * 100.0,
                known.percent,
                known.why
            ));
        }
        text
    }
}

/// The matchup's winner, loser, and the winner's rate.
fn winning(matchup: &Matchup) -> (MobileKind, MobileKind, Rate) {
    let [a, b] = matchup.mobiles;
    let rate = matchup.first;
    if rate.rate >= 0.5 {
        (a, b, rate)
    } else {
        (b, a, Rate::new(f64::from(rate.games) - rate.wins, rate.games))
    }
}

/// Judges every matchup `report` played against `band` and `known`.
#[must_use]
pub fn verdict<'r>(report: &'r Report, band: Band, known: &'static [Known]) -> Verdict<'r> {
    let mut verdict = Verdict {
        failures: Vec::new(),
        known: Vec::new(),
        back: Vec::new(),
    };
    let within = |rate: f64| (band.low..=band.high).contains(&rate);
    for matchup in &report.matchups {
        let [a, b] = matchup.mobiles;
        let listed = known.iter().find(|k| (k.winner, k.loser) == (a, b) || (k.winner, k.loser) == (b, a));
        let inside = within(matchup.first.rate) && within(1.0 - matchup.first.rate);
        match (inside, listed) {
            (true, None) => {}
            (true, Some(listed)) => verdict.back.push((matchup, listed)),
            (false, None) => verdict.failures.push((matchup, None)),
            (false, Some(listed)) => {
                let (winner, _, rate) = winning(matchup);
                if winner == listed.winner && rate.rate <= listed.percent / 100.0 + rate.margin {
                    verdict.known.push((matchup, listed));
                } else {
                    verdict.failures.push((matchup, Some(listed)));
                }
            }
        }
    }
    verdict
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        balance::{Outcome, Side, Spec},
        report::BAND,
    };
    use lobshot_sim::bot::Level;

    /// `games` between `a` and `b`, `a` winning `wins` of them.
    fn run(a: MobileKind, b: MobileKind, wins: u32, games: u32) -> Vec<Outcome> {
        (0..games)
            .map(|i| Outcome {
                spec: Spec {
                    mobiles: [a, b],
                    map: "flat".into(),
                    variant: "A".into(),
                    level: Level::Normal,
                    seed: u64::from(i),
                },
                winner: Some(u8::from(i >= wins)),
                turns: 10,
                first: 0,
                sides: [Side::default(); 2],
            })
            .collect()
    }

    const LISTED: &[Known] = &[known(Coilback, Hookclaw, 75.0, "for the test")];

    #[test]
    fn a_matchup_outside_the_band_and_not_known_fails_the_gate() {
        let outcomes = run(Coilback, Sparkhorn, 160, 200);
        let report = Report::new(&outcomes, &[Coilback, Sparkhorn]);
        let verdict = verdict(&report, BAND, LISTED);
        assert!(!verdict.passed());
        assert!(verdict.to_text(BAND).contains("FAIL Coilback beats Sparkhorn 80% ±6: outside the band"), "{}", verdict.to_text(BAND));
    }

    #[test]
    fn a_known_matchup_passes_at_its_measured_rate_or_within_its_margin_and_fails_worse() {
        for (wins, passes) in [(150, true), (160, true), (170, false)] {
            // Either seating names the matchup.
            let outcomes = run(Hookclaw, Coilback, 200 - wins, 200);
            let report = Report::new(&outcomes, &[Hookclaw, Coilback]);
            let verdict = verdict(&report, BAND, LISTED);
            assert_eq!(verdict.passed(), passes, "Coilback winning {wins} of 200: {}", verdict.to_text(BAND));
        }
        let outcomes = run(Hookclaw, Coilback, 150, 200);
        let report = Report::new(&outcomes, &[Hookclaw, Coilback]);
        assert!(!verdict(&report, BAND, LISTED).passed(), "the other side winning as often is no known matchup");
    }

    #[test]
    fn a_known_matchup_back_inside_the_band_is_noted_and_passes() {
        let outcomes = run(Coilback, Hookclaw, 110, 200);
        let report = Report::new(&outcomes, &[Coilback, Hookclaw]);
        let verdict = verdict(&report, BAND, LISTED);
        assert!(verdict.passed());
        assert_eq!(verdict.back.len(), 1);
        assert!(
            verdict.to_text(BAND).contains("back Coilback beats Hookclaw 55%: known at 75%, now inside"),
            "{}",
            verdict.to_text(BAND)
        );
    }

    #[test]
    fn the_known_list_names_each_matchup_once_and_only_outside_the_band() {
        for (i, a) in KNOWN.iter().enumerate() {
            assert!(a.percent > BAND.high * 100.0 && a.percent <= 100.0, "{a:?}");
            assert_ne!(a.winner, a.loser);
            for b in &KNOWN[i + 1..] {
                assert!((a.winner, a.loser) != (b.winner, b.loser) && (a.winner, a.loser) != (b.loser, b.winner), "{a:?} twice");
            }
        }
    }
}
