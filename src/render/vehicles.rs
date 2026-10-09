//! Vehicle visuals: rounded bodies with a cabin, headlights, brake lights that glow when braking,
//! blinking indicators before turns, a patience tint and a honk bubble.

use bevy::prelude::*;

use super::shapes::ShapeBuilder;
use super::z;
use crate::map::CityMap;
use crate::map::graph::Turn;
use crate::vehicles::kind::VehicleKind;
use crate::vehicles::traffic::{Vehicle, next_junction};

const CAR_PAINTS: [Color; 10] = [
    Color::srgb(0.91, 0.33, 0.30),
    Color::srgb(0.96, 0.56, 0.22),
    Color::srgb(0.98, 0.80, 0.25),
    Color::srgb(0.16, 0.66, 0.62),
    Color::srgb(0.27, 0.56, 0.89),
    Color::srgb(0.42, 0.42, 0.86),
    Color::srgb(0.66, 0.42, 0.82),
    Color::srgb(0.92, 0.93, 0.95),
    Color::srgb(0.34, 0.37, 0.43),
    Color::srgb(0.45, 0.80, 0.55),
];
const BUS_PAINTS: [Color; 3] = [
    Color::srgb(0.98, 0.74, 0.18),
    Color::srgb(0.20, 0.55, 0.85),
    Color::srgb(0.86, 0.30, 0.28),
];
const ANGRY: Color = Color::srgb(0.95, 0.12, 0.10);
/// Distance before a turn at which indicators start blinking.
const SIGNAL_DISTANCE: f32 = 30.0;

/// Per-vehicle handles to the parts that change while driving.
#[derive(Component)]
pub struct VehicleLook {
    paint: Color,
    body: Handle<ColorMaterial>,
    brakes: [Entity; 2],
    /// Front-left, rear-left, front-right, rear-right.
    indicators: [Entity; 4],
    beacons: Option<[Entity; 2]>,
    honk: Entity,
}

#[derive(Component)]
pub struct HonkBubble;

#[derive(Resource)]
pub struct VehicleAssets {
    shapes: [KindMeshes; 4],
    lamp: Handle<Mesh>,
    blinker: Handle<Mesh>,
    beacon: Handle<Mesh>,
    bubble: Handle<Mesh>,
    head: Handle<ColorMaterial>,
    brake_dim: Handle<ColorMaterial>,
    brake_lit: Handle<ColorMaterial>,
    amber: Handle<ColorMaterial>,
    beacon_red: Handle<ColorMaterial>,
    beacon_blue: Handle<ColorMaterial>,
    beacon_off: Handle<ColorMaterial>,
    plain: Handle<ColorMaterial>,
}

struct KindMeshes {
    shadow: Handle<Mesh>,
    body: Handle<Mesh>,
    /// Vertex-coloured details drawn over the body (glass, cargo box, stripes).
    details: Handle<Mesh>,
    /// Roof panel in body colour, drawn over the details.
    roof: Option<Handle<Mesh>>,
}

fn kind_index(kind: VehicleKind) -> usize {
    match kind {
        VehicleKind::Car => 0,
        VehicleKind::Bus => 1,
        VehicleKind::Truck => 2,
        VehicleKind::Ambulance => 3,
    }
}

const GLASS: Color = Color::srgb(0.11, 0.14, 0.19);

