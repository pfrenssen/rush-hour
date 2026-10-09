//! Traffic light heads and the hovered-junction highlight.

use bevy::prelude::*;

use super::shapes::ShapeBuilder;
use super::z;
use crate::input::HoveredNode;
use crate::lights::{Aspect, Signals};
use crate::map::CityMap;
use crate::map::graph::{HALF, NodeId};
use crate::map::grid::{Axis, TILE};
use crate::states::AppState;

const LAMP_RADIUS: f32 = 0.5;
const LAMP_SPACING: f32 = 1.15;

/// One lamp of a light head.
#[derive(Component)]
pub struct Lamp {
    node: NodeId,
    axis: Axis,
    aspect: Aspect,
}

/// Soft halo behind a lamp, visible while the lamp is lit.
#[derive(Component)]
pub struct Halo;

#[derive(Component)]
pub struct HoverRing;

#[derive(Resource)]
pub struct LampMaterials {
    lit: [Handle<ColorMaterial>; 3],
    dim: [Handle<ColorMaterial>; 3],
}

fn index(aspect: Aspect) -> usize {
    match aspect {
        Aspect::Red => 0,
        Aspect::Amber => 1,
        Aspect::Green => 2,
    }
}

impl FromWorld for LampMaterials {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        // Lit lamps use HDR colours so the bloom pass makes them glow.
        let lit = [
            Color::linear_rgb(3.0, 0.12, 0.08),
            Color::linear_rgb(2.5, 0.8, 0.05),
            Color::linear_rgb(0.12, 2.5, 0.5),
        ];
        let dim = [
            Color::srgb(0.25, 0.08, 0.08),
            Color::srgb(0.25, 0.18, 0.06),
            Color::srgb(0.07, 0.22, 0.10),
        ];
        LampMaterials {
            lit: lit.map(|c| materials.add(c)),
            dim: dim.map(|c| materials.add(c)),
        }
    }
}

pub fn spawn_lights(
    mut commands: Commands,
    map: Res<CityMap>,
    lamp_materials: Res<LampMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let graph = &map.graph;
    let housing_mesh = {
        let mut b = ShapeBuilder::new();
        b.rounded_rect(
            Vec2::ZERO,
            Vec2::new(LAMP_SPACING * 2.0 + 1.5, 1.5),
            0.75,
            Color::srgb(0.08, 0.09, 0.11),
        );
        b.rounded_rect(
            Vec2::new(0.0, 0.05),
            Vec2::new(LAMP_SPACING * 2.0 + 1.2, 1.2),
            0.6,
            Color::srgb(0.16, 0.17, 0.20),
        );
        meshes.add(b.build())
    };
    let shadow_mesh = {
        let mut b = ShapeBuilder::new();
        b.rounded_rect(
            Vec2::new(0.35, -0.35),
            Vec2::new(LAMP_SPACING * 2.0 + 1.5, 1.5),
            0.75,
            Color::srgba(0.0, 0.0, 0.0, 0.35),
        );
        meshes.add(b.build())
    };
    let lamp_mesh = {
        let mut b = ShapeBuilder::new();
        b.circle(Vec2::ZERO, LAMP_RADIUS, Color::WHITE);
        meshes.add(b.build())
    };
    let halo_mesh = {
        let mut b = ShapeBuilder::new();
        b.circle(Vec2::ZERO, LAMP_RADIUS * 2.6, Color::WHITE);
        meshes.add(b.build())
    };
    let white = materials.add(ColorMaterial::default());

    for (node_id, node) in graph.nodes.iter().enumerate() {
        if !node.is_signal() {
            continue;
        }
        for &lane in &node.incoming {
            let dir = graph.lanes[lane].dir().expect("road lane");
            let d = dir.vec();
            let r = dir.right().vec();
            // On the corner to the right of the approaching driver, just before the box.
            let pos = node.center - d * (HALF + 1.9) + r * (HALF + 1.0);
            let rotation = Quat::from_rotation_z(d.to_angle());
            commands
                .spawn((
                    DespawnOnExit(AppState::InGame),
                    Transform::from_translation(pos.extend(z::LIGHTS)).with_rotation(rotation),
                    Visibility::default(),
                ))
                .with_children(|head| {
                    head.spawn((
                        Mesh2d(shadow_mesh.clone()),
                        MeshMaterial2d(white.clone()),
                        Transform::from_xyz(0.0, 0.0, -0.02),
                    ));
                    head.spawn((
                        Mesh2d(housing_mesh.clone()),
                        MeshMaterial2d(white.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.0),
                    ));
                    // Red nearest the junction, green furthest away.
                    for (aspect, x) in [
                        (Aspect::Red, LAMP_SPACING),
                        (Aspect::Amber, 0.0),
                        (Aspect::Green, -LAMP_SPACING),
                    ] {
                        head.spawn((
                            Lamp {
                                node: node_id,
                                axis: dir.axis(),
                                aspect,
                            },
                            Mesh2d(lamp_mesh.clone()),
                            MeshMaterial2d(lamp_materials.dim[index(aspect)].clone()),
                            Transform::from_xyz(x, 0.0, 0.02),
                        ))
                        .with_child((
                            Halo,
                            Mesh2d(halo_mesh.clone()),
                            MeshMaterial2d(materials.add(halo_color(aspect))),
                            Transform::from_xyz(0.0, 0.0, -0.01),
                            Visibility::Hidden,
                        ));
                    }
                });
        }
    }

    // Highlight frame around the hovered junction.
    let mut ring = ShapeBuilder::new();
    let outer = TILE + 3.2;
    let t = 0.55;
    for (offset, size) in [
        (Vec2::new(0.0, (outer - t) / 2.0), Vec2::new(outer, t)),
        (Vec2::new(0.0, -(outer - t) / 2.0), Vec2::new(outer, t)),
        (
            Vec2::new((outer - t) / 2.0, 0.0),
            Vec2::new(t, outer - 2.0 * t),
        ),
        (
            Vec2::new(-(outer - t) / 2.0, 0.0),
            Vec2::new(t, outer - 2.0 * t),
        ),
    ] {
        ring.rect(offset, size, Color::WHITE);
    }
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        HoverRing,
        Mesh2d(meshes.add(ring.build())),
        MeshMaterial2d(materials.add(Color::srgba(1.0, 1.0, 1.0, 0.75))),
        Transform::from_xyz(0.0, 0.0, z::OVERLAY),
        Visibility::Hidden,
    ));
}

