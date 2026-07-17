use super::*;

impl Ui {
    /// Renders a demo window (previously called a test window), which demonstrates most
    /// Dear ImGui features.
    ///
    /// # Safety
    ///
    /// The upstream demo can open font-atlas debug panels whose destructive controls mutate or
    /// delete fonts during the frame and may continue using invalidated native pointers. The
    /// caller must ensure those panels and controls cannot be activated.
    #[doc(alias = "ShowDemoWindow")]
    pub unsafe fn show_demo_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowDemoWindow(opened);
        });
    }

    /// Renders an about window.
    ///
    /// Displays the Dear ImGui version/credits, and build/system information.
    #[doc(alias = "ShowAboutWindow")]
    pub fn show_about_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowAboutWindow(opened);
        });
    }

    /// Renders a metrics/debug window.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    ///
    /// # Safety
    ///
    /// The upstream metrics window exposes font-atlas controls that can mutate or delete fonts
    /// during the frame and may continue using invalidated native pointers. The caller must ensure
    /// the Fonts section and its destructive controls cannot be activated.
    #[doc(alias = "ShowMetricsWindow")]
    pub unsafe fn show_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowMetricsWindow(opened);
        });
    }

    /// Renders a basic help/info block (not a window)
    #[doc(alias = "ShowUserGuide")]
    pub fn show_user_guide(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowUserGuide();
        });
    }

    // ============================================================================
    // Additional Demo, Debug, Information (non-duplicate methods)
    // ============================================================================

    /// Renders a debug log window.
    ///
    /// Displays a simplified log of important dear imgui events.
    #[doc(alias = "ShowDebugLogWindow")]
    pub fn show_debug_log_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowDebugLogWindow(opened);
        });
    }

    /// Renders an ID stack tool window.
    ///
    /// Hover items with mouse to query information about the source of their unique ID.
    #[doc(alias = "ShowIDStackToolWindow")]
    pub fn show_id_stack_tool_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowIDStackToolWindow(opened);
        });
    }

    /// Renders a table that breaks `text` down into UTF-8 bytes and codepoints.
    ///
    /// This is intended for diagnosing text encoding and missing-glyph issues.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains an interior NUL byte, which the upstream
    /// NUL-terminated API cannot represent.
    #[doc(alias = "DebugTextEncoding")]
    pub fn debug_text_encoding(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        assert!(
            !text.contains('\0'),
            "Ui::debug_text_encoding() text must not contain interior NUL bytes"
        );
        let text = self.scratch_txt(text);
        self.run_with_bound_context(|| unsafe { sys::igDebugTextEncoding(text) });
    }

    /// Temporarily flashes a style color in Dear ImGui's debug tools.
    #[doc(alias = "DebugFlashStyleColor")]
    pub fn debug_flash_style_color(&self, color: crate::StyleColor) {
        self.run_with_bound_context(|| unsafe { sys::igDebugFlashStyleColor(color as i32) });
    }

    /// Starts Dear ImGui's interactive item picker debug tool.
    #[doc(alias = "DebugStartItemPicker")]
    pub fn debug_start_item_picker(&self) {
        self.run_with_bound_context(|| unsafe { sys::igDebugStartItemPicker() });
    }

    /// Returns the Dear ImGui version string
    #[doc(alias = "GetVersion")]
    pub fn get_version(&self) -> &str {
        self.run_with_bound_context(|| unsafe {
            let version_ptr = sys::igGetVersion();
            if version_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn font_atlas_debug_windows_are_explicitly_unsafe() {
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_demo_window;
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_metrics_window;
    }

    #[test]
    fn public_debug_helpers_are_safe_to_call_in_a_frame() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas().build();
        let ui = ctx.frame();

        ui.window("debug_helpers").build(|| {
            let cursor_y = ui.cursor_pos_y();
            ui.debug_text_encoding("A UTF-8 string: 界");
            assert!(ui.cursor_pos_y() > cursor_y);

            ui.debug_flash_style_color(crate::StyleColor::Text);
            ui.debug_start_item_picker();
        });

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.debug_text_encoding("A\0B");
            }))
            .is_err()
        );
    }
}
