//! Layout and cursor helpers
//!
//! Spacing, separators, horizontal layout (`same_line`), grouping, cursor
//! positioning and clipping helpers. These functions help arrange widgets and
//! content within windows.
//!
//! Example:
//! ```no_run
//! # use dear_imgui::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.text("Left");
//! ui.same_line();
//! ui.text("Right");
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::Ui;
use crate::sys;

create_token!(
    /// Tracks a layout group that can be ended with `end` or by dropping.
    pub struct GroupToken<'ui>;

    /// Drops the layout group manually. You can also just allow this token
    /// to drop on its own.
    drop { unsafe { sys::igEndGroup() } }
);

create_token!(
    /// Tracks a pushed clip rect that will be popped on drop.
    pub struct ClipRectToken<'ui>;

    /// Pops a clip rect pushed with [`Ui::push_clip_rect`].
    drop { unsafe { sys::igPopClipRect() } }
);

/// # Cursor / Layout
impl Ui {
    /// Renders a separator (generally horizontal).
    ///
    /// This becomes a vertical separator inside a menu bar or in horizontal layout mode.
    #[doc(alias = "Separator")]
    pub fn separator(&self) {
        unsafe { sys::igSeparator() }
    }

    /// Renders a separator with text.
    #[doc(alias = "SeparatorText")]
    pub fn separator_with_text(&self, text: impl AsRef<str>) {
        unsafe { sys::igSeparatorText(self.scratch_txt(text)) }
    }

    /// Creates a vertical separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_vertical(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Vertical as i32, 1.0) }
    }

    /// Creates a horizontal separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_horizontal(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Horizontal as i32, 1.0) }
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    ///
    /// This is equivalent to calling [same_line_with_pos](Self::same_line_with_pos)
    /// with the `pos` set to 0.0, which uses `Style::item_spacing`.
    #[doc(alias = "SameLine")]
    pub fn same_line(&self) {
        self.same_line_with_pos(0.0);
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    ///
    /// This is equivalent to calling [same_line_with_spacing](Self::same_line_with_spacing)
    /// with the `spacing` set to -1.0, which means no extra spacing.
    #[doc(alias = "SameLine")]
    pub fn same_line_with_pos(&self, pos_x: f32) {
        self.same_line_with_spacing(pos_x, -1.0)
    }

    /// Call between widgets or groups to layout them horizontally.
    ///
    /// X position is given in window coordinates.
    #[doc(alias = "SameLine")]
    pub fn same_line_with_spacing(&self, pos_x: f32, spacing_w: f32) {
        unsafe { sys::igSameLine(pos_x, spacing_w) }
    }

    /// Undo a `same_line` call or force a new line when in horizontal layout mode
    #[doc(alias = "NewLine")]
    pub fn new_line(&self) {
        unsafe { sys::igNewLine() }
    }

    /// Adds vertical spacing
    #[doc(alias = "Spacing")]
    pub fn spacing(&self) {
        unsafe { sys::igSpacing() }
    }

    /// Fills a space of `size` in pixels with nothing on the current window.
    ///
    /// Can be used to move the cursor on the window.
    #[doc(alias = "Dummy")]
    pub fn dummy(&self, size: impl Into<[f32; 2]>) {
        let size_vec: sys::ImVec2 = size.into().into();
        unsafe { sys::igDummy(size_vec) }
    }

    /// Moves content position to the right by `Style::indent_spacing`
    ///
    /// This is equivalent to [indent_by](Self::indent_by) with `width` set to
    /// `Style::indent_spacing`.
    #[doc(alias = "Indent")]
    pub fn indent(&self) {
        self.indent_by(0.0)
    }

    /// Moves content position to the right by `width`
    #[doc(alias = "Indent")]
    pub fn indent_by(&self, width: f32) {
        unsafe { sys::igIndent(width) };
    }

    /// Moves content position to the left by `Style::indent_spacing`
    ///
    /// This is equivalent to [unindent_by](Self::unindent_by) with `width` set to
    /// `Style::indent_spacing`.
    #[doc(alias = "Unindent")]
    pub fn unindent(&self) {
        self.unindent_by(0.0)
    }

    /// Moves content position to the left by `width`
    #[doc(alias = "Unindent")]
    pub fn unindent_by(&self, width: f32) {
        unsafe { sys::igUnindent(width) };
    }

    /// Creates a layout group and starts appending to it.
    ///
    /// Returns a `GroupToken` that must be ended by calling `.end()`.
    #[doc(alias = "BeginGroup")]
    pub fn begin_group(&self) -> GroupToken<'_> {
        unsafe { sys::igBeginGroup() };
        GroupToken::new(self)
    }

    /// Creates a layout group and runs a closure to construct the contents.
    ///
    /// May be useful to handle the same mouse event on a group of items, for example.
    #[doc(alias = "BeginGroup")]
    pub fn group<R, F: FnOnce() -> R>(&self, f: F) -> R {
        let group = self.begin_group();
        let result = f();
        group.end();
        result
    }

    /// Returns the cursor position (in window coordinates)
    #[doc(alias = "GetCursorPos")]
    pub fn cursor_pos(&self) -> [f32; 2] {
        unsafe {
            let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetCursorPos(&mut pos);
            [pos.x, pos.y]
        }
    }

    /// Returns the cursor position (in absolute screen coordinates)
    #[doc(alias = "GetCursorScreenPos")]
    pub fn cursor_screen_pos(&self) -> [f32; 2] {
        unsafe {
            let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetCursorScreenPos(&mut pos);
            [pos.x, pos.y]
        }
    }

    /// Sets the cursor position (in window coordinates)
    #[doc(alias = "SetCursorPos")]
    pub fn set_cursor_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorPos(pos_vec) };
    }

    /// Sets the cursor position (in absolute screen coordinates)
    #[doc(alias = "SetCursorScreenPos")]
    pub fn set_cursor_screen_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        unsafe { sys::igSetCursorScreenPos(pos_vec) };
    }

    /// Returns the X cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosX")]
    pub fn cursor_pos_x(&self) -> f32 {
        unsafe { sys::igGetCursorPosX() }
    }

    /// Returns the Y cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosY")]
    pub fn cursor_pos_y(&self) -> f32 {
        unsafe { sys::igGetCursorPosY() }
    }

    /// Sets the X cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosX")]
    pub fn set_cursor_pos_x(&self, x: f32) {
        unsafe { sys::igSetCursorPosX(x) };
    }

    /// Sets the Y cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosY")]
    pub fn set_cursor_pos_y(&self, y: f32) {
        unsafe { sys::igSetCursorPosY(y) };
    }

    /// Returns the initial cursor position (in window coordinates)
    #[doc(alias = "GetCursorStartPos")]
    pub fn cursor_start_pos(&self) -> [f32; 2] {
        unsafe {
            let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
            sys::igGetCursorStartPos(&mut pos);
            [pos.x, pos.y]
        }
    }
}

