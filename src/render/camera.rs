//! Camera that fits the whole map below the HUD and follows window resizes.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use crate::map::CityMap;
use crate::map::graph::EDGE_EXTENSION;

/// Logical pixels reserved for the HUD bar at the top of the window.
pub const HUD_HEIGHT: f32 = 60.0;
const MARGIN: f32 = 12.0;

#[derive(Component)]
pub struct MainCamera;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
}

pub fn fit_camera(
    map: Option<Res<CityMap>>,
    window: Single<&Window>,
    camera: Single<(&mut Projection, &mut Transform), With<MainCamera>>,
) {
    let Some(map) = map else { return };
    let (mut projection, mut transform) = camera.into_inner();
    let world = map.grid.world_size() + Vec2::splat(2.0 * EDGE_EXTENSION);
    let avail = Vec2::new(
        window.width() - 2.0 * MARGIN,
        window.height() - HUD_HEIGHT - 2.0 * MARGIN,
    )
    .max(Vec2::splat(32.0));
    let scale = (world.x / avail.x).max(world.y / avail.y);
    if let Projection::Orthographic(ortho) = &mut *projection
        && (ortho.scale != scale || !matches!(ortho.scaling_mode, ScalingMode::WindowSize))
    {
        ortho.scaling_mode = ScalingMode::WindowSize;
        ortho.scale = scale;
    }
    let y = HUD_HEIGHT / 2.0 * scale;
    if transform.translation.y != y || transform.translation.x != 0.0 {
        transform.translation.x = 0.0;
        transform.translation.y = y;
    }
}
