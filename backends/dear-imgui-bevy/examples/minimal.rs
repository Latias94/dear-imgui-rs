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
    println!("ui_system called!");
    let ui = context.ui();
    println!("Got UI context successfully!");

    // Debug: Print display size and mouse info
    let io = ui.io();
    println!("Display size: {:?}", io.display_size());
    println!(
        "Display framebuffer scale: {:?}",
        io.display_framebuffer_scale()
    );
    println!("Mouse pos: {:?}", io.mouse_pos());

    // Show the demo window
    if state.demo_window_open {
        println!("Showing demo window");
        ui.show_demo_window(&mut state.demo_window_open);
    }

    // Show a custom window - force it to be visible and in the center
    println!("Creating custom window");
    let window = ui
        .window("Hello Dear ImGui!")
        .size([400.0, 300.0], Condition::Always) // Force size every frame
        .position([100.0, 100.0], Condition::Always) // Force position every frame
        .bg_alpha(1.0) // Ensure it's not transparent
        .build(|| {
            println!("Inside window build callback");
            ui.text("Hello from dear-imgui-bevy!");
            ui.text("This window should be visible!");
            ui.separator();

            if ui.button("Click me!") {
                state.counter += 1;
                println!("Button clicked! Counter: {}", state.counter);
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
        });

    println!("Window created: {:?}", window.is_some());
}
