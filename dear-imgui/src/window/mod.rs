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
use std::borrow::Cow;
use std::f32;

use crate::sys;
use crate::{Condition, Ui};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use child_window::*;

pub(crate) mod child_window;
pub(crate) mod content_region;
pub(crate) mod scroll;

// Window-focused/hovered helpers are available via utils.rs variants.
// Window hovered/focused flag helpers are provided by crate::utils::HoveredFlags.

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
        /// Indicate this window is a dock node host. Generally set by imgui internally when hosting a DockSpace.
        const DOCK_NODE_HOST = sys::ImGuiWindowFlags_DockNodeHost as i32;
        /// Disable gamepad/keyboard navigation and focusing
        const NO_NAV = Self::NO_NAV_INPUTS.bits() | Self::NO_NAV_FOCUS.bits();
        /// Disable all window decorations
        const NO_DECORATION = Self::NO_TITLE_BAR.bits() | Self::NO_RESIZE.bits() | Self::NO_SCROLLBAR.bits() | Self::NO_COLLAPSE.bits();
        /// Disable all user interactions
        const NO_INPUTS = Self::NO_MOUSE_INPUTS.bits() | Self::NO_NAV_INPUTS.bits();
    }
}

#[cfg(feature = "serde")]
impl Serialize for WindowFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for WindowFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(WindowFlags::from_bits_truncate(bits))
    }
}

/// Represents a window that can be built
pub struct Window<'ui> {
    ui: &'ui Ui,
    name: Cow<'ui, str>,
    opened: Option<&'ui mut bool>,
    flags: WindowFlags,
    size: Option<[f32; 2]>,
    size_condition: Condition,
    pos: Option<[f32; 2]>,
    pos_condition: Condition,
    pos_pivot: [f32; 2],
    content_size: Option<[f32; 2]>,
    content_size_condition: Condition,
    collapsed: Option<bool>,
    collapsed_condition: Condition,
    focused: Option<bool>,
    bg_alpha: Option<f32>,
}

impl<'ui> Window<'ui> {
    /// Creates a new window builder
    pub fn new(ui: &'ui Ui, name: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            name: name.into(),
            opened: None,
            flags: WindowFlags::empty(),
            size: None,
            size_condition: Condition::Always,
            pos: None,
            pos_condition: Condition::Always,
            pos_pivot: [0.0, 0.0],
            content_size: None,
            content_size_condition: Condition::Always,
            collapsed: None,
            collapsed_condition: Condition::Always,
            focused: None,
            bg_alpha: None,
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
    pub fn size(mut self, size: [f32; 2], condition: Condition) -> Self {
        self.size = Some(size);
        self.size_condition = condition;
        self
    }

    /// Sets window position
    pub fn position(mut self, pos: [f32; 2], condition: Condition) -> Self {
        self.pos = Some(pos);
        self.pos_condition = condition;
        self
    }

    /// Sets window position pivot
    pub fn position_pivot(mut self, pivot: [f32; 2]) -> Self {
        self.pos_pivot = pivot;
        self
    }

    /// Sets window content size
    pub fn content_size(mut self, size: [f32; 2], condition: Condition) -> Self {
        self.content_size = Some(size);
        self.content_size_condition = condition;
        self
    }

    /// Sets window collapsed state
    pub fn collapsed(mut self, collapsed: bool, condition: Condition) -> Self {
        self.collapsed = Some(collapsed);
        self.collapsed_condition = condition;
        self
    }

    /// Sets window focused state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self
    }

    /// Sets window background alpha
    pub fn bg_alpha(mut self, alpha: f32) -> Self {
        self.bg_alpha = Some(alpha);
        self
    }

