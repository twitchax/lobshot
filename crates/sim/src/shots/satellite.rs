//! A mobile's own satellite (Lensowl's): it hovers straight over its mobile wherever it goes, at a
//! height that depends on the shot picked (`MobileKind::satellite`), and strikes where the mobile's
//! tracer lands. The tracer is a projectile that hurts and carves nothing (WP06-list, GF-lee);
//! the strikes come from wherever the satellite is, so whatever stands between it and the tracer
//! takes them (SW-mob, SNX-asate). It never touches the ground or a mobile itself: it can sit
//! inside a hill (GF-asate), and nothing hits it. Every satellite strike that lands is a landing
//! like a projectile's: Thunder, Electricity and Skyeye follow each one (GF-asate: "You still get 6
//! thor lasers").

use super::{Gun, beam};
use crate::{
    game::{Contact, Effect, Game, Shot, Track},
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
