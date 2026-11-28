//! Layout and cursor helpers
//!
//! Spacing, separators, horizontal layout (`same_line`), grouping, cursor
//! positioning and clipping helpers. These functions help arrange widgets and
//! content within windows.
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
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
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::validation::{assert_finite_f32, assert_finite_vec2};
use crate::{Ui, sys};

#[cfg(feature = "stack-layout")]
mod stack_layout;

#[cfg(feature = "stack-layout")]
pub use stack_layout::{
    HorizontalStackLayoutToken, StackLayoutId, StackLayoutSuspensionToken, VerticalStackLayoutToken,
};

/// Tracks a pushed clip rect that will be popped on drop.
///
/// Tokens for the same draw list must be ended in LIFO order. Prefer
/// [`Ui::with_clip_rect`] for ordinary scoped use.
#[must_use]
pub struct ClipRectToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl ClipRectToken<'_> {
    /// Pops a clip rect pushed with [`Ui::push_clip_rect`].
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later UI clip token for the same draw list is active or the token is
    /// no longer in its originating window `Begin` scope.
    pub fn end(self) {}
}

impl Drop for ClipRectToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

create_token!(
    /// Tracks a layout group that can be ended with `end` or by dropping.
    #[doc(alias = "EndGroup")]
    pub struct GroupToken<'ui>;

    pop crate::scope::NativeScopePop::EndGroup;

    /// Drops the layout group manually. You can also just allow this token
    /// to drop on its own.
    drop { unsafe { sys::igEndGroup() } }
);

/// Tracks an indentation scope started with [`Ui::begin_indent`] or
/// [`Ui::begin_indent_by`].
///
/// Tokens from the same window may be ended in any order because each token removes its own
/// captured width. Cross-window cleanup is deferred until the originating window becomes current
/// again. [`Ui::with_indent`] and [`Ui::with_indent_by`] remain the canonical APIs for ordinary
/// scoped use.
#[must_use]
pub struct IndentToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl IndentToken<'_> {
    /// Ends the indentation scope explicitly.
    pub fn end(self) {}
}

impl Drop for IndentToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

