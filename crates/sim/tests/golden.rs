//! Golden replays (PRD-0002 uat-003): recorded games that must replay to the same final state hash
//! forever, natively and under wasm32 (`cargo make determinism`, uat-004). A change that moves a
//! hash changes how recorded games play out; if that is intended, re-record (the ignored
//! `tool_record`) and update `GOLDEN` in the same commit.
//!
//! Each game is stored twice: as JSON (readable, and diffable when re-recorded) and in the wire
//! format, which pins the encoding itself.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use pretty_assertions::assert_eq;

use lobshot_sim::{
    drops::{Drop, DropKind},
    game::{Contact, Effect, Event, Game, MAX_SEATS, MobileKind, Phase, Roll, Seat, Setup, Shot, SuddenDeath, SuddenDeathKind, Team},
    items::{Item, Loadout, MAX_ITEMS},
    log::Log,
    map::{MapRef, NoAssets},
    units::{Centis, Facing, Power, Wind},
    weather::{Forecast, WeatherKind},
};

/// Name, JSON, wire bytes, event count, final state hash.
const GOLDEN: &[(&str, &str, &[u8], usize, u64)] = &[
    ("duel_flat", include_str!("golden/duel_flat.json"), include_bytes!("golden/duel_flat.bin"), 32, 0xf41c_fd63_93d4_27b1),
    (
        "duel_hills_wind",
        include_str!("golden/duel_hills_wind.json"),
        include_bytes!("golden/duel_hills_wind.bin"),
        44,
        0xcc67_363f_80fd_6825,
    ),
    ("brawl_2v2", include_str!("golden/brawl_2v2.json"), include_bytes!("golden/brawl_2v2.bin"), 183, 0x0b84_576f_b904_f244),
    ("brawl_4v4", include_str!("golden/brawl_4v4.json"), include_bytes!("golden/brawl_4v4.bin"), 167, 0x7671_97d2_a3c6_3639),
    (
        "duel_big_bomb",
        include_str!("golden/duel_big_bomb.json"),
        include_bytes!("golden/duel_big_bomb.bin"),
        73,
        0x5465_b18c_def9_03a3,
    ),
    (
        "duel_ss_death",
        include_str!("golden/duel_ss_death.json"),
        include_bytes!("golden/duel_ss_death.bin"),
        33,
        0x3229_6916_11f1_63ed,
    ),
    (
        "roster_thistleback",
        include_str!("golden/roster_thistleback.json"),
        include_bytes!("golden/roster_thistleback.bin"),
        63,
        0x04c8_d350_1189_422b,
    ),
];

#[test]
fn golden_logs_replay_to_their_recorded_hashes() {
    for &(name, json, wire, events, hash) in GOLDEN {
        let log = Log::from_json(json).unwrap();
        assert_eq!(Log::decode(wire).unwrap(), log, "{name}: the wire form decodes to the JSON form");
        assert_eq!(log.events.len(), events, "{name}: event count");
        let game = log.replay(&NoAssets).unwrap();
        assert_eq!(game.state_hash().unwrap(), hash, "{name}: final state hash (0x{:016x})", game.state_hash().unwrap());
    }
}

#[test]
fn golden_games_finish() {
    for &(name, json, ..) in GOLDEN {
        let game = Log::from_json(json).unwrap().replay(&NoAssets).unwrap();
        assert!(matches!(game.phase(), Phase::Over { .. }), "{name} ends in a finished game");
    }
}

