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
//! Child window example:
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
use bitflags::bitflags;
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use std::f32;

use crate::error::context::ContextError;

use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2};
use crate::{Condition, Id, Ui};

// Window-focused/hovered helpers are available via utils.rs variants.
// Window hovered/focused flag helpers are provided by crate::utils.

bitflags! {
    /// Configuration flags for windows
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowFlags: i32 {
        /// Disable title-bar
        const NO_TITLE_BAR = sys::ImGuiWindowFlags_NoTitleBar;
        /// Disable user resizing with the lower-right grip
        const NO_RESIZE = sys::ImGuiWindowFlags_NoResize;
        /// Disable user moving the window
        const NO_MOVE = sys::ImGuiWindowFlags_NoMove;
        /// Disable scrollbars (window can still scroll with mouse or programmatically)
        const NO_SCROLLBAR = sys::ImGuiWindowFlags_NoScrollbar;
        /// Disable user vertically scrolling with mouse wheel
        const NO_SCROLL_WITH_MOUSE = sys::ImGuiWindowFlags_NoScrollWithMouse;
        /// Disable user collapsing window by double-clicking on it
        const NO_COLLAPSE = sys::ImGuiWindowFlags_NoCollapse;
        /// Resize every window to its content every frame
        const ALWAYS_AUTO_RESIZE = sys::ImGuiWindowFlags_AlwaysAutoResize;
        /// Disable drawing background color (WindowBg, etc.) and outside border
        const NO_BACKGROUND = sys::ImGuiWindowFlags_NoBackground;
        /// Never load/save settings in .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiWindowFlags_NoSavedSettings;
        /// Disable catching mouse, hovering test with pass through
        const NO_MOUSE_INPUTS = sys::ImGuiWindowFlags_NoMouseInputs;
        /// Has a menu-bar
        const MENU_BAR = sys::ImGuiWindowFlags_MenuBar;
        /// Allow horizontal scrollbar to appear (off by default)
        const HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_HorizontalScrollbar;
        /// Disable taking focus when transitioning from hidden to visible state
        const NO_FOCUS_ON_APPEARING = sys::ImGuiWindowFlags_NoFocusOnAppearing;
        /// Disable bringing window to front when taking focus (e.g. clicking on it or programmatically giving it focus)
        const NO_BRING_TO_FRONT_ON_FOCUS = sys::ImGuiWindowFlags_NoBringToFrontOnFocus;
        /// Always show vertical scrollbar (even if ContentSize.y < Size.y)
        const ALWAYS_VERTICAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysVerticalScrollbar;
        /// Always show horizontal scrollbar (even if ContentSize.x < Size.x)
        const ALWAYS_HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysHorizontalScrollbar;
        /// No gamepad/keyboard navigation within the window
        const NO_NAV_INPUTS = sys::ImGuiWindowFlags_NoNavInputs;
        /// No focusing toward this window with gamepad/keyboard navigation (e.g. skipped by CTRL+TAB)
        const NO_NAV_FOCUS = sys::ImGuiWindowFlags_NoNavFocus;
        /// Display a dot next to the title. When used in a tab/docking context, tab is selected when clicking the X + closure is not assumed (will wait for user to stop submitting the tab). Otherwise closure is assumed when pressing the X, so if you keep submitting the tab may reappear at end of tab bar.
        const UNSAVED_DOCUMENT = sys::ImGuiWindowFlags_UnsavedDocument;
        // Docking related flags
        /// Disable docking for this window (the window will not be able to dock into another and others won't be able to dock into it)
        const NO_DOCKING = sys::ImGuiWindowFlags_NoDocking;
        /// Disable gamepad/keyboard navigation and focusing
        const NO_NAV = sys::ImGuiWindowFlags_NoNav;
        /// Disable all window decorations
        const NO_DECORATION = sys::ImGuiWindowFlags_NoDecoration;
        /// Disable all user interactions
        const NO_INPUTS = sys::ImGuiWindowFlags_NoInputs;
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

fn assert_unit_ratio(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(
        (0.0..=1.0).contains(&value),
        "{caller} {name} must be between 0.0 and 1.0"
    );
}

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
    /// Returns the size of the content region available for widgets
    ///
    /// This is the size of the window minus decorations (title bar, scrollbars, etc.)
    #[doc(alias = "GetContentRegionAvail")]
    pub fn content_region_avail(&self) -> [f32; 2] {
        let size = self.run_with_bound_context(|| unsafe { sys::igGetContentRegionAvail() });
        [size.x, size.y]
    }

    /// Returns the width of the content region available for widgets
    ///
    /// This is equivalent to `content_region_avail()[0]`
    pub fn content_region_avail_width(&self) -> f32 {
        self.content_region_avail()[0]
    }

    /// Returns the height of the content region available for widgets
    ///
    /// This is equivalent to `content_region_avail()[1]`
    pub fn content_region_avail_height(&self) -> f32 {
        self.content_region_avail()[1]
    }

    /// Returns the current scroll position of the window
    #[doc(alias = "GetScrollX")]
    pub fn scroll_x(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetScrollX() })
    }

    /// Returns the current vertical scroll position of the window
    #[doc(alias = "GetScrollY")]
    pub fn scroll_y(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetScrollY() })
    }

    /// Returns the maximum horizontal scroll position
    #[doc(alias = "GetScrollMaxX")]
    pub fn scroll_max_x(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetScrollMaxX() })
    }

    /// Returns the maximum vertical scroll position
    #[doc(alias = "GetScrollMaxY")]
    pub fn scroll_max_y(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetScrollMaxY() })
    }

    /// Sets the horizontal scroll position
    #[doc(alias = "SetScrollX")]
    pub fn set_scroll_x(&self, scroll_x: f32) {
        assert_finite_f32("Ui::set_scroll_x()", "scroll_x", scroll_x);
        self.run_with_bound_context(|| unsafe { sys::igSetScrollX_Float(scroll_x) });
    }

    /// Sets the vertical scroll position
    #[doc(alias = "SetScrollY")]
    pub fn set_scroll_y(&self, scroll_y: f32) {
        assert_finite_f32("Ui::set_scroll_y()", "scroll_y", scroll_y);
        self.run_with_bound_context(|| unsafe { sys::igSetScrollY_Float(scroll_y) });
    }

    /// Sets the horizontal scroll position to center on the given position
    ///
    /// The center_x_ratio parameter should be between 0.0 (left) and 1.0 (right)
    #[doc(alias = "SetScrollFromPosX")]
    pub fn set_scroll_from_pos_x(&self, local_x: f32, center_x_ratio: f32) {
        assert_finite_f32("Ui::set_scroll_from_pos_x()", "local_x", local_x);
        assert_unit_ratio(
            "Ui::set_scroll_from_pos_x()",
            "center_x_ratio",
            center_x_ratio,
        );
        self.run_with_bound_context(|| unsafe {
            sys::igSetScrollFromPosX_Float(local_x, center_x_ratio);
        });
    }

    /// Sets the vertical scroll position to center on the given position
    ///
    /// The center_y_ratio parameter should be between 0.0 (top) and 1.0 (bottom)
    #[doc(alias = "SetScrollFromPosY")]
    pub fn set_scroll_from_pos_y(&self, local_y: f32, center_y_ratio: f32) {
        assert_finite_f32("Ui::set_scroll_from_pos_y()", "local_y", local_y);
        assert_unit_ratio(
            "Ui::set_scroll_from_pos_y()",
            "center_y_ratio",
            center_y_ratio,
        );
        self.run_with_bound_context(|| unsafe {
            sys::igSetScrollFromPosY_Float(local_y, center_y_ratio);
        });
    }

    /// Scrolls to make the current item visible
    ///
    /// This is useful when you want to ensure a specific item is visible in a scrollable region
    #[doc(alias = "SetScrollHereX")]
    pub fn set_scroll_here_x(&self, center_x_ratio: f32) {
        assert_unit_ratio("Ui::set_scroll_here_x()", "center_x_ratio", center_x_ratio);
        self.run_with_bound_context(|| unsafe { sys::igSetScrollHereX(center_x_ratio) });
    }

    /// Scrolls to make the current item visible vertically
    ///
    /// This is useful when you want to ensure a specific item is visible in a scrollable region
    #[doc(alias = "SetScrollHereY")]
    pub fn set_scroll_here_y(&self, center_y_ratio: f32) {
        assert_unit_ratio("Ui::set_scroll_here_y()", "center_y_ratio", center_y_ratio);
        self.run_with_bound_context(|| unsafe { sys::igSetScrollHereY(center_y_ratio) });
    }

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

