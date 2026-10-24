//! GunBound's community aiming formulas as acceptance tests (PRD-0002 uat-002): each holds in
//! lobshot within 0.05 bar, or one degree for angle rules. `docs/research/physics-fit.md` says which
//! formulas are in, which are out and why, and how the fitted constants were found (the ignored
//! `tool_fit` below).
//!
//! The setup is a duel on flat ground: the shooter (Coilback, Armor's behavior and GunBound's
//! reference shot, unless a test says otherwise) at x = 300 shooting right, and a Coilback target
//! `distance` px away. A mobile that fires over its back faces away, and every angle here is above
//! level toward the target, as players read it (`AimRange::elevation`). "Ideal power" is the
//! continuous power whose flight passes through the target's hitbox center. Aim limits don't apply
//! here: the formulas are about the ballistics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lobshot_sim::{
    ballistics::{self, Flight},
    game::{Game, MobileKind, Seat, Setup, Shot, Team},
    items::Loadout,
    map::{MapRef, NoAssets},
    mode::Mode,
    tuning::{WIND_X_PER_UNIT, WIND_Y_PER_UNIT, spoutpot},
    units::{Facing, SCREEN, TICK, Vec2, Wind, ticks, to_radians},
};

const TOLERANCE_BARS: f64 = 0.05;

/// CR-fixed's cbchui chart: the power that lands 60 degrees one screen away in zero wind, for each
/// mobile's GunBound counterpart (Armor, Mage, Boomer, Lightning, Grub, Turtle, Ice, Trico, Raon).
/// J.Frog came after the chart: Sudsotter's is Creedo's own cbchui-style figure for it (CR-frog). Dragon
/// isn't on it: Kitewyrm's is the chart's power for JD and Lightning, the shots GF-lee weighs Light,
/// as it weighs Dragon's. A.Sate's (Lensowl's) is on the chart, and CR-sate's fixed-power method
/// agrees.
const CBCHUI: [(MobileKind, f64); 12] = [
    (MobileKind::Coilback, 2.45),
    (MobileKind::Lanternwing, 2.38),
    (MobileKind::Hookclaw, 2.25),
    (MobileKind::Sparkhorn, 2.30),
    (MobileKind::Marblemole, 2.33),
    (MobileKind::Spoutpot, 2.39),
    (MobileKind::Sudsotter, 2.10),
    (MobileKind::Frostfin, 2.33),
    (MobileKind::Thistleback, 2.65),
    (MobileKind::Kitewyrm, 2.30),
    (MobileKind::Lensowl, 2.50),
    (MobileKind::Tinbadger, 2.50),
];

/// Mobiles the chart leaves out whose shot weight the sources tie to a charted one's, with that
/// one: Knight's (Needlemouse's) weighs what Lightning's (Sparkhorn's) does (WP06-list, CR-1sd,
/// CR-3bar). Each is fitted to its twin's chart power (`twin_power`).
const WEIGHS_AS: [(MobileKind, MobileKind); 1] = [(MobileKind::Needlemouse, MobileKind::Sparkhorn)];

/// The chart power of `twin`, a charted mobile.
fn twin_power(twin: MobileKind) -> f64 {
    CBCHUI.iter().find(|&&(mobile, _)| mobile == twin).map(|&(_, bars)| bars).expect("the twin is on the chart")
}

/// GF-ice's 70-degree table for Ice, in eighths of a screen and bars: the entries in the acceptance
/// set (docs/research/frostfin.md says which are out, and why).
const FROSTFIN_SEVENTY: [(i32, f64); 3] = [(6, 2.30), (8, 2.70), (16, 3.90)];

/// Kalsiddon's power for 60 degrees one screen away in zero wind, in cbchui-style shooting: 2.65
/// (CR-kal) and 2.7 (CR-kal30). Herdhound is fitted midway, so both hold.
const KAL_ONE_SCREEN: [f64; 2] = [2.65, 2.7];

/// CR-nak, the community's Nak formula, in zero wind: the bars that land `screens` away at `aim`.
fn cr_nak(screens: f64, aim: i16) -> f64 {
    cr_nak_in(screens, aim, 0.0)
}

/// CR-nak with `down` units of wind pushing down (negative: up), which adds to its gravity:
/// `Power = Sqrt(Distance / ((47.87^2) * SIN(2*A) * ((1 / (111.615 + W*SINX)) + (1/610)))) * 4`,
/// with distance in its UHA parts, nine to a screen. It leaves side wind out.
fn cr_nak_in(screens: f64, aim: i16, down: f64) -> f64 {
    let parts = 9.0 * screens;
    let sin = libm::sin(2.0 * to_radians(f64::from(aim)));
    libm::sqrt(parts / (47.87 * 47.87 * sin * (1.0 / (111.615 + down) + 1.0 / 610.0))) * 4.0
}

/// How many bars `up` units of up-wind (negative: down) save at 60 degrees over one screen, by
/// CR-nak.
fn nak_saving(up: f64) -> f64 {
    cr_nak(1.0, 60) - cr_nak_in(1.0, 60, -up)
}

/// The vertical winds CR-nak's wind term is checked at, up (positive) and down, to the room's
/// highest; 14 up is the one Augerdillo's wind sensitivity is fitted to.
const NAK_WINDS: [i8; 6] = [7, 14, 26, -7, -14, -26];

/// Nak's power for 60 degrees one screen away in zero wind: cbchui's chart (CR-fixed, 2.70) and
/// the Nak formula (CR-nak, about 2.62). Augerdillo is fitted midway, so both hold.
fn nak_one_screen() -> [f64; 2] {
    [2.70, cr_nak(1.0, 60)]
}

/// The Nak formula's entries in the acceptance set: the angles Augerdillo fires at from three
/// quarters of a screen to one and a quarter (docs/research/augerdillo.md says what is out, and
/// why).
const NAK_FORMULA: [(i16, f64); 6] = [(45, 0.75), (45, 1.0), (45, 1.25), (50, 0.75), (50, 1.0), (50, 1.25)];

