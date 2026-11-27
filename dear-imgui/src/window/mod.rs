//! Windows and window utilities
//!
//! This module exposes the `Window` builder and related flags for creating
//! top-level Dear ImGui windows. It also houses helpers for child windows,
//! querying content-region size, and controlling window scrolling.
//!
//! Basic usage:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.window("Hello")
//!     .size([320.0, 240.0], Condition::FirstUseEver)
//!     .position([60.0, 60.0], Condition::FirstUseEver)
//!     .build(|| {
//!         ui.text("Window contents go here");
//!     });
//! ```
//!
//! See also:
//! - `child_window` for scoped child areas
//! - `content_region` for available size queries
//! - `scroll` for reading and setting scroll positions
//!
//! Quick example (flags + size/pos conditions):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! use dear_imgui_rs::WindowFlags;
//! ui.window("Tools")
//!     .flags(WindowFlags::NO_RESIZE | WindowFlags::NO_COLLAPSE)
//!     .size([300.0, 200.0], Condition::FirstUseEver)
//!     .position([50.0, 60.0], Condition::FirstUseEver)
//!     .build(|| {
//!         ui.text("Toolbox contents...");
//!     });
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use bitflags::bitflags;
use std::f32;

use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2};
use crate::{Condition, Ui};

mod child_window;
pub(crate) mod content_region;
mod key;
pub(crate) mod scroll;

pub use child_window::{ChildFlags, ChildWindow};
pub use key::{WindowKey, WindowKeyError, WindowLabel};

// Window-focused/hovered helpers are available via utils.rs variants.
// Window hovered/focused flag helpers are provided by crate::utils.

bitflags! {
    /// Configuration flags for windows
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowFlags: i32 {
        /// Disable title-bar
        const NO_TITLE_BAR = sys::ImGuiWindowFlags_NoTitleBar as i32;
        /// Disable user resizing with the lower-right grip
        const NO_RESIZE = sys::ImGuiWindowFlags_NoResize as i32;
        /// Disable user moving the window
        const NO_MOVE = sys::ImGuiWindowFlags_NoMove as i32;
        /// Disable scrollbars (window can still scroll with mouse or programmatically)
        const NO_SCROLLBAR = sys::ImGuiWindowFlags_NoScrollbar as i32;
        /// Disable user vertically scrolling with mouse wheel
        const NO_SCROLL_WITH_MOUSE = sys::ImGuiWindowFlags_NoScrollWithMouse as i32;
        /// Disable user collapsing window by double-clicking on it
        const NO_COLLAPSE = sys::ImGuiWindowFlags_NoCollapse as i32;
        /// Resize every window to its content every frame
        const ALWAYS_AUTO_RESIZE = sys::ImGuiWindowFlags_AlwaysAutoResize as i32;
        /// Disable drawing background color (WindowBg, etc.) and outside border
        const NO_BACKGROUND = sys::ImGuiWindowFlags_NoBackground as i32;
        /// Never load/save settings in .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiWindowFlags_NoSavedSettings as i32;
        /// Disable catching mouse, hovering test with pass through
        const NO_MOUSE_INPUTS = sys::ImGuiWindowFlags_NoMouseInputs as i32;
        /// Has a menu-bar
        const MENU_BAR = sys::ImGuiWindowFlags_MenuBar as i32;
        /// Allow horizontal scrollbar to appear (off by default)
        const HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_HorizontalScrollbar as i32;
        /// Disable taking focus when transitioning from hidden to visible state
        const NO_FOCUS_ON_APPEARING = sys::ImGuiWindowFlags_NoFocusOnAppearing as i32;
        /// Disable bringing window to front when taking focus (e.g. clicking on it or programmatically giving it focus)
        const NO_BRING_TO_FRONT_ON_FOCUS = sys::ImGuiWindowFlags_NoBringToFrontOnFocus as i32;
        /// Always show vertical scrollbar (even if ContentSize.y < Size.y)
        const ALWAYS_VERTICAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysVerticalScrollbar as i32;
        /// Always show horizontal scrollbar (even if ContentSize.x < Size.x)
        const ALWAYS_HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysHorizontalScrollbar as i32;
        /// No gamepad/keyboard navigation within the window
        const NO_NAV_INPUTS = sys::ImGuiWindowFlags_NoNavInputs as i32;
        /// No focusing toward this window with gamepad/keyboard navigation (e.g. skipped by CTRL+TAB)
        const NO_NAV_FOCUS = sys::ImGuiWindowFlags_NoNavFocus as i32;
        /// Display a dot next to the title. When used in a tab/docking context, tab is selected when clicking the X + closure is not assumed (will wait for user to stop submitting the tab). Otherwise closure is assumed when pressing the X, so if you keep submitting the tab may reappear at end of tab bar.
        const UNSAVED_DOCUMENT = sys::ImGuiWindowFlags_UnsavedDocument as i32;
        // Docking related flags
        /// Disable docking for this window (the window will not be able to dock into another and others won't be able to dock into it)
        const NO_DOCKING = sys::ImGuiWindowFlags_NoDocking as i32;
        /// Disable gamepad/keyboard navigation and focusing
        const NO_NAV = sys::ImGuiWindowFlags_NoNav as i32;
        /// Disable all window decorations
        const NO_DECORATION = sys::ImGuiWindowFlags_NoDecoration as i32;
        /// Disable all user interactions
        const NO_INPUTS = sys::ImGuiWindowFlags_NoInputs as i32;
    }
}

