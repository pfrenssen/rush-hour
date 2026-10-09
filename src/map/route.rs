//! Shortest routes through the lane graph.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::graph::{LaneGraph, LaneId, LaneKind, TerminalId, Turn};
use super::grid::TILE;

/// Extra cost (metres) for turning, so that among equally long grid routes vehicles prefer the
/// one with the fewest turns instead of zig-zagging.
const LEFT_TURN_PENALTY: f32 = 6.0;
const RIGHT_TURN_PENALTY: f32 = 3.0;

fn lane_cost(graph: &LaneGraph, lane: LaneId) -> f32 {
    let l = &graph.lanes[lane];
    match l.kind {
        // Turn lanes all span one junction box; their arc length would otherwise make right
        // turns look like shortcuts.
        LaneKind::Turn { turn, .. } => {
            TILE + match turn {
                Turn::Straight => 0.0,
                Turn::Left => LEFT_TURN_PENALTY,
                Turn::Right => RIGHT_TURN_PENALTY,
            }
        }
        LaneKind::Road { .. } => l.length,
    }
}

#[derive(PartialEq)]
struct Entry {
    cost: f32,
    lane: LaneId,
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap on cost; tie-break on lane id for determinism.
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.lane.cmp(&self.lane))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Shortest route (list of lanes, both ends included) from `from` to `to`.
pub fn shortest_route(graph: &LaneGraph, from: LaneId, to: LaneId) -> Option<Vec<LaneId>> {
    let n = graph.lanes.len();
    let mut dist = vec![f32::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    dist[from] = 0.0;
    heap.push(Entry {
        cost: 0.0,
        lane: from,
    });
    while let Some(Entry { cost, lane }) = heap.pop() {
        if lane == to {
            break;
        }
        if cost > dist[lane] {
            continue;
        }
        for &next in &graph.lanes[lane].next {
            let c = cost + lane_cost(graph, next);
            if c < dist[next] {
                dist[next] = c;
                prev[next] = lane;
                heap.push(Entry {
                    cost: c,
                    lane: next,
                });
            }
        }
    }
    if !dist[to].is_finite() {
        return None;
    }
    let mut route = vec![to];
    let mut cur = to;
    while cur != from {
        cur = prev[cur];
        route.push(cur);
    }
    route.reverse();
    Some(route)
}

/// Routes between every pair of terminals on different map sides, indexed `[entry][exit]`.
pub fn route_table(graph: &LaneGraph) -> Vec<Vec<Option<Vec<LaneId>>>> {
    graph
        .terminals
        .iter()
        .map(|a| {
            graph
                .terminals
                .iter()
                .map(|b| {
                    if a.side == b.side {
                        None
                    } else {
                        shortest_route(graph, a.entry, b.exit)
                    }
                })
                .collect()
        })
        .collect()
}

/// Terminals reachable from `entry` on another side of the map.
pub fn exits_for(table: &[Vec<Option<Vec<LaneId>>>], entry: TerminalId) -> Vec<TerminalId> {
    table[entry]
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.as_ref().map(|_| i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::grid::{Dir, Grid};

    fn grid_2x2() -> LaneGraph {
        let grid = Grid::parse(&[
            "...|....|...",
            "...|....|...",
            "---+----+---",
            "...|....|...",
            "...|....|...",
            "...|....|...",
            "---+----+---",
            "...|....|...",
        ])
        .unwrap();
        LaneGraph::build(&grid)
    }

    fn terminal(g: &LaneGraph, side: Dir, nth: usize) -> usize {
        g.terminals
            .iter()
            .enumerate()
            .filter(|(_, t)| t.side == side)
            .nth(nth)
            .unwrap()
            .0
    }

    fn turns(g: &LaneGraph, route: &[LaneId]) -> Vec<Turn> {
        route.iter().filter_map(|&l| g.lanes[l].turn()).collect()
    }

    #[test]
    fn straight_through_route_has_no_turns() {
        let g = grid_2x2();
        let w = terminal(&g, Dir::W, 0);
        let e = terminal(&g, Dir::E, 0);
        let route = shortest_route(&g, g.terminals[w].entry, g.terminals[e].exit).unwrap();
        assert_eq!(turns(&g, &route), vec![Turn::Straight, Turn::Straight]);
        // Consecutive lanes are connected.
        for pair in route.windows(2) {
            assert!(g.lanes[pair[0]].next.contains(&pair[1]));
        }
    }

    #[test]
    fn corner_route_prefers_a_single_turn() {
        let g = grid_2x2();
        // From the west end of the top road to the bottom of the right street.
        let w = terminal(&g, Dir::W, 0);
        let s = terminal(&g, Dir::S, 1);
        let route = shortest_route(&g, g.terminals[w].entry, g.terminals[s].exit).unwrap();
        let t = turns(&g, &route);
        assert_eq!(t.iter().filter(|&&x| x != Turn::Straight).count(), 1);
    }

    #[test]
    fn every_entry_reaches_another_side() {
        let g = grid_2x2();
        let table = route_table(&g);
        for (i, t) in g.terminals.iter().enumerate() {
            let exits = exits_for(&table, i);
            assert!(!exits.is_empty());
            assert!(exits.iter().all(|&e| g.terminals[e].side != t.side));
        }
    }

    #[test]
    fn no_route_into_an_entry_lane() {
        let g = grid_2x2();
        let a = terminal(&g, Dir::W, 0);
        let b = terminal(&g, Dir::E, 0);
        assert!(shortest_route(&g, g.terminals[a].entry, g.terminals[b].entry).is_none());
    }
}
