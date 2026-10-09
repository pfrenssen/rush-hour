//! Main menu with level select and a short how-to-play.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::{ACCENT, PANEL, TEXT, TEXT_DIM, button, small_button};
use crate::levels::LEVELS;
use crate::map::graph::LaneGraph;
use crate::map::grid::Grid;
use crate::scoring::Game;
use crate::states::{Action, AppState};

fn send(action: Action) -> impl FnMut(On<Activate>, MessageWriter<Action>) {
    move |_, mut actions| {
        actions.write(action);
    }
}

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// One lamp of the animated traffic light next to the title.
#[derive(Component)]
pub struct EmblemLamp(usize);

const LAMP_COLORS: [Color; 3] = [
    Color::srgb(0.98, 0.30, 0.26),
    Color::srgb(1.0, 0.74, 0.20),
    Color::srgb(0.30, 0.90, 0.48),
];

fn emblem() -> impl Bundle {
    (
        Node {
            column_gap: px(12),
            padding: UiRect::axes(px(16), px(11)),
            border_radius: BorderRadius::all(px(26)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.09, 0.11)),
        BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.6),
            px(0),
            px(6),
            px(0),
            px(14),
        ),
        Children::spawn(SpawnIter((0..3).map(|i| {
            (
                EmblemLamp(i),
                Node {
                    width: px(30),
                    height: px(30),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(dim(LAMP_COLORS[i])),
                BoxShadow::default(),
            )
        }))),
    )
}

fn dim(color: Color) -> Color {
    color.mix(&Color::srgb(0.10, 0.10, 0.12), 0.72)
}

/// Cycle the emblem like a real light: green, amber, red.
pub fn animate_emblem(
    time: Res<Time<Real>>,
    mut lamps: Query<(&EmblemLamp, &mut BackgroundColor, &mut BoxShadow)>,
) {
    let t = time.elapsed_secs() % 6.0;
    let lit = if t < 2.6 {
        2
    } else if t < 3.6 {
        1
    } else {
        0
    };
    for (lamp, mut bg, mut shadow) in &mut lamps {
        let on = lamp.0 == lit;
        let color = if on {
            LAMP_COLORS[lamp.0]
        } else {
            dim(LAMP_COLORS[lamp.0])
        };
        if bg.0 != color {
            bg.0 = color;
            *shadow = if on {
                BoxShadow::new(
                    LAMP_COLORS[lamp.0].with_alpha(0.8),
                    px(0),
                    px(0),
                    px(2),
                    px(16),
                )
            } else {
                BoxShadow::default()
            };
        }
    }
}

pub fn spawn_menu(mut commands: Commands, game: Res<Game>) {
    commands
        .spawn((
            DespawnOnExit(AppState::Menu),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(emblem());
            root.spawn(text("RUSH HOUR", 76.0, ACCENT));
            root.spawn(text(
                "Keep the city moving. You only control the traffic lights.",
                20.0,
                TEXT_DIM,
            ));
            if game.score > 0 {
                root.spawn(text(
                    format!("Last score: {}", super::hud::format_score(game.score)),
                    18.0,
                    TEXT,
                ));
            }
            root.spawn(Node {
                height: px(8),
                ..default()
            });
            root.spawn(button("Play", 240.0)).observe(send(Action::Play(0)));
            root.spawn(text("or pick a level", 14.0, TEXT_DIM));
            root.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(10),
                row_gap: px(10),
                max_width: px(760),
                ..default()
            })
            .with_children(|grid| {
                for (i, level) in LEVELS.iter().enumerate() {
                    let signals = Grid::parse(level.rows)
                        .map(|g| {
                            LaneGraph::build(&g)
                                .nodes
                                .iter()
                                .filter(|n| n.is_signal())
                                .count()
                        })
                        .unwrap_or(0);
                    grid.spawn(small_button(240.0))
                        .insert(Node {
                            width: px(240),
                            height: px(58),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(px(10)),
                            ..default()
                        })
                        .with_children(|b| {
                            b.spawn(text(format!("{}  {}", i + 1, level.name), 18.0, TEXT));
                            b.spawn(text(
                                format!("{signals} signals - deliver {}", level.target),
                                12.0,
                                TEXT_DIM,
                            ));
                        })
                        .observe(send(Action::Play(i)));
                }
            });
            root.spawn((
                Node {
                    margin: UiRect::top(px(16)),
                    padding: UiRect::axes(px(22), px(14)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    max_width: px(760),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|help| {
                for line in [
                    "Click a junction to switch its lights: amber, a moment of all-red, then the other direction goes green.",
                    "Drivers lose patience while they wait. If one gives up, or an entry stays blocked for 8 s, you lose a life.",
                    "Fast trips score more, steady flow builds a combo. Ambulances are impatient but pay a bonus when quick.",
                    "Keys: P / Esc pause   F speed 1x/2x   R restart   M menu   1-6 pick a level",
                ] {
                    help.spawn(text(line, 14.0, TEXT_DIM));
                }
            });
        });
}
