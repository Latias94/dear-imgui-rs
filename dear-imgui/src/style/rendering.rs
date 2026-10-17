use super::Style;
use super::validation::assert_positive_f32;

impl Style {
    /// Returns the input-text cursor thickness.
    #[doc(alias = "InputTextCursorSize")]
    pub fn input_text_cursor_size(&self) -> f32 {
        self.inner().InputTextCursorSize
    }

    /// Sets the input-text cursor thickness.
    #[doc(alias = "InputTextCursorSize")]
    pub fn set_input_text_cursor_size(&mut self, value: f32) {
        assert_positive_f32("Style::set_input_text_cursor_size()", "value", value);
        self.inner_mut().InputTextCursorSize = value;
    }

    pub fn mouse_cursor_scale(&self) -> f32 {
        self.inner().MouseCursorScale
    }
    pub fn set_mouse_cursor_scale(&mut self, v: f32) {
        assert_positive_f32("Style::set_mouse_cursor_scale()", "v", v);
        self.inner_mut().MouseCursorScale = v;
    }

    pub fn anti_aliased_lines(&self) -> bool {
        self.inner().AntiAliasedLines
    }
    pub fn set_anti_aliased_lines(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLines = v;
    }

    /// Returns whether open line ends are anti-aliased.
    #[doc(alias = "AntiAliasedLineEnds")]
    pub fn anti_aliased_line_ends(&self) -> bool {
        self.inner().AntiAliasedLineEnds
    }
    /// Sets anti-aliasing for open line ends.
    #[doc(alias = "AntiAliasedLineEnds")]
    pub fn set_anti_aliased_line_ends(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLineEnds = v;
    }

    pub fn anti_aliased_fill(&self) -> bool {
        self.inner().AntiAliasedFill
    }
    pub fn set_anti_aliased_fill(&mut self, v: bool) {
        self.inner_mut().AntiAliasedFill = v;
    }

    /// Returns the maximum curve tessellation error in pixels.
    #[doc(alias = "CurveTessellationMaxError")]
    pub fn curve_tessellation_max_error(&self) -> f32 {
        self.inner().CurveTessellationMaxError
    }
    /// Sets the maximum curve tessellation error in pixels.
    ///
    /// To migrate from the former squared-pixel tolerance, pass its square root.
    #[doc(alias = "CurveTessellationMaxError")]
    pub fn set_curve_tessellation_max_error(&mut self, v: f32) {
        assert_positive_f32("Style::set_curve_tessellation_max_error()", "v", v);
        self.inner_mut().CurveTessellationMaxError = v;
    }

    pub fn circle_tessellation_max_error(&self) -> f32 {
        self.inner().CircleTessellationMaxError
    }
    pub fn set_circle_tessellation_max_error(&mut self, v: f32) {
        assert_positive_f32("Style::set_circle_tessellation_max_error()", "v", v);
        self.inner_mut().CircleTessellationMaxError = v;
    }
}
