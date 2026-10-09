mod render;
mod states;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rush Hour".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            states::GameStatesPlugin,
            render::GameRenderPlugin,
            ui::GameUiPlugin,
        ))
        .run();
}
