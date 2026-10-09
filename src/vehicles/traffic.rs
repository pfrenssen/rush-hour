//! The driving model, in plain Rust.
//!
//! Vehicles follow their route lane by lane. Longitudinal motion uses the Intelligent Driver
//! Model (smooth acceleration, braking and safe gaps), plus a hard clamp so a vehicle can never
//! move into the space of the vehicle ahead, whatever the time step.
//!
//! Junction boxes are protected by reservations: before crossing its stop line a vehicle must
//! *commit* to its turn lane, which is only possible while no conflicting turn lane is reserved
//! or occupied. A vehicle that cannot commit treats the stop line as a wall. Vehicles keep their
//! reservation until their tail has left the box. This makes junction collisions impossible by
//! construction, independent of the lights.

use std::borrow::Borrow;
use std::sync::Arc;

use super::kind::{KindParams, VehicleKind};
use crate::map::graph::{LaneGraph, LaneId, NodeId, STOP_SETBACK, TerminalId};

/// Standstill gap kept to the vehicle ahead (IDM `s0`).
pub const JAM_GAP: f32 = 2.0;
/// Desired time headway (IDM `T`).
pub const HEADWAY: f32 = 1.1;
/// Bumper gap enforced by clamping, as a last line of defence.
pub const HARD_GAP: f32 = 0.5;
/// Hardest possible braking.
pub const MAX_DECEL: f32 = 8.0;
/// Deceleration used to slow down for turns ahead.
const CURVE_DECEL: f32 = 2.0;
/// Distance kept to a stop line when standing in front of it.
const STOP_GAP: f32 = 0.3;
/// How far ahead a driver looks for vehicles, junctions and turns.
pub const LOOKAHEAD: f32 = 80.0;
/// Below this speed a vehicle counts as standing still.
pub const STOPPED_SPEED: f32 = 0.5;
/// Unsignalled bends have their "stop line" right at the box edge.
const BEND_SETBACK: f32 = 0.3;
/// Patience regained per second of driving, relative to how fast it drains.
const PATIENCE_RECOVERY: f32 = 0.25;

#[derive(bevy::prelude::Component, Clone, Debug)]
pub struct Vehicle {
    pub id: u32,
    pub kind: VehicleKind,
    pub params: KindParams,
    /// Desired speed (the type's cruising speed with a little personal variation).
    pub v0: f32,
    pub route: Arc<[LaneId]>,
    #[allow(dead_code)]
    pub entry: TerminalId,
    pub exit: TerminalId,
    /// Index into `route` of the lane holding the front bumper.
    pub idx: usize,
    /// Distance of the front bumper along `route[idx]`.
    pub s: f32,
    pub v: f32,
    pub acc: f32,
    /// Turn lane this vehicle has reserved but not yet entered.
    pub committed: Option<LaneId>,
    /// 1.0 is calm, 0.0 means the player loses a life.
    pub patience: f32,
    /// Total seconds spent standing still.
    pub waited: f32,
    /// Seconds since spawning.
    pub age: f32,
    /// Trip time on empty roads, for scoring.
    pub ideal_time: f32,
    /// Reached the end of its route.
    pub done: bool,
    /// Cosmetic colour index.
    pub paint: u8,
}

impl Vehicle {
    pub fn new(
        id: u32,
        kind: VehicleKind,
        route: Arc<[LaneId]>,
        entry: TerminalId,
        exit: TerminalId,
        graph: &LaneGraph,
        speed_factor: f32,
        paint: u8,
    ) -> Vehicle {
        let params = kind.params();
        let v0 = params.speed * speed_factor;
        let ideal_time = route
            .iter()
            .map(|&l| graph.lanes[l].length / v0.min(graph.lanes[l].speed_limit))
            .sum();
        Vehicle {
            id,
            kind,
            params,
            v0,
            route,
            entry,
            exit,
            idx: 0,
            s: params.length,
            v: 0.0,
            acc: 0.0,
            committed: None,
            patience: 1.0,
            waited: 0.0,
            age: 0.0,
            ideal_time,
            done: false,
            paint,
        }
    }

    pub fn length(&self) -> f32 {
        self.params.length
    }

    pub fn lane(&self) -> LaneId {
        self.route[self.idx]
    }

    /// Point `back` metres behind the front bumper, following the route backwards.
    pub fn point_behind(&self, graph: &LaneGraph, back: f32) -> bevy::math::Vec2 {
        let mut idx = self.idx;
        let mut s = self.s - back;
        while s < 0.0 && idx > 0 {
            idx -= 1;
            s += graph.lanes[self.route[idx]].length;
        }
        graph.lanes[self.route[idx]].geom.point_at(s)
    }

