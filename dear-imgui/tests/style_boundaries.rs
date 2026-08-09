use dear_imgui_rs as imgui;
use imgui::internal::RawWrapper;
use std::any::Any;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);

    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn first_unknown_bit(known_bits: i32) -> i32 {
    (0..31)
        .map(|shift| 1_i32 << shift)
        .find(|candidate| known_bits & candidate == 0)
        .expect("test requires at least one spare positive flag bit")
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "non-string panic payload".to_owned(),
        },
    }
}

macro_rules! assert_panics {
    ($body:block) => {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err());
    };
}

#[test]
fn style_setters_reject_values_that_would_trip_new_frame_assertions() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let style = ctx.style_mut();

    style.set_alpha(0.5);
    style.set_window_min_size([1.0, 2.0]);
    style.set_curve_tessellation_tol(0.1);
    style.set_circle_tessellation_max_error(0.1);
    style.set_window_border_hover_padding(0.1);

    assert_eq!(style.alpha(), 0.5);
    assert_eq!(style.window_min_size(), [1.0, 2.0]);
    assert_eq!(style.curve_tessellation_tol(), 0.1);
    assert_eq!(style.circle_tessellation_max_error(), 0.1);
    assert_eq!(style.window_border_hover_padding(), 0.1);

    assert_panics!({
        style.set_alpha(1.1);
    });
    assert_eq!(style.alpha(), 0.5);

    assert_panics!({
        style.set_window_min_size([0.0, 2.0]);
    });
    assert_eq!(style.window_min_size(), [1.0, 2.0]);

    assert_panics!({
        style.set_curve_tessellation_tol(0.0);
    });
    assert_eq!(style.curve_tessellation_tol(), 0.1);

    assert_panics!({
        style.set_circle_tessellation_max_error(f32::NAN);
    });
    assert_eq!(style.circle_tessellation_max_error(), 0.1);

    assert_panics!({
        style.set_window_border_hover_padding(-0.1);
    });
    assert_eq!(style.window_border_hover_padding(), 0.1);
}

#[test]
fn style_setters_reject_invalid_direction_and_tree_line_values() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let style = ctx.style_mut();

    style.set_window_menu_button_position(imgui::Direction::None);
    style.set_window_menu_button_position(imgui::Direction::Right);
    style.set_color_button_position(imgui::Direction::Left);
    style.set_tree_lines_mode(imgui::TreeLineMode::FULL);

    assert_eq!(style.window_menu_button_position(), imgui::Direction::Right);
    assert_eq!(style.color_button_position(), imgui::Direction::Left);
    assert_eq!(style.tree_lines_mode(), imgui::TreeLineMode::FULL);

    assert_panics!({
        style.set_window_menu_button_position(imgui::Direction::Up);
    });
    assert_eq!(style.window_menu_button_position(), imgui::Direction::Right);

    assert_panics!({
        style.set_color_button_position(imgui::Direction::None);
    });
    assert_eq!(style.color_button_position(), imgui::Direction::Left);
}

#[test]
fn style_tree_lines_mode_getter_preserves_unknown_raw_bits() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let known_bits = imgui::TreeLineMode::NONE.bits()
        | imgui::TreeLineMode::FULL.bits()
        | imgui::TreeLineMode::TO_NODES.bits();
    let unknown = first_unknown_bit(known_bits);

    let style = ctx.style_mut();
    unsafe {
        style.raw_mut().TreeLinesFlags =
            (imgui::TreeLineMode::FULL.bits() | unknown) as imgui::sys::ImGuiTreeNodeFlags;
    }

    assert_eq!(
        style.tree_lines_mode().bits(),
        imgui::TreeLineMode::FULL.bits() | unknown
    );
}

