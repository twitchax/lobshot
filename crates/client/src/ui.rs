//! Shared UI pieces: the theme, text and button helpers, art that may not be drawn yet, and one
//! route for every button. A button carries a `UiAction`; one observer turns its `Activate` into a
//! `Clicked` message that ordinary systems handle with whatever they need. A button with
//! `InteractionDisabled` shows dimmed and can't be pressed.

use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::Activate,
};
use lobshot_protocol::{MemberId, RoomOptions};
use lobshot_sim::{
    game::{MobileKind, Shot, Team},
    items::Item,
};

/// The palette: night-sea panels, warm brass accents, team colors.
pub mod theme {
    use bevy::prelude::Color;

    pub const BACKDROP: Color = Color::srgb_u8(0x1b, 0x23, 0x33);
    pub const PANEL: Color = Color::srgba(0.07, 0.10, 0.16, 0.86);
    pub const BUTTON: Color = Color::srgb_u8(0x2c, 0x3b, 0x56);
    pub const BUTTON_HOVER: Color = Color::srgb_u8(0x3a, 0x4e, 0x72);
    pub const BUTTON_PRESSED: Color = Color::srgb_u8(0x22, 0x2e, 0x44);
    pub const BUTTON_DISABLED: Color = Color::srgb_u8(0x24, 0x2a, 0x36);
    /// The box behind art that hasn't loaded (or doesn't exist yet).
    pub const ART_BOX: Color = Color::srgb_u8(0x3b, 0x34, 0x4a);
    pub const ACCENT: Color = Color::srgb_u8(0xf2, 0xb8, 0x4b);
    pub const TEXT: Color = Color::srgb_u8(0xf4, 0xec, 0xd8);
    pub const MUTED: Color = Color::srgb_u8(0x9a, 0xa7, 0xbd);
    pub const TEAM_A: Color = Color::srgb_u8(0x4f, 0xa3, 0xe0);
    pub const TEAM_B: Color = Color::srgb_u8(0xe0, 0x5a, 0x4f);
    pub const GOOD: Color = Color::srgb_u8(0x7c, 0xd6, 0x7a);
    /// Pale ice, so a shield reads apart from team A's blue HP bar under it.
    pub const SHIELD: Color = Color::srgb_u8(0xc4, 0xf1, 0xff);
}

/// The color for a team.
#[must_use]
pub fn team_color(team: Team) -> Color {
    match team {
        Team::A => theme::TEAM_A,
        Team::B => theme::TEAM_B,
    }
}

/// What a button does.
#[derive(Component, Debug, Clone, PartialEq)]
pub enum UiAction {
    NewGame,
    /// Practice 1v1 against a bot.
    Practice,
    /// Practice 2v2: you and a bot against two bots.
    PracticeTeams,
    /// The garage: your mobile and items, outside any room.
    Garage,
    /// Back to the title screen (no page load).
    Title,
    CopyLink,
    Join(Option<Team>),
    Ready(bool),
    AddBot(Team),
    Kick(MemberId),
    Start,
    Configure(RoomOptions),
    /// A mobile, or `None` for Random: this player's, or the bot's the picker is set to.
    PickMobile(Option<MobileKind>),
    /// Point the mobile picker at one of the room's bots (master only), or `None` back at yours.
    PickFor(Option<MemberId>),
    /// Add an item to the end of the loadout.
    AddItem(Item),
    /// Take the item at this loadout position out.
    RemoveItem(u8),
    ClearItems,
    Shot(Shot),
    /// Use the item at this loadout position.
    Item(u8),
    /// Spin the slot machine (dead players).
    Spin,
    Pass,
    Chat,
    Leave,
    /// Reload the room page: the saved token takes the seat back.
    Rejoin,
}

/// A button was pressed.
#[derive(Message, Debug, Clone)]
pub struct Clicked(pub UiAction);

fn route(activate: On<Activate>, actions: Query<&UiAction>, mut clicked: MessageWriter<Clicked>) {
    if let Ok(action) = actions.get(activate.entity) {
        clicked.write(Clicked(action.clone()));
    }
}

/// Text in the theme.
#[must_use]
pub fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// A themed button with no content of its own: give it children. `node` lays it out.
#[must_use]
pub fn button_node(action: UiAction, node: Node) -> impl Bundle {
    (
        bevy::ui_widgets::Button,
        action,
        Hovered::default(),
        Node {
            border_radius: BorderRadius::all(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..node
        },
        BackgroundColor(theme::BUTTON),
    )
}

/// A themed button.
#[must_use]
pub fn button(label: impl Into<String>, action: UiAction) -> impl Bundle {
    (
        button_node(
            action,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                margin: UiRect::all(Val::Px(4.0)),
                ..default()
            },
        ),
        children![text(label, 18.0, theme::TEXT)],
    )
}