#[test]
fn golden_games_play_every_m3_rule() {
    // Every item used, SS locked, each sudden death begun, a drop landed, and every mobile fired.
    let mut items = Vec::new();
    let mut sudden_deaths = Vec::new();
    let mut mobiles = Vec::new();
    let (mut locked, mut dropped) = (false, false);
    for &(_, json, ..) in GOLDEN {
        let log = Log::from_json(json).unwrap();
        let mut game = Game::new(&log.setup, &NoAssets).unwrap();
        for event in &log.events {
            for effect in game.apply(*event).unwrap() {
                match effect {
                    Effect::ItemUsed { item, .. } => items.push(item),
                    Effect::SuddenDeath { kind } => sudden_deaths.push(kind),
                    Effect::Volley { seat, .. } => mobiles.push(game.players()[usize::from(seat)].mobile),
                    Effect::SsLocked { .. } => locked = true,
                    Effect::Dropped { .. } => dropped = true,
                    _ => {}
                }
            }
        }
    }
    assert!(Item::ALL.iter().all(|item| items.contains(item)), "items used: {items:?}");
    assert!(
        [SuddenDeathKind::BigBomb, SuddenDeathKind::Double, SuddenDeathKind::SsDeath]
            .iter()
            .all(|kind| sudden_deaths.contains(kind)),
        "sudden deaths begun: {sudden_deaths:?}"
    );
    assert!(MobileKind::ALL.iter().all(|mobile| mobiles.contains(mobile)), "mobiles that fired: {mobiles:?}");
    assert!(locked && dropped, "SS locked: {locked}; a drop landed: {dropped}");
}

// ── Recording ────────────────────────────────────────────────────────────────

/// Re-records every golden game and prints the `GOLDEN` table. Native only (it writes files).
#[test]
#[ignore = "a tool, not a test: rewrites tests/golden/"]
fn tool_record() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, log) in scenarios() {
        std::fs::write(dir.join(format!("{name}.json")), log.to_json().unwrap() + "\n").unwrap();
        std::fs::write(dir.join(format!("{name}.bin")), log.encode().unwrap()).unwrap();
        let hash = log.replay(&NoAssets).unwrap().state_hash().unwrap();
        println!(
            "    (\"{name}\", include_str!(\"golden/{name}.json\"), include_bytes!(\"golden/{name}.bin\"), {}, 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            log.events.len(),
            hash >> 48,
            (hash >> 32) & 0xffff,
            (hash >> 16) & 0xffff,
            hash & 0xffff,
        );
    }
}

fn seat(team: Team, spawn_x: i32, facing: Facing) -> Seat {
    armed(team, MobileKind::Coilback, spawn_x, facing, &[])
}

fn armed(team: Team, mobile: MobileKind, spawn_x: i32, facing: Facing, items: &[Item]) -> Seat {
    Seat {
        team,
        mobile,
        spawn_x,
        facing,
        items: Loadout::new(items).unwrap(),
    }
}

/// Four loadouts that between them carry every item, the ones that don't add damage first so a
/// short game still gets to them all.
const LOADOUTS: [&[Item]; 4] = [
    &[Item::Heal, Item::WindFlip, Item::Dig, Item::PowerUp, Item::Dual],
    &[Item::Sacrifice, Item::Teleport, Item::Heal, Item::Thunder],
    &[Item::Swap, Item::BigHeal, Item::DualPlus],
    &[Item::Dig, Item::Thunder, Item::PowerUp, Item::Dual],
];

fn scenarios() -> Vec<(&'static str, Log)> {
    let winds = [Wind::new(6, 30).unwrap(), Wind::new(14, 200).unwrap(), Wind::new(3, 270).unwrap(), Wind::new(20, 95).unwrap()];
    vec![
        ("duel_flat", play(duel_flat(), &[])),
        ("duel_hills_wind", play(duel_hills_wind(), &winds)),
        ("brawl_2v2", play(brawl_2v2(), &winds)),
        ("brawl_4v4", play(brawl_4v4(), &winds)),
        ("duel_big_bomb", play(duel_big_bomb(), &winds)),
        ("duel_ss_death", play(duel_ss_death(), &winds)),
        ("roster_thistleback", play(roster_thistleback(), &winds)),
    ]
}

