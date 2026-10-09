//! Player input: hovering and clicking junctions, pause and speed keys.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::lights::SwitchRequest;
use crate::map::CityMap;
use crate::map::graph::{HALF, NodeId};
use crate::render::camera::MainCamera;
use crate::states::{AppState, GamePhase};

/// How far outside its box a junction still reacts to the mouse.
const PICK_MARGIN: f32 = 4.0;

/// The signalled junction under the mouse cursor.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct HoveredNode(pub Option<NodeId>);

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredNode>()
            .add_systems(
                Update,
                (
                    hover_and_click.run_if(in_state(GamePhase::Running)),
                    pause_keys,
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnExit(GamePhase::Running), clear_hover);
    }
}

/// Signalled junction nearest to a world-space point, if the point is close enough.
pub fn pick_node(map: &CityMap, p: Vec2) -> Option<NodeId> {
    map.graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.is_signal())
        .map(|(i, n)| (i, (p - n.center).abs().max_element()))
        .filter(|&(_, d)| d <= HALF + PICK_MARGIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn hover_and_click(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    map: Res<CityMap>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut hovered: ResMut<HoveredNode>,
    mut requests: MessageWriter<SwitchRequest>,
) {
    let (camera, transform) = *camera;
    let node = window
        .cursor_position()
        .and_then(|c| camera.viewport_to_world_2d(transform, c).ok())
        .and_then(|p| pick_node(&map, p));
    if hovered.0 != node {
        hovered.0 = node;
    }
    if buttons.just_pressed(MouseButton::Left)
        && let Some(node) = node
    {
        requests.write(SwitchRequest(node));
    }
}

fn clear_hover(mut hovered: ResMut<HoveredNode>) {
    hovered.0 = None;
}

fn pause_keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<GamePhase>>,
    mut next_phase: ResMut<NextState<GamePhase>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyP]) {
        match phase.get() {
            GamePhase::Running => next_phase.set(GamePhase::Paused),
            GamePhase::Paused => next_phase.set(GamePhase::Running),
        }
    }
    if *phase.get() == GamePhase::Paused && keys.just_pressed(KeyCode::KeyM) {
        next.set(AppState::Menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_junction_under_the_cursor() {
        let map = CityMap::load(0);
        for (i, n) in map.graph.nodes.iter().enumerate() {
            if n.is_signal() {
                assert_eq!(pick_node(&map, n.center), Some(i));
                assert_eq!(pick_node(&map, n.center + Vec2::new(5.0, -5.0)), Some(i));
            }
        }
        // Far away from any junction.
        let corner = map.grid.cell_center(0, 0);
        assert_eq!(pick_node(&map, corner), None);
    }
}
