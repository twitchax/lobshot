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
    jewels::{JewelDrop, JewelKind},
    log::Log,
    map::{MapRef, NoAssets},
    mode::Mode,
    units::{Centis, Facing, Power, Wind},
    weather::{Forecast, WeatherKind},
};

/// Name, JSON, wire bytes, event count, final state hash.
const GOLDEN: &[(&str, &str, &[u8], usize, u64)] = &[
    ("duel_flat", include_str!("golden/duel_flat.json"), include_bytes!("golden/duel_flat.bin"), 32, 0x51bc_acfe_f530_0bfb),
    (
        "duel_hills_wind",
        include_str!("golden/duel_hills_wind.json"),
        include_bytes!("golden/duel_hills_wind.bin"),
        44,
        0xeb9d_3672_872e_2e5c,
    ),
    ("brawl_2v2", include_str!("golden/brawl_2v2.json"), include_bytes!("golden/brawl_2v2.bin"), 114, 0x9310_ee2e_20e4_226c),
    ("brawl_4v4", include_str!("golden/brawl_4v4.json"), include_bytes!("golden/brawl_4v4.bin"), 167, 0x3997_afc9_6adb_ed1a),
    (
        "duel_big_bomb",
        include_str!("golden/duel_big_bomb.json"),
        include_bytes!("golden/duel_big_bomb.bin"),
        62,
        0x1bfb_48dc_95bf_4eb4,
    ),
    (
        "duel_ss_death",
        include_str!("golden/duel_ss_death.json"),
        include_bytes!("golden/duel_ss_death.bin"),
        33,
        0xa381_96c7_c746_6ac7,
    ),
    (
        "roster_spoutpot",
        include_str!("golden/roster_spoutpot.json"),
        include_bytes!("golden/roster_spoutpot.bin"),
        52,
        0x2d67_7684_48f7_b15a,
    ),
    (
        "roster_tuskrack",
        include_str!("golden/roster_tuskrack.json"),
        include_bytes!("golden/roster_tuskrack.bin"),
        52,
        0x88a7_b022_9dd5_f31c,
    ),
    (
        "roster_sudsotter",
        include_str!("golden/roster_sudsotter.json"),
        include_bytes!("golden/roster_sudsotter.bin"),
        54,
        0x34fa_6a8e_69fb_ea76,
    ),
    (
        "roster_frostfin",
        include_str!("golden/roster_frostfin.json"),
        include_bytes!("golden/roster_frostfin.bin"),
        54,
        0xb03f_f31a_d9fc_330b,
    ),
    (
        "roster_herdhound",
        include_str!("golden/roster_herdhound.json"),
        include_bytes!("golden/roster_herdhound.bin"),
        54,
        0x8b56_0d4c_d42a_a19c,
    ),
    (
        "roster_thistleback",
        include_str!("golden/roster_thistleback.json"),
        include_bytes!("golden/roster_thistleback.bin"),
        63,
        0xea75_4830_5626_fad4,
    ),
    (
        "roster_kitewyrm",
        include_str!("golden/roster_kitewyrm.json"),
        include_bytes!("golden/roster_kitewyrm.bin"),
        32,
        0x6788_f159_099e_07e0,
    ),
    (
        "roster_flarecrow",
        include_str!("golden/roster_flarecrow.json"),
        include_bytes!("golden/roster_flarecrow.bin"),
        74,
        0x2fc4_1741_ed3a_19c1,
    ),
    (
        "roster_augerdillo",
        include_str!("golden/roster_augerdillo.json"),
        include_bytes!("golden/roster_augerdillo.bin"),
        99,
        0x751b_cb69_1fa8_4b3b,
    ),
    (
        "roster_lensowl",
        include_str!("golden/roster_lensowl.json"),
        include_bytes!("golden/roster_lensowl.bin"),
        65,
        0x1580_c23f_360c_70c5,
    ),
    (
        "roster_needlemouse",
        include_str!("golden/roster_needlemouse.json"),
        include_bytes!("golden/roster_needlemouse.bin"),
        32,
        0xf606_d4b2_4bde_51a2,
    ),
    (
        "roster_tinbadger",
        include_str!("golden/roster_tinbadger.json"),
        include_bytes!("golden/roster_tinbadger.bin"),
        52,
        0x6126_598e_30e9_54f2,
    ),
    (
        "mode_score",
        include_str!("golden/mode_score.json"),
        include_bytes!("golden/mode_score.bin"),
        153,
        0xd5c1_5a65_981b_e721,
    ),
    ("mode_tag", include_str!("golden/mode_tag.json"), include_bytes!("golden/mode_tag.bin"), 80, 0x3b98_477a_b37b_2108),
    (
        "mode_jewel",
        include_str!("golden/mode_jewel.json"),
        include_bytes!("golden/mode_jewel.bin"),
        287,
        0xc015_bd10_ad12_029b,
    ),
    (
        "roster_lodestoad",
        include_str!("golden/roster_lodestoad.json"),
        include_bytes!("golden/roster_lodestoad.bin"),
        80,
        0xd87c_98ed_5039_4d1e,
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

#[test]
fn roster_frostfin_frosts_its_target() {
    // Frostfin fires all three shots, and its S2s and SSs leave the Coilback brittle.
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "roster_frostfin").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut shots, mut frosts) = (Vec::new(), Vec::new());
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Volley { seat: 0, shot } => shots.push(shot),
                Effect::Frosted { seat, brittle } => frosts.push((seat, brittle)),
                _ => {}
            }
        }
    }
    assert!([Shot::S1, Shot::S2, Shot::Ss].iter().all(|shot| shots.contains(shot)), "{shots:?}");
    assert!(frosts.len() >= 2 && frosts.iter().all(|&(seat, _)| seat == 1), "{frosts:?}");
    assert_eq!(game.players()[1].brittle, frosts.last().unwrap().1);
}

