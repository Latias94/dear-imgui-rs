use crate::sys;
use crate::ui::Ui;

/// Tracks a popup that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct PopupToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> PopupToken<'ui> {
    /// Creates a new popup token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        PopupToken { _ui: ui }
    }

    /// Ends the popup
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for PopupToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndPopup();
        }
    }
}

/// Tracks a modal popup that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct ModalPopupToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> ModalPopupToken<'ui> {
    /// Creates a new modal popup token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        ModalPopupToken { _ui: ui }
    }

    /// Ends the modal popup
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ModalPopupToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndPopup();
        }
    }
}
