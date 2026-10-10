//! City blocks: houses line the streets, the inside of a block is green.
//!
//! A block cell that touches a road becomes a street lot with a house facing that road (corner
//! lots get a corner house). Cells that do not touch any road form the inside of the block; each
//! connected inside area becomes a garden or a small park, often with a playground.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::palette;
use super::shapes::ShapeBuilder;
use crate::map::graph::HALF;
use crate::map::grid::{Cell, Dir, Grid, TILE};

/// Width of the sidewalk along a road, inside the lot.
pub const SIDEWALK: f32 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lot {
    /// Touches the road on these sides.
    Street(Vec<Dir>),
    Garden,
    Park,
    Playground,
}

/// Decide what every block cell holds; `None` for cells that are not blocks.
pub fn plan_lots(grid: &Grid, seed: u64) -> Vec<Option<Lot>> {
    let idx = |col: usize, row: usize| row * grid.width + col;
    let mut lots: Vec<Option<Lot>> = vec![None; grid.width * grid.height];
    for row in 0..grid.height {
        for col in 0..grid.width {
            if grid.cell(col, row) != Cell::Block {
                continue;
            }
            let roads: Vec<Dir> = Dir::ALL
                .into_iter()
                .filter(|&d| grid.neighbour(col, row, d).is_some_and(Cell::is_road))
                .collect();
            if !roads.is_empty() {
                lots[idx(col, row)] = Some(Lot::Street(roads));
            }
        }
    }

    // Connected inside areas share one theme so a block reads as one garden or one park.
    let mut seen = vec![false; lots.len()];
    for row in 0..grid.height {
        for col in 0..grid.width {
            let start = idx(col, row);
            if seen[start] || grid.cell(col, row) != Cell::Block || lots[start].is_some() {
                continue;
            }
            let mut area = Vec::new();
            let mut queue = VecDeque::from([(col, row)]);
            seen[start] = true;
            while let Some((c, r)) = queue.pop_front() {
                area.push((c, r));
                for d in Dir::ALL {
                    let (dc, dr) = d.step();
                    let (nc, nr) = (c as isize + dc, r as isize + dr);
                    if grid.get(nc, nr) != Some(Cell::Block) {
                        continue;
                    }
                    let n = idx(nc as usize, nr as usize);
                    if !seen[n] && lots[n].is_none() {
                        seen[n] = true;
                        queue.push_back((nc as usize, nr as usize));
                    }
                }
            }
            let mut rng = fastrand::Rng::with_seed(seed ^ (start as u64).wrapping_mul(0x9E37_79B9));
            let theme = if rng.f32() < 0.6 {
                Lot::Garden
            } else {
                Lot::Park
            };
            for &(c, r) in &area {
                lots[idx(c, r)] = Some(theme.clone());
            }
            if area.len() >= 3 || rng.f32() < 0.3 {
                let (c, r) = area[rng.usize(..area.len())];
                lots[idx(c, r)] = Some(Lot::Playground);
            }
        }
    }
    lots
}

/// Draw one lot. Ground colour goes to `ground`, flat details to `marks`, raised things (houses,
/// trees, play equipment) to `shadows` and `tops`.
pub fn draw_lot(
    lot: &Lot,
    center: Vec2,
    rng: &mut fastrand::Rng,
    ground: &mut ShapeBuilder,
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
) {
    match lot {
        Lot::Street(roads) => street_lot(center, roads, rng, shadows, tops),
        Lot::Garden => garden(center, rng, ground, shadows, tops),
        Lot::Park => {
            ground.rect(center, Vec2::splat(TILE), palette::PARK);
            trees(shadows, tops, center, rng);
        }
        Lot::Playground => playground(center, rng, ground, shadows, tops),
    }
}

/// Size of a footprint `along` the street and `depth` away from it, in world axes.
fn footprint(road: Dir, along: f32, depth: f32) -> Vec2 {
    match road {
        Dir::N | Dir::S => Vec2::new(along, depth),
        Dir::E | Dir::W => Vec2::new(depth, along),
    }
}

