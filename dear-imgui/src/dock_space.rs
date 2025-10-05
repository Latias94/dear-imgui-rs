#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
//! Docking space functionality for Dear ImGui
//!
//! This module provides high-level Rust bindings for Dear ImGui's docking system,
//! allowing you to create dockable windows and manage dock spaces.
//!
//! # Notes
//!
//! Docking is always enabled in this crate; no feature flag required.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a dockspace over the main viewport
//! let dockspace_id = ui.dockspace_over_main_viewport();
//!
//! // Dock a window to the dockspace
//! ui.set_next_window_dock_id(dockspace_id);
//! ui.window("Tool Window").build(|| {
//!     ui.text("This window is docked!");
//! });
//! ```

use crate::sys;
use crate::Id;
use crate::ui::Ui;
use std::ptr;

bitflags::bitflags! {
    /// Flags for dock nodes
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct DockNodeFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiDockNodeFlags_None as i32;
        /// Don't display the dockspace node but keep it alive. Windows docked into this dockspace node won't be undocked.
        const KEEP_ALIVE_ONLY = sys::ImGuiDockNodeFlags_KeepAliveOnly as i32;
        /// Disable docking over the Central Node, which will be always kept empty.
        const NO_DOCKING_OVER_CENTRAL_NODE = sys::ImGuiDockNodeFlags_NoDockingOverCentralNode as i32;
        /// Enable passthru dockspace: 1) DockSpace() will render a ImGuiCol_WindowBg background covering everything excepted the Central Node when empty. 2) When Central Node is empty: let inputs pass-through + won't display a DockingEmptyBg background.
        const PASSTHRU_CENTRAL_NODE = sys::ImGuiDockNodeFlags_PassthruCentralNode as i32;
        /// Disable other windows/nodes from splitting this node.
        const NO_DOCKING_SPLIT = sys::ImGuiDockNodeFlags_NoDockingSplit as i32;
        /// Disable resizing node using the splitter/separators. Useful with programmatically setup dockspaces.
        const NO_RESIZE = sys::ImGuiDockNodeFlags_NoResize as i32;
        /// Tab bar will automatically hide when there is a single window in the dock node.
        const AUTO_HIDE_TAB_BAR = sys::ImGuiDockNodeFlags_AutoHideTabBar as i32;
        /// Disable undocking this node.
        const NO_UNDOCKING = sys::ImGuiDockNodeFlags_NoUndocking as i32;
    }
}

/// Window class for docking configuration
#[derive(Debug, Clone)]
pub struct WindowClass {
    /// User data. 0 = Default class (unclassed). Windows of different classes cannot be docked with each others.
    pub class_id: sys::ImGuiID,
    /// Hint for the platform backend. -1: use default. 0: request platform backend to not parent the platform. != 0: request platform backend to create a parent<>child relationship between the platform windows.
    pub parent_viewport_id: sys::ImGuiID,
    /// ID of parent window for shortcut focus route evaluation
    pub focus_route_parent_window_id: sys::ImGuiID,
    /// Set to true to enforce single floating windows of this class always having their own docking node
    pub docking_always_tab_bar: bool,
    /// Set to true to allow windows of this class to be docked/merged with an unclassed window
    pub docking_allow_unclassed: bool,
}

impl Default for WindowClass {
    fn default() -> Self {
        Self {
            class_id: 0,
            parent_viewport_id: !0, // -1 as u32
            focus_route_parent_window_id: 0,
            docking_always_tab_bar: false,
            docking_allow_unclassed: true,
        }
    }
}

impl WindowClass {
    /// Creates a new window class with the specified class ID
    pub fn new(class_id: sys::ImGuiID) -> Self {
        Self {
            class_id,
            ..Default::default()
        }
    }

    /// Sets the parent viewport ID
    pub fn parent_viewport_id(mut self, id: sys::ImGuiID) -> Self {
        self.parent_viewport_id = id;
        self
    }

    /// Sets the focus route parent window ID
    pub fn focus_route_parent_window_id(mut self, id: sys::ImGuiID) -> Self {
        self.focus_route_parent_window_id = id;
        self
    }

    /// Enables always showing tab bar for single floating windows
    pub fn docking_always_tab_bar(mut self, enabled: bool) -> Self {
        self.docking_always_tab_bar = enabled;
        self
    }

    /// Allows docking with unclassed windows
    pub fn docking_allow_unclassed(mut self, enabled: bool) -> Self {
        self.docking_allow_unclassed = enabled;
        self
    }

    /// Converts to ImGui's internal representation
    fn to_imgui(&self) -> sys::ImGuiWindowClass {
        sys::ImGuiWindowClass {
            ClassId: self.class_id,
            ParentViewportId: self.parent_viewport_id,
            FocusRouteParentWindowId: self.focus_route_parent_window_id,
            ViewportFlagsOverrideSet: 0,
            ViewportFlagsOverrideClear: 0,
            TabItemFlagsOverrideSet: 0,
            DockNodeFlagsOverrideSet: 0,
            DockingAlwaysTabBar: self.docking_always_tab_bar,
            DockingAllowUnclassed: self.docking_allow_unclassed,
        }
    }
}

