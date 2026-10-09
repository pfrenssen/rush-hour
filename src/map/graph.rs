//! Lane graph built from a [`Grid`].
//!
//! Every road between two graph nodes (junctions, bends or map-edge terminals) carries two
//! directed road lanes, offset to the right of the centre line (right-hand traffic). Inside each
//! junction, turn lanes connect every incoming road lane to every outgoing one except the U-turn.
//! Turn lanes whose paths come too close to each other are marked as conflicting.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::math::Vec2;

use super::geom::Geom;
use super::grid::{Dir, Grid, TILE};

pub type LaneId = usize;
pub type NodeId = usize;
pub type TerminalId = usize;

/// Half the width of a cell / junction box.
pub const HALF: f32 = TILE / 2.0;
/// Distance of a lane centre line from the road centre line.
pub const LANE_OFFSET: f32 = TILE / 4.0;
/// Entry and exit lanes reach this far beyond the map edge so vehicles slide in and out of view.
pub const EDGE_EXTENSION: f32 = TILE;
/// Vehicles stop this far before the junction box, leaving room for a zebra crossing.
pub const STOP_SETBACK: f32 = 2.5;
/// Two turn lanes conflict when their centre lines come closer than this.
pub const CONFLICT_DISTANCE: f32 = 2.9;
pub const ROAD_SPEED: f32 = 13.0;
/// Lateral acceleration used to derive turn speed limits from the turn radius.
const LATERAL_ACCEL: f32 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Turn {
    Straight,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endpoint {
    Node(NodeId),
    Edge(TerminalId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaneKind {
    Road {
        dir: Dir,
        from: Endpoint,
        to: Endpoint,
    },
    Turn {
        node: NodeId,
        turn: Turn,
        /// Direction of travel when entering the junction.
        from_dir: Dir,
        entry: LaneId,
        exit: LaneId,
    },
}

#[derive(Clone, Debug)]
pub struct Lane {
    pub geom: Geom,
    pub length: f32,
    pub kind: LaneKind,
    pub next: Vec<LaneId>,
    pub speed_limit: f32,
    /// Turn lanes (at the same junction) that may not be used at the same time.
    pub conflicts: Vec<LaneId>,
}

impl Lane {
    pub fn is_turn(&self) -> bool {
        matches!(self.kind, LaneKind::Turn { .. })
    }

    pub fn turn(&self) -> Option<Turn> {
        match self.kind {
            LaneKind::Turn { turn, .. } => Some(turn),
            LaneKind::Road { .. } => None,
        }
    }

    /// Node at the end of a road lane, if it ends at a junction rather than the map edge.
    pub fn end_node(&self) -> Option<NodeId> {
        match self.kind {
            LaneKind::Road {
                to: Endpoint::Node(n),
                ..
            } => Some(n),
            _ => None,
        }
    }

    pub fn dir(&self) -> Option<Dir> {
        match self.kind {
            LaneKind::Road { dir, .. } => Some(dir),
            LaneKind::Turn { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Three or more approaches: controlled by traffic lights.
    Signal,
    /// A corner with two approaches: no lights, vehicles just follow the road.
    Bend,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub col: usize,
    pub row: usize,
    pub center: Vec2,
    pub kind: NodeKind,
    /// Road lanes that end at this node.
    pub incoming: Vec<LaneId>,
    /// Road lanes that start at this node.
    pub outgoing: Vec<LaneId>,
    pub turns: Vec<LaneId>,
}

impl Node {
    pub fn is_signal(&self) -> bool {
        self.kind == NodeKind::Signal
    }
}

#[derive(Clone, Debug)]
pub struct Terminal {
    /// The map side this terminal sits on.
    pub side: Dir,
    pub col: usize,
    pub row: usize,
    /// Lane that brings vehicles into the map.
    pub entry: LaneId,
    /// Lane that takes vehicles off the map.
    pub exit: LaneId,
    /// Point on the map edge at the road centre.
    pub pos: Vec2,
}

#[derive(Clone, Debug)]
pub struct LaneGraph {
    pub lanes: Vec<Lane>,
    pub nodes: Vec<Node>,
    pub terminals: Vec<Terminal>,
}

impl LaneGraph {
    pub fn build(grid: &Grid) -> LaneGraph {
        let mut nodes = Vec::new();
        let mut node_at = HashMap::new();
        for row in 0..grid.height {
            for col in 0..grid.width {
                if grid.is_node(col, row) {
                    let kind = if grid.openings(col, row).len() >= 3 {
                        NodeKind::Signal
                    } else {
                        NodeKind::Bend
                    };
                    node_at.insert((col, row), nodes.len());
                    nodes.push(Node {
                        col,
                        row,
                        center: grid.cell_center(col, row),
                        kind,
                        incoming: Vec::new(),
                        outgoing: Vec::new(),
                        turns: Vec::new(),
                    });
                }
            }
        }

        // Terminals: straight road cells on the border that open off the map.
        let mut terminals: Vec<Terminal> = Vec::new();
        let mut terminal_at = HashMap::new();
        for row in 0..grid.height {
            for col in 0..grid.width {
                if grid.cell(col, row) == super::grid::Cell::Junction {
                    continue;
                }
                for side in Dir::ALL {
                    if grid.opens(col, row, side) && grid.neighbour(col, row, side).is_none() {
                        terminal_at.insert((col, row, side), terminals.len());
                        terminals.push(Terminal {
                            side,
                            col,
                            row,
                            entry: usize::MAX,
                            exit: usize::MAX,
                            pos: grid.cell_center(col, row) + side.vec() * HALF,
                        });
                    }
                }
            }
        }

        // Road lanes: one directed lane per (start endpoint, outward direction).
        let mut lanes: Vec<Lane> = Vec::new();
        let mut starts: Vec<(Endpoint, usize, usize, Dir)> = Vec::new();
        for (id, n) in nodes.iter().enumerate() {
            for d in grid.openings(n.col, n.row) {
                starts.push((Endpoint::Node(id), n.col, n.row, d));
            }
        }
        for (id, t) in terminals.iter().enumerate() {
            starts.push((Endpoint::Edge(id), t.col, t.row, t.side.opposite()));
        }
        for (from, col, row, dir) in starts {
            let right = dir.right().vec() * LANE_OFFSET;
            let start = match from {
                Endpoint::Node(_) => grid.cell_center(col, row) + dir.vec() * HALF + right,
                Endpoint::Edge(_) => {
                    grid.cell_center(col, row) - dir.vec() * (HALF + EDGE_EXTENSION) + right
                }
            };
            // Walk along the road until the next node or the map edge.
            let (mut c, mut r) = (col as isize, row as isize);
            if matches!(from, Endpoint::Node(_)) {
                let (dc, dr) = dir.step();
                c += dc;
                r += dr;
            }
            let (to, end) = loop {
                let (cu, ru) = (c as usize, r as usize);
                if let Some(&n) = node_at.get(&(cu, ru)) {
                    let end = grid.cell_center(cu, ru) - dir.vec() * HALF + right;
                    break (Endpoint::Node(n), end);
                }
                let (dc, dr) = dir.step();
                if grid.get(c + dc, r + dr).is_none() {
                    let t = terminal_at[&(cu, ru, dir)];
                    let end =
                        grid.cell_center(cu, ru) + dir.vec() * (HALF + EDGE_EXTENSION) + right;
                    break (Endpoint::Edge(t), end);
                }
                c += dc;
                r += dr;
            };
            let id = lanes.len();
            let geom = Geom::Line { a: start, b: end };
            lanes.push(Lane {
                geom,
                length: geom.length(),
                kind: LaneKind::Road { dir, from, to },
                next: Vec::new(),
                speed_limit: ROAD_SPEED,
                conflicts: Vec::new(),
            });
            match from {
                Endpoint::Node(n) => nodes[n].outgoing.push(id),
                Endpoint::Edge(t) => terminals[t].entry = id,
            }
            match to {
                Endpoint::Node(n) => nodes[n].incoming.push(id),
                Endpoint::Edge(t) => terminals[t].exit = id,
            }
        }

        // Turn lanes inside every node.
        for (node_id, node) in nodes.iter_mut().enumerate() {
            let center = node.center;
            for &entry in &node.incoming {
                let d_in = lanes[entry].dir().expect("road lane");
                for &exit in &node.outgoing {
                    let d_out = lanes[exit].dir().expect("road lane");
                    if d_out == d_in.opposite() {
                        continue;
                    }
                    let (turn, geom) = turn_geometry(center, d_in, d_out);
                    let speed_limit = match geom {
                        Geom::Line { .. } => ROAD_SPEED,
                        Geom::Arc { radius, .. } => (LATERAL_ACCEL * radius).sqrt(),
                    };
                    let id = lanes.len();
                    lanes.push(Lane {
                        geom,
                        length: geom.length(),
                        kind: LaneKind::Turn {
                            node: node_id,
                            turn,
                            from_dir: d_in,
                            entry,
                            exit,
                        },
                        next: vec![exit],
                        speed_limit,
                        conflicts: Vec::new(),
                    });
                    lanes[entry].next.push(id);
                    node.turns.push(id);
                }
            }
        }

        // Conflicts between turn lanes of the same node that start from different lanes.
        for node in &nodes {
            let samples: Vec<(LaneId, Vec<Vec2>)> = node
                .turns
                .iter()
                .map(|&t| (t, lanes[t].geom.sample(0.25)))
                .collect();
            for (i, (a, pa)) in samples.iter().enumerate() {
                for (b, pb) in samples.iter().skip(i + 1) {
                    if entry_of(&lanes[*a]) == entry_of(&lanes[*b]) {
                        continue;
                    }
                    let close = pa
                        .iter()
                        .any(|p| pb.iter().any(|q| p.distance(*q) < CONFLICT_DISTANCE));
                    if close {
                        lanes[*a].conflicts.push(*b);
                        lanes[*b].conflicts.push(*a);
                    }
                }
            }
        }

        LaneGraph {
            lanes,
            nodes,
            terminals,
        }
    }

    #[cfg(test)]
    pub fn signal_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_signal()).count()
    }

    /// The incoming road lane at `node` whose vehicles travel in `dir`.
    #[cfg(test)]
    pub fn approach(&self, node: NodeId, dir: Dir) -> Option<LaneId> {
        self.nodes[node]
            .incoming
            .iter()
            .copied()
            .find(|&l| self.lanes[l].dir() == Some(dir))
    }
}

fn entry_of(lane: &Lane) -> Option<LaneId> {
    match lane.kind {
        LaneKind::Turn { entry, .. } => Some(entry),
        LaneKind::Road { .. } => None,
    }
}

/// Path through a junction box centred at `c`, from a lane travelling `d_in` to one travelling
/// `d_out`.
fn turn_geometry(c: Vec2, d_in: Dir, d_out: Dir) -> (Turn, Geom) {
    let vin = d_in.vec();
    let rin = d_in.right().vec();
    let p_in = c - vin * HALF + rin * LANE_OFFSET;
    let p_out = c + d_out.vec() * HALF + d_out.right().vec() * LANE_OFFSET;
    if d_out == d_in {
        return (Turn::Straight, Geom::Line { a: p_in, b: p_out });
    }
    let (turn, center, radius, sweep) = if d_out == d_in.right() {
        (
            Turn::Right,
            c - vin * HALF + rin * HALF,
            HALF - LANE_OFFSET,
            -FRAC_PI_2,
        )
    } else {
        (
            Turn::Left,
            c - vin * HALF - rin * HALF,
            HALF + LANE_OFFSET,
            FRAC_PI_2,
        )
    };
    let rel = p_in - center;
    let geom = Geom::Arc {
        center,
        radius,
        start: rel.y.atan2(rel.x),
        sweep,
    };
    (turn, geom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cross() -> LaneGraph {
        let grid = Grid::parse(&[
            "...|...", "...|...", "...|...", "---+---", "...|...", "...|...", "...|...",
        ])
        .unwrap();
        LaneGraph::build(&grid)
    }

    fn turn_lane(g: &LaneGraph, from_dir: Dir, turn: Turn) -> LaneId {
        g.nodes[0]
            .turns
            .iter()
            .copied()
            .find(|&t| {
                matches!(g.lanes[t].kind, LaneKind::Turn { from_dir: d, turn: tt, .. } if d == from_dir && tt == turn)
            })
            .unwrap()
    }

    #[test]
    fn crossroads_has_expected_shape() {
        let g = cross();
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.signal_count(), 1);
        assert_eq!(g.terminals.len(), 4);
        let roads = g.lanes.iter().filter(|l| !l.is_turn()).count();
        let turns = g.lanes.iter().filter(|l| l.is_turn()).count();
        assert_eq!(roads, 8);
        assert_eq!(turns, 12);
    }

    #[test]
    fn lanes_connect_end_to_start() {
        let g = cross();
        for lane in &g.lanes {
            for &n in &lane.next {
                let gap = lane.geom.end().distance(g.lanes[n].geom.start());
                assert!(gap < 1e-3, "gap of {gap} between connected lanes");
            }
        }
    }

    #[test]
    fn traffic_drives_on_the_right() {
        let g = cross();
        let centre = g.nodes[0].center;
        let north = g.approach(0, Dir::N).unwrap();
        assert!(g.lanes[north].geom.start().x > centre.x);
        let east = g.approach(0, Dir::E).unwrap();
        assert!(g.lanes[east].geom.start().y < centre.y);
    }

    #[test]
    fn terminals_have_entry_and_exit() {
        let g = cross();
        for t in &g.terminals {
            assert!(!g.lanes[t.entry].is_turn());
            assert!(g.lanes[t.exit].next.is_empty());
            assert_eq!(g.lanes[t.entry].dir(), Some(t.side.opposite()));
            assert_eq!(g.lanes[t.exit].dir(), Some(t.side));
        }
    }

    #[test]
    fn conflicts_follow_geometry() {
        let g = cross();
        let ns = turn_lane(&g, Dir::N, Turn::Straight);
        let ss = turn_lane(&g, Dir::S, Turn::Straight);
        let es = turn_lane(&g, Dir::E, Turn::Straight);
        let sl = turn_lane(&g, Dir::S, Turn::Left);
        let nr = turn_lane(&g, Dir::N, Turn::Right);
        let sr = turn_lane(&g, Dir::S, Turn::Right);
        // Opposing straights and opposing right turns run side by side.
        assert!(!g.lanes[ns].conflicts.contains(&ss));
        assert!(!g.lanes[nr].conflicts.contains(&sr));
        // Crossing straights, and a left turn across oncoming traffic, conflict.
        assert!(g.lanes[ns].conflicts.contains(&es));
        assert!(g.lanes[sl].conflicts.contains(&ns));
        // Northbound right and southbound left both merge into the eastbound exit.
        assert!(g.lanes[nr].conflicts.contains(&sl));
        // Conflicts are symmetric.
        for (id, lane) in g.lanes.iter().enumerate() {
            for &c in &lane.conflicts {
                assert!(g.lanes[c].conflicts.contains(&id));
            }
        }
    }

    #[test]
    fn turns_slow_down_with_tighter_radius() {
        let g = cross();
        let right = turn_lane(&g, Dir::N, Turn::Right);
        let left = turn_lane(&g, Dir::N, Turn::Left);
        let straight = turn_lane(&g, Dir::N, Turn::Straight);
        assert!(g.lanes[right].speed_limit < g.lanes[left].speed_limit);
        assert!(g.lanes[left].speed_limit < g.lanes[straight].speed_limit);
    }

    #[test]
    fn bends_are_unsignalled_nodes() {
        let grid = Grid::parse(&["...|.", "...|.", "---+.", "....."]).unwrap();
        let g = LaneGraph::build(&grid);
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.nodes[0].kind, NodeKind::Bend);
        assert_eq!(g.signal_count(), 0);
        assert_eq!(g.lanes.iter().filter(|l| l.is_turn()).count(), 2);
        assert!(g.lanes.iter().all(|l| l.conflicts.is_empty()));
    }
}
