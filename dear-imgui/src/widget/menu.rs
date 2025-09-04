use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Menu widgets
///
/// This module contains all menu-related UI components like menu bars, menus, menu items, etc.

/// # Widgets: Menu
impl<'frame> Ui<'frame> {
    /// Begin a main menu bar
    ///
    /// Returns `true` if the menu bar is visible and should be populated.
    /// Must call `end_main_menu_bar()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// if frame.begin_main_menu_bar() {
    ///     if frame.begin_menu("File") {
    ///         if frame.menu_item("New") {
    ///             println!("New file");
    ///         }
    ///         frame.end_menu();
    ///     }
    ///     frame.end_main_menu_bar();
    /// }
    /// ```
    pub fn begin_main_menu_bar(&mut self) -> bool {
        unsafe { sys::ImGui_BeginMainMenuBar() }
    }

    /// End main menu bar (must be called after begin_main_menu_bar returns true)
    pub fn end_main_menu_bar(&mut self) {
        unsafe {
            sys::ImGui_EndMainMenuBar();
        }
    }

    /// Begin a menu bar for the current window
    ///
    /// Returns `true` if the menu bar is visible and should be populated.
    /// Must call `end_menu_bar()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_menu_bar() {
    ///     if ui.begin_menu("Edit") {
    ///         if ui.menu_item("Copy") {
    ///             println!("Copy");
    ///         }
    ///         ui.end_menu();
    ///     }
    ///     ui.end_menu_bar();
    /// }
    /// # });
    /// ```
    pub fn begin_menu_bar(&mut self) -> bool {
        unsafe { sys::ImGui_BeginMenuBar() }
    }

    /// End menu bar (must be called after begin_menu_bar returns true)
    pub fn end_menu_bar(&mut self) {
        unsafe {
            sys::ImGui_EndMenuBar();
        }
    }

    /// Begin a menu
    ///
    /// Returns `true` if the menu is open and should be populated.
    /// Must call `end_menu()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_menu("Options") {
    ///     if ui.menu_item("Setting 1") {
    ///         println!("Setting 1 clicked");
    ///     }
    ///     ui.menu_item_bool("Setting 2", &mut true);
    ///     ui.end_menu();
    /// }
    /// # });
    /// ```
    pub fn begin_menu(&mut self, label: impl AsRef<str>) -> bool {
        unsafe { sys::ImGui_BeginMenu(self.scratch_txt(label), true) }
    }

    /// End menu (must be called after begin_menu returns true)
    pub fn end_menu(&mut self) {
        unsafe {
            sys::ImGui_EndMenu();
        }
    }

    /// Create a menu item
    ///
    /// Returns `true` if the menu item was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.menu_item("Save") {
    ///     println!("Save clicked");
    /// }
    /// # });
    /// ```
    pub fn menu_item(&mut self, label: impl AsRef<str>) -> bool {
        unsafe {
            sys::ImGui_MenuItem(
                self.scratch_txt(label),
                std::ptr::null(), // No shortcut
                false,            // Not selected
                true,             // Enabled
            )
        }
    }

    /// Create a menu item with a boolean state
    ///
    /// Returns `true` if the menu item was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut enabled = true;
    /// # frame.window("Example").show(|ui| {
    /// if ui.menu_item_bool("Enable Feature", &mut enabled) {
    ///     println!("Feature toggled: {}", enabled);
    /// }
    /// # });
    /// ```
    pub fn menu_item_bool(&mut self, label: impl AsRef<str>, selected: &mut bool) -> bool {
        unsafe {
            sys::ImGui_MenuItem1(
                self.scratch_txt(label),
                std::ptr::null(), // No shortcut
                selected as *mut bool,
                true, // Enabled
            )
        }
    }
}
