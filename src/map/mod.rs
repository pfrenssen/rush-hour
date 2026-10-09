//! The city map: ASCII parsing, lane graph and route finding.

// Parts of the level data and lane graph are only consumed once vehicles drive.
#![allow(dead_code)]

pub mod geom;
pub mod graph;
pub mod grid;
pub mod route;

use bevy::prelude::*;

use crate::levels::{LEVELS, LevelDef};
use crate::states::{AppState, LevelLoad};
use graph::{LaneGraph, LaneId};
use grid::Grid;

/// Index into [`LEVELS`] of the level to play.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CurrentLevel(pub usize);

impl CurrentLevel {
    pub fn def(&self) -> &'static LevelDef {
        &LEVELS[self.0]
    }
}

/// The loaded level.
#[derive(Resource)]
pub struct CityMap {
    pub level: usize,
    pub grid: Grid,
    pub graph: LaneGraph,
    /// Shortest routes between terminals on different sides, indexed `[entry][exit]`.
    pub routes: Vec<Vec<Option<Vec<LaneId>>>>,
}

impl CityMap {
    pub fn load(level: usize) -> CityMap {
        let def = &LEVELS[level];
        let grid = Grid::parse(def.rows)
            .unwrap_or_else(|e| panic!("level {:?} is malformed: {e}", def.name));
        let graph = LaneGraph::build(&grid);
        let routes = route::route_table(&graph);
        CityMap {
            level,
            grid,
            graph,
            routes,
        }
    }

    pub fn def(&self) -> &'static LevelDef {
        &LEVELS[self.level]
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentLevel>()
            .add_systems(OnEnter(AppState::InGame), load_map.in_set(LevelLoad::Map));
    }
}

fn load_map(mut commands: Commands, current: Res<CurrentLevel>) {
    commands.insert_resource(CityMap::load(current.0));
}
