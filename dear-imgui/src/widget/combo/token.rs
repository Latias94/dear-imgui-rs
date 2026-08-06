use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Tracks a combo box that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in the exact window `Begin`
/// scope that created it. Prefer the combo closure builder for ordinary use.
#[must_use]
#[doc(alias = "EndCombo")]
pub struct ComboBoxToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> ComboBoxToken<'ui> {
    /// Creates a new combo box token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndCombo, "ComboBoxToken"),
        }
    }

    /// Ends the combo box
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is no longer in its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for ComboBoxToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
