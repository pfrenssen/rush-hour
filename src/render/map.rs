//! Static map visuals: ground, roads, markings, buildings and trees.

use bevy::prelude::*;

use super::shapes::ShapeBuilder;
use super::{palette, z};
use crate::map::CityMap;
use crate::map::graph::{EDGE_EXTENSION, HALF, LANE_OFFSET, LaneGraph, STOP_SETBACK};
use crate::map::grid::{Cell, Dir, Grid, TILE};
use crate::states::AppState;

const SIDEWALK: f32 = 1.0;

pub fn spawn_map(
    mut commands: Commands,
    map: Res<CityMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let grid = &map.grid;
    let graph = &map.graph;
    let mut ground = ShapeBuilder::new();
    let mut marks = ShapeBuilder::new();
    let mut shadows = ShapeBuilder::new();
    let mut tops = ShapeBuilder::new();

    // A frame around the board.
    let size = grid.world_size();
    ground.rounded_rect(
        Vec2::ZERO,
        size + Vec2::splat(2.0 * EDGE_EXTENSION),
        3.0,
        palette::FRAME,
    );
    ground.rect(Vec2::ZERO, size + Vec2::splat(0.6), palette::CURB);

    for row in 0..grid.height {
        for col in 0..grid.width {
            let center = grid.cell_center(col, row);
            let cell = grid.cell(col, row);
            let tile = Vec2::splat(TILE);
            let mut rng = fastrand::Rng::with_seed(cell_seed(map.level, col, row));
            match cell {
                Cell::Block => {
                    ground.rect(center, tile, palette::LAWN);
                    sidewalks(&mut ground, grid, col, row);
                    buildings(&mut shadows, &mut tops, center, &mut rng);
                }
                Cell::Park => {
                    ground.rect(center, tile, palette::PARK);
                    sidewalks(&mut ground, grid, col, row);
                    trees(&mut shadows, &mut tops, center, &mut rng);
                }
                Cell::Water => {
                    ground.rect(center, tile, palette::WATER);
                    ripples(&mut marks, center, &mut rng);
                }
                Cell::RoadH | Cell::RoadV | Cell::Junction => {
                    ground.rect(center, tile + Vec2::splat(0.02), palette::ASPHALT);
                    centre_dashes(&mut marks, grid, col, row);
                    bridge_rails(&mut marks, grid, col, row);
                }
            }
        }
    }

    edge_fades(&mut ground, graph);
    stop_lines(&mut marks, graph);

    for (builder, z) in [
        (ground, z::GROUND),
        (marks, z::MARKINGS),
        (shadows, z::SHADOWS),
        (tops, z::BUILDINGS),
    ] {
        if builder.is_empty() {
            continue;
        }
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            Mesh2d(meshes.add(builder.build())),
            MeshMaterial2d(materials.add(ColorMaterial::default())),
            Transform::from_xyz(0.0, 0.0, z),
        ));
    }
}

fn cell_seed(level: usize, col: usize, row: usize) -> u64 {
    (level as u64 + 1) * 1_000_003 + (row as u64) * 7_919 + col as u64 * 104_729
}

fn sidewalks(b: &mut ShapeBuilder, grid: &Grid, col: usize, row: usize) {
    let center = grid.cell_center(col, row);
    for dir in Dir::ALL {
        if grid.neighbour(col, row, dir).is_some_and(Cell::is_road) {
            let along = dir.right().vec();
            let pos = center + dir.vec() * (HALF - SIDEWALK / 2.0);
            b.rotated_rect(pos, Vec2::new(TILE, SIDEWALK), along, palette::SIDEWALK);
            // Thin curb line between the sidewalk and the road.
            b.rotated_rect(
                center + dir.vec() * (HALF - 0.08),
                Vec2::new(TILE, 0.16),
                along,
                palette::CURB,
            );
        }
    }
}

fn buildings(
    shadows: &mut ShapeBuilder,
    tops: &mut ShapeBuilder,
    center: Vec2,
    rng: &mut fastrand::Rng,
) {
    let roof = palette::ROOFS[rng.usize(..palette::ROOFS.len())];
    let style = rng.u8(0..10);
    let margin = SIDEWALK + 0.5;
    let max = TILE - 2.0 * margin;
    if style < 7 {
        // One building.
        let size = Vec2::new(rng.f32() * 1.6 + max - 1.6, rng.f32() * 1.6 + max - 1.6);
        let slack = (Vec2::splat(max) - size) / 2.0;
        let pos = center
            + Vec2::new(
                (rng.f32() * 2.0 - 1.0) * slack.x,
                (rng.f32() * 2.0 - 1.0) * slack.y,
            );
        house(shadows, tops, pos, size, roof);
    } else if style < 9 {
        // Two narrow buildings side by side.
        let vertical = rng.bool();
        let (a, b) = if vertical {
            (Vec2::new(0.0, max / 4.0), Vec2::new(0.0, -max / 4.0))
        } else {
            (Vec2::new(max / 4.0, 0.0), Vec2::new(-max / 4.0, 0.0))
        };
        let size = if vertical {
            Vec2::new(max - 0.4, max / 2.0 - 0.3)
        } else {
            Vec2::new(max / 2.0 - 0.3, max - 0.4)
        };
        let roof2 = palette::ROOFS[rng.usize(..palette::ROOFS.len())];
        house(shadows, tops, center + a, size, roof);
        house(shadows, tops, center + b, size, roof2);
    } else {
        // A small yard with a tree.
        tree(shadows, tops, center + Vec2::new(0.6, 0.4), 1.6);
    }
}

