use crate::sys;
use crate::ui::Ui;

/// Tracks a combo box that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct ComboBoxToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> ComboBoxToken<'ui> {
    /// Creates a new combo box token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        ComboBoxToken { _ui: ui }
    }

    /// Ends the combo box
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for ComboBoxToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndCombo();
        }
    }
}
