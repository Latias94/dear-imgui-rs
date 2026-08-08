use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Tracks a popup that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::popup`] for ordinary use.
#[must_use]
#[doc(alias = "EndPopup")]
pub struct PopupToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> PopupToken<'ui> {
    /// Creates a new popup token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndPopup, "PopupToken"),
        }
    }

    /// Ends the popup
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for PopupToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Tracks a modal popup that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::modal_popup`] for ordinary use.
#[must_use]
pub struct ModalPopupToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> ModalPopupToken<'ui> {
    /// Creates a new modal popup token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndPopup, "ModalPopupToken"),
        }
    }

    /// Ends the modal popup
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ModalPopupToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
