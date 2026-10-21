//! Battle items, GunBound's eleven under plain names, and the loadout a player brings into a game:
//! six slots, each item filling one or two (SW-gp). An item is used once, on its owner's turn, one
//! a turn, and never in sudden death (`Game::can_use_item`). Heals, Wind Flip, Sacrifice's cost
//! and Swap act at once (`use_item`, here); the rest ride on the turn's shot (`shots::Salvo`). The
//! numbers live in `tuning::items`.

use anyhow::ensure;
use serde::{Deserialize, Serialize};

use crate::{
    base::Res,
    game::{Effect, Game},
    tuning::{
        ArmorClass,
        items::{self, BIG_HEAL_SHARE, BIONIC_HEAL_EXTRA, HEAL_SHARE, ItemTuning, LOADOUT_SLOTS, SACRIFICE_COST},
    },
};

/// The most items a loadout holds: every item fills at least one slot.
pub const MAX_ITEMS: usize = 6;

/// A battle item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Item {
    /// The selected shot (S1 or S2, never SS) fires twice.
    Dual,
    /// The selected shot, then the other of S1 and S2.
    DualPlus,
    /// The shot hurts no one and carries its shooter to where it lands.
    Teleport,
    /// This turn's shot hits harder.
    PowerUp,
    /// Pay some HP for a harder-hitting shot this turn.
    Sacrifice,
    /// This turn's shot carves bigger craters.
    Dig,
    /// A small heal.
    Heal,
    /// A big heal.
    BigHeal,
    /// The wind turns around.
    WindFlip,
    /// A bolt strikes wherever this turn's shot lands.
    Thunder,
    /// Trade places with your weakest living teammate, ending the turn.
    Swap,
}

impl Item {
    /// Every item, in the order the loadout screen lists them.
    pub const ALL: [Self; 11] = [
        Self::Dual,
        Self::DualPlus,
        Self::Teleport,
        Self::PowerUp,
        Self::Sacrifice,
        Self::Dig,
        Self::Heal,
        Self::BigHeal,
        Self::WindFlip,
        Self::Thunder,
        Self::Swap,
    ];

    /// The player-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dual => "Dual",
            Self::DualPlus => "Dual+",
            Self::Teleport => "Teleport",
            Self::PowerUp => "Power Up",
            Self::Sacrifice => "Sacrifice",
            Self::Dig => "Dig",
            Self::Heal => "Heal",
            Self::BigHeal => "Big Heal",
            Self::WindFlip => "Wind Flip",
            Self::Thunder => "Thunder",
            Self::Swap => "Swap",
        }
    }

    #[must_use]
    pub const fn tuning(self) -> &'static ItemTuning {
        match self {
            Self::Dual => &items::DUAL,
            Self::DualPlus => &items::DUAL_PLUS,
            Self::Teleport => &items::TELEPORT,
            Self::PowerUp => &items::POWER_UP,
            Self::Sacrifice => &items::SACRIFICE,
            Self::Dig => &items::DIG,
            Self::Heal => &items::HEAL,
            Self::BigHeal => &items::BIG_HEAL,
            Self::WindFlip => &items::WIND_FLIP,
            Self::Thunder => &items::THUNDER,
            Self::Swap => &items::SWAP,
        }
    }

    /// How many loadout slots it fills.
    #[must_use]
    pub const fn slots(self) -> u8 {
        self.tuning().slots.value
    }
}

/// The items a player brings into a game, in slot order, filling at most `LOADOUT_SLOTS` slots.
/// Checked whenever one is built or read, so a loadout from the wire or a log is always legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<Item>", into = "Vec<Item>")]
pub struct Loadout([Option<Item>; MAX_ITEMS]);

impl Loadout {
    /// # Errors
    ///
    /// Fails when the items fill more than `LOADOUT_SLOTS` slots.
    pub fn new(items: &[Item]) -> Res<Self> {
        let slots: u32 = items.iter().map(|item| u32::from(item.slots())).sum();
        ensure!(slots <= u32::from(LOADOUT_SLOTS.value), "a loadout fills at most {} slots, not {slots}", LOADOUT_SLOTS.value);
        let mut picked = [None; MAX_ITEMS];
        for (slot, &item) in picked.iter_mut().zip(items) {
            *slot = Some(item);
        }
        Ok(Self(picked))
    }

    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.0.iter().flatten().copied()
    }

    /// How many slots the items fill.
    #[must_use]
    pub fn slots(&self) -> u8 {
        self.items().map(Item::slots).sum()
    }

    /// The items by position, as a player's unused items start out.
    #[must_use]
    pub fn positions(&self) -> [Option<Item>; MAX_ITEMS] {
        self.0
    }
}