#[test]
fn style_setters_reject_non_finite_or_invalid_runtime_numbers_before_storing() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let style = ctx.style_mut();

    style.set_font_scale_main(1.2);
    style.set_font_scale_dpi(1.3);
    style.set_color(imgui::StyleColor::Text, [1.0, 0.5, 0.25, 1.0]);
    style.set_table_angled_headers_angle(0.25);
    style.set_drag_drop_target_rounding(-1.0);
    style.set_drag_drop_target_border_size(2.0);
    style.set_drag_drop_target_padding(3.0);
    style.set_color_marker_size(4.0);
    style.set_tab_close_button_min_width_selected(-1.0);
    style.set_tab_close_button_min_width_unselected(100.0);
    style.set_button_text_align([0.0, 1.0]);
    style.set_mouse_cursor_scale(1.0);
    style.set_input_text_cursor_size(2.0);
    style.set_menu_item_rounding(3.0);
    style.set_selectable_rounding(4.0);
    style.set_hover_delay_short(0.0);

    assert_eq!(style.font_scale_main(), 1.2);
    assert_eq!(style.font_scale_dpi(), 1.3);
    assert_eq!(style.color(imgui::StyleColor::Text), [1.0, 0.5, 0.25, 1.0]);
    assert_eq!(style.table_angled_headers_angle(), 0.25);
    assert_eq!(style.drag_drop_target_rounding(), -1.0);
    assert_eq!(style.drag_drop_target_border_size(), 2.0);
    assert_eq!(style.drag_drop_target_padding(), 3.0);
    assert_eq!(style.color_marker_size(), 4.0);
    assert_eq!(style.tab_close_button_min_width_selected(), -1.0);
    assert_eq!(style.tab_close_button_min_width_unselected(), 100.0);
    assert_eq!(style.button_text_align(), [0.0, 1.0]);
    assert_eq!(style.mouse_cursor_scale(), 1.0);
    assert_eq!(style.input_text_cursor_size(), 2.0);
    assert_eq!(style.menu_item_rounding(), 3.0);
    assert_eq!(style.selectable_rounding(), 4.0);
    assert_eq!(style.hover_delay_short(), 0.0);

    assert_panics!({
        style.set_font_scale_main(0.0);
    });
    assert_eq!(style.font_scale_main(), 1.2);

    assert_panics!({
        style.set_font_scale_main(f32::NAN);
    });
    assert_eq!(style.font_scale_main(), 1.2);

    assert_panics!({
        style.set_font_scale_dpi(f32::INFINITY);
    });
    assert_eq!(style.font_scale_dpi(), 1.3);

    assert_panics!({
        style.set_color(imgui::StyleColor::Text, [1.0, f32::NAN, 0.25, 1.0]);
    });
    assert_eq!(style.color(imgui::StyleColor::Text), [1.0, 0.5, 0.25, 1.0]);

    assert_panics!({
        style.set_table_angled_headers_angle(1.0);
    });
    assert_eq!(style.table_angled_headers_angle(), 0.25);

    assert_panics!({
        style.set_drag_drop_target_rounding(f32::NAN);
    });
    assert_eq!(style.drag_drop_target_rounding(), -1.0);

    assert_panics!({
        style.set_drag_drop_target_border_size(-0.1);
    });
    assert_eq!(style.drag_drop_target_border_size(), 2.0);

    assert_panics!({
        style.set_drag_drop_target_padding(f32::INFINITY);
    });
    assert_eq!(style.drag_drop_target_padding(), 3.0);

    assert_panics!({
        style.set_color_marker_size(-0.1);
    });
    assert_eq!(style.color_marker_size(), 4.0);

    assert_panics!({
        style.set_tab_close_button_min_width_selected(-2.0);
    });
    assert_eq!(style.tab_close_button_min_width_selected(), -1.0);

    assert_panics!({
        style.set_tab_close_button_min_width_unselected(f32::INFINITY);
    });
    assert_eq!(style.tab_close_button_min_width_unselected(), 100.0);

    assert_panics!({
        style.set_button_text_align([1.1, 0.5]);
    });
    assert_eq!(style.button_text_align(), [0.0, 1.0]);

    assert_panics!({
        style.set_mouse_cursor_scale(0.0);
    });
    assert_eq!(style.mouse_cursor_scale(), 1.0);

    assert_panics!({
        style.set_input_text_cursor_size(0.0);
    });
    assert_eq!(style.input_text_cursor_size(), 2.0);

    assert_panics!({
        style.set_menu_item_rounding(-0.1);
    });
    assert_eq!(style.menu_item_rounding(), 3.0);

    assert_panics!({
        style.set_selectable_rounding(f32::NAN);
    });
    assert_eq!(style.selectable_rounding(), 4.0);

    assert_panics!({
        style.set_hover_delay_short(-0.1);
    });
    assert_eq!(style.hover_delay_short(), 0.0);
}

