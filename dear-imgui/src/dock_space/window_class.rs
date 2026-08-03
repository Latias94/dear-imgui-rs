use super::flags::WindowClassDockNodeFlags;
use super::validation::assert_nonzero_id;
use crate::{Id, sys};
use std::ptr;
use thiserror::Error;

/// Parent viewport policy for a docking window class.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowClassParentViewport {
    /// Use Dear ImGui's default parent viewport behavior.
    #[default]
    Default,
    /// Request the platform backend to avoid parent-child platform windows.
    NoParent,
    /// Request a specific parent viewport.
    Parent(Id),
}

impl WindowClassParentViewport {
    fn try_raw(self) -> Result<sys::ImGuiID, WindowClassError> {
        match self {
            Self::Default => Ok(!0),
            Self::NoParent => Ok(0),
            Self::Parent(id) if id.raw() != 0 => Ok(id.raw()),
            Self::Parent(_) => Err(WindowClassError::ZeroParentViewportId),
        }
    }
}

/// Validation failure for a [`WindowClass`].
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowClassError {
    #[error("parent viewport ID must be non-zero")]
    ZeroParentViewportId,
    #[error("viewport overrides contain unsupported ImGuiViewportFlags bits: 0x{bits:X}")]
    UnsupportedViewportFlags { bits: i32 },
    #[error("viewport overrides set and clear the same ImGuiViewportFlags bits: 0x{bits:X}")]
    OverlappingViewportFlags { bits: i32 },
    #[error("tab overrides contain unsupported ImGuiTabItemFlags bits: 0x{bits:X}")]
    UnsupportedTabItemFlags { bits: i32 },
    #[error("dock-node overrides contain unsupported ImGuiDockNodeFlags bits: 0x{bits:X}")]
    UnsupportedDockNodeFlags { bits: i32 },
}

/// Window class for docking configuration.
///
/// Native pointer fields are intentionally private so safe code cannot bypass their lifetime
/// contracts.
///
/// ```compile_fail
/// # use dear_imgui_rs::WindowClass;
/// let mut class = WindowClass::default();
/// class.platform_icon_data = None;
/// ```
#[derive(Debug, Clone)]
pub struct WindowClass {
    /// User class ID. `None` means the default unclassed window class.
    class_id: Option<Id>,
    /// Hint for the platform backend parent viewport behavior.
    parent_viewport: WindowClassParentViewport,
    /// ID of parent window for shortcut focus route evaluation
    focus_route_parent_window_id: Option<Id>,
    /// Viewport flags to set when a window of this class owns a viewport.
    viewport_flags_override_set: crate::WindowClassViewportFlags,
    /// Viewport flags to clear when a window of this class owns a viewport.
    viewport_flags_override_clear: crate::WindowClassViewportFlags,
    /// Tab item flags to set when a window of this class is submitted into a dock node tab bar.
    tab_item_flags_override_set: crate::widget::TabItemOptions,
    /// Dock node flags to set when a window of this class is hosted by a dock node.
    dock_node_flags_override_set: WindowClassDockNodeFlags,
    /// Set to true to enforce single floating windows of this class always having their own docking node
    docking_always_tab_bar: bool,
    /// Set to true to allow windows of this class to be docked/merged with an unclassed window
    docking_allow_unclassed: bool,
    /// Opaque platform-backend icon payload.
    ///
    /// Dear ImGui treats this as backend-owned data. Keep the pointed-to allocation valid for as
    /// long as the platform backend may inspect this window class.
    platform_icon_data: Option<ptr::NonNull<std::ffi::c_void>>,
}

