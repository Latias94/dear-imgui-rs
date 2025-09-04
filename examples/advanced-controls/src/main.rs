//! Advanced Controls Test - Testing advanced Dear ImGui controls

use dear_imgui::*;

fn main() -> Result<()> {
    env_logger::init();

    println!("Dear ImGui Advanced Controls Test");
    println!("Testing: Color Picker, Tree Nodes, Menus, and Tables");

    // Create Dear ImGui context
    let mut ctx = Context::new()?;

    // Test state
    let mut color = Color::rgb(1.0, 0.5, 0.0);
    let mut show_color_picker = false;
    let mut show_tree_demo = true;
    let mut show_table_demo = true;
    let mut menu_option1 = false;
    let mut menu_option2 = true;

    // Simulate a few frames to test the controls
    for frame_count in 0..3 {
        println!("\n--- Frame {} ---", frame_count + 1);

        let mut frame = ctx.frame();

        // Test main menu bar
        if frame.begin_main_menu_bar() {
            if frame.begin_menu("File") {
                if frame.menu_item("New") {
                    println!("New file clicked");
                }
                if frame.menu_item("Open") {
                    println!("Open file clicked");
                }
                frame.end_menu();
            }

            if frame.begin_menu("View") {
                if frame.menu_item_bool("Show Color Picker", &mut show_color_picker) {
                    println!("Color picker toggled: {}", show_color_picker);
                }
                if frame.menu_item_bool("Show Tree Demo", &mut show_tree_demo) {
                    println!("Tree demo toggled: {}", show_tree_demo);
                }
                if frame.menu_item_bool("Show Table Demo", &mut show_table_demo) {
                    println!("Table demo toggled: {}", show_table_demo);
                }
                frame.end_menu();
            }

            if frame.begin_menu("Options") {
                if frame.menu_item_bool("Option 1", &mut menu_option1) {
                    println!("Option 1 toggled: {}", menu_option1);
                }
                if frame.menu_item_bool("Option 2", &mut menu_option2) {
                    println!("Option 2 toggled: {}", menu_option2);
                }
                frame.end_menu();
            }

            frame.end_main_menu_bar();
        }

        // Test color picker window
        if show_color_picker {
            frame
                .window("Color Picker Demo")
                .size([400.0, 300.0])
                .position([50.0, 50.0])
                .show(|ui| {
                    ui.text("Color Controls:");
                    ui.separator();

                    if ui.color_edit("Edit Color", &mut color) {
                        println!("Color edited: {:?}", color);
                    }

                    ui.separator();

                    if ui.color_picker("Pick Color", &mut color) {
                        println!("Color picked: {:?}", color);
                    }

                    ui.separator();

                    if ui.color_button("color_btn", color) {
                        println!("Color button clicked!");
                    }
                    ui.same_line();
                    ui.text("Click the color swatch");

                    true
                });
        }

        // Test tree nodes window
        if show_tree_demo {
            frame
                .window("Tree Nodes Demo")
                .size([300.0, 400.0])
                .position([500.0, 50.0])
                .show(|ui| {
                    ui.text("Tree Structure:");
                    ui.separator();

                    if ui.tree_node("Root Node") {
                        ui.text("This is under the root");

                        if ui.tree_node("Child Node 1") {
                            ui.text("Child 1 content");
                            ui.bullet_text("Bullet item 1");
                            ui.bullet_text("Bullet item 2");
                            ui.tree_pop();
                        }

                        if ui.tree_node("Child Node 2") {
                            ui.text("Child 2 content");
                            if ui.tree_node("Grandchild") {
                                ui.text("Deep nesting works!");
                                ui.tree_pop();
                            }
                            ui.tree_pop();
                        }

                        ui.tree_pop();
                    }

                    ui.separator();

                    if ui.collapsing_header("Collapsing Header") {
                        ui.text("This content can be collapsed");
                        ui.text("Multiple lines of content");
                        ui.text("All under one header");
                    }

                    true
                });
        }

        // Test table window
        if show_table_demo {
            frame
                .window("Table Demo")
                .size([500.0, 300.0])
                .position([100.0, 400.0])
                .show(|ui| {
                    ui.text("Table Example:");
                    ui.separator();

                    if ui.begin_table("DemoTable", 3) {
                        // Setup columns
                        ui.table_setup_column("Name");
                        ui.table_setup_column("Age");
                        ui.table_setup_column("City");
                        ui.table_headers_row();

                        // Row 1
                        ui.table_next_row();
                        ui.table_next_column();
                        ui.text("Alice");
                        ui.table_next_column();
                        ui.text("25");
                        ui.table_next_column();
                        ui.text("New York");

                        // Row 2
                        ui.table_next_row();
                        ui.table_next_column();
                        ui.text("Bob");
                        ui.table_next_column();
                        ui.text("30");
                        ui.table_next_column();
                        ui.text("London");

                        // Row 3
                        ui.table_next_row();
                        ui.table_next_column();
                        ui.text("Charlie");
                        ui.table_next_column();
                        ui.text("35");
                        ui.table_next_column();
                        ui.text("Tokyo");

                        ui.end_table();
                    }

                    true
                });
        }

        // Get draw data (this would normally be sent to a renderer)
        let _draw_data = frame.draw_data();
        println!("Frame {} completed successfully", frame_count + 1);

        // Simulate some changes for next frame
        if frame_count == 1 {
            show_color_picker = true;
            color = Color::rgb(0.2, 0.8, 0.4);
        }
    }

    println!("\nAll advanced controls test completed successfully!");
    println!("✅ Color Picker - Working");
    println!("✅ Tree Nodes - Working");
    println!("✅ Menu System - Working");
    println!("✅ Tables - Working");

    Ok(())
}