/// Spends `seat`'s item at `slot` and does what it does at once; the caller has checked it may
/// (`Game::can_use_item`). Returns whether it ended the turn (Swap).
pub(crate) fn use_item(game: &mut Game, seat: u8, slot: u8, effects: &mut Vec<Effect>) -> bool {
    let index = usize::from(seat);
    let Some(item) = game.players[index].items.get_mut(usize::from(slot)).and_then(Option::take) else {
        return false;
    };
    game.players[index].armed = Some(item);
    effects.push(Effect::ItemUsed { seat, slot, item });
    let player = game.players[index];
    let max_hp = player.tuning().hp.value;
    // Bionic mobiles gain more from healing (SW-mobC, SW-gp).
    let bionic = if player.tuning().armor.value == ArmorClass::Bionic { BIONIC_HEAL_EXTRA.value } else { 0.0 };
    match item {
        Item::Heal => game.heal(seat, HEAL_SHARE.value + bionic, effects),
        Item::BigHeal => game.heal(seat, BIG_HEAL_SHARE.value + bionic, effects),
        Item::WindFlip => {
            game.wind.direction = (game.wind.direction + 180) % 360;
            effects.push(Effect::WindChanged(game.wind));
        }
        Item::Sacrifice => {
            // A share of at most 1 of a few thousand HP: far inside i32.
            #[allow(clippy::cast_possible_truncation)]
            let cost = ((f64::from(max_hp) * SACRIFICE_COST.value).floor() as i32).min(player.hp - 1);
            if cost > 0 {
                game.players[index].hp -= cost;
                effects.push(Effect::Damaged { seat, amount: cost });
            }
        }
        Item::Swap => {
            let Some(with) = game.swap_partner(seat) else { return false };
            let theirs = game.players[usize::from(with)].footing;
            game.place(seat, theirs);
            game.place(with, player.footing);
            effects.push(Effect::Swapped { seat, with });
            return true;
        }
        Item::Dual | Item::DualPlus | Item::Teleport | Item::PowerUp | Item::Dig | Item::Thunder => {}
    }
    false
}

impl TryFrom<Vec<Item>> for Loadout {
    type Error = anyhow::Error;

    fn try_from(items: Vec<Item>) -> Res<Self> {
        Self::new(&items)
    }
}

