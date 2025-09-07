use dear_imgui::*;
use std::sync::Mutex;

// Use a global mutex to ensure only one test runs at a time
static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_utils_keyboard_functions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Test keyboard functions
    let _key_down = ui.is_key_down(Key::A);
    let _key_pressed = ui.is_key_pressed(Key::B);
    let _key_pressed_repeat = ui.is_key_pressed_with_repeat(Key::C, true);
    let _key_released = ui.is_key_released(Key::D);
    let _key_amount = ui.get_key_pressed_amount(Key::E, 0.1, 0.05);
    let _key_name = ui.get_key_name(Key::F);

    println!("Keyboard functions test completed");
}

#[test]
fn test_utils_mouse_functions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Test mouse functions
    let _mouse_down = ui.is_mouse_down(MouseButton::Left);
    let _mouse_clicked = ui.is_mouse_clicked(MouseButton::Right);
    let _mouse_clicked_repeat = ui.is_mouse_clicked_with_repeat(MouseButton::Middle, false);
    let _mouse_released = ui.is_mouse_released(MouseButton::Left);
    let _mouse_double_clicked = ui.is_mouse_double_clicked(MouseButton::Left);
    let _mouse_clicked_count = ui.get_mouse_clicked_count(MouseButton::Left);
    let _mouse_dragging = ui.is_mouse_dragging(MouseButton::Left);
    let _mouse_pos = ui.get_mouse_pos();
    let _mouse_pos_popup = ui.get_mouse_pos_on_opening_current_popup();
    let _mouse_drag_delta = ui.get_mouse_drag_delta(MouseButton::Left, 1.0);
    ui.reset_mouse_drag_delta(MouseButton::Left);
    let _mouse_wheel = ui.get_mouse_wheel();
    let _mouse_wheel_h = ui.get_mouse_wheel_h();
    let _any_mouse_down = ui.is_any_mouse_down();

    println!("Mouse functions test completed");
}

#[test]
fn test_utils_general_functions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Test general utility functions
    let time = ui.time();
    let frame_count = ui.frame_count();
    let style_color = ui.style_color(StyleColor::Text);
    let style_color_name = ui.style_color_name(StyleColor::Text);
    let rect_visible = ui.is_rect_visible([100.0, 100.0]);
    let rect_visible_ex = ui.is_rect_visible_ex([0.0, 0.0], [100.0, 100.0]);

    println!("Time: {}", time);
    println!("Frame count: {}", frame_count);
    println!("Text color: {:?}", style_color);
    println!("Text color name: {}", style_color_name);
    println!("Rect visible: {}", rect_visible);
    println!("Rect visible ex: {}", rect_visible_ex);

    // Test geometry functions
    let cursor_pos = ui.get_cursor_screen_pos();
    let content_region = ui.get_content_region_avail();
    let point_in_rect = ui.is_point_in_rect([50.0, 50.0], [0.0, 0.0], [100.0, 100.0]);
    let distance = ui.distance([0.0, 0.0], [3.0, 4.0]);
    let distance_sq = ui.distance_squared([0.0, 0.0], [3.0, 4.0]);
    let normalized = ui.normalize([3.0, 4.0]);
    let dot_product = ui.dot_product([1.0, 0.0], [0.0, 1.0]);
    let angle = ui.angle_between_vectors([1.0, 0.0], [0.0, 1.0]);
    let point_in_circle = ui.is_point_in_circle([1.0, 1.0], [0.0, 0.0], 2.0);
    let triangle_area = ui.triangle_area([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]);

    println!("Cursor pos: {:?}", cursor_pos);
    println!("Content region: {:?}", content_region);
    println!("Point in rect: {}", point_in_rect);
    println!("Distance: {}", distance);
    println!("Distance squared: {}", distance_sq);
    println!("Normalized: {:?}", normalized);
    println!("Dot product: {}", dot_product);
    println!("Angle: {}", angle);
    println!("Point in circle: {}", point_in_circle);
    println!("Triangle area: {}", triangle_area);

    assert_eq!(distance, 5.0);
    assert_eq!(distance_sq, 25.0);
    assert!(point_in_rect);
    assert!(point_in_circle);
    assert_eq!(triangle_area, 0.5);
}

#[test]
fn test_columns_basic_functions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Test basic columns functions
    ui.columns(3, "test_columns", true);

    let column_count = ui.column_count();
    let current_index = ui.current_column_index();
    let current_width = ui.current_column_width();
    let current_offset = ui.current_column_offset();

    println!("Column count: {}", column_count);
    println!("Current column index: {}", current_index);
    println!("Current column width: {}", current_width);
    println!("Current column offset: {}", current_offset);

    // Test column manipulation
    ui.set_current_column_width(150.0);
    ui.set_current_column_offset(10.0);

    // Move to next column
    ui.next_column();
    let next_index = ui.current_column_index();
    println!("Next column index: {}", next_index);

    // Test specific column functions
    let column_width = ui.column_width(0);
    let column_offset = ui.column_offset(0);
    ui.set_column_width(1, 200.0);
    ui.set_column_offset(1, 20.0);

    println!("Column 0 width: {}", column_width);
    println!("Column 0 offset: {}", column_offset);

    assert_eq!(column_count, 3);
    assert!(current_index >= 0);
    assert!(next_index >= 0);
}

#[test]
fn test_columns_advanced_functions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Test advanced columns with flags
    use dear_imgui::OldColumnFlags;

    ui.begin_columns("advanced_columns", 4, OldColumnFlags::NO_BORDER | OldColumnFlags::NO_RESIZE);

    let column_count = ui.column_count();
    let columns_id = ui.get_columns_id("advanced_columns", 4);
    let total_width = ui.get_columns_total_width();
    let is_resizing = ui.is_any_column_resizing();

    println!("Advanced column count: {}", column_count);
    println!("Columns ID: {}", columns_id);
    println!("Total width: {}", total_width);
    println!("Is resizing: {}", is_resizing);

    // Test percentage-based width functions
    let width_percentage = ui.get_column_width_percentage(0);
    println!("Column 0 width percentage: {}%", width_percentage);

    // Set equal widths
    ui.set_columns_equal_width();

    // Test column utilities
    ui.push_column_clip_rect(0);
    ui.push_columns_background();
    ui.pop_columns_background();

    ui.end_columns();

    assert_eq!(column_count, 4);
    assert!(!is_resizing); // Should not be resizing initially
}

#[test]
fn test_item_utilities() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let mut ctx = Context::create();
    ctx.io_mut().set_display_size([800.0, 600.0]);

    let ui = ctx.frame();

    // Create a button to test item utilities
    ui.button("Test Button");

    // Test item state functions
    let item_edited = ui.is_item_edited();
    let item_toggled = ui.is_item_toggled_open();
    let item_rect_min = ui.item_rect_min();
    let item_rect_max = ui.item_rect_max();

    println!("Item edited: {}", item_edited);
    println!("Item toggled: {}", item_toggled);
    println!("Item rect min: {:?}", item_rect_min);
    println!("Item rect max: {:?}", item_rect_max);

    // Test window state functions
    let window_hovered = ui.is_window_hovered();
    let window_focused = ui.is_window_focused();

    println!("Window hovered: {}", window_hovered);
    println!("Window focused: {}", window_focused);
}
