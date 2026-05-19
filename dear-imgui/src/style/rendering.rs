use super::Style;
use super::validation::assert_positive_f32;

impl Style {
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

    pub fn anti_aliased_lines_use_tex(&self) -> bool {
        self.inner().AntiAliasedLinesUseTex
    }
    pub fn set_anti_aliased_lines_use_tex(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLinesUseTex = v;
    }

    pub fn anti_aliased_fill(&self) -> bool {
        self.inner().AntiAliasedFill
    }
    pub fn set_anti_aliased_fill(&mut self, v: bool) {
        self.inner_mut().AntiAliasedFill = v;
    }

    pub fn curve_tessellation_tol(&self) -> f32 {
        self.inner().CurveTessellationTol
    }
    pub fn set_curve_tessellation_tol(&mut self, v: f32) {
        assert_positive_f32("Style::set_curve_tessellation_tol()", "v", v);
        self.inner_mut().CurveTessellationTol = v;
    }

    pub fn circle_tessellation_max_error(&self) -> f32 {
        self.inner().CircleTessellationMaxError
    }
    pub fn set_circle_tessellation_max_error(&mut self, v: f32) {
        assert_positive_f32("Style::set_circle_tessellation_max_error()", "v", v);
        self.inner_mut().CircleTessellationMaxError = v;
    }
}
