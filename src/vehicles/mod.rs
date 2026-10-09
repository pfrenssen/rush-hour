//! Vehicles: spawning, routing and motion. The driving model itself lives in [`traffic`].

pub mod kind;
pub mod spawn;
pub mod traffic;

use bevy::prelude::*;

use crate::lights::Signals;
use crate::map::CityMap;
use crate::map::graph::TerminalId;
use crate::sim::{SimRng, SimSet};
use crate::states::{AppState, LevelLoad};
use kind::VehicleKind;
use spawn::{Spawner, spawn_rate};
use traffic::Vehicle;

// The events below are consumed by scoring, which arrives in a later commit.

/// A vehicle reached its exit.
#[allow(dead_code)]
#[derive(Message, Clone, Debug)]
pub struct Delivered {
    pub kind: VehicleKind,
    pub ideal_time: f32,
    pub trip_time: f32,
    pub exit: TerminalId,
}

/// A vehicle ran out of patience.
#[allow(dead_code)]
#[derive(Message, Clone, Debug)]
pub struct FedUp {
    pub pos: Vec2,
}

/// A spawn point stayed blocked for too long.
#[allow(dead_code)]
#[derive(Message, Clone, Debug)]
pub struct SpawnBlocked {
    pub terminal: TerminalId,
}

/// Running totals for the current level.
#[derive(Resource, Default, Debug, Clone)]
pub struct TrafficStats {
    pub spawned: u32,
    pub delivered: u32,
    /// Longest total standing time of any single vehicle.
    pub max_wait: f32,
    /// Ticks in which two vehicle bodies overlapped (summed over pairs). Must stay zero.
    pub overlaps: u32,
}

pub struct VehiclesPlugin;

impl Plugin for VehiclesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Delivered>()
            .add_message::<FedUp>()
            .add_message::<SpawnBlocked>()
            .init_resource::<TrafficStats>()
            .insert_resource(Spawner::new(0))
            .add_systems(OnEnter(AppState::InGame), reset.in_set(LevelLoad::Sim))
            .add_systems(FixedUpdate, spawn_vehicles.in_set(SimSet::Spawn))
            .add_systems(FixedUpdate, drive_vehicles.in_set(SimSet::Move));
    }
}

fn reset(mut commands: Commands, map: Res<CityMap>) {
    commands.insert_resource(Spawner::new(map.graph.terminals.len()));
    commands.insert_resource(TrafficStats::default());
}

fn spawn_vehicles(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<CityMap>,
    mut spawner: ResMut<Spawner>,
    mut rng: ResMut<SimRng>,
    mut stats: ResMut<TrafficStats>,
    cars: Query<&Vehicle>,
    mut blocked: MessageWriter<SpawnBlocked>,
) {
    let level = map.def();
    let progress = stats.delivered as f32 / level.target as f32;
    let mut vehicles: Vec<&Vehicle> = cars.iter().collect();
    vehicles.sort_by_key(|v| v.id);
    let report = spawner.tick(
        time.delta_secs(),
        spawn_rate(level, progress),
        level,
        &map.graph,
        &map.routes,
        &vehicles,
        &mut rng.0,
    );
    for v in report.spawned {
        stats.spawned += 1;
        commands.spawn((DespawnOnExit(AppState::InGame), v));
    }
    for terminal in report.blocked {
        blocked.write(SpawnBlocked { terminal });
    }
}

fn drive_vehicles(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<CityMap>,
    signals: Res<Signals>,
    mut cars: Query<(Entity, &mut Vehicle)>,
    mut stats: ResMut<TrafficStats>,
    mut delivered: MessageWriter<Delivered>,
    mut fed_up: MessageWriter<FedUp>,
) {
    let mut items: Vec<(Entity, Mut<Vehicle>)> = cars.iter_mut().collect();
    items.sort_by_key(|(_, v)| v.id);
    let entities: Vec<Entity> = items.iter().map(|(e, _)| *e).collect();
    let mut refs: Vec<&mut Vehicle> = items.iter_mut().map(|(_, v)| &mut **v).collect();
    let events = traffic::step(&map.graph, &signals.0, &mut refs, time.delta_secs());

    for v in &refs {
        stats.max_wait = stats.max_wait.max(v.waited);
    }
    stats.overlaps += traffic::count_overlaps(&map.graph, &refs) as u32;
    for &i in &events.fed_up {
        let (front, rear) = refs[i].pose(&map.graph);
        fed_up.write(FedUp {
            pos: (front + rear) / 2.0,
        });
    }
    for &i in &events.delivered {
        let v = &refs[i];
        stats.delivered += 1;
        delivered.write(Delivered {
            kind: v.kind,
            ideal_time: v.ideal_time,
            trip_time: v.age,
            exit: v.exit,
        });
        commands.entity(entities[i]).despawn();
    }
}