fn street_lot(
    center: Vec2,
    roads: &[Dir],
    rng: &mut fastrand::Rng,
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
) {
    // Distance from the cell centre to the inner edge of the sidewalk.
    let inner = HALF - SIDEWALK;
    let setback = 0.3 + rng.f32() * 0.6;
    let roof = palette::ROOFS[rng.usize(..palette::ROOFS.len())];
    let opposite = roads.len() == 2 && roads[0].opposite() == roads[1];
    match roads {
        [d] => {
            let depth = 3.4 + rng.f32() * 1.4;
            let front = center + d.vec() * (inner - setback - depth / 2.0);
            let along = d.right().vec();
            if rng.f32() < 0.25 {
                // Two narrow row houses.
                let w = TILE / 2.0 - 0.45;
                for (side, colour) in [(-1.0, roof), (1.0, pick_roof(rng))] {
                    let pos = front + along * side * (TILE / 4.0);
                    house(shadows, tops, pos, footprint(*d, w, depth), colour);
                }
            } else {
                let w = TILE - 0.5 - rng.f32() * 0.7;
                house(shadows, tops, front, footprint(*d, w, depth), roof);
            }
            // A tree in some back gardens.
            let back = inner + SIDEWALK - setback - depth;
            if back > 2.0 && rng.f32() < 0.35 {
                let pos = center - d.vec() * (HALF - back / 2.0) + along * (rng.f32() * 3.0 - 1.5);
                tree(shadows, tops, pos, (back / 2.0 - 0.1).min(1.2));
            }
        }
        [a, b] if !opposite => {
            // A corner house.
            let size = 4.6 + rng.f32() * 1.0;
            let pos = center + (a.vec() + b.vec()) * (inner - setback - size / 2.0);
            house(shadows, tops, pos, Vec2::splat(size), roof);
        }
        [a, b] => {
            // A narrow block between two streets: two houses back to back.
            let depth = 2.4 + rng.f32() * 0.4;
            for (d, colour) in [(a, roof), (b, pick_roof(rng))] {
                let pos = center + d.vec() * (inner - setback - depth / 2.0);
                house(shadows, tops, pos, footprint(*d, TILE - 0.8, depth), colour);
            }
        }
        _ => house(shadows, tops, center, Vec2::splat(5.2), roof),
    }
}

fn pick_roof(rng: &mut fastrand::Rng) -> Color {
    palette::ROOFS[rng.usize(..palette::ROOFS.len())]
}

pub fn house(
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
    pos: Vec2,
    size: Vec2,
    roof: Color,
) {
    shadows.rounded_rect(pos + Vec2::new(0.45, -0.45), size, 0.5, palette::SHADOW);
    tops.rounded_rect(pos, size, 0.5, roof);
    // A lighter ridge gives the flat roof some depth.
    let ridge = if size.x > size.y {
        Vec2::new(size.x * 0.7, size.y * 0.18)
    } else {
        Vec2::new(size.x * 0.18, size.y * 0.7)
    };
    tops.rounded_rect(pos, ridge, 0.2, roof.lighter(0.06));
}

pub fn trees(
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
    center: Vec2,
    rng: &mut fastrand::Rng,
) {
    let n = rng.usize(2..=3);
    for i in 0..n {
        let angle = (i as f32 / n as f32 + rng.f32() * 0.2) * std::f32::consts::TAU;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * (1.2 + rng.f32() * 0.6);
        tree(shadows, tops, pos, 1.3 + rng.f32() * 0.5);
    }
}

pub fn tree(shadows: &mut ShapeBuilder, tops: &mut ShapeBuilder, pos: Vec2, r: f32) {
    shadows.circle(pos + Vec2::new(0.4, -0.4), r, palette::SHADOW);
    tops.circle(pos, r, palette::TREE);
    tops.circle(
        pos + Vec2::new(-0.25, 0.25) * r,
        r * 0.55,
        palette::TREE_LIGHT,
    );
}

const FLOWERS: [Color; 4] = [
    Color::srgb(0.96, 0.46, 0.55),
    Color::srgb(0.99, 0.84, 0.34),
    Color::srgb(0.70, 0.56, 0.93),
    Color::srgb(0.98, 0.98, 0.95),
];

fn garden(
    center: Vec2,
    rng: &mut fastrand::Rng,
    ground: &mut ShapeBuilder,
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
) {
    ground.rect(center, Vec2::splat(TILE), palette::GARDEN);
    // Now and then a flower bed.
    if rng.f32() < 0.4 {
        let bed = center + Vec2::new(rng.f32() * 2.0 - 1.0, rng.f32() * 2.0 - 1.0);
        let size = if rng.bool() {
            Vec2::new(3.6, 1.6)
        } else {
            Vec2::new(1.6, 3.6)
        };
        ground.rounded_rect(bed, size, 0.6, palette::SOIL);
        for _ in 0..6 {
            let p = bed + (Vec2::new(rng.f32(), rng.f32()) - 0.5) * (size - Vec2::splat(0.7));
            ground.circle(p, 0.28, FLOWERS[rng.usize(..FLOWERS.len())]);
        }
    }
    // Round bushes towards the corners.
    for _ in 0..rng.usize(1..=2) {
        let corner = Vec2::new(
            if rng.bool() { 1.0 } else { -1.0 },
            if rng.bool() { 1.0 } else { -1.0 },
        );
        let pos = center + corner * (2.4 + rng.f32() * 0.6);
        let r = 0.8 + rng.f32() * 0.3;
        shadows.circle(pos + Vec2::new(0.25, -0.25), r, palette::SHADOW);
        tops.circle(pos, r, palette::BUSH);
    }
}

