//! A mobile's own satellite (Lensowl's moon, Needlemouse's pincushion): it hovers straight over its
//! mobile wherever it goes, at a height that depends on the shot picked (`MobileKind::satellite`),
//! and strikes where the mobile's tracer lands, with lasers (`laser`) or pins (`pins`). The tracer
//! is a projectile that hurts and carves nothing (WP06-list, GF-lee); the strikes come from
//! wherever the satellite is, so whatever stands between it and the tracer takes them (SW-mob,
//! SNX-asate, SNX-knight). It never touches the ground or a mobile itself: it can sit
//! inside a hill (GF-asate), and nothing hits it. Every satellite strike that lands is a landing
//! like a projectile's: Thunder, Electricity and Skyeye follow each one (GF-asate: "You still get 6
//! thor lasers").

use super::{Gun, beam, land_in_order};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Senses, Shot, Track},
    tuning::{DamageRange, column_width},
    units::Vec2,
    weather::Marks,
};

/// Where `seat`'s satellite hovers while `shot` is picked, over its feet; `None` for a mobile
/// without one.
pub(crate) fn home(game: &Game, seat: u8, shot: Shot) -> Option<Vec2> {
    let player = game.players().get(usize::from(seat))?;
    player.mobile.satellite(player.footing.world(), shot)
}

/// Where an S1 aims, as A.Sate's laser does: `high` px above `guide` when the satellite at `from`
/// is right of it, so it fires leftward, and at it otherwise. "When shooting from the right side of
/// the opponent, the laser tends to shoot about half a centimeter above the guide" (SW06-mob).
pub(crate) fn sighted(from: Vec2, guide: Vec2, high: f64) -> Vec2 {
    if from.x > guide.x { guide - Vec2::new(0.0, high) } else { guide }
}

/// The satellite glides to `at`, leaving at stage tick `from` and arriving at `to`.
pub(crate) fn glide(gun: &Gun, at: Vec2, from: u32, to: u32, effects: &mut Vec<Effect>) {
    effects.push(Effect::Satellite { seat: gun.seat, at, from, to });
}

/// Fires the volley's tracer and shows its path. Returns its track if it landed on the ground or a
/// mobile; `None` if it left the map.
pub(crate) fn tracer(game: &Game, gun: &Gun, effects: &mut Vec<Effect>) -> Option<Track> {
    let flight = gun.launch(game, 0, 0.0)?;
    let track = game.track(flight, 0, 0);
    effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
    matches!(track.impact.contact, Contact::Terrain | Contact::Mobile(_)).then_some(track)
}

/// What a satellite's strike does where it lands: a blast this wide (its crater too) dealing this
/// much, from the volley's weapon.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hit {
    pub radius: f64,
    pub damage: DamageRange,
}

/// A laser from the satellite at `from` through `through` at `tick`: `hit` where it strikes, then
/// what follows a landing (`Gun::follow_up`). A beam meets the weather columns it crosses: Force,
/// Drain and Electricity mark it, as they would a projectile, but nothing bends it.
pub(crate) fn laser(game: &mut Game, gun: &Gun, from: Vec2, through: Vec2, tick: u32, hit: Hit, effects: &mut Vec<Effect>) {
    let Hit { radius, damage } = hit;
    let Some(strike) = beam::strike(game, from, through) else { return };
    effects.push(Effect::Laser {
        seat: gun.seat,
        from: strike.from,
        to: strike.at,
        tick,
    });
    if !matches!(strike.contact, Contact::Terrain | Contact::Mobile(_)) {
        return;
    }
    let marks = crossed(game, strike.from, strike.at);
    let blast = gun.blast(strike.at, radius, damage, marks.boost());
    beam::burn(game, &strike, &blast, radius * gun.salvo.crater, tick, effects);
    gun.follow_up(game, strike.at, tick, marks, effects);
}

/// The marks of the weather columns a straight line from `from` to `to` crosses: they stand the
/// map's whole height, so it crosses one whose span its own overlaps. A Random column acts as what
/// it hides.
fn crossed(game: &Game, from: Vec2, to: Vec2) -> Marks {
    let (left, right) = (from.x.min(to.x), from.x.max(to.x));
    let mut marks = Marks::default();
    for column in &game.weather().columns {
        let half = column_width(column.kind) / 2.0;
        let x = f64::from(column.x);
        if left < x + half && right >= x - half {
            marks.mark(column.acts_as());
        }
    }
    marks
}

/// How a satellite's pins fly (Needlemouse's): straight, at a steady speed, untouched by gravity
/// and wind; one still flying `life` ticks after it leaves is gone.
pub(crate) struct Pins {
    pub speed: f64,
    pub life: u32,
    pub hit: Hit,
    /// The crater each carves, narrower than its reach.
    pub crater: f64,
}

/// A pin's flight from `from` straight at `target`; `None` if they are the same point.
fn pin(from: Vec2, target: Vec2, speed: f64) -> Option<Flight> {
    let path = target - from;
    let length = path.length();
    (length > 0.0).then(|| Flight {
        origin: from,
        velocity: path * (speed / length),
        accel: Vec2::ZERO,
    })
}

/// Pins leaving the satellite, each from its point at its stage tick (`launches`), flying straight
/// at `target`. Each lands like any projectile (`Gun::land`), in the order they strike
/// (`land_in_order`), meeting the weather on the way.
pub(crate) fn pins(game: &mut Game, gun: &Gun, launches: &[(Vec2, u32)], target: Vec2, pins: &Pins, effects: &mut Vec<Effect>) {
    let flights: Vec<(Flight, u32)> = launches.iter().filter_map(|&(from, tick)| Some((pin(from, target, pins.speed)?, tick))).collect();
    let (paths, mut landings) = land_in_order(
        game,
        flights.len(),
        |game, index| {
            let (flight, tick) = flights[index];
            game.track_until(flight, 0, tick, Senses::ALL, tick + pins.life).track
        },
        |game, _, track, out| gun.land(game, &gun.blast_on(track, pins.hit.radius, pins.hit.damage), pins.crater, &track.impact, track.marks, out),
    );
    effects.extend(paths.into_iter().map(|legs| Effect::Fired { seat: gun.seat, legs }));
    effects.append(&mut landings);
}

/// Where a pin from `from` flying at `target` stops, for SearchBot's aim pass.
pub(crate) fn pin_strike(game: &Game, from: Vec2, target: Vec2, speed: f64) -> Option<Impact> {
    Some(game.track(pin(from, target, speed)?, 0, 0).impact)
}
