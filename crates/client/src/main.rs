//! The lobshot client: Bevy, compiled to wasm32 and built by trunk (`cargo make dist`), drawn into
//! the page's `<canvas id="lobshot">`. The title screen creates rooms or starts practice; a room
//! page (`/r/{code}`) joins that room over its WebSocket.

// Bevy systems receive their parameters (`Res<T>`, `Query<..>`, ...) by value; the ECS hands them
// over that way and a reference is not an option, so pedantic's `needless_pass_by_value` flags
// nearly every system. Allowed for this crate only, by Aaron's call (PRD-0003 T-015); `sim`,
// `server` and `lab` keep it denied.
#![allow(clippy::needless_pass_by_value)]

mod assets;
mod game;
mod model;
mod net;
mod screens;
mod ui;
mod web;

use bevy::{asset::AssetMetaCheck, prelude::*};

fn main() -> AppExit {
    App::new()
        .insert_resource(ClearColor(ui::theme::BACKDROP))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "lobshot".into(),
                        // Draw into the page's canvas instead of appending a new one, sized to its parent.
                        canvas: Some("#lobshot".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                // No asset has a `.meta` file; asking for each one is a wasted 404 on the web.
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .init_resource::<web::TextInput>()
        .add_plugins((assets::AssetsPlugin, net::NetPlugin, model::ModelPlugin, ui::UiPlugin, screens::ScreensPlugin, game::GamePlugin))
        .run()
}
