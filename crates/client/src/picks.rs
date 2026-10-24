//! This player's picks: a mobile (or Random) and a loadout, remembered in `localStorage` like the
//! name and rider. The lobby and the garage edit them (`mobile_picker`, `loadout_editor`), practice
//! plays with them, and in a room they go to the server whenever they differ from what it has.

use bevy::{prelude::*, ui::InteractionDisabled};
use lobshot_assets::ART_SCALE;
use lobshot_protocol::{ClientMsg, Stage};
use lobshot_sim::{
    base::Res as SimRes,
    game::MobileKind,
    items::{Item, Loadout},
    tuning::items::LOADOUT_SLOTS,
};

use crate::{
    assets::{ImageCache, Rigs},
    model::{Screen, Session},
    net::Socket,
    ui::{UiAction, art, button_node, chosen, small_button, text, theme},
    web,
};

/// This player's mobile and loadout.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct Picks {
    /// `None` is Random: the server (or practice) picks when the game starts.
    pub mobile: Option<MobileKind>,
    pub loadout: Loadout,
}

impl Default for Picks {
    fn default() -> Self {
        Self {
            mobile: Some(MobileKind::Coilback),
            loadout: starter(),
        }
    }
}

/// What a new player brings: six slots of everyday items, so a first game has some to try.
fn starter() -> Loadout {
    Loadout::new(&[Item::Dual, Item::PowerUp, Item::Heal, Item::WindFlip, Item::Dig]).unwrap_or_default()
}

impl Picks {
    /// The remembered picks, or the defaults for anything not remembered (or unreadable).
    fn load() -> Self {
        let defaults = Self::default();
        Self {
            mobile: web::load("mobile").and_then(|name| parse_mobile(&name).ok()).unwrap_or(defaults.mobile),
            loadout: web::load("loadout").and_then(|json| lobshot_protocol::from_json(&json).ok()).unwrap_or(defaults.loadout),
        }
    }

    fn save(&self) {
        web::save("mobile", mobile_name(self.mobile));
        if let Ok(json) = lobshot_protocol::to_json(&self.loadout) {
            web::save("loadout", &json);
        }
    }

    pub fn pick(&mut self, mobile: Option<MobileKind>) {
        self.mobile = mobile;
        self.save();
    }

    /// # Errors
    ///
    /// Fails, saying why, when the item doesn't fit.
    pub fn add(&mut self, item: Item) -> SimRes<()> {
        self.loadout = self.loadout.with(item)?;
        self.save();
        Ok(())
    }

    pub fn remove(&mut self, position: u8) {
        self.loadout = self.loadout.without(position);
        self.save();
    }

    pub fn clear(&mut self) {
        self.loadout = Loadout::default();
        self.save();
    }
}

/// A mobile's player-facing name, or Random.
#[must_use]
pub fn mobile_name(mobile: Option<MobileKind>) -> &'static str {
    mobile.map_or("Random", |mobile| mobile.tuning().name)
}

/// The mobile a remembered name stands for (`None` is Random).
///
/// # Errors
///
/// Fails on a name that's neither a mobile nor Random.
pub fn parse_mobile(name: &str) -> SimRes<Option<MobileKind>> {
    if name == mobile_name(None) {
        return Ok(None);
    }
    let mobile = MobileKind::ALL.into_iter().find(|&mobile| mobile_name(Some(mobile)) == name);
    mobile.map(Some).ok_or_else(|| anyhow::anyhow!("no mobile is called {name}"))
}

