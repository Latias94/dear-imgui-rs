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
    /// Returns an `ItemWidthStackToken`. The pushed width item is popped when either
    /// `ItemWidthStackToken` goes out of scope, or `.end()` is called.
    #[doc(alias = "PushItemWidth")]
    pub fn push_item_width_text(&self, text: impl AsRef<str>) -> ItemWidthStackToken<'_> {
        let text_width = unsafe {
            let text_ptr = self.scratch_txt(text);
            let out = sys::igCalcTextSize(text_ptr, std::ptr::null(), false, -1.0);
            out.x
        };
        self.push_item_width(text_width)
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
    pub struct ItemWidthStackToken<'ui>;

    /// Pops an item width change made with [`Ui::push_item_width`].
    #[doc(alias = "PopItemWidth")]
    drop { unsafe { sys::igPopItemWidth() } }
);

create_token!(
    /// Tracks a change made with [`Ui::push_text_wrap_pos`] that can be popped
    /// by calling [`TextWrapPosStackToken::end`] or dropping.
    pub struct TextWrapPosStackToken<'ui>;

    /// Pops a text wrap position change made with [`Ui::push_text_wrap_pos`].
    #[doc(alias = "PopTextWrapPos")]
    drop { unsafe { sys::igPopTextWrapPos() } }
);
