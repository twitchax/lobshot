//! Bolts of lightning: straight lines from the sky (or a side of the map) to the first mobile or
//! ground in their way, and what they do there. Sparkhorn's shots, the Thunder item and the slot
//! machine's bolt share them. Land above a mobile blocks a bolt meant for it (DB-mob).

use crate::{
    damage::{self, Blast},
    game::{Contact, Effect, Game, MAX_SEATS},
    tuning::SKY,
    units::Vec2,
};

/// Where a bolt ran: from its start to what it struck.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Strike {
    pub from: Vec2,
    pub at: Vec2,
    /// A mobile, the ground, or `Gone` if it left the map's sides or bottom first.
    pub contact: Contact,
}

/// Runs a bolt from `from` along `direction` (a unit vector), a pixel at a time, to the first
/// living mobile or solid pixel in its way.
pub(crate) fn strike(game: &Game, from: Vec2, direction: Vec2) -> Strike {
    let terrain = game.terrain();
    let (width, height) = (f64::from(terrain.width()), f64::from(terrain.height()));
    let frames = game.hit_frames();
    // No bolt is longer than the sky plus the map's width and height.
    let longest = terrain.width() + terrain.height() + 2 * terrain.height().max(1);
    for step in 0..=longest {
        let at = from + direction * f64::from(step);
        if at.x < 0.0 || at.x >= width || at.y >= height {
            return Strike { from, at, contact: Contact::Gone };
        }
        if let Some(&(seat, _)) = frames.iter().find(|(_, frame)| frame.contains(at)) {
            return Strike { from, at, contact: Contact::Mobile(seat) };
        }
        // Inside the map's sides and above its bottom, so the pixel is in i32 range.
        #[allow(clippy::cast_possible_truncation)]
        if terrain.is_solid(at.x.floor() as i32, at.y.floor() as i32) {
            return Strike { from, at, contact: Contact::Terrain };
        }
    }
    let at = from + direction * f64::from(longest);
    Strike { from, at, contact: Contact::Gone }
}

/// A bolt straight down column `x` from the sky.
pub(crate) fn from_sky(game: &Game, x: f64) -> Strike {
    strike(game, Vec2::new(x, -SKY.value), Vec2::new(0.0, 1.0))
}

/// A bolt heading along `direction` (a unit vector pointing down) whose line passes through
/// `through`: it starts where that line leaves the sky, or a side of the map, behind it.
pub(crate) fn toward(game: &Game, through: Vec2, direction: Vec2) -> Strike {
    let width = f64::from(game.terrain().width());
    // How far back along the line to reach an edge `gap` away, closing at `rate` per unit.
    let back = |gap: f64, rate: f64| if rate > 0.0 { gap / rate } else { f64::MAX };
    let to_sky = back(through.y + SKY.value, direction.y);
    let to_side = if direction.x > 0.0 {
        back(through.x, direction.x)
    } else {
        back(width - 0.5 - through.x, -direction.x)
    };
    let length = to_sky.min(to_side).max(0.0);
    strike(game, through - direction * length, direction)
}

/// Shows a strike at `tick` (`Bolt`) and, where it struck the ground, carves a crater of `crater`.
pub(crate) fn flash(game: &mut Game, strike: &Strike, crater: f64, tick: u32, effects: &mut Vec<Effect>) {
    effects.push(Effect::Bolt { from: strike.from, to: strike.at, tick });
    if strike.contact == Contact::Terrain {
        let dirty = game.terrain.carve(strike.at, crater);
        effects.push(Effect::Exploded {
            at: strike.at,
            radius: crater,
            tick,
            dirty,
        });
    }
}

/// The point on the strike's path nearest `point`.
pub(crate) fn nearest(strike: &Strike, point: Vec2) -> Vec2 {
    let path = strike.at - strike.from;
    let length_squared = path.x * path.x + path.y * path.y;
    if length_squared <= 0.0 {
        return strike.at;
    }
    let offset = point - strike.from;
    let along = ((offset.x * path.x + offset.y * path.y) / length_squared).clamp(0.0, 1.0);
    strike.from + path * along
}

