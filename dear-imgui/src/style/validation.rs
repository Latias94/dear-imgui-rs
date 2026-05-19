use super::{Direction, StyleVar};
use crate::widget::TreeNodeFlags;

const TABLE_ANGLED_HEADERS_MAX_ANGLE: f32 = 50.0 * std::f32::consts::PI / 180.0;

pub(super) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(crate) fn validate_style_color(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

pub(super) fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

pub(super) fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(super) fn assert_non_negative_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

pub(super) fn assert_unit_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(
        (0.0..=1.0).contains(&value),
        "{caller} {name} must be between 0.0 and 1.0"
    );
}

pub(super) fn assert_unit_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        (0.0..=1.0).contains(&value[0]) && (0.0..=1.0).contains(&value[1]),
        "{caller} {name} must contain values between 0.0 and 1.0"
    );
}

pub(super) fn assert_window_min_size(caller: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, "value", value);
    assert!(
        value[0] >= 1.0 && value[1] >= 1.0,
        "{caller} value must contain values greater than or equal to 1.0"
    );
}

pub(super) fn assert_tab_close_button_min_width(caller: &str, value: f32) {
    assert_finite_f32(caller, "value", value);
    assert!(
        value >= 0.0 || value == -1.0,
        "{caller} value must be non-negative, or -1.0 to always show the close button"
    );
}

pub(super) fn assert_table_angled_headers_angle(caller: &str, value: f32) {
    assert_finite_f32(caller, "value", value);
    assert!(
        (-TABLE_ANGLED_HEADERS_MAX_ANGLE..=TABLE_ANGLED_HEADERS_MAX_ANGLE).contains(&value),
        "{caller} value must be between -50 and 50 degrees in radians"
    );
}

pub(super) fn validate_window_menu_button_position(caller: &str, direction: Direction) {
    assert!(
        matches!(
            direction,
            Direction::None | Direction::Left | Direction::Right
        ),
        "{caller} accepts only Direction::None, Direction::Left, or Direction::Right"
    );
}

pub(super) fn validate_color_button_position(caller: &str, direction: Direction) {
    assert!(
        matches!(direction, Direction::Left | Direction::Right),
        "{caller} accepts only Direction::Left or Direction::Right"
    );
}

pub(super) fn validate_tree_lines_flags(caller: &str, flags: TreeNodeFlags) {
    assert!(
        matches!(
            flags,
            TreeNodeFlags::DRAW_LINES_NONE
                | TreeNodeFlags::DRAW_LINES_FULL
                | TreeNodeFlags::DRAW_LINES_TO_NODES
        ),
        "{caller} accepts only TreeNodeFlags::DRAW_LINES_NONE, DRAW_LINES_FULL, or DRAW_LINES_TO_NODES"
    );
}

pub(crate) fn validate_style_var(caller: &str, style_var: StyleVar) {
    use StyleVar::*;

    match style_var {
        Alpha(value) => assert_unit_f32(caller, "Alpha", value),
        DisabledAlpha(value) => assert_unit_f32(caller, "DisabledAlpha", value),
        WindowPadding(value) => assert_non_negative_vec2(caller, "WindowPadding", value),
        WindowRounding(value) => assert_non_negative_f32(caller, "WindowRounding", value),
        WindowBorderSize(value) => assert_non_negative_f32(caller, "WindowBorderSize", value),
        WindowMinSize(value) => assert_non_negative_vec2(caller, "WindowMinSize", value),
        WindowTitleAlign(value) => assert_unit_vec2(caller, "WindowTitleAlign", value),
        ChildRounding(value) => assert_non_negative_f32(caller, "ChildRounding", value),
        ChildBorderSize(value) => assert_non_negative_f32(caller, "ChildBorderSize", value),
        PopupRounding(value) => assert_non_negative_f32(caller, "PopupRounding", value),
        PopupBorderSize(value) => assert_non_negative_f32(caller, "PopupBorderSize", value),
        FramePadding(value) => assert_non_negative_vec2(caller, "FramePadding", value),
        FrameRounding(value) => assert_non_negative_f32(caller, "FrameRounding", value),
        ImageRounding(value) => assert_non_negative_f32(caller, "ImageRounding", value),
        ImageBorderSize(value) => assert_non_negative_f32(caller, "ImageBorderSize", value),
        FrameBorderSize(value) => assert_non_negative_f32(caller, "FrameBorderSize", value),
        ItemSpacing(value) => assert_non_negative_vec2(caller, "ItemSpacing", value),
        ItemInnerSpacing(value) => assert_non_negative_vec2(caller, "ItemInnerSpacing", value),
        IndentSpacing(value) => assert_non_negative_f32(caller, "IndentSpacing", value),
        CellPadding(value) => assert_non_negative_vec2(caller, "CellPadding", value),
        ScrollbarSize(value) => assert_non_negative_f32(caller, "ScrollbarSize", value),
        ScrollbarRounding(value) => assert_non_negative_f32(caller, "ScrollbarRounding", value),
        ScrollbarPadding(value) => assert_non_negative_f32(caller, "ScrollbarPadding", value),
        GrabMinSize(value) => assert_non_negative_f32(caller, "GrabMinSize", value),
        GrabRounding(value) => assert_non_negative_f32(caller, "GrabRounding", value),
        TabRounding(value) => assert_non_negative_f32(caller, "TabRounding", value),
        TabBorderSize(value) => assert_non_negative_f32(caller, "TabBorderSize", value),
        TabMinWidthBase(value) => assert_non_negative_f32(caller, "TabMinWidthBase", value),
        TabMinWidthShrink(value) => assert_non_negative_f32(caller, "TabMinWidthShrink", value),
        TabBarBorderSize(value) => assert_non_negative_f32(caller, "TabBarBorderSize", value),
        TabBarOverlineSize(value) => assert_non_negative_f32(caller, "TabBarOverlineSize", value),
        TableAngledHeadersAngle(value) => assert_table_angled_headers_angle(caller, value),
        TableAngledHeadersTextAlign(value) => {
            assert_unit_vec2(caller, "TableAngledHeadersTextAlign", value);
        }
        TreeLinesSize(value) => assert_non_negative_f32(caller, "TreeLinesSize", value),
        TreeLinesRounding(value) => assert_non_negative_f32(caller, "TreeLinesRounding", value),
        DragDropTargetRounding(value) => assert_finite_f32(caller, "DragDropTargetRounding", value),
        ButtonTextAlign(value) => assert_unit_vec2(caller, "ButtonTextAlign", value),
        SelectableTextAlign(value) => assert_unit_vec2(caller, "SelectableTextAlign", value),
        SeparatorSize(value) => assert_non_negative_f32(caller, "SeparatorSize", value),
        SeparatorTextBorderSize(value) => {
            assert_non_negative_f32(caller, "SeparatorTextBorderSize", value);
        }
        SeparatorTextAlign(value) => assert_unit_vec2(caller, "SeparatorTextAlign", value),
        SeparatorTextPadding(value) => {
            assert_non_negative_vec2(caller, "SeparatorTextPadding", value);
        }
        DockingSeparatorSize(value) => {
            assert_non_negative_f32(caller, "DockingSeparatorSize", value);
        }
    }
}
