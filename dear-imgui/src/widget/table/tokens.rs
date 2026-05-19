use crate::sys;
use crate::ui::Ui;

/// Tracks a table that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct TableToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableToken<'ui> {
    /// Creates a new table token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        TableToken { _ui: ui }
    }

    /// Ends the table
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for TableToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndTable();
        }
    }
}

/// Tracks a pushed table background draw channel.
#[must_use = "dropping the token pops the table background draw channel immediately"]
pub struct TableBackgroundChannelToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableBackgroundChannelToken<'ui> {
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Pops the table background draw channel.
    pub fn pop(self) {}

    /// Pops the table background draw channel.
    pub fn end(self) {}
}

impl Drop for TableBackgroundChannelToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igTablePopBackgroundChannel();
        }
    }
}

/// Tracks a pushed table column draw channel.
#[must_use = "dropping the token pops the table column draw channel immediately"]
pub struct TableColumnChannelToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableColumnChannelToken<'ui> {
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Pops the table column draw channel.
    pub fn pop(self) {}

    /// Pops the table column draw channel.
    pub fn end(self) {}
}

impl Drop for TableColumnChannelToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igTablePopColumnChannel();
        }
    }
}
