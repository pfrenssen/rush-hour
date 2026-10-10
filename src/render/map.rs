//! Static map visuals: ground, roads and markings; block contents come from [`super::blocks`].

use bevy::prelude::*;

use super::blocks::{self, SIDEWALK};
use super::shapes::ShapeBuilder;
use super::{palette, z};
use crate::map::CityMap;
use crate::map::graph::{EDGE_EXTENSION, HALF, LANE_OFFSET, LaneGraph, STOP_SETBACK};
use crate::map::grid::{Cell, Dir, Grid, TILE};
use crate::states::AppState;

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

    let lots = blocks::plan_lots(grid, map.level as u64);
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
                    if let Some(lot) = &lots[row * grid.width + col] {
                        blocks::draw_lot(
                            lot,
                            center,
                            &mut rng,
                            &mut ground,
                            &mut shadows,
                            &mut tops,
                        );
                    }
                }
                Cell::Park => {
                    ground.rect(center, tile, palette::PARK);
                    sidewalks(&mut ground, grid, col, row);
                    blocks::trees(&mut shadows, &mut tops, center, &mut rng);
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

fn ripples(b: &mut ShapeBuilder, center: Vec2, rng: &mut fastrand::Rng) {
    for _ in 0..2 {
        let pos = center + Vec2::new(rng.f32() * 4.0 - 2.0, rng.f32() * 4.0 - 2.0);
        b.rounded_rect(pos, Vec2::new(2.2, 0.22), 0.1, palette::RIPPLE);
    }
}

fn centre_dashes(b: &mut ShapeBuilder, grid: &Grid, col: usize, row: usize) {
    let center = grid.cell_center(col, row);
    if grid.is_node(col, row) {
        let open = grid.openings(col, row);
        if open.len() == 2 {
            bend_dashes(b, center, open[0], open[1]);
        }
        return;
    }
    let along = match grid.cell(col, row) {
        Cell::RoadV => Vec2::Y,
        Cell::RoadH => Vec2::X,
        _ if grid.opens(col, row, Dir::N) => Vec2::Y,
        _ => Vec2::X,
    };
    // Solid lines right next to a signalled junction, dashes elsewhere.
    let next_to_signal = Dir::ALL.iter().any(|&d| {
        let (dc, dr) = d.step();
        let (c, r) = (col as isize + dc, row as isize + dr);
        grid.get(c, r).is_some()
            && grid.is_node(c as usize, r as usize)
            && grid.openings(c as usize, r as usize).len() >= 3
    });
    if next_to_signal {
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

/// Dashed centre line curving through a bend between the openings `a` and `b`.
fn bend_dashes(b: &mut ShapeBuilder, center: Vec2, a: Dir, c: Dir) {
    let pivot = center + (a.vec() + c.vec()) * HALF;
    let start = (-c.vec()).to_angle();
    let mut sweep = (-a.vec()).to_angle() - start;
    if sweep > std::f32::consts::PI {
        sweep -= std::f32::consts::TAU;
    } else if sweep < -std::f32::consts::PI {
        sweep += std::f32::consts::TAU;
    }
    let point = |t: f32| pivot + HALF * Vec2::from_angle(start + sweep * t);
    for (t0, t1) in [(0.16, 0.48), (0.52, 0.84)] {
        let pts: Vec<Vec2> = (0..=4)
            .map(|i| point(t0 + (t1 - t0) * i as f32 / 4.0))
            .collect();
        b.stroke(&pts, 0.22, palette::MARKING);
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
