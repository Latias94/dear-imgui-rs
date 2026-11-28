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
//! Docking support is always compiled into this crate; no Cargo feature is required. Set
//! [`ConfigFlags::DOCKING_ENABLE`](crate::ConfigFlags::DOCKING_ENABLE) before the first frame when
//! the Context will use docking. The setting is intentionally stable for that Context's lifetime
//! because Dear ImGui destroys live dock nodes when it is disabled at runtime.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # ctx.io_mut().set_display_size([1280.0, 720.0]);
//! # let flags = ctx.io().config_flags() | ConfigFlags::DOCKING_ENABLE;
//! # ctx.io_mut().set_config_flags(flags);
//! # ctx.font_atlas()
//! #     .try_claim_legacy_renderer()
//! #     .expect("legacy renderer font atlas should be available")
//! #     .build();
//! # let ui = ctx.frame();
//! let tools = WindowKey::new("tools", "Tools")?;
//! let layout = DockLayout::tabs([&tools]);
//! ui.dockspace()
//!     .layout(&layout, DockLayoutApply::IfMissing)
//!     .build()?;
//!
//! ui.window(&tools).build(|| {
//!     ui.text("This window is docked!");
//! });
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The old parallel convenience methods are intentionally absent:
//!
//! ```compile_fail
//! # use dear_imgui_rs::{Context, Id};
//! # let mut context = Context::create();
//! # let ui = context.frame();
//! let _ = ui.dock_space(Id::from(1_u32), [400.0, 300.0]);
//! ```

use std::ffi::{CStr, CString};
use std::ptr;

use crate::dock_layout::{DockspaceConfig, DockspaceHost, submit_and_apply, submit_without_layout};
use crate::error::context::{ContextError, WindowClassKind};
use crate::{DockLayout, DockLayoutApply, Id, Ui, sys};