#[test]
fn roster_flarecrow_calls_skyeye_down() {
    // Flarecrow fires all three shots, and its flares or its SS call Skyeye's beam.
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "roster_flarecrow").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut shots, mut beams) = (Vec::new(), 0);
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Volley { seat: 0, shot } => shots.push(shot),
                Effect::Beam { .. } => beams += 1,
                _ => {}
            }
        }
    }
    assert!([Shot::S1, Shot::S2, Shot::Ss].iter().all(|shot| shots.contains(shot)), "{shots:?}");
    assert!(beams >= 3, "{beams} beams");
}

#[test]
fn roster_augerdillo_bores_and_rolls_through_the_hills() {
    // Augerdillo fires all three shots over its back: an S2 bores through the ground and comes up
    // to blow, and its SS rolls through the hills into the Coilback.
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "roster_augerdillo").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut shots, mut bored, mut rolled) = (Vec::new(), false, false);
    for event in &log.events {
        let effects = game.apply(*event).unwrap();
        let Event::Shoot { seat: 0, shot, .. } = *event else { continue };
        shots.push(shot);
        let blasts = effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count();
        let legs = effects.iter().find_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.len()) } else { None }).unwrap_or(0);
        bored |= shot == Shot::S2 && legs >= 2 && blasts > 0;
        rolled |= shot == Shot::Ss && effects.iter().any(|e| matches!(e, Effect::Damaged { seat: 1, .. }));
    }
    assert!([Shot::S1, Shot::S2, Shot::Ss].iter().all(|shot| shots.contains(shot)), "{shots:?}");
    assert!(bored && rolled, "bored: {bored}; rolled into the target: {rolled}");
}

#[test]
fn roster_tinbadger_lays_mines_that_outlive_their_turn() {
    // Tinbadger fires all three shots; its S2's mines stay on the map past the turn that laid
    // them, and crawl as its turn comes up.
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "roster_tinbadger").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut shots, mut laid, mut crawled, mut outlived) = (Vec::new(), 0, 0, false);
    for event in &log.events {
        let before = game.mines().to_vec();
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Volley { seat: 0, shot } => shots.push(shot),
                Effect::MineLaid { .. } => laid += 1,
                Effect::MineCrawled { .. } => crawled += 1,
                _ => {}
            }
        }
        outlived |= matches!(event, Event::Pass { .. } | Event::Shoot { seat: 1, .. }) && !before.is_empty();
    }
    assert!([Shot::S1, Shot::S2, Shot::Ss].iter().all(|shot| shots.contains(shot)), "{shots:?}");
    assert!(laid >= 2 && crawled >= 1 && outlived, "laid {laid}, crawled {crawled}, outlived a turn: {outlived}");
}