/// What a mobile is good at, in a line.
#[must_use]
pub fn role(mobile: Option<MobileKind>) -> &'static str {
    match mobile {
        Some(MobileKind::Coilback) => "A sturdy mortar. Its SS swells in flight and hits hard.",
        Some(MobileKind::Lanternwing) => "A shielded laser moth. Quick bolts that dig well.",
        Some(MobileKind::Hookclaw) => "A boomerang crab. Wind bends its shots into hooks.",
        Some(MobileKind::Sparkhorn) => "A shielded beetle. Its shells call bolts from the sky.",
        Some(MobileKind::Marblemole) => "A mine-cart mole. Its marbles bounce and roll into pits.",
        Some(MobileKind::Spoutpot) => "A teapot hippo. Twin water streams that join in the air.",
        Some(MobileKind::Tuskrack) => "A walrus with a rocket rack. Its shots scatter and dig deep.",
        Some(MobileKind::Sudsotter) => "A bathtub otter. Its sponges walk the ground, then pop.",
        Some(MobileKind::Frostfin) => "A sledding narwhal. Its tusk's ice leaves foes brittle.",
        Some(MobileKind::Herdhound) => "A sheepdog mech. Its missiles loop, then home on its whistle.",
        Some(MobileKind::Thistleback) => "A hedgehog gardener. Lobs heavy cabbages; its SS digs deep.",
        Some(MobileKind::Kitewyrm) => "A kite dragon that floats. Breathes fireballs; its SS calls dragons.",
        Some(MobileKind::Augerdillo) => "A drilling armadillo. Fires over its back; its drills burrow.",
        None => "Any of them: the room picks when the game starts.",
    }
}

