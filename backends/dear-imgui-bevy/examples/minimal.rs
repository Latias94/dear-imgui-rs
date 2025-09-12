//! Minimal example showing how to use dear-imgui-bevy

use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

#[derive(Resource)]
struct UiState {
    demo_window_open: bool,
    counter: i32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ImguiPlugin::default())
        .insert_resource(UiState {
            demo_window_open: true,
            counter: 0,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn a camera
    commands.spawn(Camera3d::default());
}

fn ui_system(context: NonSendMut<ImguiContext>, mut state: ResMut<UiState>) {
    context.with_ui(|ui| {
        // Show the demo window
        if state.demo_window_open {
            ui.show_demo_window(&mut state.demo_window_open);
        }

        // Show a custom window - use proper conditions to avoid flicker
        ui.window("Hello Dear ImGui!")
            .size([400.0, 300.0], Condition::FirstUseEver) // Only set size on first use
            .position([100.0, 100.0], Condition::FirstUseEver) // Only set position on first use
            .build(|| {
                ui.text("Hello from dear-imgui-bevy!");
                ui.text("This window should be stable now!");
                ui.separator();

                if ui.button("Click me!") {
                    state.counter += 1;
                }
                ui.same_line();
                ui.text(format!("Clicked {} times", state.counter));

                ui.separator();
                ui.checkbox("Show demo window", &mut state.demo_window_open);

                // Add some colorful content to make it more visible
                ui.separator();
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "RED TEXT");
                ui.text_colored([0.0, 1.0, 0.0, 1.0], "GREEN TEXT");
                ui.text_colored([0.0, 0.0, 1.0, 1.0], "BLUE TEXT");

                // Show FPS info
                ui.separator();
                let io = ui.io();
                ui.text(format!("FPS: {:.1}", io.framerate()));
                ui.text(format!("Frame Time: {:.3}ms", 1000.0 / io.framerate()));
            });
    });
}