    /// Front and rear bumper positions in world space.
    pub fn pose(&self, graph: &LaneGraph) -> (bevy::math::Vec2, bevy::math::Vec2) {
        (
            self.point_behind(graph, 0.0),
            self.point_behind(graph, self.length()),
        )
    }

    pub fn is_stopped(&self) -> bool {
        self.v < STOPPED_SPEED
    }
}

/// The part of a vehicle's body that lies on one lane.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub veh: usize,
    pub front: f32,
    pub rear: f32,
}

/// Body pieces per lane.
pub fn occupancy<V: Borrow<Vehicle>>(graph: &LaneGraph, vehicles: &[V]) -> Vec<Vec<Piece>> {
    let mut occ = vec![Vec::new(); graph.lanes.len()];
    for (i, v) in vehicles.iter().enumerate() {
        let v = v.borrow();
        let mut remaining = v.length();
        let mut k = v.idx;
        let mut front = v.s;
        loop {
            let rear = front - remaining;
            if rear >= 0.0 || k == 0 {
                occ[v.route[k]].push(Piece {
                    veh: i,
                    front,
                    rear,
                });
                break;
            }
            occ[v.route[k]].push(Piece {
                veh: i,
                front,
                rear: 0.0,
            });
            remaining -= front;
            k -= 1;
            front = graph.lanes[v.route[k]].length;
        }
    }
    occ
}

/// The next junction on a vehicle's route that its front has not entered yet.
#[derive(Clone, Copy, Debug)]
pub struct Ahead {
    pub turn: LaneId,
    pub node: NodeId,
    /// Distance from the front bumper to where the vehicle must stop (negative once past it).
    pub dist: f32,
}

pub fn next_junction(graph: &LaneGraph, v: &Vehicle) -> Option<Ahead> {
    let mut dist = graph.lanes[v.lane()].length - v.s;
    for &lane in &v.route[v.idx + 1..] {
        let l = &graph.lanes[lane];
        if let crate::map::graph::LaneKind::Turn { node, .. } = l.kind {
            let setback = if graph.nodes[node].is_signal() {
                STOP_SETBACK
            } else {
                BEND_SETBACK
            };
            return Some(Ahead {
                turn: lane,
                node,
                dist: dist - setback,
            });
        }
        dist += l.length;
        if dist > LOOKAHEAD {
            break;
        }
    }
    None
}

/// Nearest body piece of another vehicle ahead along `v`'s route: `(gap, vehicle index)`.
pub fn leader<V: Borrow<Vehicle>>(
    graph: &LaneGraph,
    occ: &[Vec<Piece>],
    vehicles: &[V],
    i: usize,
) -> Option<(f32, usize)> {
    let v = vehicles[i].borrow();
    let mut base = -v.s;
    for (k, &lane) in v.route.iter().enumerate().skip(v.idx) {
        let mut best: Option<(f32, usize)> = None;
        for p in &occ[lane] {
            if p.veh == i || (k == v.idx && p.front <= v.s) {
                continue;
            }
            let gap = base + p.rear;
            if best.is_none_or(|(g, _)| gap < g) {
                best = Some((gap, p.veh));
            }
        }
        if let Some((gap, j)) = best {
            return Some((gap.max(0.0), j));
        }
        base += graph.lanes[lane].length;
        if base > LOOKAHEAD {
            break;
        }
    }
    None
}

/// Desired speed, lowered in time for slow turns ahead.
fn desired_speed(graph: &LaneGraph, v: &Vehicle) -> f32 {
    let mut v0 = v.v0.min(graph.lanes[v.lane()].speed_limit);
    let mut dist = graph.lanes[v.lane()].length - v.s;
    for &lane in &v.route[v.idx + 1..] {
        let limit = graph.lanes[lane].speed_limit;
        v0 = v0.min((limit * limit + 2.0 * CURVE_DECEL * dist.max(0.0)).sqrt());
        dist += graph.lanes[lane].length;
        if dist > 40.0 {
            break;
        }
    }
    v0
}

/// IDM free-road term.
fn free_accel(v: f32, v0: f32, p: &KindParams) -> f32 {
    (p.accel * (1.0 - (v / v0.max(0.1)).powi(4))).max(-p.decel)
}