fn plural(count: u8, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

/// What an item costs: the slots it fills and the delay it adds.
fn costs(item: Item) -> (String, String) {
    let tuning = item.tuning();
    let slots = tuning.slots.value;
    (format!("{slots} {}", plural(slots, "slot", "slots")), format!("+{} delay", tuning.delay.value))
}

/// An item's icon, `height` tall and as wide as the slots it fills, labeled until it's drawn.
#[must_use]
pub fn item_art(server: &AssetServer, cache: &mut ImageCache, item: Item, height: f32) -> impl Bundle + use<> {
    let size = Vec2::new(height * f32::from(item.slots()), height);
    art(cache.get(server, lobshot_assets::item_path(item)), item.name(), size, (height / 3.2).clamp(9.0, 13.0))
}

/// A mobile's portrait, drawn from its rig's parts once they've all loaded.
#[derive(Component)]
pub struct Portrait(pub MobileKind);

#[derive(Component)]
struct Drawn;

/// Drawing portraits, which runs before the screens rebuild, so it never draws into a portrait
/// they're about to take down.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DrawPortraits;

/// One image in a portrait: a rig part, or the rider.
struct Layer {
    z: i32,
    image: Handle<Image>,
    /// Where its pivot (the image's center) sits in the body frame.
    at: (f64, f64),
    aim: bool,
}

/// A portrait's frame, in UI pixels: as wide as the longest name under it.
const PORTRAIT: Vec2 = Vec2::new(66.0, 54.0);
/// World pixels to portrait pixels: Coilback with its rider stands about 95 px tall.
const PORTRAIT_SCALE: f32 = 0.5;
/// The picker and editor's width: six mobile cards, or four item cards, side by side.
pub const PICKS_WIDTH: f32 = 442.0;
/// The cannon's aim in a portrait, in degrees above level.
const PORTRAIT_AIM: f32 = 35.0;

/// A portrait's cannon aim for `mobile`: `PORTRAIT_AIM` above level ahead, or over the back for a
/// mobile that fires that way (Augerdillo).
fn portrait_aim(mobile: MobileKind) -> f32 {
    if mobile.tuning().fires_rear() { 180.0 - PORTRAIT_AIM } else { PORTRAIT_AIM }
}

fn portrait_frame() -> impl Bundle {
    (
        Node {
            width: Val::Px(PORTRAIT.x),
            height: Val::Px(PORTRAIT.y),
            overflow: Overflow::clip(),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
    )
}

/// The mobile picker: a card for each mobile (its portrait, drawn from its rig) and Random, then
/// the picked one's role. `bot` names the bot the room master is picking for, if it isn't their own.
pub fn mobile_picker(parent: &mut ChildSpawnerCommands, picked: Option<MobileKind>, bot: Option<&str>) {
    parent
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| match bot {
            Some(name) => {
                row.spawn(text(format!("{name}'s mobile"), 15.0, theme::ACCENT));
                row.spawn(small_button("Back to yours", UiAction::PickFor(None)));
            }
            None => {
                row.spawn(text("Mobile", 15.0, theme::MUTED));
            }
        });
    parent.spawn(Node { column_gap: Val::Px(2.0), ..default() }).with_children(|row| {
        for mobile in MobileKind::ALL.map(Some).into_iter().chain([None]) {
            let card = Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(1.0), Val::Px(3.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            };
            row.spawn((button_node(UiAction::PickMobile(mobile), card), chosen(picked == mobile))).with_children(|card| {
                match mobile {
                    Some(mobile) => card.spawn((Portrait(mobile), portrait_frame())),
                    None => card.spawn((portrait_frame(), children![text("?", 40.0, theme::ACCENT)])),
                };
                card.spawn(text(mobile_name(mobile), 10.0, theme::TEXT));
            });
        }
    });
    parent.spawn(text(format!("{}: {}", mobile_name(picked), role(picked)), 14.0, theme::TEXT));
}

/// One loadout slot's size, in UI pixels.
const SLOT: f32 = 46.0;
const SLOT_GAP: f32 = 6.0;

/// The loadout editor: the six slots with what's in them (click one to take it out), then every
/// item with its costs (click one to add it). An item that doesn't fit can't be added, so an
/// over-full loadout never reaches the server.
pub fn loadout_editor(parent: &mut ChildSpawnerCommands, picks: &Picks, server: &AssetServer, cache: &mut ImageCache) {
    let free = picks.loadout.free_slots();
    parent
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn(text(format!("Items  {}/{} slots", picks.loadout.slots(), LOADOUT_SLOTS.value), 15.0, theme::MUTED));
            if picks.loadout.slots() > 0 {
                row.spawn(small_button("Clear", UiAction::ClearItems));
            }
        });
    parent
        .spawn(Node {
            column_gap: Val::Px(SLOT_GAP),
            ..default()
        })
        .with_children(|strip| {
            for (position, item) in (0_u8..).zip(picks.loadout.items()) {
                let slots = f32::from(item.slots());
                let node = Node {
                    width: Val::Px(SLOT * slots + SLOT_GAP * (slots - 1.0)),
                    height: Val::Px(SLOT),
                    ..default()
                };
                strip.spawn(button_node(UiAction::RemoveItem(position), node)).with_children(|slot| {
                    slot.spawn(item_art(server, cache, item, SLOT - 10.0));
                });
            }
            for _ in 0..free {
                strip.spawn((
                    Node {
                        width: Val::Px(SLOT),
                        height: Val::Px(SLOT),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                ));
            }
        });
    parent.spawn(text("Click a slot to empty it, an item to add it.", 12.0, theme::MUTED));
    parent
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(5.0),
            row_gap: Val::Px(5.0),
            width: Val::Px(PICKS_WIDTH),
            ..default()
        })
        .with_children(|grid| {
            for item in Item::ALL {
                let card = Node {
                    width: Val::Px(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(3.0)),
                    ..default()
                };
                let (slots, delay) = costs(item);
                let mut button = grid.spawn(button_node(UiAction::AddItem(item), card));
                if item.slots() > free {
                    button.insert(InteractionDisabled);
                }
                button.with_children(|card| {
                    card.spawn(item_art(server, cache, item, 20.0));
                    card.spawn(text(item.name(), 12.0, theme::TEXT));
                    card.spawn(text(slots, 10.0, theme::MUTED));
                    card.spawn(text(delay, 10.0, theme::MUTED));
                });
            }
        });
}

