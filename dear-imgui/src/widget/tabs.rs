use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Tab widgets
///
/// This module contains all tab-related UI components like tab bars and tab items.

/// # Widgets: Tabs
impl<'frame> Ui<'frame> {
    /// Begin a tab bar
    ///
    /// Returns `true` if the tab bar is visible and should be populated.
    /// Must call `end_tab_bar()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_tab_bar("MyTabBar") {
    ///     if ui.begin_tab_item("Tab 1") {
    ///         ui.text("Content of tab 1");
    ///         ui.end_tab_item();
    ///     }
    ///     if ui.begin_tab_item("Tab 2") {
    ///         ui.text("Content of tab 2");
    ///         ui.end_tab_item();
    ///     }
    ///     ui.end_tab_bar();
    /// }
    /// # });
    /// ```
    pub fn begin_tab_bar(&mut self, str_id: impl AsRef<str>) -> bool {
        unsafe {
            sys::ImGui_BeginTabBar(self.scratch_txt(str_id), 0) // Default flags
        }
    }

    /// End tab bar (must be called after begin_tab_bar returns true)
    pub fn end_tab_bar(&mut self) {
        unsafe {
            sys::ImGui_EndTabBar();
        }
    }

    /// Begin a tab item
    ///
    /// Returns `true` if the tab is selected and should show its content.
    /// Must call `end_tab_item()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_tab_bar("MyTabBar") {
    ///     if ui.begin_tab_item("Settings") {
    ///         ui.text("Settings content here");
    ///         ui.end_tab_item();
    ///     }
    ///     ui.end_tab_bar();
    /// }
    /// # });
    /// ```
    pub fn begin_tab_item(&mut self, label: impl AsRef<str>) -> bool {
        unsafe {
            sys::ImGui_BeginTabItem(
                self.scratch_txt(label),
                std::ptr::null_mut(), // No open flag
                0,                    // Default flags
            )
        }
    }

    /// Begin a tab item with close button
    ///
    /// Returns `true` if the tab is selected and should show its content.
    /// The `open` parameter will be set to `false` if the close button is clicked.
    /// Must call `end_tab_item()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut tab_open = true;
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_tab_bar("MyTabBar") {
    ///     if ui.begin_tab_item_with_close("Closable Tab", &mut tab_open) {
    ///         ui.text("This tab can be closed");
    ///         ui.end_tab_item();
    ///     }
    ///     if !tab_open {
    ///         println!("Tab was closed!");
    ///     }
    ///     ui.end_tab_bar();
    /// }
    /// # });
    /// ```
    pub fn begin_tab_item_with_close(&mut self, label: impl AsRef<str>, open: &mut bool) -> bool {
        unsafe {
            sys::ImGui_BeginTabItem(
                self.scratch_txt(label),
                open as *mut bool,
                0, // Default flags
            )
        }
    }

    /// End tab item (must be called after begin_tab_item returns true)
    pub fn end_tab_item(&mut self) {
        unsafe {
            sys::ImGui_EndTabItem();
        }
    }

    /// Create a tab item button (for adding new tabs)
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_tab_bar("MyTabBar") {
    ///     if ui.tab_item_button("+") {
    ///         println!("Add new tab!");
    ///     }
    ///     ui.end_tab_bar();
    /// }
    /// # });
    /// ```
    pub fn tab_item_button(&mut self, label: impl AsRef<str>) -> bool {
        unsafe {
            sys::ImGui_TabItemButton(self.scratch_txt(label), 0) // Default flags
        }
    }

    /// Set the next tab item to be selected
    ///
    /// Call this before begin_tab_item() to make that tab selected.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_tab_bar("MyTabBar") {
    ///     // Select the second tab programmatically
    ///     ui.set_tab_item_closed("Tab 2");
    ///     
    ///     if ui.begin_tab_item("Tab 1") {
    ///         ui.text("Tab 1 content");
    ///         ui.end_tab_item();
    ///     }
    ///     if ui.begin_tab_item("Tab 2") {
    ///         ui.text("Tab 2 content (selected)");
    ///         ui.end_tab_item();
    ///     }
    ///     ui.end_tab_bar();
    /// }
    /// # });
    /// ```
    pub fn set_tab_item_closed(&mut self, tab_or_docked_window_label: impl AsRef<str>) {
        unsafe {
            sys::ImGui_SetTabItemClosed(self.scratch_txt(tab_or_docked_window_label));
        }
    }
}
