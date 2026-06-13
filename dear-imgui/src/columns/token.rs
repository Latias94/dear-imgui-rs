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

/// Token representing a pushed legacy columns background draw channel.
#[must_use]
pub struct ColumnsBackgroundToken<'ui> {
    pub(super) ui: &'ui Ui,
}

impl Drop for ColumnsBackgroundToken<'_> {
    fn drop(&mut self) {
        self.ui.pop_columns_background();
    }
}
