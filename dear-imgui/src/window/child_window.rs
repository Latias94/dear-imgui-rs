//! Child windows
//!
//! Tools for building scrollable, optionally framed child regions within a
//! parent window. Useful for panels, property views or nested layouts.
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.window("Parent").build(|| {
//!     ui.child_window("pane")
//!         .size([200.0, 120.0])
//!         .border(true)
//!         .build(&ui, || {
//!             ui.text("Inside child window");
//!         });
//! });
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::sys;
use crate::{Ui, WindowFlags};
use std::borrow::Cow;

bitflags::bitflags! {
    /// Configuration flags for child windows
    #[repr(transparent)]
    pub struct ChildFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Show an outer border and enable WindowPadding
        const BORDERS = sys::ImGuiChildFlags_Borders as u32;
        /// Pad with style.WindowPadding even if no border are drawn
        const ALWAYS_USE_WINDOW_PADDING = sys::ImGuiChildFlags_AlwaysUseWindowPadding as u32;
        /// Allow resize from right border
        const RESIZE_X = sys::ImGuiChildFlags_ResizeX as u32;
        /// Allow resize from bottom border
        const RESIZE_Y = sys::ImGuiChildFlags_ResizeY as u32;
        /// Enable auto-resizing width
        const AUTO_RESIZE_X = sys::ImGuiChildFlags_AutoResizeX as u32;
        /// Enable auto-resizing height
        const AUTO_RESIZE_Y = sys::ImGuiChildFlags_AutoResizeY as u32;
        /// Combined with AutoResizeX/AutoResizeY. Always measure size even when child is hidden
        const ALWAYS_AUTO_RESIZE = sys::ImGuiChildFlags_AlwaysAutoResize as u32;
        /// Style the child window like a framed item
        const FRAME_STYLE = sys::ImGuiChildFlags_FrameStyle as u32;
        /// Share focus scope, allow gamepad/keyboard navigation to cross over parent border
        const NAV_FLATTENED = sys::ImGuiChildFlags_NavFlattened as u32;
    }
}

/// Represents a child window that can be built
pub struct ChildWindow<'ui> {
    name: Cow<'ui, str>,
    size: [f32; 2],
    child_flags: ChildFlags,
    flags: WindowFlags,
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> ChildWindow<'ui> {
    /// Creates a new child window builder
    pub fn new(name: impl Into<Cow<'ui, str>>) -> Self {
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
    fn begin(self, ui: &'ui Ui) -> Option<ChildWindowToken<'ui>> {
        let name_ptr = ui.scratch_txt(self.name);

        let result = unsafe {
            let size_vec = sys::ImVec2 {
                x: self.size[0],
                y: self.size[1],
            };
            sys::igBeginChild_Str(
                name_ptr,
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
    pub fn child_window<'ui>(&'ui self, name: impl Into<Cow<'ui, str>>) -> ChildWindow<'ui> {
        ChildWindow::new(name)
    }
}
