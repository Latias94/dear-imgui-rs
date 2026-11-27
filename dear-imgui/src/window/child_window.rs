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
//!         .build(|| {
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
// NOTE: Keep explicit `as u32` casts when using bindgen-generated flag constants.
// The exact Rust type of `sys::ImGui*Flags_*` may vary across platforms/toolchains, while our
// public wrapper APIs intentionally expose fixed underlying integer types.
use super::validate_window_flags;
use crate::sys;
use crate::validation::assert_finite_vec2;
use crate::{Ui, WindowFlags};
use std::borrow::Cow;

bitflags::bitflags! {
    /// Configuration flags for child windows
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

fn validate_child_flags(caller: &str, child_flags: ChildFlags, window_flags: WindowFlags) {
    let unsupported = child_flags.bits() & !ChildFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiChildFlags bits: 0x{unsupported:X}"
    );
    validate_window_flags(caller, window_flags);
    assert!(
        !window_flags.contains(WindowFlags::ALWAYS_AUTO_RESIZE),
        "{caller} cannot use WindowFlags::ALWAYS_AUTO_RESIZE; use ChildFlags::ALWAYS_AUTO_RESIZE"
    );
    if child_flags.contains(ChildFlags::ALWAYS_AUTO_RESIZE) {
        assert!(
            !child_flags.intersects(ChildFlags::RESIZE_X | ChildFlags::RESIZE_Y),
            "{caller} cannot combine ALWAYS_AUTO_RESIZE with RESIZE_X or RESIZE_Y"
        );
        assert!(
            child_flags.intersects(ChildFlags::AUTO_RESIZE_X | ChildFlags::AUTO_RESIZE_Y),
            "{caller} requires AUTO_RESIZE_X or AUTO_RESIZE_Y when using ALWAYS_AUTO_RESIZE"
        );
    }
}

/// Represents a child window that can be built
#[must_use]
pub struct ChildWindow<'ui> {
    ui: &'ui Ui,
    name: Cow<'ui, str>,
    size: [f32; 2],
    child_flags: ChildFlags,
    flags: WindowFlags,
}

impl<'ui> ChildWindow<'ui> {
    /// Sets the size of the child window
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = size.into();
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

    /// Builds the child window and calls the closure while its Context is current.
    #[doc(alias = "BeginChild", alias = "EndChild")]
    pub fn build<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        validate_child_flags("ChildWindow::build()", self.child_flags, self.flags);
        assert_finite_vec2("ChildWindow::build()", "size", self.size);

        let ui = self.ui;
        ui.with_bound_context(|| {
            let visible = ui.with_scratch_c_string(&self.name, |name| unsafe {
                sys::igBeginChild_Str(
                    name,
                    self.size.into(),
                    self.child_flags.bits() as i32,
                    self.flags.bits(),
                )
            });
            if !visible {
                unsafe { sys::igEndChild() };
                return None;
            }
            let child = ChildWindowGuard::new(ui);
            let result = f();
            drop(child);
            Some(result)
        })
    }
}

/// Internal guard representing an active child window.
struct ChildWindowGuard<'ui> {
    scope: crate::scope::NativeScopeToken<'ui>,
}

impl<'ui> ChildWindowGuard<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui
                .begin_native_scope(crate::scope::NativeScopePop::EndChild, "ChildWindowGuard"),
        }
    }
}

impl Drop for ChildWindowGuard<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

impl Ui {
    /// Creates a child window builder
    pub fn child_window<'ui>(&'ui self, name: impl Into<Cow<'ui, str>>) -> ChildWindow<'ui> {
        ChildWindow {
            ui: self,
            name: name.into(),
            size: [0.0, 0.0],
            child_flags: ChildFlags::NONE,
            flags: WindowFlags::empty(),
        }
    }
}