// ============================================================================
// Metrics helpers & clip rect stack
// ============================================================================

impl Ui {
    /// Return ~ FontSize.
    #[doc(alias = "GetTextLineHeight")]
    pub fn text_line_height(&self) -> f32 {
        unsafe { sys::igGetTextLineHeight() }
    }

    /// Return ~ FontSize + style.ItemSpacing.y.
    #[doc(alias = "GetTextLineHeightWithSpacing")]
    pub fn text_line_height_with_spacing(&self) -> f32 {
        unsafe { sys::igGetTextLineHeightWithSpacing() }
    }

    /// Return ~ FontSize + style.FramePadding.y * 2.
    #[doc(alias = "GetFrameHeight")]
    pub fn frame_height(&self) -> f32 {
        unsafe { sys::igGetFrameHeight() }
    }

    /// Return ~ FontSize + style.FramePadding.y * 2 + style.ItemSpacing.y.
    #[doc(alias = "GetFrameHeightWithSpacing")]
    pub fn frame_height_with_spacing(&self) -> f32 {
        unsafe { sys::igGetFrameHeightWithSpacing() }
    }

    /// Push a clipping rectangle in screen space.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
    ) {
        let min = min.into();
        let max = max.into();
        let min_v = sys::ImVec2 {
            x: min[0],
            y: min[1],
        };
        let max_v = sys::ImVec2 {
            x: max[0],
            y: max[1],
        };
        unsafe { sys::igPushClipRect(min_v, max_v, intersect_with_current) }
    }

    /// Pop a clipping rectangle from the stack.
    #[doc(alias = "PopClipRect")]
    pub fn pop_clip_rect(&self) {
        unsafe { sys::igPopClipRect() }
    }

    /// Run a closure with a clip rect pushed and automatically popped.
    pub fn with_clip_rect<R>(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
        f: impl FnOnce() -> R,
    ) -> R {
        self.push_clip_rect(min, max, intersect_with_current);
        let _t = ClipRectToken::new(self);
        f()
    }

    /// Returns true if the specified rectangle (min,max) is visible (not clipped).
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_min_max(
        &self,
        rect_min: impl Into<[f32; 2]>,
        rect_max: impl Into<[f32; 2]>,
    ) -> bool {
        let mn = rect_min.into();
        let mx = rect_max.into();
        let mn_v = sys::ImVec2 { x: mn[0], y: mn[1] };
        let mx_v = sys::ImVec2 { x: mx[0], y: mx[1] };
        unsafe { sys::igIsRectVisible_Vec2(mn_v, mx_v) }
    }

    /// Returns true if a rectangle of given size at the current cursor pos is visible.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_with_size(&self, size: impl Into<[f32; 2]>) -> bool {
        let s = size.into();
        let v = sys::ImVec2 { x: s[0], y: s[1] };
        unsafe { sys::igIsRectVisible_Nil(v) }
    }

    /// Vertically align upcoming text baseline to FramePadding.y (align text to framed items).
    #[doc(alias = "AlignTextToFramePadding")]
    pub fn align_text_to_frame_padding(&self) {
        unsafe { sys::igAlignTextToFramePadding() }
    }
}
