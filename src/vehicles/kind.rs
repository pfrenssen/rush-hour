//! Vehicle types and their driving characteristics.

use crate::levels::Mix;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VehicleKind {
    Car,
    Bus,
    Truck,
    Ambulance,
}

#[derive(Clone, Copy, Debug)]
pub struct KindParams {
    pub length: f32,
    pub width: f32,
    /// Desired cruising speed (m/s).
    pub speed: f32,
    pub accel: f32,
    /// Comfortable braking deceleration (m/s², positive).
    pub decel: f32,
    /// Seconds of standing still before patience runs out.
    pub patience: f32,
    /// Points for a delivery at full speed, before the combo multiplier.
    #[allow(dead_code)]
    pub points: u32,
}

impl VehicleKind {
    pub const ALL: [VehicleKind; 4] = [
        VehicleKind::Car,
        VehicleKind::Bus,
        VehicleKind::Truck,
        VehicleKind::Ambulance,
    ];

    pub fn params(self) -> KindParams {
        match self {
            VehicleKind::Car => KindParams {
                length: 4.4,
                width: 1.9,
                speed: 12.5,
                accel: 2.6,
                decel: 3.0,
                patience: 30.0,
                points: 100,
            },
            VehicleKind::Bus => KindParams {
                length: 9.5,
                width: 2.45,
                speed: 9.5,
                accel: 1.4,
                decel: 2.5,
                patience: 34.0,
                points: 200,
            },
            VehicleKind::Truck => KindParams {
                length: 7.6,
                width: 2.35,
                speed: 10.0,
                accel: 1.5,
                decel: 2.5,
                patience: 34.0,
                points: 150,
            },
            VehicleKind::Ambulance => KindParams {
                length: 5.4,
                width: 2.1,
                speed: 14.0,
                accel: 3.0,
                decel: 3.5,
                patience: 12.0,
                points: 150,
            },
        }
    }

    /// Pick a type according to a level's mix, from a uniform sample `u` in `0..1`.
    pub fn pick(mix: &Mix, u: f32) -> VehicleKind {
        let weights = [mix.car, mix.bus, mix.truck, mix.ambulance];
        let total: f32 = weights.iter().sum();
        let mut x = u * total;
        for (kind, w) in Self::ALL.into_iter().zip(weights) {
            if x < w {
                return kind;
            }
            x -= w;
        }
        VehicleKind::Car
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn larger_vehicles_are_slower() {
        let car = VehicleKind::Car.params();
        for kind in [VehicleKind::Bus, VehicleKind::Truck] {
            let p = kind.params();
            assert!(p.length > car.length);
            assert!(p.speed < car.speed);
            assert!(p.accel < car.accel);
        }
    }

    #[test]
    fn pick_follows_the_mix() {
        let mix = Mix {
            car: 0.5,
            bus: 0.25,
            truck: 0.25,
            ambulance: 0.0,
        };
        assert_eq!(VehicleKind::pick(&mix, 0.1), VehicleKind::Car);
        assert_eq!(VehicleKind::pick(&mix, 0.6), VehicleKind::Bus);
        assert_eq!(VehicleKind::pick(&mix, 0.9), VehicleKind::Truck);
    }
}
