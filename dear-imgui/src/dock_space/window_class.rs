use super::flags::{DockNodeFlags, validate_dock_node_flags};
use super::validation::{assert_nonzero_id, optional_nonzero_id_raw};
use crate::{Id, sys};
use std::ptr;

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
    pub(super) fn raw(self, caller: &str) -> sys::ImGuiID {
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
    pub(super) fn to_imgui(&self, caller: &str) -> sys::ImGuiWindowClass {
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
