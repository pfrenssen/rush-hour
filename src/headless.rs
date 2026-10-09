//! `--headless` mode: run the simulation without a window and print a report.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use crate::autopilot::Autopilot;
use crate::map::CurrentLevel;
use crate::scoring::{EndConditions, Game};
use crate::sim::TICK_HZ;
use crate::states::AppState;
use crate::vehicles::TrafficStats;

pub struct Report {
    pub delivered: u32,
    pub spawned: u32,
    pub lives_lost: u32,
    pub max_wait: f32,
    pub overlaps: u32,
    pub score: u64,
    pub completed_at: Option<f32>,
    pub game_over_at: Option<f32>,
}

/// Simulate `seconds` of `level` (0-based) and report what happened. Losing every life or
/// reaching the target does not stop the run, so stress tests can go on as long as asked.
pub fn simulate(level: usize, seconds: f32, seed: u64, autopilot: bool) -> Report {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        crate::states::GameStatesPlugin,
        crate::sim::SimPlugin { seed },
        crate::autopilot::AutopilotPlugin,
    ))
    // Every update runs exactly one fixed tick, as fast as the CPU allows.
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .insert_resource(CurrentLevel(level))
    .insert_resource(EndConditions(false))
    .insert_resource(Autopilot(autopilot))
    .insert_state(AppState::InGame);
    app.finish();
    app.cleanup();

    let ticks = (seconds as f64 * TICK_HZ).round() as u64;
    // The first update performs the state transition into the level.
    for _ in 0..=ticks {
        app.update();
    }

    let world = app.world();
    let stats = world.resource::<TrafficStats>();
    let game = world.resource::<Game>();
    Report {
        delivered: stats.delivered,
        spawned: stats.spawned,
        lives_lost: game.lives.lost,
        max_wait: stats.max_wait,
        overlaps: stats.overlaps,
        score: game.score,
        completed_at: game.completed_at,
        game_over_at: game.game_over_at,
    }
}

pub fn run(level: usize, seconds: f32, seed: u64, autopilot: bool) -> std::process::ExitCode {
    let def = &crate::levels::LEVELS[level];
    let r = simulate(level, seconds, seed, autopilot);
    let fmt_time = |t: Option<f32>| t.map_or("never".to_string(), |t| format!("{t:.1} s"));
    println!("level        {} ({})", level + 1, def.name);
    println!("simulated    {seconds:.0} s, seed {seed}, autopilot {autopilot}");
    println!("spawned      {}", r.spawned);
    println!("delivered    {} (target {})", r.delivered, def.target);
    println!("lives lost   {}", r.lives_lost);
    println!("max wait     {:.1} s", r.max_wait);
    println!("overlaps     {}", r.overlaps);
    println!("score        {}", r.score);
    println!("target met   {}", fmt_time(r.completed_at));
    println!("game over    {}", fmt_time(r.game_over_at));
    if r.overlaps > 0 {
        eprintln!("error: vehicles overlapped");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_run_delivers_vehicles_without_overlaps() {
        let r = simulate(0, 60.0, 1, true);
        assert!(r.delivered > 5, "only {} delivered", r.delivered);
        assert_eq!(r.overlaps, 0);
    }

    #[test]
    fn runs_are_reproducible() {
        let a = simulate(1, 30.0, 9, true);
        let b = simulate(1, 30.0, 9, true);
        assert_eq!(
            (a.delivered, a.spawned, a.score),
            (b.delivered, b.spawned, b.score)
        );
    }
}
