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
    game::{Contact, Event, Game, MobileKind, Phase, Roll, Seat, Setup, Shot, Team},
    items::Loadout,
    log::Log,
    map::{MapRef, NoAssets},
    units::{Centis, Facing, Power, Wind},
    weather::{Forecast, WeatherKind},
};

/// Name, JSON, wire bytes, event count, final state hash.
const GOLDEN: &[(&str, &str, &[u8], usize, u64)] = &[
    ("duel_flat", include_str!("golden/duel_flat.json"), include_bytes!("golden/duel_flat.bin"), 32, 0x90f0_7e89_eb25_26b4),
    (
        "duel_hills_wind",
        include_str!("golden/duel_hills_wind.json"),
        include_bytes!("golden/duel_hills_wind.bin"),
        35,
        0x895b_61f8_58d8_c04c,
    ),
    ("brawl_2v2", include_str!("golden/brawl_2v2.json"), include_bytes!("golden/brawl_2v2.bin"), 87, 0xc357_cdeb_7284_e5c5),
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
    Seat {
        team,
        mobile: MobileKind::Coilback,
        spawn_x,
        facing,
        items: Loadout::default(),
    }
}

fn scenarios() -> Vec<(&'static str, Log)> {
    let winds = [Wind::new(6, 30).unwrap(), Wind::new(14, 200).unwrap(), Wind::new(3, 270).unwrap(), Wind::new(20, 95).unwrap()];
    vec![
        (
            "duel_flat",
            play(
                Setup {
                    map: MapRef::Flat { width: 1600, height: 700, ground: 560 },
                    seats: vec![seat(Team::A, 350, Facing::Right), seat(Team::B, 1150, Facing::Left)],
                    order: vec![0, 1],
                    wind: Wind::default(),
                    sudden_death: None,
                },
                &[],
            ),
        ),
        (
            "duel_hills_wind",
            play(
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
                },
                &winds,
            ),
        ),
        (
            "brawl_2v2",
            play(
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
                },
                &winds,
            ),
        ),
    ]
}

/// Plays a game with a simple scripted player and returns its log. Every third turn the wind
/// changes (if `winds` has any); every fourth, the player walks toward the nearest enemy; every
/// seventh, it times out. Otherwise it cycles S1, S2, SS at the middle of the shot's true range,
/// with the power step that lands closest to the enemy.
fn play(setup: Setup, winds: &[Wind]) -> Log {
    let mut game = Game::new(&setup, &NoAssets).unwrap();
    let mut events = Vec::new();
    let mut apply = |game: &mut Game, event: Event| {
        game.apply(event).unwrap();
        events.push(event);
    };

    let mut rolls = 0_usize;
    for turn in 0..60_usize {
        let Phase::Turn { seat } = game.phase() else { break };
        // Answer the server's rolls from fixed cycles: weather kinds and columns across the map,
        // and the given winds (or calm). The calm duel gets only Force far off to the side.
        while let Some(roll) = game.pending_roll() {
            rolls += 1;
            let event = match roll {
                Roll::Weather { .. } if winds.is_empty() => Event::Weather(Forecast { kind: WeatherKind::Force, x: 1 }),
                Roll::Weather { width } => {
                    let kind = WeatherKind::ALL[rolls % WeatherKind::ALL.len()];
                    let x = i32::try_from((rolls * 389) % usize::try_from(width).unwrap()).unwrap();
                    Event::Weather(Forecast { kind, x })
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
        let time = Centis(u32::try_from(300 + turn * 137 % 1500).unwrap());
        if turn % 7 == 6 {
            apply(&mut game, Event::Pass { seat, time: Centis(2000) });
            continue;
        }

        let shot = [Shot::S1, Shot::S2, Shot::Ss][turn % 3];
        let range = shot.tuning(me.tuning()).aim.value;
        let aim = i16::midpoint(range.true_min, range.true_max);
        let enemy_index = game.players().iter().position(|p| *p == enemy).unwrap();
        let center = game.players()[enemy_index].pose().center(enemy.tuning().hitbox.value);
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
