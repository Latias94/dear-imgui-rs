use crate::sys;
use crate::ui::Ui;

use super::{MainMenuBarToken, MenuBarToken, MenuToken};

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
        if unsafe { sys::igBeginMenu(label_ptr, enabled) } {
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
}
