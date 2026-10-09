//! Player input: hovering and clicking junctions, pause and speed keys.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::lights::SwitchRequest;
use crate::map::CityMap;
use crate::map::graph::{HALF, NodeId};
use crate::render::camera::MainCamera;
use crate::sim::SimSpeed;
use crate::states::{Action, AppState, GamePhase};

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
                hover_and_click
                    .run_if(in_state(GamePhase::Running))
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(Update, keys)
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

/// Keyboard shortcuts for every screen.
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppState>>,
    phase: Option<Res<State<GamePhase>>>,
    mut speed: ResMut<SimSpeed>,
    mut actions: MessageWriter<Action>,
) {
    const LEVEL_KEYS: [KeyCode; 6] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    let pressed = |k: KeyCode| keys.just_pressed(k);
    let confirm = pressed(KeyCode::Enter) || pressed(KeyCode::NumpadEnter);
    let pause = pressed(KeyCode::Escape) || pressed(KeyCode::KeyP);
    let action = match (app_state.get(), phase.map(|p| *p.get())) {
        (AppState::Menu, _) => LEVEL_KEYS
            .iter()
            .position(|&k| pressed(k))
            .filter(|&i| i < crate::levels::LEVELS.len())
            .map(Action::Play)
            .or(confirm.then_some(Action::Play(0))),
        (AppState::InGame, Some(GamePhase::Running)) => {
            if pressed(KeyCode::KeyF) || pressed(KeyCode::Tab) {
                speed.toggle();
            }
            pause.then_some(Action::Pause)
        }
        (AppState::InGame, Some(GamePhase::Paused)) => {
            if pause || confirm {
                Some(Action::Resume)
            } else if pressed(KeyCode::KeyR) {
                Some(Action::Restart)
            } else {
                pressed(KeyCode::KeyM).then_some(Action::Menu)
            }
        }
        (AppState::InGame, Some(GamePhase::LevelComplete)) => {
            if confirm || pressed(KeyCode::KeyN) {
                Some(Action::Next)
            } else if pressed(KeyCode::KeyR) {
                Some(Action::Restart)
            } else {
                pressed(KeyCode::KeyM).then_some(Action::Menu)
            }
        }
        (AppState::InGame, Some(GamePhase::GameOver)) => {
            if confirm || pressed(KeyCode::KeyR) {
                Some(Action::Restart)
            } else {
                pressed(KeyCode::KeyM).then_some(Action::Menu)
            }
        }
        (AppState::InGame, None) => None,
    };
    if let Some(action) = action {
        actions.write(action);
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