/// CR-fixed gives Bigfoot's power as "??". DBR's per-mobile shot weights (`ax`, clone, unverified)
/// track the chart's powers as their square roots (docs/research/mechanics.md, 2.3): every mobile
/// both give, as (`ax`, chart power): Boomer, JD, Lightning, Ice, Grub, Aduka, Mage, Turtle, Armor,
/// A.Sate, Raon, Trico, Nak.
const AX_AND_CBCHUI: [(f64, f64); 13] = [
    (62.5, 2.25),
    (62.5, 2.30),
    (65.0, 2.30),
    (62.5, 2.33),
    (61.0, 2.33),
    (65.5, 2.35),
    (71.5, 2.38),
    (73.5, 2.39),
    (73.5, 2.45),
    (76.0, 2.50),
    (73.5, 2.50),
    (84.0, 2.65),
    (82.0, 2.70),
];

/// The mobiles whose counterpart the chart leaves out, with its DBR shot weight: Bigfoot's 88. Each
/// is fitted to the power that weight implies (`inferred_cbchui`).
const INFERRED: [(MobileKind, f64); 1] = [(MobileKind::Tuskrack, 88.0)];

/// The cbchui power a DBR shot weight of `ax` implies: the least-squares fit of the chart's powers
/// to the square roots of the weights, over `AX_AND_CBCHUI`.
fn inferred_cbchui(ax: f64) -> f64 {
    let (products, squares) = AX_AND_CBCHUI
        .iter()
        .fold((0.0, 0.0), |(products, squares), &(weight, bars)| (products + bars * weight.sqrt(), squares + weight));
    products / squares * ax.sqrt()
}

/// Aduka's power for 60 degrees one screen away in zero wind: 2.35 on cbchui's chart (CR-fixed) and
/// by CR-aduka's own rule, 2.40 in GN-aduka's table ("like 2.4", CR-aduka adds). Flarecrow is
/// fitted midway, so both hold.
const ADUKA_ONE_SCREEN: [f64; 2] = [2.35, 2.40];

/// Aduka's power tables, in zero wind, as (source, aim, quarter screens, bars): the entries in the
/// acceptance set. GN-aduka's and CR-aduka's fixed-angle tables (CR-aduka credits its table to the
/// KNAT guild), and CR-aduka's "3 bar formula", whose 1-screen landmark is 70 degrees at 2.75.
/// docs/research/flarecrow.md says which entries are out, and why.
const ADUKA_POWERS: [(&str, i16, i32, f64); 6] = [
    ("GN-aduka", 60, 2, 1.60),
    ("GN-aduka", 80, 3, 3.30),
    ("GN-aduka", 80, 4, 3.80),
    ("CR-aduka", 60, 2, 1.6),
    ("CR-aduka", 80, 3, 3.3),
    ("CR-aduka 3 bar", 70, 4, 2.75),
];

/// The entries left out of the acceptance set, for `tool_fit`'s report.
const ADUKA_POWERS_OUT: [(&str, i16, i32, f64); 25] = [
    ("GN-aduka", 60, 1, 1.10),
    ("GN-aduka", 60, 3, 2.10),
    ("GN-aduka", 60, 5, 3.10),
    ("GN-aduka", 60, 6, 3.60),
    ("GN-aduka", 70, 1, 1.35),
    ("GN-aduka", 70, 2, 2.00),
    ("GN-aduka", 70, 3, 2.45),
    ("GN-aduka", 70, 4, 2.85),
    ("GN-aduka", 70, 5, 3.25),
    ("GN-aduka", 70, 6, 3.65),
    ("GN-aduka", 80, 1, 1.70),
    ("GN-aduka", 80, 2, 2.40),
    ("GN-aduka", 85, 2, 3.80),
    ("CR-aduka", 60, 1, 1.1),
    ("CR-aduka", 60, 3, 2.1),
    ("CR-aduka", 70, 1, 1.35),
    ("CR-aduka", 70, 2, 2.0),
    ("CR-aduka", 70, 3, 2.45),
    ("CR-aduka", 70, 4, 2.85),
    ("CR-aduka", 80, 1, 1.95),
    ("CR-aduka", 80, 2, 2.7),
    ("CR-aduka", 80, 4, 3.9),
    ("CR-aduka 3 bar", 80, 2, 2.55),
    ("CR-aduka 3 bar", 60, 6, 2.85),
    ("CR-aduka high angle", 81, 4, 3.8),
];

/// CR-aduka's full-power high angles: (px to the target, the angle that lands there).
const ADUKA_FULL_POWER: [(i32, i16); 3] = [(267, 87), (533, 84), (800, 81)];

/// UC-boomer: 10 wind toward the target saves Boomer about 0.45 bars on a 70-degree, one-screen
/// shot.
const BOOMER_HOOK: (i16, u8, f64) = (70, 10, 0.45);

fn duel(distance: i32) -> Game {
    duel_of(MobileKind::Coilback, distance)
}

fn duel_of(shooter: MobileKind, distance: i32) -> Game {
    Game::new(
        &Setup {
            map: MapRef::Flat { width: 3200, height: 800, ground: 700 },
            seats: vec![
                Seat {
                    team: Team::A,
                    mobile: shooter,
                    spawn_x: 300,
                    // A mobile that fires over its back turns it to the target.
                    facing: if shooter.tuning().s1.aim.value.backward() { Facing::Left } else { Facing::Right },
                    items: Loadout::default(),
                },
                Seat {
                    team: Team::B,
                    mobile: MobileKind::Coilback,
                    spawn_x: 300 + distance,
                    facing: Facing::Left,
                    items: Loadout::default(),
                },
            ],
            order: vec![0, 1],
            wind: Wind::default(),
            sudden_death: None,
            mode: Mode::Solo,
        },
        &NoAssets,
    )
    .unwrap()
}

fn target_center(game: &Game) -> Vec2 {
    let target = game.players()[1];
    target.pose().center(target.tuning().hitbox.value)
}

/// How far below the target's center the flight passes as it crosses the center's x (negative:
/// above), or `None` if it never gets there.
fn miss_below(flight: &Flight, target: Vec2) -> Option<f64> {
    let mut previous = flight.at(0);
    for tick in 1..200_000 {
        let at = flight.at(tick);
        if at.x >= target.x {
            // Interpolate between the two ticks that straddle the crossing.
            let fraction = (target.x - previous.x) / (at.x - previous.x);
            return Some(previous.y + (at.y - previous.y) * fraction - target.y);
        }
        if at.y > target.y + 2000.0 {
            return None;
        }
        previous = at;
    }
    None
}

