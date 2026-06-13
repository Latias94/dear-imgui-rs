use crate::Ui;
use crate::sys;

impl Ui {
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
        let text = self.scratch_txt(text);
        self.run_with_bound_context(|| unsafe { sys::igSeparatorText(text) });
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
}
