//! Menus, overlays and the in-game HUD.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{Activate, Button};

use crate::states::AppState;

pub const TEXT: Color = Color::srgb(0.93, 0.94, 0.96);
pub const TEXT_DIM: Color = Color::srgb(0.62, 0.66, 0.72);
pub const ACCENT: Color = Color::srgb(1.0, 0.76, 0.24);
const BUTTON_BG: Color = Color::srgb(0.16, 0.18, 0.22);
const BUTTON_HOVER: Color = Color::srgb(0.24, 0.27, 0.33);
const BUTTON_PRESS: Color = Color::srgb(0.32, 0.36, 0.44);

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(OnEnter(AppState::InGame), spawn_placeholder)
            .add_systems(Update, style_buttons);
    }
}

/// A rounded, flat button that reacts to hover and press.
pub fn button(label: &str, width: f32) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Node {
            width: px(width),
            height: px(46),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(TEXT),
        )],
    )
}

fn style_buttons(mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>) {
    for (hovered, pressed, mut bg) in &mut buttons {
        let color = match (hovered.get(), pressed) {
            (_, true) => BUTTON_PRESS,
            (true, false) => BUTTON_HOVER,
            _ => BUTTON_BG,
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::Menu),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(18),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("RUSH HOUR"),
                TextFont {
                    font_size: FontSize::Px(72.0),
                    ..default()
                },
                TextColor(ACCENT),
            ));
            root.spawn((
                Text::new("Keep the city moving. You control the lights."),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(TEXT_DIM),
            ));
            root.spawn(button("Play", 220.0)).observe(
                |_: On<Activate>, mut next: ResMut<NextState<AppState>>| {
                    next.set(AppState::InGame);
                },
            );
        });
}

fn spawn_placeholder(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Text::new(
            "Click a junction to switch its lights - P pauses, M in pause returns to the menu",
        ),
        TextColor(TEXT_DIM),
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            left: px(16),
            ..default()
        },
    ));
}
