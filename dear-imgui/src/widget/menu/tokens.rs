use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::ui::Ui;

/// Tracks a main menu bar that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested menu or window-like scope and in its originating
/// window `Begin` scope. Prefer [`crate::Ui::main_menu_bar`] for ordinary use.
#[must_use]
#[doc(alias = "EndMainMenuBar")]
pub struct MainMenuBarToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> MainMenuBarToken<'ui> {
    /// Creates a new main menu bar token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndMainMenuBar, "MainMenuBarToken"),
        }
    }

    /// Ends the main menu bar
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested scope is active or this token is outside its originating
    /// window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MainMenuBarToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Tracks a menu bar that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested menu and in its originating window `Begin` scope.
/// Prefer [`crate::Ui::menu_bar`] for ordinary use.
#[must_use]
#[doc(alias = "EndMenuBar")]
pub struct MenuBarToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> MenuBarToken<'ui> {
    /// Creates a new menu bar token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndMenuBar, "MenuBarToken"),
        }
    }

    /// Ends the menu bar
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested menu-bar scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MenuBarToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Tracks a menu that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::menu`] for ordinary use.
#[must_use]
#[doc(alias = "EndMenu")]
pub struct MenuToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> MenuToken<'ui> {
    /// Creates a new menu token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndMenu, "MenuToken"),
        }
    }

    /// Ends the menu
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MenuToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
