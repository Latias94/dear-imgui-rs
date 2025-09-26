#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::sys;
use crate::{Ui, WindowFlags};
use std::ffi::CString;

bitflags::bitflags! {
    /// Configuration flags for child windows
    #[repr(transparent)]
    pub struct ChildFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Show an outer border and enable WindowPadding
        const BORDERS = 1 << 0;
        /// Pad with style.WindowPadding even if no border are drawn
        const ALWAYS_USE_WINDOW_PADDING = 1 << 1;
        /// Allow resize from right border
        const RESIZE_X = 1 << 2;
        /// Allow resize from bottom border
        const RESIZE_Y = 1 << 3;
        /// Enable auto-resizing width
        const AUTO_RESIZE_X = 1 << 4;
        /// Enable auto-resizing height
        const AUTO_RESIZE_Y = 1 << 5;
        /// Combined with AutoResizeX/AutoResizeY. Always measure size even when child is hidden
        const ALWAYS_AUTO_RESIZE = 1 << 6;
        /// Style the child window like a framed item
        const FRAME_STYLE = 1 << 7;
        /// Share focus scope, allow gamepad/keyboard navigation to cross over parent border
        const NAV_FLATTENED = 1 << 8;
    }
}

/// Represents a child window that can be built
pub struct ChildWindow<'ui> {
    name: String,
    size: [f32; 2],
    child_flags: ChildFlags,
    flags: WindowFlags,
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> ChildWindow<'ui> {
    /// Creates a new child window builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: [0.0, 0.0],
            child_flags: ChildFlags::NONE,
            flags: WindowFlags::empty(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sets the size of the child window
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the child window has a border
    pub fn border(mut self, border: bool) -> Self {
        self.child_flags.set(ChildFlags::BORDERS, border);
        self
    }

    /// Sets child flags for the child window
    pub fn child_flags(mut self, child_flags: ChildFlags) -> Self {
        self.child_flags = child_flags;
        self
    }

    /// Sets window flags for the child window
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the child window and calls the provided closure
    pub fn build<F, R>(self, ui: &'ui Ui, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let token = self.begin(ui)?;
        let result = f();
        drop(token); // Explicitly drop the token to call EndChild
        Some(result)
    }

    /// Begins the child window and returns a token
    fn begin(self, _ui: &'ui Ui) -> Option<ChildWindowToken<'ui>> {
        let name_cstr = CString::new(self.name).ok()?;

        let result = unsafe {
            let size_vec = sys::ImVec2 {
                x: self.size[0],
                y: self.size[1],
            };
            sys::igBeginChild_Str(
                name_cstr.as_ptr(),
                size_vec,
                self.child_flags.bits() as i32,
                self.flags.bits(),
            )
        };

        // IMPORTANT: According to ImGui documentation, BeginChild/EndChild are inconsistent
        // with other Begin/End functions. EndChild() must ALWAYS be called regardless of
        // what BeginChild() returns. However, if BeginChild returns false, EndChild must
        // be called immediately and no content should be rendered.
        if result {
            Some(ChildWindowToken {
                _phantom: std::marker::PhantomData,
            })
        } else {
            // If BeginChild returns false, call EndChild immediately and return None
            unsafe {
                sys::igEndChild();
            }
            None
        }
    }
}

/// Token representing an active child window
pub struct ChildWindowToken<'ui> {
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> Drop for ChildWindowToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndChild();
        }
    }
}

impl Ui {
    /// Creates a child window builder
    pub fn child_window(&self, name: impl Into<String>) -> ChildWindow<'_> {
        ChildWindow::new(name)
    }
}
