//! Lane centre-line geometry: straight segments and circular arcs.

use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geom {
    Line {
        a: Vec2,
        b: Vec2,
    },
    /// Circular arc around `center`, starting at angle `start` (radians) and sweeping `sweep`
    /// radians; positive sweeps run counter-clockwise.
    Arc {
        center: Vec2,
        radius: f32,
        start: f32,
        sweep: f32,
    },
}

impl Geom {
    pub fn length(&self) -> f32 {
        match *self {
            Geom::Line { a, b } => a.distance(b),
            Geom::Arc { radius, sweep, .. } => radius * sweep.abs(),
        }
    }

    /// Point at distance `s` from the start. Lines extrapolate beyond their ends, arcs clamp.
    pub fn point_at(&self, s: f32) -> Vec2 {
        match *self {
            Geom::Line { a, b } => {
                let len = a.distance(b);
                if len <= f32::EPSILON {
                    a
                } else {
                    a + (b - a) * (s / len)
                }
            }
            Geom::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let t = (s / self.length()).clamp(0.0, 1.0);
                let angle = start + sweep * t;
                center + radius * Vec2::new(angle.cos(), angle.sin())
            }
        }
    }

    /// Unit direction of travel at distance `s`.
    pub fn tangent_at(&self, s: f32) -> Vec2 {
        match *self {
            Geom::Line { a, b } => (b - a).normalize_or_zero(),
            Geom::Arc { start, sweep, .. } => {
                let t = (s / self.length()).clamp(0.0, 1.0);
                let angle = start + sweep * t;
                let radial = Vec2::new(angle.cos(), angle.sin());
                if sweep >= 0.0 {
                    radial.perp()
                } else {
                    -radial.perp()
                }
            }
        }
    }

    pub fn start(&self) -> Vec2 {
        self.point_at(0.0)
    }

    pub fn end(&self) -> Vec2 {
        self.point_at(self.length())
    }

    /// Points spaced at most `step` apart, including both ends.
    pub fn sample(&self, step: f32) -> Vec<Vec2> {
        let len = self.length();
        let n = (len / step).ceil().max(1.0) as usize;
        (0..=n)
            .map(|i| self.point_at(len * i as f32 / n as f32))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn line_points_and_tangent() {
        let g = Geom::Line {
            a: Vec2::ZERO,
            b: Vec2::new(10.0, 0.0),
        };
        assert_eq!(g.length(), 10.0);
        assert!(g.point_at(2.5).abs_diff_eq(Vec2::new(2.5, 0.0), 1e-5));
        assert!(g.tangent_at(1.0).abs_diff_eq(Vec2::X, 1e-5));
    }

    #[test]
    fn arc_counter_clockwise_quarter() {
        let g = Geom::Arc {
            center: Vec2::ZERO,
            radius: 2.0,
            start: 0.0,
            sweep: FRAC_PI_2,
        };
        assert!((g.length() - std::f32::consts::PI).abs() < 1e-5);
        assert!(g.start().abs_diff_eq(Vec2::new(2.0, 0.0), 1e-5));
        assert!(g.end().abs_diff_eq(Vec2::new(0.0, 2.0), 1e-5));
        assert!(g.tangent_at(0.0).abs_diff_eq(Vec2::Y, 1e-5));
    }

    #[test]
    fn arc_clockwise_tangent_points_along_travel() {
        let g = Geom::Arc {
            center: Vec2::ZERO,
            radius: 2.0,
            start: std::f32::consts::PI,
            sweep: -FRAC_PI_2,
        };
        assert!(g.start().abs_diff_eq(Vec2::new(-2.0, 0.0), 1e-5));
        assert!(g.end().abs_diff_eq(Vec2::new(0.0, 2.0), 1e-5));
        assert!(g.tangent_at(0.0).abs_diff_eq(Vec2::Y, 1e-5));
        assert!(g.tangent_at(g.length()).abs_diff_eq(Vec2::X, 1e-5));
    }
}