#[test]
fn roster_lodestoad_pulls_and_pushes_its_target() {
    // Lodestoad fires all three shots: its S2's magnet slides the Coilback toward where it struck,
    // and its SS's pyramid away.
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "roster_lodestoad").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut shots, mut slides) = (Vec::new(), Vec::new());
    let mut shot = None;
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Volley { seat: 0, shot: fired } => {
                    shots.push(fired);
                    shot = Some(fired);
                }
                Effect::Volley { .. } => shot = None,
                Effect::Slid { seat: 1, .. } => slides.extend(shot),
                _ => {}
            }
        }
    }
    assert!([Shot::S1, Shot::S2, Shot::Ss].iter().all(|shot| shots.contains(shot)), "{shots:?}");
    assert!(slides.contains(&Shot::S2) && slides.contains(&Shot::Ss), "the shots that slid the Coilback: {slides:?}");
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
        second: None,
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
        ("roster_spoutpot", play(roster_spoutpot(), &winds)),
        ("roster_tuskrack", play(roster_tuskrack(), &winds)),
        ("roster_sudsotter", play(roster_sudsotter(), &winds)),
        ("roster_frostfin", play(roster_frostfin(), &winds)),
        ("roster_herdhound", play(roster_herdhound(), &winds)),
        ("roster_thistleback", play(roster_thistleback(), &winds)),
        ("roster_kitewyrm", play(roster_kitewyrm(), &winds)),
        ("roster_flarecrow", play(roster_flarecrow(), &winds)),
        ("roster_augerdillo", play(roster_augerdillo(), &winds)),
        ("roster_lensowl", play(roster_lensowl(), &winds)),
        ("roster_needlemouse", play(roster_needlemouse(), &winds)),
        ("roster_tinbadger", play(roster_tinbadger(), &winds)),
        ("mode_score", play(mode_score(), &winds)),
        ("mode_tag", play(mode_tag(), &winds)),
        ("mode_jewel", play(mode_jewel(), &winds)),
        ("roster_lodestoad", play(roster_lodestoad(), &winds)),
    ]
}

fn duel_flat() -> Setup {
    Setup {
        map: MapRef::Flat { width: 1600, height: 700, ground: 560 },
        seats: vec![seat(Team::A, 350, Facing::Right), seat(Team::B, 1150, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::default(),
        sudden_death: None,
        mode: Mode::Solo,
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
        mode: Mode::Solo,
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
        mode: Mode::Solo,
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
        mode: Mode::Solo,
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
        mode: Mode::Solo,
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
        mode: Mode::Solo,
    }
}

/// Spoutpot firing S1, S2 and SS: a 1v1 on hills in wind.
fn roster_spoutpot() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Spoutpot, 420, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(10, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Tuskrack (PRD-0006 T-001): its ripples of rockets and its bursting grenades, on hills in wind.
fn roster_tuskrack() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Tuskrack, 440, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(12, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Sudsotter's sponges walking the hills, and its suds, in wind.
fn roster_sudsotter() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![
            armed(Team::A, MobileKind::Sudsotter, 440, Facing::Right, &[]),
            armed(Team::B, MobileKind::Coilback, 1420, Facing::Left, &[]),
        ],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Frostfin (PRD-0006 T-001) against Coilback on hills in wind: its S1, S2 and SS, and its target
/// growing brittle.
fn roster_frostfin() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Frostfin, 380, Facing::Right, &[]), seat(Team::B, 1420, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Herdhound (Kalsiddon's behavior) in a 1v1 on hills in wind: its S1, S2 and SS.
fn roster_herdhound() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![
            armed(Team::A, MobileKind::Herdhound, 420, Facing::Right, &[]),
            armed(Team::B, MobileKind::Coilback, 1380, Facing::Left, &[]),
        ],
        order: vec![0, 1],
        wind: Wind::new(8, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
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
        mode: Mode::Solo,
    }
}

/// A Score 2v2 on hills in wind: deaths cost lives, the dead drop back in, and a team runs out.
fn mode_score() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2400,
            height: 900,
            ground: 780,
            amplitude: 90,
            period: 700,
        },
        seats: vec![
            armed(Team::A, MobileKind::Coilback, 350, Facing::Right, &[]),
            armed(Team::B, MobileKind::Lanternwing, 2050, Facing::Left, &[]),
            armed(Team::A, MobileKind::Hookclaw, 900, Facing::Right, &[]),
            armed(Team::B, MobileKind::Sparkhorn, 1500, Facing::Left, &[]),
        ],
        order: vec![0, 1, 2, 3],
        wind: Wind::new(5, 60).unwrap(),
        sudden_death: None,
        mode: Mode::Score,
    }
}

/// A Tag 2v2 on hills in wind, each seat a pair: Tinbadger with Lensowl, Kitewyrm with Coilback,
/// Hookclaw with Frostfin, Lanternwing with Needlemouse. They switch every few turns, and a
/// player is out when the mobile in play falls.
fn mode_tag() -> Setup {
    let pair = |team, first, second, x, facing| Seat {
        second: Some(second),
        ..armed(team, first, x, facing, &[])
    };
    Setup {
        map: MapRef::Hills {
            width: 2400,
            height: 900,
            ground: 780,
            amplitude: 90,
            period: 700,
        },
        seats: vec![
            pair(Team::A, MobileKind::Tinbadger, MobileKind::Lensowl, 350, Facing::Right),
            pair(Team::B, MobileKind::Kitewyrm, MobileKind::Coilback, 2050, Facing::Left),
            pair(Team::A, MobileKind::Hookclaw, MobileKind::Frostfin, 900, Facing::Right),
            pair(Team::B, MobileKind::Lanternwing, MobileKind::Needlemouse, 1500, Facing::Left),
        ],
        order: vec![0, 1, 2, 3],
        wind: Wind::new(5, 60).unwrap(),
        sudden_death: None,
        mode: Mode::Tag,
    }
}

/// A Jewel 2v2 on hills in wind: jewels fall and are taken, no one is hurt, and a team reaches 100.
fn mode_jewel() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2400,
            height: 900,
            ground: 780,
            amplitude: 90,
            period: 700,
        },
        seats: vec![
            armed(Team::A, MobileKind::Coilback, 350, Facing::Right, &[Item::Dual, Item::Sacrifice]),
            armed(Team::B, MobileKind::Tinbadger, 2050, Facing::Left, &[Item::Dual]),
            armed(Team::A, MobileKind::Augerdillo, 900, Facing::Right, &[]),
            armed(Team::B, MobileKind::Flarecrow, 1500, Facing::Left, &[]),
        ],
        order: vec![0, 1, 2, 3],
        wind: Wind::new(5, 60).unwrap(),
        sudden_death: None,
        mode: Mode::Jewel,
    }
}