bitflags::bitflags! {
    /// Flags accepted when submitting a dockspace.
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

bitflags::bitflags! {
    /// Dock-node policies contributed by every window using a [`crate::WindowClass`].
    ///
    /// Dear ImGui combines these flags across all windows in the same dock node. Dockspace
    /// submission controls such as [`DockNodeFlags::KEEP_ALIVE_ONLY`] and
    /// [`DockNodeFlags::PASSTHRU_CENTRAL_NODE`] are intentionally not part of this type.
    /// Several policies mirror version-pinned experimental Dear ImGui node behavior, but remain
    /// memory-safe because they do not accept caller-owned pointers or topology identities.
    ///
    /// These policies are a separate semantic domain from dockspace submission flags:
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::{DockNodeFlags, WindowClass};
    /// let class = WindowClass::default()
    ///     .dock_node_flags_override_set(DockNodeFlags::NO_RESIZE);
    /// # let _ = class;
    /// ```
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowClassDockNodeFlags: i32 {
        /// No dock-node policy.
        const NONE = sys::ImGuiDockNodeFlags_None as i32;
        /// Prevent docking over a central node.
        const NO_DOCKING_OVER_CENTRAL_NODE =
            sys::ImGuiDockNodeFlags_NoDockingOverCentralNode as i32;
        /// Prevent other windows or nodes from splitting this node.
        const NO_DOCKING_SPLIT = sys::ImGuiDockNodeFlags_NoDockingSplit as i32;
        /// Prevent splitter resizing.
        const NO_RESIZE = sys::ImGuiDockNodeFlags_NoResize as i32;
        /// Hide the tab bar automatically when the node contains one window.
        const AUTO_HIDE_TAB_BAR = sys::ImGuiDockNodeFlags_AutoHideTabBar as i32;
        /// Prevent windows from being undocked from this node.
        const NO_UNDOCKING = sys::ImGuiDockNodeFlags_NoUndocking as i32;
        /// Never create a tab bar for this node.
        const NO_TAB_BAR = sys::ImGuiDockNodeFlags_NoTabBar as i32;
        /// Keep the existing tab bar hidden.
        const HIDDEN_TAB_BAR = sys::ImGuiDockNodeFlags_HiddenTabBar as i32;
        /// Hide the tab-bar window menu button.
        const NO_WINDOW_MENU_BUTTON = sys::ImGuiDockNodeFlags_NoWindowMenuButton as i32;
        /// Hide the tab-bar close button.
        const NO_CLOSE_BUTTON = sys::ImGuiDockNodeFlags_NoCloseButton as i32;
        /// Prevent horizontal splitter resizing.
        const NO_RESIZE_X = sys::ImGuiDockNodeFlags_NoResizeX as i32;
        /// Prevent vertical splitter resizing.
        const NO_RESIZE_Y = sys::ImGuiDockNodeFlags_NoResizeY as i32;
        /// Include docked windows in their host window's focus route.
        const DOCKED_WINDOWS_IN_FOCUS_ROUTE =
            sys::ImGuiDockNodeFlags_DockedWindowsInFocusRoute as i32;
        /// Prevent this node from splitting another node while docking.
        const NO_DOCKING_SPLIT_OTHER = sys::ImGuiDockNodeFlags_NoDockingSplitOther as i32;
        /// Prevent other payloads from docking over this node.
        const NO_DOCKING_OVER_ME = sys::ImGuiDockNodeFlags_NoDockingOverMe as i32;
        /// Prevent this payload from docking over another non-empty node.
        const NO_DOCKING_OVER_OTHER = sys::ImGuiDockNodeFlags_NoDockingOverOther as i32;
        /// Prevent this payload from docking over an empty node.
        const NO_DOCKING_OVER_EMPTY = sys::ImGuiDockNodeFlags_NoDockingOverEmpty as i32;
        /// Prevent all docking operations involving this node.
        const NO_DOCKING = sys::ImGuiDockNodeFlags_NoDocking as i32;
    }
}

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
    fn try_raw(self) -> Result<sys::ImGuiID, WindowClassKind> {
        match self {
            Self::Default => Ok(!0),
            Self::NoParent => Ok(0),
            Self::Parent(id) if id.raw() != 0 => Ok(id.raw()),
            Self::Parent(_) => Err(WindowClassKind::ZeroParentViewportId),
        }
    }
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
    pub fn validate(&self) -> Result<(), ContextError> {
        let viewport_bits =
            (self.viewport_flags_override_set | self.viewport_flags_override_clear).bits();
        let unsupported_viewport = viewport_bits & !crate::WindowClassViewportFlags::all().bits();
        if unsupported_viewport != 0 {
            return Err(WindowClassKind::UnsupportedViewportFlags {
                bits: unsupported_viewport,
            }
            .into());
        }
        let overlap =
            self.viewport_flags_override_set.bits() & self.viewport_flags_override_clear.bits();
        if overlap != 0 {
            return Err(WindowClassKind::OverlappingViewportFlags { bits: overlap }.into());
        }
        let unsupported_tab =
            self.tab_item_flags_override_set.flags.bits() & !crate::TabItemFlags::all().bits();
        if unsupported_tab != 0 {
            return Err(WindowClassKind::UnsupportedTabItemFlags {
                bits: unsupported_tab,
            }
            .into());
        }
        let unsupported_dock =
            self.dock_node_flags_override_set.bits() & !WindowClassDockNodeFlags::all().bits();
        if unsupported_dock != 0 {
            return Err(WindowClassKind::UnsupportedDockNodeFlags {
                bits: unsupported_dock,
            }
            .into());
        }
        self.parent_viewport.try_raw()?;
        Ok(())
    }

    pub(crate) fn try_to_imgui(&self) -> Result<sys::ImGuiWindowClass, ContextError> {
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

/// Canonical builder for a dockspace submission.
///
/// The builder defaults to the main viewport and lets Dear ImGui derive its stable root ID. Use
/// [`Self::current_window`] plus [`Self::root_id`] for an explicit host window. Adding a layout
/// makes validation, staging, replacement, and rollback part of the same submission transaction.
#[must_use = "a dockspace builder does nothing until build() is called"]
pub struct DockspaceBuilder<'ui, 'layout> {
    ui: &'ui Ui,
    root_id: Option<Id>,
    host: DockspaceHost,
    flags: DockNodeFlags,
    window_class: Option<WindowClass>,
    layout: Option<(&'layout DockLayout, DockLayoutApply)>,
}

impl<'ui> DockspaceBuilder<'ui, 'static> {
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            ui,
            root_id: None,
            host: DockspaceHost::MainViewport,
            flags: DockNodeFlags::NONE,
            window_class: None,
            layout: None,
        }
    }
}

