//! Application state machine.
//!
//! `AppState` decides whether a level is loaded at all; `GamePhase` is a sub-state that only
//! exists while a level is loaded, so pausing or finishing a level never tears the map down.
//! Re-entering `AppState::InGame` (restart, next level) despawns and rebuilds the level.

use bevy::prelude::*;

use crate::levels::LEVELS;
use crate::map::CurrentLevel;
use crate::scoring::Game;

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
    LevelComplete,
    GameOver,
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

/// Something the player asked for, from a button or a key.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start a new game at this level (0-based), with the score reset.
    Play(usize),
    /// Continue with the next level, keeping the score.
    Next,
    /// Restart the current level with the score it started with.
    Restart,
    Pause,
    Resume,
    Menu,
}

pub struct GameStatesPlugin;

impl Plugin for GameStatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<GamePhase>()
            .add_message::<Action>()
            .configure_sets(
                OnEnter(AppState::InGame),
                (LevelLoad::Map, LevelLoad::Sim, LevelLoad::Visuals).chain(),
            )
            .add_systems(PreUpdate, apply_actions);
    }
}

fn apply_actions(
    mut actions: MessageReader<Action>,
    mut level: ResMut<CurrentLevel>,
    mut game: ResMut<Game>,
    phase: Option<Res<State<GamePhase>>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_phase: ResMut<NextState<GamePhase>>,
) {
    let phase = phase.map(|p| *p.get());
    for action in actions.read() {
        // Re-entering `InGame` rebuilds the level but keeps the sub-state, so every (re)start
        // also asks for a running level.
        if matches!(action, Action::Play(_) | Action::Next | Action::Restart) {
            next_phase.set(GamePhase::Running);
        }
        match *action {
            Action::Play(n) => {
                level.0 = n.min(LEVELS.len() - 1);
                game.level_start_score = 0;
                next_app.set(AppState::InGame);
            }
            Action::Next if level.0 + 1 < LEVELS.len() => {
                level.0 += 1;
                game.level_start_score = game.score;
                next_app.set(AppState::InGame);
            }
            Action::Next | Action::Menu => next_app.set(AppState::Menu),
            Action::Restart => next_app.set(AppState::InGame),
            Action::Pause if phase == Some(GamePhase::Running) => {
                next_phase.set(GamePhase::Paused);
            }
            Action::Resume if phase == Some(GamePhase::Paused) => {
                next_phase.set(GamePhase::Running);
            }
            Action::Pause | Action::Resume => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, GameStatesPlugin))
            .init_resource::<CurrentLevel>()
            .init_resource::<Game>();
        app.update();
        app
    }

    fn act(app: &mut App, action: Action) {
        app.world_mut().write_message(action);
        app.update();
        app.update();
    }

    fn phase(app: &App) -> Option<GamePhase> {
        app.world()
            .get_resource::<State<GamePhase>>()
            .map(|s| *s.get())
    }

    #[test]
    fn play_pause_resume_and_restart() {
        let mut app = app();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        assert_eq!(phase(&app), None);

        act(&mut app, Action::Play(2));
        assert_eq!(app.world().resource::<CurrentLevel>().0, 2);
        assert_eq!(phase(&app), Some(GamePhase::Running));

        act(&mut app, Action::Pause);
        assert_eq!(phase(&app), Some(GamePhase::Paused));
        act(&mut app, Action::Resume);
        assert_eq!(phase(&app), Some(GamePhase::Running));

        // A restart from any phase comes back running.
        act(&mut app, Action::Pause);
        act(&mut app, Action::Restart);
        assert_eq!(phase(&app), Some(GamePhase::Running));
    }

    #[test]
    fn next_level_keeps_the_score_and_the_last_level_returns_to_the_menu() {
        let mut app = app();
        act(&mut app, Action::Play(LEVELS.len() - 2));
        app.world_mut().resource_mut::<Game>().score = 1234;
        act(&mut app, Action::Next);
        assert_eq!(app.world().resource::<CurrentLevel>().0, LEVELS.len() - 1);
        assert_eq!(app.world().resource::<Game>().level_start_score, 1234);
        act(&mut app, Action::Next);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
    }
}
