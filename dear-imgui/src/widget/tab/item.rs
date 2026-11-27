use crate::ui::Ui;

use super::{TabItemOptions, TabItemPlacement};

/// Builder for a tab item
#[derive(Debug)]
#[must_use]
pub struct TabItem<'ui, T> {
    ui: &'ui Ui,
    label: T,
    opened: Option<&'ui mut bool>,
    options: TabItemOptions,
}

impl<'ui, T: AsRef<str>> TabItem<'ui, T> {
    /// Creates a new tab item builder
    #[doc(alias = "BeginTabItem")]
    pub fn new(ui: &'ui Ui, label: T) -> Self {
        Self {
            ui,
            label,
            opened: None,
            options: TabItemOptions::new(),
        }
    }

    /// Will open or close the tab.
    ///
    /// True to display the tab. Tab item is visible by default.
    pub fn opened(mut self, opened: &'ui mut bool) -> Self {
        self.opened = Some(opened);
        self
    }

    /// Set the flags of the tab item.
    ///
    /// Flags are empty by default
    pub fn flags(mut self, flags: impl Into<TabItemOptions>) -> Self {
        self.options = flags.into();
        self
    }

    /// Set the tab placement.
    pub fn placement(mut self, placement: TabItemPlacement) -> Self {
        self.options.placement = Some(placement);
        self
    }

    /// Creates a tab item and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if the tab item is not selected
    pub fn build<R>(self, f: impl FnOnce() -> R) -> Option<R> {
        self.ui.with_bound_context(|| {
            let _token = self
                .ui
                .tab_item_with_flags(self.label, self.opened, self.options)?;
            Some(f())
        })
    }
}
