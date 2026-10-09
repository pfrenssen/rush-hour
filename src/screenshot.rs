//! `--screenshot` mode: run a level (or the main menu, without `--level`) for a fixed number of
//! frames, save a PNG and exit.
//!
//! Shaders compile in the background, so the run starts with a short warm-up in which time is
//! held still; only then are the requested frames simulated and counted.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

/// Wall-clock time (and at least this many frames) to let render pipelines compile.
const WARM_UP: Duration = Duration::from_millis(1500);
const WARM_UP_FRAMES: u32 = 30;

#[derive(Resource)]
struct ScreenshotPlan {
    path: PathBuf,
    frames: u32,
    elapsed: u32,
    requested: bool,
    started: Option<Instant>,
    warm_frames: u32,
}

pub struct ScreenshotPlugin {
    pub path: PathBuf,
    pub frames: u32,
}

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScreenshotPlan {
            path: self.path.clone(),
            frames: self.frames,
            elapsed: 0,
            requested: false,
            started: None,
            warm_frames: 0,
        })
        .add_systems(Update, take_screenshot);
    }
}

fn take_screenshot(
    mut commands: Commands,
    mut plan: ResMut<ScreenshotPlan>,
    mut time: ResMut<Time<Virtual>>,
) {
    if plan.requested {
        return;
    }
    let started = *plan.started.get_or_insert_with(Instant::now);
    if plan.warm_frames < WARM_UP_FRAMES || started.elapsed() < WARM_UP {
        plan.warm_frames += 1;
        time.pause();
        return;
    }
    time.unpause();
    plan.elapsed += 1;
    if plan.elapsed < plan.frames {
        return;
    }
    plan.requested = true;
    if let Some(dir) = plan.path.parent()
        && !dir.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        error!("cannot create {}: {e}", dir.display());
    }
    let path = plan.path.clone();
    info!("saving screenshot to {}", path.display());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}
