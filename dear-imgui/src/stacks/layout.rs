use crate::{Ui, sys};

/// # Parameter stacks (current window)
impl Ui {
    /// Changes the item width by pushing a change to the item width stack.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    ///
    /// - `> 0.0`: width is `item_width` pixels
    /// - `= 0.0`: default to ~2/3 of window width
    /// - `< 0.0`: `item_width` pixels relative to the right of window (-1.0 always aligns width to
    ///   the right side)
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width(&self, item_width: f32) -> ItemWidthStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushItemWidth(item_width) });
        ItemWidthStackToken::new(self)
    }

    /// Sets the width of the next item(s) to be the same as the width of the given text.
    ///
    /// Text is measured with [`Ui::calc_text_size`] using its default options.
    ///
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width_text(&self, text: impl AsRef<str>) -> ItemWidthStackToken<'_> {
        let text = text.as_ref();
        self.run_with_bound_context(|| unsafe {
            let text_width = self.calc_text_size_bound(text, false, -1.0)[0];
            sys::igPushItemWidth(text_width);
        });
        ItemWidthStackToken::new(self)
    }

    /// Sets the position where text will wrap around.
    ///
    /// Returns a `TextWrapPosStackToken`. The pushed wrap position is popped when either
    /// `TextWrapPosStackToken` goes out of scope, or `.end()` is called.
    ///
    /// - `wrap_pos_x < 0.0`: no wrapping
    /// - `wrap_pos_x = 0.0`: wrap to end of window (or column)
    /// - `wrap_pos_x > 0.0`: wrap at `wrap_pos_x` position in window local space
    #[doc(alias = "PushTextWrapPos")]
    pub fn push_text_wrap_pos(&self, wrap_pos_x: f32) -> TextWrapPosStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushTextWrapPos(wrap_pos_x) });
        TextWrapPosStackToken::new(self)
    }
}

create_token!(
    /// Tracks a change made with [`Ui::push_item_width`] that can be popped
    /// by calling [`ItemWidthStackToken::end`] or dropping.
    #[doc(alias = "PopItemWidth")]
    pub struct ItemWidthStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopItemWidth;

    /// Pops an item width change made with [`Ui::push_item_width`].
    #[doc(alias = "PopItemWidth")]
    drop { unsafe { sys::igPopItemWidth() } }
);

create_token!(
    /// Tracks a change made with [`Ui::push_text_wrap_pos`] that can be popped
    /// by calling [`TextWrapPosStackToken::end`] or dropping.
    #[doc(alias = "PopTextWrapPos")]
    pub struct TextWrapPosStackToken<'ui>;

    pop crate::scope::NativeScopePop::PopTextWrap;

    /// Pops a text wrap position change made with [`Ui::push_text_wrap_pos`].
    #[doc(alias = "PopTextWrapPos")]
    drop { unsafe { sys::igPopTextWrapPos() } }
);

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
    fn text_item_width_scope_matches_measurement_and_restores_previous_width() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("text_item_width").build(|| {
            let original_width = ui.calc_item_width();
            let text_width = ui.calc_text_size("measured width")[0];

            {
                let _width = ui.push_item_width_text("measured width");
                assert_eq!(ui.calc_item_width(), text_width);
            }

            assert_eq!(ui.calc_item_width(), original_width);
        });
    }
}
