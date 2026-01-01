//! Text helpers
//!
//! Convenience functions for colored text, wrapped text, disabled text and
//! label helpers.
//!
//! Quick examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.text("normal");
//! ui.text_colored([1.0, 0.5, 0.0, 1.0], "warning");
//! ui.text_disabled("disabled");
//! ui.text_wrapped("very long text that will wrap when needed...");
//! ```
//!
use crate::Ui;
use crate::style::StyleColor;
use crate::sys;

impl Ui {
    /// Display colored text
    ///
    /// This implementation uses zero-copy optimization with `igTextEx`,
    /// avoiding string allocation and null-termination overhead.
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.text_colored([1.0, 0.0, 0.0, 1.0], "Red text");
    /// ui.text_colored([0.0, 1.0, 0.0, 1.0], "Green text");
    /// ```
    #[doc(alias = "TextColored")]
    pub fn text_colored(&self, color: [f32; 4], text: impl AsRef<str>) {
        let s = text.as_ref();

        // Temporarily set the text color
        let _token = self.push_style_color(StyleColor::Text, color);

        // Use igTextEx with zero-copy (begin/end pointers)
        unsafe {
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextEx(begin, end, 0); // ImGuiTextFlags_None = 0
        }
    }

    /// Display disabled (grayed out) text
    ///
    /// This implementation uses zero-copy optimization with `igTextEx`,
    /// avoiding string allocation and null-termination overhead.
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.text_disabled("This option is not available");
    /// ```
    #[doc(alias = "TextDisabled")]
    pub fn text_disabled(&self, text: impl AsRef<str>) {
        let s = text.as_ref();

        // Get the disabled color from the current style
        let disabled_color = self.style_color(StyleColor::TextDisabled);

        // Temporarily set the text color to disabled color
        let _token = self.push_style_color(StyleColor::Text, disabled_color);

        // Use igTextEx with zero-copy (begin/end pointers)
        unsafe {
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextEx(begin, end, 0); // ImGuiTextFlags_None = 0
        }
    }

    /// Display text wrapped to fit the current item width
    ///
    /// This uses `PushTextWrapPos + TextUnformatted + PopTextWrapPos` to avoid
    /// calling C variadic APIs and to keep the input string unformatted.
    #[doc(alias = "TextWrapped")]
    pub fn text_wrapped(&self, text: impl AsRef<str>) {
        let s = text.as_ref();
        unsafe {
            sys::igPushTextWrapPos(0.0);
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextUnformatted(begin, end);
            sys::igPopTextWrapPos();
        }
    }

    /// Display a label and text on the same line
    #[doc(alias = "LabelText")]
    pub fn label_text(&self, label: impl AsRef<str>, text: impl AsRef<str>) {
        let (label_ptr, text_ptr) = self.scratch_txt_two(label, text);
        unsafe {
            // Always treat the value as unformatted user text.
            const FMT: &[u8; 3] = b"%s\0";
            sys::igLabelText(
                label_ptr,
                FMT.as_ptr() as *const std::os::raw::c_char,
                text_ptr,
            );
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
