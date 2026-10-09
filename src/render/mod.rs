//! Everything that draws the world: camera, map, vehicles, lights and effects.

use bevy::prelude::*;

pub const BACKGROUND: Color = Color::srgb(0.07, 0.08, 0.10);

pub struct GameRenderPlugin;

impl Plugin for GameRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(BACKGROUND))
            .add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
