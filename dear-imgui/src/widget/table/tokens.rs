use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Tracks a table that can be ended by calling `.end()` or by dropping
#[must_use]
#[doc(alias = "EndTable")]
pub struct TableToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TableToken<'ui> {
    /// Creates a new table token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTable, "TableToken"),
        }
    }

    /// Ends the table.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested table or any native scope created inside this table is still
    /// active, or if this table is no longer the current native table instance. Create scopes
    /// outside the table when they intentionally need to span the table lifetime.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for TableToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

pub(super) struct TableChannelGuard<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TableChannelGuard<'ui> {
    pub(super) fn background(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::PopTableBackgroundChannel,
                "table background channel",
            ),
        }
    }

    pub(super) fn column(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::PopTableColumnChannel,
                "table column channel",
            ),
        }
    }
}

impl Drop for TableChannelGuard<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