fn duel_flat() -> Setup {
    Setup {
        map: MapRef::Flat { width: 1600, height: 700, ground: 560 },
        seats: vec![seat(Team::A, 350, Facing::Right), seat(Team::B, 1150, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::default(),
        sudden_death: None,
    }
}

fn duel_hills_wind() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 140,
            period: 520,
        },
        seats: vec![seat(Team::A, 420, Facing::Right), seat(Team::B, 1460, Facing::Left)],
        order: vec![1, 0],
        wind: Wind::new(9, 160).unwrap(),
        sudden_death: None,
    }
}

fn brawl_2v2() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2400,
            height: 900,
            ground: 780,
            amplitude: 90,
            period: 700,
        },
        seats: vec![
            seat(Team::A, 300, Facing::Right),
            seat(Team::B, 1300, Facing::Left),
            seat(Team::A, 700, Facing::Right),
            seat(Team::B, 2000, Facing::Left),
        ],
        order: vec![0, 1, 2, 3],
        wind: Wind::new(4, 0).unwrap(),
        sudden_death: None,
    }
}

/// Every mobile, every item, the slot machine and Double.
fn brawl_4v4() -> Setup {
    let mobiles = [
        MobileKind::Coilback,
        MobileKind::Marblemole,
        MobileKind::Lanternwing,
        MobileKind::Hookclaw,
        MobileKind::Hookclaw,
        MobileKind::Sparkhorn,
        MobileKind::Sparkhorn,
        MobileKind::Lanternwing,
    ];
    let seats = (0_i32..)
        .zip(mobiles)
        .map(|(i, mobile)| {
            let (team, facing) = if i % 2 == 0 { (Team::A, Facing::Right) } else { (Team::B, Facing::Left) };
            armed(team, mobile, 200 + 350 * i, facing, LOADOUTS[usize::try_from(i).unwrap() % LOADOUTS.len()])
        })
        .collect();
    Setup {
        map: MapRef::Hills {
            width: 3000,
            height: 1000,
            ground: 860,
            amplitude: 110,
            period: 800,
        },
        seats,
        order: (0..8).collect(),
        wind: Wind::new(8, 20).unwrap(),
        // Earlier than a room offers (SUDDEN_DEATH_TURNS), so the brawl reaches it.
        sudden_death: Some(SuddenDeath { kind: SuddenDeathKind::Double, turn: 16 }),
    }
}

fn duel_big_bomb() -> Setup {
    Setup {
        map: MapRef::Flat { width: 1800, height: 700, ground: 560 },
        seats: vec![
            armed(Team::A, MobileKind::Sparkhorn, 400, Facing::Right, LOADOUTS[1]),
            armed(Team::B, MobileKind::Marblemole, 1300, Facing::Left, LOADOUTS[0]),
        ],
        order: vec![1, 0],
        wind: Wind::new(5, 300).unwrap(),
        sudden_death: Some(SuddenDeath { kind: SuddenDeathKind::BigBomb, turn: 8 }),
    }
}

fn duel_ss_death() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 80,
            period: 600,
        },
        seats: vec![
            armed(Team::A, MobileKind::Hookclaw, 420, Facing::Right, LOADOUTS[3]),
            armed(Team::B, MobileKind::Lanternwing, 1380, Facing::Left, LOADOUTS[2]),
        ],
        order: vec![0, 1],
        wind: Wind::new(11, 170).unwrap(),
        sudden_death: Some(SuddenDeath { kind: SuddenDeathKind::SsDeath, turn: 8 }),
    }
}

/// Thistleback against Coilback on hills in wind: its S1, S2 and SS each fire.
fn roster_thistleback() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Thistleback, 400, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
    }
}

