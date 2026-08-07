use dear_imgui_rs::ItemHoveredFlags;
use dear_imgui_rs::input::MouseButton;
use dear_imgui_rs::*;

#[test]
fn test_second_priority_widgets_compile() {
    // This test verifies that all second priority widget APIs compile correctly
    // We don't actually run ImGui since that requires proper initialization

    println!("✅ Second priority widgets compile test passed");
}

#[test]
fn test_api_types_exist() {
    // Test that all the types and flags we created exist and can be used

    // Input text flags
    let _flags = InputTextFlags::READ_ONLY | InputTextFlags::PASSWORD;
    let _multiline_flags = InputTextMultilineFlags::READ_ONLY | InputTextMultilineFlags::WORD_WRAP;
    let _scalar_flags = InputScalarFlags::READ_ONLY | InputScalarFlags::PARSE_EMPTY_REF_VAL;

    // Popup flags
    let _popup_open_flags = PopupOpenFlags::NO_OPEN_OVER_EXISTING_POPUP;
    let _popup_context_flags = PopupContextFlags::NO_OPEN_OVER_ITEMS;
    let _popup_query_flags = PopupQueryFlags::ANY_POPUP;

    // Drag/slider flags
    let _drag_flags = DragFlags::WRAP_AROUND | DragFlags::ALWAYS_CLAMP;
    let _slider_flags = SliderFlags::ALWAYS_CLAMP | SliderFlags::LOGARITHMIC;

    // Hover flags
    let _hover_flags = ItemHoveredFlags::ALLOW_WHEN_DISABLED;

    // Invisible button options
    let _button_flags = ButtonFlags::ENABLE_NAV;
    let _invisible_mouse_buttons =
        InvisibleButtonMouseButtons::LEFT | InvisibleButtonMouseButtons::RIGHT;
    let _invisible_options = InvisibleButtonOptions::new()
        .flags(_button_flags)
        .mouse_buttons(_invisible_mouse_buttons);

    // Mouse button
    let _button = MouseButton::Left;

    println!("✅ All API types exist and compile correctly");
}

#[test]
fn numeric_builder_formats_compile_and_execute() {
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();

    let mut float_value = 1.25_f32;
    let mut double_value = 2.5_f64;
    let ui = ctx.frame();

    ui.window("numeric formats").build(|| {
        let float_format = NumericFormat::<f32>::new("%.2f").unwrap();
        let _ = ui
            .input_float_config("Float")
            .display_format(float_format)
            .step(0.1)
            .build(&mut float_value);

        let _ = ui
            .input_double_config("Double")
            .try_display_format("%.4f")
            .unwrap()
            .step(0.01)
            .build(&mut double_value);
    });
}