/// Hurts every living mobile the strike struck or passed near, by `blast`'s damage model: the
/// point of its path nearest each mobile's center stands in for the blast's center, since a bolt
/// runs the whole way down rather than bursting at one point.
pub(crate) fn zap(game: &mut Game, strike: &Strike, blast: &Blast, effects: &mut Vec<Effect>) {
    for seat in 0..game.players().len() {
        let player = game.players()[seat];
        let Ok(seat) = u8::try_from(seat) else { continue };
        if !player.alive {
            continue;
        }
        let blast = Blast {
            center: nearest(strike, player.center()),
            ..*blast
        };
        game.hurt(seat, damage::damage(&blast, &player.target(), strike.contact == Contact::Mobile(seat)), effects);
    }
}

/// Splash: a flat `amount` to each living mobile within `reach` of the strike's path that splash
/// hasn't hit yet this turn (`splashed`). It ignores armor, angle and boosts (GF-max).
pub(crate) fn splash(game: &mut Game, splashed: &mut [bool; MAX_SEATS], strike: &Strike, amount: i32, reach: f64, effects: &mut Vec<Effect>) {
    for (seat, hit) in splashed.iter_mut().enumerate().take(game.players().len()) {
        let player = game.players()[seat];
        let Ok(seat) = u8::try_from(seat) else { continue };
        let near = player.pose().gap(player.tuning().hitbox.value, nearest(strike, player.center())) <= reach;
        if player.alive && !*hit && near {
            *hit = true;
            game.hurt(seat, amount, effects);
        }
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            MobileKind, Team,
            testing::{damage_to, duel, game_on, seat},
        },
        movement::Footing,
        terrain::Terrain,
        units::Facing,
    };

    #[test]
    fn a_bolt_from_the_sky_strikes_the_first_mobile_in_its_column() {
        let game = duel(800);
        let x = game.players()[1].center().x;
        let strike = from_sky(&game, x);
        assert_eq!(strike.contact, Contact::Mobile(1));
        assert!(strike.at.y < game.players()[1].center().y, "it strikes from above: {strike:?}");
        assert_eq!(from_sky(&game, 1500.0).contact, Contact::Terrain, "open ground");
    }

    #[test]
    fn land_above_a_mobile_blocks_its_bolt() {
        // A roof at rows 300..310 over x 1000..1200, and the ground at 500.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((1000..1200).contains(&x) && (300..310).contains(&y))).unwrap();
        let mut game = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Coilback, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        );
        // Spawns drop from the top onto the roof; stand it under the roof instead.
        game.place(1, Footing { x: 1100, y: 500 });
        let strike = from_sky(&game, game.players()[1].center().x);
        assert_eq!(strike.contact, Contact::Terrain);
        assert!((strike.at.y - 300.0).abs() < 1.0, "{strike:?}");
        let mut splashed = [false; MAX_SEATS];
        let mut effects = Vec::new();
        splash(&mut game, &mut splashed, &strike, 76, 25.0, &mut effects);
        assert_eq!(damage_to(&effects, 1), 0, "the roof is far above it");
    }

    #[test]
    fn a_bolt_toward_a_point_comes_down_its_line_from_the_sky() {
        let game = duel(800);
        let target = Vec2::new(1000.0, 499.0);
        let down_right = Vec2::new(core::f64::consts::FRAC_1_SQRT_2, core::f64::consts::FRAC_1_SQRT_2);
        let strike = toward(&game, target, down_right);
        assert!((strike.from.y + 100.0).abs() < 1e-6 && strike.from.x < target.x, "from the upper left: {strike:?}");
        assert_eq!(strike.contact, Contact::Terrain);
        assert!(strike.at.distance(target) < 2.0, "{strike:?}");
    }

    #[test]
    fn splash_lands_once_a_turn_on_each_mobile() {
        let mut game = duel(800);
        let strike = from_sky(&game, game.players()[1].center().x);
        let mut splashed = [false; MAX_SEATS];
        let mut first = Vec::new();
        splash(&mut game, &mut splashed, &strike, 76, 25.0, &mut first);
        let mut second = Vec::new();
        splash(&mut game, &mut splashed, &strike, 76, 25.0, &mut second);
        assert_eq!((damage_to(&first, 1), damage_to(&second, 1)), (76, 0));
    }
}