/// The flight from seat 0 at `aim` and `bars`, with the acceleration `accel` (so the fit can try
/// constants other than the tuned ones).
fn flight(game: &Game, aim: i16, bars: f64, accel: Vec2) -> Flight {
    flight_of(game, Shot::S1, aim, bars, accel)
}

/// `flight`, leaving `shot`'s barrel, `aim` degrees above level toward the target: over the back
/// for a mobile that fires that way.
fn flight_of(game: &Game, shot: Shot, aim: i16, bars: f64, accel: Vec2) -> Flight {
    let backward = shot.tuning(game.players()[0].tuning()).aim.value.backward();
    let mut flight = game.launch(0, shot, if backward { 180 - aim } else { aim }, bars).unwrap();
    flight.accel = accel;
    flight
}

/// The continuous S1 power that passes through the target's center, by bisection.
fn ideal_power(game: &Game, aim: i16, accel: Vec2) -> f64 {
    ideal_power_of(game, Shot::S1, aim, accel)
}

/// `ideal_power` for `fired`, from its own barrel.
fn ideal_power_of(game: &Game, fired: Shot, aim: i16, accel: Vec2) -> f64 {
    let target = target_center(game);
    let (mut short, mut long) = (0.1, 8.0);
    for _ in 0..60 {
        let mid = f64::midpoint(short, long);
        match miss_below(&flight_of(game, fired, aim, mid, accel), target) {
            Some(below) if below < 0.0 => long = mid,
            _ => short = mid,
        }
    }
    f64::midpoint(short, long)
}

/// The tuned acceleration on Coilback's shots in `wind`.
fn tuned(wind: Wind) -> Vec2 {
    tuned_for(MobileKind::Coilback, wind)
}

/// The tuned acceleration on `mobile`'s shots in `wind`.
fn tuned_for(mobile: MobileKind, wind: Wind) -> Vec2 {
    let mobile = mobile.tuning();
    ballistics::acceleration(mobile.gravity.value, wind, mobile.wind_sensitivity.value)
}

/// How many bars `wind` toward the target saves `mobile` at `aim` over one screen, with gravity
/// `gravity` and wind sensitivity `sensitivity`.
fn wind_saving(mobile: MobileKind, aim: i16, wind: Wind, gravity: f64, sensitivity: f64) -> f64 {
    let game = duel_of(mobile, 800);
    let calm = ideal_power(&game, aim, ballistics::acceleration(gravity, Wind::default(), sensitivity));
    calm - ideal_power(&game, aim, ballistics::acceleration(gravity, wind, sensitivity))
}

/// The whole-degree aim that lands closest to the target's center at a fixed power. Every reachable
/// distance has a low and a high solution; fixed-power systems use the high one, above 45 degrees.
fn best_aim(game: &Game, bars: f64, accel: Vec2) -> i16 {
    let target = target_center(game);
    (46..=89)
        .min_by(|&a, &b| {
            let miss = |aim| miss_below(&flight(game, aim, bars, accel), target).map_or(f64::MAX, f64::abs);
            miss(a).total_cmp(&miss(b))
        })
        .unwrap()
}

/// `best_aim` among high lobs only (80 to 89 degrees): at full power the low solution crosses a
/// near target's center on the way up.
fn lob_aim(game: &Game, bars: f64, accel: Vec2) -> i16 {
    let target = target_center(game);
    (80..=89)
        .min_by(|&a, &b| {
            let miss = |aim| miss_below(&flight(game, aim, bars, accel), target).map_or(f64::MAX, f64::abs);
            miss(a).total_cmp(&miss(b))
        })
        .unwrap()
}

fn assert_bars(name: &str, actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= TOLERANCE_BARS, "{name}: needs {actual:.3} bars, the formula says {expected}");
}

#[test]
fn formula_cbchui_fixed_power_60_degrees_lands_one_screen() {
    // CR-fixed: Armor's fixed power, 60 degrees = 1 screen, zero wind.
    assert_bars("cbchui", ideal_power(&duel(800), 60, tuned(Wind::default())), 2.45);
}

#[test]
fn formula_cbchui_fixed_power_by_mobile() {
    // CR-fixed, per mobile: each fitted gravity puts 60 degrees one screen away at its chart power.
    for (mobile, bars) in CBCHUI {
        assert_bars(&format!("cbchui {mobile:?}"), ideal_power(&duel_of(mobile, 800), 60, tuned_for(mobile, Wind::default())), bars);
    }
}

#[test]
fn formula_shot_weight_predicts_the_chart() {
    // The inference behind `INFERRED`: DBR's weights put every charted mobile within a tenth of a
    // bar of its CR-fixed power, the heaviest (Trico, Nak) included.
    for (weight, bars) in AX_AND_CBCHUI {
        let predicted = inferred_cbchui(weight);
        assert!((predicted - bars).abs() <= 0.1, "ax {weight}: predicts {predicted:.3}, the chart says {bars}");
    }
}

#[test]
fn formula_inferred_power_by_mobile() {
    // Each mobile the chart leaves out lands 60 degrees one screen away at the power its shot weight
    // implies.
    for (mobile, weight) in INFERRED {
        let bars = inferred_cbchui(weight);
        assert_bars(&format!("inferred {mobile:?}"), ideal_power(&duel_of(mobile, 800), 60, tuned_for(mobile, Wind::default())), bars);
    }
}

#[test]
fn formula_twinned_power_by_mobile() {
    // Each mobile whose shot weight the sources tie to a charted one's lands 60 degrees one screen
    // away at that one's chart power.
    for (mobile, twin) in WEIGHS_AS {
        assert_bars(
            &format!("{mobile:?} as {twin:?}"),
            ideal_power(&duel_of(mobile, 800), 60, tuned_for(mobile, Wind::default())),
            twin_power(twin),
        );
    }
}