impl<'ui, 'layout> DockspaceBuilder<'ui, 'layout> {
    /// Use an explicit stable root ID.
    pub fn root_id(mut self, root_id: Id) -> Self {
        self.root_id = Some(root_id);
        self
    }

    /// Host the dockspace over the main viewport.
    pub fn main_viewport(mut self) -> Self {
        self.host = DockspaceHost::MainViewport;
        self
    }

    /// Host the dockspace at the current window cursor with a positive size.
    pub fn current_window(mut self, size: [f32; 2]) -> Self {
        self.host = DockspaceHost::CurrentWindow { size };
        self
    }

    /// Set the public dock node flags used for submission.
    pub fn flags(mut self, flags: DockNodeFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the optional window class used by the dockspace.
    pub fn window_class(mut self, window_class: WindowClass) -> Self {
        self.window_class = Some(window_class);
        self
    }

    /// Apply a complete declarative layout with the selected persistence policy.
    pub fn layout<'next>(
        self,
        layout: &'next DockLayout,
        apply: DockLayoutApply,
    ) -> DockspaceBuilder<'ui, 'next> {
        DockspaceBuilder {
            ui: self.ui,
            root_id: self.root_id,
            host: self.host,
            flags: self.flags,
            window_class: self.window_class,
            layout: Some((layout, apply)),
        }
    }

    /// Validate and submit the dockspace.
    ///
    /// Call this before any window hosted by the target dock tree. A failed declarative layout
    /// application preserves an existing root and its persisted settings.
    pub fn build(self) -> Result<Id, ContextError> {
        let submission = self.host;
        let root_id = match self.host {
            DockspaceHost::MainViewport => match self.root_id {
                Some(root_id) if root_id.raw() == 0 => return Err(ContextError::ZeroRootId),
                Some(root_id) => root_id,
                None => resolve_main_viewport_dockspace_id(self.ui),
            },
            DockspaceHost::CurrentWindow { .. } => {
                let root_id = self.root_id.ok_or(ContextError::MissingRootId)?;
                if root_id.raw() == 0 {
                    return Err(ContextError::ZeroRootId);
                }
                root_id
            }
        };
        let config = DockspaceConfig::new(root_id, self.flags, self.window_class);
        match self.layout {
            Some((layout, apply)) => submit_and_apply(self.ui, &config, layout, apply, submission),
            None => submit_without_layout(self.ui, &config, submission),
        }
    }
}

fn resolve_main_viewport_dockspace_id(ui: &Ui) -> Id {
    ui.run_with_bound_context(|| main_viewport_dockspace_id("DockspaceBuilder::build()", None))
}

/// Docking-related functionality
impl Ui {
    /// Configure and submit a dockspace through the canonical builder.
    pub fn dockspace(&self) -> crate::DockspaceBuilder<'_, 'static> {
        crate::DockspaceBuilder::new(self)
    }