#[test]
fn style_stack_rejects_invalid_values_before_push_and_leaves_style_unchanged() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().clone() };

    let token = ui.push_style_var(imgui::StyleVar::Alpha(0.25));
    assert_eq!(unsafe { ui.style().alpha() }, 0.25);
    token.pop();
    assert_eq!(unsafe { ui.style().alpha() }, baseline.alpha());

    let token = ui.push_style_var(imgui::StyleVar::WindowMinSize([0.0, 0.0]));
    assert_eq!(unsafe { ui.style().window_min_size() }, [0.0, 0.0]);
    token.pop();
    assert_eq!(
        unsafe { ui.style().window_min_size() },
        baseline.window_min_size()
    );

    let token = ui.push_style_color(imgui::StyleColor::Text, [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        [0.0, 1.0, 0.0, 1.0]
    );
    token.pop();
    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        baseline.color(imgui::StyleColor::Text)
    );

    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::Alpha(f32::NAN));
    });
    assert_eq!(unsafe { ui.style().alpha() }, baseline.alpha());

    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::WindowPadding([-1.0, 0.0]));
    });
    assert_eq!(
        unsafe { ui.style().window_padding() },
        baseline.window_padding()
    );

    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::TableAngledHeadersAngle(1.0));
    });
    assert_eq!(
        unsafe { ui.style().table_angled_headers_angle() },
        baseline.table_angled_headers_angle()
    );

    let token = ui.push_style_var(imgui::StyleVar::DragDropTargetRounding(-1.0));
    assert_eq!(unsafe { ui.style().drag_drop_target_rounding() }, -1.0);
    token.pop();
    assert_eq!(
        unsafe { ui.style().drag_drop_target_rounding() },
        baseline.drag_drop_target_rounding()
    );

    let token = ui.push_style_var(imgui::StyleVar::MenuItemRounding(4.0));
    assert_eq!(unsafe { ui.style().menu_item_rounding() }, 4.0);
    token.pop();
    assert_eq!(
        unsafe { ui.style().menu_item_rounding() },
        baseline.menu_item_rounding()
    );

    let token = ui.push_style_var(imgui::StyleVar::SelectableRounding(5.0));
    assert_eq!(unsafe { ui.style().selectable_rounding() }, 5.0);
    token.pop();
    assert_eq!(
        unsafe { ui.style().selectable_rounding() },
        baseline.selectable_rounding()
    );

    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::MenuItemRounding(-0.1));
    });
    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::SelectableRounding(f32::NAN));
    });

    assert_panics!({
        let _ = ui.push_style_var(imgui::StyleVar::DragDropTargetRounding(f32::NAN));
    });
    assert_eq!(
        unsafe { ui.style().drag_drop_target_rounding() },
        baseline.drag_drop_target_rounding()
    );

    assert_panics!({
        let _ = ui.push_style_color(imgui::StyleColor::Text, [0.0, f32::NAN, 0.0, 1.0]);
    });
    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        baseline.color(imgui::StyleColor::Text)
    );
}

#[test]
fn tree_node_flags_include_public_upstream_draw_line_bits() {
    assert_eq!(
        imgui::StyleColor::CheckboxSelectedBg as i32,
        imgui::sys::ImGuiCol_CheckboxSelectedBg
    );

    assert_eq!(
        imgui::TreeNodeFlags::ALLOW_OVERLAP.bits(),
        imgui::sys::ImGuiTreeNodeFlags_AllowOverlap
    );
    assert_eq!(
        imgui::TreeNodeFlags::SPAN_LABEL_WIDTH.bits(),
        imgui::sys::ImGuiTreeNodeFlags_SpanLabelWidth
    );
    assert_eq!(
        imgui::TreeNodeFlags::LABEL_SPAN_ALL_COLUMNS.bits(),
        imgui::sys::ImGuiTreeNodeFlags_LabelSpanAllColumns
    );
    assert_eq!(
        imgui::TreeNodeFlags::DRAW_LINES_NONE.bits(),
        imgui::sys::ImGuiTreeNodeFlags_DrawLinesNone
    );
    assert_eq!(
        imgui::TreeNodeFlags::DRAW_LINES_FULL.bits(),
        imgui::sys::ImGuiTreeNodeFlags_DrawLinesFull
    );
    assert_eq!(
        imgui::TreeNodeFlags::DRAW_LINES_TO_NODES.bits(),
        imgui::sys::ImGuiTreeNodeFlags_DrawLinesToNodes
    );
    assert_eq!(
        imgui::TreeNodeFlags::COLLAPSING_HEADER.bits(),
        imgui::sys::ImGuiTreeNodeFlags_CollapsingHeader
    );
}