#[inline]
pub(crate) fn validate_window_flags(caller: &str, flags: WindowFlags) {
    let unsupported = flags.bits() & !WindowFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiWindowFlags bits: 0x{unsupported:X}"
    );
}

#[cfg(feature = "serde")]
crate::validation::impl_serde_i32_flags!(WindowFlags);

/// Represents a window that can be built
#[must_use]
pub struct Window<'ui> {
    ui: &'ui Ui,
    name: WindowLabel<'ui>,
    opened: Option<&'ui mut bool>,
    flags: WindowFlags,
    size: Option<[f32; 2]>,
    size_condition: Condition,
    size_constraints: Option<([f32; 2], [f32; 2])>,
    pos: Option<[f32; 2]>,
    pos_condition: Condition,
    content_size: Option<[f32; 2]>,
    collapsed: Option<bool>,
    collapsed_condition: Condition,
    focused: Option<bool>,
    bg_alpha: Option<f32>,
    scroll: Option<[f32; 2]>,
}

impl<'ui> Window<'ui> {
    /// Creates a new window builder
    #[inline]
    pub fn new(ui: &'ui Ui, name: impl Into<WindowLabel<'ui>>) -> Self {
        Self {
            ui,
            name: name.into(),
            opened: None,
            flags: WindowFlags::empty(),
            size: None,
            size_condition: Condition::Always,
            size_constraints: None,
            pos: None,
            pos_condition: Condition::Always,
            content_size: None,
            collapsed: None,
            collapsed_condition: Condition::Always,
            focused: None,
            bg_alpha: None,
            scroll: None,
        }
    }

    /// Sets window flags
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Controls whether the window is open (adds a title-bar close button).
    ///
    /// In Dear ImGui, a window is "closed" by the user by toggling the `p_open` boolean.
    /// When the close button (X) is pressed, `opened` will be set to `false`.
    ///
    /// Note: as an immediate-mode UI, you should stop submitting this window when
    /// `*opened == false` (typically by guarding the `window(...).build(...)` call).
    #[doc(alias = "Begin")]
    pub fn opened(mut self, opened: &'ui mut bool) -> Self {
        self.opened = Some(opened);
        self
    }

    /// Sets window size
    #[doc(alias = "SetNextWindowSize")]
    pub fn size(mut self, size: impl Into<[f32; 2]>, condition: Condition) -> Self {
        self.size = Some(size.into());
        self.size_condition = condition;
        self
    }

    /// Sets window size constraints for the next Begin call.
    ///
    /// This is a convenience wrapper over `ImGui::SetNextWindowSizeConstraints`
    /// without a custom size callback.
    #[doc(alias = "SetNextWindowSizeConstraints")]
    pub fn size_constraints(mut self, min: impl Into<[f32; 2]>, max: impl Into<[f32; 2]>) -> Self {
        self.size_constraints = Some((min.into(), max.into()));
        self
    }

    /// Sets window position
    #[doc(alias = "SetNextWindowPos")]
    pub fn position(mut self, pos: impl Into<[f32; 2]>, condition: Condition) -> Self {
        self.pos = Some(pos.into());
        self.pos_condition = condition;
        self
    }