fn playground(
    center: Vec2,
    rng: &mut fastrand::Rng,
    ground: &mut ShapeBuilder,
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
) {
    ground.rect(center, Vec2::splat(TILE), palette::GARDEN);
    ground.rounded_rect(center, Vec2::splat(6.6), 1.2, palette::SAND);
    // Four spots, three pieces of equipment in a random arrangement.
    let mut spots = [
        Vec2::new(-1.6, 1.6),
        Vec2::new(1.6, 1.6),
        Vec2::new(-1.6, -1.6),
        Vec2::new(1.6, -1.6),
    ];
    rng.shuffle(&mut spots);
    let red = Color::srgb(0.92, 0.36, 0.30);
    let blue = Color::srgb(0.30, 0.58, 0.92);
    let steel = Color::srgb(0.55, 0.58, 0.62);
    // Slide: a platform and a chute.
    let p = center + spots[0];
    shadows.rect(
        p + Vec2::new(0.25, -0.25),
        Vec2::new(0.8, 2.6),
        palette::SHADOW,
    );
    tops.rect(p + Vec2::new(0.0, 0.85), Vec2::splat(0.9), blue);
    tops.rounded_rect(p - Vec2::new(0.0, 0.4), Vec2::new(0.6, 1.9), 0.25, red);
    // Swing set: a frame with two seats.
    let p = center + spots[1];
    shadows.rect(
        p + Vec2::new(0.2, -0.2),
        Vec2::new(2.6, 0.25),
        palette::SHADOW,
    );
    tops.rect(p, Vec2::new(2.6, 0.22), steel);
    for x in [-0.6, 0.6] {
        tops.rect(
            p + Vec2::new(x, -0.45),
            Vec2::new(0.5, 0.25),
            Color::srgb(0.98, 0.78, 0.25),
        );
    }
    // Sandbox or roundabout.
    let p = center + spots[2];
    if rng.bool() {
        tops.rect(p, Vec2::splat(1.9), Color::srgb(0.62, 0.45, 0.30));
        tops.rect(p, Vec2::splat(1.4), Color::srgb(0.93, 0.86, 0.66));
    } else {
        shadows.circle(p + Vec2::new(0.2, -0.2), 0.95, palette::SHADOW);
        tops.circle(p, 0.95, Color::srgb(0.25, 0.70, 0.62));
        tops.rect(p, Vec2::new(1.5, 0.18), steel);
        tops.rect(p, Vec2::new(0.18, 1.5), steel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LEVELS;

    #[test]
    fn houses_only_stand_along_streets() {
        for (level, def) in LEVELS.iter().enumerate() {
            let grid = Grid::parse(def.rows).unwrap();
            let lots = plan_lots(&grid, level as u64);
            for row in 0..grid.height {
                for col in 0..grid.width {
                    let lot = &lots[row * grid.width + col];
                    let touches_road = Dir::ALL
                        .iter()
                        .any(|&d| grid.neighbour(col, row, d).is_some_and(Cell::is_road));
                    match (grid.cell(col, row), lot) {
                        (Cell::Block, Some(Lot::Street(roads))) => {
                            assert!(touches_road && !roads.is_empty());
                        }
                        (Cell::Block, Some(_)) => assert!(!touches_road),
                        (Cell::Block, None) => panic!("block cell without a lot"),
                        (_, lot) => assert!(lot.is_none()),
                    }
                }
            }
        }
    }

    #[test]
    fn inside_areas_get_gardens_parks_and_playgrounds() {
        let mut kinds = Vec::new();
        for (level, def) in LEVELS.iter().enumerate() {
            let grid = Grid::parse(def.rows).unwrap();
            kinds.extend(plan_lots(&grid, level as u64).into_iter().flatten());
        }
        for kind in [Lot::Garden, Lot::Park, Lot::Playground] {
            assert!(kinds.contains(&kind), "no {kind:?} anywhere");
        }
    }
}
