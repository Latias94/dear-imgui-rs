//! Basic test to verify dear-imgui-bevy is working

use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

fn main() {
    println!("Starting dear-imgui-bevy test...");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dear ImGui Bevy Test".to_string(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ImguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (ui_system, exit_system))
        .run();
}

fn setup(mut commands: Commands) {
    println!("Setup called - spawning camera");
    commands.spawn(Camera3d::default());
}

fn ui_system(context: NonSendMut<ImguiContext>) {
    println!("UI system called!");

    context.with_ui(|ui| {
        println!("Got UI frame!");

        // Simple window
        ui.window("Test Window")
            .size([300.0, 200.0], Condition::FirstUseEver)
            .position([50.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                println!("Inside window build!");
                ui.text("Hello from Dear ImGui!");
                ui.text("If you can see this, it's working!");

                if ui.button("Test Button") {
                    println!("Button clicked!");
                }
            });

        println!("Window created successfully");
    });
}

fn exit_system(keyboard_input: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        println!("Escape pressed, exiting...");
        exit.send(AppExit::Success);
    }
}
