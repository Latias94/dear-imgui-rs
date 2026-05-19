use crate::io::{BoundContextGuard, Io, assert_positive_f32};
use crate::sys;

impl Io {
    /// Get the global font scale (not available in current Dear ImGui version)
    /// Compatibility shim: maps to style.FontScaleMain (Dear ImGui 1.92+)
    pub fn font_global_scale(&self) -> f32 {
        unsafe {
            let _guard = BoundContextGuard::bind(self.context_ptr("Io::font_global_scale()"));
            let style = sys::igGetStyle();
            assert!(
                !style.is_null(),
                "Io::font_global_scale() requires a valid ImGui context"
            );
            (*style).FontScaleMain
        }
    }

    /// Set the global font scale (not available in current Dear ImGui version)
    /// Compatibility shim: maps to style.FontScaleMain (Dear ImGui 1.92+)
    pub fn set_font_global_scale(&mut self, scale: f32) {
        assert_positive_f32("Io::set_font_global_scale()", "scale", scale);
        unsafe {
            let _guard = BoundContextGuard::bind(self.context_ptr("Io::set_font_global_scale()"));
            let style = sys::igGetStyle();
            assert!(
                !style.is_null(),
                "Io::set_font_global_scale() requires a valid ImGui context"
            );
            (*style).FontScaleMain = scale;
        }
    }
}
