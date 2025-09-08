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

fn ui_system(mut context: NonSendMut<ImguiContext>, mut state: ResMut<UiState>) {
    let ui = context.ui();

    // Show the demo window
    if state.demo_window_open {
        ui.show_demo_window(&mut state.demo_window_open);
    }

    // Show a custom window
    ui.window("Hello Dear ImGui!")
        .size([300.0, 200.0], Condition::FirstUseEver)
        .position([50.0, 50.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Hello from dear-imgui-bevy!");
            ui.separator();
            
            if ui.button("Click me!") {
                state.counter += 1;
            }
            ui.same_line();
            ui.text(format!("Clicked {} times", state.counter));
            
            ui.separator();
            ui.checkbox("Show demo window", &mut state.demo_window_open);
        });
}
