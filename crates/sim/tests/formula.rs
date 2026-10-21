//! GunBound's community aiming formulas as acceptance tests (PRD-0002 uat-002): each holds in
//! lobshot within 0.05 bar, or one degree for angle rules. `docs/research/physics-fit.md` says which
//! formulas are in, which are out and why, and how the fitted constants were found (the ignored
//! `tool_fit` below).
//!
//! The setup is a duel on flat ground: the shooter (Coilback, Armor's behavior and GunBound's
//! reference shot, unless a test says otherwise) at x = 300 facing right, and a Coilback target
//! `distance` px away. "Ideal power" is the continuous power whose flight passes through the
//! target's hitbox center. Aim limits don't apply here: the formulas are about the ballistics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lobshot_sim::{
    ballistics::{self, Flight},
    game::{Game, MobileKind, Seat, Setup, Team},
    items::Loadout,
    map::{MapRef, NoAssets},
    tuning::{WIND_X_PER_UNIT, WIND_Y_PER_UNIT},
    units::{Facing, Vec2, Wind},
};

const TOLERANCE_BARS: f64 = 0.05;

/// CR-fixed's cbchui chart: the power that lands 60 degrees one screen away in zero wind, for each
/// mobile's GunBound counterpart (Armor, Mage, Boomer, Lightning, Grub).
const CBCHUI: [(MobileKind, f64); 5] = [
    (MobileKind::Coilback, 2.45),
    (MobileKind::Lanternwing, 2.38),
    (MobileKind::Hookclaw, 2.25),
    (MobileKind::Sparkhorn, 2.30),
    (MobileKind::Marblemole, 2.33),
];

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
                    facing: Facing::Right,
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
    let mut flight = game.launch(0, aim, bars).unwrap();
    flight.accel = accel;
    flight
}

/// The continuous power that passes through the target's center, by bisection.
fn ideal_power(game: &Game, aim: i16, accel: Vec2) -> f64 {
    let target = target_center(game);
    let (mut short, mut long) = (0.1, 8.0);
    for _ in 0..60 {
        let mid = f64::midpoint(short, long);
        match miss_below(&flight(game, aim, mid, accel), target) {
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

/// Derives the fitted constants. The four M3 mobiles' gravities (cbchui), and Hookclaw's wind
/// sensitivity on top of its gravity (UC-boomer), are written straight into their tuning files;
/// Coilback's gravity and the vertical and horizontal wind factors are printed, to copy by hand.
/// Run with `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, then the
/// acceptance tests (and re-record the golden replays if the constants moved).
#[test]
#[ignore = "a tool, not a test: rewrites the fitted constants"]
fn tool_fit() {
    for (mobile, bars) in CBCHUI.into_iter().skip(1) {
        let gravity = fit_gravity(mobile, bars);
        let file = format!("{}.rs", mobile.tuning().name.to_lowercase());
        rewrite(&file, "GRAVITY", &format!("{gravity:.1}"));
        println!("{mobile:?} gravity: {gravity:.1}");
        if mobile == MobileKind::Hookclaw {
            // More sensitivity saves more power: find the one that saves exactly UC-boomer's.
            let (aim, strength, saving) = BOOMER_HOOK;
            let tail = Wind::new(strength, 0).unwrap();
            let sensitivity = bisect(0.0, 10.0, &|s| wind_saving(mobile, aim, tail, gravity, s) < saving);
            rewrite(&file, "WIND_SENSITIVITY", &format!("{sensitivity:.2}"));
            println!("{mobile:?} wind sensitivity: {sensitivity:.2}");
        }
    }

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
    let lob = flight(&duel(800), 80, ideal_power(&duel(800), 80, calm), calm);
    let target = target_center(&duel(800));
    let ticks = (0..20_000).find(|&t| lob.at(t).x >= target.x).unwrap();
    println!("80 deg one-screen flight: {:.2} s", f64::from(ticks) / 2048.0);
}
