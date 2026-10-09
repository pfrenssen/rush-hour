//! A simple built-in light controller: at every junction, switch towards the longer queue.
//!
//! Used by headless runs and available in the windowed game with `--autopilot`.

use bevy::prelude::*;

use crate::lights::{Phase, Signals, SwitchRequest};
use crate::map::CityMap;
use crate::map::graph::{LaneGraph, LaneKind};
use crate::map::grid::Axis;
use crate::sim::SimSet;
use crate::vehicles::traffic::{Vehicle, next_junction};

/// A switch costs two seconds of nobody moving, so keep each green at least this long.
pub const MIN_GREEN: f32 = 5.0;
/// Even a busy direction eventually yields to waiting traffic.
pub const MAX_GREEN: f32 = 30.0;
/// Only vehicles this close to the stop line count as queued.
const QUEUE_RANGE: f32 = 45.0;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Autopilot(pub bool);

pub struct AutopilotPlugin;

impl Plugin for AutopilotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Autopilot>().add_systems(
            FixedUpdate,
            control
                .run_if(|a: Res<Autopilot>| a.0)
                .in_set(SimSet::Control),
        );
    }
}

/// Queue pressure per node and axis: vehicles waiting or approaching, weighted by impatience.
pub fn demand<'a>(graph: &LaneGraph, vehicles: impl Iterator<Item = &'a Vehicle>) -> Vec<[f32; 2]> {
    let mut demand = vec![[0.0; 2]; graph.nodes.len()];
    for v in vehicles {
        let Some(a) = next_junction(graph, v) else {
            continue;
        };
        if v.committed.is_some() || a.dist > QUEUE_RANGE {
            continue;
        }
        let LaneKind::Turn { from_dir, .. } = graph.lanes[a.turn].kind else {
            continue;
        };
        let weight = 1.0 + 2.0 * (1.0 - v.patience);
        demand[a.node][axis_index(from_dir.axis())] += weight;
    }
    demand
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::NorthSouth => 0,
        Axis::EastWest => 1,
    }
}

/// Should a signal that has been green for `green_for` seconds switch?
pub fn should_switch(green: f32, red: f32, green_for: f32) -> bool {
    if green_for < MIN_GREEN || red <= 0.0 {
        return false;
    }
    green_for >= MAX_GREEN || green == 0.0 || red > green * 1.3 + 1.0
}

fn control(
    map: Res<CityMap>,
    signals: Res<Signals>,
    vehicles: Query<&Vehicle>,
    mut requests: MessageWriter<SwitchRequest>,
) {
    let demand = demand(&map.graph, vehicles.iter());
    for (node, signal) in signals.0.iter().enumerate() {
        let Some(signal) = signal else { continue };
        let Phase::Green(axis) = signal.phase else {
            continue;
        };
        let green = demand[node][axis_index(axis)];
        let red = demand[node][axis_index(axis.other())];
        if should_switch(green, red, signal.green_for) {
            requests.write(SwitchRequest(node));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_towards_the_longer_queue() {
        assert!(should_switch(1.0, 5.0, MIN_GREEN));
        assert!(!should_switch(5.0, 1.0, MIN_GREEN));
        assert!(should_switch(0.0, 1.0, MIN_GREEN));
    }

    #[test]
    fn respects_minimum_and_maximum_green() {
        assert!(!should_switch(0.0, 10.0, MIN_GREEN - 0.1));
        assert!(should_switch(10.0, 1.0, MAX_GREEN));
        assert!(!should_switch(10.0, 0.0, MAX_GREEN));
    }
}