#[test]
fn formula_sudsotter_holds_creedos_other_frog_powers() {
    // CR-frog, unfitted: cbchui-style "1.95 (half screen shots)" at 75 degrees, and Banpao-style
    // "between 2.5 and 2.6 bars" at 60 degrees, 1.5 screens ("untested").
    let (mobile, calm) = (MobileKind::Sudsotter, tuned_for(MobileKind::Sudsotter, Wind::default()));
    assert_bars("frog cbchui half screen", ideal_power(&duel_of(mobile, 400), 75, calm), 1.95);
    let banpao = ideal_power(&duel_of(mobile, 1200), 60, calm);
    assert!((2.5 - TOLERANCE_BARS..=2.6 + TOLERANCE_BARS).contains(&banpao), "frog banpao: needs {banpao:.3} bars");
}

#[test]
fn formula_frostfin_tusk_lands_where_its_cannon_does() {
    // GF-ice: S2 leaves the trunk, lower than S1's cannon, yet "hits the exact same place" in the
    // open. Fitted from the cannon, the tusk's shots hold CR-fixed's power too.
    let game = duel_of(MobileKind::Frostfin, 800);
    let calm = tuned_for(MobileKind::Frostfin, Wind::default());
    for shot in [Shot::S2, Shot::Ss] {
        assert_bars(&format!("Frostfin {shot:?} from the tusk"), ideal_power_of(&game, shot, 60, calm), 2.33);
    }
}

#[test]
fn formula_frostfin_seventy_degree_table() {
    // GF-ice's 70-degree power table, which doesn't say which shot it was made with: from the
    // cannon and from the tusk alike, at the distances docs/research/frostfin.md keeps.
    let calm = tuned_for(MobileKind::Frostfin, Wind::default());
    for (eighths, bars) in FROSTFIN_SEVENTY {
        let game = duel_of(MobileKind::Frostfin, 100 * eighths);
        for shot in [Shot::S1, Shot::S2] {
            assert_bars(&format!("Frostfin {shot:?} at 70 degrees, {eighths}/8 screen"), ideal_power_of(&game, shot, 70, calm), bars);
        }
    }
}

#[test]
fn formula_kalsiddon_60_degrees_lands_one_screen() {
    // CR-kal, CR-kal30: Herdhound's gravity puts 60 degrees one screen away at both sources' power.
    let power = ideal_power(&duel_of(MobileKind::Herdhound, 800), 60, tuned_for(MobileKind::Herdhound, Wind::default()));
    for bars in KAL_ONE_SCREEN {
        assert_bars("Kalsiddon one screen", power, bars);
    }
}

#[test]
fn formula_aduka_60_degrees_lands_one_screen() {
    // CR-fixed, GN-aduka: Flarecrow's gravity puts 60 degrees one screen away, over its back, at
    // both sources' power.
    let power = ideal_power(&duel_of(MobileKind::Flarecrow, 800), 60, tuned_for(MobileKind::Flarecrow, Wind::default()));
    for bars in ADUKA_ONE_SCREEN {
        assert_bars("Aduka one screen", power, bars);
    }
}

#[test]
fn formula_flarecrow_holds_adukas_power_tables() {
    // GN-aduka's and CR-aduka's tables, unfitted, at the entries docs/research/flarecrow.md keeps.
    let calm = tuned_for(MobileKind::Flarecrow, Wind::default());
    for (source, aim, quarters, bars) in ADUKA_POWERS {
        let power = ideal_power(&duel_of(MobileKind::Flarecrow, 200 * quarters), aim, calm);
        assert_bars(&format!("{source}: {aim} degrees, {quarters}/4 screen"), power, bars);
    }
}

#[test]
fn formula_flarecrow_full_power_lobs_land_where_creedo_says() {
    // CR-aduka, unfitted: at full power, 87 degrees lands a third of a screen away, 84 two thirds
    // and 81 one screen.
    let calm = tuned_for(MobileKind::Flarecrow, Wind::default());
    for (distance, aim) in ADUKA_FULL_POWER {
        let landed = lob_aim(&duel_of(MobileKind::Flarecrow, distance), 4.0, calm);
        assert!((landed - aim).abs() <= 1, "{distance} px at full power: {landed} degrees, CR-aduka says {aim}");
    }
}

#[test]
fn formula_nak_60_degrees_lands_one_screen() {
    // CR-fixed, CR-nak: Augerdillo's gravity puts 60 degrees, fired over its back, one screen away
    // at both sources' power.
    let power = ideal_power(&duel_of(MobileKind::Augerdillo, 800), 60, tuned_for(MobileKind::Augerdillo, Wind::default()));
    for bars in nak_one_screen() {
        assert_bars("Nak one screen", power, bars);
    }
}

#[test]
fn formula_nak_shot_formula_at_its_own_angles() {
    // CR-nak, unfitted: the formula's powers at the angles Augerdillo fires at.
    let calm = tuned_for(MobileKind::Augerdillo, Wind::default());
    for (aim, screens) in NAK_FORMULA {
        // A quarter screen is a whole number of pixels.
        #[allow(clippy::cast_possible_truncation)]
        let game = duel_of(MobileKind::Augerdillo, (screens * SCREEN) as i32);
        assert_bars(&format!("Nak formula, {aim} degrees, {screens} screens"), ideal_power(&game, aim, calm), cr_nak(screens, aim));
    }
}

#[test]
fn formula_nak_guides_high_shots() {
    // CR-ga1, unfitted: 70 degrees lands half a screen at about 2.1 bars. CR-ga1, CR-sage and CR-fun
    // put a full-power shot one screen away at 76 to 79 degrees.
    let calm = tuned_for(MobileKind::Augerdillo, Wind::default());
    assert_bars("Nak 70 degrees, half a screen", ideal_power(&duel_of(MobileKind::Augerdillo, 400), 70, calm), 2.1);
    let game = duel_of(MobileKind::Augerdillo, 800);
    let target = target_center(&game);
    let full = (60..=89)
        .min_by(|&a, &b| {
            let miss = |aim| miss_below(&flight(&game, aim, 4.0, calm), target).map_or(f64::MAX, f64::abs);
            miss(a).total_cmp(&miss(b))
        })
        .unwrap();
    assert!((76..=79).contains(&full), "a full-power shot lands one screen away at {full} degrees");
}