fn halo_color(aspect: Aspect) -> Color {
    match aspect {
        Aspect::Red => Color::srgba(1.0, 0.25, 0.2, 0.28),
        Aspect::Amber => Color::srgba(1.0, 0.7, 0.15, 0.28),
        Aspect::Green => Color::srgba(0.3, 1.0, 0.45, 0.28),
    }
}

pub fn update_lamps(
    signals: Res<Signals>,
    lamp_materials: Res<LampMaterials>,
    mut lamps: Query<(&Lamp, &mut MeshMaterial2d<ColorMaterial>, &Children)>,
    mut halos: Query<&mut Visibility, With<Halo>>,
) {
    for (lamp, mut material, children) in &mut lamps {
        let Some(Some(signal)) = signals.0.get(lamp.node) else {
            continue;
        };
        let on = signal.aspect(lamp.axis) == lamp.aspect;
        let wanted = if on {
            &lamp_materials.lit[index(lamp.aspect)]
        } else {
            &lamp_materials.dim[index(lamp.aspect)]
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
        for child in children.iter() {
            if let Ok(mut vis) = halos.get_mut(child) {
                vis.set_if_neq(if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

pub fn update_hover_ring(
    hovered: Res<HoveredNode>,
    map: Option<Res<CityMap>>,
    signals: Res<Signals>,
    time: Res<Time>,
    ring: Single<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial2d<ColorMaterial>,
        ),
        With<HoverRing>,
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (mut transform, mut vis, material) = ring.into_inner();
    let (Some(node), Some(map)) = (hovered.0, map) else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    vis.set_if_neq(Visibility::Inherited);
    let center = map.graph.nodes[node].center;
    transform.translation.x = center.x;
    transform.translation.y = center.y;
    let progress = signals
        .0
        .get(node)
        .and_then(|s| s.as_ref())
        .and_then(|s| s.transition_progress());
    // While switching the frame turns amber and pulses: further clicks are ignored.
    let color = match progress {
        Some(p) => Color::srgba(
            1.0,
            0.75,
            0.25,
            0.45 + 0.3 * p + 0.15 * (time.elapsed_secs() * 10.0).sin(),
        ),
        None => Color::srgba(1.0, 1.0, 1.0, 0.8),
    };
    if materials.get(&material.0).is_some_and(|m| m.color != color)
        && let Some(mut m) = materials.get_mut(&material.0)
    {
        m.color = color;
    }
}
