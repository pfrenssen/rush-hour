//! ASCII city grids.
//!
//! | char | meaning                                        |
//! |------|------------------------------------------------|
//! | `.`  | city block with buildings                      |
//! | `T`  | park with trees                                |
//! | `~`  | water                                          |
//! | `-`  | east-west road                                 |
//! | `|`  | north-south road                               |
//! | `+`  | junction: connects to every neighbouring road  |
//!
//! Roads are two-way with one lane per direction. A `-` or `|` on the border that points off the
//! map is a terminal where vehicles enter and leave.

use std::fmt;

use bevy::math::Vec2;

/// World size of one ASCII cell in metres. A road fills the whole cell.
pub const TILE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    NorthSouth,
    EastWest,
}

impl Axis {
    pub fn other(self) -> Axis {
        match self {
            Axis::NorthSouth => Axis::EastWest,
            Axis::EastWest => Axis::NorthSouth,
        }
    }
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];

    /// Unit vector in world space (y points up / north).
    pub fn vec(self) -> Vec2 {
        match self {
            Dir::N => Vec2::Y,
            Dir::E => Vec2::X,
            Dir::S => Vec2::NEG_Y,
            Dir::W => Vec2::NEG_X,
        }
    }

    pub fn opposite(self) -> Dir {
        match self {
            Dir::N => Dir::S,
            Dir::E => Dir::W,
            Dir::S => Dir::N,
            Dir::W => Dir::E,
        }
    }

    pub fn right(self) -> Dir {
        match self {
            Dir::N => Dir::E,
            Dir::E => Dir::S,
            Dir::S => Dir::W,
            Dir::W => Dir::N,
        }
    }

    pub fn axis(self) -> Axis {
        match self {
            Dir::N | Dir::S => Axis::NorthSouth,
            Dir::E | Dir::W => Axis::EastWest,
        }
    }

    /// Step in grid coordinates (column, row); rows grow downwards.
    pub fn step(self) -> (isize, isize) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Block,
    Park,
    Water,
    RoadH,
    RoadV,
    Junction,
}

