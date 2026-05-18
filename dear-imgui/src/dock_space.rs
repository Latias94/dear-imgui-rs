#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
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

use crate::Id;
use crate::sys;
use crate::ui::Ui;
use std::ptr;

bitflags::bitflags! {
    /// Flags for dock nodes
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

pub(crate) fn validate_dock_node_flags(caller: &str, flags: DockNodeFlags) {
    let unsupported = flags.bits() & !DockNodeFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDockNodeFlags bits: 0x{unsupported:X}"
    );
}

pub(crate) fn assert_nonzero_id(caller: &str, name: &str, id: Id) {
    assert!(id.raw() != 0, "{caller} {name} must be non-zero");
}

fn optional_nonzero_id_raw(caller: &str, name: &str, id: Option<Id>) -> sys::ImGuiID {
    id.map_or(0, |id| {
        assert_nonzero_id(caller, name, id);
        id.raw()
    })
}

pub(crate) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(crate) fn assert_positive_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value[0] > 0.0 && value[1] > 0.0,
        "{caller} {name} must contain positive values"
    );
}

/// Parent viewport policy for a docking window class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowClassParentViewport {
    /// Use Dear ImGui's default parent viewport behavior.
    Default,
    /// Request the platform backend to avoid parent-child platform windows.
    NoParent,
    /// Request a specific parent viewport.
    Parent(Id),
}

impl Default for WindowClassParentViewport {
    fn default() -> Self {
        Self::Default
    }
}

impl WindowClassParentViewport {
    fn raw(self, caller: &str) -> sys::ImGuiID {
        match self {
            Self::Default => !0,
            Self::NoParent => 0,
            Self::Parent(id) => {
                assert_nonzero_id(caller, "parent_viewport_id", id);
                id.raw()
            }
        }
    }
}

/// Window class for docking configuration
#[derive(Debug, Clone)]
pub struct WindowClass {
    /// User class ID. `None` means the default unclassed window class.
    pub class_id: Option<Id>,
    /// Hint for the platform backend parent viewport behavior.
    pub parent_viewport: WindowClassParentViewport,
    /// ID of parent window for shortcut focus route evaluation
    pub focus_route_parent_window_id: Option<Id>,
    /// Viewport flags to set when a window of this class owns a viewport.
    pub viewport_flags_override_set: crate::ViewportFlags,
    /// Viewport flags to clear when a window of this class owns a viewport.
    pub viewport_flags_override_clear: crate::ViewportFlags,
    /// Tab item flags to set when a window of this class is submitted into a dock node tab bar.
    pub tab_item_flags_override_set: crate::widget::TabItemOptions,
    /// Dock node flags to set when a window of this class is hosted by a dock node.
    pub dock_node_flags_override_set: DockNodeFlags,
    /// Set to true to enforce single floating windows of this class always having their own docking node
    pub docking_always_tab_bar: bool,
    /// Set to true to allow windows of this class to be docked/merged with an unclassed window
    pub docking_allow_unclassed: bool,
    /// Opaque platform-backend icon payload.
    ///
    /// Dear ImGui treats this as backend-owned data. Keep the pointed-to allocation valid for as
    /// long as the platform backend may inspect this window class.
    pub platform_icon_data: Option<ptr::NonNull<std::ffi::c_void>>,
}

impl Default for WindowClass {
    fn default() -> Self {
        Self {
            class_id: None,
            parent_viewport: WindowClassParentViewport::Default,
            focus_route_parent_window_id: None,
            viewport_flags_override_set: crate::ViewportFlags::NONE,
            viewport_flags_override_clear: crate::ViewportFlags::NONE,
            tab_item_flags_override_set: crate::widget::TabItemOptions::new(),
            dock_node_flags_override_set: DockNodeFlags::NONE,
            docking_always_tab_bar: false,
            docking_allow_unclassed: true,
            platform_icon_data: None,
        }
    }
}

impl WindowClass {
    /// Creates a new window class with the specified class ID
    pub fn new(class_id: Id) -> Self {
        assert_nonzero_id("WindowClass::new()", "class_id", class_id);
        Self {
            class_id: Some(class_id),
            ..Default::default()
        }
    }

    /// Sets the parent viewport policy.
    pub fn parent_viewport(mut self, parent: WindowClassParentViewport) -> Self {
        self.parent_viewport = parent;
        self
    }

    /// Requests the platform backend to avoid parenting this class's platform windows.
    pub fn no_parent_viewport(mut self) -> Self {
        self.parent_viewport = WindowClassParentViewport::NoParent;
        self
    }

    /// Requests a specific parent viewport ID.
    pub fn parent_viewport_id(mut self, id: Id) -> Self {
        assert_nonzero_id("WindowClass::parent_viewport_id()", "id", id);
        self.parent_viewport = WindowClassParentViewport::Parent(id);
        self
    }

    /// Sets the focus route parent window ID
    pub fn focus_route_parent_window_id(mut self, id: Id) -> Self {
        assert_nonzero_id("WindowClass::focus_route_parent_window_id()", "id", id);
        self.focus_route_parent_window_id = Some(id);
        self
    }