/// IDM interaction term for an obstacle `gap` metres ahead closing at `dv` m/s.
fn interaction(v: f32, gap: f32, dv: f32, s0: f32, p: &KindParams) -> f32 {
    let s_star = s0 + (v * HEADWAY + v * dv / (2.0 * (p.accel * p.decel).sqrt())).max(0.0);
    -p.accel * (s_star / gap.max(0.01)).powi(2)
}

/// Distance before its stop line at which a vehicle decides whether it may enter the box.
pub fn decision_distance(v: &Vehicle) -> f32 {
    v.v * v.v / (2.0 * v.params.decel) + 4.0 + v.v * 0.6
}

/// Who may use each turn lane: vehicles on it and vehicles committed to it.
struct Holds(Vec<Vec<usize>>);

impl Holds {
    fn build<V: Borrow<Vehicle>>(graph: &LaneGraph, occ: &[Vec<Piece>], vehicles: &[V]) -> Holds {
        let mut holds = vec![Vec::new(); graph.lanes.len()];
        for (lane, pieces) in occ.iter().enumerate() {
            if graph.lanes[lane].is_turn() {
                for p in pieces {
                    if !holds[lane].contains(&p.veh) {
                        holds[lane].push(p.veh);
                    }
                }
            }
        }
        for (i, v) in vehicles.iter().enumerate() {
            if let Some(t) = v.borrow().committed
                && !holds[t].contains(&i)
            {
                holds[t].push(i);
            }
        }
        Holds(holds)
    }

    /// No other vehicle holds a turn lane that conflicts with `turn`.
    fn clear_for(&self, graph: &LaneGraph, turn: LaneId, i: usize) -> bool {
        graph.lanes[turn]
            .conflicts
            .iter()
            .all(|&c| self.0[c].iter().all(|&h| h == i))
    }

    fn add(&mut self, turn: LaneId, i: usize) {
        if !self.0[turn].contains(&i) {
            self.0[turn].push(i);
        }
    }
}

#[derive(Default, Debug)]
pub struct StepEvents {
    /// Indices of vehicles that reached the end of their route.
    pub delivered: Vec<usize>,
    /// Indices of vehicles whose patience ran out this step.
    pub fed_up: Vec<usize>,
}