fn kind_meshes(kind: VehicleKind, meshes: &mut Assets<Mesh>) -> KindMeshes {
    let p = kind.params();
    let (l, w) = (p.length, p.width);
    let mut shadow = ShapeBuilder::new();
    shadow.rounded_rect(
        Vec2::new(0.25, -0.25),
        Vec2::new(l + 0.5, w + 0.5),
        0.8,
        Color::srgba(0.0, 0.0, 0.0, 0.3),
    );
    let mut body = ShapeBuilder::new();
    body.rounded_rect(Vec2::ZERO, Vec2::new(l, w), 0.55, Color::WHITE);
    let mut details = ShapeBuilder::new();
    let mut roof = ShapeBuilder::new();
    match kind {
        VehicleKind::Car => {
            details.rounded_rect(
                Vec2::new(-0.04 * l, 0.0),
                Vec2::new(0.54 * l, 0.82 * w),
                0.35,
                GLASS,
            );
            roof.rounded_rect(
                Vec2::new(-0.09 * l, 0.0),
                Vec2::new(0.32 * l, 0.70 * w),
                0.3,
                Color::WHITE,
            );
        }
        VehicleKind::Bus => {
            details.rounded_rect(Vec2::ZERO, Vec2::new(l - 0.5, w - 0.3), 0.4, GLASS);
            roof.rounded_rect(
                Vec2::new(-0.15, 0.0),
                Vec2::new(l - 1.3, w - 0.75),
                0.3,
                Color::WHITE,
            );
        }
        VehicleKind::Truck => {
            let cargo = l - 2.5;
            details.rounded_rect(
                Vec2::new(l / 2.0 - 1.3, 0.0),
                Vec2::new(0.9, w * 0.8),
                0.3,
                GLASS,
            );
            details.rounded_rect(
                Vec2::new(-l / 2.0 + cargo / 2.0, 0.0),
                Vec2::new(cargo, w + 0.05),
                0.3,
                Color::srgb(0.86, 0.87, 0.89),
            );
            details.rect(
                Vec2::new(-l / 2.0 + cargo / 2.0, 0.0),
                Vec2::new(cargo - 0.8, 0.12),
                Color::srgb(0.72, 0.74, 0.77),
            );
        }
        VehicleKind::Ambulance => {
            details.rounded_rect(
                Vec2::new(l / 2.0 - 1.15, 0.0),
                Vec2::new(0.8, w * 0.84),
                0.3,
                GLASS,
            );
            // Red cross on the roof.
            let c = Vec2::new(-0.6, 0.0);
            let red = Color::srgb(0.88, 0.16, 0.16);
            details.rect(c, Vec2::new(1.6, 0.5), red);
            details.rect(c, Vec2::new(0.5, 1.6), red);
            details.rect(
                Vec2::new(0.0, w / 2.0 - 0.15),
                Vec2::new(l - 0.8, 0.18),
                red,
            );
            details.rect(
                Vec2::new(0.0, -w / 2.0 + 0.15),
                Vec2::new(l - 0.8, 0.18),
                red,
            );
        }
    }
    KindMeshes {
        shadow: meshes.add(shadow.build()),
        body: meshes.add(body.build()),
        details: meshes.add(details.build()),
        roof: (!roof.is_empty()).then(|| meshes.add(roof.build())),
    }
}

impl FromWorld for VehicleAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let shapes = VehicleKind::ALL.map(|k| kind_meshes(k, &mut meshes));
        let mut mesh = |f: &dyn Fn(&mut ShapeBuilder)| {
            let mut b = ShapeBuilder::new();
            f(&mut b);
            meshes.add(b.build())
        };
        let lamp = mesh(&|b| b.rounded_rect(Vec2::ZERO, Vec2::new(0.3, 0.5), 0.12, Color::WHITE));
        let blinker = mesh(&|b| b.circle(Vec2::ZERO, 0.26, Color::WHITE));
        let beacon =
            mesh(&|b| b.rounded_rect(Vec2::ZERO, Vec2::new(0.45, 0.7), 0.15, Color::WHITE));
        let bubble = mesh(&|b| {
            b.circle(Vec2::ZERO, 1.5, Color::WHITE);
            // Speech-bubble tail pointing at the vehicle.
            b.convex(
                &[
                    Vec2::new(-0.5, -1.2),
                    Vec2::new(0.3, -1.3),
                    Vec2::new(-0.6, -2.1),
                ],
                Color::WHITE,
            );
        });
        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        VehicleAssets {
            shapes,
            lamp,
            blinker,
            beacon,
            bubble,
            head: materials.add(Color::linear_rgb(1.6, 1.5, 1.1)),
            brake_dim: materials.add(Color::srgb(0.45, 0.06, 0.06)),
            brake_lit: materials.add(Color::linear_rgb(4.0, 0.15, 0.1)),
            amber: materials.add(Color::linear_rgb(3.0, 1.2, 0.05)),
            beacon_red: materials.add(Color::linear_rgb(4.0, 0.2, 0.15)),
            beacon_blue: materials.add(Color::linear_rgb(0.2, 0.6, 5.0)),
            beacon_off: materials.add(Color::srgb(0.35, 0.36, 0.40)),
            plain: materials.add(ColorMaterial::default()),
        }
    }
}

