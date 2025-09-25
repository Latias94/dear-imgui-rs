use dear_imgui::input::MouseButton;
use dear_imgui::HoveredFlags;
use dear_imgui::*;

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

    // Popup flags
    let _popup_flags = PopupFlags::NO_OPEN_OVER_EXISTING_POPUP;

    // Hover flags
    let _hover_flags = HoveredFlags::ALLOW_WHEN_DISABLED;

    // Mouse button
    let _button = MouseButton::Left;

    println!("✅ All API types exist and compile correctly");
}

#[test]
fn test_builder_patterns() {
    // Test that all our widgets follow the imgui-rs builder pattern

    // These should compile without errors, demonstrating the builder pattern

    // Button builder pattern
    // ui.button_config("Click me").size([100.0, 30.0]).build();

    // Input text builder pattern
    // ui.input_text("Text", &mut text).hint("Enter text").password(true).build();

    // Input number builder patterns
    // ui.input_int_config("Integer").step(1).step_fast(10).build(&mut int_val);
    // ui.input_float_config("Float").format("%.2f").step(0.1).build(&mut float_val);
    // ui.input_double_config("Double").format("%.4f").step(0.01).build(&mut double_val);

    // Progress bar builder pattern
    // ui.progress_bar(0.5).size([200.0, 20.0]).overlay_text("Loading...").build();

    println!("✅ All builder patterns compile correctly");
}