impl From<Loadout> for Vec<Item> {
    fn from(loadout: Loadout) -> Self {
        loadout.items().collect()
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_loadout_fills_six_slots_with_one_and_two_slot_items() {
        let full = Loadout::new(&[Item::Dual, Item::PowerUp, Item::Heal, Item::Dig, Item::WindFlip]).unwrap();
        assert_eq!(full.slots(), 6);
        assert_eq!(full.items().count(), 5);
        assert!(Loadout::new(&[Item::Dual, Item::Teleport, Item::Thunder]).is_ok(), "three big items");
        assert!(Loadout::new(&[Item::Dual, Item::Teleport, Item::Thunder, Item::Heal]).is_err(), "seven slots");
        assert!(Loadout::new(&[Item::Heal; 7]).is_err());
        assert_eq!(Loadout::default().slots(), 0);
    }

    #[test]
    fn a_loadout_off_the_wire_is_checked_too() {
        let loadout = Loadout::new(&[Item::Swap, Item::Sacrifice]).unwrap();
        let json = serde_json::to_string(&loadout).unwrap();
        assert_eq!(json, r#"["Swap","Sacrifice"]"#);
        assert_eq!(serde_json::from_str::<Loadout>(&json).unwrap(), loadout);
        assert_eq!(postcard::from_bytes::<Loadout>(&postcard::to_allocvec(&loadout).unwrap()).unwrap(), loadout);
        assert!(serde_json::from_str::<Loadout>(r#"["Dual","Dual","Dual","Heal"]"#).is_err());
    }

    #[test]
    fn every_item_fills_one_or_two_slots() {
        for item in Item::ALL {
            assert!((1..=2).contains(&item.slots()), "{item:?}");
        }
    }

    use crate::{
        game::{
            Effect, Event, Game, MobileKind, Phase, Shot, Team,
            testing::{answer_rolls, duel, duel_of, game_of, pass, pass_until, seat, shoot},
        },
        units::{Centis, Facing, Power, Wind},
    };

    /// `game` with seat 0 carrying `items`.
    fn carrying(mut game: Game, items: &[Item]) -> Game {
        game.players[0].items = Loadout::new(items).unwrap().positions();
        game
    }

    fn use_slot(game: &mut Game, slot: u8) -> Vec<Effect> {
        game.apply(Event::UseItem { seat: 0, slot }).unwrap()
    }

    #[test]
    fn an_item_is_used_once_one_a_turn_on_its_owners_turn() {
        let mut game = carrying(duel(800), &[Item::Heal, Item::WindFlip]);
        assert!(!game.can_use_item(1, 0), "not seat 1's turn");
        assert!(!game.can_use_item(0, 5), "an empty slot");
        let effects = use_slot(&mut game, 0);
        assert_eq!(effects[0], Effect::ItemUsed { seat: 0, slot: 0, item: Item::Heal });
        assert_eq!(game.players()[0].items[0], None, "spent");
        assert_eq!(game.players()[0].armed, Some(Item::Heal));
        assert!(!game.can_use_item(0, 1), "one a turn");
        assert!(game.apply(Event::UseItem { seat: 0, slot: 1 }).is_err());
        pass(&mut game, 0);
        assert_eq!(game.players()[0].armed, None, "the turn's item clears with the turn");
        // A pass with Heal: base 520 + pass 200 + Heal 100 + Armor's surcharge 40.
        assert_eq!(game.players()[0].clock.delay, 860);
    }

    #[test]
    fn heals_restore_a_share_of_max_hp_up_to_it_and_more_for_bionics() {
        let mut game = carrying(duel(800), &[Item::Heal, Item::BigHeal]);
        game.players[0].hp = 500;
        assert!(use_slot(&mut game, 0).contains(&Effect::Healed { seat: 0, amount: 120 }));
        pass(&mut game, 0);
        pass_until(&mut game, 0);
        game.players[0].hp = 900;
        assert!(use_slot(&mut game, 1).contains(&Effect::Healed { seat: 0, amount: 100 }), "capped at max");
        assert_eq!(game.players()[0].hp, 1000);

        // Hookclaw is Bionic: Big Heal's 25% becomes 30%.
        let mut game = carrying(duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 800), &[Item::BigHeal]);
        game.players[0].hp = 100;
        assert!(use_slot(&mut game, 0).contains(&Effect::Healed { seat: 0, amount: 300 }));
    }

    #[test]
    fn wind_flip_turns_the_wind_around() {
        let mut game = carrying(duel(800), &[Item::WindFlip]);
        game.wind = Wind::new(14, 30).unwrap();
        let flipped = Wind::new(14, 210).unwrap();
        assert!(use_slot(&mut game, 0).contains(&Effect::WindChanged(flipped)));
        assert_eq!(game.wind(), flipped);
    }

    #[test]
    fn sacrifice_costs_eight_percent_but_never_the_last_hp() {
        let mut game = carrying(duel(800), &[Item::Sacrifice, Item::Sacrifice]);
        assert!(use_slot(&mut game, 0).contains(&Effect::Damaged { seat: 0, amount: 80 }));
        assert_eq!(game.players()[0].hp, 920);
        pass(&mut game, 0);
        pass_until(&mut game, 0);
        game.players[0].hp = 30;
        use_slot(&mut game, 1);
        assert_eq!(game.players()[0].hp, 1, "down to 1, no lower");
    }

    #[test]
    fn swap_trades_places_with_the_weakest_teammate_and_ends_the_turn() {
        let mut game = game_of(vec![
            seat(Team::A, MobileKind::Coilback, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1100, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 700, Facing::Right),
            seat(Team::A, MobileKind::Coilback, 1500, Facing::Right),
        ]);
        game.players[0].items = Loadout::new(&[Item::Swap]).unwrap().positions();
        game.players[3].hp = 400;
        let (mine, theirs) = (game.players()[0].footing, game.players()[3].footing);
        let effects = use_slot(&mut game, 0);
        assert!(effects.contains(&Effect::Swapped { seat: 0, with: 3 }), "{effects:?}");
        assert_eq!((game.players()[0].footing, game.players()[3].footing), (theirs, mine));
        assert_eq!(game.phase(), Phase::Turn { seat: 1 }, "the turn ends");
        assert_eq!(game.players()[0].clock.delay, 520 + 50 + 40, "base, Swap and Armor's surcharge: no shot or pass cost");

        let game = carrying(duel(800), &[Item::Swap]);
        assert!(!game.can_use_item(0, 0), "no teammate to swap with");
    }

    #[test]
    fn items_that_ride_on_the_shot_wait_for_it() {
        for item in [Item::Dual, Item::DualPlus, Item::Teleport, Item::PowerUp, Item::Dig, Item::Thunder] {
            let mut game = carrying(duel(800), &[item]);
            let effects = use_slot(&mut game, 0);
            assert_eq!(effects, [Effect::ItemUsed { seat: 0, slot: 0, item }], "{item:?} does nothing yet");
            assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_ok());
            answer_rolls(&mut game);
        }
        let mut game = carrying(duel(800), &[Item::PowerUp]);
        use_slot(&mut game, 0);
        shoot(&mut game, 0, Shot::S1, 35, Power::nearest(2.4));
        assert_eq!(game.players()[0].clock.delay, 520 + 250 + 150 + 40);
    }
}