#[test]
fn ui_native_scope_tokens_reject_out_of_order_drop_before_ffi_and_recover() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().clone() };
    let _ = ui.window("native scope order").build(|| {
        let outer = ui.push_style_color(imgui::StyleColor::Text, [0.2, 0.3, 0.4, 1.0]);
        let inner = ui.push_style_color(imgui::StyleColor::Button, [0.4, 0.3, 0.2, 1.0]);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(outer)))
            .expect_err("out-of-order drop should panic");
        assert!(panic_message(panic).contains("native scope order violation"));

        let blocked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.text("must not cross FFI while recovery is pending");
        }))
        .expect_err("ordinary UI calls should be blocked during recovery");
        assert!(panic_message(blocked).contains("native scope order was violated"));

        drop(inner);
        assert_eq!(
            unsafe { ui.style().color(imgui::StyleColor::Text) },
            baseline.color(imgui::StyleColor::Text)
        );
        assert_eq!(
            unsafe { ui.style().color(imgui::StyleColor::Button) },
            baseline.color(imgui::StyleColor::Button)
        );
        ui.text("scope tracker recovered");
    });
}

#[test]
fn independent_native_stacks_can_be_ended_in_either_order() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().clone() };
    let _ = ui.window("independent native stacks").build(|| {
        let color = ui.push_style_color(imgui::StyleColor::Text, [0.2, 0.3, 0.4, 1.0]);
        let id = ui.push_id("independent-id");

        color.pop();
        assert_eq!(
            unsafe { ui.style().color(imgui::StyleColor::Text) },
            baseline.color(imgui::StyleColor::Text)
        );
        ui.text("ID stack remains active");
        id.pop();
        ui.text("both stacks recovered");
    });
}

#[test]
fn style_tokens_may_span_nested_windows_without_escaping_their_source_scope() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().color(imgui::StyleColor::Text) };
    let color = ui.push_style_color(imgui::StyleColor::Text, [0.2, 0.3, 0.4, 1.0]);

    let _ = ui.window("nested style consumer").build(|| {
        assert_eq!(
            unsafe { ui.style().color(imgui::StyleColor::Text) },
            [0.2, 0.3, 0.4, 1.0]
        );
        ui.text("the outer style remains active");
    });
    color.pop();
    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        baseline
    );
}

#[test]
fn style_tokens_cannot_be_popped_from_a_nested_window() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().color(imgui::StyleColor::Text) };
    let color = ui.push_style_color(imgui::StyleColor::Text, [0.2, 0.3, 0.4, 1.0]);

    let _ = ui.window("foreign style consumer").build(|| {
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| color.pop()))
            .expect_err("a style token must remain in its source window scope");
        assert!(panic_message(panic).contains("outside its original frame and window Begin scope"));
    });

    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        baseline
    );
    ui.text("deferred style cleanup restored the source scope");
}

#[test]
fn style_tokens_cannot_escape_the_window_that_created_them() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let baseline = unsafe { ui.style().color(imgui::StyleColor::Text) };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui
            .window("style token source window")
            .build(|| ui.push_style_color(imgui::StyleColor::Text, [0.2, 0.3, 0.4, 1.0]));
    }))
    .expect_err("ending a window with an active style token must panic before FFI");
    assert!(panic_message(panic).contains("window-local scope"));
    assert_eq!(
        unsafe { ui.style().color(imgui::StyleColor::Text) },
        baseline
    );
    ui.text("unwinding restored both the style and window scopes");
}
