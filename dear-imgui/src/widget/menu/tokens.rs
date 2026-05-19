use crate::sys;
use crate::ui::Ui;

/// Tracks a main menu bar that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MainMenuBarToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> MainMenuBarToken<'ui> {
    /// Creates a new main menu bar token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        MainMenuBarToken { _ui: ui }
    }

    /// Ends the main menu bar
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MainMenuBarToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMainMenuBar();
        }
    }
}

/// Tracks a menu bar that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MenuBarToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> MenuBarToken<'ui> {
    /// Creates a new menu bar token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        MenuBarToken { _ui: ui }
    }

    /// Ends the menu bar
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MenuBarToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMenuBar();
        }
    }
}

/// Tracks a menu that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MenuToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> MenuToken<'ui> {
    /// Creates a new menu token
    pub(super) fn new(ui: &'ui Ui) -> Self {
        MenuToken { _ui: ui }
    }

    /// Ends the menu
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for MenuToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMenu();
        }
    }
}
