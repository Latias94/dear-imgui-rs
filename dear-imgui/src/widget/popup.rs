use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Popup widgets
///
/// This module contains all popup-related UI components like popups and modals.
use std::ffi::CString;

/// # Widgets: Popup
impl<'frame> Ui<'frame> {
    /// Begin a popup
    ///
    /// Returns `true` if the popup is open and should be populated.
    /// Must call `end_popup()` if this returns true.
    /// Call `open_popup()` first to open the popup.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.button("Open Popup") {
    ///     ui.open_popup("my_popup");
    /// }
    ///
    /// if ui.begin_popup("my_popup") {
    ///     ui.text("This is a popup!");
    ///     if ui.button("Close") {
    ///         ui.close_current_popup();
    ///     }
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup(&mut self, str_id: impl AsRef<str>) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe {
            sys::ImGui_BeginPopup(c_str_id.as_ptr(), 0) // Default flags
        }
    }

    /// Begin a modal popup
    ///
    /// Returns `true` if the modal is open and should be populated.
    /// Must call `end_popup()` if this returns true.
    /// Call `open_popup()` first to open the modal.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.button("Open Modal") {
    ///     ui.open_popup("my_modal");
    /// }
    ///
    /// if ui.begin_popup_modal("my_modal") {
    ///     ui.text("This is a modal popup!");
    ///     ui.text("You must close it to continue.");
    ///     
    ///     if ui.button("OK") {
    ///         ui.close_current_popup();
    ///     }
    ///     ui.same_line();
    ///     if ui.button("Cancel") {
    ///         ui.close_current_popup();
    ///     }
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup_modal(&mut self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        let c_name = CString::new(name).unwrap_or_default();
        unsafe {
            sys::ImGui_BeginPopupModal(
                c_name.as_ptr(),
                std::ptr::null_mut(), // No open flag
                0,                    // Default flags
            )
        }
    }

    /// Begin a modal popup with close button
    ///
    /// Returns `true` if the modal is open and should be populated.
    /// The `open` parameter will be set to `false` if the close button is clicked.
    /// Must call `end_popup()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut modal_open = true;
    /// # frame.window("Example").show(|ui| {
    /// if ui.button("Open Modal") {
    ///     ui.open_popup("my_modal");
    /// }
    ///
    /// if ui.begin_popup_modal_with_close("my_modal", &mut modal_open) {
    ///     ui.text("Modal with close button");
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup_modal_with_close(&mut self, name: impl AsRef<str>, open: &mut bool) -> bool {
        let name = name.as_ref();
        let c_name = CString::new(name).unwrap_or_default();
        unsafe {
            sys::ImGui_BeginPopupModal(
                c_name.as_ptr(),
                open as *mut bool,
                0, // Default flags
            )
        }
    }

    /// End popup (must be called after begin_popup or begin_popup_modal returns true)
    pub fn end_popup(&mut self) {
        unsafe {
            sys::ImGui_EndPopup();
        }
    }

    /// Open a popup
    ///
    /// Call this to open a popup that was defined with begin_popup().
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.button("Open") {
    ///     ui.open_popup("context_menu");
    /// }
    /// # });
    /// ```
    pub fn open_popup(&mut self, str_id: impl AsRef<str>) {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe {
            sys::ImGui_OpenPopup(c_str_id.as_ptr(), 0); // Default flags
        }
    }

    /// Close the current popup
    ///
    /// Call this from within a popup to close it.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_popup("my_popup") {
    ///     if ui.button("Close") {
    ///         ui.close_current_popup();
    ///     }
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn close_current_popup(&mut self) {
        unsafe {
            sys::ImGui_CloseCurrentPopup();
        }
    }

    /// Begin a popup context item
    ///
    /// Returns `true` if a context popup is open for the last item.
    /// Must call `end_popup()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Right-click me!");
    /// if ui.begin_popup_context_item() {
    ///     if ui.menu_item("Copy") {
    ///         println!("Copy clicked");
    ///     }
    ///     if ui.menu_item("Paste") {
    ///         println!("Paste clicked");
    ///     }
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup_context_item(&mut self) -> bool {
        unsafe {
            sys::ImGui_BeginPopupContextItem(
                std::ptr::null(), // Use default ID
                1,                // Right mouse button
            )
        }
    }

    /// Begin a popup context item with custom ID
    ///
    /// Returns `true` if a context popup is open for the last item.
    /// Must call `end_popup()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Right-click me!");
    /// if ui.begin_popup_context_item_with_id("item_context") {
    ///     ui.text("Custom context menu");
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup_context_item_with_id(&mut self, str_id: impl AsRef<str>) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe {
            sys::ImGui_BeginPopupContextItem(
                c_str_id.as_ptr(),
                1, // Right mouse button
            )
        }
    }

    /// Begin a popup context window
    ///
    /// Returns `true` if a context popup is open for the current window.
    /// Must call `end_popup()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_popup_context_window() {
    ///     ui.text("Window context menu");
    ///     if ui.menu_item("Close Window") {
    ///         // Handle window close
    ///     }
    ///     ui.end_popup();
    /// }
    /// # });
    /// ```
    pub fn begin_popup_context_window(&mut self) -> bool {
        unsafe {
            sys::ImGui_BeginPopupContextWindow(
                std::ptr::null(), // Use default ID
                1,                // Right mouse button
            )
        }
    }

    /// Check if a popup is open
    ///
    /// Returns `true` if the specified popup is currently open.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.is_popup_open("my_popup") {
    ///     ui.text("Popup is open!");
    /// }
    /// # });
    /// ```
    pub fn is_popup_open(&mut self, str_id: impl AsRef<str>) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe {
            sys::ImGui_IsPopupOpen(c_str_id.as_ptr(), 0) // Default flags
        }
    }
}