    /// Sets window content size
    #[doc(alias = "SetNextWindowContentSize")]
    pub fn content_size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.content_size = Some(size.into());
        self
    }

    /// Sets window collapsed state
    #[doc(alias = "SetNextWindowCollapsed")]
    pub fn collapsed(mut self, collapsed: bool, condition: Condition) -> Self {
        self.collapsed = Some(collapsed);
        self.collapsed_condition = condition;
        self
    }

    /// Sets window focused state
    #[doc(alias = "SetNextWindowFocus")]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self
    }

    /// Sets window background alpha
    #[doc(alias = "SetNextWindowBgAlpha")]
    pub fn bg_alpha(mut self, alpha: f32) -> Self {
        self.bg_alpha = Some(alpha);
        self
    }

    /// Sets the initial scroll position for the next Begin call.
    #[doc(alias = "SetNextWindowScroll")]
    pub fn scroll(mut self, scroll: impl Into<[f32; 2]>) -> Self {
        self.scroll = Some(scroll.into());
        self
    }

    /// Builds the window and calls the closure while the window's Context is current.
    ///
    /// # Panics
    ///
    /// Panics before staging any native window options if the plain title is empty,
    /// geometry is non-finite, or the window flags are unsupported.
    pub fn build<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        self.validate();
        self.ui.with_bound_context(|| {
            let window = self.begin_bound()?;
            let result = f();
            drop(window);
            Some(result)
        })
    }

    #[inline]
    fn validate(&self) {
        const CALLER: &str = "Window::build()";
        assert!(
            !matches!(&self.name, WindowLabel::Plain(title) if title.is_empty()),
            "{CALLER} requires a non-empty plain window title"
        );
        validate_window_flags(CALLER, self.flags);
        if let Some(size) = self.size {
            assert_finite_vec2(CALLER, "size", size);
        }
        if let Some((min, max)) = self.size_constraints {
            assert_finite_vec2(CALLER, "minimum size constraint", min);
            assert_finite_vec2(CALLER, "maximum size constraint", max);
        }
        if let Some(pos) = self.pos {
            assert_finite_vec2(CALLER, "position", pos);
        }
        if let Some(content_size) = self.content_size {
            assert_finite_vec2(CALLER, "content size", content_size);
        }
        if let Some(alpha) = self.bg_alpha {
            assert_finite_f32(CALLER, "background alpha", alpha);
        }
        if let Some(scroll) = self.scroll {
            assert_finite_vec2(CALLER, "scroll", scroll);
        }
    }

    #[inline]
    fn begin_bound(mut self) -> Option<WindowGuard<'ui>> {
        unsafe {
            if let Some(size) = self.size {
                sys::igSetNextWindowSize(size.into(), self.size_condition as i32);
            }
            if let Some((min, max)) = self.size_constraints {
                sys::igSetNextWindowSizeConstraints(
                    min.into(),
                    max.into(),
                    None,
                    std::ptr::null_mut(),
                );
            }
            if let Some(pos) = self.pos {
                sys::igSetNextWindowPos(pos.into(), self.pos_condition as i32, [0.0, 0.0].into());
            }
            if let Some(content_size) = self.content_size {
                sys::igSetNextWindowContentSize(content_size.into());
            }
            if let Some(collapsed) = self.collapsed {
                sys::igSetNextWindowCollapsed(collapsed, self.collapsed_condition as i32);
            }
            if self.focused == Some(true) {
                sys::igSetNextWindowFocus();
            }
            if let Some(alpha) = self.bg_alpha {
                sys::igSetNextWindowBgAlpha(alpha);
            }
            if let Some(scroll) = self.scroll {
                sys::igSetNextWindowScroll(scroll.into());
            }

            let open = self
                .opened
                .as_deref_mut()
                .map_or(std::ptr::null_mut(), std::ptr::from_mut);
            let begin = |name| sys::igBegin(name, open, self.flags.bits());
            let result = match &self.name {
                WindowLabel::Plain(title) => self.ui.with_scratch_c_string(title, begin),
                WindowLabel::Keyed { key, title } => self.ui.with_scratch_buffer(|buffer| {
                    begin(buffer.scratch_txt_concat(&[title, "###", key.stable_id()]))
                }),
            };
            if result && self.opened.is_none_or(|opened| *opened) {
                Some(WindowGuard::new(self.ui))
            } else {
                sys::igEnd();
                None
            }
        }
    }
}

/// Internal guard representing an active window.
#[doc(alias = "End")]
struct WindowGuard<'ui> {
    scope: crate::scope::NativeScopeToken<'ui>,
}

impl<'ui> WindowGuard<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(crate::scope::NativeScopePop::EndWindow, "WindowGuard"),
        }
    }
}

impl Drop for WindowGuard<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
