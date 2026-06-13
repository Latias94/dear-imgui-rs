use crate::fonts::FontId;
use crate::{Ui, sys};

/// # Parameter stacks (shared)
impl Ui {
    /// Switches to the given font by pushing it to the font stack.
    ///
    /// Returns a `FontStackToken` that must be popped by calling `.pop()`
    ///
    /// # Panics
    ///
    /// Panics before calling Dear ImGui if the `FontId` came from a different atlas,
    /// was invalidated by font atlas mutation, or is no longer present in the
    /// current context's atlas.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let font_data_sources = [];
    /// // At initialization time
    /// let my_custom_font = ctx.fonts().add_font(&font_data_sources);
    /// # let ui = ctx.frame();
    /// // During UI construction
    /// let font = ui.push_font(my_custom_font);
    /// ui.text("I use the custom font!");
    /// font.pop();
    /// ```
    #[doc(alias = "PushFont")]
    pub fn push_font(&self, id: FontId) -> FontStackToken<'_> {
        self.run_with_bound_context(|| unsafe {
            let font_ptr =
                crate::fonts::validate_font_id_for_current_context(id, "Ui::push_font()");
            sys::igPushFont(font_ptr, 0.0);
        });
        FontStackToken::new(self)
    }
}

create_token!(
    /// Tracks a font pushed to the font stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct FontStackToken<'ui>;

    /// Pops a change from the font stack.
    drop { unsafe { sys::igPopFont() } }
);

impl FontStackToken<'_> {
    /// Pops a change from the font stack.
    pub fn pop(self) {
        self.end()
    }
}