fn paint_for(v: &Vehicle) -> Color {
    let i = v.paint as usize;
    match v.kind {
        VehicleKind::Car | VehicleKind::Truck => CAR_PAINTS[i % CAR_PAINTS.len()],
        VehicleKind::Bus => BUS_PAINTS[i % BUS_PAINTS.len()],
        VehicleKind::Ambulance => Color::srgb(0.96, 0.96, 0.97),
    }
}

fn base_z(v: &Vehicle) -> f32 {
    z::VEHICLES + (v.id % 64) as f32 * 0.005
}

/// Give freshly spawned vehicles their looks.
pub fn dress_vehicles(
    mut commands: Commands,
    new: Query<(Entity, &Vehicle), Added<Vehicle>>,
    map: Res<CityMap>,
    assets: Res<VehicleAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (entity, v) in &new {
        let p = v.params;
        let (l, w) = (p.length, p.width);
        let shapes = &assets.shapes[kind_index(v.kind)];
        let paint = paint_for(v);
        let body = materials.add(paint);
        let z0 = base_z(v);
        let child = |mesh: &Handle<Mesh>, material: &Handle<ColorMaterial>, pos: Vec3| {
            (
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(pos),
            )
        };
        let mut e = commands.entity(entity);
        e.insert((pose_transform(v, &map), Visibility::default()));
        // Shadows share one layer below all vehicle bodies.
        e.with_child(child(
            &shapes.shadow,
            &assets.plain,
            Vec3::new(0.0, 0.0, z::VEHICLE_SHADOWS - z0),
        ));
        e.with_child(child(&shapes.body, &body, Vec3::ZERO));
        e.with_child(child(
            &shapes.details,
            &assets.plain,
            Vec3::new(0.0, 0.0, 0.001),
        ));
        if let Some(roof) = &shapes.roof {
            e.with_child(child(roof, &body, Vec3::new(0.0, 0.0, 0.002)));
        }
        for side in [-1.0, 1.0] {
            e.with_child(child(
                &assets.lamp,
                &assets.head,
                Vec3::new(l / 2.0 - 0.2, side * (w / 2.0 - 0.38), 0.003),
            ));
        }
        let mut brakes = [Entity::PLACEHOLDER; 2];
        for (k, side) in [-1.0, 1.0].into_iter().enumerate() {
            brakes[k] = commands
                .spawn((
                    child(
                        &assets.lamp,
                        &assets.brake_dim,
                        Vec3::new(-l / 2.0 + 0.17, side * (w / 2.0 - 0.38), 0.003),
                    ),
                    ChildOf(entity),
                ))
                .id();
        }
        let mut indicators = [Entity::PLACEHOLDER; 4];
        for (k, (x, y)) in [
            (l / 2.0 - 0.3, w / 2.0 - 0.12),
            (-l / 2.0 + 0.3, w / 2.0 - 0.12),
            (l / 2.0 - 0.3, -w / 2.0 + 0.12),
            (-l / 2.0 + 0.3, -w / 2.0 + 0.12),
        ]
        .into_iter()
        .enumerate()
        {
            indicators[k] = commands
                .spawn((
                    child(&assets.blinker, &assets.amber, Vec3::new(x, y, 0.004)),
                    Visibility::Hidden,
                    ChildOf(entity),
                ))
                .id();
        }
        let beacons = (v.kind == VehicleKind::Ambulance).then(|| {
            [-1.0, 1.0].map(|side| {
                commands
                    .spawn((
                        child(
                            &assets.beacon,
                            &assets.beacon_off,
                            Vec3::new(0.55, side * 0.42, 0.004),
                        ),
                        ChildOf(entity),
                    ))
                    .id()
            })
        });
        let honk = commands
            .spawn((
                HonkBubble,
                Mesh2d(assets.bubble.clone()),
                MeshMaterial2d(materials.add(Color::WHITE)),
                Transform::from_xyz(0.0, 0.0, z::BUBBLES - z0),
                Visibility::Hidden,
                ChildOf(entity),
                children![(
                    Text2d::new("!"),
                    TextFont {
                        font_size: FontSize::Px(40.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.12, 0.12, 0.14)),
                    Transform::from_xyz(0.0, 0.05, 0.01).with_scale(Vec3::splat(0.065)),
                )],
            ))
            .id();
        commands.entity(entity).insert(VehicleLook {
            paint,
            body,
            brakes,
            indicators,
            beacons,
            honk,
        });
    }
}

