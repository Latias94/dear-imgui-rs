//! Menus and menu bars
//!
//! Helpers to build main-menu bars, menu bars within windows, and nested menus
//! with optional enable/disable states and separators.
//!
use crate::sys;
use crate::ui::Ui;

/// # Menu Widgets
impl Ui {
    /// Creates and starts appending to a full-screen menu bar.
    ///
    /// Returns `Some(MainMenuBarToken)` if the menu bar is visible. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the menu bar is not visible and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginMainMenuBar")]
    pub fn begin_main_menu_bar(&self) -> Option<MainMenuBarToken<'_>> {
        if unsafe { sys::igBeginMainMenuBar() } {
            Some(MainMenuBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates and starts appending to a menu bar for a window.
    ///
    /// Returns `Some(MenuBarToken)` if the menu bar is visible. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the menu bar is not visible and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginMenuBar")]
    pub fn begin_menu_bar(&self) -> Option<MenuBarToken<'_>> {
        if unsafe { sys::igBeginMenuBar() } {
            Some(MenuBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates a menu and starts appending to it.
    ///
    /// Returns `Some(MenuToken)` if the menu is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the menu is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginMenu")]
    pub fn begin_menu(&self, label: impl AsRef<str>) -> Option<MenuToken<'_>> {
        self.begin_menu_with_enabled(label, true)
    }

    /// Creates a menu with enabled state and starts appending to it.
    ///
    /// Returns `Some(MenuToken)` if the menu is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the menu is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginMenu")]
    pub fn begin_menu_with_enabled(
        &self,
        label: impl AsRef<str>,
        enabled: bool,
    ) -> Option<MenuToken<'_>> {
        let label_ptr = self.scratch_txt(label);
        if unsafe { sys::igBeginMenu_Str(label_ptr, enabled) } {
            Some(MenuToken::new(self))
        } else {
            None
        }
    }

    /// Creates a menu and runs a closure to construct the contents.
    ///
    /// Note: the closure is not called if the menu is not visible.
    ///
    /// This is the equivalent of [menu_with_enabled](Self::menu_with_enabled)
    /// with `enabled` set to `true`.
    #[doc(alias = "BeginMenu")]
    pub fn menu<F: FnOnce()>(&self, label: impl AsRef<str>, f: F) {
        self.menu_with_enabled(label, true, f);
    }

    /// Creates a menu and runs a closure to construct the contents.
    ///
    /// Note: the closure is not called if the menu is not visible.
    #[doc(alias = "BeginMenu")]
    pub fn menu_with_enabled<F: FnOnce()>(&self, label: impl AsRef<str>, enabled: bool, f: F) {
        if let Some(_menu) = self.begin_menu_with_enabled(label, enabled) {
            f();
        }
    }

    /// Creates a menu item.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item(&self, label: impl AsRef<str>) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igMenuItemEx_Str(label_ptr, std::ptr::null(), std::ptr::null(), false, true) }
    }

    /// Creates a menu item with a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
    ) -> bool {
        let label_ptr = self.scratch_txt(label);
        let shortcut_ptr = self.scratch_txt(shortcut);
        unsafe { sys::igMenuItemEx_Str(label_ptr, std::ptr::null(), shortcut_ptr, false, true) }
    }

    /// Creates a menu item with explicit enabled/selected state.
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected(
        &self,
        label: impl AsRef<str>,
        shortcut: Option<impl AsRef<str>>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        let label_ptr = self.scratch_txt(label);
        let shortcut_ptr = shortcut
            .as_ref()
            .map(|s| self.scratch_txt(s.as_ref()))
            .unwrap_or(std::ptr::null());
        unsafe { sys::igMenuItem_StrStrBool(label_ptr, shortcut_ptr, selected, enabled) }
    }

    /// Creates a toggleable menu item bound to `selected` (updated in place).
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle(
        &self,
        label: impl AsRef<str>,
        shortcut: Option<impl AsRef<str>>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        let label_ptr = self.scratch_txt(label);
        let shortcut_ptr = shortcut
            .as_ref()
            .map(|s| self.scratch_txt(s.as_ref()))
            .unwrap_or(std::ptr::null());
        unsafe { sys::igMenuItem_StrStrBoolPtr(label_ptr, shortcut_ptr, selected, enabled) }
    }
}

/// Tracks a main menu bar that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MainMenuBarToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> MainMenuBarToken<'ui> {
    /// Creates a new main menu bar token
    fn new(ui: &'ui Ui) -> Self {
        MainMenuBarToken { ui }
    }

    /// Ends the main menu bar
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for MainMenuBarToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMainMenuBar();
        }
    }
}

/// Tracks a menu bar that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MenuBarToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> MenuBarToken<'ui> {
    /// Creates a new menu bar token
    fn new(ui: &'ui Ui) -> Self {
        MenuBarToken { ui }
    }

    /// Ends the menu bar
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for MenuBarToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMenuBar();
        }
    }
}

/// Tracks a menu that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct MenuToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> MenuToken<'ui> {
    /// Creates a new menu token
    fn new(ui: &'ui Ui) -> Self {
        MenuToken { ui }
    }

    /// Ends the menu
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for MenuToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndMenu();
        }
    }
}
