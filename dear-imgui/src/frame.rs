use std::ffi::CString;
use std::marker::PhantomData;

use crate::context::Context;
use crate::draw_data::DrawData;
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Represents a single Dear ImGui frame
///
/// A frame is created by calling `Context::frame()` and represents one frame
/// of Dear ImGui rendering. The frame automatically begins when created and
/// ends when dropped.
///
/// # Example
///
/// ```rust,no_run
/// use dear_imgui::Context;
///
/// let mut ctx = Context::new().unwrap();
/// let mut frame = ctx.frame();
///
/// // Build UI using the frame
/// frame.window("Hello").show(|ui| {
///     ui.text("Hello, world!");
/// });
///
/// // Frame automatically ends when dropped
/// ```
pub struct Frame<'ctx> {
    context: &'ctx mut Context,
    _marker: PhantomData<&'ctx mut Context>,
}

impl<'ctx> Frame<'ctx> {
    /// Create a new frame (internal use only)
    pub(crate) fn new(context: &'ctx mut Context) -> Self {
        Self {
            context,
            _marker: PhantomData,
        }
    }

    /// Create a window builder
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// frame.window("My Window")
    ///     .size([400.0, 300.0])
    ///     .show(|ui| {
    ///         ui.text("Window content");
    ///     });
    /// ```
    pub fn window(&mut self, title: &str) -> crate::window::Window<'_, 'ctx> {
        crate::window::Window::new(self, title)
    }

    /// Get the draw data for this frame
    ///
    /// This should be called after all UI has been built for the frame.
    /// The returned draw data can be used by a renderer to actually draw
    /// the UI to the screen.
    pub fn draw_data(&self) -> DrawData {
        unsafe {
            sys::ImGui_Render();
            let draw_data_ptr = sys::ImGui_GetDrawData();
            DrawData::from_raw(&*draw_data_ptr)
        }
    }

    /// Get the raw ImGui context pointer
    ///
    /// # Safety
    ///
    /// This is unsafe because it returns a raw pointer that could be used
    /// to violate memory safety. Only use this if you need to call Dear ImGui
    /// functions that are not yet wrapped by this library.
    pub unsafe fn raw_context(&self) -> *mut sys::ImGuiContext {
        self.context.raw()
    }

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

    /// Begin a menu
    ///
    /// Returns `true` if the menu is open and should be populated.
    /// Must call `end_menu()` if this returns true.
    pub fn begin_menu(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_BeginMenu(c_label.as_ptr(), true) }
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
    pub fn menu_item(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_MenuItem(
                c_label.as_ptr(),
                std::ptr::null(), // No shortcut
                false,            // Not selected
                true,             // Enabled
            )
        }
    }

    /// Create a menu item with a boolean state
    ///
    /// Returns `true` if the menu item was clicked.
    pub fn menu_item_bool(&mut self, label: impl AsRef<str>, selected: &mut bool) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_MenuItem1(
                c_label.as_ptr(),
                std::ptr::null(), // No shortcut
                selected as *mut bool,
                true, // Enabled
            )
        }
    }

    /// End the frame manually
    ///
    /// This is called automatically when the frame is dropped, but you can
    /// call it manually if needed.
    fn end_frame(&mut self) {
        unsafe {
            sys::ImGui_EndFrame();
        }
    }
}

impl<'ctx> Drop for Frame<'ctx> {
    fn drop(&mut self) {
        self.end_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn test_frame_lifecycle() {
        let mut ctx = Context::new().unwrap();
        {
            let _frame = ctx.frame();
            // Frame should end automatically when dropped
        }
    }
}
