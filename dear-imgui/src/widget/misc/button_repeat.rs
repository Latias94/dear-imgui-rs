use crate::{ItemFlagStackToken, ItemFlags, Ui};

// ============================================================================
// Button repeat (convenience over item flag)
// ============================================================================

/// Tracks a button repeat item flag pushed with [`Ui::push_button_repeat`].
pub type ButtonRepeatToken<'ui> = ItemFlagStackToken<'ui>;

impl Ui {
    /// Enable/disable repeating behavior for subsequent buttons.
    ///
    /// Internally uses `PushItemFlag(ImGuiItemFlags_ButtonRepeat, repeat)`.
    #[doc(alias = "PushButtonRepeat")]
    pub fn push_button_repeat(&self, repeat: bool) -> ButtonRepeatToken<'_> {
        self.push_item_flag(ItemFlags::BUTTON_REPEAT, repeat)
    }

    /// Push a button repeat item flag, run `f`, then pop the flag.
    ///
    /// The flag is popped during unwinding if `f` panics.
    #[doc(alias = "PushButtonRepeat", alias = "PopButtonRepeat")]
    pub fn with_button_repeat<R>(&self, repeat: bool, f: impl FnOnce() -> R) -> R {
        let _repeat = self.push_button_repeat(repeat);
        f()
    }
}