const ID_SEPARATOR: &str = "###";

#[derive(Debug)]
struct WindowKeyIdentity {
    stable_id: Box<str>,
    default_title: Box<str>,
    docking_name: CString,
    native_id: Id,
}

/// Stable Dear ImGui identity for a top-level window.
///
/// A key stores a default displayed title separately from the identity used by docking and INI
/// persistence. Use the same key in [`DockLayout`](crate::DockLayout) and [`Ui::window`](crate::Ui::window)
/// so a displayed-title change cannot silently create a different native window.
///
/// ```no_run
/// # use dear_imgui_rs::*;
/// # fn draw(ui: &Ui) -> Result<(), ContextError> {
/// let scene = WindowKey::new("scene", "Scene")?;
/// let layout = DockLayout::tabs([&scene]);
///
/// ui.window(scene.label("Scene (Debug)"))
///     .build(|| ui.text("The stable identity is still `scene`."));
/// # let _ = layout;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct WindowKey {
    identity: Arc<WindowKeyIdentity>,
}

impl WindowKey {
    /// Create a validated stable identity and its default displayed title.
    pub fn new(
        stable_id: impl Into<String>,
        default_title: impl Into<String>,
    ) -> Result<Self, ContextError> {
        let stable_id = stable_id.into();
        if stable_id.is_empty() {
            return Err(ContextError::EmptyWindowId);
        }
        if stable_id.as_bytes().contains(&0) {
            return Err(ContextError::WindowIdContainsNul);
        }
        if stable_id.contains(ID_SEPARATOR) {
            return Err(ContextError::WindowIdContainsSeparator);
        }

        let mut docking_name = String::with_capacity(ID_SEPARATOR.len() + stable_id.len());
        docking_name.push_str(ID_SEPARATOR);
        docking_name.push_str(&stable_id);
        let docking_name = CString::new(docking_name)
            .expect("a validated window key must produce a valid native name");
        // SAFETY: `docking_name` is readable and NUL-terminated. ImHashStr is context-free.
        let native_id = Id::from(unsafe { sys::igImHashStr(docking_name.as_ptr(), 0, 0) });
        if native_id.raw() == 0 {
            return Err(ContextError::WindowIdIsZero);
        }

        Ok(Self {
            identity: Arc::new(WindowKeyIdentity {
                stable_id: stable_id.into_boxed_str(),
                default_title: default_title.into().into_boxed_str(),
                docking_name,
                native_id,
            }),
        })
    }

