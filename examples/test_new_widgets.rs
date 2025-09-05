use dear_imgui::*;

fn main() {
    println!("Testing new widgets implementation...");

    // Create context
    let mut context = Context::create();
    context.io_mut().set_display_size([800.0, 600.0]);

    // Test state
    let mut color3 = [1.0, 0.5, 0.2];
    let mut color4 = [0.4, 0.7, 0.0, 0.5];
    let plot_values = vec![0.6, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2];
    let histogram_values = vec![0.2, 0.1, 1.0, 0.5, 0.92, 0.1, 0.2, 0.8, 0.3];
    let texture_id = ImageTextureId::new(12345);

    println!("Initial color3: {:?}", color3);
    println!("Initial color4: {:?}", color4);

    // Test frame
    let ui = context.frame();

    println!("Testing Color Edit widgets...");

    // Test color edit 3
    let changed3 = ui.color_edit3("Test Color RGB", &mut color3);
    println!(
        "Color edit 3 changed: {}, new value: {:?}",
        changed3, color3
    );

    // Test color edit 4
    let changed4 = ui.color_edit4("Test Color RGBA", &mut color4);
    println!(
        "Color edit 4 changed: {}, new value: {:?}",
        changed4, color4
    );

    // Test color picker 3
    let picker_changed = ui.color_picker3("Test Picker", &mut color3);
    println!("Color picker 3 changed: {}", picker_changed);

    // Test color picker 4
    let picker4_changed = ui.color_picker4("Test Picker 4", &mut color4);
    println!("Color picker 4 changed: {}", picker4_changed);

    // Test color button
    let button_clicked = ui.color_button("test_btn", color4);
    println!("Color button clicked: {}", button_clicked);

    println!("Testing Image widgets...");

    // Test image
    ui.image(texture_id, [100.0, 100.0]);
    println!("Image widget rendered");

    // Test image button
    let img_button_clicked = ui.image_button("test_img_btn", texture_id, [50.0, 50.0]);
    println!("Image button clicked: {}", img_button_clicked);

    println!("Testing Plot widgets...");

    // Test plot lines
    ui.plot_lines("Test Plot", &plot_values);
    println!("Plot lines rendered with {} values", plot_values.len());

    // Test plot histogram
    ui.plot_histogram("Test Histogram", &histogram_values);
    println!(
        "Plot histogram rendered with {} values",
        histogram_values.len()
    );

    println!("Testing advanced configurations...");

    // Test color edit with flags
    let advanced_changed = ui
        .color_edit4_config("Advanced Color", &mut color4)
        .flags(ColorEditFlags::ALPHA_BAR | ColorEditFlags::DISPLAY_HSV)
        .build();
    println!("Advanced color edit changed: {}", advanced_changed);

    // Test color picker with reference
    let mut picker_color = [0.8, 0.2, 0.6, 1.0];
    let reference_color = [1.0, 1.0, 1.0, 1.0];
    let ref_picker_changed = ui
        .color_picker4_config("Picker with Ref", &mut picker_color)
        .reference_color(reference_color)
        .flags(ColorEditFlags::ALPHA_BAR)
        .build();
    println!("Reference picker changed: {}", ref_picker_changed);

    // Test image with custom settings
    ui.image_config(texture_id, [80.0, 80.0])
        .uv0([0.0, 0.0])
        .uv1([0.5, 0.5])
        .tint_color([1.0, 0.5, 0.5, 1.0])
        .border_color([1.0, 1.0, 0.0, 1.0])
        .build();
    println!("Custom image rendered");

    // Test image button with custom settings
    let custom_img_btn = ui
        .image_button_config("custom_img_btn", texture_id, [60.0, 60.0])
        .uv0([0.0, 0.0])
        .uv1([1.0, 1.0])
        .bg_color([0.2, 0.2, 0.2, 1.0])
        .tint_color([0.8, 1.0, 0.8, 1.0])
        .build();
    println!("Custom image button clicked: {}", custom_img_btn);

    // Test plot with custom settings
    ui.plot_lines_config("Custom Plot", &plot_values)
        .scale_min(0.0)
        .scale_max(1.0)
        .graph_size([200.0, 100.0])
        .overlay_text("Custom overlay")
        .build();
    println!("Custom plot lines rendered");

    // Test histogram with custom settings
    ui.plot_histogram_config("Custom Histogram", &histogram_values)
        .scale_min(0.0)
        .scale_max(1.2)
        .graph_size([150.0, 80.0])
        .overlay_text("Max: 1.2")
        .build();
    println!("Custom histogram rendered");

    // Test color button with custom settings
    let custom_color_btn = ui
        .color_button_config("custom_color_btn", [0.9, 0.3, 0.1, 1.0])
        .flags(ColorEditFlags::NO_BORDER | ColorEditFlags::ALPHA_PREVIEW)
        .size([40.0, 40.0])
        .build();
    println!("Custom color button clicked: {}", custom_color_btn);

    println!("All widget tests completed successfully!");
    println!("Final color3: {:?}", color3);
    println!("Final color4: {:?}", color4);

    // Test flag operations
    println!("Testing ColorEditFlags operations...");
    let flags1 = ColorEditFlags::ALPHA_BAR;
    let flags2 = ColorEditFlags::NO_BORDER;
    let combined = flags1 | flags2;
    println!("Combined flags bits: {}", combined.bits());
    println!(
        "Contains ALPHA_BAR: {}",
        combined.contains(ColorEditFlags::ALPHA_BAR)
    );
    println!(
        "Contains NO_BORDER: {}",
        combined.contains(ColorEditFlags::NO_BORDER)
    );
    println!(
        "Contains DISPLAY_RGB: {}",
        combined.contains(ColorEditFlags::DISPLAY_RGB)
    );

    println!("Testing Tab widgets...");

    // Test tab bar
    if let Some(_tab_bar) = ui.tab_bar("test_tab_bar") {
        println!("Tab bar opened");

        // Test tab items
        if let Some(_tab1) = ui.tab_item("Tab 1") {
            ui.text("Content of tab 1");
            println!("Tab 1 is active");
        }

        if let Some(_tab2) = ui.tab_item("Tab 2") {
            ui.text("Content of tab 2");
            println!("Tab 2 is active");
        }

        let mut tab3_open = true;
        if let Some(_tab3) = ui.tab_item_with_opened("Tab 3 (closable)", &mut tab3_open) {
            ui.text("Content of tab 3 - this tab can be closed");
            println!("Tab 3 is active, open: {}", tab3_open);
        }

        // Test tab with flags
        if let Some(_tab4) =
            ui.tab_item_with_flags("Tab 4 (unsaved)", None, TabItemFlags::UNSAVED_DOCUMENT)
        {
            ui.text("This tab shows as unsaved");
            println!("Tab 4 (unsaved) is active");
        }
    } else {
        println!("Tab bar not opened");
    }

    // Test tab bar with flags
    if let Some(_tab_bar) = ui.tab_bar_with_flags(
        "test_tab_bar_flags",
        TabBarFlags::REORDERABLE | TabBarFlags::AUTO_SELECT_NEW_TABS,
    ) {
        println!("Tab bar with flags opened");

        if let Some(_tab) = ui.tab_item("Reorderable Tab") {
            ui.text("This tab can be reordered");
        }
    }

    // Test TabBar builder pattern
    TabBar::new("builder_tab_bar")
        .reorderable(true)
        .flags(TabBarFlags::TAB_LIST_POPUP_BUTTON)
        .build(&ui, || {
            println!("Builder tab bar opened");

            TabItem::new("Builder Tab 1")
                .flags(TabItemFlags::LEADING)
                .build(&ui, || {
                    ui.text("This is a leading tab");
                    println!("Builder tab 1 active");
                });

            TabItem::new("Builder Tab 2")
                .flags(TabItemFlags::TRAILING)
                .build(&ui, || {
                    ui.text("This is a trailing tab");
                    println!("Builder tab 2 active");
                });
        });

    println!("Tab widget tests completed!");
}
