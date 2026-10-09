//! Everything that draws the world: camera, map, vehicles, lights and effects.

pub mod camera;
pub mod lights;
pub mod map;
pub mod shapes;
pub mod vehicles;

use bevy::prelude::*;

use crate::states::{AppState, LevelLoad};

/// Draw order of the world layers.
pub mod z {
    pub const GROUND: f32 = 0.0;
    pub const MARKINGS: f32 = 1.0;
    pub const SHADOWS: f32 = 2.0;
    pub const BUILDINGS: f32 = 3.0;
    pub const VEHICLE_SHADOWS: f32 = 3.9;
    pub const VEHICLES: f32 = 4.0;
    pub const LIGHTS: f32 = 6.0;
    pub const BUBBLES: f32 = 7.0;
    pub const OVERLAY: f32 = 8.0;
}

pub mod palette {
    use bevy::prelude::Color;

    pub const BACKGROUND: Color = Color::srgb(0.07, 0.08, 0.10);
    pub const FRAME: Color = Color::srgb(0.11, 0.12, 0.15);
    pub const LAWN: Color = Color::srgb(0.33, 0.52, 0.36);
    pub const PARK: Color = Color::srgb(0.37, 0.60, 0.38);
    pub const WATER: Color = Color::srgb(0.22, 0.45, 0.66);
    pub const RIPPLE: Color = Color::srgba(0.75, 0.88, 1.0, 0.35);
    pub const ASPHALT: Color = Color::srgb(0.20, 0.21, 0.24);
    pub const SIDEWALK: Color = Color::srgb(0.62, 0.65, 0.66);
    pub const CURB: Color = Color::srgb(0.47, 0.50, 0.52);
    pub const RAIL: Color = Color::srgb(0.78, 0.80, 0.82);
    pub const MARKING: Color = Color::srgba(0.95, 0.95, 0.92, 0.85);
    pub const ZEBRA: Color = Color::srgba(0.95, 0.95, 0.92, 0.55);
    pub const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.28);
    pub const TREE: Color = Color::srgb(0.17, 0.40, 0.24);
    pub const TREE_LIGHT: Color = Color::srgb(0.27, 0.53, 0.31);
    pub const ROOFS: [Color; 6] = [
        Color::srgb(0.85, 0.80, 0.71),
        Color::srgb(0.79, 0.62, 0.53),
        Color::srgb(0.62, 0.70, 0.78),
        Color::srgb(0.72, 0.72, 0.78),
        Color::srgb(0.88, 0.84, 0.76),
        Color::srgb(0.66, 0.71, 0.63),
    ];
}

pub struct GameRenderPlugin;

impl Plugin for GameRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(palette::BACKGROUND))
            .init_resource::<lights::LampMaterials>()
            .init_resource::<vehicles::VehicleAssets>()
            .add_systems(Startup, camera::spawn_camera)
            .add_systems(
                OnEnter(AppState::InGame),
                (map::spawn_map, lights::spawn_lights).in_set(LevelLoad::Visuals),
            )
            .add_systems(
                Update,
                (
                    lights::update_lamps,
                    lights::update_hover_ring,
                    (vehicles::dress_vehicles, vehicles::animate_vehicles).chain(),
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                PostUpdate,
                camera::fit_camera.before(TransformSystems::Propagate),
            );
    }
}
