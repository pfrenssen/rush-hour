//! In-game HUD: level, score, combo, delivered/target, lives, speed and pause.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::{ACCENT, DANGER, TEXT, TEXT_DIM, small_button};
use crate::map::CityMap;
use crate::render::camera::HUD_HEIGHT;
use crate::scoring::{Game, LIVES};
use crate::sim::SimSpeed;
use crate::states::{Action, AppState};

const HUD_BG: Color = Color::srgba(0.06, 0.07, 0.09, 0.94);
const BAR_BG: Color = Color::srgb(0.18, 0.20, 0.24);
const BAR_FILL: Color = Color::srgb(0.36, 0.80, 0.52);
const LIFE_ON: Color = Color::srgb(0.95, 0.30, 0.32);
const LIFE_OFF: Color = Color::srgb(0.22, 0.24, 0.28);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudText {
    Level,
    Score,
    Combo,
    Delivered,
    Speed,
}

#[derive(Component)]
pub struct LifeDot(u32);

#[derive(Component)]
pub struct ProgressFill;

/// Level title card shown for a few seconds when a level starts.
#[derive(Component)]
pub struct Intro {
    age: f32,
}

const INTRO_TIME: f32 = 4.0;

fn label(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(TEXT_DIM),
    )
}

fn value(kind: HudText, size: f32, color: Color) -> impl Bundle {
    (
        kind,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn column() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(2),
        ..default()
    }
}

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                left: px(0),
                width: percent(100),
                height: px(HUD_HEIGHT),
                padding: UiRect::horizontal(px(20)),
                align_items: AlignItems::Center,
                column_gap: px(32),
                ..default()
            },
            BackgroundColor(HUD_BG),
        ))
        .with_children(|bar| {
            bar.spawn(column()).with_children(|c| {
                c.spawn(label("LEVEL"));
                c.spawn(value(HudText::Level, 20.0, TEXT));
            });
            bar.spawn(column()).with_children(|c| {
                c.spawn(label("SCORE"));
                c.spawn(value(HudText::Score, 24.0, TEXT));
            });
            bar.spawn(value(HudText::Combo, 18.0, ACCENT));
            bar.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            bar.spawn(column()).with_children(|c| {
                c.spawn(label("DELIVERED"));
                c.spawn(value(HudText::Delivered, 18.0, TEXT));
                c.spawn((
                    Node {
                        width: px(130),
                        height: px(6),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(BAR_BG),
                ))
                .with_child((
                    ProgressFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(BAR_FILL),
                ));
            });
            bar.spawn(column()).with_children(|c| {
                c.spawn(label("LIVES"));
                c.spawn(Node {
                    column_gap: px(6),
                    padding: UiRect::top(px(4)),
                    ..default()
                })
                .with_children(|dots| {
                    for i in 0..LIVES {
                        dots.spawn((
                            LifeDot(i),
                            Node {
                                width: px(16),
                                height: px(16),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(LIFE_ON),
                        ));
                    }
                });
            });
            bar.spawn(small_button(56.0))
                .with_child(value(HudText::Speed, 18.0, TEXT))
                .observe(|_: On<Activate>, mut speed: ResMut<SimSpeed>| speed.toggle());
            bar.spawn(small_button(56.0))
                .with_child((
                    Text::new("II"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(TEXT),
                ))
                .observe(|_: On<Activate>, mut actions: MessageWriter<Action>| {
                    actions.write(Action::Pause);
                });
        });
}

pub fn spawn_intro(mut commands: Commands, map: Res<CityMap>) {
    let def = map.def();
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Intro { age: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                top: percent(36),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    padding: UiRect::axes(px(36), px(18)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.85)),
            ))
            .with_children(|card| {
                for (text, size, color) in [
                    (format!("LEVEL {}", map.level + 1), 16.0, TEXT_DIM),
                    (def.name.to_string(), 40.0, ACCENT),
                    (def.tagline.to_string(), 18.0, TEXT),
                    (format!("Deliver {} vehicles", def.target), 16.0, TEXT_DIM),
                ] {
                    card.spawn((
                        Text::new(text),
                        TextFont {
                            font_size: FontSize::Px(size),
                            ..default()
                        },
                        TextColor(color),
                    ));
                }
            });
        });
}

/// Fade the title card out, then remove it.
pub fn fade_intro(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut intros: Query<(Entity, &mut Intro, &Children)>,
    mut cards: Query<(&mut BackgroundColor, &Children), Without<Intro>>,
    mut texts: Query<&mut TextColor>,
) {
    for (entity, mut intro, children) in &mut intros {
        intro.age += time.delta_secs();
        if intro.age >= INTRO_TIME {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = ((INTRO_TIME - intro.age) / 0.6).min(1.0);
        for card in children.iter() {
            let Ok((mut bg, lines)) = cards.get_mut(card) else {
                continue;
            };
            bg.0 = bg.0.with_alpha(0.85 * alpha);
            for line in lines.iter() {
                if let Ok(mut color) = texts.get_mut(line) {
                    color.0 = color.0.with_alpha(alpha);
                }
            }
        }
    }
}

pub fn update_hud(
    game: Res<Game>,
    map: Res<CityMap>,
    speed: Res<SimSpeed>,
    mut texts: Query<(&HudText, &mut Text)>,
    mut dots: Query<(&LifeDot, &mut BackgroundColor)>,
    mut fill: Query<&mut Node, With<ProgressFill>>,
) {
    for (kind, mut text) in &mut texts {
        let s = match kind {
            HudText::Level => format!("{}  {}", map.level + 1, map.def().name),
            HudText::Score => format_score(game.score),
            HudText::Combo => match game.combo.multiplier() {
                1 => String::new(),
                m => format!("COMBO x{m}"),
            },
            HudText::Delivered => format!("{} / {}", game.delivered.min(game.target), game.target),
            HudText::Speed => format!("{}x", speed.0 as u32),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (dot, mut bg) in &mut dots {
        let c = if dot.0 < game.lives.left {
            LIFE_ON
        } else {
            LIFE_OFF
        };
        // Flash the dots while the post-hit grace period runs.
        let c = if game.lives.grace > 0.0 && (game.lives.grace * 6.0).fract() < 0.5 {
            DANGER
        } else {
            c
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    let frac = (game.delivered as f32 / game.target.max(1) as f32).min(1.0);
    for mut node in &mut fill {
        let w = percent(frac * 100.0);
        if node.width != w {
            node.width = w;
        }
    }
}

pub fn format_score(score: u64) -> String {
    let digits = score.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::format_score;

    #[test]
    fn scores_get_thousands_separators() {
        assert_eq!(format_score(0), "0");
        assert_eq!(format_score(999), "999");
        assert_eq!(format_score(1000), "1,000");
        assert_eq!(format_score(1234567), "1,234,567");
    }
}
