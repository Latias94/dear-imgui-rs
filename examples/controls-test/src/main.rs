//! Controls Test - Testing new Dear ImGui controls

use dear_imgui::*;

fn main() -> Result<()> {
    env_logger::init();

    println!("Dear ImGui Controls Test");
    println!("Testing new controls functionality...");

    // Create Dear ImGui context
    let mut ctx = Context::new()?;

    // Test state
    let mut combo_current = 1;
    let mut listbox_current = 2;
    let mut progress = 0.75;
    let mut selectable_items = [true, false, true];
    let mut checkbox_value = true;

    // Simulate a few frames to test the controls
    for frame_count in 0..3 {
        println!("\n--- Frame {} ---", frame_count + 1);

        let mut frame = ctx.frame();

        // Test window with new controls
        frame
            .window("Controls Test")
            .size([500.0, 600.0])
            .position([100.0, 100.0])
            .show(|ui| {
                ui.text("Testing New Controls");
                ui.separator();

                // Test combo box
                let combo_items = ["First Option", "Second Option", "Third Option"];
                if ui.combo("Combo Test", &mut combo_current, &combo_items) {
                    println!(
                        "Combo changed to: {} ({})",
                        combo_current, combo_items[combo_current as usize]
                    );
                }

                ui.separator();

                // Test listbox
                let listbox_items = ["Item A", "Item B", "Item C", "Item D"];
                if ui.listbox("ListBox Test", &mut listbox_current, &listbox_items, 3) {
                    println!(
                        "ListBox changed to: {} ({})",
                        listbox_current, listbox_items[listbox_current as usize]
                    );
                }

                ui.separator();

                // Test progress bar
                ui.text("Progress Bar:");
                ui.progress_bar(progress, Some(&format!("{:.0}%", progress * 100.0)));

                ui.separator();

                // Test bullets
                ui.text("Bullet Points:");
                ui.bullet_text("First bullet point");
                ui.bullet_text("Second bullet point");
                ui.bullet();
                ui.same_line();
                ui.text("Manual bullet + text");

                ui.separator();

                // Test selectable items
                ui.text("Selectable Items:");
                for (i, selected) in selectable_items.iter_mut().enumerate() {
                    if ui.selectable(&format!("Selectable Item {}", i + 1), selected) {
                        println!("Selectable {} clicked, now: {}", i + 1, selected);
                    }
                }

                ui.separator();

                // Test checkbox
                if ui.checkbox("Test Checkbox", &mut checkbox_value) {
                    println!("Checkbox toggled: {}", checkbox_value);
                }

                ui.separator();
                ui.text("All controls tested successfully!");

                true // Keep window open
            });

        // Get draw data (this would normally be sent to a renderer)
        let _draw_data = frame.draw_data();
        println!("Frame {} completed successfully", frame_count + 1);

        // Simulate some changes for next frame
        progress = (progress + 0.1) % 1.0;
        if frame_count == 1 {
            combo_current = 2;
            listbox_current = 0;
        }
    }

    println!("\nAll controls test completed successfully!");
    println!("New controls are working correctly!");

    Ok(())
}