    /// Submit Dear ImGui's low-level dockspace-over-viewport operation for the main viewport.
    ///
    /// This creates Dear ImGui's hidden main-viewport host window, applies the viewport work
    /// rectangle and platform ownership, and submits a dockspace within that host.
    /// Submit it before every window that can be hosted by this dockspace. A
    /// `KEEP_ALIVE_ONLY` submission may be made later because it does not create a visible host.
    /// Without an earlier submission, Dear ImGui may already undock a window when that window is
    /// begun, before this method can diagnose the ordering error.
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
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame, when the effective dockspace
    /// ID names a child of another dock tree, when the dockspace was already submitted without
    /// `KEEP_ALIVE_ONLY` during this frame, or when a hosted window was submitted before a visible
    /// dockspace submission while that window is still attached. `KEEP_ALIVE_ONLY` remains valid
    /// after hosted windows.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dock_space_over_main_viewport_raw(
    ///     0.into(),
    ///     DockNodeFlags::PASSTHRU_CENTRAL_NODE
    /// );
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dock_space_over_main_viewport_raw(&self, dockspace_id: Id, flags: DockNodeFlags) -> Id {
        const CALLER: &str = "Ui::dock_space_over_main_viewport_raw()";
        validate_dock_node_flags(CALLER, flags);
        self.run_with_bound_context(|| {
            assert_docking_available(CALLER);
            let requested = (dockspace_id.raw() != 0).then_some(dockspace_id);
            let effective_id = main_viewport_dockspace_id(CALLER, requested);
            assert_existing_dockspace_node_is_root(CALLER, effective_id);
            let claim = claim_dockspace_submission(self, CALLER, effective_id, flags, false)
                .unwrap_or_else(|_| {
                    panic!("{CALLER} cannot submit dockspace {effective_id:?} twice in one frame")
                });
            if !flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) {
                assert_dockspace_has_no_active_content(CALLER, effective_id);
            }
            let submitted = unsafe {
                Id::from(sys::igDockSpaceOverViewport(
                    effective_id.into(),
                    sys::igGetMainViewport(),
                    flags.bits(),
                    ptr::null(),
                ))
            };
            if let Some(claim) = claim {
                claim.commit();
            }
            assert_eq!(
                submitted, effective_id,
                "{CALLER} native submission returned an unexpected dockspace ID"
            );
            submitted
        })
    }

    /// Submit Dear ImGui's low-level dockspace operation in the current window.
    ///
    /// Submit it before every window that can be hosted by this dockspace. A
    /// `KEEP_ALIVE_ONLY` submission may be made later because it does not create a visible host.
    /// Without an earlier submission, Dear ImGui may already undock a window when that window is
    /// begun, before this method can diagnose the ordering error.
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
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame, when `id` names a child of
    /// another dock tree, when `id` was already submitted without `KEEP_ALIVE_ONLY` during this
    /// frame, or when a hosted window was submitted before a visible dockspace submission while
    /// that window is still attached. `KEEP_ALIVE_ONLY` remains valid after hosted windows.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space_raw(
    ///     dockspace_id,
    ///     [800.0, 600.0],
    ///     DockNodeFlags::NO_DOCKING_SPLIT,
    ///     Some(&WindowClass::new(Id::from(1u32)))
    /// );
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space_raw(
        &self,
        id: Id,
        size: [f32; 2],
        flags: DockNodeFlags,
        window_class: Option<&WindowClass>,
    ) -> Id {
        const CALLER: &str = "Ui::dock_space_raw()";
        validate_dock_node_flags(CALLER, flags);
        assert_nonzero_id(CALLER, "id", id);
        assert_dockspace_size(CALLER, "size", size);
        let size_vec = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
        let imgui_window_class = window_class.map(|class| class.to_imgui(CALLER));
        let window_class_ptr = imgui_window_class
            .as_ref()
            .map_or(ptr::null(), |wc| wc as *const _);
        self.run_with_bound_context(|| {
            assert_dockspace_host_name_supported(CALLER);
            assert_existing_dockspace_node_is_root(CALLER, id);
            let host_skipped = current_window_skips_items(CALLER);
            let claim =
                claim_dockspace_submission(self, CALLER, id, flags, true).unwrap_or_else(|_| {
                    panic!("{CALLER} cannot submit dockspace {id:?} twice in one frame")
                });
            if !flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) && !host_skipped {
                assert_dockspace_has_no_active_content(CALLER, id);
            }
            let submitted = unsafe {
                Id::from(sys::igDockSpace(
                    id.into(),
                    size_vec,
                    flags.bits(),
                    window_class_ptr,
                ))
            };
            if let Some(claim) = claim {
                claim.commit();
            }
            assert_eq!(
                submitted, id,
                "{CALLER} native submission returned an unexpected dockspace ID"
            );
            submitted
        })
    }

    /// Sets the dock ID for the next window with condition
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame.
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
    /// let dockspace_id = ui.dockspace().build()?;
    /// ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// # Ok::<(), ContextError>(())
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id_with_cond(&self, dock_id: Id, cond: crate::Condition) {
        const CALLER: &str = "Ui::set_next_window_dock_id_with_cond()";
        self.run_with_bound_context(|| {
            assert_docking_available(CALLER);
            unsafe {
                sys::igSetNextWindowDockID(dock_id.into(), cond as i32);
            }
        });
    }

    /// Sets the dock ID for the next window
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    /// Uses `Condition::Always` by default.
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame.
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
    /// let dockspace_id = ui.dockspace().build()?;
    /// ui.set_next_window_dock_id(dockspace_id);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// # Ok::<(), ContextError>(())
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
        let imgui_wc = window_class.to_imgui("Ui::set_next_window_class()");
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextWindowClass(&imgui_wc as *const _);
        });
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
        self.run_with_bound_context(|| unsafe { Id::from(sys::igGetWindowDockID()) })
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
        self.run_with_bound_context(|| unsafe { sys::igIsWindowDocked() })
    }
}

