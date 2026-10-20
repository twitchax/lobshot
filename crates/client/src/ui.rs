//! Shared UI pieces: the theme, text and button helpers, and one route for every button. A button
//! carries a `UiAction`; one observer turns its `Activate` into a `Clicked` message that ordinary
//! systems handle with whatever they need.

use bevy::{picking::hover::Hovered, prelude::*, ui::Pressed, ui_widgets::Activate};
use lobshot_protocol::{MemberId, RoomOptions};
use lobshot_sim::game::{Shot, Team};

/// The palette: night-sea panels, warm brass accents, team colors.
pub mod theme {
    use bevy::prelude::Color;

    pub const BACKDROP: Color = Color::srgb_u8(0x1b, 0x23, 0x33);
    pub const PANEL: Color = Color::srgba(0.07, 0.10, 0.16, 0.86);
    pub const BUTTON: Color = Color::srgb_u8(0x2c, 0x3b, 0x56);
    pub const BUTTON_HOVER: Color = Color::srgb_u8(0x3a, 0x4e, 0x72);
    pub const BUTTON_PRESSED: Color = Color::srgb_u8(0x22, 0x2e, 0x44);
    pub const ACCENT: Color = Color::srgb_u8(0xf2, 0xb8, 0x4b);
    pub const TEXT: Color = Color::srgb_u8(0xf4, 0xec, 0xd8);
    pub const MUTED: Color = Color::srgb_u8(0x9a, 0xa7, 0xbd);
    pub const TEAM_A: Color = Color::srgb_u8(0x4f, 0xa3, 0xe0);
    pub const TEAM_B: Color = Color::srgb_u8(0xe0, 0x5a, 0x4f);
    pub const GOOD: Color = Color::srgb_u8(0x7c, 0xd6, 0x7a);
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
    Practice,
    CopyLink,
    Join(Option<Team>),
    Ready(bool),
    AddBot(Team),
    Kick(MemberId),
    Start,
    Configure(RoomOptions),
    Shot(Shot),
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

/// A themed button.
#[must_use]
pub fn button(label: impl Into<String>, action: UiAction) -> impl Bundle {
    (
        bevy::ui_widgets::Button,
        action,
        Hovered::default(),
        Node {
            padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
            margin: UiRect::all(Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(theme::BUTTON),
        children![text(label, 18.0, theme::TEXT)],
    )
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

/// Each shown button's center in CSS pixels, by its action (`Practice`), for the browser smoke test
/// to click.
#[derive(Resource, Default)]
pub struct ButtonSpots(pub Vec<(String, Vec2)>);

fn locate_buttons(buttons: Query<(&UiAction, &ComputedNode, &UiGlobalTransform, &InheritedVisibility)>, mut spots: ResMut<ButtonSpots>) {
    spots.0.clear();
    for (action, node, transform, shown) in &buttons {
        if shown.get() {
            spots.0.push((format!("{action:?}"), transform.translation * node.inverse_scale_factor()));
        }
    }
}

fn shade_buttons(mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<UiAction>>) {
    for (hovered, pressed, mut color) in &mut buttons {
        let shade = if pressed {
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
            .add_systems(Update, shade_buttons)
            .add_systems(PostUpdate, locate_buttons.after(bevy::ui::UiSystems::Layout));
    }
}
