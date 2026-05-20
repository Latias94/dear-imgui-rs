use crate::Ui;

/// Token representing an active columns layout.
#[must_use]
pub struct ColumnsToken<'ui> {
    pub(super) ui: &'ui Ui,
}

impl Drop for ColumnsToken<'_> {
    fn drop(&mut self) {
        self.ui.end_columns();
    }
}
