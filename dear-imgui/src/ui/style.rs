use super::*;

impl Ui {
    /// Renders a style editor block (not a window) for the given `Style` structure
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_style_editor(&self, style: &mut crate::style::Style) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(style.raw_mut());
        });
    }

    /// Renders a style editor block (not a window) for the currently active style
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_default_style_editor(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(std::ptr::null_mut());
        });
    }

    // ============================================================================
    // Style Access
    // ============================================================================

    /// Returns a shared reference to the current [`Style`].
    ///
    /// ## Safety
    ///
    /// This function is tagged as `unsafe` because pushing via
    /// [`push_style_color`](crate::Ui::push_style_color) or
    /// [`push_style_var`](crate::Ui::push_style_var) or popping via
    /// [`ColorStackToken::pop`](crate::ColorStackToken::pop) or
    /// [`StyleStackToken::pop`](crate::StyleStackToken::pop) will modify the values in the returned
    /// shared reference. Therefore, you should not retain this reference across calls to push and
    /// pop. The [`clone_style`](Ui::clone_style) version may instead be used to avoid `unsafe`.
    #[doc(alias = "GetStyle")]
    pub unsafe fn style(&self) -> &crate::Style {
        self.run_with_bound_context(|| unsafe {
            // safe because Style is a transparent wrapper around sys::ImGuiStyle
            &*(sys::igGetStyle() as *const crate::Style)
        })
    }

    /// Returns a copy of the current style.
    ///
    /// This is a safe alternative to [`style`](Self::style) that avoids the lifetime issues.
    #[doc(alias = "GetStyle")]
    pub fn clone_style(&self) -> crate::Style {
        unsafe { self.style().clone() }
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

    /// Write the Dark style values into the provided [`Style`] object.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsDark(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Light style values into the provided [`Style`] object.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsLight(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Classic style values into the provided [`Style`] object.
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
        self.run_with_bound_context(|| unsafe { sys::igShowStyleSelector(self.scratch_txt(label)) })
    }

    /// Renders a font selector combo box.
    #[doc(alias = "ShowFontSelector")]
    pub fn show_font_selector(&self, label: impl AsRef<str>) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowFontSelector(self.scratch_txt(label));
        });
    }
}
