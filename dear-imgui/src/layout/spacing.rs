use super::validation::{assert_finite_f32, assert_finite_vec2};
use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Ui, sys};

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
}

impl Ui {
    /// Vertically align upcoming text baseline to FramePadding.y (align text to framed items).
    #[doc(alias = "AlignTextToFramePadding")]
    pub fn align_text_to_frame_padding(&self) {
        self.run_with_bound_context(|| unsafe { sys::igAlignTextToFramePadding() });
    }
}