/// Draws each portrait from its rig: every part (and the rider) at its pivot, in draw order, the
/// cannon at a jaunty aim.
fn draw_portraits(
    portraits: Query<(Entity, &Portrait), Without<Drawn>>,
    mut rigs: ResMut<Rigs>,
    server: Res<AssetServer>,
    mut cache: ResMut<ImageCache>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    for (entity, &Portrait(mobile)) in &portraits {
        rigs.request(&server, mobile);
        let Some(rig) = rigs.get(mobile) else { continue };
        let mut layers: Vec<Layer> = rig
            .parts
            .iter()
            .map(|part| Layer {
                z: part.z,
                image: cache.get(&server, lobshot_assets::part_path(mobile, &part.file)),
                at: part.at,
                aim: part.aim,
            })
            .collect();
        let rider = web::load("rider").unwrap_or_else(|| "pip".to_owned());
        layers.push(Layer {
            z: rig.rider_socket.z,
            image: cache.get(&server, lobshot_assets::rider_path(&rider)),
            at: rig.rider_socket.at,
            aim: false,
        });
        // Stable: parts on the same z keep the rig's order.
        layers.sort_by_key(|layer| layer.z);
        let Some(sizes) = layers.iter().map(|layer| images.get(&layer.image).map(Image::size_f32)).collect::<Option<Vec<Vec2>>>() else {
            continue;
        };
        // ART_SCALE is 2.0; rig offsets are tens of pixels: both exact in f32.
        #[allow(clippy::cast_possible_truncation)]
        let (art_scale, offset) = (ART_SCALE as f32, |at: (f64, f64)| Vec2::new(at.0 as f32, at.1 as f32));
        // The body frame's origin, the middle of the ground contact, sits near the frame's foot.
        let origin = Vec2::new(PORTRAIT.x / 2.0, PORTRAIT.y - 3.0);
        commands.entity(entity).insert(Drawn).with_children(|frame| {
            for (Layer { image, at, aim, .. }, size) in layers.into_iter().zip(sizes) {
                let size = size / art_scale * PORTRAIT_SCALE;
                let center = origin + offset(at) * PORTRAIT_SCALE;
                let mut layer = frame.spawn((
                    ImageNode::new(image),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(center.x - size.x / 2.0),
                        top: Val::Px(center.y - size.y / 2.0),
                        width: Val::Px(size.x),
                        height: Val::Px(size.y),
                        ..default()
                    },
                ));
                if aim {
                    // UI rotation turns clockwise on screen; aim counts counterclockwise.
                    layer.insert(UiTransform::from_rotation(Rot2::degrees(-portrait_aim(mobile))));
                }
            }
        });
    }
}

/// Tells the room this player's picks whenever they differ from what it has: on joining, and after
/// each change. Once per change, so a refusal isn't repeated every frame.
fn sync(session: Res<Session>, picks: Res<Picks>, mut socket: NonSendMut<Socket>, mut sent: Local<Option<Picks>>) {
    let Some(room) = session.room.as_ref() else { return };
    let Some(mine) = room.members.iter().find(|member| Some(member.id) == session.me) else { return };
    if !matches!(room.stage, Stage::Lobby) {
        return;
    }
    if mine.mobile == picks.mobile && mine.items == picks.loadout {
        *sent = None;
        return;
    }
    if sent.as_ref() == Some(&*picks) {
        return;
    }
    if mine.mobile != picks.mobile {
        socket.send(&ClientMsg::Mobile(picks.mobile));
    }
    if mine.items != picks.loadout {
        socket.send(&ClientMsg::Loadout(picks.loadout));
    }
    *sent = Some(picks.clone());
}

pub struct PicksPlugin;

impl Plugin for PicksPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Picks::load())
            .add_systems(Update, (draw_portraits.in_set(DrawPortraits), sync.run_if(in_state(Screen::Lobby))));
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn the_starter_loadout_is_full_and_legal() {
        assert_eq!(starter().slots(), LOADOUT_SLOTS.value);
        assert_eq!(starter().items().count(), 5);
    }

    #[test]
    // Exact: the aims are a constant and 180 minus it.
    #[allow(clippy::float_cmp)]
    fn a_portrait_aims_over_the_back_of_a_mobile_that_fires_that_way() {
        assert_eq!(portrait_aim(MobileKind::Coilback), PORTRAIT_AIM);
        assert_eq!(portrait_aim(MobileKind::Augerdillo), 180.0 - PORTRAIT_AIM);
    }

    #[test]
    fn mobile_names_round_trip_with_random() {
        for mobile in MobileKind::ALL.map(Some).into_iter().chain([None]) {
            assert_eq!(parse_mobile(mobile_name(mobile)).unwrap(), mobile);
        }
        assert_eq!(parse_mobile("Random").unwrap(), None);
        assert!(parse_mobile("Tank").is_err());
    }
}
