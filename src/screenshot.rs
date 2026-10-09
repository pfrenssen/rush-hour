//! `--screenshot` mode: run a level (or the main menu, without `--level`) for a fixed number of
//! frames, save a PNG and exit.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

#[derive(Resource)]
struct ScreenshotPlan {
    path: PathBuf,
    frames: u32,
    elapsed: u32,
    requested: bool,
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
        })
        .add_systems(Update, take_screenshot);
    }
}

fn take_screenshot(mut commands: Commands, mut plan: ResMut<ScreenshotPlan>) {
    if plan.requested {
        return;
    }
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
