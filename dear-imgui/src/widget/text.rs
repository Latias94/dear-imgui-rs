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
    /// Calculates the size required to render text with the current font and font size.
    ///
    /// This is equivalent to [`Ui::calc_text_size_with_opts`] with
    /// `hide_text_after_double_hash` set to `false` and wrapping disabled.
    #[doc(alias = "CalcTextSize")]
    pub fn calc_text_size(&self, text: impl AsRef<str>) -> [f32; 2] {
        self.calc_text_size_with_opts(text, false, -1.0)
    }

    /// Calculates the size required to render text with explicit display options.
    ///
    /// When `hide_text_after_double_hash` is `true`, the `##` label suffix is
    /// excluded from the measurement. A positive `wrap_width` enables wrapping;
    /// values at or below zero disable it.
    ///
    /// # Panics
    ///
    /// Panics if `wrap_width` is not finite.
    #[doc(alias = "CalcTextSize")]
    pub fn calc_text_size_with_opts(
        &self,
        text: impl AsRef<str>,
        hide_text_after_double_hash: bool,
        wrap_width: f32,
    ) -> [f32; 2] {
        Self::assert_finite_f32("Ui::calc_text_size_with_opts()", "wrap_width", wrap_width);
        let text = text.as_ref();

        self.run_with_bound_context(|| unsafe {
            self.calc_text_size_bound(text, hide_text_after_double_hash, wrap_width)
        })
    }

    /// Measures text while the owning ImGui context is already current.
    ///
    /// # Safety
    ///
    /// The caller must bind this `Ui`'s live ImGui context for the duration of
    /// the call.
    pub(crate) unsafe fn calc_text_size_bound(
        &self,
        text: &str,
        hide_text_after_double_hash: bool,
        wrap_width: f32,
    ) -> [f32; 2] {
        let text_range = self.scratch_txt_range(text);
        let size = unsafe {
            sys::igCalcTextSize(
                text_range.start,
                text_range.end,
                hide_text_after_double_hash,
                wrap_width,
            )
        };
        [size.x, size.y]
    }

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
        self.run_with_bound_context(|| unsafe {
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextEx(begin, end, 0); // ImGuiTextFlags_None = 0
        })
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
        self.run_with_bound_context(|| unsafe {
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextEx(begin, end, 0); // ImGuiTextFlags_None = 0
        })
    }

    /// Display text wrapped to fit the current item width
    ///
    /// This uses `PushTextWrapPos + TextUnformatted + PopTextWrapPos` to avoid
    /// calling C variadic APIs and to keep the input string unformatted.
    #[doc(alias = "TextWrapped")]
    pub fn text_wrapped(&self, text: impl AsRef<str>) {
        let s = text.as_ref();
        let _wrap = self.push_text_wrap_pos(0.0);
        self.run_with_bound_context(|| unsafe {
            let begin = s.as_ptr() as *const std::os::raw::c_char;
            let end = begin.add(s.len());
            sys::igTextUnformatted(begin, end);
        })
    }

    /// Display a label and text on the same line
    #[doc(alias = "LabelText")]
    pub fn label_text(&self, label: impl AsRef<str>, text: impl AsRef<str>) {
        let (label_ptr, text_ptr) = self.scratch_txt_two(label, text);
        self.run_with_bound_context(|| unsafe {
            // Always treat the value as unformatted user text.
            const FMT: &[u8; 3] = b"%s\0";
            sys::igLabelText(
                label_ptr,
                FMT.as_ptr() as *const std::os::raw::c_char,
                text_ptr,
            );
        })
    }

    /// Render a hyperlink-style text button. Returns true when clicked.
    #[doc(alias = "TextLink")]
    pub fn text_link(&self, label: impl AsRef<str>) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igTextLink(self.scratch_txt(label)) })
    }

    /// Render a hyperlink-style text button, and open the given URL when clicked.
    /// Returns true when clicked.
    #[doc(alias = "TextLinkOpenURL")]
    pub fn text_link_open_url(&self, label: impl AsRef<str>, url: impl AsRef<str>) -> bool {
        let (label_ptr, url_ptr) = self.scratch_txt_two(label, url);
        self.run_with_bound_context(|| unsafe { sys::igTextLinkOpenURL(label_ptr, url_ptr) })
    }
}

#[cfg(test)]
mod tests {
    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas().build();
        ctx
    }

    #[test]
    fn calc_text_size_supports_default_and_advanced_options() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        let default_size = ui.calc_text_size("Column##sort_key");
        let explicit_default = ui.calc_text_size_with_opts("Column##sort_key", false, -1.0);
        let visible_label = ui.calc_text_size_with_opts("Column##sort_key", true, -1.0);
        let trailing_hash = ui.calc_text_size_with_opts("Column#", true, -1.0);

        assert_eq!(default_size, explicit_default);
        assert!(visible_label[0] < default_size[0]);
        assert_eq!(visible_label[1], default_size[1]);
        assert_eq!(trailing_hash, ui.calc_text_size("Column#"));

        let unwrapped = ui.calc_text_size("one two three four");
        let wrapped = ui.calc_text_size_with_opts("one two three four", false, unwrapped[0] / 2.0);
        assert!(wrapped[0] < unwrapped[0]);
        assert!(wrapped[1] > unwrapped[1]);

        for wrap_width in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.calc_text_size_with_opts("text", false, wrap_width);
                }))
                .is_err()
            );
        }
    }
}
