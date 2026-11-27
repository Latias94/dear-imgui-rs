use super::*;

impl Ui {
    /// Renders a style editor block (not a window) for the given `Style` structure.
    ///
    /// The safe editor retains all upstream style controls except its destructive Fonts tab.
    /// Font selection and font-scale controls remain available because they do not mutate the
    /// atlas topology.
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_style_editor(&self, style: &mut crate::style::Style) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_style_editor_without_font_atlas(style.raw_mut());
        });
    }

    /// Renders a style editor block (not a window) for the currently active style.
    ///
    /// The safe editor retains all upstream style controls except its destructive Fonts tab.
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_default_style_editor(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_style_editor_without_font_atlas(std::ptr::null_mut());
        });
    }

    /// Renders the exact upstream style editor for the given `Style` structure.
    ///
    /// Prefer [`show_style_editor`](Self::show_style_editor) unless the application deliberately
    /// owns the full font-atlas mutation contract.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tab exposes destructive font-atlas operations that bypass Rust's atlas
    /// generation tracking. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_style_editor(&self, style: &mut crate::style::Style) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(style.raw_mut());
        });
    }

    /// Renders the exact upstream style editor for the active style.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tab exposes destructive font-atlas operations that bypass Rust's atlas
    /// generation tracking. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_default_style_editor(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(std::ptr::null_mut());
        });
    }

    // ============================================================================
    // Style Access
    // ============================================================================

    /// Borrows the live [`crate::Style`] of this UI's Context.
    ///
    /// Accessors reflect subsequent style changes, including stack pushes and pops.
    /// Use [`Self::clone_style`] for an independent snapshot.
    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> &crate::Style {
        self.run_with_bound_context(|| unsafe { &*sys::igGetStyle().cast::<crate::Style>() })
    }

    /// Returns an independent copy of the current style.
    #[doc(alias = "GetStyle")]
    pub fn clone_style(&self) -> crate::Style {
        self.style().clone()
    }

    /// Apply the built-in Dark style to the current style.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsDark(std::ptr::null_mut()) });
    }

    /// Apply the built-in Light style to the current style.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsLight(std::ptr::null_mut()) });
    }

    /// Apply the built-in Classic style to the current style.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsClassic(std::ptr::null_mut()) });
    }

    /// Write the Dark style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsDark(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Light style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsLight(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Classic style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsClassic(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Renders a style selector combo box.
    ///
    /// Returns true when a different style was selected.
    #[doc(alias = "ShowStyleSelector")]
    pub fn show_style_selector(&self, label: impl AsRef<str>) -> bool {
        self.with_scratch_txt(label, |label| unsafe { sys::igShowStyleSelector(label) })
    }

    /// Renders a font selector combo box.
    #[doc(alias = "ShowFontSelector")]
    pub fn show_font_selector(&self, label: impl AsRef<str>) {
        self.with_scratch_txt(label, |label| unsafe {
            sys::igShowFontSelector(label);
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn safe_style_editors_keep_font_atlas_controls_explicit() {
        let _: fn(&crate::Ui, &mut crate::Style) = crate::Ui::show_style_editor;
        let _: fn(&crate::Ui) = crate::Ui::show_default_style_editor;
        let _: unsafe fn(&crate::Ui, &mut crate::Style) = crate::Ui::show_upstream_style_editor;
        let _: unsafe fn(&crate::Ui) = crate::Ui::show_upstream_default_style_editor;
    }
}