pub(crate) const MAX_DOCKSPACE_HOST_NAME_BYTES: usize = 236;
const MIN_TRUNCATABLE_DOCKSPACE_SIZE: f32 = -2_147_483_648.0;
const MAX_TRUNCATABLE_DOCKSPACE_SIZE_EXCLUSIVE: f32 = 2_147_483_648.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateDockspaceSubmission;

pub(crate) fn assert_docking_available(caller: &str) {
    unsafe {
        let context = sys::igGetCurrentContext();
        assert!(
            !context.is_null(),
            "{caller} requires a valid ImGui context"
        );
        assert!(
            (*context).WithinFrameScope,
            "{caller} requires an open Dear ImGui frame"
        );

        let io = sys::igGetIO_Nil();
        assert!(!io.is_null(), "{caller} requires a valid ImGui IO object");
        let requested = (*io).ConfigFlags & sys::ImGuiConfigFlags_DockingEnable as i32;
        let active = (*context).ConfigFlagsCurrFrame & sys::ImGuiConfigFlags_DockingEnable as i32;
        assert_eq!(
            requested, active,
            "{caller} cannot change ConfigFlags::DOCKING_ENABLE while a frame is open"
        );
        assert!(
            active != 0,
            "{caller} requires ConfigFlags::DOCKING_ENABLE before the first frame"
        );
    }
}

pub(crate) fn claim_dockspace_submission(
    ui: &Ui,
    caller: &str,
    id: Id,
    flags: DockNodeFlags,
    current_window_submission: bool,
) -> Result<Option<crate::context::binding::DockspaceFrameClaim>, DuplicateDockspaceSubmission> {
    assert_docking_available(caller);
    let current_window_skips_items =
        current_window_submission && current_window_skips_items(caller);
    if flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) || current_window_skips_items {
        return Ok(None);
    }

    let frame = unsafe { sys::igGetFrameCount() };
    ui.binding()
        .claim_dockspace_submission(frame, id.raw())
        .map(Some)
        .ok_or(DuplicateDockspaceSubmission)
}

