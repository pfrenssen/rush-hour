//! Simulation wiring: fixed timestep, system ordering and the seeded RNG.
//!
//! The simulation plugins (map, lights, vehicles, scoring, autopilot) only need `MinimalPlugins`
//! plus `StatesPlugin`, so the same code drives the windowed game and headless runs.

use bevy::prelude::*;

use crate::map::CurrentLevel;
use crate::states::{AppState, GamePhase, LevelLoad};

pub const TICK_HZ: f64 = 60.0;

/// Order of the simulation systems inside `FixedUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Light controllers (the autopilot).
    Control,
    Lights,
    Spawn,
    Move,
    Score,
}

/// Run-wide settings.
#[derive(Resource, Clone, Debug)]
pub struct SimConfig {
    pub seed: u64,
}

/// The only source of randomness in the simulation.
#[derive(Resource)]
pub struct SimRng(pub fastrand::Rng);

/// Counts level starts so that restarts get fresh traffic while staying reproducible.
#[derive(Resource, Default)]
pub struct Attempts(pub u64);

pub struct SimPlugin {
    pub seed: u64,
}

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
            .insert_resource(SimConfig { seed: self.seed })
            .insert_resource(SimRng(fastrand::Rng::with_seed(self.seed)))
            .init_resource::<Attempts>()
            .configure_sets(
                FixedUpdate,
                (
                    SimSet::Control,
                    SimSet::Lights,
                    SimSet::Spawn,
                    SimSet::Move,
                    SimSet::Score,
                )
                    .chain()
                    .run_if(in_state(GamePhase::Running)),
            )
            .add_systems(OnEnter(AppState::InGame), reseed.in_set(LevelLoad::Sim))
            .add_plugins((
                crate::map::MapPlugin,
                crate::lights::LightsPlugin,
                crate::vehicles::VehiclesPlugin,
            ));
    }
}

fn reseed(
    config: Res<SimConfig>,
    level: Res<CurrentLevel>,
    mut attempts: ResMut<Attempts>,
    mut rng: ResMut<SimRng>,
) {
    let seed = config
        .seed
        .wrapping_add(level.0 as u64 * 0x9E37_79B9)
        .wrapping_add(attempts.0 * 0x85EB_CA6B);
    attempts.0 += 1;
    rng.0 = fastrand::Rng::with_seed(seed);
}