    /// Sets the title bar
    pub fn title_bar(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_TITLE_BAR, !value);
        self
    }

    /// Sets resizing with the lower-right grip
    pub fn resizable(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_RESIZE, !value);
        self
    }

    /// Sets moving the window
    pub fn movable(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_MOVE, !value);
        self
    }

    /// Sets scrollbars (scrolling is still possible with the mouse or programmatically)
    pub fn scroll_bar(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_SCROLLBAR, !value);
        self
    }

    /// Sets vertical scrolling with the mouse wheel
    pub fn scrollable(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_SCROLL_WITH_MOUSE, !value);
        self
    }

    /// Sets collapsing the window by double-clicking it
    pub fn collapsible(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_COLLAPSE, !value);
        self
    }

    /// Sets resizing the window to its content on every frame
    pub fn always_auto_resize(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::ALWAYS_AUTO_RESIZE, value);
        self
    }

    /// Sets drawing of background color and outside border
    pub fn draw_background(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_BACKGROUND, !value);
        self
    }

    /// Sets loading and saving of settings (e.g. from/to an .ini file)
    pub fn save_settings(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_SAVED_SETTINGS, !value);
        self
    }

    /// Sets catching mouse input.
    pub fn mouse_inputs(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_MOUSE_INPUTS, !value);
        self
    }

    /// Sets the menu bar
    pub fn menu_bar(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::MENU_BAR, value);
        self
    }

    /// Sets the horizontal scrollbar
    pub fn horizontal_scrollbar(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::HORIZONTAL_SCROLLBAR, value);
        self
    }

    /// Sets taking focus when transitioning from hidden to visible state
    pub fn focus_on_appearing(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_FOCUS_ON_APPEARING, !value);
        self
    }

    /// Sets bringing the window to front when taking focus (e.g. clicking it or programmatically
    /// giving it focus).
    pub fn bring_to_front_on_focus(mut self, value: bool) -> Self {
        self.flags
            .set(WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS, !value);
        self
    }

    /// When enabled, forces the vertical scrollbar to render regardless of the content size
    pub fn always_vertical_scrollbar(mut self, value: bool) -> Self {
        self.flags
            .set(WindowFlags::ALWAYS_VERTICAL_SCROLLBAR, value);
        self
    }

    /// When enabled, forces the horizontal scrollbar to render regardless of the content size
    pub fn always_horizontal_scrollbar(mut self, value: bool) -> Self {
        self.flags
            .set(WindowFlags::ALWAYS_HORIZONTAL_SCROLLBAR, value);
        self
    }

    /// Sets gamepad/keyboard navigation within the window
    pub fn nav_inputs(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_NAV_INPUTS, !value);
        self
    }

    /// Sets focusing toward this window with gamepad/keyboard navigation (e.g. CTRL+TAB)
    pub fn nav_focus(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::NO_NAV_FOCUS, !value);
        self
    }
    /// When enabled, appends '*' to title without affecting the ID, as a convenience
    pub fn unsaved_document(mut self, value: bool) -> Self {
        self.flags.set(WindowFlags::UNSAVED_DOCUMENT, value);
        self
    }

    /// Disable gamepad/keyboard navigation and focusing
    pub fn no_nav(mut self) -> Self {
        self.flags |= WindowFlags::NO_NAV;
        self
    }

    /// Disable all window decorations
    pub fn no_decoration(mut self) -> Self {
        self.flags |= WindowFlags::NO_DECORATION;
        self
    }

    /// Disable input handling
    pub fn no_inputs(mut self) -> Self {
        self.flags |= WindowFlags::NO_INPUTS;
        self
    }

    /// Builds the window and calls the provided closure
    pub fn build<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let _token = self.begin()?;
        Some(f())
    }

    /// Begins the window and returns a token
    fn begin(self) -> Option<WindowToken<'ui>> {
        let name = self.name;
        let name_ptr = self.ui.scratch_txt(name);

        // Set window properties before beginning
        if let Some(size) = self.size {
            unsafe {
                let size_vec = crate::sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                };
                crate::sys::igSetNextWindowSize(size_vec, self.size_condition as i32);
            }
        }

        if let Some(pos) = self.pos {
            unsafe {
                let pos_vec = crate::sys::ImVec2 {
                    x: pos[0],
                    y: pos[1],
                };

                let pivot_vec = crate::sys::ImVec2 {
                    x: self.pos_pivot[0],
                    y: self.pos_pivot[1],
                };

                crate::sys::igSetNextWindowPos(pos_vec, self.pos_condition as i32, pivot_vec);
            }
        }

        if let Some(content_size) = self.content_size {
            unsafe {
                let content_size_vec = crate::sys::ImVec2 {
                    x: content_size[0],
                    y: content_size[1],
                };
                crate::sys::igSetNextWindowContentSize(content_size_vec);
            }
        }

        if let Some(collapsed) = self.collapsed {
            unsafe {
                crate::sys::igSetNextWindowCollapsed(collapsed, self.collapsed_condition as i32);
            }
        }

        if let Some(focused) = self.focused
            && focused
        {
            unsafe {
                crate::sys::igSetNextWindowFocus();
            }
        }

        if let Some(alpha) = self.bg_alpha {
            unsafe {
                crate::sys::igSetNextWindowBgAlpha(alpha);
            }
        }

        // Begin the window
        let mut opened = self.opened;
        let opened_ptr: *mut bool = match opened.as_deref_mut() {
            Some(opened) => opened as *mut bool,
            None => std::ptr::null_mut(),
        };
        let result = unsafe { crate::sys::igBegin(name_ptr, opened_ptr, self.flags.bits()) };
        let is_open = opened.is_none_or(|opened| *opened);

        // IMPORTANT: According to ImGui documentation, Begin/End calls must be balanced.
        // If Begin returns false, we need to call End immediately and return None.
        if result && is_open {
            Some(WindowToken {
                _phantom: std::marker::PhantomData,
            })
        } else {
            // If Begin returns false, call End immediately and return None
            unsafe {
                crate::sys::igEnd();
            }
            None
        }
    }
}

/// Token representing an active window
pub struct WindowToken<'ui> {
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> Drop for WindowToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            crate::sys::igEnd();
        }
    }
}