pub(crate) fn assert_existing_dockspace_node_is_root(caller: &str, id: Id) {
    let node = unsafe { sys::igDockBuilderGetNode(id.raw()) };
    assert!(
        node.is_null() || unsafe { sys::ImGuiDockNode_IsRootNode(node) },
        "{caller} ID {id:?} resolves to a child of another dock tree"
    );
}

pub(crate) fn assert_dockspace_has_no_active_content(caller: &str, id: Id) {
    assert!(
        unsafe { sys::dear_imgui_rs_dock_builder_root_has_active_content_window(id.raw()) } == 0,
        "{caller} must submit dockspace {id:?} before its hosted windows"
    );
}

pub(crate) fn current_window_skips_items(caller: &str) -> bool {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        assert!(
            !window.is_null(),
            "{caller} requires a current ImGui window"
        );
        (*window).SkipItems
    }
}

pub(crate) fn main_viewport_dockspace_host_name(caller: &str) -> CString {
    unsafe {
        let viewport = sys::igGetMainViewport();
        assert!(!viewport.is_null(), "{caller} requires a main viewport");
        CString::new(format!("WindowOverViewport_{:08X}", (*viewport).ID))
            .expect("generated viewport host name cannot contain a NUL byte")
    }
}

pub(crate) fn main_viewport_dockspace_id(caller: &str, requested: Option<Id>) -> Id {
    if let Some(requested) = requested {
        return requested;
    }

    let host_name = main_viewport_dockspace_host_name(caller);
    unsafe {
        let host_id = sys::igGetIDWithSeed_Str(host_name.as_ptr(), ptr::null(), 0);
        Id::from(sys::igGetIDWithSeed_Str(
            c"DockSpace".as_ptr(),
            ptr::null(),
            host_id,
        ))
    }
}

pub(crate) fn assert_dockspace_host_name_supported(caller: &str) {
    let name_len = current_dockspace_host_name_len(caller);
    assert!(
        name_len <= MAX_DOCKSPACE_HOST_NAME_BYTES,
        "{caller} host window name is {name_len} bytes; DockSpace supports at most \
         {MAX_DOCKSPACE_HOST_NAME_BYTES} bytes without truncating its internal window identity"
    );
}

pub(crate) fn current_dockspace_host_name_len(caller: &str) -> usize {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        assert!(
            !window.is_null(),
            "{caller} requires a current ImGui window"
        );
        let name = (*window).Name;
        assert!(!name.is_null(), "{caller} requires a named host window");
        CStr::from_ptr(name).to_bytes().len()
    }
}

pub(crate) fn assert_nonzero_id(caller: &str, name: &str, id: Id) {
    assert!(id.raw() != 0, "{caller} {name} must be non-zero");
}

pub(crate) fn assert_dockspace_size(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value
            .iter()
            .all(|component| is_valid_dockspace_size_component(*component)),
        "{caller} {name} components must be finite and safely truncatable to i32"
    );
}

pub(crate) fn is_valid_dockspace_size_component(value: f32) -> bool {
    value.is_finite()
        && (MIN_TRUNCATABLE_DOCKSPACE_SIZE..MAX_TRUNCATABLE_DOCKSPACE_SIZE_EXCLUSIVE)
            .contains(&value)
}

#[cfg(test)]
mod tests {
    use super::assert_dockspace_size;

    #[test]
    fn dockspace_size_accepts_exact_native_cast_boundaries() {
        let upper_exclusive = 2_147_483_648.0f32;
        let largest_valid = f32::from_bits(upper_exclusive.to_bits() - 1);

        assert_dockspace_size("test", "size", [-2_147_483_648.0, largest_valid]);
        for invalid in [f32::NAN, f32::INFINITY, f32::MAX, upper_exclusive] {
            assert!(
                std::panic::catch_unwind(|| {
                    assert_dockspace_size("test", "size", [invalid, 0.0]);
                })
                .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(|| {
                assert_dockspace_size("test", "size", [-f32::MAX, 0.0]);
            })
            .is_err()
        );
    }
}