impl Ui {
    /// Push a clipping rectangle in screen space.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
    ) -> ClipRectToken<'_> {
        let min = min.into();
        let max = max.into();
        assert_finite_vec2("Ui::push_clip_rect()", "min", min);
        assert_finite_vec2("Ui::push_clip_rect()", "max", max);
        let min_v = sys::ImVec2 {
            x: min[0],
            y: min[1],
        };
        let max_v = sys::ImVec2 {
            x: max[0],
            y: max[1],
        };
        let draw_list = self.run_with_bound_context(|| unsafe {
            sys::igPushClipRect(min_v, max_v, intersect_with_current);
            sys::igGetWindowDrawList()
        });
        assert!(
            !draw_list.is_null(),
            "Ui::push_clip_rect() requires a current window draw list"
        );
        ClipRectToken {
            scope: self
                .begin_native_scope(NativeScopePop::PopUiClipRect(draw_list), "ClipRectToken"),
        }
    }

    /// Run a closure with a clip rect pushed and automatically popped.
    pub fn with_clip_rect<R>(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
        f: impl FnOnce() -> R,
    ) -> R {
        let token = self.push_clip_rect(min, max, intersect_with_current);
        let result = f();
        drop(token);
        result
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
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_min", mn);
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_max", mx);
        let mn_v = sys::ImVec2 { x: mn[0], y: mn[1] };
        let mx_v = sys::ImVec2 { x: mx[0], y: mx[1] };
        self.run_with_bound_context(|| unsafe { sys::igIsRectVisible_Vec2(mn_v, mx_v) })
    }

    /// Returns true if a rectangle of given size at the current cursor pos is visible.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_with_size(&self, size: impl Into<[f32; 2]>) -> bool {
        let s = size.into();
        assert_finite_vec2("Ui::is_rect_visible_with_size()", "size", s);
        let v = sys::ImVec2 { x: s[0], y: s[1] };
        self.run_with_bound_context(|| unsafe { sys::igIsRectVisible_Nil(v) })
    }

    /// Creates a layout group and starts appending to it.
    ///
    /// Returns a `GroupToken` that must be ended by calling `.end()`.
    #[doc(alias = "BeginGroup")]
    pub fn begin_group(&self) -> GroupToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igBeginGroup() });
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
        assert_finite_f32("Ui::same_line_with_spacing()", "pos_x", pos_x);
        assert_finite_f32("Ui::same_line_with_spacing()", "spacing_w", spacing_w);
        self.run_with_bound_context(|| unsafe { sys::igSameLine(pos_x, spacing_w) });
    }

    /// Undo a `same_line` call or force a new line when in horizontal layout mode
    #[doc(alias = "NewLine")]
    pub fn new_line(&self) {
        self.run_with_bound_context(|| unsafe { sys::igNewLine() });
    }

    /// Adds vertical spacing
    #[doc(alias = "Spacing")]
    pub fn spacing(&self) {
        self.run_with_bound_context(|| unsafe { sys::igSpacing() });
    }

    /// Fills a space of `size` in pixels with nothing on the current window.
    ///
    /// Can be used to move the cursor on the window.
    #[doc(alias = "Dummy")]
    pub fn dummy(&self, size: impl Into<[f32; 2]>) {
        let size = size.into();
        assert_finite_vec2("Ui::dummy()", "size", size);
        let size_vec: sys::ImVec2 = size.into();
        self.run_with_bound_context(|| unsafe { sys::igDummy(size_vec) });
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
        assert_finite_f32("Ui::indent_by()", "width", width);
        self.run_with_bound_context(|| unsafe { sys::igIndent(width) });
    }

    /// Starts an indentation scope using [`Style::indent_spacing`](crate::Style::indent_spacing).
    ///
    /// The returned token restores the exact width captured at creation and may be dropped in any
    /// order relative to other indentation tokens from the same window.
    #[doc(alias = "Indent")]
    pub fn begin_indent(&self) -> IndentToken<'_> {
        self.begin_indent_by(0.0)
    }

    /// Starts an indentation scope with a custom width.
    ///
    /// Passing `0.0` snapshots the current [`Style::indent_spacing`](crate::Style::indent_spacing)
    /// so a later style change cannot alter restoration.
    #[doc(alias = "Indent")]
    pub fn begin_indent_by(&self, width: f32) -> IndentToken<'_> {
        assert_finite_f32("Ui::begin_indent_by()", "width", width);
        let width = self.run_with_bound_context(|| unsafe {
            let width = if width == 0.0 {
                (*sys::igGetStyle()).IndentSpacing
            } else {
                width
            };
            sys::igIndent(width);
            width
        });
        IndentToken {
            scope: self
                .begin_provenance_native_scope(NativeScopePop::Unindent(width), "IndentToken"),
        }
    }

    /// Runs `f` in an indentation scope using [`Style::indent_spacing`](crate::Style::indent_spacing).
    ///
    /// The indentation is restored if `f` returns early or panics. Prefer this closure-based
    /// scope over manually pairing [`Self::indent`] and [`Self::unindent`].
    #[doc(alias = "Indent", alias = "Unindent")]
    pub fn with_indent<R>(&self, f: impl FnOnce() -> R) -> R {
        let indent = self.begin_indent();
        let result = f();
        drop(indent);
        result
    }

    /// Runs `f` in an indentation scope with a custom width.
    ///
    /// The indentation is restored if `f` returns early or panics.
    #[doc(alias = "Indent", alias = "Unindent")]
    pub fn with_indent_by<R>(&self, width: f32, f: impl FnOnce() -> R) -> R {
        let indent = self.begin_indent_by(width);
        let result = f();
        drop(indent);
        result
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
        assert_finite_f32("Ui::unindent_by()", "width", width);
        self.run_with_bound_context(|| unsafe { sys::igUnindent(width) });
    }

    /// Vertically align upcoming text baseline to FramePadding.y (align text to framed items).
    #[doc(alias = "AlignTextToFramePadding")]
    pub fn align_text_to_frame_padding(&self) {
        self.run_with_bound_context(|| unsafe { sys::igAlignTextToFramePadding() });
    }

    /// Renders a separator (generally horizontal).
    ///
    /// This becomes a vertical separator inside a menu bar or in horizontal layout mode.
    #[doc(alias = "Separator")]
    pub fn separator(&self) {
        self.run_with_bound_context(|| unsafe { sys::igSeparator() });
    }

    /// Renders a separator with text.
    #[doc(alias = "SeparatorText")]
    pub fn separator_with_text(&self, text: impl AsRef<str>) {
        self.with_scratch_txt(text, |text| unsafe { sys::igSeparatorText(text) });
    }

    /// Creates a vertical separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_vertical(&self) {
        self.run_with_bound_context(|| unsafe {
            sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Vertical as i32, 1.0)
        });
    }

    /// Creates a horizontal separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_horizontal(&self) {
        self.run_with_bound_context(|| unsafe {
            sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Horizontal as i32, 1.0)
        });
    }

    /// Returns the cursor position (in window coordinates)
    #[doc(alias = "GetCursorPos")]
    pub fn cursor_pos(&self) -> [f32; 2] {
        let pos = self.run_with_bound_context(|| unsafe { sys::igGetCursorPos() });
        [pos.x, pos.y]
    }

    /// Returns the cursor position (in absolute screen coordinates)
    #[doc(alias = "GetCursorScreenPos")]
    pub fn cursor_screen_pos(&self) -> [f32; 2] {
        let pos = self.run_with_bound_context(|| unsafe { sys::igGetCursorScreenPos() });
        [pos.x, pos.y]
    }

    /// Sets the cursor position (in window coordinates)
    #[doc(alias = "SetCursorPos")]
    pub fn set_cursor_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        self.run_with_bound_context(|| unsafe { sys::igSetCursorPos(pos_vec) });
    }

    /// Sets the cursor position (in absolute screen coordinates)
    #[doc(alias = "SetCursorScreenPos")]
    pub fn set_cursor_screen_pos(&self, pos: impl Into<[f32; 2]>) {
        let pos_array = pos.into();
        assert_finite_vec2("Ui::set_cursor_screen_pos()", "position", pos_array);
        let pos_vec = sys::ImVec2 {
            x: pos_array[0],
            y: pos_array[1],
        };
        self.run_with_bound_context(|| unsafe { sys::igSetCursorScreenPos(pos_vec) });
    }

    /// Returns the X cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosX")]
    pub fn cursor_pos_x(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetCursorPosX() })
    }

    /// Returns the Y cursor position (in window coordinates)
    #[doc(alias = "GetCursorPosY")]
    pub fn cursor_pos_y(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetCursorPosY() })
    }

    /// Sets the X cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosX")]
    pub fn set_cursor_pos_x(&self, x: f32) {
        assert_finite_f32("Ui::set_cursor_pos_x()", "x", x);
        self.run_with_bound_context(|| unsafe { sys::igSetCursorPosX(x) });
    }

    /// Sets the Y cursor position (in window coordinates)
    #[doc(alias = "SetCursorPosY")]
    pub fn set_cursor_pos_y(&self, y: f32) {
        assert_finite_f32("Ui::set_cursor_pos_y()", "y", y);
        self.run_with_bound_context(|| unsafe { sys::igSetCursorPosY(y) });
    }

    /// Returns the initial cursor position (in window coordinates)
    #[doc(alias = "GetCursorStartPos")]
    pub fn cursor_start_pos(&self) -> [f32; 2] {
        let pos = self.run_with_bound_context(|| unsafe { sys::igGetCursorStartPos() });
        [pos.x, pos.y]
    }

    /// Return ~ FontSize.
    #[doc(alias = "GetTextLineHeight")]
    pub fn text_line_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetTextLineHeight() })
    }

    /// Return ~ FontSize + style.ItemSpacing.y.
    #[doc(alias = "GetTextLineHeightWithSpacing")]
    pub fn text_line_height_with_spacing(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetTextLineHeightWithSpacing() })
    }

    /// Return ~ FontSize + style.FramePadding.y * 2.
    #[doc(alias = "GetFrameHeight")]
    pub fn frame_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetFrameHeight() })
    }

    /// Return ~ FontSize + style.FramePadding.y * 2 + style.ItemSpacing.y.
    #[doc(alias = "GetFrameHeightWithSpacing")]
    pub fn frame_height_with_spacing(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetFrameHeightWithSpacing() })
    }
}
