use crate::Ui;
use crate::sys;

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
            sys::igTextColored(color_vec, text_ptr);
        }
    }

    /// Display disabled (grayed out) text
    #[doc(alias = "TextDisabled")]
    pub fn text_disabled(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::igTextDisabled(text_ptr);
        }
    }

    /// Display text wrapped to fit the current item width
    #[doc(alias = "TextWrapped")]
    pub fn text_wrapped(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            sys::igTextWrapped(text_ptr);
        }
    }

    /// Display a label and text on the same line
    #[doc(alias = "LabelText")]
    pub fn label_text(&self, label: impl AsRef<str>, text: impl AsRef<str>) {
        let (label_ptr, text_ptr) = self.scratch_txt_two(label, text);
        unsafe {
            sys::igLabelText(label_ptr, text_ptr);
        }
    }

    /// Render a hyperlink-style text button. Returns true when clicked.
    #[doc(alias = "TextLink")]
    pub fn text_link(&self, label: impl AsRef<str>) -> bool {
        unsafe { sys::igTextLink(self.scratch_txt(label)) }
    }

    /// Render a hyperlink-style text button, and open the given URL when clicked.
    /// Returns true when clicked.
    #[doc(alias = "TextLinkOpenURL")]
    pub fn text_link_open_url(&self, label: impl AsRef<str>, url: impl AsRef<str>) -> bool {
        let (label_ptr, url_ptr) = self.scratch_txt_two(label, url);
        unsafe { sys::igTextLinkOpenURL(label_ptr, url_ptr) }
    }
}
