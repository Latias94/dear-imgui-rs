use crate::ui::Ui;

use super::{TabBarFittingPolicy, TabBarFlags, TabBarOptions, TabBarToken};

/// Builder for a tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBar<T> {
    id: T,
    options: TabBarOptions,
}

impl<T: AsRef<str>> TabBar<T> {
    /// Creates a new tab bar builder
    #[doc(alias = "BeginTabBar")]
    pub fn new(id: T) -> Self {
        Self {
            id,
            options: TabBarOptions::new(),
        }
    }

    /// Enable/Disable the reorderable property
    ///
    /// Disabled by default
    pub fn reorderable(mut self, value: bool) -> Self {
        if value {
            self.options.flags |= TabBarFlags::REORDERABLE;
        } else {
            self.options.flags &= !TabBarFlags::REORDERABLE;
        }
        self
    }

    /// Set the flags of the tab bar
    ///
    /// Flags are empty by default
    pub fn flags(mut self, flags: impl Into<TabBarOptions>) -> Self {
        self.options = flags.into();
        self
    }

    /// Set the tab fitting policy.
    pub fn fitting_policy(mut self, policy: TabBarFittingPolicy) -> Self {
        self.options.fitting_policy = Some(policy);
        self
    }

    /// Begins the tab bar and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabBarToken<'_>> {
        ui.tab_bar_with_flags(self.id, self.options)
    }

    /// Creates a tab bar and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if no tabbar content is visible
    pub fn build<R, F: FnOnce() -> R>(self, ui: &Ui, f: F) -> Option<R> {
        self.begin(ui).map(|_tab| f())
    }
}
