//! Hand-made levels. See `map::grid` for the ASCII legend.

/// Relative spawn weights of each vehicle type.
#[derive(Clone, Copy, Debug)]
pub struct Mix {
    pub car: f32,
    pub bus: f32,
    pub truck: f32,
    pub ambulance: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelDef {
    pub name: &'static str,
    pub tagline: &'static str,
    pub rows: &'static [&'static str],
    /// Vehicles to deliver to win the level.
    pub target: u32,
    /// Spawns per minute at the start and at the end of the level; the rate ramps up with the
    /// fraction of the target delivered.
    pub spawn_per_min: (f32, f32),
    pub mix: Mix,
}

pub const LEVELS: &[LevelDef] = &[
    LevelDef {
        name: "Main Street",
        tagline: "Two crossings. Click one to switch its lights.",
        rows: &[
            "..TT...|........|.......",
            "..TT...|........|..~~~..",
            ".......|..TTTT..|..~~~..",
            ".......|..TTTT..|.......",
            ".......|........|.......",
            "-------+--------+-------",
            ".......|........|.......",
            "...~~..|........|..TT...",
            "...~~..|...TT...|..TT...",
            ".......|...TT...|.......",
            ".......|........|.......",
        ],
        target: 25,
        spawn_per_min: (14.0, 26.0),
        mix: Mix {
            car: 0.88,
            bus: 0.04,
            truck: 0.06,
            ambulance: 0.02,
        },
    },
    LevelDef {
        name: "Twin Avenues",
        tagline: "Four crossings. Keep the avenues flowing.",
        rows: &[
            "........|..........|........",
            "..TTT...|..........|...~~...",
            "..TTT...|...TTTT...|...~~...",
            "........|...TTTT...|........",
            "--------+----------+--------",
            "........|..........|........",
            "...~~...|....TT....|..TTT...",
            "...~~...|....TT....|..TTT...",
            "........|..........|........",
            "--------+----------+--------",
            "........|..........|........",
            "..TT....|...~~~~...|....TT..",
            "..TT....|...~~~~...|....TT..",
            "........|..........|........",
        ],
        target: 35,
        spawn_per_min: (20.0, 34.0),
        mix: Mix {
            car: 0.84,
            bus: 0.06,
            truck: 0.07,
            ambulance: 0.03,
        },
    },
    LevelDef {
        name: "Old Town",
        tagline: "A T-junction joins the grid. Buses appear more often.",
        rows: &[
            "......|..................|......",
            "..TT..|...TTTTTTTTTTTT...|..~~..",
            "..TT..|...TTTTTTTTTTTT...|..~~..",
            "......|..................|......",
            "------+--------+---------+------",
            "......|........|.........|......",
            "..~~..|..TT....|.........|..TT..",
            "..~~..|..TT....|...~~~...|..TT..",
            "......|........|...~~~...|......",
            "......|....TT..|.........|......",
            "......|........|.........|......",
            "------+--------+---------+------",
            "......|........|.........|......",
            "..TT..|..~~~...|...TTT...|......",
            "..TT..|..~~~...|...TTT...|..~~..",
            "......|........|.........|......",
        ],
        target: 45,
        spawn_per_min: (26.0, 42.0),
        mix: Mix {
            car: 0.78,
            bus: 0.09,
            truck: 0.09,
            ambulance: 0.04,
        },
    },
    LevelDef {
        name: "Riverside",
        tagline: "Three bridges over the river. Mind the T-junctions.",
        rows: &[
            "..............|...~~~...|.........",
            "..TTT.........|...~~~...|..TTTT...",
            "..TTT.........|...~~~...|..TTTT...",
            "-----+--------+---------+---------",
            ".....|........|...~~~...|.........",
            ".....|..TT....|...~~~...|...TT....",
            ".....|..TT....|...~~~...|...TT....",
            ".....|........|...~~~...|.........",
            "-----+--------+---------+---------",
            ".....|........|...~~~...|.........",
            ".TT..|...TT...|...~~~...|..~~.....",
            ".TT..|...TT...|...~~~...|..~~.....",
            ".....|........|...~~~...|.........",
            "-----+--------+---------+---------",
            ".....|........|...~~~.............",
            ".....|..TTT...|...~~~......TTT....",
            ".....|..TTT...|...~~~......TTT....",
        ],
        target: 55,
        spawn_per_min: (30.0, 48.0),
        mix: Mix {
            car: 0.76,
            bus: 0.09,
            truck: 0.11,
            ambulance: 0.04,
        },
    },
    LevelDef {
        name: "Uptown",
        tagline: "Twelve signals. Spread the green around.",
        rows: &[
            ".....|.................|........|.....",
            "..T..|...TTTT....~~~...|..TTTT..|.....",
            "..T..|...TTTT....~~~...|..TTTT..|..~~.",
            ".....|.................|........|..~~.",
            "-----+--------+--------+--------+-----",
            ".....|........|........|........|.....",
            ".TT..|..~~~...|...TT...|........|..T..",
            ".TT..|..~~~...|...TT...|..TTT...|..T..",
            ".....|........|........|..TTT...|.....",
            "-----+--------+--------+--------+-----",
            ".....|........|........|........|.....",
            "..~~.|...TT...|..~~~~..|...TT...|.TT..",
            "..~~.|...TT...|..~~~~..|...TT...|.TT..",
            ".....|........|........|........|.....",
            "-----+--------+--------+--------+.....",
            ".....|........|........|..............",
            ".TT..|..TTT...|...~~...|...TTTT...~~..",
            ".TT..|..TTT...|...~~...|...TTTT...~~..",
            ".....|........|........|..............",
        ],
        target: 65,
        spawn_per_min: (32.0, 52.0),
        mix: Mix {
            car: 0.74,
            bus: 0.10,
            truck: 0.11,
            ambulance: 0.05,
        },
    },
    LevelDef {
        name: "Rush Hour",
        tagline: "The whole city wants to go home. Good luck.",
        rows: &[
            "...........|.......|.......|.......|....",
            ".TT........|..TTT..|.~~~...|..TT...|....",
            ".TT........|..TTT..|.~~~...|..TT...|.~~.",
            "...........|.......|.......|.......|.~~.",
            "....+------+-------+-------+-------+----",
            "....|......|.......|.......|.......|....",
            ".TT.|..~~..|..TT...|.......|..TTT..|.TT.",
            ".TT.|..~~..|..TT...|..~~~..|..TTT..|.TT.",
            "....|......|.......|..~~~..|.......|....",
            "....|......|.......|.......|.......|....",
            "----+------+-------+-------+-------+----",
            "....|......|.......|.......|.......|....",
            ".~~.|..TT..|..~~...|..TT...|.......|..~.",
            ".~~.|..TT..|..~~...|..TT...|..TTT..|..~.",
            "....|......|.......|.......|..TTT..|....",
            "....|......|.......|.......|.......|....",
            "----+------+-------+-------+-------+----",
            "....|......|.......|...............|....",
            ".TT.|..TT..|..~~~..|...TTT....TT...|.~~.",
            ".TT.|..TT..|..~~~..|...TTT....TT...|.~~.",
            "....|......|.......|...............|....",
        ],
        target: 80,
        spawn_per_min: (38.0, 60.0),
        mix: Mix {
            car: 0.72,
            bus: 0.10,
            truck: 0.12,
            ambulance: 0.06,
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::graph::LaneGraph;
    use crate::map::grid::Grid;
    use crate::map::route::{exits_for, route_table};

    fn graph(level: &LevelDef) -> LaneGraph {
        let grid = Grid::parse(level.rows)
            .unwrap_or_else(|e| panic!("level {:?} does not parse: {e}", level.name));
        LaneGraph::build(&grid)
    }

    #[test]
    fn there_are_at_least_five_levels() {
        assert!(LEVELS.len() >= 5);
    }

    #[test]
    fn every_level_parses_and_connects() {
        for level in LEVELS {
            let g = graph(level);
            let table = route_table(&g);
            for (i, t) in g.terminals.iter().enumerate() {
                assert!(
                    !exits_for(&table, i).is_empty(),
                    "{}: terminal at {},{} reaches no exit",
                    level.name,
                    t.col,
                    t.row
                );
            }
        }
    }

    #[test]
    fn levels_grow_in_size_signals_and_traffic() {
        let mut prev: Option<(usize, usize, f32, u32)> = None;
        for level in LEVELS {
            let g = graph(level);
            let grid = Grid::parse(level.rows).unwrap();
            let cur = (
                grid.width * grid.height,
                g.signal_count(),
                level.spawn_per_min.1,
                level.target,
            );
            if let Some(p) = prev {
                assert!(cur.0 > p.0, "{} is not larger", level.name);
                assert!(cur.1 > p.1, "{} has no more signals", level.name);
                assert!(cur.2 > p.2, "{} does not spawn faster", level.name);
                assert!(cur.3 > p.3, "{} has no higher target", level.name);
            }
            prev = Some(cur);
        }
    }

    #[test]
    fn spawn_rate_ramps_up_within_each_level() {
        for level in LEVELS {
            assert!(level.spawn_per_min.1 > level.spawn_per_min.0);
        }
    }
}