/// Docking-related functionality
impl Ui {
    /// Creates a dockspace over the main viewport
    ///
    /// This is a convenience function that creates a dockspace covering the entire main viewport.
    /// It's equivalent to calling `dock_space` with the main viewport's ID and size.
    ///
    /// # Parameters
    ///
    /// * `dockspace_id` - The ID for the dockspace (use 0 to auto-generate)
    /// * `flags` - Dock node flags
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
    ///     0,
    ///     DockNodeFlags::PASSTHRU_CENTRAL_NODE
    /// );
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport_with_flags(
        &self,
        dockspace_id: Id,
        flags: DockNodeFlags,
    ) -> Id {
        unsafe {
            Id::from(sys::igDockSpaceOverViewport(
                dockspace_id.into(),
                sys::igGetMainViewport(),
                flags.bits(),
                ptr::null(),
            ))
        }
    }

    /// Creates a dockspace over the main viewport with default settings
    ///
    /// This is a convenience function that creates a dockspace covering the entire main viewport
    /// with passthrough central node enabled.
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport(&self) -> Id {
        self.dockspace_over_main_viewport_with_flags(Id::from(0u32), DockNodeFlags::PASSTHRU_CENTRAL_NODE)
    }

    /// Creates a dockspace with the specified ID, size, and flags
    ///
    /// # Parameters
    ///
    /// * `id` - The ID for the dockspace (use 0 to auto-generate)
    /// * `size` - The size of the dockspace in pixels
    /// * `flags` - Dock node flags
    /// * `window_class` - Optional window class for docking configuration
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dock_space_with_class(
    ///     0,
    ///     [800.0, 600.0],
    ///     DockNodeFlags::NO_DOCKING_SPLIT,
    ///     Some(&WindowClass::new(1))
    /// );
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space_with_class(
        &self,
        id: Id,
        size: [f32; 2],
        flags: DockNodeFlags,
        window_class: Option<&WindowClass>,
    ) -> Id {
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            let window_class_ptr = if let Some(wc) = window_class {
                let imgui_wc = wc.to_imgui();
                &imgui_wc as *const _
            } else {
                ptr::null()
            };
            Id::from(sys::igDockSpace(id.into(), size_vec, flags.bits(), window_class_ptr))
        }
    }

    /// Creates a dockspace with the specified ID and size
    ///
    /// # Parameters
    ///
    /// * `id` - The ID for the dockspace (use 0 to auto-generate)
    /// * `size` - The size of the dockspace in pixels
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dock_space(0, [800.0, 600.0]);
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space(&self, id: Id, size: [f32; 2]) -> Id {
        self.dock_space_with_class(id, size, DockNodeFlags::NONE, None)
    }

    /// Sets the dock ID for the next window with condition
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    ///
    /// # Parameters
    ///
    /// * `dock_id` - The ID of the dock node to dock the next window to
    /// * `cond` - Condition for when to apply the docking
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id_with_cond(&self, dock_id: Id, cond: crate::Condition) {
        unsafe {
            sys::igSetNextWindowDockID(dock_id.into(), cond as i32);
        }
    }

    /// Sets the dock ID for the next window
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    /// Uses `Condition::Always` by default.
    ///
    /// # Parameters
    ///
    /// * `dock_id` - The ID of the dock node to dock the next window to
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ui.set_next_window_dock_id(dockspace_id);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id(&self, dock_id: Id) {
        self.set_next_window_dock_id_with_cond(dock_id, crate::Condition::Always)
    }

    /// Sets the window class for the next window
    ///
    /// This function must be called before creating a window to apply the window class configuration.
    ///
    /// # Parameters
    ///
    /// * `window_class` - The window class configuration
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let window_class = WindowClass::new(1).docking_always_tab_bar(true);
    /// ui.set_next_window_class(&window_class);
    /// ui.window("Classed Window").build(|| {
    ///     ui.text("This window has a custom class!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowClass")]
    pub fn set_next_window_class(&self, window_class: &WindowClass) {
        unsafe {
            let imgui_wc = window_class.to_imgui();
            sys::igSetNextWindowClass(&imgui_wc as *const _);
        }
    }

    /// Gets the dock ID of the current window
    ///
    /// # Returns
    ///
    /// The dock ID of the current window, or 0 if the window is not docked
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     let dock_id = ui.get_window_dock_id();
    ///     if dock_id != 0 {
    ///         ui.text(format!("This window is docked with ID: {}", dock_id));
    ///     } else {
    ///         ui.text("This window is not docked");
    ///     }
    /// });
    /// ```
    #[doc(alias = "GetWindowDockID")]
    pub fn get_window_dock_id(&self) -> Id {
        unsafe { Id::from(sys::igGetWindowDockID()) }
    }

    /// Checks if the current window is docked
    ///
    /// # Returns
    ///
    /// `true` if the current window is docked, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     if ui.is_window_docked() {
    ///         ui.text("This window is docked!");
    ///     } else {
    ///         ui.text("This window is floating");
    ///     }
    /// });
    /// ```
    #[doc(alias = "IsWindowDocked")]
    pub fn is_window_docked(&self) -> bool {
        unsafe { sys::igIsWindowDocked() }
    }
}

impl crate::Condition {
    /// Converts the condition to ImGui's internal representation
    pub(crate) fn as_imgui_cond(self) -> sys::ImGuiCond {
        self as i32
    }
}

// Re-export DockNodeFlags for convenience
pub use DockNodeFlags as DockFlags;
