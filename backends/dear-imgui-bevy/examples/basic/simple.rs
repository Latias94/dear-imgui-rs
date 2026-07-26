//! Smallest visible Dear ImGui overlay in a normal Bevy app.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example simple`

use bevy::{
    app::AppExit,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, ImguiUi, configure_example_context,
    render::ImguiOverlayCamera,
};
use dear_imgui_rs::Condition;

#[derive(Resource, Default)]
struct SimpleUiState {
    clicks: u32,
    show_demo_window: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
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
        .add_systems(Update, close_on_escape)
        .add_systems(ImguiPrimaryContextPass, simple_ui)
        .run();
}

fn setup(mut commands: Commands, mut contexts: NonSendMut<ImguiContexts>) -> Result {
    commands.spawn((Camera2d, ImguiOverlayCamera));
    let primary_id = contexts
        .primary_id()
        .ok_or("ImguiPlugin should install a primary Context before Startup")?;
    contexts
        .configure(primary_id, |context| {
            configure_example_context(context, false);
        })
        .map_err(|error| format!("failed to configure the primary Dear ImGui Context: {error}"))?;
    Ok(())
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn simple_ui(imgui: ImguiUi, mut state: ResMut<SimpleUiState>) -> Result {
    let frame_index = imgui.frame_index()?;
    let ui = imgui.ui()?;

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

    Ok(())
}