#[test]
fn formula_nak_vertical_wind_acts_as_its_gravity() {
    // CR-nak: vertical wind adds to the formula's gravity. Augerdillo's wind sensitivity (fitted at 14
    // up) saves what the formula does at every vertical wind, up or down.
    let tuning = MobileKind::Augerdillo.tuning();
    for up in NAK_WINDS {
        let wind = Wind::new(up.unsigned_abs(), if up > 0 { 90 } else { 270 }).unwrap();
        let saved = wind_saving(MobileKind::Augerdillo, 60, wind, tuning.gravity.value, tuning.wind_sensitivity.value);
        assert_bars(&format!("Nak formula, {up} up-wind"), saved, nak_saving(f64::from(up)));
    }
}

#[test]
fn formula_boomer_hooks_with_the_wind() {
    // UC-boomer: Hookclaw's wind sensitivity saves it about 0.45 bars with 10 wind behind a
    // 70-degree, one-screen shot.
    let (aim, strength, saving) = BOOMER_HOOK;
    let tuning = MobileKind::Hookclaw.tuning();
    let saved = wind_saving(MobileKind::Hookclaw, aim, Wind::new(strength, 0).unwrap(), tuning.gravity.value, tuning.wind_sensitivity.value);
    assert_bars("boomer hook", saved, saving);
}

#[test]
fn formula_plain_throw_45_degrees_lands_one_screen() {
    // GF-tiro: 45 degrees with 2.25 bars is about 1 screen.
    assert_bars("plain throw", ideal_power(&duel(800), 45, tuned(Wind::default())), 2.25);
}

#[test]
fn formula_banpao_three_bars_60_degrees_lands_a_screen_and_a_half() {
    // CR-using: Banpao, about 3.0 bars, 60 degrees lands exactly 1.5 screens.
    assert_bars("banpao", ideal_power(&duel(1200), 60, tuned(Wind::default())), 3.0);
}

#[test]
fn formula_vertical_wind_moves_power_a_tenth_of_a_bar_per_seven() {
    // CR-wind, GN-aduka: 0.1 bar less per 7 units of up-wind, 0.1 more per 7 of down-wind.
    let game = duel(800);
    for (strength, direction, expected) in [(7, 90, 2.35), (14, 90, 2.25), (7, 270, 2.55), (14, 270, 2.65)] {
        let wind = Wind::new(strength, direction).unwrap();
        assert_bars(&format!("{strength} wind at {direction}"), ideal_power(&game, 60, tuned(wind)), expected);
    }
}

#[test]
fn formula_side_wind_moves_the_angle_half_a_degree_per_unit() {
    // CR-wind, CR-philo: at fixed power, add half the tail wind to the angle (subtract half the
    // head wind).
    let game = duel(800);
    for strength in [4_u8, 10, 16] {
        let half = i16::from(strength) / 2;
        let tail = best_aim(&game, 2.45, tuned(Wind::new(strength, 0).unwrap()));
        let head = best_aim(&game, 2.45, tuned(Wind::new(strength, 180).unwrap()));
        assert!((tail - (60 + half)).abs() <= 1, "{strength} tail wind: {tail}, the rule says {}", 60 + half);
        assert!((head - (60 - half)).abs() <= 1, "{strength} head wind: {head}, the rule says {}", 60 - half);
    }
}

// ── Spoutpot (Turtle): airtime ──────────────────────────────────────────────
//
// Turtle's shots are timed, so CR-turtle's tables give airtimes as well as distances: its long
// forks land on S2's fifth crossing, its skybombs open near the top of the arc, and its timebombs
// open just short of the target (docs/research/spoutpot.md). `tool_fit` sets S2's hertz from the
// forks and the share of speed the SS's balls keep from one skybomb; the rest are checks.

/// CR-turtle's long forks: aim, bars and screens, each landing on S2's fifth crossing.
const FORKS: [(i16, f64, f64); 8] = [
    (82, 1.55, 0.125),
    (73, 1.61, 0.25),
    (65, 1.69, 0.375),
    (59, 1.81, 0.5),
    (53, 1.92, 0.625),
    (47, 2.08, 0.75),
    (43, 2.25, 0.875),
    (40, 2.4, 1.0),
];

/// The skybomb `tool_fit` fits the balls' speed to: about one screen (CR-turtle).
const SKYBOMB_FIT: (i16, f64, f64) = (76, 3.91, 795.0);

/// CR-turtle's skybombs: aim, bars, and px to the target, each opening near the top of its arc.
const SKYBOMBS: [(i16, f64, f64); 17] = [
    (88, 3.68, 112.0),
    (87, 3.68, 167.0),
    (86, 3.69, 220.0),
    (85, 3.7, 274.0),
    (84, 3.7, 328.0),
    (83, 3.72, 387.0),
    (82, 3.74, 446.0),
    (81, 3.77, 503.0),
    (80, 3.79, 560.0),
    (79, 3.82, 617.0),
    (78, 3.85, 676.0),
    (77, 3.88, 735.0),
    (76, 3.91, 795.0),
    (75, 3.94, 860.0),
    (74, 3.97, 933.0),
    (73, 4.0, 1009.0),
    (72, 4.0, 1088.0),
];

/// CR-turtle's timebombs: aim, bars and screens, each opening a split second before the target.
const TIMEBOMBS: [(i16, f64, f64); 8] = [
    (84, 1.8, 0.125),
    (78, 1.85, 0.25),
    (71, 1.9, 0.375),
    (66, 2.0, 0.5),
    (60, 2.05, 0.625),
    (56, 2.15, 0.75),
    (52, 2.3, 0.875),
    (48, 2.4, 1.0),
];

fn spoutpot_duel(screens: f64) -> Game {
    // Table distances are whole pixels to a few.
    #[allow(clippy::cast_possible_truncation)]
    duel_of(MobileKind::Spoutpot, (screens * SCREEN).round() as i32)
}

