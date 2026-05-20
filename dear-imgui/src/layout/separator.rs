use crate::Ui;
use crate::sys;

impl Ui {
    /// Renders a separator (generally horizontal).
    ///
    /// This becomes a vertical separator inside a menu bar or in horizontal layout mode.
    #[doc(alias = "Separator")]
    pub fn separator(&self) {
        unsafe { sys::igSeparator() }
    }

    /// Renders a separator with text.
    #[doc(alias = "SeparatorText")]
    pub fn separator_with_text(&self, text: impl AsRef<str>) {
        unsafe { sys::igSeparatorText(self.scratch_txt(text)) }
    }

    /// Creates a vertical separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_vertical(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Vertical as i32, 1.0) }
    }

    /// Creates a horizontal separator
    #[doc(alias = "SeparatorEx")]
    pub fn separator_horizontal(&self) {
        unsafe { sys::igSeparatorEx(sys::ImGuiSeparatorFlags_Horizontal as i32, 1.0) }
    }
}