fn pose_transform(v: &Vehicle, map: &CityMap) -> Transform {
    let (front, rear) = v.pose(&map.graph);
    let center = (front + rear) / 2.0;
    let dir = front - rear;
    let angle = if dir.length_squared() > 1e-6 {
        dir.y.atan2(dir.x)
    } else {
        0.0
    };
    Transform::from_translation(center.extend(base_z(v)))
        .with_rotation(Quat::from_rotation_z(angle))
}

pub fn animate_vehicles(
    map: Res<CityMap>,
    time: Res<Time<Real>>,
    assets: Res<VehicleAssets>,
    mut vehicles: Query<(&Vehicle, &VehicleLook, &mut Transform), Without<HonkBubble>>,
    mut parts: Query<(&mut MeshMaterial2d<ColorMaterial>, &mut Visibility), Without<HonkBubble>>,
    mut bubbles: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial2d<ColorMaterial>,
        ),
        With<HonkBubble>,
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let t = time.elapsed_secs();
    let blink_on = (t * 3.0).fract() < 0.5;
    for (v, look, mut transform) in &mut vehicles {
        *transform = pose_transform(v, &map);

        // Impatience tints the body towards red.
        let anger = ((1.0 - v.patience) * 1.25 - 0.2).clamp(0.0, 1.0).powf(1.3) * 0.85;
        let color = look.paint.mix(&ANGRY, anger);
        if materials.get(&look.body).is_some_and(|m| m.color != color)
            && let Some(mut m) = materials.get_mut(&look.body)
        {
            m.color = color;
        }

        let braking = v.acc < -0.8 || v.v < 0.3;
        let brake = if braking {
            &assets.brake_lit
        } else {
            &assets.brake_dim
        };
        for &e in &look.brakes {
            if let Ok((mut m, _)) = parts.get_mut(e)
                && m.0 != *brake
            {
                m.0 = brake.clone();
            }
        }

        let turn = upcoming_turn(v, &map);
        for (k, &e) in look.indicators.iter().enumerate() {
            let left_side = k < 2;
            let on = blink_on
                && match turn {
                    Some(Turn::Left) => left_side,
                    Some(Turn::Right) => !left_side,
                    _ => false,
                };
            if let Ok((_, mut vis)) = parts.get_mut(e) {
                vis.set_if_neq(if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }

        if let Some(beacons) = look.beacons {
            let phase = (t * 4.0).fract() < 0.5;
            for (k, &e) in beacons.iter().enumerate() {
                let m = match (k == 0) == phase {
                    true => &assets.beacon_red,
                    false => &assets.beacon_blue,
                };
                if let Ok((mut mat, _)) = parts.get_mut(e)
                    && mat.0 != *m
                {
                    mat.0 = m.clone();
                }
            }
        }

        // The honk bubble stays upright above the vehicle.
        if let Ok((mut bt, mut vis, bm)) = bubbles.get_mut(look.honk) {
            let show = v.patience < 0.55;
            vis.set_if_neq(if show {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
            if show {
                let inv = transform.rotation.inverse();
                let z_local = bt.translation.z;
                bt.rotation = inv;
                bt.translation = (inv * Vec3::new(0.0, 3.4, 0.0)).with_z(z_local);
                let urgency = (1.0 - v.patience / 0.55).clamp(0.0, 1.0);
                let pulse = if v.patience < 0.25 {
                    1.0 + 0.15 * (t * 12.0).sin()
                } else {
                    1.0
                };
                bt.scale = Vec3::splat(pulse);
                let c = Color::srgb(1.0, 0.92 - 0.6 * urgency, 0.35 - 0.25 * urgency);
                if materials.get(&bm.0).is_some_and(|m| m.color != c)
                    && let Some(mut m) = materials.get_mut(&bm.0)
                {
                    m.color = c;
                }
            }
        }
    }
}

fn upcoming_turn(v: &Vehicle, map: &CityMap) -> Option<Turn> {
    if let Some(turn) = map.graph.lanes[v.lane()].turn() {
        // Keep blinking through the turn itself.
        return Some(turn);
    }
    let a = next_junction(&map.graph, v)?;
    (a.dist < SIGNAL_DISTANCE)
        .then(|| map.graph.lanes[a.turn].turn())
        .flatten()
}
