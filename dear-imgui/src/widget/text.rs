use crate::sys;
use crate::Ui;

impl Ui {
    /// Display colored text
    #[doc(alias = "TextColored")]
    pub fn text_colored(&self, color: [f32; 4], text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            let color_vec = sys::ImVec4 {
                x: color[0],
                y: color[1],
                z: color[2],
                w: color[3],
            };
            sys::ImGui_TextColored(&color_vec, text_ptr);
        }
    }

    /// Display disabled (grayed out) text
    #[doc(alias = "TextDisabled")]
    pub fn text_disabled(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::ImGui_TextDisabled(text_ptr);
        }
    }

    /// Display text wrapped to fit the current item width
    #[doc(alias = "TextWrapped")]
    pub fn text_wrapped(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::ImGui_TextWrapped(text_ptr);
        }
    }

    /// Display a label and text on the same line
    #[doc(alias = "LabelText")]
    pub fn label_text(&self, label: impl AsRef<str>, text: impl AsRef<str>) {
        let (label_ptr, text_ptr) = self.scratch_txt_two(label, text);
        unsafe {
            sys::ImGui_LabelText(label_ptr, text_ptr);
        }
    }
}
