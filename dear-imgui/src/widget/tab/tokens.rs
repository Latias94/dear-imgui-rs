use crate::sys;
use crate::ui::Ui;

/// Token representing an active tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBarToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TabBarToken<'ui> {
    /// Creates a new tab bar token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Ends the tab bar
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabBarToken<'ui> {
    fn drop(&mut self) {
        self._ui
            .run_with_bound_context(|| unsafe { sys::igEndTabBar() });
    }
}

/// Token representing an active tab item
#[derive(Debug)]
#[must_use]
pub struct TabItemToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TabItemToken<'ui> {
    /// Creates a new tab item token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Ends the tab item
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabItemToken<'ui> {
    fn drop(&mut self) {
        self._ui
            .run_with_bound_context(|| unsafe { sys::igEndTabItem() });
    }
}
