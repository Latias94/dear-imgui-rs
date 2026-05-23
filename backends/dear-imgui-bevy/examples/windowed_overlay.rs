//! Persistent windowed Dear ImGui overlay inside a normal Bevy app.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example windowed_overlay`

use bevy::{
    app::AppExit,
    diagnostic::FrameCount,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiFrameOutput, ImguiPlugin, ImguiPrimaryContextPass,
};
use dear_imgui_rs::{Condition, ConfigFlags};

#[derive(Resource, Debug)]
struct RuntimeOverlayState {
    clicks: u32,
    show_metrics: bool,
    route_keyboard_to_imgui: bool,
}

impl Default for RuntimeOverlayState {
    fn default() -> Self {
        Self {
            clicks: 0,
            show_metrics: true,
            route_keyboard_to_imgui: true,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "dear-imgui-bevy windowed overlay".to_owned(),
                resolution: (1280, 720).into(),
                present_mode: PresentMode::AutoVsync,
                window_theme: Some(WindowTheme::Dark),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ImguiPlugin::default())
        .init_resource::<RuntimeOverlayState>()
        .add_systems(Startup, setup_imgui)
        .add_systems(Update, close_on_escape)
        .add_systems(ImguiPrimaryContextPass, runtime_overlay_ui)
        .run();
}

fn setup_imgui(mut commands: Commands, mut imgui: NonSendMut<ImguiContext>) {
    commands.spawn(Camera2d);

    let context = imgui.context_mut();
    context.io_mut().set_config_input_trickle_event_queue(false);
    let flags = context.io().config_flags() | ConfigFlags::DOCKING_ENABLE;
    context.io_mut().set_config_flags(flags);
    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn runtime_overlay_ui(
    mut contexts: ImguiContexts,
    mut state: ResMut<RuntimeOverlayState>,
    frame_count: Res<FrameCount>,
    output: Res<ImguiFrameOutput>,
) {
    let imgui_frame_index = contexts.frame_index();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    ui.window("Dear ImGui Runtime Overlay")
        .size([420.0, 260.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("This app uses Bevy's normal windowed runner.");
            ui.text("Press Escape to close the window.");
            ui.separator();
            ui.text(format!("Bevy frames: {}", frame_count.0));
            ui.text(format!("ImGui frame output: {}", output.frame_index()));
            ui.text(format!(
                "want_capture_mouse: {}",
                ui.io().want_capture_mouse()
            ));
            ui.text(format!(
                "want_capture_keyboard: {}",
                ui.io().want_capture_keyboard()
            ));
            ui.separator();
            if ui.button("Count click") {
                state.clicks = state.clicks.saturating_add(1);
            }
            ui.same_line();
            ui.text(format!("clicks: {}", state.clicks));
            ui.checkbox("Show metrics panel", &mut state.show_metrics);
            ui.checkbox(
                "Route keyboard shortcuts to ImGui",
                &mut state.route_keyboard_to_imgui,
            );
        });

    if state.show_metrics {
        ui.window("Runtime Smoke State")
            .size([320.0, 160.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Backend frame index: {imgui_frame_index:?}"));
                ui.text(format!(
                    "Route keyboard policy: {}",
                    state.route_keyboard_to_imgui
                ));
                if let Some(snapshot) = output.snapshot() {
                    ui.text(format!("Draw lists: {}", snapshot.draw.draw_lists.len()));
                    ui.text(format!(
                        "Texture requests: {}",
                        snapshot.texture_requests.len()
                    ));
                } else if let Some(error) = output.snapshot_error() {
                    ui.text(format!("Snapshot error: {error}"));
                }
            });
    }
}