/// Where `flight` comes down through the height of the target's center: how long it takes, and how
/// far from the shooter's feet a target would stand for it to pass through its center there.
/// Turtle's tables are airtimes: a landmark lands at about its distance after about its time.
fn landing(game: &Game, flight: &Flight) -> (f64, f64) {
    let (shooter, target) = (game.players()[0], game.players()[1]);
    let center = target_center(game);
    let offset = center.x - target.footing.world().x;
    let mut tick = 0;
    while flight.at(tick).y < center.y || flight.velocity_at(tick).y <= 0.0 {
        tick += 1;
    }
    (f64::from(tick) * TICK, flight.at(tick).x - offset - shooter.footing.world().x)
}

/// The middle of an SS's opening (between its two middle balls, the way the blob was going), for
/// gravity `gravity`, opening after `split` seconds with `kept` of the blob's speed.
fn opened(game: &Game, aim: i16, bars: f64, gravity: f64, split: f64, kept: f64) -> Flight {
    let blob = flight(game, aim, bars, Vec2::new(0.0, gravity));
    let tick = ticks(split);
    Flight {
        origin: blob.at(tick),
        velocity: blob.velocity_at(tick) * kept,
        accel: blob.accel,
    }
}

/// The hertz that puts the mean long fork's arrival on S2's fifth crossing, under `gravity`.
fn fit_hertz(gravity: f64) -> f64 {
    let times: Vec<f64> = FORKS
        .iter()
        .map(|&(aim, bars, screens)| {
            let game = spoutpot_duel(screens);
            landing(&game, &flight(&game, aim, bars, Vec2::new(0.0, gravity))).0
        })
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    5.0 / (2.0 * mean)
}

/// The share of speed the SS's balls keep that lands `SKYBOMB_FIT` on its distance.
fn fit_speed_kept(gravity: f64, hertz: f64) -> f64 {
    let (aim, bars, distance) = SKYBOMB_FIT;
    let game = spoutpot_duel(1.0);
    let split = spoutpot::SS_SPLIT_CROSSING.value / (2.0 * hertz);
    bisect(0.05, 1.0, &|kept| landing(&game, &opened(&game, aim, bars, gravity, split, kept)).1 < distance)
}

/// The SS opens this many seconds after launch.
fn ss_split() -> f64 {
    spoutpot::crossing_seconds(spoutpot::SS_SPLIT_CROSSING.value)
}

#[test]
fn formula_spoutpot_long_forks_meet_their_targets_as_the_streams_cross() {
    // CR-turtle: each long fork lands on S2's fifth crossing. A quarter of a crossing either way
    // leaves the streams at most 0.71 of their widest apart, both still on a mobile.
    let gravity = MobileKind::Spoutpot.tuning().gravity.value;
    for (aim, bars, screens) in FORKS {
        let game = spoutpot_duel(screens);
        let (at, _) = landing(&game, &flight(&game, aim, bars, Vec2::new(0.0, gravity)));
        let crossing = 2.0 * spoutpot::S2_HERTZ.value * at;
        assert!((crossing - 5.0).abs() <= 0.25, "fork {aim} deg, {bars} bars, {screens} screens: crossing {crossing:.2}");
    }
}

#[test]
fn formula_spoutpot_s2_comes_together_given_the_airtime() {
    // CR-turtle's FAQ: at 60 degrees S2 has the airtime to come together at 1 screen but not at
    // 1/2; at 70 degrees it is "barely enough" for 1/2 screen. (Its 60 degrees at 3/4 screen falls
    // 0.09 s short: docs/research/spoutpot.md.)
    let merge = spoutpot::crossing_seconds(spoutpot::S2_MERGE_CROSSING.value);
    let calm = tuned_for(MobileKind::Spoutpot, Wind::default());
    let airtime = |aim: i16, screens: f64| {
        let game = spoutpot_duel(screens);
        landing(&game, &flight(&game, aim, ideal_power(&game, aim, calm), calm)).0
    };
    assert!(airtime(60, 1.0) >= merge, "60 degrees at 1 screen");
    assert!(airtime(60, 0.5) < merge, "60 degrees at 1/2 screen");
    let barely = airtime(70, 0.5);
    assert!((barely - merge).abs() <= 0.1, "70 degrees at 1/2 screen: {barely:.2} s, coming together at {merge:.2} s");
}

#[test]
fn formula_spoutpot_skybombs_land_where_the_table_says() {
    // CR-turtle: "these won't be pixel perfect": within 6% of the table, or 10 px up close (one
    // fitted, the rest checks).
    let gravity = MobileKind::Spoutpot.tuning().gravity.value;
    let game = spoutpot_duel(1.0);
    for (aim, bars, distance) in SKYBOMBS {
        let (_, landed) = landing(&game, &opened(&game, aim, bars, gravity, ss_split(), spoutpot::SS_SPEED_KEPT.value));
        assert!(
            (landed - distance).abs() <= (0.06 * distance).max(10.0),
            "skybomb {aim} deg, {bars} bars: {landed:.0} px, the table says {distance}"
        );
    }
}

#[test]
fn formula_spoutpot_timebombs_open_a_split_second_before_their_targets() {
    // CR-turtle: a timebomb opens "a split second before" it would reach the target.
    let calm = tuned_for(MobileKind::Spoutpot, Wind::default());
    for (aim, bars, screens) in TIMEBOMBS {
        let game = spoutpot_duel(screens);
        let (at, _) = landing(&game, &flight(&game, aim, bars, calm));
        let early = at - ss_split();
        assert!((0.0..=0.2).contains(&early), "timebomb {aim} deg, {bars} bars, {screens} screens: opens {early:.2} s early");
    }
}

fn bisect(mut low: f64, mut high: f64, too_low: &dyn Fn(f64) -> bool) -> f64 {
    for _ in 0..60 {
        let mid = f64::midpoint(low, high);
        if too_low(mid) { low = mid } else { high = mid }
    }
    f64::midpoint(low, high)
}

/// The gravity that lands 60 degrees one screen away at `bars` for `mobile`, in zero wind. Heavier
/// gravity needs more power.
fn fit_gravity(mobile: MobileKind, bars: f64) -> f64 {
    let game = duel_of(mobile, 800);
    bisect(100.0, 2000.0, &|g| ideal_power(&game, 60, Vec2::new(0.0, g)) < bars)
}

