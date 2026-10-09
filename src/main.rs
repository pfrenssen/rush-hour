mod cli;
mod input;
mod levels;
mod lights;
mod map;
mod render;
mod scoring;
mod screenshot;
mod sim;
mod states;
mod ui;
mod vehicles;

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PresentMode, WindowResolution};

use cli::{Mode, ParseOutcome};
use states::AppState;

fn main() {
    let args = match cli::parse(std::env::args().skip(1)) {
        Ok(ParseOutcome::Run(args)) => args,
        Ok(ParseOutcome::Help) => {
            print!("{}", cli::USAGE);
            return;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };

    let mut resolution = WindowResolution::new(1280, 720);
    let screenshot = matches!(args.mode, Mode::Screenshot { .. });
    if screenshot {
        // Screenshots come out at exactly 1280x720 regardless of the display's DPI; a fixed-size
        // window also keeps tiling window managers from resizing it.
        resolution = resolution.with_scale_factor_override(1.0);
    }

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Rush Hour".into(),
            resolution,
            resizable: !screenshot,
            // Screenshot runs render as fast as possible instead of waiting for vsync.
            present_mode: if screenshot {
                PresentMode::AutoNoVsync
            } else {
                PresentMode::AutoVsync
            },
            ..default()
        }),
        ..default()
    }))
    .add_plugins((
        states::GameStatesPlugin,
        sim::SimPlugin { seed: 1 },
        input::InputPlugin,
        render::GameRenderPlugin,
        ui::GameUiPlugin,
    ));

    if let Some(level) = args.level {
        app.insert_resource(map::CurrentLevel(level))
            .insert_state(AppState::InGame);
    }
    if let Mode::Screenshot { path, frames } = args.mode {
        // Advance time by exactly one 60 Hz frame per update so a run is reproducible.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_plugins(screenshot::ScreenshotPlugin { path, frames });
    }
    app.run();
}
