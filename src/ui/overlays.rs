//! Overlays for pause, level complete and game over.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::hud::format_score;
use super::{ACCENT, DANGER, PANEL, TEXT, TEXT_DIM, button};
use crate::levels::LEVELS;
use crate::map::CityMap;
use crate::scoring::{Game, LossReason};
use crate::states::{Action, GamePhase};

fn send(action: Action) -> impl FnMut(On<Activate>, MessageWriter<Action>) {
    move |_, mut actions| {
        actions.write(action);
    }
}

fn line(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// A dimmed full-screen layer with a centred card, removed when `phase` ends.
fn overlay(
    commands: &mut Commands,
    phase: GamePhase,
    title: &str,
    title_color: Color,
    lines: Vec<(String, Color)>,
    buttons: &[(&str, Action)],
    hint: &str,
) {
    commands
        .spawn((
            DespawnOnExit(phase),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.55)),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    padding: UiRect::axes(px(44), px(30)),
                    border_radius: BorderRadius::all(px(16)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|card| {
                card.spawn(line(title, 44.0, title_color));
                for (text, color) in lines {
                    card.spawn(line(text, 18.0, color));
                }
                card.spawn(Node {
                    height: px(8),
                    ..default()
                });
                for &(label, action) in buttons {
                    card.spawn(button(label, 260.0)).observe(send(action));
                }
                card.spawn(line(hint, 13.0, TEXT_DIM));
            });
        });
}

pub fn spawn_pause(mut commands: Commands, map: Res<CityMap>) {
    overlay(
        &mut commands,
        GamePhase::Paused,
        "PAUSED",
        TEXT,
        vec![(
            format!("Level {}  {}", map.level + 1, map.def().name),
            TEXT_DIM,
        )],
        &[
            ("Resume", Action::Resume),
            ("Restart level", Action::Restart),
            ("Main menu", Action::Menu),
        ],
        "P / Esc resume   R restart   M menu",
    );
}

fn summary(game: &Game) -> Vec<(String, Color)> {
    vec![
        (format!("Score  {}", format_score(game.score)), TEXT),
        (
            format!(
                "Delivered {}   Best combo {}   Lives lost {}",
                game.delivered, game.combo.best, game.lives.lost
            ),
            TEXT_DIM,
        ),
    ]
}

pub fn spawn_level_complete(mut commands: Commands, map: Res<CityMap>, game: Res<Game>) {
    let last = map.level + 1 >= LEVELS.len();
    let mut lines = vec![(
        format!("{} cleared in {:.0} s", map.def().name, game.elapsed),
        TEXT_DIM,
    )];
    lines.extend(summary(&game));
    if last {
        lines.push(("You beat the whole city!".to_string(), ACCENT));
        overlay(
            &mut commands,
            GamePhase::LevelComplete,
            "CITY CLEARED",
            ACCENT,
            lines,
            &[
                ("Main menu", Action::Menu),
                ("Replay level", Action::Restart),
            ],
            "Enter menu   R replay",
        );
    } else {
        overlay(
            &mut commands,
            GamePhase::LevelComplete,
            "LEVEL COMPLETE",
            ACCENT,
            lines,
            &[
                ("Next level", Action::Next),
                ("Replay level", Action::Restart),
                ("Main menu", Action::Menu),
            ],
            "Enter next   R replay   M menu",
        );
    }
}

pub fn spawn_game_over(mut commands: Commands, map: Res<CityMap>, game: Res<Game>) {
    let reason = match game.last_loss {
        Some(LossReason::Blocked) => "An entry road stayed blocked for too long.",
        _ => "The drivers ran out of patience.",
    };
    let mut lines = vec![(reason.to_string(), DANGER)];
    lines.extend(summary(&game));
    lines.push((
        format!("Level {}  {}", map.level + 1, map.def().name),
        TEXT_DIM,
    ));
    overlay(
        &mut commands,
        GamePhase::GameOver,
        "GAME OVER",
        DANGER,
        lines,
        &[("Try again", Action::Restart), ("Main menu", Action::Menu)],
        "Enter / R try again   M menu",
    );
}