/// Kitewyrm against Coilback on hills in wind: its S1, S2 and SS each fire.
fn roster_kitewyrm() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Kitewyrm, 400, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(9, 150).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Flarecrow (Aduka's behavior) against Coilback on hills in wind, its back to the target: its S1,
/// S2 and SS each fire, and its flares call Skyeye.
fn roster_flarecrow() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Flarecrow, 400, Facing::Left, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Augerdillo (Nak's behavior) against Coilback on hills in wind: its S1, its burrowing S2 and its
/// SS through the hills. The ground runs deep, so a bore can come up.
fn roster_augerdillo() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 1100,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Augerdillo, 650, Facing::Left, &[]), seat(Team::B, 1350, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Lensowl (A.Sate's behavior) against Coilback on hills in wind: its S1, S2 and SS each fire.
fn roster_lensowl() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Lensowl, 400, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Needlemouse (Knight's behavior) against Coilback on hills in wind: its S1, S2 and SS each fire.
fn roster_needlemouse() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Needlemouse, 400, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Tinbadger against Coilback on hills in wind: its S1, S2 and SS each fire, and its mines stay on
/// the map from turn to turn.
fn roster_tinbadger() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 80,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Tinbadger, 420, Facing::Right, &[]), seat(Team::B, 1400, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Lodestoad against Coilback on hills in wind, from a crest (from a slope, its low aims hit the
/// next hill) at a Coilback on the far slope: its S1, S2 and SS each fire, and the magnet and the
/// pyramid slide the Coilback along the ground.
fn roster_lodestoad() -> Setup {
    Setup {
        map: MapRef::Hills {
            width: 2000,
            height: 900,
            ground: 760,
            amplitude: 120,
            period: 560,
        },
        seats: vec![armed(Team::A, MobileKind::Lodestoad, 280, Facing::Right, &[]), seat(Team::B, 1300, Facing::Left)],
        order: vec![0, 1],
        wind: Wind::new(7, 140).unwrap(),
        sudden_death: None,
        mode: Mode::Solo,
    }
}

/// Plays a game with a simple scripted player and returns its log. The dead spin their slot
/// machines whenever they may, or drop back in where they spawned. Every third turn the wind changes (if `winds` has any); every
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
            if let Some(dead) = (0..MAX_SEATS).filter_map(|s| u8::try_from(s).ok()).find(|&s| game.can_respawn(s)) {
                let x = setup.seats[usize::from(dead)].spawn_x;
                apply(&mut game, Event::Respawn { seat: dead, x });
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
                Roll::Jewel { width } => {
                    let kind = JewelKind::ALL[rolls % JewelKind::ALL.len()];
                    let x = i32::try_from((rolls * 389) % usize::try_from(width).unwrap()).unwrap();
                    Event::Jewel(JewelDrop { kind, x })
                }
            };
            apply(&mut game, event);
        }
        let Phase::Turn { seat } = game.phase() else { break };
        // Tag: every third turn, the mobile sitting out comes in.
        if turn % 3 == 1 && game.can_switch(seat) {
            apply(&mut game, Event::Switch { seat });
        }
        let me = game.players()[usize::from(seat)];
        let enemy = game
            .players()
            .iter()
            .filter(|p| p.alive && p.team != me.team)
            .min_by(|a, b| (a.footing.x - me.footing.x).abs().cmp(&(b.footing.x - me.footing.x).abs()))
            .copied()
            .unwrap();
        // In Jewel it shoots the nearest jewel worth points, while there is one.
        let jewel = game
            .jewels()
            .iter()
            .filter(|jewel| jewel.kind.points() > 0)
            .min_by_key(|jewel| (jewel.at.x - me.footing.x).abs())
            .copied();
        let target_x = jewel.map_or(enemy.footing.x, |jewel| jewel.at.x);
        let toward = if target_x > me.footing.x { Facing::Right } else { Facing::Left };
        let steps = if turn % 4 == 1 { 25 } else { 0 };
        apply(&mut game, Event::Walk { seat, facing: toward, steps });
        if !matches!(game.phase(), Phase::Turn { seat: s } if s == seat) {
            continue;
        }
        // A mobile that fires over its back turns it to the enemy.
        if me.tuning().s1.aim.value.backward() {
            apply(&mut game, Event::Walk { seat, facing: toward.flip(), steps: 0 });
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
        let center = jewel.map_or_else(|| game.players()[enemy_index].center(), |jewel| jewel.middle());
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.fly(&game.launch(seat, shot, aim, power.bars())?, 0);
                (impact.contact != Contact::Gone).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Power::MAX, |(_, power)| power);
        apply(&mut game, Event::Shoot { seat, shot, aim, power, time });
    }
    Log { setup, events }
}

#[test]
fn mode_tag_switches_and_a_player_is_out_when_its_mobile_in_play_falls() {
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "mode_tag").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut switched, mut out_with_a_mobile_left) = (Vec::new(), 0);
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Switched { mobile, .. } => switched.push(mobile),
                Effect::Died { seat, .. } if game.bench(seat).is_some_and(|bench| bench.hp > 0) => out_with_a_mobile_left += 1,
                _ => {}
            }
        }
    }
    let seconds: Vec<MobileKind> = log.setup.seats.iter().filter_map(|seat| seat.second).collect();
    assert!(seconds.iter().filter(|mobile| switched.contains(mobile)).count() >= 3, "switched in: {switched:?}");
    assert!(out_with_a_mobile_left >= 2, "{out_with_a_mobile_left} out with a mobile left");
    assert!(matches!(game.phase(), Phase::Over { winner: Some(_) }), "{:?}", game.phase());
}