fn house(shadows: &mut ShapeBuilder, tops: &mut ShapeBuilder, pos: Vec2, size: Vec2, roof: Color) {
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

fn trees(
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

fn tree(shadows: &mut ShapeBuilder, tops: &mut ShapeBuilder, pos: Vec2, r: f32) {
    shadows.circle(pos + Vec2::new(0.4, -0.4), r, palette::SHADOW);
    tops.circle(pos, r, palette::TREE);
    tops.circle(
        pos + Vec2::new(-0.25, 0.25) * r,
        r * 0.55,
        palette::TREE_LIGHT,
    );
}

fn ripples(b: &mut ShapeBuilder, center: Vec2, rng: &mut fastrand::Rng) {
    for _ in 0..2 {
        let pos = center + Vec2::new(rng.f32() * 4.0 - 2.0, rng.f32() * 4.0 - 2.0);
        b.rounded_rect(pos, Vec2::new(2.2, 0.22), 0.1, palette::RIPPLE);
    }
}

fn centre_dashes(b: &mut ShapeBuilder, grid: &Grid, col: usize, row: usize) {
    if grid.is_node(col, row) {
        return;
    }
    let center = grid.cell_center(col, row);
    let along = match grid.cell(col, row) {
        Cell::RoadV => Vec2::Y,
        Cell::RoadH => Vec2::X,
        _ if grid.opens(col, row, Dir::N) => Vec2::Y,
        _ => Vec2::X,
    };
    // Solid lines right next to a junction, dashes elsewhere.
    let next_to_node = Dir::ALL.iter().any(|&d| {
        let (dc, dr) = d.step();
        let (c, r) = (col as isize + dc, row as isize + dr);
        grid.get(c, r).is_some() && grid.is_node(c as usize, r as usize)
    });
    if next_to_node {
        b.rotated_rect(center, Vec2::new(TILE, 0.22), along, palette::MARKING);
    } else {
        for offset in [-2.0, 2.0] {
            b.rotated_rect(
                center + along * offset,
                Vec2::new(2.0, 0.22),
                along,
                palette::MARKING,
            );
        }
    }
}

fn bridge_rails(b: &mut ShapeBuilder, grid: &Grid, col: usize, row: usize) {
    let center = grid.cell_center(col, row);
    for dir in Dir::ALL {
        if grid.neighbour(col, row, dir) == Some(Cell::Water) {
            b.rotated_rect(
                center + dir.vec() * (HALF - 0.25),
                Vec2::new(TILE, 0.5),
                dir.right().vec(),
                palette::RAIL,
            );
        }
    }
}

/// Roads fade into the frame where they leave the map.
fn edge_fades(b: &mut ShapeBuilder, graph: &LaneGraph) {
    for t in &graph.terminals {
        let out = t.side.vec();
        let across = t.side.right().vec() * HALF;
        let near = t.pos;
        let far = t.pos + out * EDGE_EXTENSION;
        let solid = palette::ASPHALT;
        let clear = palette::FRAME;
        b.gradient_quad(
            [near - across, near + across, far + across, far - across],
            [solid, solid, clear, clear],
        );
    }
}

fn stop_lines(b: &mut ShapeBuilder, graph: &LaneGraph) {
    for node in graph.nodes.iter().filter(|n| n.is_signal()) {
        for &lane in &node.incoming {
            let l = &graph.lanes[lane];
            let dir = l.dir().expect("road lane").vec();
            let end = l.geom.end();
            // Stop line across the lane, set back from the junction box.
            b.rotated_rect(
                end - dir * (STOP_SETBACK - 0.25),
                Vec2::new(0.5, LANE_OFFSET * 2.0 - 0.3),
                dir,
                palette::MARKING,
            );
            // Zebra crossing between the stop line and the box, over the full road width.
            let across = -dir.perp(); // points to the right of travel
            let road_mid = end - across * LANE_OFFSET - dir * (STOP_SETBACK / 2.0 + 0.1);
            let stripes = 7;
            for i in 0..stripes {
                let t = (i as f32 + 0.5) / stripes as f32 - 0.5;
                b.rotated_rect(
                    road_mid + across * (t * (TILE - 1.2)),
                    Vec2::new(STOP_SETBACK - 0.9, 0.5),
                    dir,
                    palette::ZEBRA,
                );
            }
        }
    }
}