impl Default for WindowClass {
    fn default() -> Self {
        Self {
            class_id: None,
            parent_viewport: WindowClassParentViewport::Default,
            focus_route_parent_window_id: None,
            viewport_flags_override_set: crate::WindowClassViewportFlags::empty(),
            viewport_flags_override_clear: crate::WindowClassViewportFlags::empty(),
            tab_item_flags_override_set: crate::widget::TabItemOptions::new(),
            dock_node_flags_override_set: WindowClassDockNodeFlags::NONE,
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

    /// Returns the user class ID, or `None` for the default unclassed class.
    pub fn class_id(&self) -> Option<Id> {
        self.class_id
    }

    /// Returns the platform parent viewport policy.
    pub fn parent_viewport_policy(&self) -> WindowClassParentViewport {
        self.parent_viewport
    }

    /// Returns the raw focus-route parent window ID, when configured.
    pub fn focus_route_parent_window_id_raw(&self) -> Option<Id> {
        self.focus_route_parent_window_id
    }

    /// Returns the viewport flags this class sets.
    pub fn viewport_flags_to_set(&self) -> crate::WindowClassViewportFlags {
        self.viewport_flags_override_set
    }

    /// Returns the viewport flags this class clears.
    pub fn viewport_flags_to_clear(&self) -> crate::WindowClassViewportFlags {
        self.viewport_flags_override_clear
    }

    /// Returns the tab item options this class applies.
    pub fn tab_item_options(&self) -> crate::widget::TabItemOptions {
        self.tab_item_flags_override_set
    }

    /// Returns the dock node flags this class applies.
    pub fn dock_node_flags_to_set(&self) -> WindowClassDockNodeFlags {
        self.dock_node_flags_override_set
    }

    /// Returns whether single floating windows always receive a tab bar.
    pub fn always_tab_bar(&self) -> bool {
        self.docking_always_tab_bar
    }

    /// Returns whether this class may dock with unclassed windows.
    pub fn allows_unclassed(&self) -> bool {
        self.docking_allow_unclassed
    }

    /// Sets the raw parent viewport policy.
    ///
    /// # Safety
    ///
    /// For [`WindowClassParentViewport::Parent`], the target viewport must be live when the
    /// window begins, belong to the same Context, and the resulting parent graph must contain no
    /// self-edge or cycle. Dear ImGui stores a raw parent pointer and traverses it without cycle
    /// detection. The `Default` and `NoParent` policies satisfy these requirements inherently.
    pub unsafe fn parent_viewport(mut self, parent: WindowClassParentViewport) -> Self {
        self.parent_viewport = parent;
        self
    }

    /// Requests the platform backend to avoid parenting this class's platform windows.
    pub fn no_parent_viewport(mut self) -> Self {
        self.parent_viewport = WindowClassParentViewport::NoParent;
        self
    }

    /// Requests a specific raw parent viewport ID.
    ///
    /// # Safety
    ///
    /// The target viewport must be live when the window begins, belong to the same Context, and
    /// the resulting parent graph must contain no self-edge or cycle.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::{Id, WindowClass};
    /// let class = WindowClass::default().parent_viewport_id(Id::from(1u32));
    /// # let _ = class;
    /// ```
    pub unsafe fn parent_viewport_id(mut self, id: Id) -> Self {
        assert_nonzero_id("WindowClass::parent_viewport_id()", "id", id);
        self.parent_viewport = WindowClassParentViewport::Parent(id);
        self
    }

    /// Sets the raw focus-route parent window ID.
    ///
    /// # Safety
    ///
    /// A window with `id` must already exist when a window using this class begins. The resulting
    /// focus route must not contain a cycle and the class must not be applied to the parent window
    /// itself. Dear ImGui assumes these conditions and may dereference the resolved parent without
    /// checking it in non-assert builds.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::{Id, WindowClass};
    /// let class = WindowClass::default().focus_route_parent_window_id(Id::from(1u32));
    /// # let _ = class;
    /// ```
    pub unsafe fn focus_route_parent_window_id(mut self, id: Id) -> Self {
        assert_nonzero_id("WindowClass::focus_route_parent_window_id()", "id", id);
        self.focus_route_parent_window_id = Some(id);
        self
    }

    /// Sets viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_override_set(mut self, flags: crate::WindowClassViewportFlags) -> Self {
        self.viewport_flags_override_set = flags;
        self
    }

    /// Clears viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_override_clear(mut self, flags: crate::WindowClassViewportFlags) -> Self {
        self.viewport_flags_override_clear = flags;
        self
    }

    /// Sets and clears viewport flags when a window of this class owns a viewport.
    pub fn viewport_flags_overrides(
        mut self,
        set: crate::WindowClassViewportFlags,
        clear: crate::WindowClassViewportFlags,
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
    pub fn dock_node_flags_override_set(mut self, flags: WindowClassDockNodeFlags) -> Self {
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

    /// Validate every typed override without touching Dear ImGui state.
    pub fn validate(&self) -> Result<(), WindowClassError> {
        let viewport_bits =
            (self.viewport_flags_override_set | self.viewport_flags_override_clear).bits();
        let unsupported_viewport = viewport_bits & !crate::WindowClassViewportFlags::all().bits();
        if unsupported_viewport != 0 {
            return Err(WindowClassError::UnsupportedViewportFlags {
                bits: unsupported_viewport,
            });
        }
        let overlap =
            self.viewport_flags_override_set.bits() & self.viewport_flags_override_clear.bits();
        if overlap != 0 {
            return Err(WindowClassError::OverlappingViewportFlags { bits: overlap });
        }
        let unsupported_tab =
            self.tab_item_flags_override_set.flags.bits() & !crate::TabItemFlags::all().bits();
        if unsupported_tab != 0 {
            return Err(WindowClassError::UnsupportedTabItemFlags {
                bits: unsupported_tab,
            });
        }
        let unsupported_dock =
            self.dock_node_flags_override_set.bits() & !WindowClassDockNodeFlags::all().bits();
        if unsupported_dock != 0 {
            return Err(WindowClassError::UnsupportedDockNodeFlags {
                bits: unsupported_dock,
            });
        }
        self.parent_viewport.try_raw()?;
        Ok(())
    }

    pub(crate) fn try_to_imgui(&self) -> Result<sys::ImGuiWindowClass, WindowClassError> {
        self.validate()?;
        Ok(sys::ImGuiWindowClass {
            ClassId: self.class_id.map_or(0, Id::raw),
            ParentViewportId: self.parent_viewport.try_raw()?,
            FocusRouteParentWindowId: self.focus_route_parent_window_id.map_or(0, Id::raw),
            ViewportFlagsOverrideSet: self.viewport_flags_override_set.bits(),
            ViewportFlagsOverrideClear: self.viewport_flags_override_clear.bits(),
            TabItemFlagsOverrideSet: self.tab_item_flags_override_set.bits(),
            DockNodeFlagsOverrideSet: self.dock_node_flags_override_set.bits(),
            DockingAlwaysTabBar: self.docking_always_tab_bar,
            DockingAllowUnclassed: self.docking_allow_unclassed,
            PlatformIconData: self
                .platform_icon_data
                .map_or(ptr::null_mut(), ptr::NonNull::as_ptr),
        })
    }

    /// Convert to Dear ImGui's internal representation for infallible direct APIs.
    pub(crate) fn to_imgui(&self, caller: &str) -> sys::ImGuiWindowClass {
        self.try_to_imgui()
            .unwrap_or_else(|error| panic!("{caller}: {error}"))
    }
}
