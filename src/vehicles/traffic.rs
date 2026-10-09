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
//!
//! On top of that, the traffic rules decide when a vehicle *wants* to commit:
//! - lights: go on green; on amber only if stopping would need hard braking; never on red;
//! - left turns yield to oncoming straight and right-turning traffic, and the first waiting
//!   left-turner may clear the junction on amber once oncoming traffic stops;
//! - don't block the box: only enter when the exit lane has room for the whole vehicle, counting
//!   every vehicle already on or headed for that lane.

use std::borrow::Borrow;
use std::sync::Arc;

use bevy::math::Vec2;

use super::kind::{KindParams, VehicleKind};
use crate::lights::{Aspect, Signal};
use crate::map::graph::{LaneGraph, LaneId, LaneKind, NodeId, STOP_SETBACK, TerminalId, Turn};

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
/// Hardest braking a driver accepts to stop for an amber light.
pub const AMBER_DECEL: f32 = 3.4;
/// A left-turner waits for oncoming vehicles that would reach the junction within this time.
pub const YIELD_GAP: f32 = 4.0;
/// Near the start of a junction, vehicles on sibling turn lanes (same entry) still count as
/// being ahead: the paths have not diverged enough to pass side by side.
const SIBLING_ZONE: f32 = 6.0;

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
    /// Committed while clearing the junction as a left-turner on amber: no backing out.
    pub clearing: bool,
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
            clearing: false,
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

/// Distance between a junction box and the stop line in front of it.
pub fn setback(graph: &LaneGraph, node: NodeId) -> f32 {
    if graph.nodes[node].is_signal() {
        STOP_SETBACK
    } else {
        BEND_SETBACK
    }
}