#[test]
fn mode_score_spends_lives_and_drops_the_dead_back_in() {
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "mode_score").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut points, mut picked, mut landed) = (Vec::new(), 0, 0);
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::Points { team, points: left } => points.push((team, left)),
                Effect::RespawnPicked { .. } => picked += 1,
                Effect::Respawned { to: Some(_), .. } => landed += 1,
                _ => {}
            }
        }
    }
    assert!(landed >= 1, "{picked} picks, {landed} landed");
    let Phase::Over { winner: Some(winner) } = game.phase() else { panic!("{:?}", game.phase()) };
    let loser = if winner == Team::A { Team::B } else { Team::A };
    assert_eq!(game.points(loser), Some(0), "{points:?}");
    assert!(game.points(winner).unwrap() > 0);
}

#[test]
fn mode_jewel_takes_jewels_hurts_no_one_and_plays_to_its_points() {
    let (_, json, ..) = GOLDEN.iter().find(|(name, ..)| *name == "mode_jewel").unwrap();
    let log = Log::from_json(json).unwrap();
    let mut game = Game::new(&log.setup, &NoAssets).unwrap();
    let (mut dropped, mut taken, mut hurt) = (0, 0, 0);
    for event in &log.events {
        for effect in game.apply(*event).unwrap() {
            match effect {
                Effect::JewelDropped { to: Some(_), .. } => dropped += 1,
                Effect::JewelTaken { .. } => taken += 1,
                Effect::Damaged { .. } | Effect::Shielded { .. } | Effect::ShieldStripped { .. } => hurt += 1,
                _ => {}
            }
        }
    }
    assert_eq!(hurt, 0, "no one is hurt in Jewel");
    assert!(dropped > taken && taken >= 5, "{dropped} jewels landed, {taken} taken");
    let Phase::Over { winner: Some(winner) } = game.phase() else { panic!("{:?}", game.phase()) };
    assert!(game.points(winner).unwrap() >= lobshot_sim::tuning::jewels::TARGET_POINTS.value, "{:?}", game.points(winner));
}
