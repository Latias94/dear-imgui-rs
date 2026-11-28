//! Menus and menu bars
//!
//! Helpers to build main-menu bars, menu bars within windows, and nested menus
//! with optional enable/disable states and separators.
//!

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Ui, sys};

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
        if self.run_with_bound_context(|| unsafe { sys::igBeginMainMenuBar() }) {
            Some(MainMenuBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates the full-screen main menu bar and runs a closure to construct its contents.
    ///
    /// Returns `None` without calling `f` when the menu bar is not visible. The menu bar is ended
    /// before a successful closure result is returned and during unwinding if `f` panics.
    #[doc(alias = "BeginMainMenuBar", alias = "EndMainMenuBar")]
    pub fn main_menu_bar<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        let token = self.begin_main_menu_bar()?;
        let result = f();
        drop(token);
        Some(result)
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
        if self.run_with_bound_context(|| unsafe { sys::igBeginMenuBar() }) {
            Some(MenuBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates the current window's menu bar and runs a closure to construct its contents.
    ///
    /// Returns `None` without calling `f` when the menu bar is not visible. The menu bar is ended
    /// before a successful closure result is returned and during unwinding if `f` panics.
    #[doc(alias = "BeginMenuBar", alias = "EndMenuBar")]
    pub fn menu_bar<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        let token = self.begin_menu_bar()?;
        let result = f();
        drop(token);
        Some(result)
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
        if self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igBeginMenu(label_ptr, enabled)
        }) {
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
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igMenuItemEx(label_ptr, std::ptr::null(), std::ptr::null(), false, true)
        })
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
        self.with_scratch_txt_two(label, shortcut, |label_ptr, shortcut_ptr| unsafe {
            sys::igMenuItemEx(label_ptr, std::ptr::null(), shortcut_ptr, false, true)
        })
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
        self.with_scratch_txt_with_opt(label, shortcut, |label_ptr, shortcut_ptr| unsafe {
            sys::igMenuItem_Bool(label_ptr, shortcut_ptr, selected, enabled)
        })
    }

    /// Creates a menu item with explicit enabled/selected state (no shortcut).
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected_no_shortcut(
        &self,
        label: impl AsRef<str>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_enabled_selected(label, None::<&str>, selected, enabled)
    }

    /// Creates a menu item with explicit enabled/selected state and a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_enabled_selected(label, Some(shortcut), selected, enabled)
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
        self.with_scratch_txt_with_opt(label, shortcut, |label_ptr, shortcut_ptr| unsafe {
            sys::igMenuItem_BoolPtr(label_ptr, shortcut_ptr, selected, enabled)
        })
    }

    /// Creates a toggleable menu item bound to `selected` (no shortcut).
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle_no_shortcut(
        &self,
        label: impl AsRef<str>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_toggle(label, None::<&str>, selected, enabled)
    }

    /// Creates a toggleable menu item bound to `selected` with a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_toggle(label, Some(shortcut), selected, enabled)
    }
}

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
    fn new(ui: &'ui Ui) -> Self {
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
    fn new(ui: &'ui Ui) -> Self {
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
    fn new(ui: &'ui Ui) -> Self {
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
