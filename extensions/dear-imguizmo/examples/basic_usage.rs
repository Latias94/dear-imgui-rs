//! Basic ImGuizmo usage example
//!
//! This example demonstrates how to use ImGuizmo for 3D object manipulation.

use dear_imgui::*;
use dear_imguizmo::*;

fn main() {
    println!("ImGuizmo Basic Usage Example");

    // Create ImGui context
    let mut imgui_ctx = Context::create_or_panic();

    // Create ImGuizmo context
    let gizmo_ctx = GuizmoContext::create(&imgui_ctx);

    // Example transformation matrix (identity matrix)
    let mut object_matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    // Example camera matrices
    let view_matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, -10.0, 0.0, 0.0, 0.0, 1.0,
    ];

    let projection_matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    // Simulate a frame
    let ui = imgui_ctx.frame();
    let gizmo_ui = gizmo_ctx.get_ui(&ui);

    // Set up the viewport
    gizmo_ui.set_rect(0.0, 0.0, 800.0, 600.0);
    gizmo_ui.enable(true);

    // Test matrix decomposition
    if let Ok((translation, rotation, scale)) = gizmo_ui.decompose_matrix(&object_matrix) {
        println!("Translation: {:?}", translation);
        println!("Rotation: {:?}", rotation);
        println!("Scale: {:?}", scale);

        // Test matrix recomposition
        let recomposed = gizmo_ui.recompose_matrix(&translation, &rotation, &scale);
        println!("Recomposed matrix: {:?}", recomposed);
    }

    // Test manipulation (this would normally be in a render loop)
    if let Some(result) = gizmo_ui
        .manipulate(&view_matrix, &projection_matrix)
        .operation(Operation::TRANSLATE)
        .mode(Mode::World)
        .matrix(&mut object_matrix)
        .build()
    {
        if result.used {
            println!("Object was manipulated!");
            if let Some(delta) = result.delta_matrix {
                println!("Delta matrix: {:?}", delta);
            }
        }

        if result.hovered {
            println!("Gizmo is hovered");
        }
    }

    // Test style configuration
    let mut style = gizmo_ui.get_style();
    style.translation_line_thickness = 5.0;
    style.colors[ColorType::DirectionX as usize] = [1.0, 0.0, 0.0, 1.0]; // Red X axis
    gizmo_ui.set_style(&style);

    // Test using style builder
    let custom_style = StyleBuilder::new()
        .translation_line_thickness(4.0)
        .color(ColorType::DirectionY, [0.0, 1.0, 0.0, 1.0]) // Green Y axis
        .color(ColorType::DirectionZ, [0.0, 0.0, 1.0, 1.0]) // Blue Z axis
        .build();

    gizmo_ui.set_style(&custom_style);

    // Test ID management
    {
        let _id_guard = IdGuard::new(&gizmo_ui, "object1");
        // Gizmo operations for object1 would go here
    } // ID is automatically popped when guard goes out of scope

    println!("Example completed successfully!");
}
