//! Dear ImGui integrated into an existing Bevy game/app loop.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example app_integration`

use bevy::{
    app::AppExit,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
    input::ImguiInputCapture, render::ImguiOverlayCamera,
};
use dear_imgui_rs::Condition;

const ARENA_SIZE: Vec2 = Vec2::new(820.0, 420.0);
const PLAYER_SIZE: Vec2 = Vec2::new(72.0, 72.0);

#[derive(Component)]
struct Player;

#[derive(Resource, Debug)]
struct AppState {
    paused: bool,
    speed: f32,
    tint: [f32; 3],
    show_input_status: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            paused: false,
            speed: 260.0,
            tint: [0.20, 0.68, 0.92],
            show_input_status: true,
        }
    }
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "dear-imgui-bevy app integration".to_owned(),
                    resolution: (1280, 720).into(),
                    present_mode: PresentMode::AutoVsync,
                    window_theme: Some(WindowTheme::Dark),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            DebugUiPlugin,
        ))
        .init_resource::<AppState>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (close_on_escape, move_player, apply_player_tint))
        .run();
}

struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ImguiPlugin::default())
            .add_systems(Startup, setup_imgui)
            .add_systems(ImguiPrimaryContextPass, tools_ui);
    }
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Sprite::from_color(Color::srgb(0.10, 0.12, 0.16), ARENA_SIZE),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
    commands.spawn((
        Sprite::from_color(Color::srgb(0.20, 0.68, 0.92), PLAYER_SIZE),
        Transform::default(),
        Player,
    ));
}

fn setup_imgui(mut commands: Commands, mut imgui: NonSendMut<ImguiContext>) {
    commands.spawn((Camera2d, ImguiOverlayCamera));
    configure_example_context(&mut imgui, false);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn move_player(
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    capture: Res<ImguiInputCapture>,
    state: Res<AppState>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    if state.paused || capture.wants_keyboard_input() {
        return;
    }

    let mut direction = Vec2::ZERO;
    if input.pressed(KeyCode::KeyA) || input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) || input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if input.pressed(KeyCode::KeyW) || input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if direction == Vec2::ZERO {
        return;
    }

    let Ok(mut transform) = player.single_mut() else {
        return;
    };
    let step = direction.normalize() * state.speed * time.delta().as_secs_f32();
    transform.translation.x =
        (transform.translation.x + step.x).clamp(-ARENA_SIZE.x * 0.5, ARENA_SIZE.x * 0.5);
    transform.translation.y =
        (transform.translation.y + step.y).clamp(-ARENA_SIZE.y * 0.5, ARENA_SIZE.y * 0.5);
}

fn apply_player_tint(state: Res<AppState>, mut player: Query<&mut Sprite, With<Player>>) {
    if !state.is_changed() {
        return;
    }
    let Ok(mut sprite) = player.single_mut() else {
        return;
    };
    sprite.color = Color::srgb(state.tint[0], state.tint[1], state.tint[2]);
}

fn tools_ui(
    mut contexts: ImguiContexts,
    capture: Res<ImguiInputCapture>,
    mut state: ResMut<AppState>,
    player: Query<&Transform, With<Player>>,
) {
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    ui.window("App Tools")
        .size([360.0, 260.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("WASD / arrows move the square.");
            ui.checkbox("Pause app movement", &mut state.paused);
            ui.slider_f32("Movement speed", &mut state.speed, 80.0, 520.0);
            ui.color_edit3("Tint", &mut state.tint);
            ui.checkbox("Show input status", &mut state.show_input_status);

            if let Ok(transform) = player.single() {
                ui.separator();
                ui.text(format!(
                    "Player: x={:.1}, y={:.1}",
                    transform.translation.x, transform.translation.y
                ));
            }
        });

    if state.show_input_status {
        ui.window("Input Policy")
            .size([320.0, 150.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!(
                    "ImGui wants mouse: {}",
                    capture.wants_pointer_input()
                ));
                ui.text(format!(
                    "ImGui wants keyboard: {}",
                    capture.wants_keyboard_input()
                ));
                ui.text(format!("ImGui wants text: {}", capture.wants_text_input()));
            });
    }
}
