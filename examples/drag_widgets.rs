//! Example demonstrating drag widgets
//!
//! This example shows how to use various drag widgets including:
//! - Basic drag widgets for float and int values
//! - Drag widgets with custom configuration

use dear_imgui::*;

fn main() {
    println!("Dear ImGui Drag Widgets Example");

    // Create context
    let mut ctx = Context::create();

    // Create a frame
    let ui = ctx.frame();

    // Test basic drag widgets
    let mut float_value = 0.0f32;
    let mut int_value = 0i32;

    // Basic drag widgets (using the actual available methods)
    ui.drag_float("Basic Float", &mut float_value);
    ui.drag_int("Basic Int", &mut int_value);

    // Test with different ranges and speeds
    let mut configured_float = 1.0f32;
    let mut configured_int = 10i32;

    // Use the drag method with configuration (correct API)
    ui.drag_config("Configured Float")
        .speed(0.1)
        .range(0.0, 100.0)
        .build(&ui, &mut configured_float);

    ui.drag_config("Configured Int")
        .speed(1.0)
        .range(0, 1000)
        .build(&ui, &mut configured_int);

    // Test multiple values
    let mut float_array = [1.0f32, 2.0, 3.0];
    let mut int_array = [10i32, 20, 30];

    // Drag individual array elements
    for (i, value) in float_array.iter_mut().enumerate() {
        ui.drag_config(&format!("Float {}", i))
            .speed(0.1)
            .range(0.0, 10.0)
            .build(&ui, value);
    }

    for (i, value) in int_array.iter_mut().enumerate() {
        ui.drag_config(&format!("Int {}", i))
            .speed(1.0)
            .range(0, 100)
            .build(&ui, value);
    }

    println!("Basic values: float={}, int={}", float_value, int_value);
    println!("Configured values: float={:.2}, int={}", configured_float, configured_int);
    println!("Float array: {:?}", float_array);
    println!("Int array: {:?}", int_array);

    println!("Drag widgets example completed successfully!");
}