    /// Sets viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_override_set(mut self, flags: crate::ViewportFlags) -> Self {
        self.viewport_flags_override_set = flags;
        self
    }

    /// Clears viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_override_clear(mut self, flags: crate::ViewportFlags) -> Self {
        self.viewport_flags_override_clear = flags;
        self
    }

    /// Sets and clears viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_overrides(
        mut self,
        set: crate::ViewportFlags,
        clear: crate::ViewportFlags,
    ) -> Self {
        self.viewport_flags_override_set = set;
        self.viewport_flags_override_clear = clear;
        self
    }

    /// Sets tab item flags when a window of this class is submitted into a dock node tab bar.
    pub fn tab_item_flags_override_set(
        mut self,
        options: impl Into<crate::widget::TabItemOptions>,
    ) -> Self {
        self.tab_item_flags_override_set = options.into();
        self
    }

    /// Sets dock node flags when a window of this class is hosted by a dock node.
    pub fn dock_node_flags_override_set(mut self, flags: DockNodeFlags) -> Self {
        self.dock_node_flags_override_set = flags;
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

    /// Sets opaque icon data consumed by the platform backend.
    ///
    /// # Safety
    ///
    /// `data` must remain valid for as long as the platform backend may read it, and it must point
    /// to the representation expected by that backend.
    pub unsafe fn platform_icon_data_raw(mut self, data: *mut std::ffi::c_void) -> Self {
        self.platform_icon_data = ptr::NonNull::new(data);
        self
    }

    fn validate(&self, caller: &str) {
        crate::io::validate_viewport_flags(
            caller,
            self.viewport_flags_override_set | self.viewport_flags_override_clear,
        );
        let overlap =
            self.viewport_flags_override_set.bits() & self.viewport_flags_override_clear.bits();
        assert!(
            overlap == 0,
            "{caller} cannot set and clear the same ImGuiViewportFlags bits: 0x{overlap:X}"
        );
        self.tab_item_flags_override_set
            .validate_for_tab_item(caller);
        validate_dock_node_flags(caller, self.dock_node_flags_override_set);
    }

    /// Converts to ImGui's internal representation
    fn to_imgui(&self, caller: &str) -> sys::ImGuiWindowClass {
        self.validate(caller);
        sys::ImGuiWindowClass {
            ClassId: optional_nonzero_id_raw(caller, "class_id", self.class_id),
            ParentViewportId: self.parent_viewport.raw(caller),
            FocusRouteParentWindowId: optional_nonzero_id_raw(
                caller,
                "focus_route_parent_window_id",
                self.focus_route_parent_window_id,
            ),
            ViewportFlagsOverrideSet: self.viewport_flags_override_set.bits(),
            ViewportFlagsOverrideClear: self.viewport_flags_override_clear.bits(),
            TabItemFlagsOverrideSet: self.tab_item_flags_override_set.bits(),
            DockNodeFlagsOverrideSet: self.dock_node_flags_override_set.bits(),
            DockingAlwaysTabBar: self.docking_always_tab_bar,
            DockingAllowUnclassed: self.docking_allow_unclassed,
            PlatformIconData: self
                .platform_icon_data
                .map_or(ptr::null_mut(), ptr::NonNull::as_ptr),
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
    ///     0.into(),
    ///     DockNodeFlags::PASSTHRU_CENTRAL_NODE
    /// );
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport_with_flags(
        &self,
        dockspace_id: Id,
        flags: DockNodeFlags,
    ) -> Id {
        validate_dock_node_flags("Ui::dockspace_over_main_viewport_with_flags()", flags);
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
        self.dockspace_over_main_viewport_with_flags(
            Id::from(0u32),
            DockNodeFlags::PASSTHRU_CENTRAL_NODE,
        )
    }

    /// Creates a dockspace with the specified ID, size, and flags
    ///
    /// # Parameters
    ///
    /// * `id` - The non-zero ID for the dockspace. Use [`Ui::get_id`] to create one.
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
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space_with_class(
    ///     dockspace_id,
    ///     [800.0, 600.0],
    ///     DockNodeFlags::NO_DOCKING_SPLIT,
    ///     Some(&WindowClass::new(Id::from(1u32)))
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
        validate_dock_node_flags("Ui::dock_space_with_class()", flags);
        assert_nonzero_id("Ui::dock_space_with_class()", "id", id);
        assert_finite_vec2("Ui::dock_space_with_class()", "size", size);
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            let imgui_window_class =
                window_class.map(|class| class.to_imgui("Ui::dock_space_with_class()"));
            let window_class_ptr = imgui_window_class
                .as_ref()
                .map_or(ptr::null(), |wc| wc as *const _);
            Id::from(sys::igDockSpace(
                id.into(),
                size_vec,
                flags.bits(),
                window_class_ptr,
            ))
        }
    }

    /// Creates a dockspace with the specified ID and size
    ///
    /// # Parameters
    ///
    /// * `id` - The non-zero ID for the dockspace
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
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space(dockspace_id, [800.0, 600.0]);
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
    /// let window_class = WindowClass::new(Id::from(1u32)).docking_always_tab_bar(true);
    /// ui.set_next_window_class(&window_class);
    /// ui.window("Classed Window").build(|| {
    ///     ui.text("This window has a custom class!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowClass")]
    pub fn set_next_window_class(&self, window_class: &WindowClass) {
        unsafe {
            let imgui_wc = window_class.to_imgui("Ui::set_next_window_class()");
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
    ///     if dock_id != 0.into() {
    ///         ui.text(format!("This window is docked with ID: {}", dock_id.raw()));
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

// Re-export DockNodeFlags for convenience
pub use DockNodeFlags as DockFlags;
