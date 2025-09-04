//! Hello World example for Dear ImGui Rust bindings

use dear_imgui::*;

fn main() -> Result<()> {
    env_logger::init();

    println!("Dear ImGui Rust Bindings - Hello World Example");
    println!("Testing basic UI functionality...");

    // Create Dear ImGui context
    let mut ctx = Context::new()?;

    // Application state
    let mut show_demo = true;
    let mut counter = 0;
    let mut text_input = String::from("Hello, World!");
    let mut float_value = 0.5f32;
    let mut int_value = 42i32;
    let mut checkbox_value = true;

    // Simulate a few frames to test the UI
    for frame_count in 0..3 {
        println!("\n--- Frame {} ---", frame_count + 1);

        let mut frame = ctx.frame();

        // Main window
        let window_open = frame
            .window("Hello, Dear ImGui!")
            .size([400.0, 300.0])
            .position([100.0, 100.0])
            .show(|ui| {
                ui.text("Welcome to Dear ImGui Rust bindings!");
                ui.text_colored(Color::GREEN, "This text is green!");
                ui.text_disabled("This text is disabled");

                ui.separator();

                if ui.button("Click me!") {
                    counter += 1;
                    println!("Button clicked! Counter: {}", counter);
                }

                ui.same_line();
                ui.text(format!("Counter: {}", counter));

                ui.separator();

                if ui.checkbox("Show demo", &mut show_demo) {
                    println!("Demo checkbox toggled: {}", show_demo);
                }

                if ui.slider_float("Float value", &mut float_value, 0.0, 1.0) {
                    println!("Float value changed: {}", float_value);
                }

                if ui.slider_int("Int value", &mut int_value, 0, 100) {
                    println!("Int value changed: {}", int_value);
                }

                if ui.input_text("Text input", &mut text_input) {
                    println!("Text changed: {}", text_input);
                }

                ui.spacing();

                if ui.small_button("Small") {
                    println!("Small button clicked!");
                }

                ui.same_line();
                if ui.button_with_size("Big Button", Vec2::new(100.0, 30.0)) {
                    println!("Big button clicked!");
                }

                true // Keep window open
            });

        if !window_open {
            println!("Window was closed");
            break;
        }

        // Get draw data (this would normally be sent to a renderer)
        let _draw_data = frame.draw_data();
        println!("Frame {} completed successfully", frame_count + 1);
    }

    println!("\nAll frames completed successfully!");
    println!("Dear ImGui Rust bindings are working!");

    Ok(())
}