/// Plays a game with a simple scripted player and returns its log. The dead spin their slot
/// machines whenever they may. Every third turn the wind changes (if `winds` has any); every
/// fourth, the player walks toward the nearest enemy; every seventh, it times out. Every turn it
/// first uses the first item it can. Otherwise it cycles S1, S2, SS (S1 when SS is locked or off) at the
/// middle of the shot's true range, with the power step that lands closest to the enemy.
fn play(setup: Setup, winds: &[Wind]) -> Log {
    let mut game = Game::new(&setup, &NoAssets).unwrap();
    let mut events = Vec::new();
    let mut apply = |game: &mut Game, event: Event| {
        game.apply(event).unwrap();
        events.push(event);
    };

    let mut rolls = 0_usize;
    for turn in 0..300_usize {
        // Answer the server's rolls from fixed cycles: weather kinds and columns across the map,
        // drops likewise, and the given winds (or calm). The calm duel gets only Force far off to
        // the side. The dead spin first, so their drops are owed too.
        loop {
            if let Some(dead) = (0..MAX_SEATS).filter_map(|s| u8::try_from(s).ok()).find(|&s| game.can_spin(s)) {
                apply(&mut game, Event::Spin { seat: dead });
            }
            let Some(roll) = game.pending_roll() else { break };
            rolls += 1;
            let event = match roll {
                Roll::Weather { .. } if winds.is_empty() => Event::Weather(Forecast::new(WeatherKind::Force, 1)),
                Roll::Weather { width, .. } => {
                    let kind = WeatherKind::ALL[rolls % WeatherKind::ALL.len()];
                    let x = i32::try_from((rolls * 389) % usize::try_from(width).unwrap()).unwrap();
                    let hides = (kind == WeatherKind::Random).then_some(WeatherKind::HIDDEN[rolls % WeatherKind::HIDDEN.len()]);
                    Event::Weather(Forecast { kind, x, hides })
                }
                Roll::Wind => Event::Wind(winds.get(rolls % winds.len().max(1)).copied().unwrap_or_default()),
                Roll::Drop { width } => {
                    let kind = DropKind::ALL[rolls % DropKind::ALL.len()];
                    let x = i32::try_from((rolls * 389) % usize::try_from(width).unwrap()).unwrap();
                    Event::Drop(Drop { kind, x })
                }
            };
            apply(&mut game, event);
        }
        let Phase::Turn { seat } = game.phase() else { break };
        let me = game.players()[usize::from(seat)];
        let enemy = game
            .players()
            .iter()
            .filter(|p| p.alive && p.team != me.team)
            .min_by(|a, b| (a.footing.x - me.footing.x).abs().cmp(&(b.footing.x - me.footing.x).abs()))
            .copied()
            .unwrap();
        let toward = if enemy.footing.x > me.footing.x { Facing::Right } else { Facing::Left };
        let steps = if turn % 4 == 1 { 25 } else { 0 };
        apply(&mut game, Event::Walk { seat, facing: toward, steps });
        if !matches!(game.phase(), Phase::Turn { seat: s } if s == seat) {
            continue;
        }
        if let Some(slot) = (0..MAX_ITEMS).filter_map(|s| u8::try_from(s).ok()).find(|&s| game.can_use_item(seat, s)) {
            apply(&mut game, Event::UseItem { seat, slot, time: Centis(0) });
            if !matches!(game.phase(), Phase::Turn { seat: s } if s == seat) {
                continue;
            }
        }
        let time = Centis(u32::try_from(300 + turn * 137 % 1500).unwrap());
        if turn % 7 == 6 {
            apply(&mut game, Event::Pass { seat, time: Centis(2000) });
            continue;
        }

        let shot = [Shot::S1, Shot::S2, Shot::Ss][turn % 3];
        // A locked or disabled SS falls back to S1.
        let shot = if game.can_shoot(seat, shot) { shot } else { Shot::S1 };
        let range = shot.tuning(me.tuning()).aim.value;
        let aim = i16::midpoint(range.true_min, range.true_max);
        let enemy_index = game.players().iter().position(|p| *p == enemy).unwrap();
        let center = game.players()[enemy_index].center();
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.fly(&game.launch(seat, aim, power.bars())?, 0);
                (impact.contact != Contact::Gone).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Power::MAX, |(_, power)| power);
        apply(&mut game, Event::Shoot { seat, shot, aim, power, time });
    }
    Log { setup, events }
}
