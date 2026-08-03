//! Smallest visible Dear ImGui overlay in a normal Bevy app.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example simple`

use bevy::{
    app::AppExit,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::prelude::*;
use dear_imgui_rs::Condition;

#[derive(Resource, Default)]
struct SimpleUiState {
    clicks: u32,
    show_demo_window: bool,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy simple".to_owned(),
            resolution: (1024, 640).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(ImguiPlugin::default())
    .init_resource::<SimpleUiState>()
    .add_systems(Startup, setup)
    .add_systems(Update, close_on_escape);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(simple_ui))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn simple_ui(frame: ImguiFrame<'_>, mut state: ResMut<SimpleUiState>) {
    let frame_index = frame.frame_index();
    let ui = frame.ui();

    ui.window("Tools")
        .size([320.0, 180.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Dear ImGui is drawing in Bevy.");
            ui.text(format!("Frame: {frame_index}"));
            ui.separator();

            if ui.button("Count click") {
                state.clicks = state.clicks.saturating_add(1);
            }
            ui.same_line();
            ui.text(format!("{}", state.clicks));
            ui.checkbox("Show demo window", &mut state.show_demo_window);
        });

    if state.show_demo_window {
        ui.show_demo_window(&mut state.show_demo_window);
    }
}
