//! Short-lived feedback: score pop-ups at exits, a red flash when a life is lost, and warnings
//! at blocked spawn points.

use bevy::prelude::*;

use super::camera::MainCamera;
use super::shapes::ShapeBuilder;
use super::z;
use crate::map::CityMap;
use crate::map::graph::LANE_OFFSET;
use crate::map::grid::TILE;
use crate::scoring::{LifeLost, LossReason, Scored};
use crate::states::AppState;
use crate::vehicles::spawn::{BLOCK_LIMIT, Spawner};

/// Spawn points show a warning once they have been blocked this long.
const WARN_AFTER: f32 = 1.0;

#[derive(Component)]
pub struct Popup {
    age: f32,
    life: f32,
    scale: f32,
    color: Color,
}

#[derive(Component)]
pub struct Flash {
    age: f32,
}

#[derive(Component)]
pub struct BlockedMarker(usize);

#[derive(Component)]
pub struct Countdown;

fn camera_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    }
}

fn popup(commands: &mut Commands, pos: Vec2, text: String, color: Color, px: f32, scale: f32) {
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Popup {
            age: 0.0,
            life: 1.4,
            scale: scale * px / 32.0,
            color,
        },
        Text2d::new(text),
        TextFont {
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextLayout::justify(Justify::Center),
        TextColor(color),
        Transform::from_translation(pos.extend(z::POPUPS)).with_scale(Vec3::ZERO),
    ));
}

pub fn spawn_feedback(
    mut commands: Commands,
    mut scored: MessageReader<Scored>,
    mut lost: MessageReader<LifeLost>,
    projection: Single<&Projection, With<MainCamera>>,
) {
    let scale = camera_scale(&projection);
    for s in scored.read() {
        let a = s.award;
        let mut text = format!("+{}", a.total());
        if a.bonus > 0 {
            text.push_str("\nFAST!");
        }
        let color = if a.multiplier > 1 || a.bonus > 0 {
            Color::srgb(1.0, 0.82, 0.30)
        } else {
            Color::srgb(0.55, 0.95, 0.65)
        };
        popup(&mut commands, s.pos, text, color, 20.0, scale);
    }
    for l in lost.read() {
        let text = match l.reason {
            LossReason::Impatient => "-1 LIFE\nfed up!",
            LossReason::Blocked => "-1 LIFE\nblocked!",
        };
        popup(
            &mut commands,
            l.pos + Vec2::Y * 3.0,
            text.to_string(),
            Color::srgb(1.0, 0.35, 0.32),
            24.0,
            scale,
        );
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            Flash { age: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::srgba(0.9, 0.1, 0.1, 0.0)),
            GlobalZIndex(5),
        ));
    }
}

pub fn animate_feedback(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut popups: Query<(Entity, &mut Popup, &mut Transform, &mut TextColor)>,
    mut flashes: Query<(Entity, &mut Flash, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut transform, mut color) in &mut popups {
        p.age += dt;
        let t = p.age / p.life;
        if t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Pop in, float upwards, fade out.
        let grow = (t / 0.12).min(1.0);
        let overshoot = 1.0 + 0.25 * (grow * std::f32::consts::PI).sin();
        transform.scale = Vec3::splat(p.scale * grow * overshoot);
        transform.translation.y += dt * 6.0;
        color.0 = p.color.with_alpha(1.0 - t * t);
    }
    for (e, mut f, mut bg) in &mut flashes {
        f.age += dt;
        let life = 0.7;
        if f.age >= life {
            commands.entity(e).despawn();
            continue;
        }
        let t = f.age / life;
        bg.0 = bg.0.with_alpha(0.4 * (1.0 - t) * (1.0 - t));
    }
}

pub fn spawn_blocked_markers(
    mut commands: Commands,
    map: Res<CityMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut b = ShapeBuilder::new();
    b.circle(Vec2::ZERO, 2.6, Color::srgba(0.0, 0.0, 0.0, 0.35));
    b.circle(Vec2::ZERO, 2.2, Color::linear_rgb(2.2, 0.15, 0.12));
    b.circle(Vec2::ZERO, 1.6, Color::srgb(0.75, 0.08, 0.08));
    let mesh = meshes.add(b.build());
    let material = materials.add(ColorMaterial::default());
    for (i, t) in map.graph.terminals.iter().enumerate() {
        // Next to the entry lane, just inside the map edge.
        let pos = t.pos - t.side.vec() * (TILE * 0.9) - t.side.right().vec() * (LANE_OFFSET + 3.6);
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            BlockedMarker(i),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(pos.extend(z::BUBBLES)),
            Visibility::Hidden,
            children![(
                Countdown,
                Text2d::new(""),
                TextFont {
                    font_size: FontSize::Px(32.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, 0.1, 0.01).with_scale(Vec3::splat(0.075)),
            )],
        ));
    }
}

pub fn update_blocked_markers(
    spawner: Res<Spawner>,
    time: Res<Time<Real>>,
    mut markers: Query<(&BlockedMarker, &mut Visibility, &mut Transform, &Children)>,
    mut texts: Query<&mut Text2d, With<Countdown>>,
) {
    for (marker, mut vis, mut transform, children) in &mut markers {
        let blocked = spawner.blocked_for.get(marker.0).copied().unwrap_or(0.0);
        let show = blocked > WARN_AFTER;
        vis.set_if_neq(if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !show {
            continue;
        }
        let pulse = 1.0 + 0.12 * (time.elapsed_secs() * 8.0).sin();
        transform.scale = Vec3::splat(pulse);
        let left = (BLOCK_LIMIT - blocked).ceil().max(0.0) as u32;
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                let s = left.to_string();
                if text.0 != s {
                    text.0 = s;
                }
            }
        }
    }
}