pub fn next_junction(graph: &LaneGraph, v: &Vehicle) -> Option<Ahead> {
    let mut dist = graph.lanes[v.lane()].length - v.s;
    for &lane in &v.route[v.idx + 1..] {
        let l = &graph.lanes[lane];
        if let LaneKind::Turn { node, .. } = l.kind {
            return Some(Ahead {
                turn: lane,
                node,
                dist: dist - setback(graph, node),
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
        let mut consider = |p: &Piece| {
            if p.veh == i || (k == v.idx && p.front <= v.s) {
                return;
            }
            let gap = base + p.rear;
            if best.is_none_or(|(g, _)| gap < g) {
                best = Some((gap, p.veh));
            }
        };
        occ[lane].iter().for_each(&mut consider);
        if let LaneKind::Turn { entry, .. } = graph.lanes[lane].kind {
            for &sibling in graph.lanes[entry].next.iter().filter(|&&l| l != lane) {
                occ[sibling]
                    .iter()
                    .filter(|p| p.rear < SIBLING_ZONE)
                    .for_each(&mut consider);
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

    fn remove(&mut self, turn: LaneId, i: usize) {
        self.0[turn].retain(|&h| h != i);
    }

    /// No conflicting turn lane is held by another vehicle that is itself clearing on amber
    /// (two of those would wait for each other forever).
    fn no_clearing_conflict(
        &self,
        graph: &LaneGraph,
        turn: LaneId,
        i: usize,
        vehicles: &[&mut Vehicle],
    ) -> bool {
        graph.lanes[turn]
            .conflicts
            .iter()
            .all(|&c| self.0[c].iter().all(|&h| h == i || !vehicles[h].clearing))
    }
}

/// Could this vehicle stop within `dist` without braking harder than drivers accept at amber?
pub fn can_stop(v: &Vehicle, dist: f32) -> bool {
    v.v < 0.3 || v.v * v.v / (2.0 * dist.max(0.01)) <= AMBER_DECEL
}

/// The light a vehicle sees at the junction ahead (bends are always green).
pub fn aspect_at(graph: &LaneGraph, signals: &[Option<Signal>], a: &Ahead) -> Aspect {
    match (signals.get(a.node), graph.lanes[a.turn].kind) {
        (Some(Some(signal)), LaneKind::Turn { from_dir, .. }) => signal.aspect(from_dir.axis()),
        _ => Aspect::Green,
    }
}

/// Space each vehicle claims on the lane it is on or headed for.
fn claim(v: &Vehicle) -> f32 {
    v.length() + JAM_GAP
}

/// Claimed length per road lane: vehicles on it, in the box before it, or committed to enter it.
fn lane_loads(graph: &LaneGraph, vehicles: &[&mut Vehicle]) -> Vec<f32> {
    let mut load = vec![0.0; graph.lanes.len()];
    for v in vehicles {
        let lane = &graph.lanes[v.lane()];
        let counted = if lane.is_turn() {
            lane.next[0]
        } else {
            v.lane()
        };
        load[counted] += claim(v);
        if let Some(t) = v.committed {
            load[graph.lanes[t].next[0]] += claim(v);
        }
    }
    load
}

/// Unclaimed length of a lane; lanes leaving the map never fill up.
pub fn room(graph: &LaneGraph, lane: LaneId, load: &[f32]) -> f32 {
    let l = &graph.lanes[lane];
    match l.end_node() {
        Some(n) => l.length - setback(graph, n) - load[lane],
        None => f32::INFINITY,
    }
}

/// Is there an oncoming (opposite approach), uncommitted, straight or right-turning vehicle
/// that matches `pred`?
fn oncoming(
    graph: &LaneGraph,
    vehicles: &[&mut Vehicle],
    ahead: &[Option<Ahead>],
    a: &Ahead,
    from_dir: crate::map::grid::Dir,
    pred: impl Fn(&Vehicle, &Ahead) -> bool,
) -> bool {
    vehicles.iter().zip(ahead).any(|(w, aj)| {
        let Some(aj) = aj else { return false };
        let LaneKind::Turn {
            turn, from_dir: dj, ..
        } = graph.lanes[aj.turn].kind
        else {
            return false;
        };
        aj.node == a.node
            && dj == from_dir.opposite()
            && turn != Turn::Left
            && w.committed.is_none()
            && pred(w, aj)
    })
}

/// A left-turner yields to oncoming vehicles that would reach the junction soon.
fn must_yield(w: &Vehicle, aj: &Ahead) -> bool {
    w.v > STOPPED_SPEED && aj.dist.max(0.0) / w.v < YIELD_GAP
}

/// An oncoming vehicle that is too close to stop for amber will still come through.
fn runs_amber(w: &Vehicle, aj: &Ahead) -> bool {
    w.v > STOPPED_SPEED && !can_stop(w, aj.dist)
}

#[derive(Default, Debug)]
pub struct StepEvents {
    /// Indices of vehicles that reached the end of their route.
    pub delivered: Vec<usize>,
    /// Indices of vehicles whose patience ran out this step.
    pub fed_up: Vec<usize>,
}

/// Advance all vehicles by `dt` seconds. `vehicles` must be in a stable order (by id) and
/// `signals` is indexed by node.
pub fn step(
    graph: &LaneGraph,
    signals: &[Option<Signal>],
    vehicles: &mut [&mut Vehicle],
    dt: f32,
) -> StepEvents {
    let n = vehicles.len();
    let occ = occupancy(graph, vehicles);
    let mut holds = Holds::build(graph, &occ, vehicles);
    let mut load = lane_loads(graph, vehicles);
    let ahead: Vec<Option<Ahead>> = vehicles.iter().map(|v| next_junction(graph, v)).collect();
    let leaders: Vec<Option<(f32, usize)>> =
        (0..n).map(|i| leader(graph, &occ, vehicles, i)).collect();

    // Junction decisions, one vehicle at a time so reservations stay exclusive.
    for i in 0..n {
        let Some(a) = ahead[i] else { continue };
        let aspect = aspect_at(graph, signals, &a);
        let exit = graph.lanes[a.turn].next[0];
        let need = claim(vehicles[i]);

        if let Some(t) = vehicles[i].committed {
            // The light changed: stop after all if that is comfortable.
            let v = &*vehicles[i];
            if aspect != Aspect::Green && !v.clearing && a.dist > 0.5 && can_stop(v, a.dist) {
                holds.remove(t, i);
                load[exit] -= need;
                vehicles[i].committed = None;
            }
            continue;
        }
        if a.dist > decision_distance(vehicles[i]) {
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
        let LaneKind::Turn { turn, from_dir, .. } = graph.lanes[a.turn].kind else {
            continue;
        };
        let v = &*vehicles[i];
        // The first left-turner waiting at the line may clear the junction on amber. It reserves
        // its turn right away, so cross traffic cannot start, and then waits at the line until
        // the last oncoming vehicles have passed.
        let clearing = aspect == Aspect::Amber && turn == Turn::Left && v.v < 1.0 && a.dist < 3.0;
        let light_ok = match aspect {
            Aspect::Green => true,
            Aspect::Amber => clearing || !can_stop(v, a.dist),
            Aspect::Red => false,
        };
        let conflicts_ok = if clearing {
            holds.no_clearing_conflict(graph, a.turn, i, vehicles)
                && !oncoming(graph, vehicles, &ahead, &a, from_dir, runs_amber)
        } else {
            holds.clear_for(graph, a.turn, i)
        };
        if !light_ok
            || room(graph, exit, &load) < need
            || !conflicts_ok
            || (turn == Turn::Left
                && aspect == Aspect::Green
                && oncoming(graph, vehicles, &ahead, &a, from_dir, must_yield))
        {
            continue;
        }
        holds.add(a.turn, i);
        load[exit] += need;
        vehicles[i].committed = Some(a.turn);
        vehicles[i].clearing = clearing;
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
        // Stop at the line unless committed; a vehicle clearing on amber also waits there
        // until the conflicting vehicles still in or heading into the box have passed.
        if let Some(a) = ahead[i]
            && (v.committed != Some(a.turn) || (v.clearing && !holds.clear_for(graph, a.turn, i)))
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
                v.clearing = false;
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

/// Oriented box covering half of a vehicle body.
#[derive(Clone, Copy, Debug)]
struct Obb {
    center: Vec2,
    axis: Vec2,
    half: Vec2,
}

impl Obb {
    fn between(a: Vec2, b: Vec2, width: f32) -> Obb {
        let d = a - b;
        let len = d.length();
        Obb {
            center: (a + b) / 2.0,
            axis: if len > 1e-6 { d / len } else { Vec2::X },
            half: Vec2::new(len / 2.0, width / 2.0),
        }
    }

    fn radius_along(&self, dir: Vec2) -> f32 {
        self.half.x * self.axis.dot(dir).abs() + self.half.y * self.axis.perp().dot(dir).abs()
    }

    /// Separating-axis test, with a small tolerance for touching boxes.
    fn overlaps(&self, other: &Obb) -> bool {
        const TOLERANCE: f32 = 0.05;
        let d = other.center - self.center;
        [self.axis, self.axis.perp(), other.axis, other.axis.perp()]
            .iter()
            .all(|&axis| {
                d.dot(axis).abs() + TOLERANCE < self.radius_along(axis) + other.radius_along(axis)
            })
    }
}

fn body_boxes(graph: &LaneGraph, v: &Vehicle) -> [Obb; 2] {
    let len = v.length();
    let front = v.point_behind(graph, 0.0);
    let mid = v.point_behind(graph, len / 2.0);
    let rear = v.point_behind(graph, len);
    let w = v.params.width;
    [Obb::between(front, mid, w), Obb::between(mid, rear, w)]
}

/// Number of vehicle pairs whose bodies overlap. Should always be zero.
pub fn count_overlaps<V: Borrow<Vehicle>>(graph: &LaneGraph, vehicles: &[V]) -> usize {
    let boxes: Vec<[Obb; 2]> = vehicles
        .iter()
        .map(|v| body_boxes(graph, v.borrow()))
        .collect();
    let mut count = 0;
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            let reach = vehicles[i].borrow().length() + vehicles[j].borrow().length();
            if boxes[i][0].center.distance(boxes[j][0].center) > reach {
                continue;
            }
            if boxes[i]
                .iter()
                .any(|a| boxes[j].iter().any(|b| a.overlaps(b)))
            {
                count += 1;
            }
        }
    }
    count
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

    pub const DT: f32 = 1.0 / 60.0;

    /// One simulation tick: lights, then vehicles. Returns the ids of delivered vehicles.
    pub fn tick(g: &LaneGraph, signals: &mut [Option<Signal>], vs: &mut Vec<Vehicle>) -> Vec<u32> {
        for s in signals.iter_mut().flatten() {
            s.tick(DT);
        }
        let mut refs: Vec<&mut Vehicle> = vs.iter_mut().collect();
        let ev = step(g, signals, &mut refs, DT);
        let mut done = Vec::new();
        for &i in ev.delivered.iter().rev() {
            done.push(vs[i].id);
            vs.remove(i);
        }
        done
    }

    pub fn run(g: &LaneGraph, vs: &mut Vec<Vehicle>, seconds: f32) -> Vec<u32> {
        let mut signals: Vec<Option<Signal>> = Vec::new();
        let mut all = Vec::new();
        for _ in 0..(seconds * 60.0) as usize {
            all.extend(tick(g, &mut signals, vs));
        }
        all
    }

    const STRAIGHT: &[&str] = &["--------------------"];

    #[test]
    fn a_lone_car_crosses_the_map() {
        let g = graph(STRAIGHT);
        let mut vs = vec![vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E)];
        let delivered = run(&g, &mut vs, 30.0);
        assert_eq!(delivered, vec![0]);
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

    // ---- Traffic rules -------------------------------------------------------------------

    use crate::lights::Signal;
    use crate::map::grid::Axis;

    /// One signalled crossroads with 6-tile approaches.
    const CROSS: &[&str] = &[
        "......|......",
        "......|......",
        "......|......",
        "......|......",
        "......|......",
        "......|......",
        "------+------",
        "......|......",
        "......|......",
        "......|......",
        "......|......",
        "......|......",
        "......|......",
    ];

    fn signals(g: &LaneGraph, green: Axis) -> Vec<Option<Signal>> {
        g.nodes
            .iter()
            .map(|n| n.is_signal().then(|| Signal::new(green)))
            .collect()
    }

    /// Place a vehicle `dist` metres before its first stop line, at speed `v`.
    fn before_line(g: &LaneGraph, mut veh: Vehicle, dist: f32, v: f32) -> Vehicle {
        let a = next_junction(g, &veh).unwrap();
        veh.s += a.dist - dist;
        veh.v = v;
        veh
    }

    fn in_box(g: &LaneGraph, v: &Vehicle) -> bool {
        g.lanes[v.lane()].is_turn()
    }

    #[test]
    fn stops_at_red_and_goes_on_green() {
        let g = graph(CROSS);
        let mut sig = signals(&g, Axis::NorthSouth);
        let mut vs = vec![vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E)];
        for _ in 0..(20 * 60) {
            tick(&g, &mut sig, &mut vs);
            assert!(!in_box(&g, &vs[0]), "ran a red light");
        }
        let a = next_junction(&g, &vs[0]).unwrap();
        assert!(vs[0].v < 0.05);
        assert!(
            (0.0..1.5).contains(&a.dist),
            "stopped {} m before the line",
            a.dist
        );

        sig[0].as_mut().unwrap().request_switch();
        let mut delivered = Vec::new();
        for _ in 0..(20 * 60) {
            delivered.extend(tick(&g, &mut sig, &mut vs));
        }
        assert_eq!(delivered, vec![0]);
    }

    #[test]
    fn amber_stops_vehicles_that_can_stop_and_lets_the_others_clear() {
        let g = graph(CROSS);
        let mut sig = signals(&g, Axis::EastWest);
        let near = before_line(
            &g,
            vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E),
            12.0,
            12.5,
        );
        let far = before_line(
            &g,
            vehicle(&g, 1, VehicleKind::Car, Dir::E, Dir::W),
            45.0,
            12.5,
        );
        let mut vs = vec![near, far];
        tick(&g, &mut sig, &mut vs);
        assert!(vs[0].committed.is_some());
        sig[0].as_mut().unwrap().request_switch();
        let mut near_entered = false;
        for _ in 0..(10 * 60) {
            tick(&g, &mut sig, &mut vs);
            if vs.iter().any(|v| v.id == 0 && in_box(&g, v)) {
                near_entered = true;
            }
            let far = vs.iter().find(|v| v.id == 1).unwrap();
            assert!(!in_box(&g, far), "the far vehicle should have stopped");
        }
        assert!(
            near_entered,
            "the near vehicle should have cleared the junction"
        );
        let far = vs.iter().find(|v| v.id == 1).unwrap();
        assert!(far.v < 0.05);
    }

    #[test]
    fn left_turn_yields_to_oncoming_traffic() {
        let g = graph(CROSS);
        let mut sig = signals(&g, Axis::EastWest);
        // Westbound-from-the-west vehicle turning left (north), waiting at the line.
        let turner = before_line(
            &g,
            vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::N),
            0.5,
            0.0,
        );
        // Oncoming car going straight, 30 m away at full speed.
        let oncoming = before_line(
            &g,
            vehicle(&g, 1, VehicleKind::Car, Dir::E, Dir::W),
            30.0,
            12.5,
        );
        let mut vs = vec![turner, oncoming];
        let mut oncoming_entered_at = None;
        let mut turner_entered_at = None;
        for step in 0..(15 * 60) {
            tick(&g, &mut sig, &mut vs);
            for v in &vs {
                if in_box(&g, v) {
                    let slot = if v.id == 0 {
                        &mut turner_entered_at
                    } else {
                        &mut oncoming_entered_at
                    };
                    slot.get_or_insert(step);
                }
            }
        }
        let (Some(o), Some(t)) = (oncoming_entered_at, turner_entered_at) else {
            panic!("both vehicles should pass: {oncoming_entered_at:?} {turner_entered_at:?}");
        };
        assert!(t > o, "the left-turner went first");
    }

    #[test]
    fn waiting_left_turner_clears_on_amber() {
        let g = graph(CROSS);
        let mut sig = signals(&g, Axis::EastWest);
        let mut vs = Vec::new();
        let mut next_id = 1;
        let mut entered_at = None;
        for step in 0..(25 * 60) {
            // The left-turner arrives once a steady oncoming stream is established.
            if step == 5 * 60 {
                let turner = vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::N);
                vs.push(before_line(&g, turner, 0.5, 0.0));
            }
            // Oncoming cars two seconds apart for the first ten seconds.
            if step < 10 * 60 && step % 120 == 0 {
                let mut v = vehicle(&g, next_id, VehicleKind::Car, Dir::E, Dir::W);
                v.v = 12.0;
                vs.push(v);
                next_id += 1;
            }
            if step == 10 * 60 {
                sig[0].as_mut().unwrap().request_switch();
            }
            tick(&g, &mut sig, &mut vs);
            if entered_at.is_none() && vs.iter().any(|v| v.id == 0 && in_box(&g, v)) {
                entered_at = Some(step);
            }
            assert_eq!(count_overlaps(&g, &vs), 0);
        }
        let entered = entered_at.expect("the left-turner never got through");
        assert!(
            entered >= 10 * 60,
            "no gap in the stream, so it must wait for amber"
        );
        assert!(
            entered < 15 * 60,
            "it should clear right after the switch, entered at step {entered}"
        );
    }

    #[test]
    fn dont_block_the_box() {
        // Two junctions; the second one holds east-west traffic at red.
        let g = graph(&[
            ".....|.....|.....",
            ".....|.....|.....",
            ".....|.....|.....",
            ".....|.....|.....",
            "-----+-----+-----",
            ".....|.....|.....",
            ".....|.....|.....",
            ".....|.....|.....",
            ".....|.....|.....",
        ]);
        let first = g.nodes.iter().position(|n| n.col == 5).unwrap();
        let second = g.nodes.iter().position(|n| n.col == 11).unwrap();
        let mut sig = signals(&g, Axis::EastWest);
        sig[second].as_mut().unwrap().request_switch();
        let mut vs: Vec<Vehicle> = Vec::new();
        let west = terminal(&g, Dir::W);
        let east = terminal(&g, Dir::E);
        for step in 0..(90 * 60) {
            if step % 90 == 0 {
                let entry = g.terminals[west].entry;
                let clear = vs.iter().all(|v| v.lane() != entry || v.s > 12.0);
                if clear {
                    vs.push(vehicle(&g, step as u32, VehicleKind::Car, Dir::W, Dir::E));
                }
            }
            tick(&g, &mut sig, &mut vs);
            for v in &vs {
                let lane = &g.lanes[v.lane()];
                if let LaneKind::Turn { node, .. } = lane.kind
                    && node == first
                {
                    assert!(v.v > 0.1 || step < 60, "vehicle {} stuck in the box", v.id);
                }
            }
        }
        // The queue between the junctions filled up, and nobody stands in the first box.
        let middle = g.lanes.iter().position(|l| {
            matches!(l.kind, LaneKind::Road { dir: Dir::E, from: crate::map::graph::Endpoint::Node(a), .. } if a == first)
        });
        let middle = middle.unwrap();
        let queued = vs.iter().filter(|v| v.lane() == middle).count();
        assert!(queued >= 4, "only {queued} vehicles queued");
        assert!(vs.iter().all(|v| !in_box(&g, v)));
        let _ = east;
    }

    /// The whole system on every level, with lights switched at random: no two vehicles may ever
    /// overlap, and traffic must keep flowing.
    #[test]
    fn car_following_never_overlaps_on_any_level() {
        use crate::map::CityMap;
        use crate::vehicles::spawn::{Spawner, spawn_rate};

        for level in 0..crate::levels::LEVELS.len() {
            let map = CityMap::load(level);
            let g = &map.graph;
            let def = map.def();
            let mut sig = signals(g, Axis::EastWest);
            let mut spawner = Spawner::new(g.terminals.len());
            let mut rng = fastrand::Rng::with_seed(42 + level as u64);
            let mut vs: Vec<Vehicle> = Vec::new();
            let mut delivered = 0;
            for _ in 0..(90 * 60) {
                for s in sig.iter_mut().flatten() {
                    if rng.f32() < DT / 6.0 {
                        s.request_switch();
                    }
                }
                let report =
                    spawner.tick(DT, spawn_rate(def, 0.5), def, g, &map.routes, &vs, &mut rng);
                vs.extend(report.spawned);
                delivered += tick(g, &mut sig, &mut vs).len();
                let overlaps = count_overlaps(g, &vs);
                assert_eq!(overlaps, 0, "{}: {overlaps} overlapping vehicles", def.name);
            }
            assert!(delivered > 20, "{}: only {delivered} delivered", def.name);
        }
    }

    #[test]
    fn overlap_detector_sees_overlaps() {
        let g = graph(STRAIGHT);
        let a = vehicle(&g, 0, VehicleKind::Car, Dir::W, Dir::E);
        let mut b = a.clone();
        b.id = 1;
        b.s += 2.0;
        assert_eq!(count_overlaps(&g, &[a.clone(), b.clone()]), 1);
        b.s += a.length() + 0.2;
        assert_eq!(count_overlaps(&g, &[a, b]), 0);
    }
}