    /// Return the stable identity string.
    pub fn stable_id(&self) -> &str {
        &self.identity.stable_id
    }

    /// Return the default displayed title.
    pub fn default_title(&self) -> &str {
        &self.identity.default_title
    }

    /// Use a different displayed title without changing the stable identity.
    pub fn label<'a>(&'a self, title: impl Into<Cow<'a, str>>) -> WindowLabel<'a> {
        WindowLabel::Keyed {
            key: self,
            title: title.into(),
        }
    }

    pub(crate) fn docking_name(&self) -> &CStr {
        &self.identity.docking_name
    }

    pub(crate) fn native_id(&self) -> Id {
        self.identity.native_id
    }
}

impl PartialEq for WindowKey {
    fn eq(&self, other: &Self) -> bool {
        self.stable_id() == other.stable_id()
    }
}

impl Eq for WindowKey {}

impl Hash for WindowKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.stable_id().hash(state);
    }
}

impl fmt::Debug for WindowKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowKey")
            .field("stable_id", &self.stable_id())
            .field("default_title", &self.default_title())
            .field("native_id", &self.native_id())
            .finish()
    }
}

impl From<&WindowKey> for WindowKey {
    fn from(key: &WindowKey) -> Self {
        key.clone()
    }
}

/// Window label accepted by [`Ui::window`](crate::Ui::window).
///
/// Plain strings retain Dear ImGui's native `##`/`###` behavior. Labels created by
/// [`WindowKey::label`] always append the validated stable identity after the displayed title.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum WindowLabel<'a> {
    Plain(Cow<'a, str>),
    Keyed {
        key: &'a WindowKey,
        title: Cow<'a, str>,
    },
}

impl WindowLabel<'_> {
    /// Return the displayed title supplied to Dear ImGui.
    pub fn title(&self) -> &str {
        match self {
            Self::Plain(title) | Self::Keyed { title, .. } => title,
        }
    }

    /// Return the stable key, when this label is keyed.
    pub fn key(&self) -> Option<&WindowKey> {
        match self {
            Self::Plain(_) => None,
            Self::Keyed { key, .. } => Some(key),
        }
    }
}

impl<'a> From<&'a str> for WindowLabel<'a> {
    fn from(title: &'a str) -> Self {
        Self::Plain(Cow::Borrowed(title))
    }
}

impl<'a> From<&'a String> for WindowLabel<'a> {
    fn from(title: &'a String) -> Self {
        Self::Plain(Cow::Borrowed(title))
    }
}

impl From<String> for WindowLabel<'_> {
    fn from(title: String) -> Self {
        Self::Plain(Cow::Owned(title))
    }
}

impl<'a> From<Cow<'a, str>> for WindowLabel<'a> {
    fn from(title: Cow<'a, str>) -> Self {
        Self::Plain(title)
    }
}

impl<'a> From<&'a WindowKey> for WindowLabel<'a> {
    fn from(key: &'a WindowKey) -> Self {
        Self::Keyed {
            key,
            title: Cow::Borrowed(key.default_title()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_or_invalid_stable_ids() {
        assert!(matches!(
            WindowKey::new("", "Title"),
            Err(ContextError::EmptyWindowId)
        ));
        assert!(matches!(
            WindowKey::new("bad\0id", "Title"),
            Err(ContextError::WindowIdContainsNul)
        ));
        assert!(matches!(
            WindowKey::new("first###second", "Title"),
            Err(ContextError::WindowIdContainsSeparator)
        ));
    }

    #[test]
    fn title_changes_preserve_equality_and_native_identity() {
        let scene = WindowKey::new("scene", "Scene").unwrap();
        let renamed = WindowKey::new("scene", "Scene (Debug)").unwrap();
        assert_eq!(scene, renamed);
        assert_eq!(scene.native_id(), renamed.native_id());
        assert_eq!(scene.docking_name().to_bytes(), b"###scene");
    }
}
