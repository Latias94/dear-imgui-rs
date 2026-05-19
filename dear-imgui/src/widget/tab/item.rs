use crate::ui::Ui;

use super::{TabItemOptions, TabItemPlacement, TabItemToken};

/// Builder for a tab item
#[derive(Debug)]
#[must_use]
pub struct TabItem<'a, T> {
    label: T,
    opened: Option<&'a mut bool>,
    options: TabItemOptions,
}

impl<'a, T: AsRef<str>> TabItem<'a, T> {
    /// Creates a new tab item builder
    #[doc(alias = "BeginTabItem")]
    pub fn new(label: T) -> Self {
        Self {
            label,
            opened: None,
            options: TabItemOptions::new(),
        }
    }

    /// Will open or close the tab.
    ///
    /// True to display the tab. Tab item is visible by default.
    pub fn opened(mut self, opened: &'a mut bool) -> Self {
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

    /// Begins the tab item and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabItemToken<'_>> {
        ui.tab_item_with_flags(self.label, self.opened, self.options)
    }

    /// Creates a tab item and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if the tab item is not selected
    pub fn build<R, F: FnOnce() -> R>(self, ui: &Ui, f: F) -> Option<R> {
        self.begin(ui).map(|_tab| f())
    }
}
