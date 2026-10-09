//! Builds flat, vertex-coloured 2D meshes out of simple shapes.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Default)]
pub struct ShapeBuilder {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl ShapeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn vertex(&mut self, p: Vec2, color: [f32; 4]) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push([p.x, p.y, 0.0]);
        self.colors.push(color);
        i
    }

    /// Convex polygon, triangulated as a fan from its centroid.
    pub fn convex(&mut self, outline: &[Vec2], color: Color) {
        if outline.len() < 3 {
            return;
        }
        let c = LinearRgba::from(color).to_f32_array();
        let centroid = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
        let center = self.vertex(centroid, c);
        let first = self.positions.len() as u32;
        for &p in outline {
            self.vertex(p, c);
        }
        let n = outline.len() as u32;
        for i in 0..n {
            self.indices
                .extend([center, first + i, first + (i + 1) % n]);
        }
    }

    /// Quad with one colour per corner, for gradients.
    pub fn gradient_quad(&mut self, corners: [Vec2; 4], colors: [Color; 4]) {
        let first = self.positions.len() as u32;
        for (p, c) in corners.into_iter().zip(colors) {
            self.vertex(p, LinearRgba::from(c).to_f32_array());
        }
        self.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    pub fn rect(&mut self, center: Vec2, size: Vec2, color: Color) {
        self.rotated_rect(center, size, Vec2::X, color);
    }

    /// Rectangle whose local x axis points along `forward` (a unit vector).
    pub fn rotated_rect(&mut self, center: Vec2, size: Vec2, forward: Vec2, color: Color) {
        let h = size / 2.0;
        let side = forward.perp();
        let outline = [
            center - forward * h.x - side * h.y,
            center + forward * h.x - side * h.y,
            center + forward * h.x + side * h.y,
            center - forward * h.x + side * h.y,
        ];
        self.convex(&outline, color);
    }

    pub fn rounded_rect(&mut self, center: Vec2, size: Vec2, radius: f32, color: Color) {
        self.convex(&rounded_rect_outline(center, size, radius), color);
    }

    pub fn circle(&mut self, center: Vec2, radius: f32, color: Color) {
        let n = ((radius * 6.0).ceil() as usize).clamp(10, 40);
        let outline: Vec<Vec2> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                center + radius * Vec2::new(a.cos(), a.sin())
            })
            .collect();
        self.convex(&outline, color);
    }

    /// A thick polyline drawn as one quad per segment.
    pub fn stroke(&mut self, points: &[Vec2], width: f32, color: Color) {
        for w in points.windows(2) {
            let d = w[1] - w[0];
            let len = d.length();
            if len > f32::EPSILON {
                self.rotated_rect((w[0] + w[1]) / 2.0, Vec2::new(len, width), d / len, color);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

pub fn rounded_rect_outline(center: Vec2, size: Vec2, radius: f32) -> Vec<Vec2> {
    let h = size / 2.0;
    let r = radius.min(h.x).min(h.y).max(0.0);
    let corners = [
        (Vec2::new(h.x - r, h.y - r), 0.0),
        (Vec2::new(-h.x + r, h.y - r), 0.25),
        (Vec2::new(-h.x + r, -h.y + r), 0.5),
        (Vec2::new(h.x - r, -h.y + r), 0.75),
    ];
    let steps = 5;
    let mut out = Vec::with_capacity(4 * (steps + 1));
    for (c, start) in corners {
        for i in 0..=steps {
            let a = (start + 0.25 * i as f32 / steps as f32) * std::f32::consts::TAU;
            out.push(center + c + r * Vec2::new(a.cos(), a.sin()));
        }
    }
    out
}