/// Rewrites `const {name}: f64 = ...;` in `crates/sim/src/tuning/{file}` to `value`.
fn rewrite(file: &str, name: &str, value: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tuning").join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let prefix = format!("const {name}: f64 = ");
    let line = text.lines().find(|line| line.starts_with(&prefix)).unwrap_or_else(|| panic!("no {prefix}... in {file}"));
    std::fs::write(&path, text.replacen(line, &format!("{prefix}{value};"), 1)).unwrap();
}

/// What the fitted Spoutpot does with every CR-turtle table entry, beside what the table says.
fn spoutpot_report(gravity: f64, hertz: f64, kept: f64) {
    let split = spoutpot::SS_SPLIT_CROSSING.value / (2.0 * hertz);
    let merge = spoutpot::S2_MERGE_CROSSING.value / (2.0 * hertz);
    println!("  opens at {split:.3} s, comes together at {merge:.3} s");
    let calm = Vec2::new(0.0, gravity);
    for (aim, bars, screens) in FORKS {
        let game = spoutpot_duel(screens);
        let (at, landed) = landing(&game, &flight(&game, aim, bars, calm));
        println!(
            "  fork {aim} {bars} {screens}: lands {at:.3} s, crossing {:.2}, {landed:.0} px, table {:.0}",
            2.0 * hertz * at,
            screens * SCREEN
        );
    }
    let game = spoutpot_duel(1.0);
    for (aim, bars, distance) in SKYBOMBS {
        let (_, landed) = landing(&game, &opened(&game, aim, bars, gravity, split, kept));
        println!("  skybomb {aim} {bars}: {landed:.0} px, table {distance}");
    }
    for (aim, bars, screens) in TIMEBOMBS {
        let game = spoutpot_duel(screens);
        let (at, _) = landing(&game, &flight(&game, aim, bars, calm));
        let (_, landed) = landing(&game, &opened(&game, aim, bars, gravity, split, kept));
        println!(
            "  timebomb {aim} {bars} {screens}: lands {at:.3} s ({:.3} after opening); balls land {landed:.0} px, table {:.0}",
            at - split,
            screens * SCREEN
        );
    }
}

/// Fits Augerdillo's gravity (CR-fixed and CR-nak) and wind sensitivity (CR-nak's vertical wind
/// term) into its tuning file, and prints the Nak formula's entries against the fit.
fn fit_augerdillo() {
    let [chart, formula] = nak_one_screen();
    let gravity = fit_gravity(MobileKind::Augerdillo, f64::midpoint(chart, formula));
    rewrite("augerdillo.rs", "GRAVITY", &format!("{gravity:.1}"));
    println!("Augerdillo gravity: {gravity:.1}");
    // More sensitivity saves more power: find the one that saves what CR-nak does at 14 up-wind.
    let up = Wind::new(14, 90).unwrap();
    let sensitivity = bisect(0.0, 2.0, &|s| wind_saving(MobileKind::Augerdillo, 60, up, gravity, s) < nak_saving(14.0));
    rewrite("augerdillo.rs", "WIND_SENSITIVITY", &format!("{sensitivity:.2}"));
    println!("Augerdillo wind sensitivity: {sensitivity:.2}");
    // The Nak formula's other entries, against Augerdillo's fit (docs/research/augerdillo.md).
    let nak_calm = Vec2::new(0.0, gravity);
    for aim in [15, 30, 45, 50, 60, 70] {
        for eighths in [2, 4, 6, 8, 10, 12] {
            let screens = f64::from(eighths) / 8.0;
            let model = ideal_power(&duel_of(MobileKind::Augerdillo, 100 * eighths), aim, nak_calm);
            println!("Nak formula {aim} deg, {eighths}/8 screen: model {model:.2}, CR-nak {:.2}", cr_nak(screens, aim));
        }
    }
}

