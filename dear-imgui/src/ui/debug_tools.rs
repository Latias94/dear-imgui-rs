use super::*;

impl Ui {
    /// Renders Dear ImGui's demo window without its destructive font-atlas debug controls.
    ///
    /// This preserves the ordinary demo, Metrics/Debugger, and Style Editor controls. Only the
    /// panels backed by upstream `ShowFontAtlas()` are omitted, so the safe API does not bypass
    /// Rust's font-atlas lifetime and generation tracking.
    ///
    /// Use [`show_upstream_demo_window`](Self::show_upstream_demo_window) to opt into the exact
    /// upstream window, including its font-atlas controls.
    #[doc(alias = "ShowDemoWindow")]
    pub fn show_demo_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_demo_window_without_font_atlas(opened);
        });
    }

    /// Renders the exact upstream Dear ImGui demo window, including font-atlas debug controls.
    ///
    /// Prefer [`show_demo_window`](Self::show_demo_window) unless the application deliberately
    /// owns the full font-atlas mutation contract.
    ///
    /// # Safety
    ///
    /// With `BackendFlags::RENDERER_HAS_TEXTURES`, the upstream Fonts panel can delete an
    /// `ImFont` and continue reading it in the same native call. Other destructive controls also
    /// bypass Rust's atlas-generation tracking. The caller must prevent those controls from being
    /// activated or otherwise uphold the native font-atlas contract.
    pub unsafe fn show_upstream_demo_window(&self, opened: &mut bool) {
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

    /// Renders a metrics/debug window without its destructive font-atlas tree.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    #[doc(alias = "ShowMetricsWindow")]
    pub fn show_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_metrics_window_without_font_atlas(opened);
        });
    }

    /// Renders the exact upstream metrics/debug window, including its font-atlas tree.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tree can mutate or destroy font-atlas data while Rust font handles and
    /// renderer state are live. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowMetricsWindow(opened);
        });
    }

    /// Renders upstream's internal Font Atlas debug panel for this context.
    ///
    /// This is the isolated font-specific part omitted from the safe demo, metrics, and style
    /// editor APIs.
    ///
    /// # Safety
    ///
    /// The panel exposes destructive atlas operations and may continue using native font pointers
    /// after a control mutates the atlas. The caller must uphold the native font-atlas contract.
    #[doc(alias = "ShowFontAtlas")]
    pub unsafe fn show_font_atlas_debug_panel(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_font_atlas_debug_panel();
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
    fn safe_debug_windows_keep_font_atlas_controls_explicit() {
        let _: fn(&crate::Ui, &mut bool) = crate::Ui::show_demo_window;
        let _: fn(&crate::Ui, &mut bool) = crate::Ui::show_metrics_window;
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_upstream_demo_window;
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_upstream_metrics_window;
        let _: unsafe fn(&crate::Ui) = crate::Ui::show_font_atlas_debug_panel;
    }

    #[test]
    fn public_debug_helpers_are_safe_to_call_in_a_frame() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        let ui = ctx.frame();

        let mut demo_open = true;
        ui.show_demo_window(&mut demo_open);
        let mut metrics_open = true;
        ui.show_metrics_window(&mut metrics_open);

        ui.window("debug_helpers").build(|| {
            ui.show_default_style_editor();
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
