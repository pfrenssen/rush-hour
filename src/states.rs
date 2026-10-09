//! Application state machine.
//!
//! `AppState` decides whether a level is loaded at all; `GamePhase` is a sub-state that only
//! exists while a level is loaded, so pausing or finishing a level never tears the map down.
//! Re-entering `AppState::InGame` (restart, next level) despawns and rebuilds the level.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}

#[derive(SubStates, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[source(AppState = AppState::InGame)]
pub enum GamePhase {
    #[default]
    Running,
    Paused,
}

/// Ordering of the systems that set up a level when entering `AppState::InGame`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelLoad {
    /// Parse the level and build the lane graph.
    Map,
    /// Reset simulation resources that depend on the map.
    Sim,
    /// Spawn the visuals.
    Visuals,
}

pub struct GameStatesPlugin;

impl Plugin for GameStatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<GamePhase>()
            .configure_sets(
                OnEnter(AppState::InGame),
                (LevelLoad::Map, LevelLoad::Sim, LevelLoad::Visuals).chain(),
            );
    }
}
