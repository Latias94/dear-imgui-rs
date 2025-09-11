//! Simple node editor example using dear-imnodeflow
//!
//! This example demonstrates how to create a basic node editor with ImNodeFlow
//! integrated with dear-imgui.

use dear_imgui::*;
use dear_imnodeflow::*;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize Dear ImGui context
    let mut ctx = Context::create_or_panic();

    // Configure ImGui
    let io = ctx.io_mut();
    io.set_config_flags(
        io.config_flags() | ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE,
    );

    // Create node editor
    let mut node_editor = NodeEditor::new("Simple Node Editor")?;
    node_editor.set_size(ImVec2 { x: 800.0, y: 600.0 });

    // Create some example nodes
    let mut nodes = Vec::new();

    // Create a simple input node
    let mut input_node = NodeBuilder::new()
        .title("Input Node")
        .position(ImVec2 { x: 100.0, y: 100.0 })
        .style(NodeStyle::green())
        .build()?;

    // Create a processing node
    let mut process_node = NodeBuilder::new()
        .title("Process Node")
        .position(ImVec2 { x: 300.0, y: 150.0 })
        .style(NodeStyle::cyan())
        .build()?;

    // Create an output node
    let mut output_node = NodeBuilder::new()
        .title("Output Node")
        .position(ImVec2 { x: 500.0, y: 100.0 })
        .style(NodeStyle::red())
        .build()?;

    nodes.push(input_node);
    nodes.push(process_node);
    nodes.push(output_node);

    // Set up the node editor handlers for all nodes
    for node in &mut nodes {
        node.set_handler(&node_editor);
    }

    // Main application state
    let mut show_demo = true;
    let mut show_metrics = false;
    let mut node_count = 3;

    println!("Simple Node Editor Example");
    println!("- Use mouse to navigate the node editor");
    println!("- Right-click for context menu (if implemented)");
    println!("- Drag nodes around");
    println!("- Connect pins by dragging from output to input");

    // In a real application, you would have a proper event loop here
    // For this example, we'll simulate a few frames
    for frame in 0..10 {
        println!("Frame {}", frame);

        // Begin frame
        let ui = ctx.frame();

        // Main menu bar
        if let Some(menu_bar) = ui.begin_main_menu_bar() {
            if let Some(menu) = ui.begin_menu("File") {
                if ui.menu_item("New") {
                    println!("New file requested");
                }
                if ui.menu_item("Open") {
                    println!("Open file requested");
                }
                if ui.menu_item("Save") {
                    println!("Save file requested");
                }
                ui.separator();
                if ui.menu_item("Exit") {
                    println!("Exit requested");
                    break;
                }
                menu.end();
            }

            if let Some(menu) = ui.begin_menu("View") {
                ui.checkbox("Show Demo", &mut show_demo);
                ui.checkbox("Show Metrics", &mut show_metrics);
                menu.end();
            }

            if let Some(menu) = ui.begin_menu("Nodes") {
                if ui.menu_item("Add Input Node") {
                    if let Ok(mut new_node) = NodeBuilder::new()
                        .title(&format!("Input Node {}", node_count))
                        .position(ImVec2 {
                            x: 50.0,
                            y: 50.0 + (node_count as f32 * 30.0),
                        })
                        .style(NodeStyle::green())
                        .build()
                    {
                        new_node.set_handler(&node_editor);
                        nodes.push(new_node);
                        node_count += 1;
                    }
                }
                if ui.menu_item("Add Process Node") {
                    if let Ok(mut new_node) = NodeBuilder::new()
                        .title(&format!("Process Node {}", node_count))
                        .position(ImVec2 {
                            x: 250.0,
                            y: 50.0 + (node_count as f32 * 30.0),
                        })
                        .style(NodeStyle::cyan())
                        .build()
                    {
                        new_node.set_handler(&node_editor);
                        nodes.push(new_node);
                        node_count += 1;
                    }
                }
                if ui.menu_item("Add Output Node") {
                    if let Ok(mut new_node) = NodeBuilder::new()
                        .title(&format!("Output Node {}", node_count))
                        .position(ImVec2 {
                            x: 450.0,
                            y: 50.0 + (node_count as f32 * 30.0),
                        })
                        .style(NodeStyle::red())
                        .build()
                    {
                        new_node.set_handler(&node_editor);
                        nodes.push(new_node);
                        node_count += 1;
                    }
                }
                menu.end();
            }

            menu_bar.end();
        }

        // Node editor window
        ui.window("Node Editor")
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position([50.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                // Update the node editor
                node_editor.update(&ui);

                // Update all nodes
                for node in &mut nodes {
                    node.update();
                }

                // Display node editor information
                ui.text(format!("Nodes: {}", node_editor.nodes_count()));
                ui.text(format!("Editor: {}", node_editor.name()));
                ui.text(format!(
                    "Position: ({:.1}, {:.1})",
                    node_editor.position().x,
                    node_editor.position().y
                ));
                ui.text(format!(
                    "Scroll: ({:.1}, {:.1})",
                    node_editor.scroll().x,
                    node_editor.scroll().y
                ));

                if node_editor.is_node_dragged() {
                    ui.text("Node is being dragged");
                }

                if node_editor.on_free_space() {
                    ui.text("Mouse on free space");
                }

                if node_editor.on_selected_node() {
                    ui.text("Mouse on selected node");
                }
            });

        // Properties window
        ui.window("Properties")
            .size([300.0, 400.0], Condition::FirstUseEver)
            .position([870.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Node Properties");
                ui.separator();

                for (i, node) in nodes.iter().enumerate() {
                    if let Ok(name) = node.name() {
                        if ui.collapsing_header(
                            &format!("Node {}: {}", i, name),
                            TreeNodeFlags::empty(),
                        ) {
                            ui.text(format!("UID: {}", node.uid()));
                            ui.text(format!(
                                "Position: ({:.1}, {:.1})",
                                node.position().x,
                                node.position().y
                            ));
                            ui.text(format!(
                                "Size: ({:.1}, {:.1})",
                                node.size().x,
                                node.size().y
                            ));
                            ui.text(format!("Hovered: {}", node.is_hovered()));
                            ui.text(format!("Selected: {}", node.is_selected()));
                            ui.text(format!("Dragged: {}", node.is_dragged()));

                            if node.should_destroy() {
                                ui.text("Marked for destruction");
                            }
                        }
                    }
                }
            });

        // Show demo window if requested
        if show_demo {
            ui.show_demo_window(&mut show_demo);
        }

        // Show metrics window if requested
        if show_metrics {
            ui.show_metrics_window(&mut show_metrics);
        }

        // Status bar
        ui.window("Status")
            .size([1200.0, 100.0], Condition::Always)
            .position([0.0, 650.0], Condition::Always)
            .flags(WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE | WindowFlags::NO_COLLAPSE)
            .build(|| {
                ui.text(format!("Frame: {}", frame));
                ui.same_line();
                ui.text(format!("Nodes: {}", nodes.len()));
                ui.same_line();
                ui.text("Status: Running");
            });

        // Simulate frame end
        println!("  - Updated {} nodes", nodes.len());
        println!(
            "  - Node editor at ({:.1}, {:.1})",
            node_editor.position().x,
            node_editor.position().y
        );
    }

    println!("Example completed successfully!");
    Ok(())
}

// Helper function to demonstrate bezier curve drawing
fn draw_example_bezier(_ui: &Ui) {
    use dear_imnodeflow::bezier;

    let p1 = ImVec2 { x: 100.0, y: 100.0 };
    let p2 = ImVec2 { x: 300.0, y: 200.0 };
    let color = 0xFF_FF_00_00; // Red color
    let thickness = 2.0;

    // Draw a bezier curve
    bezier::smart_bezier(p1, p2, color, thickness);

    // Test collision detection
    let mouse_pos = ImVec2 { x: 200.0, y: 150.0 }; // Simulated mouse position
    let radius = 5.0;

    if bezier::smart_bezier_collider(mouse_pos, p1, p2, radius) {
        println!("Mouse is near the bezier curve!");
    }
}