/// Advance all vehicles by `dt` seconds. `vehicles` must be in a stable order (by id).
pub fn step(graph: &LaneGraph, vehicles: &mut [&mut Vehicle], dt: f32) -> StepEvents {
    let n = vehicles.len();
    let occ = occupancy(graph, vehicles);
    let mut holds = Holds::build(graph, &occ, vehicles);
    let ahead: Vec<Option<Ahead>> = vehicles.iter().map(|v| next_junction(graph, v)).collect();
    let leaders: Vec<Option<(f32, usize)>> =
        (0..n).map(|i| leader(graph, &occ, vehicles, i)).collect();

    // Junction decisions, one vehicle at a time so reservations stay exclusive.
    for i in 0..n {
        let Some(a) = ahead[i] else { continue };
        if vehicles[i].committed.is_some() || a.dist > decision_distance(&*vehicles[i]) {
            continue;
        }
        // Only the first vehicle waiting at a stop line may commit.
        if let Some((gap, j)) = leaders[i]
            && gap <= a.dist + 1.0
            && ahead[j].is_some_and(|aj| aj.node == a.node)
            && vehicles[j].committed.is_none()
        {
            continue;
        }
        if holds.clear_for(graph, a.turn, i) {
            holds.add(a.turn, i);
            vehicles[i].committed = Some(a.turn);
        }
    }

    // Accelerations from the same snapshot for everyone.
    let mut plans = Vec::with_capacity(n);
    for i in 0..n {
        let v = &*vehicles[i];
        let p = &v.params;
        let v0 = desired_speed(graph, v);
        let mut acc = free_accel(v.v, v0, p);
        let mut worst = 0.0f32;
        let mut limit = f32::INFINITY;
        if let Some((gap, j)) = leaders[i] {
            worst = worst.min(interaction(v.v, gap, v.v - vehicles[j].v, JAM_GAP, p));
            limit = limit.min((gap - HARD_GAP).max(0.0));
        }
        if let Some(a) = ahead[i]
            && v.committed != Some(a.turn)
        {
            worst = worst.min(interaction(v.v, a.dist, v.v, STOP_GAP, p));
            limit = limit.min(a.dist.max(0.0));
        }
        acc = (acc + worst).clamp(-MAX_DECEL, p.accel);
        plans.push((acc, limit));
    }

    let mut events = StepEvents::default();
    for (i, (acc, limit)) in plans.into_iter().enumerate() {
        let v = &mut *vehicles[i];
        let mut v_new = v.v + acc * dt;
        let mut ds = if v_new < 0.0 {
            v_new = 0.0;
            if acc < 0.0 {
                v.v * v.v / (-2.0 * acc)
            } else {
                0.0
            }
        } else {
            v.v * dt + 0.5 * acc * dt * dt
        };
        if ds > limit {
            ds = limit;
            v_new = v_new.min(limit / dt);
        }
        v.acc = acc;
        v.v = v_new;
        v.s += ds;
        v.age += dt;
        while v.s >= graph.lanes[v.lane()].length {
            if v.idx + 1 == v.route.len() {
                v.done = true;
                break;
            }
            v.s -= graph.lanes[v.lane()].length;
            v.idx += 1;
            if v.committed == Some(v.lane()) {
                v.committed = None;
            }
        }
        if v.done {
            events.delivered.push(i);
            continue;
        }

        let drain = 1.0 / v.params.patience;
        if v.is_stopped() {
            v.waited += dt;
            v.patience -= drain * dt;
            if v.patience <= 0.0 {
                v.patience = 1.0;
                events.fed_up.push(i);
            }
        } else {
            v.patience = (v.patience + drain * PATIENCE_RECOVERY * dt).min(1.0);
        }
    }
    events
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::map::grid::{Dir, Grid};
    use crate::map::route::shortest_route;

    pub fn graph(rows: &[&str]) -> LaneGraph {
        LaneGraph::build(&Grid::parse(rows).unwrap())
    }

    pub fn terminal(g: &LaneGraph, side: Dir) -> usize {
        g.terminals.iter().position(|t| t.side == side).unwrap()
    }

    pub fn vehicle(g: &LaneGraph, id: u32, kind: VehicleKind, from: Dir, to: Dir) -> Vehicle {
        let a = terminal(g, from);
        let b = terminal(g, to);
        let route = shortest_route(g, g.terminals[a].entry, g.terminals[b].exit).unwrap();
        Vehicle::new(id, kind, route.into(), a, b, g, 1.0, 0)
    }

    pub fn run(g: &LaneGraph, vs: &mut Vec<Vehicle>, seconds: f32) -> StepEvents {
        let mut all = StepEvents::default();
        let steps = (seconds * 60.0) as usize;
        for _ in 0..steps {
            let mut refs: Vec<&mut Vehicle> = vs.iter_mut().collect();
            let ev = step(g, &mut refs, 1.0 / 60.0);
            for &i in ev.delivered.iter().rev() {
                all.delivered.push(vs[i].id as usize);
                vs.remove(i);
            }
            all.fed_up.extend(ev.fed_up);
        }
        all
    }

    const STRAIGHT: &[&str] = &["--------------------"];

    #[test]
    fn a_lone_car_crosses_the_map() {
        let g = graph(STRAIGHT);
        let mut vs = vec![vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E)];
        let ev = run(&g, &mut vs, 30.0);
        assert_eq!(ev.delivered, vec![0]);
        assert!(vs.is_empty());
    }

    #[test]
    fn accelerates_smoothly_to_cruising_speed() {
        let g = graph(&["------------------------------------------------------------"]);
        let mut vs = vec![vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E)];
        let mut prev = 0.0;
        for _ in 0..(8 * 60) {
            run(&g, &mut vs, 1.0 / 60.0);
            let v = vs[0].v;
            assert!(v >= prev - 1e-4, "speed dropped from {prev} to {v}");
            assert!(v - prev < 0.1, "jerky acceleration");
            prev = v;
        }
        assert!(prev > 0.9 * vs[0].v0);
    }

    #[test]
    fn follower_keeps_its_distance_behind_a_stopped_car() {
        let g = graph(&["------------------------------"]);
        let mut lead = vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E);
        lead.s = 120.0;
        let mut follower = vehicle(&g, 1, VehicleKind::Car, Dir::W, Dir::E);
        follower.v = 12.0;
        let mut vs = vec![lead, follower];
        for _ in 0..(20 * 60) {
            run(&g, &mut vs, 1.0 / 60.0);
            // The leader is broken down.
            vs[0].s = 120.0;
            vs[0].v = 0.0;
        }
        let gap = vs[0].s - vs[0].length() - vs[1].s;
        assert!(vs[1].v < 0.05);
        assert!((HARD_GAP..JAM_GAP + 1.0).contains(&gap), "gap {gap}");
    }
}
