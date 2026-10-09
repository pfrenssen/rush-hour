//! Vehicle spawning at the map-edge terminals.

use std::borrow::Borrow;
use std::sync::Arc;

use super::kind::VehicleKind;
use super::traffic::{JAM_GAP, Vehicle, occupancy};
use crate::levels::LevelDef;
use crate::map::graph::{LaneGraph, LaneId, TerminalId};
use crate::map::route::exits_for;

/// Seconds a spawn point may stay blocked before the player loses a life.
pub const BLOCK_LIMIT: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct Pending {
    pub kind: VehicleKind,
    pub exit: TerminalId,
}

#[derive(bevy::prelude::Resource, Clone, Debug)]
pub struct Spawner {
    /// Seconds until the next vehicle wants to enter.
    pub cooldown: f32,
    /// Per terminal: a vehicle waiting for room to enter.
    pub pending: Vec<Option<Pending>>,
    /// Per terminal: how long the pending vehicle has been waiting.
    pub blocked_for: Vec<f32>,
    pub next_id: u32,
    pub spawned: u32,
}

#[derive(Default, Debug)]
pub struct SpawnReport {
    pub spawned: Vec<Vehicle>,
    /// Terminals that were blocked for longer than [`BLOCK_LIMIT`].
    pub blocked: Vec<TerminalId>,
}

/// Spawn rate in vehicles per minute for a given fraction of the level target delivered.
pub fn spawn_rate(level: &LevelDef, progress: f32) -> f32 {
    let (a, b) = level.spawn_per_min;
    a + (b - a) * progress.clamp(0.0, 1.0)
}

impl Spawner {
    pub fn new(terminals: usize) -> Spawner {
        Spawner {
            cooldown: 0.5,
            pending: vec![None; terminals],
            blocked_for: vec![0.0; terminals],
            next_id: 0,
            spawned: 0,
        }
    }

    pub fn tick<V: Borrow<Vehicle>>(
        &mut self,
        dt: f32,
        rate_per_min: f32,
        level: &LevelDef,
        graph: &LaneGraph,
        routes: &[Vec<Option<Vec<LaneId>>>],
        vehicles: &[V],
        rng: &mut fastrand::Rng,
    ) -> SpawnReport {
        let mut report = SpawnReport::default();

        self.cooldown -= dt;
        if self.cooldown <= 0.0 {
            let interval = 60.0 / rate_per_min.max(0.1);
            self.cooldown += interval * (0.6 + 0.8 * rng.f32());
            let free: Vec<TerminalId> = (0..self.pending.len())
                .filter(|&t| self.pending[t].is_none() && !exits_for(routes, t).is_empty())
                .collect();
            if !free.is_empty() {
                let t = free[rng.usize(..free.len())];
                let exits = exits_for(routes, t);
                self.pending[t] = Some(Pending {
                    kind: VehicleKind::pick(&level.mix, rng.f32()),
                    exit: exits[rng.usize(..exits.len())],
                });
                self.blocked_for[t] = 0.0;
            }
        }

        let occ = occupancy(graph, vehicles);
        let slots = self.pending.iter_mut().zip(self.blocked_for.iter_mut());
        for (t, (pending, blocked_for)) in slots.enumerate() {
            let Some(p) = *pending else { continue };
            let entry = graph.terminals[t].entry;
            let room = occ[entry]
                .iter()
                .map(|piece| piece.rear)
                .fold(graph.lanes[entry].length, f32::min);
            let len = p.kind.params().length;
            if room >= len + JAM_GAP + 1.0 {
                let route: Arc<[LaneId]> = routes[t][p.exit]
                    .as_ref()
                    .expect("pending exits are reachable")
                    .as_slice()
                    .into();
                let mut v = Vehicle::new(
                    self.next_id,
                    p.kind,
                    route,
                    p.exit,
                    graph,
                    0.92 + 0.16 * rng.f32(),
                    rng.u8(..),
                );
                // Enter at a speed that still allows stopping behind whatever is ahead.
                let clear = room - len - JAM_GAP;
                v.v = (v.v0 * 0.8).min((2.0 * v.params.decel * clear.max(0.0)).sqrt() * 0.7);
                self.next_id += 1;
                self.spawned += 1;
                *pending = None;
                *blocked_for = 0.0;
                report.spawned.push(v);
            } else {
                *blocked_for += dt;
                if *blocked_for > BLOCK_LIMIT {
                    *pending = None;
                    *blocked_for = 0.0;
                    report.blocked.push(t);
                }
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LEVELS;
    use crate::map::CityMap;

    #[test]
    fn spawns_vehicles_over_time() {
        let map = CityMap::load(0);
        let level = &LEVELS[0];
        let mut spawner = Spawner::new(map.graph.terminals.len());
        let mut rng = fastrand::Rng::with_seed(7);
        // Vehicles are taken off the road right away, so no spawn point is ever blocked.
        let mut vehicles: Vec<Vehicle> = Vec::new();
        let none: [Vehicle; 0] = [];
        for _ in 0..(60 * 60) {
            let report = spawner.tick(
                1.0 / 60.0,
                spawn_rate(level, 0.0),
                level,
                &map.graph,
                &map.routes,
                &none,
                &mut rng,
            );
            vehicles.extend(report.spawned);
            assert!(report.blocked.is_empty());
        }
        // About one vehicle every 60 / rate seconds.
        let expected = spawn_rate(level, 0.0);
        assert!((vehicles.len() as f32 - expected).abs() < expected * 0.35);
        // Every vehicle starts on its entry lane and leaves on another side.
        for v in &vehicles {
            let entry = map
                .graph
                .terminals
                .iter()
                .find(|t| t.entry == v.route[0])
                .expect("starts on an entry lane");
            assert_ne!(entry.side, map.graph.terminals[v.exit].side);
        }
    }

    #[test]
    fn a_blocked_spawn_point_is_reported_after_the_limit() {
        let map = CityMap::load(0);
        let level = &LEVELS[0];
        let mut spawner = Spawner::new(map.graph.terminals.len());
        let mut rng = fastrand::Rng::with_seed(3);
        // Park a vehicle right at the start of every entry lane.
        let blockers: Vec<Vehicle> = map
            .graph
            .terminals
            .iter()
            .enumerate()
            .map(|(t, term)| {
                let route: Arc<[LaneId]> = vec![term.entry].into();
                Vehicle::new(
                    1000 + t as u32,
                    VehicleKind::Car,
                    route,
                    t,
                    &map.graph,
                    1.0,
                    0,
                )
            })
            .collect();
        let mut blocked = Vec::new();
        for _ in 0..(10 * 60) {
            let report = spawner.tick(
                1.0 / 60.0,
                60.0,
                level,
                &map.graph,
                &map.routes,
                &blockers,
                &mut rng,
            );
            assert!(report.spawned.is_empty());
            blocked.extend(report.blocked);
        }
        assert!(!blocked.is_empty());
    }

    #[test]
    fn rate_ramps_with_progress() {
        let level = &LEVELS[0];
        assert!(spawn_rate(level, 1.0) > spawn_rate(level, 0.0));
        assert_eq!(spawn_rate(level, 2.0), spawn_rate(level, 1.0));
    }
}
