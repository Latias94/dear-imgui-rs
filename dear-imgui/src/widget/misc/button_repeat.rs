use crate::{Ui, sys};

// ============================================================================
// Button repeat (convenience over item flag)
// ============================================================================

create_token!(
    /// Tracks a button repeat item flag pushed with [`Ui::push_button_repeat`].
    pub struct ButtonRepeatToken<'ui>;

    /// Pops the button repeat item flag.
    #[doc(alias = "PopButtonRepeat")]
    drop { unsafe { sys::igPopItemFlag() } }
);

impl ButtonRepeatToken<'_> {
    /// Pops the button repeat item flag.
    pub fn pop(self) {
        self.end()
    }
}

impl Ui {
    /// Enable/disable repeating behavior for subsequent buttons.
    ///
    /// Internally uses `PushItemFlag(ImGuiItemFlags_ButtonRepeat, repeat)`.
    #[doc(alias = "PushButtonRepeat")]
    pub fn push_button_repeat(&self, repeat: bool) -> ButtonRepeatToken<'_> {
        self.run_with_bound_context(|| unsafe {
            sys::igPushItemFlag(sys::ImGuiItemFlags_ButtonRepeat as i32, repeat)
        });
        ButtonRepeatToken::new(self)
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