/// A smaller button, for crowded screens (the lobby with eight players).
#[must_use]
pub fn small_button(label: impl Into<String>, action: UiAction) -> impl Bundle {
    (
        button_node(
            action,
            Node {
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                margin: UiRect::all(Val::Px(3.0)),
                ..default()
            },
        ),
        children![text(label, 14.0, theme::TEXT)],
    )
}

/// The border that marks the chosen button of a group. Give every button in the group the same
/// border width, so choosing one doesn't shift the others.
#[must_use]
pub fn chosen(chosen: bool) -> BorderColor {
    BorderColor::all(if chosen { theme::ACCENT } else { Color::NONE })
}

/// Art that may not be drawn yet (item icons, drop sprites): a box with `label` in it until the
/// image loads, and for good if the file doesn't exist.
#[derive(Component)]
pub struct Art(Handle<Image>);

/// The image inside an `Art` box, shown once it loads.
#[derive(Component)]
struct ArtImage;

/// An `Art` box `size` UI pixels big.
#[must_use]
pub fn art(image: Handle<Image>, label: &str, size: Vec2, font: f32) -> impl Bundle {
    (
        Art(image.clone()),
        Node {
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::ART_BOX),
        children![
            (text(label, font, theme::TEXT), TextLayout::justify(Justify::Center)),
            (
                ArtImage,
                ImageNode::new(image),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
            ),
        ],
    )
}

/// Swaps an `Art` box's label for its image once the image has loaded.
fn reveal_art(arts: Query<(Entity, &Art, &Children)>, images: Res<Assets<Image>>, mut nodes: Query<(&mut Node, Has<ArtImage>), Without<Art>>, mut commands: Commands) {
    for (entity, art, children) in &arts {
        if !images.contains(&art.0) {
            continue;
        }
        for child in children.iter() {
            if let Ok((mut node, image)) = nodes.get_mut(child) {
                node.display = if image { Display::Flex } else { Display::None };
            }
        }
        // `try_`: a screen rebuilt this frame may already have taken the box down.
        commands.entity(entity).try_remove::<Art>().try_insert(BackgroundColor(Color::NONE));
    }
}

/// A themed panel.
#[must_use]
pub fn panel(node: Node) -> impl Bundle {
    (
        Node {
            padding: UiRect::all(Val::Px(12.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..node
        },
        BackgroundColor(theme::PANEL),
    )
}

/// A shown button, for the browser smoke test to find and click.
#[derive(Debug, Clone)]
pub struct Spot {
    /// The button's action, as `Debug` prints it (`Practice`, `AddItem(Dual)`).
    pub action: String,
    /// Its center in CSS pixels.
    pub at: Vec2,
    /// Whether it can be pressed (not `InteractionDisabled`).
    pub enabled: bool,
}

/// Every shown button.
#[derive(Resource, Default)]
pub struct ButtonSpots(pub Vec<Spot>);

fn locate_buttons(buttons: Query<(&UiAction, &ComputedNode, &UiGlobalTransform, &InheritedVisibility, Has<InteractionDisabled>)>, mut spots: ResMut<ButtonSpots>) {
    spots.0.clear();
    for (action, node, transform, shown, disabled) in &buttons {
        // A node under a `Display::None` parent keeps its visibility but has no size.
        if shown.get() && node.size() != Vec2::ZERO {
            spots.0.push(Spot {
                action: format!("{action:?}"),
                at: transform.translation * node.inverse_scale_factor(),
                enabled: !disabled,
            });
        }
    }
}

/// A button's hover, press and disabled states, and the color they set.
type Shading<'w, 's> = Query<'w, 's, (&'static Hovered, Has<Pressed>, Has<InteractionDisabled>, &'static mut BackgroundColor), With<UiAction>>;

fn shade_buttons(mut buttons: Shading) {
    for (hovered, pressed, disabled, mut color) in &mut buttons {
        let shade = if disabled {
            theme::BUTTON_DISABLED
        } else if pressed {
            theme::BUTTON_PRESSED
        } else if hovered.get() {
            theme::BUTTON_HOVER
        } else {
            theme::BUTTON
        };
        if color.0 != shade {
            color.0 = shade;
        }
    }
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // The widgets' plugins (button activation, focus) come with `DefaultPlugins`.
        app.add_message::<Clicked>()
            .init_resource::<ButtonSpots>()
            .add_observer(route)
            .add_systems(Update, (shade_buttons, reveal_art))
            .add_systems(PostUpdate, locate_buttons.after(bevy::ui::UiSystems::Layout));
    }
}
