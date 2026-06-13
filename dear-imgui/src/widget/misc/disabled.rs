use crate::Ui;
use crate::sys;

// ============================================================================
// Disabled scope (RAII)
// ============================================================================

/// Tracks a disabled scope begun with [`Ui::begin_disabled`] and ended on drop.
#[must_use]
pub struct DisabledToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> DisabledToken<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        DisabledToken { _ui: ui }
    }

    /// Ends the disabled scope explicitly.
    pub fn end(self) {
        // Drop will call EndDisabled
    }
}

impl<'ui> Drop for DisabledToken<'ui> {
    fn drop(&mut self) {
        self._ui
            .run_with_bound_context(|| unsafe { sys::igEndDisabled() });
    }
}

impl Ui {
    /// Begin a disabled scope for subsequent items.
    ///
    /// All following widgets will be disabled (grayed out and non-interactive)
    /// until the returned token is dropped.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled(&self) -> DisabledToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igBeginDisabled(true) });
        DisabledToken::new(self)
    }

    /// Begin a conditionally disabled scope for subsequent items.
    ///
    /// If `disabled` is false, this still needs to be paired with the returned
    /// token being dropped to correctly balance the internal stack.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled_with_cond(&self, disabled: bool) -> DisabledToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igBeginDisabled(disabled) });
        DisabledToken::new(self)
    }
}
