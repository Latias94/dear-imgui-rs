//! Minimal embedded Dear ImGui overlay inside a Bevy app.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example simple`

use bevy_app::{App, ScheduleRunnerPlugin, Startup};
use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
};

#[derive(Resource, Default)]
struct OverlayState {
    frames: u64,
    clicks: u32,
}

fn main() {
    App::new()
        .add_plugins(ScheduleRunnerPlugin::run_once())
        .add_plugins(ImguiPlugin::default())
        .init_resource::<OverlayState>()
        .add_systems(Startup, setup)
        .add_systems(ImguiPrimaryContextPass, overlay_ui)
        .run();
}

fn setup(mut commands: Commands, mut imgui: NonSendMut<ImguiContext>) {
    commands.spawn((
        Window {
            title: "dear-imgui-bevy simple".to_owned(),
            resolution: WindowResolution::new(1280, 720),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    configure_example_context(&mut imgui, false);
}

fn overlay_ui(mut contexts: ImguiContexts, mut state: ResMut<OverlayState>) {
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    state.frames = state.frames.saturating_add(1);

    ui.window("Dear ImGui Overlay").build(|| {
        ui.text("Dear ImGui is running inside Bevy schedules.");
        ui.separator();
        ui.text(format!("Frame: {}", state.frames));
        ui.text(format!("Button clicks: {}", state.clicks));
        if ui.button("Count click") {
            state.clicks = state.clicks.saturating_add(1);
        }
    });
}