/// Derives the fitted constants. The M3 mobiles' and Spoutpot's gravities (cbchui), the inferred
/// mobiles' (`INFERRED`), Herdhound's (CR-kal), Flarecrow's (CR-fixed and GN-aduka), Augerdillo's
/// (`fit_augerdillo`), Hookclaw's wind sensitivity on top of its gravity (UC-boomer), and
/// Spoutpot's spiral and SS balls (CR-turtle) are written straight into their tuning files;
/// Coilback's gravity and the vertical and horizontal wind factors are printed, to copy by hand.
/// Run with `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, then the
/// acceptance tests (and re-record the golden replays if the constants moved).
#[test]
#[ignore = "a tool, not a test: rewrites the fitted constants"]
fn tool_fit() {
    let inferred = INFERRED.map(|(mobile, weight)| (mobile, inferred_cbchui(weight)));
    let twinned = WEIGHS_AS.map(|(mobile, twin)| (mobile, twin_power(twin)));
    for (mobile, bars) in CBCHUI.into_iter().skip(1).chain(inferred).chain(twinned) {
        let gravity = fit_gravity(mobile, bars);
        let file = format!("{}.rs", mobile.tuning().name.to_lowercase());
        rewrite(&file, "GRAVITY", &format!("{gravity:.1}"));
        println!("{mobile:?} gravity: {gravity:.1}");
        if mobile == MobileKind::Spoutpot {
            let hertz = fit_hertz(gravity);
            rewrite(&file, "HERTZ", &format!("{hertz:.3}"));
            let kept = fit_speed_kept(gravity, hertz);
            rewrite(&file, "SPEED_KEPT", &format!("{kept:.3}"));
            println!("{mobile:?} S2 hertz: {hertz:.3}; SS speed kept: {kept:.3}");
            spoutpot_report(gravity, hertz, kept);
        }
        if mobile == MobileKind::Hookclaw {
            // More sensitivity saves more power: find the one that saves exactly UC-boomer's.
            let (aim, strength, saving) = BOOMER_HOOK;
            let tail = Wind::new(strength, 0).unwrap();
            let sensitivity = bisect(0.0, 10.0, &|s| wind_saving(mobile, aim, tail, gravity, s) < saving);
            rewrite(&file, "WIND_SENSITIVITY", &format!("{sensitivity:.2}"));
            println!("{mobile:?} wind sensitivity: {sensitivity:.2}");
        }
    }

    fit_augerdillo();

    let kal = f64::midpoint(KAL_ONE_SCREEN[0], KAL_ONE_SCREEN[1]);
    let gravity = fit_gravity(MobileKind::Herdhound, kal);
    rewrite("herdhound.rs", "GRAVITY", &format!("{gravity:.1}"));
    println!("Herdhound gravity: {gravity:.1}");

    let aduka = f64::midpoint(ADUKA_ONE_SCREEN[0], ADUKA_ONE_SCREEN[1]);
    let gravity = fit_gravity(MobileKind::Flarecrow, aduka);
    rewrite("flarecrow.rs", "GRAVITY", &format!("{gravity:.1}"));
    println!("Flarecrow gravity: {gravity:.1}");

    let game = duel(800);
    let gravity = fit_gravity(MobileKind::Coilback, 2.45);
    println!("coilback gravity: {gravity:.1}");

    // Up-wind 7 should save exactly 0.1 bar.
    let vertical = bisect(0.0, 0.05, &|w| ideal_power(&game, 60, Vec2::new(0.0, gravity * (1.0 - 7.0 * w))) > 2.35);
    println!("wind y per unit: {vertical:.5} (tuned {})", WIND_Y_PER_UNIT.value);

    // A 10-unit tail wind should need exactly 5 more degrees at 2.45 bars: find the factor that
    // makes 65 degrees land on target. The shift is linear enough in this range to fit at one point.
    let target = target_center(&game);
    let horizontal = bisect(0.0, 0.05, &|w| {
        let accel = Vec2::new(10.0 * w * gravity, gravity);
        miss_below(&flight(&game, 65, 2.45, accel), target).is_some_and(|below| below > 0.0)
    });
    println!("wind x per unit: {horizontal:.5} (tuned {})", WIND_X_PER_UNIT.value);

    // What the tuned model predicts for the table entries left out of the acceptance set, next to
    // what each table says (docs/research/physics-fit.md).
    let calm = tuned(Wind::default());
    for (name, aim, distance, says) in [
        ("cbchui 75 deg, 1/2 screen", 75, 400, 2.45),
        ("2.4 system 70 deg, 2/3 screen", 70, 533, 2.4),
        ("BJSL 80 deg, 1/2 screen (Armor)", 80, 400, 2.9),
        ("Aduka 60 deg, 1/4 screen", 60, 200, 1.10),
        ("Aduka 60 deg, 1 1/4 screens", 60, 1000, 3.10),
        ("Aduka 60 deg, 1 1/2 screens", 60, 1200, 3.60),
        ("Aduka 70 deg, 1 screen", 70, 800, 2.85),
        ("Aduka 80 deg, 1/2 screen", 80, 400, 2.40),
        ("Aduka 80 deg, 1 screen", 80, 800, 3.80),
        ("plain throw 45 deg, 1/2 screen", 45, 400, 1.33),
    ] {
        println!("{name}: model {:.2}, table {says}", ideal_power(&duel(distance), aim, calm));
    }
    // GF-ice's 70-degree BJSL power table, for Frostfin (docs/research/frostfin.md).
    let frostfin = tuned_for(MobileKind::Frostfin, Wind::default());
    for (eighths, says) in [(1, 0.90), (2, 1.35), (4, 1.90), (6, 2.30), (8, 2.70), (12, 3.30), (16, 3.90)] {
        let game = duel_of(MobileKind::Frostfin, 100 * eighths);
        let (s1, s2) = (ideal_power(&game, 70, frostfin), ideal_power_of(&game, Shot::S2, 70, frostfin));
        println!("Frostfin 70 deg, {eighths}/8 screen: model S1 {s1:.2}, S2 {s2:.2}, table {says}");
    }

    // CR-kal's other Banpao-style, fixed-70 and cbchui figures for Kalsiddon, against Herdhound's fit.
    let kal_duel = |distance| duel_of(MobileKind::Herdhound, distance);
    let kal_calm = tuned_for(MobileKind::Herdhound, Wind::default());
    for (name, aim, distance, says) in [
        ("Kal Banpao 80 deg, 1/2 screen", 80, 400, 3.1),
        ("Kal Banpao 75 deg, 3/4 screen", 75, 600, 3.15),
        ("Kal Banpao 70 deg, 1 screen", 70, 800, 3.2),
        ("Kal Banpao 60 deg, 1 1/2 screens", 60, 1200, 3.25),
        ("Kal fixed 70, 1/2 screen", 70, 400, 2.25),
        ("Kal fixed 70, 3/4 screen", 70, 600, 2.75),
        ("Kal fixed 70, 1 1/2 screens", 70, 1200, 3.9),
        ("Kal cbchui 75 deg, 1/2 screen", 75, 400, 2.55),
    ] {
        println!("{name}: model {:.2}, CR-kal {says}", ideal_power(&kal_duel(distance), aim, kal_calm));
    }

    // Aduka's tables, in and out of the acceptance set, against Flarecrow's fit
    // (docs/research/flarecrow.md).
    let crow = tuned_for(MobileKind::Flarecrow, Wind::default());
    for (source, aim, quarters, says) in ADUKA_POWERS.into_iter().chain(ADUKA_POWERS_OUT) {
        let model = ideal_power(&duel_of(MobileKind::Flarecrow, 200 * quarters), aim, crow);
        println!("{source} {aim} deg, {quarters}/4 screen: Flarecrow {model:.3}, table {says}");
    }
    for (distance, says) in ADUKA_FULL_POWER {
        let game = duel_of(MobileKind::Flarecrow, distance);
        println!("CR-aduka full power, {distance} px: Flarecrow {} deg, guide {says}", lob_aim(&game, 4.0, crow));
    }

    let lob = flight(&duel(800), 80, ideal_power(&duel(800), 80, calm), calm);
    let target = target_center(&duel(800));
    let ticks = (0..20_000).find(|&t| lob.at(t).x >= target.x).unwrap();
    println!("80 deg one-screen flight: {:.2} s", f64::from(ticks) / 2048.0);
}
