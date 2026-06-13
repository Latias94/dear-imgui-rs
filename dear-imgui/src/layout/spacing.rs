use super::validation::{assert_finite_f32, assert_finite_vec2};
use crate::Ui;
use crate::sys;

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
