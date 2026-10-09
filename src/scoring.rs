//! Score, combo, lives and level progress.
//!
//! Each delivered vehicle earns its type's points scaled by how fast the trip was compared with
//! empty roads. Deliveries in quick succession build a combo chain that multiplies the points.
//! A life is lost when a vehicle runs out of patience or a spawn point stays blocked too long.

use bevy::prelude::*;

use crate::map::CityMap;
use crate::map::graph::LANE_OFFSET;
use crate::map::grid::TILE;
use crate::sim::SimSet;
use crate::states::{AppState, GamePhase, LevelLoad};
use crate::vehicles::kind::VehicleKind;
use crate::vehicles::{Delivered, FedUp, SpawnBlocked};

pub const LIVES: u32 = 3;
/// A combo chain breaks when no vehicle is delivered for this long.
pub const COMBO_WINDOW: f32 = 6.0;
/// After losing a life, further losses are forgiven for this long so one jam does not end the
/// game in a single second.
pub const LIFE_GRACE: f32 = 4.0;
/// Ambulances that keep up at least this speed factor earn a bonus.
pub const AMBULANCE_FAST: f32 = 0.7;
pub const AMBULANCE_BONUS: u32 = 250;

/// 0..1: how close a trip came to the time it would take on empty roads.
pub fn speed_factor(ideal_time: f32, trip_time: f32) -> f32 {
    let ratio = ideal_time / trip_time.max(0.001);
    ((ratio - 0.25) / 0.55).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Award {
    pub points: u32,
    pub bonus: u32,
    pub multiplier: u32,
}

impl Award {
    pub fn total(&self) -> u32 {
        (self.points + self.bonus) * self.multiplier
    }
}

/// Points for one delivery.
pub fn award(kind: VehicleKind, ideal_time: f32, trip_time: f32, multiplier: u32) -> Award {
    let factor = speed_factor(ideal_time, trip_time);
    let raw = kind.params().points as f32 * (0.25 + 0.75 * factor);
    let points = (raw / 5.0).round() as u32 * 5;
    let bonus = if kind == VehicleKind::Ambulance && factor >= AMBULANCE_FAST {
        AMBULANCE_BONUS
    } else {
        0
    };
    Award {
        points,
        bonus,
        multiplier,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Combo {
    pub chain: u32,
    pub timer: f32,
    pub best: u32,
}

impl Combo {
    pub fn multiplier(&self) -> u32 {
        match self.chain {
            0..=4 => 1,
            5..=11 => 2,
            12..=19 => 3,
            _ => 4,
        }
    }

    pub fn deliver(&mut self) {
        self.chain += 1;
        self.timer = COMBO_WINDOW;
        self.best = self.best.max(self.chain);
    }

    pub fn tick(&mut self, dt: f32) {
        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.chain = 0;
            }
        }
    }

    pub fn reset(&mut self) {
        self.chain = 0;
        self.timer = 0.0;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lives {
    pub left: u32,
    pub lost: u32,
    pub grace: f32,
}

impl Default for Lives {
    fn default() -> Self {
        Lives {
            left: LIVES,
            lost: 0,
            grace: 0.0,
        }
    }
}

impl Lives {
    /// Lose a life unless still in the grace period. Returns whether a life was lost.
    pub fn hit(&mut self) -> bool {
        if self.grace > 0.0 || self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.lost += 1;
        self.grace = LIFE_GRACE;
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.grace = (self.grace - dt).max(0.0);
    }
}

/// Score and state of the level being played.
#[derive(Resource, Clone, Debug, Default)]
pub struct Game {
    pub score: u64,
    /// Score when the level started, restored on restart.
    pub level_start_score: u64,
    pub combo: Combo,
    pub lives: Lives,
    pub delivered: u32,
    pub target: u32,
    pub elapsed: f32,
    /// Simulated seconds at which the target was reached.
    pub completed_at: Option<f32>,
    /// Simulated seconds at which the last life was lost.
    pub game_over_at: Option<f32>,
    pub last_loss: Option<LossReason>,
}

/// Whether reaching the target or losing all lives ends the level (off for headless runs).
#[derive(Resource, Clone, Copy, Debug)]
pub struct EndConditions(pub bool);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LossReason {
    Impatient,
    Blocked,
}

/// Points scored at a world position (for pop-ups).
#[derive(Message, Clone, Copy, Debug)]
pub struct Scored {
    pub pos: Vec2,
    pub award: Award,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct LifeLost {
    pub pos: Vec2,
    pub reason: LossReason,
}

pub struct ScoringPlugin;

impl Plugin for ScoringPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Game>()
            .insert_resource(EndConditions(true))
            .add_message::<Scored>()
            .add_message::<LifeLost>()
            .add_systems(
                OnEnter(AppState::InGame),
                start_level.in_set(LevelLoad::Sim),
            )
            .add_systems(
                FixedUpdate,
                (score_deliveries, lose_lives, tick_game, check_end)
                    .chain()
                    .in_set(SimSet::Score),
            );
    }
}

fn start_level(mut game: ResMut<Game>, map: Res<CityMap>) {
    *game = Game {
        score: game.level_start_score,
        level_start_score: game.level_start_score,
        target: map.def().target,
        ..default()
    };
}

fn score_deliveries(
    mut delivered: MessageReader<Delivered>,
    mut game: ResMut<Game>,
    map: Res<CityMap>,
    mut scored: MessageWriter<Scored>,
) {
    for d in delivered.read() {
        game.combo.deliver();
        game.delivered += 1;
        let a = award(d.kind, d.ideal_time, d.trip_time, game.combo.multiplier());
        game.score += a.total() as u64;
        // Pop up just inside the map edge where the vehicle left.
        let t = &map.graph.terminals[d.exit];
        let pos = t.pos - t.side.vec() * TILE + t.side.right().vec() * LANE_OFFSET;
        scored.write(Scored { pos, award: a });
    }
}

fn lose_lives(
    mut fed_up: MessageReader<FedUp>,
    mut blocked: MessageReader<SpawnBlocked>,
    mut game: ResMut<Game>,
    map: Res<CityMap>,
    mut lost: MessageWriter<LifeLost>,
) {
    let hits = fed_up
        .read()
        .map(|f| (f.pos, LossReason::Impatient))
        .chain(blocked.read().map(|b| {
            let t = &map.graph.terminals[b.terminal];
            (t.pos - t.side.vec() * TILE, LossReason::Blocked)
        }));
    for (pos, reason) in hits {
        if game.lives.hit() {
            game.combo.reset();
            game.last_loss = Some(reason);
            lost.write(LifeLost { pos, reason });
        }
    }
}

fn tick_game(time: Res<Time>, mut game: ResMut<Game>) {
    let dt = time.delta_secs();
    game.elapsed += dt;
    game.combo.tick(dt);
    game.lives.tick(dt);
}

fn check_end(
    mut game: ResMut<Game>,
    end: Res<EndConditions>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    if game.lives.left == 0 && game.game_over_at.is_none() {
        game.game_over_at = Some(game.elapsed);
        if end.0 {
            next.set(GamePhase::GameOver);
        }
    } else if game.delivered >= game.target && game.completed_at.is_none() {
        game.completed_at = Some(game.elapsed);
        if end.0 {
            next.set(GamePhase::LevelComplete);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_trips_score_more() {
        let fast = award(VehicleKind::Car, 20.0, 21.0, 1);
        let slow = award(VehicleKind::Car, 20.0, 60.0, 1);
        let crawl = award(VehicleKind::Car, 20.0, 400.0, 1);
        assert_eq!(fast.points, 100);
        assert!(slow.points < fast.points);
        assert!(crawl.points < slow.points);
        assert_eq!(crawl.points, 25);
    }

    #[test]
    fn bigger_vehicles_are_worth_more() {
        let car = award(VehicleKind::Car, 20.0, 20.0, 1).total();
        let bus = award(VehicleKind::Bus, 20.0, 20.0, 1).total();
        assert!(bus > car);
    }

    #[test]
    fn fast_ambulances_earn_a_bonus() {
        assert_eq!(
            award(VehicleKind::Ambulance, 20.0, 21.0, 1).bonus,
            AMBULANCE_BONUS
        );
        assert_eq!(award(VehicleKind::Ambulance, 20.0, 80.0, 1).bonus, 0);
    }

    #[test]
    fn combo_builds_with_steady_flow_and_breaks_when_it_stops() {
        let mut c = Combo::default();
        for _ in 0..5 {
            c.deliver();
            c.tick(2.0);
        }
        assert_eq!(c.multiplier(), 2);
        assert_eq!(
            award(VehicleKind::Car, 1.0, 1.0, c.multiplier()).total(),
            200
        );
        c.tick(COMBO_WINDOW);
        assert_eq!(c.chain, 0);
        assert_eq!(c.multiplier(), 1);
        assert_eq!(c.best, 5);
    }

    #[test]
    fn lives_have_a_grace_period() {
        let mut l = Lives::default();
        assert!(l.hit());
        assert!(!l.hit(), "a second hit right away is forgiven");
        l.tick(LIFE_GRACE);
        assert!(l.hit());
        l.tick(LIFE_GRACE);
        assert!(l.hit());
        assert_eq!(l.left, 0);
        l.tick(LIFE_GRACE);
        assert!(!l.hit());
        assert_eq!(l.lost, LIVES);
    }
}
