use crate::frame::Frame;
use crate::types::Vec2;
use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Window creation and management
use std::marker::PhantomData;

/// Window builder for creating Dear ImGui windows
///
/// This struct provides a fluent interface for configuring and creating
/// Dear ImGui windows. Use the various methods to configure the window
/// properties, then call `show()` to actually create and display the window.
///
/// # Example
///
/// ```rust,no_run
/// # use dear_imgui::Context;
/// # let mut ctx = Context::new().unwrap();
/// # let mut frame = ctx.frame();
/// frame.window("My Window")
///     .size([400.0, 300.0])
///     .position([100.0, 100.0])
///     .show(|ui| {
///         ui.text("Window content goes here");
///     });
/// ```
pub struct Window<'frame, 'ctx> {
    frame: &'frame mut Frame<'ctx>,
    title: String,
    size: Option<Vec2>,
    position: Option<Vec2>,
    flags: WindowFlags,
    _marker: PhantomData<&'frame mut Frame<'ctx>>,
}

impl<'frame, 'ctx> Window<'frame, 'ctx> {
    /// Create a new window builder (internal use only)
    pub(crate) fn new(frame: &'frame mut Frame<'ctx>, title: &str) -> Self {
        Self {
            frame,
            title: title.to_string(),
            size: None,
            position: None,
            flags: WindowFlags::empty(),
            _marker: PhantomData,
        }
    }

    /// Set the window size
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// frame.window("Sized Window")
    ///     .size([800.0, 600.0])
    ///     .show(|ui| {
    ///         // Window content
    ///     });
    /// ```
    pub fn size(mut self, size: impl Into<Vec2>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Set the window position
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// frame.window("Positioned Window")
    ///     .position([100.0, 50.0])
    ///     .show(|ui| {
    ///         // Window content
    ///     });
    /// ```
    pub fn position(mut self, position: impl Into<Vec2>) -> Self {
        self.position = Some(position.into());
        self
    }

    /// Set window flags
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, WindowFlags};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// frame.window("No Resize Window")
    ///     .flags(WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE)
    ///     .show(|ui| {
    ///         // Window content
    ///     });
    /// ```
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Show the window with the given content
    ///
    /// The provided closure will be called with a `Ui` object that can be used
    /// to build the window's content. The closure should return `true` to keep
    /// the window open, or `false` to close it.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// let keep_open = frame.window("Example")
    ///     .show(|ui| {
    ///         ui.text("Hello, world!");
    ///         if ui.button("Close") {
    ///             false // Close the window
    ///         } else {
    ///             true // Keep the window open
    ///         }
    ///     });
    /// ```
    pub fn show<F>(self, f: F) -> bool
    where
        F: FnOnce(&mut Ui) -> bool,
    {
        // Set window size if specified
        if let Some(size) = self.size {
            let size_vec = sys::ImVec2 {
                x: size.x,
                y: size.y,
            };
            unsafe {
                sys::ImGui_SetNextWindowSize(&size_vec as *const _, sys::ImGuiCond_FirstUseEver);
            }
        }

        // Set window position if specified
        if let Some(position) = self.position {
            let pos_vec = sys::ImVec2 {
                x: position.x,
                y: position.y,
            };
            let pivot = sys::ImVec2 { x: 0.0, y: 0.0 };
            unsafe {
                sys::ImGui_SetNextWindowPos(
                    &pos_vec as *const _,
                    sys::ImGuiCond_FirstUseEver,
                    &pivot as *const _,
                );
            }
        }

        // Begin the window
        let c_title = std::ffi::CString::new(self.title).unwrap_or_default();
        let mut open = true;
        let window_open = unsafe {
            sys::ImGui_Begin(
                c_title.as_ptr(),
                &mut open as *mut bool,
                self.flags.bits() as i32,
            )
        };

        let result = if window_open {
            let mut ui = Ui::new();
            f(&mut ui)
        } else {
            true
        };

        // End the window
        unsafe {
            sys::ImGui_End();
        }

        result && open
    }
}

bitflags::bitflags! {
    /// Window flags for configuring window behavior
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Disable title bar
        const NO_TITLE_BAR = 1 << 0;
        /// Disable user resizing with the lower-right grip
        const NO_RESIZE = 1 << 1;
        /// Disable user moving the window
        const NO_MOVE = 1 << 2;
        /// Disable scrollbars (window can still scroll with mouse or programmatically)
        const NO_SCROLLBAR = 1 << 3;
        /// Disable user vertically scrolling with mouse wheel
        const NO_SCROLL_WITH_MOUSE = 1 << 4;
        /// Disable user collapsing window by double-clicking on it
        const NO_COLLAPSE = 1 << 5;
        /// Resize every window to its content every frame
        const ALWAYS_AUTO_RESIZE = 1 << 6;
        /// Disable drawing background color (WindowBg, etc.) and outside border
        const NO_BACKGROUND = 1 << 7;
        /// Never load/save settings in .ini file
        const NO_SAVED_SETTINGS = 1 << 8;
        /// Disable catching mouse, test for bounding box of contents
        const NO_MOUSE_INPUTS = 1 << 9;
        /// Has a menu-bar
        const MENU_BAR = 1 << 10;
        /// Allow horizontal scrollbar to appear (off by default)
        const HORIZONTAL_SCROLLBAR = 1 << 11;
        /// Disable taking focus when transitioning from hidden to visible state
        const NO_FOCUS_ON_APPEARING = 1 << 12;
        /// Disable bringing window to front when taking focus
        const NO_BRING_TO_FRONT_ON_FOCUS = 1 << 13;
        /// Always show vertical scrollbar
        const ALWAYS_VERTICAL_SCROLLBAR = 1 << 14;
        /// Always show horizontal scrollbar
        const ALWAYS_HORIZONTAL_SCROLLBAR = 1 << 15;
        /// Ensure child windows without border uses style.WindowPadding
        const ALWAYS_USE_WINDOW_PADDING = 1 << 16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn test_window_flags() {
        let flags = WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE;
        assert!(flags.contains(WindowFlags::NO_RESIZE));
        assert!(flags.contains(WindowFlags::NO_MOVE));
        assert!(!flags.contains(WindowFlags::NO_TITLE_BAR));
    }
}
