use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Token representing an active tab bar.
///
/// Tab-bar and tab-item tokens from the same window must finish in reverse creation order and in
/// their originating window `Begin` scope. Prefer [`crate::TabBar::build`] for ordinary use.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndTabBar")]
pub struct TabBarToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TabBarToken<'ui> {
    /// Creates a new tab bar token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTabBar, "TabBarToken"),
        }
    }

    /// Ends the tab bar
    ///
    /// # Panics
    ///
    /// Panics before FFI if a tab item or later tab-bar token is active, or if this token is
    /// outside its originating window `Begin` scope.
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabBarToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Token representing an active tab item.
///
/// Tab-bar and tab-item tokens from the same window must finish in reverse creation order and in
/// their originating window `Begin` scope. Prefer [`crate::TabItem::build`] for ordinary use.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndTabItem")]
pub struct TabItemToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TabItemToken<'ui> {
    /// Creates a new tab item token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTabItem, "TabItemToken"),
        }
    }

    /// Ends the tab item
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later tab token is active or this token is outside its originating
    /// window `Begin` scope.
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabItemToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
