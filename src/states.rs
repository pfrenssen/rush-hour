//! Application state machine.
//!
//! `AppState` decides whether a level is loaded at all; `GamePhase` is a sub-state that only
//! exists while a level is loaded, so pausing or finishing a level never tears the map down.

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

pub struct GameStatesPlugin;

impl Plugin for GameStatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>().add_sub_state::<GamePhase>();
    }
}