impl Cell {
    pub fn is_road(self) -> bool {
        matches!(self, Cell::RoadH | Cell::RoadV | Cell::Junction)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Ragged {
        row: usize,
        expected: usize,
        found: usize,
    },
    UnknownChar {
        ch: char,
        col: usize,
        row: usize,
    },
    DeadEnd {
        col: usize,
        row: usize,
        dir: Dir,
    },
    AdjacentJunctions {
        col: usize,
        row: usize,
    },
    LonelyJunction {
        col: usize,
        row: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "the map has no cells"),
            ParseError::Ragged {
                row,
                expected,
                found,
            } => write!(f, "row {row} has {found} cells, expected {expected}"),
            ParseError::UnknownChar { ch, col, row } => {
                write!(f, "unknown character {ch:?} at column {col}, row {row}")
            }
            ParseError::DeadEnd { col, row, dir } => {
                write!(
                    f,
                    "road at column {col}, row {row} dead-ends towards {dir:?}"
                )
            }
            ParseError::AdjacentJunctions { col, row } => write!(
                f,
                "junction at column {col}, row {row} touches another junction"
            ),
            ParseError::LonelyJunction { col, row } => write!(
                f,
                "junction at column {col}, row {row} connects fewer than two roads"
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn parse(rows: &[&str]) -> Result<Grid, ParseError> {
        let height = rows.len();
        let width = rows.first().map_or(0, |r| r.chars().count());
        if height == 0 || width == 0 {
            return Err(ParseError::Empty);
        }
        let mut cells = Vec::with_capacity(width * height);
        for (row, line) in rows.iter().enumerate() {
            let found = line.chars().count();
            if found != width {
                return Err(ParseError::Ragged {
                    row,
                    expected: width,
                    found,
                });
            }
            for (col, ch) in line.chars().enumerate() {
                cells.push(match ch {
                    '.' => Cell::Block,
                    'T' => Cell::Park,
                    '~' => Cell::Water,
                    '-' => Cell::RoadH,
                    '|' => Cell::RoadV,
                    '+' => Cell::Junction,
                    _ => return Err(ParseError::UnknownChar { ch, col, row }),
                });
            }
        }
        let grid = Grid {
            width,
            height,
            cells,
        };
        grid.validate()?;
        Ok(grid)
    }

    pub fn get(&self, col: isize, row: isize) -> Option<Cell> {
        if col < 0 || row < 0 || col as usize >= self.width || row as usize >= self.height {
            None
        } else {
            Some(self.cells[row as usize * self.width + col as usize])
        }
    }

    pub fn cell(&self, col: usize, row: usize) -> Cell {
        self.cells[row * self.width + col]
    }

    pub fn neighbour(&self, col: usize, row: usize, dir: Dir) -> Option<Cell> {
        let (dc, dr) = dir.step();
        self.get(col as isize + dc, row as isize + dr)
    }

    /// Does the road in this cell continue towards `dir`? For straight roads on the border this
    /// can point off the map (a terminal).
    pub fn opens(&self, col: usize, row: usize, dir: Dir) -> bool {
        match self.cell(col, row) {
            Cell::RoadH => dir.axis() == Axis::EastWest,
            Cell::RoadV => dir.axis() == Axis::NorthSouth,
            Cell::Junction => match self.neighbour(col, row, dir) {
                Some(Cell::RoadH) => dir.axis() == Axis::EastWest,
                Some(Cell::RoadV) => dir.axis() == Axis::NorthSouth,
                Some(Cell::Junction) => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn openings(&self, col: usize, row: usize) -> Vec<Dir> {
        Dir::ALL
            .into_iter()
            .filter(|&d| self.opens(col, row, d))
            .collect()
    }

    /// A junction is a graph node unless it is just a straight piece of road.
    pub fn is_node(&self, col: usize, row: usize) -> bool {
        if self.cell(col, row) != Cell::Junction {
            return false;
        }
        let open = self.openings(col, row);
        !(open.len() == 2 && open[0].opposite() == open[1])
    }

    fn validate(&self) -> Result<(), ParseError> {
        for row in 0..self.height {
            for col in 0..self.width {
                let cell = self.cell(col, row);
                if !cell.is_road() {
                    continue;
                }
                if cell == Cell::Junction {
                    if self.openings(col, row).len() < 2 {
                        return Err(ParseError::LonelyJunction { col, row });
                    }
                    if Dir::ALL
                        .iter()
                        .any(|&d| self.neighbour(col, row, d) == Some(Cell::Junction))
                    {
                        return Err(ParseError::AdjacentJunctions { col, row });
                    }
                }
                for dir in Dir::ALL {
                    if !self.opens(col, row, dir) {
                        continue;
                    }
                    let (dc, dr) = dir.step();
                    let (nc, nr) = (col as isize + dc, row as isize + dr);
                    if self.get(nc, nr).is_none() {
                        continue; // terminal at the map edge
                    }
                    if !self.opens(nc as usize, nr as usize, dir.opposite()) {
                        return Err(ParseError::DeadEnd { col, row, dir });
                    }
                }
            }
        }
        Ok(())
    }

    /// World-space size of the whole map.
    pub fn world_size(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32) * TILE
    }

    /// World-space centre of a cell; the map is centred on the origin.
    pub fn cell_center(&self, col: usize, row: usize) -> Vec2 {
        Vec2::new(
            (col as f32 + 0.5) * TILE - self.width as f32 * TILE * 0.5,
            self.height as f32 * TILE * 0.5 - (row as f32 + 0.5) * TILE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cells_and_size() {
        let g = Grid::parse(&["..|..", "--+--", "T.|~."]).unwrap();
        assert_eq!((g.width, g.height), (5, 3));
        assert_eq!(g.cell(2, 1), Cell::Junction);
        assert_eq!(g.cell(0, 2), Cell::Park);
        assert_eq!(g.cell(3, 2), Cell::Water);
        assert!(g.is_node(2, 1));
        assert_eq!(g.openings(2, 1).len(), 4);
    }

    #[test]
    fn rejects_ragged_rows() {
        assert_eq!(
            Grid::parse(&["..|..", "--+-"]).unwrap_err(),
            ParseError::Ragged {
                row: 1,
                expected: 5,
                found: 4
            }
        );
    }

    #[test]
    fn rejects_unknown_characters() {
        assert!(matches!(
            Grid::parse(&["..x"]).unwrap_err(),
            ParseError::UnknownChar { ch: 'x', .. }
        ));
    }

    #[test]
    fn rejects_dead_ends() {
        // The east-west road runs into a block.
        assert!(matches!(
            Grid::parse(&["--.", "..."]).unwrap_err(),
            ParseError::DeadEnd { dir: Dir::E, .. }
        ));
    }

    #[test]
    fn rejects_adjacent_junctions() {
        assert!(matches!(
            Grid::parse(&[".|.|.", "-++-", ".|.|."]).unwrap_err(),
            ParseError::Ragged { .. }
        ));
        assert!(matches!(
            Grid::parse(&[".||.", "-++-", ".||."]).unwrap_err(),
            ParseError::AdjacentJunctions { .. }
        ));
    }

    #[test]
    fn straight_junction_is_not_a_node() {
        let g = Grid::parse(&["-+-"]).unwrap();
        assert!(!g.is_node(1, 0));
    }

    #[test]
    fn cell_centres_are_relative_to_the_map_centre() {
        let g = Grid::parse(&["..|..", "--+--", "..|.."]).unwrap();
        let c = g.cell_center(2, 1);
        assert!(c.abs_diff_eq(Vec2::ZERO, 1e-5));
        assert!(g.cell_center(4, 0).abs_diff_eq(Vec2::new(16.0, 8.0), 1e-5));
    }
}
