//! Dear ImGui platform-viewport bridge for Bevy-owned windows.
//!
//! PlatformIO callbacks installed here only capture intent into an engine-owned queue. Bevy systems
//! drain that queue and mutate ECS-owned [`Window`] entities outside the C ABI callback boundary.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) mod native_window;

use bevy_app::App;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_app::{Last, PreUpdate};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_camera::{
    Camera, Camera2d, CameraOutputMode, ClearColorConfig, RenderTarget, visibility::RenderLayers,
};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_core_pipeline::Core2d;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::SystemParam;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use bevy_math::IVec2;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_render::camera::CameraRenderGraph;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_window::WindowRef;
use bevy_window::{CompositeAlphaMode, PresentMode, Window, WindowTheme};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{
    CursorOptions, ExitSystems, Monitor, PrimaryWindow, WindowCloseRequested, WindowMoved,
    WindowOccluded, WindowResized,
};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use bevy_window::{WindowLevel, WindowPosition, WindowResolution};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::{WINIT_WINDOWS, WinitSettings};
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::HashSet;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ffi::{CStr, c_char, c_void};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Rc;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Weak;

/// Native desktop-position capability observed for one Bevy-managed Dear ImGui Context.
///
/// Dear ImGui's native multi-viewport contract needs both global client-area positions and
/// native window positioning. On Wayland this remains unavailable by protocol design, so the
/// backend keeps docking in the host window instead of advertising unusable platform viewports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiNativeViewportStatus {
    /// A native host window has not been created yet.
    PendingNativeWindow,
    /// Native platform viewports may be enabled.
    Available,
    /// The active window system does not provide global desktop coordinates.
    GlobalDesktopCoordinatesUnavailable,
}

/// Per-Context native multi-viewport capability observed during the latest frame.
///
/// A Context is absent until it opts into native multi-viewport and enters the private Context
/// driver. Removed, disabled, and not-yet-driven Contexts are not retained.
#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct ImguiNativeViewportSupport {
    contexts: HashMap<imgui::ContextId, ImguiNativeViewportStatus>,
}

impl ImguiNativeViewportSupport {
    /// Return the latest native multi-viewport status for `context_id`.
    #[must_use]
    pub fn get(&self, context_id: imgui::ContextId) -> Option<ImguiNativeViewportStatus> {
        self.contexts.get(&context_id).copied()
    }

    /// Iterate the Contexts which currently request native multi-viewport support.
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (imgui::ContextId, ImguiNativeViewportStatus)> + '_ {
        self.contexts
            .iter()
            .map(|(context_id, status)| (*context_id, *status))
    }

    /// Return whether native platform windows are available for `context_id`.
    #[must_use]
    pub fn is_available(&self, context_id: imgui::ContextId) -> bool {
        self.get(context_id) == Some(ImguiNativeViewportStatus::Available)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn begin_frame(&mut self, contexts: impl IntoIterator<Item = imgui::ContextId>) {
        self.contexts.clear();
        self.contexts.extend(
            contexts
                .into_iter()
                .map(|context_id| (context_id, ImguiNativeViewportStatus::PendingNativeWindow)),
        );
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn set(&mut self, context_id: imgui::ContextId, status: ImguiNativeViewportStatus) {
        self.contexts.insert(context_id, status);
    }
}

/// Policy applied to every Bevy window created for a secondary Dear ImGui viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiViewportWindowConfig {
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub desired_maximum_frame_latency: Option<std::num::NonZeroU32>,
    pub window_theme: Option<WindowTheme>,
    pub transparent: bool,
}

#[cfg(test)]
#[path = "viewport/tests/viewport.rs"]
mod viewport_tests;

/// Invalid secondary-window presentation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportWindowConfigError {
    /// A transparent window selected a compositor mode that does not guarantee alpha blending.
    TransparentCompositeAlphaModeUnsupported {
        composite_alpha_mode: CompositeAlphaMode,
    },
}

impl std::fmt::Display for ImguiViewportWindowConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransparentCompositeAlphaModeUnsupported {
                composite_alpha_mode,
            } => write!(
                formatter,
                "transparent Dear ImGui viewport windows require PreMultiplied or PostMultiplied composite alpha, got {composite_alpha_mode:?}"
            ),
        }
    }
}

impl std::error::Error for ImguiViewportWindowConfigError {}

impl Default for ImguiViewportWindowConfig {
    fn default() -> Self {
        Self::from_window(&Window::default())
    }
}

impl ImguiViewportWindowConfig {
    /// Copy the presentation policy of an existing Bevy window.
    #[must_use]
    pub fn from_window(window: &Window) -> Self {
        Self {
            present_mode: window.present_mode,
            composite_alpha_mode: window.composite_alpha_mode,
            desired_maximum_frame_latency: window.desired_maximum_frame_latency,
            window_theme: window.window_theme,
            transparent: window.transparent,
        }
    }

    /// Copy and validate the presentation policy of an existing Bevy window.
    pub fn try_from_window(window: &Window) -> Result<Self, ImguiViewportWindowConfigError> {
        Self::from_window(window).validate()
    }

    /// Validate that a transparent window uses a compositor mode which preserves alpha.
    pub fn validate(self) -> Result<Self, ImguiViewportWindowConfigError> {
        if self.transparent
            && !matches!(
                self.composite_alpha_mode,
                CompositeAlphaMode::PreMultiplied | CompositeAlphaMode::PostMultiplied
            )
        {
            return Err(
                ImguiViewportWindowConfigError::TransparentCompositeAlphaModeUnsupported {
                    composite_alpha_mode: self.composite_alpha_mode,
                },
            );
        }
        Ok(self)
    }

    #[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    fn apply_to(self, window: &mut Window) {
        window.present_mode = self.present_mode;
        window.composite_alpha_mode = self.composite_alpha_mode;
        window.desired_maximum_frame_latency = self.desired_maximum_frame_latency;
        window.window_theme = self.window_theme;
        window.transparent = self.transparent;
    }
}

/// Stable Dear ImGui viewport identifier used by the Bevy bridge.
pub type ImguiViewportId = imgui::Id;

/// Snapshot of Dear ImGui viewport state copied while a PlatformIO callback is running.
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImguiViewportSnapshot {
    pub id: ImguiViewportId,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub dpi_scale: f32,
    pub flags: imgui::ViewportFlags,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportSnapshot {
    #[must_use]
    pub fn from_viewport(viewport: &imgui::Viewport) -> Self {
        Self {
            id: viewport.id(),
            pos: viewport.pos(),
            size: viewport.size(),
            // SAFETY: `viewport` is a live Dear ImGui viewport reference for the duration of the
            // PlatformIO callback. We copy the scalar value and do not retain the raw pointer.
            dpi_scale: unsafe { (*viewport.as_raw()).DpiScale },
            flags: viewport.flags(),
        }
    }
}

/// Internal intent captured from Dear ImGui PlatformIO callbacks.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ImguiViewportCommand {
    Create(ImguiViewportSnapshot),
    Destroy {
        id: ImguiViewportId,
    },
    Show {
        id: ImguiViewportId,
    },
    Update {
        id: ImguiViewportId,
        previous_flags: Option<imgui::ViewportFlags>,
        flags: imgui::ViewportFlags,
    },
    SetPos {
        id: ImguiViewportId,
        pos: [f32; 2],
        dpi_scale: f32,
    },
    SetSize {
        id: ImguiViewportId,
        size: [f32; 2],
        dpi_scale: f32,
    },
    SetFocus {
        id: ImguiViewportId,
    },
    SetTitle {
        id: ImguiViewportId,
        title: String,
    },
}

/// Last Bevy-observed platform state for a Dear ImGui viewport window.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImguiViewportFeedback {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub dpi_scale: f32,
    pub focused: bool,
    pub minimized: bool,
}

/// Backend-owned identity marker on Bevy `Window` entities created for secondary viewports.
///
/// Applications should query this component by reference and use its accessors. Constructing or
/// moving the marker does not transfer backend ownership.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportWindow {
    context_id: imgui::ContextId,
    viewport_id: ImguiViewportId,
}

impl ImguiViewportWindow {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn new(context_id: imgui::ContextId, viewport_id: ImguiViewportId) -> Self {
        Self {
            context_id,
            viewport_id,
        }
    }

    /// Returns the Context which owns this native viewport.
    #[must_use]
    pub const fn context_id(&self) -> imgui::ContextId {
        self.context_id
    }

    /// Returns the Dear ImGui viewport identifier within [`Self::context_id`].
    #[must_use]
    pub const fn viewport_id(&self) -> ImguiViewportId {
        self.viewport_id
    }
}

/// Backend-owned identity marker on Bevy camera entities created to render secondary viewports.
///
/// Applications should query this component by reference and use its accessors. Constructing or
/// moving the marker does not transfer backend ownership.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportCamera {
    context_id: imgui::ContextId,
    viewport_id: ImguiViewportId,
}

impl ImguiViewportCamera {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn new(context_id: imgui::ContextId, viewport_id: ImguiViewportId) -> Self {
        Self {
            context_id,
            viewport_id,
        }
    }

    /// Returns the Context which owns this native viewport.
    #[must_use]
    pub const fn context_id(&self) -> imgui::ContextId {
        self.context_id
    }

    /// Returns the Dear ImGui viewport identifier within [`Self::context_id`].
    #[must_use]
    pub const fn viewport_id(&self) -> ImguiViewportId {
        self.viewport_id
    }
}

/// Private capability paired with each public native viewport identity marker.
///
/// Public markers are observable ECS data and can be moved by applications. Backend systems
/// therefore require this unforgeable companion before treating an entity as backend-owned.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ImguiViewportOwner {
    kind: ImguiViewportOwnerKind,
    context_id: imgui::ContextId,
    viewport_id: ImguiViewportId,
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum ImguiViewportOwnerKind {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    Window,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    Camera,
}

impl ImguiViewportOwner {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    const fn window(context_id: imgui::ContextId, viewport_id: ImguiViewportId) -> Self {
        Self {
            kind: ImguiViewportOwnerKind::Window,
            context_id,
            viewport_id,
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    const fn camera(context_id: imgui::ContextId, viewport_id: ImguiViewportId) -> Self {
        Self {
            kind: ImguiViewportOwnerKind::Camera,
            context_id,
            viewport_id,
        }
    }

    #[must_use]
    pub(crate) fn matches_window(&self, marker: &ImguiViewportWindow) -> bool {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            matches!(&self.kind, ImguiViewportOwnerKind::Window)
                && self.context_id == marker.context_id
                && self.viewport_id == marker.viewport_id
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        {
            let _ = (self, marker);
            false
        }
    }

    #[cfg(feature = "render")]
    #[must_use]
    pub(crate) fn matches_camera(&self, marker: &ImguiViewportCamera) -> bool {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            matches!(&self.kind, ImguiViewportOwnerKind::Camera)
                && self.context_id == marker.context_id
                && self.viewport_id == marker.viewport_id
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        {
            let _ = (self, marker);
            false
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn window_identity(&self) -> Option<(imgui::ContextId, ImguiViewportId)> {
        match &self.kind {
            ImguiViewportOwnerKind::Window => Some((self.context_id, self.viewport_id)),
            ImguiViewportOwnerKind::Camera => None,
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    const fn camera_identity(&self) -> Option<(imgui::ContextId, ImguiViewportId)> {
        match &self.kind {
            ImguiViewportOwnerKind::Window => None,
            ImguiViewportOwnerKind::Camera => Some((self.context_id, self.viewport_id)),
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) type ImguiViewportBridgeKeepalive = Rc<ImguiViewportBridgeShared>;

/// Context-qualified registry and read-only viewport lookup for Dear ImGui platform windows.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct ImguiViewportBridge {
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    inner: ImguiViewportBridgeKeepalive,
    /// Every admitted multi-viewport Context gets its own callback state.
    contexts: Rc<RefCell<HashMap<imgui::ContextId, ImguiViewportBridgeKeepalive>>>,
}

/// Cloneable registration capability used by Context admission to publish its own viewport state
/// into Bevy's global ECS bridge without borrowing the `World`.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone)]
pub(crate) struct ImguiViewportBridgeRegistration {
    contexts: Rc<RefCell<HashMap<imgui::ContextId, ImguiViewportBridgeKeepalive>>>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct ImguiViewportBridgeShared {
    state: RefCell<ImguiViewportBridgeState>,
    context_id: Cell<Option<imgui::ContextId>>,
    callback_fault: Cell<Option<ImguiViewportRuntimeError>>,
    ecs_release_pending: Cell<bool>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    native_teardown_in_progress: Cell<bool>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    core_teardown_owns_native_guard: Cell<bool>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    callback_contract: Cell<Option<ImguiViewportCallbackContract>>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    runtime_contract: Cell<Option<ImguiViewportRuntimeContract>>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    monitor_contract: RefCell<Option<ImguiViewportMonitorContract>>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
thread_local! {
    /// Independent owner registry used by C callbacks before they ever inspect ImGui userdata.
    /// The raw context address is only a lookup key; the weak owner and the equality check below
    /// are the actual capability proof.
    static VIEWPORT_BRIDGE_REGISTRY: RefCell<HashMap<usize, Weak<ImguiViewportBridgeShared>>> =
        RefCell::new(HashMap::new());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy)]
struct ImguiViewportCallbackContract {
    platform: [usize; 20],
    renderer: [usize; 5],
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy)]
struct ImguiViewportRuntimeContract {
    backend_platform_user_data: *mut c_void,
    backend_platform_name: *const c_char,
    owned_flags: i32,
    main_viewport_platform_user_data: *mut c_void,
    main_viewport_platform_handle: *mut c_void,
    main_viewport_platform_handle_raw: *mut c_void,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct ImguiViewportMonitorContract {
    data: *mut sys::ImGuiPlatformMonitor,
    size: i32,
    capacity: i32,
    monitors: Vec<sys::ImGuiPlatformMonitor>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct NativePlatformTeardownGuard<'a> {
    active: &'a Cell<bool>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl<'a> NativePlatformTeardownGuard<'a> {
    fn enter(active: &'a Cell<bool>) -> Self {
        assert!(
            !active.get(),
            "dear-imgui-bevy native platform teardown was reentered"
        );
        active.set(true);
        Self { active }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl Drop for NativePlatformTeardownGuard<'_> {
    fn drop(&mut self) {
        self.active.set(false);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) struct ImguiViewportBridgeAttachmentMarker;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) struct ImguiViewportBridgeTeardownAttachment {
    keepalive: ImguiViewportBridgeKeepalive,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_bridge_teardown_attachment(
    keepalive: ImguiViewportBridgeKeepalive,
) -> Rc<dyn imgui::ContextAttachment> {
    Rc::new(ImguiViewportBridgeTeardownAttachment { keepalive })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl imgui::ContextAttachment for ImguiViewportBridgeTeardownAttachment {
    fn begin_platform_window_teardown(
        &self,
        context: &imgui::ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), imgui::ContextAttachmentTeardownError> {
        self.keepalive.core_teardown_owns_native_guard.set(false);
        context.with_bound_context(|| {
            let context_raw = unsafe { sys::igGetCurrentContext() };
            let main_viewport = unsafe { sys::igGetMainViewport() };
            platform_callback_ownership_raw(context_raw, main_viewport, &self.keepalive)
                .map_err(|error| imgui::ContextAttachmentTeardownError::new(error.to_string()))?;
            if !self.keepalive.native_teardown_in_progress.replace(true) {
                self.keepalive.core_teardown_owns_native_guard.set(true);
            }
            Ok(())
        })
    }

    fn end_platform_window_teardown(
        &self,
        context: &imgui::ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), imgui::ContextAttachmentTeardownError> {
        if !self
            .keepalive
            .core_teardown_owns_native_guard
            .replace(false)
        {
            return Ok(());
        }
        if !self.keepalive.native_teardown_in_progress.get() {
            return Err(imgui::ContextAttachmentTeardownError::new(
                "Bevy viewport bridge lost its native teardown guard",
            ));
        }
        struct NativeTeardownReset<'a> {
            active: &'a Cell<bool>,
        }

        impl Drop for NativeTeardownReset<'_> {
            fn drop(&mut self) {
                self.active.set(false);
            }
        }

        let _reset = NativeTeardownReset {
            active: &self.keepalive.native_teardown_in_progress,
        };
        context.with_bound_context(|| {
            record_platform_runtime_contract_in_current_context(&self.keepalive);
        });
        Ok(())
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeShared {
    fn set_context_id(&self, context_id: imgui::ContextId) {
        if let Some(existing) = self.context_id.get() {
            debug_assert_eq!(
                existing, context_id,
                "a viewport bridge allocation cannot be shared by two Contexts"
            );
            return;
        }
        self.context_id.set(Some(context_id));
    }

    fn clear_viewport_state_preserving_native_handles(&self) {
        let mut state = self.state.borrow_mut();
        state.viewport_windows.clear();
        state.viewport_cameras.clear();
        state.viewport_feedback.clear();
        state.viewport_flags.clear();
        state.pending_client_placements.clear();
        state.commands.clear();
        state.focus_next_frame.clear();
        state.focus_ready.clear();
    }

    fn clear_viewport_state_preserving_pending_despawns(&self) {
        self.clear_viewport_state_preserving_native_handles();
        let mut state = self.state.borrow_mut();
        state.viewport_handles.clear();
        state.retired_viewport_handles.clear();
    }

    fn clear_viewport_state(&self) {
        self.clear_viewport_state_preserving_pending_despawns();
        self.state.borrow_mut().pending_ecs_despawns.clear();
        self.callback_fault.set(None);
        self.ecs_release_pending.set(false);
    }

    fn prepare_ecs_release(&self, main_viewport_id: ImguiViewportId) {
        let mut state = self.state.borrow_mut();
        state.viewport_windows.remove(&main_viewport_id);
        state.viewport_cameras.remove(&main_viewport_id);

        let mut secondary_viewports = state
            .viewport_windows
            .keys()
            .chain(state.viewport_cameras.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        secondary_viewports.sort_by_key(|viewport_id| viewport_id.raw());
        state.commands.clear();
        state.commands.extend(
            secondary_viewports
                .into_iter()
                .map(|id| ImguiViewportCommand::Destroy { id }),
        );
        state.viewport_feedback.clear();
        state.viewport_flags.clear();
        state.pending_client_placements.clear();
        state.focus_next_frame.clear();
        state.focus_ready.clear();
        drop(state);

        self.callback_fault.set(None);
        self.ecs_release_pending.set(true);
    }

    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.ecs_release_pending.get()
    }

    fn has_tracked_ecs_entities(&self) -> bool {
        let state = self.state.borrow();
        !state.viewport_windows.is_empty()
            || !state.viewport_cameras.is_empty()
            || !state.pending_ecs_despawns.is_empty()
    }

    fn track_ecs_despawn(&self, entity: Entity) {
        self.state.borrow_mut().pending_ecs_despawns.insert(entity);
    }

    fn track_ecs_despawns(&self, entities: impl IntoIterator<Item = Entity>) {
        self.state
            .borrow_mut()
            .pending_ecs_despawns
            .extend(entities);
    }

    fn pending_ecs_despawns(&self) -> HashSet<Entity> {
        self.state.borrow().pending_ecs_despawns.clone()
    }

    fn take_all_ecs_entities_for_release(&self) -> HashSet<Entity> {
        let mut state = self.state.borrow_mut();
        let mapped = state
            .viewport_windows
            .values()
            .chain(state.viewport_cameras.values())
            .copied()
            .collect::<Vec<_>>();
        state.pending_ecs_despawns.extend(mapped);
        state.viewport_windows.clear();
        state.viewport_cameras.clear();
        state.pending_client_placements.clear();
        state.pending_ecs_despawns.clone()
    }

    fn acknowledge_ecs_despawns(&self, mut entity_is_live: impl FnMut(Entity) -> bool) {
        {
            let mut state = self.state.borrow_mut();
            state
                .pending_ecs_despawns
                .retain(|entity| entity_is_live(*entity));
        }
    }

    fn finish_ecs_release(&self) {
        debug_assert!(!self.has_tracked_ecs_entities());
        self.clear_viewport_state();
        self.callback_contract.set(None);
        self.runtime_contract.set(None);
        self.monitor_contract.borrow_mut().take();
    }

    fn record_callback_contract(&self, context: &mut imgui::Context) {
        self.callback_contract
            .set(Some(ImguiViewportCallbackContract::capture(context)));
        self.record_runtime_contract(context);
    }

    fn record_runtime_contract(&self, context: &mut imgui::Context) {
        let binding = context.binding();
        binding.with_bound_context(|| self.record_runtime_contract_raw(context.as_raw()));
    }

    fn record_owned_platform_name(&self, context: &mut imgui::Context) {
        let binding = context.binding();
        binding.with_bound_context(|| {
            let Some(mut runtime_contract) = self.runtime_contract.get() else {
                panic!("Dear ImGui viewport runtime contract was unavailable");
            };
            let Some(io) = (unsafe { sys::igGetIO_ContextPtr(context.as_raw()).as_ref() }) else {
                panic!("Dear ImGui viewport runtime contract lost its IO");
            };
            runtime_contract.backend_platform_name = io.BackendPlatformName;
            self.runtime_contract.set(Some(runtime_contract));
        });
    }

    fn record_runtime_contract_raw(&self, context_raw: *mut sys::ImGuiContext) {
        let Some(io) = (unsafe { sys::igGetIO_ContextPtr(context_raw).as_ref() }) else {
            self.runtime_contract.set(None);
            return;
        };
        let Some(main_viewport) = (unsafe { sys::igGetMainViewport().as_ref() }) else {
            self.runtime_contract.set(None);
            return;
        };
        self.runtime_contract
            .set(Some(ImguiViewportRuntimeContract {
                backend_platform_user_data: io.BackendPlatformUserData,
                backend_platform_name: io.BackendPlatformName,
                owned_flags: io.BackendFlags & viewport_backend_flag_mask(),
                main_viewport_platform_user_data: main_viewport.PlatformUserData,
                main_viewport_platform_handle: main_viewport.PlatformHandle,
                main_viewport_platform_handle_raw: main_viewport.PlatformHandleRaw,
            }));
    }

    fn record_callback_fault(&self, error: ImguiViewportRuntimeError) {
        if self.callback_fault.get().is_none() {
            self.callback_fault.set(Some(error));
        }
    }

    fn record_monitor_contract(
        &self,
        context: &imgui::Context,
        monitors: &[sys::ImGuiPlatformMonitor],
    ) {
        let raw = unsafe { &(*context.platform_io().as_raw()).Monitors };
        debug_assert_eq!(raw.Size, i32::try_from(monitors.len()).unwrap());
        debug_assert_eq!(raw.Capacity, raw.Size);
        debug_assert_eq!(
            unsafe { std::slice::from_raw_parts(raw.Data, monitors.len()) },
            monitors
        );
        self.monitor_contract
            .replace(Some(ImguiViewportMonitorContract {
                data: raw.Data,
                size: raw.Size,
                capacity: raw.Capacity,
                monitors: monitors.to_vec(),
            }));
    }

    fn owns_current_monitors(&self, context: &imgui::Context) -> bool {
        self.owns_raw_monitors(context.platform_io().as_raw())
    }

    fn owns_raw_monitors(&self, platform_io: *const sys::ImGuiPlatformIO) -> bool {
        let Some(platform_io) = (unsafe { platform_io.as_ref() }) else {
            return false;
        };
        let raw = &platform_io.Monitors;
        let contract = self.monitor_contract.borrow();
        let Some(expected) = contract.as_ref() else {
            return raw.Data.is_null() && raw.Size == 0 && raw.Capacity == 0;
        };
        if raw.Data != expected.data
            || raw.Size != expected.size
            || raw.Capacity != expected.capacity
        {
            return false;
        }

        // The storage address still matches the Dear ImGui allocation published by this bridge.
        // Comparing its contents also detects a backend that rewrote the vector in place.
        let actual = unsafe { std::slice::from_raw_parts(raw.Data, expected.monitors.len()) };
        actual == expected.monitors.as_slice()
    }

    fn retains_any_platform_identity_raw(
        &self,
        context_raw: *mut sys::ImGuiContext,
        main_viewport: *const sys::ImGuiViewport,
    ) -> bool {
        let Some(expected_callbacks) = self.callback_contract.get() else {
            return false;
        };
        let Some(expected_runtime) = self.runtime_contract.get() else {
            return false;
        };
        let io = unsafe { sys::igGetIO_ContextPtr(context_raw) };
        let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context_raw) };
        let (Some(io), Some(main_viewport)) =
            (unsafe { io.as_ref() }, unsafe { main_viewport.as_ref() })
        else {
            return false;
        };

        if !expected_runtime.backend_platform_user_data.is_null()
            && io.BackendPlatformUserData == expected_runtime.backend_platform_user_data
        {
            return true;
        }
        if !expected_runtime.backend_platform_name.is_null()
            && io.BackendPlatformName == expected_runtime.backend_platform_name
        {
            return true;
        }
        if !expected_runtime.main_viewport_platform_user_data.is_null()
            && main_viewport.PlatformUserData == expected_runtime.main_viewport_platform_user_data
        {
            return true;
        }
        if !expected_runtime.main_viewport_platform_handle.is_null()
            && main_viewport.PlatformHandle == expected_runtime.main_viewport_platform_handle
        {
            return true;
        }
        if !expected_runtime.main_viewport_platform_handle_raw.is_null()
            && main_viewport.PlatformHandleRaw == expected_runtime.main_viewport_platform_handle_raw
        {
            return true;
        }

        let Some(actual_callbacks) =
            (unsafe { ImguiViewportCallbackContract::capture_raw(platform_io) })
        else {
            return false;
        };
        if expected_callbacks
            .platform
            .iter()
            .zip(actual_callbacks.platform)
            .any(|(expected, actual)| *expected != 0 && *expected == actual)
        {
            return true;
        }
        if self.monitor_contract.borrow().is_some() && self.owns_raw_monitors(platform_io) {
            return true;
        }

        false
    }

    fn clear_monitors_if_owned(&self, context: &mut imgui::Context) -> bool {
        if !self.owns_current_monitors(context) {
            return false;
        }
        if self.monitor_contract.borrow().is_some() {
            // SAFETY: `owns_current_monitors` proved that this is the allocation published by this
            // bridge, including its complete storage identity and contents.
            unsafe { context.platform_io_mut().set_monitors(&[]) };
        }
        self.monitor_contract.borrow_mut().take();
        true
    }

    fn register_context_owner(context: &imgui::Context, keepalive: &ImguiViewportBridgeKeepalive) {
        VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
            registry
                .borrow_mut()
                .insert(context.as_raw() as usize, Rc::downgrade(keepalive));
        });
    }

    fn unregister_context_owner(
        context: &imgui::Context,
        keepalive: &ImguiViewportBridgeKeepalive,
    ) {
        VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
            let mut registry = registry.borrow_mut();
            let key = context.as_raw() as usize;
            let remove = registry
                .get(&key)
                .and_then(Weak::upgrade)
                .is_some_and(|registered| Rc::ptr_eq(&registered, keepalive));
            if remove {
                registry.remove(&key);
            }
        });
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportCallbackContract {
    fn capture(context: &imgui::Context) -> Self {
        // SAFETY: `Context` owns a live PlatformIO for the duration of this borrow.
        unsafe { Self::capture_raw(context.platform_io().as_raw()) }
            .expect("a live Dear ImGui Context must expose PlatformIO")
    }

    unsafe fn capture_raw(platform_io: *const sys::ImGuiPlatformIO) -> Option<Self> {
        let raw = unsafe { platform_io.as_ref() }?;
        Some(Self {
            platform: [
                raw.Platform_CreateWindow
                    .map_or(0, |callback| callback as usize),
                raw.Platform_DestroyWindow
                    .map_or(0, |callback| callback as usize),
                raw.Platform_ShowWindow
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SetWindowPos
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowPos
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SetWindowSize
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowSize
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowFramebufferScale
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SetWindowFocus
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowFocus
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowMinimized
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SetWindowTitle
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SetWindowAlpha
                    .map_or(0, |callback| callback as usize),
                raw.Platform_UpdateWindow
                    .map_or(0, |callback| callback as usize),
                raw.Platform_RenderWindow
                    .map_or(0, |callback| callback as usize),
                raw.Platform_SwapBuffers
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowDpiScale
                    .map_or(0, |callback| callback as usize),
                raw.Platform_OnChangedViewport
                    .map_or(0, |callback| callback as usize),
                raw.Platform_GetWindowWorkAreaInsets
                    .map_or(0, |callback| callback as usize),
                raw.Platform_CreateVkSurface
                    .map_or(0, |callback| callback as usize),
            ],
            renderer: [
                raw.Renderer_CreateWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_DestroyWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_SetWindowSize
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_RenderWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_SwapBuffers
                    .map_or(0, |callback| callback as usize),
            ],
        })
    }

    fn first_drift(self, actual: Self) -> Option<ImguiViewportCallbackOwnershipError> {
        const PLATFORM_SLOTS: [&str; 20] = [
            "Platform_CreateWindow",
            "Platform_DestroyWindow",
            "Platform_ShowWindow",
            "Platform_SetWindowPos",
            "Platform_GetWindowPos",
            "Platform_SetWindowSize",
            "Platform_GetWindowSize",
            "Platform_GetWindowFramebufferScale",
            "Platform_SetWindowFocus",
            "Platform_GetWindowFocus",
            "Platform_GetWindowMinimized",
            "Platform_SetWindowTitle",
            "Platform_SetWindowAlpha",
            "Platform_UpdateWindow",
            "Platform_RenderWindow",
            "Platform_SwapBuffers",
            "Platform_GetWindowDpiScale",
            "Platform_OnChangedViewport",
            "Platform_GetWindowWorkAreaInsets",
            "Platform_CreateVkSurface",
        ];
        for ((actual, expected), slot) in actual
            .platform
            .into_iter()
            .zip(self.platform)
            .zip(PLATFORM_SLOTS)
        {
            if actual == expected {
                continue;
            }
            return Some(if expected == 0 {
                ImguiViewportCallbackOwnershipError::PlatformCallbackInstalled { slot }
            } else {
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced { slot }
            });
        }

        const RENDERER_SLOTS: [&str; 5] = [
            "Renderer_CreateWindow",
            "Renderer_DestroyWindow",
            "Renderer_SetWindowSize",
            "Renderer_RenderWindow",
            "Renderer_SwapBuffers",
        ];
        actual
            .renderer
            .into_iter()
            .zip(self.renderer)
            .zip(RENDERER_SLOTS)
            .find_map(|((actual, expected), slot)| {
                (actual != expected).then_some(
                    ImguiViewportCallbackOwnershipError::RendererCallbackInstalled { slot },
                )
            })
    }
}

/// Native viewport runtime failure reported through Context lifecycle operations.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportRuntimeError {
    /// A native callback re-entered the Bevy viewport runtime before its prior call completed.
    CallbackReentered,
    /// A native backend field changed after the Bevy viewport bridge claimed it.
    CallbackOwnership(ImguiViewportCallbackOwnershipError),
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::fmt::Display for ImguiViewportRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CallbackReentered => {
                formatter.write_str("a Dear ImGui viewport callback re-entered the Bevy runtime")
            }
            Self::CallbackOwnership(error) => error.fmt(formatter),
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::error::Error for ImguiViewportRuntimeError {}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct ImguiViewportBridgeState {
    commands: Vec<ImguiViewportCommand>,
    viewport_windows: HashMap<ImguiViewportId, Entity>,
    viewport_cameras: HashMap<ImguiViewportId, Entity>,
    pending_ecs_despawns: HashSet<Entity>,
    viewport_feedback: HashMap<ImguiViewportId, ImguiViewportFeedback>,
    viewport_flags: HashMap<ImguiViewportId, imgui::ViewportFlags>,
    pending_client_placements: HashMap<ImguiViewportId, PendingClientPlacement>,
    viewport_handles: HashMap<ImguiViewportId, Box<ImguiViewportPlatformHandle>>,
    retired_viewport_handles: HashMap<ImguiViewportId, Box<ImguiViewportPlatformHandle>>,
    focus_next_frame: HashSet<ImguiViewportId>,
    focus_ready: HashSet<ImguiViewportId>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug)]
struct PendingClientPlacement {
    pos: [f32; 2],
    dpi_scale: f32,
    show_requested: bool,
    focus_requested: bool,
}

/// Identifies one exact Dear ImGui viewport without retaining a dereferenceable native pointer.
///
/// Dear ImGui may omit a still-live viewport from `PlatformIO.Viewports`, and can later reuse its
/// numeric ID. Cleanup therefore resolves the ID through Dear ImGui's internal registry and
/// verifies the address before touching native fields.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ImguiViewportIdentity {
    id: ImguiViewportId,
    address: usize,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportIdentity {
    fn capture(viewport: &imgui::Viewport) -> Self {
        Self {
            id: viewport.id(),
            address: viewport.as_raw() as usize,
        }
    }

    unsafe fn resolve(self) -> Option<*mut sys::ImGuiViewport> {
        let viewport = unsafe { sys::igFindViewportByID(self.id.raw()) };
        (!viewport.is_null() && viewport as usize == self.address).then_some(viewport)
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Debug)]
struct ImguiViewportPlatformHandle {
    identity: ImguiViewportIdentity,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy)]
struct ImguiViewportHandleRef {
    identity: ImguiViewportIdentity,
    pointer: *mut c_void,
    recreate_platform_window: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeState {
    fn queue(&mut self, command: ImguiViewportCommand) {
        self.commands.push(command);
    }

    fn platform_handle(&mut self, identity: ImguiViewportIdentity) -> *mut c_void {
        let viewport_id = identity.id;
        if self
            .viewport_handles
            .get(&viewport_id)
            .is_some_and(|handle| handle.identity != identity)
        {
            // The old native viewport was removed before Dear ImGui reused its ID. No retained
            // address is dereferenced here, so this only releases a Rust-side stale handle.
            self.viewport_handles.remove(&viewport_id);
        }
        if !self.viewport_handles.contains_key(&viewport_id)
            && let Some(handle) = self.retired_viewport_handles.remove(&viewport_id)
            && handle.identity == identity
        {
            self.viewport_handles.insert(viewport_id, handle);
        }
        let handle = self
            .viewport_handles
            .entry(viewport_id)
            .or_insert_with(|| Box::new(ImguiViewportPlatformHandle { identity }));
        debug_assert_eq!(handle.identity, identity);
        (&mut **handle as *mut ImguiViewportPlatformHandle).cast::<c_void>()
    }

    fn take_platform_handle(
        &mut self,
        identity: ImguiViewportIdentity,
    ) -> Option<Box<ImguiViewportPlatformHandle>> {
        let viewport_id = identity.id;
        if self
            .viewport_handles
            .get(&viewport_id)
            .is_some_and(|handle| handle.identity == identity)
        {
            return self.viewport_handles.remove(&viewport_id);
        }
        if self
            .retired_viewport_handles
            .get(&viewport_id)
            .is_some_and(|handle| handle.identity == identity)
        {
            return self.retired_viewport_handles.remove(&viewport_id);
        }
        None
    }

    fn retire_stale_platform_handles(&mut self, live_viewports: &HashSet<ImguiViewportId>) {
        let stale = self
            .viewport_handles
            .keys()
            .filter(|viewport_id| !live_viewports.contains(viewport_id))
            .copied()
            .collect::<Vec<_>>();
        for viewport_id in stale {
            if let Some(handle) = self.viewport_handles.remove(&viewport_id) {
                self.retired_viewport_handles.insert(viewport_id, handle);
            }
        }
    }

    fn set_viewport_flags(
        &mut self,
        viewport_id: ImguiViewportId,
        flags: imgui::ViewportFlags,
    ) -> Option<imgui::ViewportFlags> {
        self.viewport_flags.insert(viewport_id, flags)
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridge {
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn commands(&self) -> Vec<ImguiViewportCommand> {
        self.inner.state.borrow().commands.clone()
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn queue(&mut self, command: ImguiViewportCommand) {
        self.inner.state.borrow_mut().queue(command);
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn queue_for_context(
        &self,
        context_id: imgui::ContextId,
        command: ImguiViewportCommand,
    ) -> bool {
        let Some(context) = self.context(context_id) else {
            return false;
        };
        context.inner.state.borrow_mut().queue(command);
        true
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn drain_commands(
        &mut self,
    ) -> Result<Vec<ImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self.inner.state.borrow_mut().commands.drain(..).collect())
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn callback_error(&self) -> Option<ImguiViewportRuntimeError> {
        self.inner.callback_fault.get()
    }

    /// Return the Bevy window currently mapped to one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context. Callers must retain the
    /// `ContextId` that created the viewport rather than assuming numeric IDs are process-wide.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn viewport_window(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<Entity> {
        self.context(context_id)
            .and_then(|context| context.viewport_window(viewport_id))
    }

    pub(crate) fn viewport_for_window(
        &self,
        context_id: imgui::ContextId,
        entity: Entity,
    ) -> Option<ImguiViewportId> {
        self.context(context_id)
            .and_then(|context| context.viewport_for_window(entity))
    }

    pub(crate) fn viewport_desktop_origin_for_window(
        &self,
        context_id: imgui::ContextId,
        entity: Entity,
    ) -> Option<[f32; 2]> {
        let context = self.context(context_id)?;
        let viewport_id = context.viewport_for_window(entity)?;
        context
            .viewport_feedback(viewport_id)
            .map(|feedback| feedback.pos)
    }

    /// Return the Bevy camera currently mapped to one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context.
    #[cfg(all(
        test,
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    #[must_use]
    pub fn viewport_camera(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<Entity> {
        self.context(context_id)
            .and_then(|context| context.viewport_camera(viewport_id))
    }

    /// Return the latest Bevy-observed state for one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context.
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn viewport_feedback(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportFeedback> {
        self.context(context_id)
            .and_then(|context| context.viewport_feedback(viewport_id))
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn set_viewport_feedback_for_test(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
        feedback: ImguiViewportFeedback,
    ) {
        self.context(context_id)
            .expect("the test viewport Context must remain registered")
            .set_viewport_feedback(viewport_id, feedback);
    }

    /// Returns a deferred callback failure from the native callback boundary.
    ///
    /// Reading the error does not clear it. The failure remains sticky until the viewport bridge is
    /// torn down and rebuilt, so callers cannot accidentally resume from a partially observed
    /// callback sequence.
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn callback_error_for(
        &self,
        context_id: imgui::ContextId,
    ) -> Option<ImguiViewportRuntimeError> {
        self.context(context_id)
            .and_then(|context| context.callback_error())
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn clear_viewport_state(&mut self) {
        self.inner.clear_viewport_state();
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn keepalive(&self) -> ImguiViewportBridgeKeepalive {
        Rc::clone(&self.inner)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn register_context(
        &mut self,
        context_id: imgui::ContextId,
        keepalive: ImguiViewportBridgeKeepalive,
    ) {
        keepalive.set_context_id(context_id);
        let mut contexts = self.contexts.borrow_mut();
        match contexts.entry(context_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(keepalive);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!("a Dear ImGui Context cannot register two viewport bridge allocations");
            }
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn set_viewport_window(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner
            .state
            .borrow_mut()
            .viewport_windows
            .insert(viewport_id, entity);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn registration(&self) -> ImguiViewportBridgeRegistration {
        ImguiViewportBridgeRegistration {
            contexts: Rc::clone(&self.contexts),
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn context(
        &self,
        context_id: imgui::ContextId,
    ) -> Option<ImguiViewportBridgeContext> {
        self.contexts
            .borrow()
            .get(&context_id)
            .cloned()
            .map(|inner| ImguiViewportBridgeContext { context_id, inner })
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn contexts(&self) -> Vec<ImguiViewportBridgeContext> {
        let mut contexts = self
            .contexts
            .borrow()
            .iter()
            .map(|(&context_id, inner)| ImguiViewportBridgeContext {
                context_id,
                inner: Rc::clone(inner),
            })
            .collect::<Vec<_>>();
        contexts.sort_by_key(|context| context.context_id.get().get());
        contexts
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeRegistration {
    pub(crate) fn register_context(
        &self,
        context_id: imgui::ContextId,
        keepalive: ImguiViewportBridgeKeepalive,
    ) {
        keepalive.set_context_id(context_id);
        let mut contexts = self.contexts.borrow_mut();
        match contexts.entry(context_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(keepalive);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!("a Dear ImGui Context cannot register two viewport bridge allocations");
            }
        }
    }

    pub(crate) fn unregister_context(&self, context_id: imgui::ContextId) {
        self.contexts.borrow_mut().remove(&context_id);
    }
}

/// A Context-qualified view of the native viewport bridge.
///
/// The ECS bridge is global because Bevy resources are global, but all mutable platform state is
/// owned by this per-Context handle. Keeping the Context id beside the keepalive makes it
/// impossible for a viewport command to accidentally resolve another Context's numeric id.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone)]
pub(crate) struct ImguiViewportBridgeContext {
    pub(crate) context_id: imgui::ContextId,
    pub(crate) inner: ImguiViewportBridgeKeepalive,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeContext {
    fn drain_commands(&self) -> Result<Vec<ImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self.inner.state.borrow_mut().commands.drain(..).collect())
    }

    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(test)]
    fn callback_error(&self) -> Option<ImguiViewportRuntimeError> {
        self.inner.callback_fault.get()
    }

    fn set_viewport_window(&self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner
            .state
            .borrow_mut()
            .viewport_windows
            .insert(viewport_id, entity);
    }

    pub(crate) fn viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .viewport_windows
            .get(&viewport_id)
            .copied()
    }

    pub(crate) fn viewport_for_window(&self, entity: Entity) -> Option<ImguiViewportId> {
        self.inner
            .state
            .borrow()
            .viewport_windows
            .iter()
            .find_map(|(&viewport_id, &window)| (window == entity).then_some(viewport_id))
    }

    fn remove_viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .viewport_windows
            .remove(&viewport_id)
    }

    #[cfg(feature = "render")]
    fn set_viewport_camera(&self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner
            .state
            .borrow_mut()
            .viewport_cameras
            .insert(viewport_id, entity);
    }

    #[cfg(feature = "render")]
    fn viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .viewport_cameras
            .get(&viewport_id)
            .copied()
    }

    #[cfg(feature = "render")]
    fn remove_viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .viewport_cameras
            .remove(&viewport_id)
    }

    pub(crate) fn viewport_feedback(
        &self,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportFeedback> {
        self.inner
            .state
            .borrow()
            .viewport_feedback
            .get(&viewport_id)
            .copied()
    }

    fn pending_client_position(&self, viewport_id: ImguiViewportId) -> Option<[f32; 2]> {
        self.inner
            .state
            .borrow()
            .pending_client_placements
            .get(&viewport_id)
            .map(|placement| placement.pos)
    }

    fn set_viewport_feedback(&self, viewport_id: ImguiViewportId, feedback: ImguiViewportFeedback) {
        self.inner
            .state
            .borrow_mut()
            .viewport_feedback
            .insert(viewport_id, feedback);
    }

    fn remove_viewport_feedback(&self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .viewport_feedback
            .remove(&viewport_id);
    }

    fn remove_viewport_flags(&self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .viewport_flags
            .remove(&viewport_id);
    }

    fn show_should_focus(&self, viewport_id: ImguiViewportId) -> bool {
        !self
            .inner
            .state
            .borrow()
            .viewport_flags
            .get(&viewport_id)
            .is_some_and(|flags| flags.contains(imgui::ViewportFlags::NO_FOCUS_ON_APPEARING))
    }

    fn request_focus_next_frame(&self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .focus_next_frame
            .insert(viewport_id);
    }

    fn clear_focus_request(&self, viewport_id: ImguiViewportId) {
        let mut state = self.inner.state.borrow_mut();
        state.focus_next_frame.remove(&viewport_id);
        state.focus_ready.remove(&viewport_id);
    }

    fn take_all_ecs_entities_for_release(&self) -> HashSet<Entity> {
        self.inner.take_all_ecs_entities_for_release()
    }

    fn pending_ecs_despawns(&self) -> HashSet<Entity> {
        self.inner.pending_ecs_despawns()
    }

    fn mapped_ecs_entities(&self) -> HashSet<Entity> {
        let state = self.inner.state.borrow();
        state
            .viewport_windows
            .values()
            .chain(state.viewport_cameras.values())
            .copied()
            .collect()
    }

    fn track_ecs_despawn(&self, entity: Entity) {
        self.inner.track_ecs_despawn(entity);
    }

    fn track_ecs_despawns(&self, entities: impl IntoIterator<Item = Entity>) {
        self.inner.track_ecs_despawns(entities);
    }

    fn acknowledge_ecs_despawns(&self, mut entity_is_live: impl FnMut(Entity) -> bool) {
        self.inner.acknowledge_ecs_despawns(&mut entity_is_live);
    }
}

pub(crate) fn install_viewport_bridge(app: &mut App) {
    app.init_resource::<ImguiNativeViewportSupport>();
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        if !sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            missing_platform_io_aggregate_hooks();
        }
        if app.world().get_non_send::<ImguiViewportBridge>().is_none() {
            app.insert_non_send(ImguiViewportBridge::default());
        }
        app.add_message::<WindowMoved>();
        app.add_message::<WindowResized>();
        app.add_message::<WindowCloseRequested>();
        app.add_message::<WindowOccluded>();
        app.add_systems(
            PreUpdate,
            sync_os_viewport_window_events.before(crate::input::ImguiInputSystems),
        );
        app.add_systems(
            crate::schedule::ImguiContextDriver,
            (
                apply_viewport_commands_system,
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred()
                .in_set(crate::schedule::ImguiContextDriverSystems::Platform),
        );
        app.add_systems(
            Last,
            (
                cleanup_secondary_viewports_when_host_is_unavailable,
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred()
                .before(ExitSystems),
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[cold]
fn missing_platform_io_aggregate_hooks() -> ! {
    panic!("dear-imgui-bevy multi-viewport requires PlatformIO aggregate ABI hooks")
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImguiViewportCallbackInstallError {
    BackendPlatformUserData,
    BackendPlatformName,
    BackendFlag { flag: &'static str },
    CallbackSlot { slot: &'static str },
    MainViewportField { field: &'static str },
    PlatformMonitors,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::fmt::Display for ImguiViewportCallbackInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendPlatformUserData => {
                formatter.write_str("Dear ImGui BackendPlatformUserData is already owned")
            }
            Self::BackendPlatformName => {
                formatter.write_str("Dear ImGui BackendPlatformName is already owned")
            }
            Self::BackendFlag { flag } => {
                write!(
                    formatter,
                    "Dear ImGui backend flag `{flag}` is already owned"
                )
            }
            Self::CallbackSlot { slot } => {
                write!(formatter, "Dear ImGui {slot} callback is already owned")
            }
            Self::MainViewportField { field } => {
                write!(
                    formatter,
                    "Dear ImGui main viewport {field} is already owned"
                )
            }
            Self::PlatformMonitors => {
                formatter.write_str("Dear ImGui PlatformIO.Monitors is already owned")
            }
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::error::Error for ImguiViewportCallbackInstallError {}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportCallbackOwnershipError {
    /// Another backend replaced `BackendPlatformUserData` while Bevy's callbacks were installed.
    BackendPlatformUserDataReplaced,
    /// Another backend replaced Bevy's exact `BackendPlatformName` allocation.
    BackendPlatformNameReplaced,
    /// Another backend changed a capability bit owned by the Bevy viewport bridge.
    BackendFlagReplaced { flag: &'static str },
    /// Another backend replaced one of Bevy's platform callbacks.
    PlatformCallbackReplaced { slot: &'static str },
    /// A foreign platform callback appeared while Bevy-owned platform handles were live.
    PlatformCallbackInstalled { slot: &'static str },
    /// A renderer callback appeared while Bevy-owned platform handles were live.
    RendererCallbackInstalled { slot: &'static str },
    /// Another backend replaced the monitor vector published by Bevy.
    PlatformMonitorsReplaced,
    /// A viewport field no longer contained the handle allocation owned by Bevy.
    ViewportFieldReplaced { field: &'static str },
    /// The bridge's installed callback fingerprint was unavailable.
    CallbackContractUnavailable,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::fmt::Display for ImguiViewportCallbackOwnershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendPlatformUserDataReplaced => {
                formatter.write_str("Dear ImGui BackendPlatformUserData was replaced")
            }
            Self::BackendPlatformNameReplaced => {
                formatter.write_str("Dear ImGui BackendPlatformName was replaced")
            }
            Self::BackendFlagReplaced { flag } => {
                write!(formatter, "Dear ImGui backend flag `{flag}` was replaced")
            }
            Self::PlatformCallbackReplaced { slot } => {
                write!(formatter, "Dear ImGui {slot} callback was replaced")
            }
            Self::PlatformCallbackInstalled { slot } => {
                write!(
                    formatter,
                    "foreign Dear ImGui {slot} callback was installed"
                )
            }
            Self::RendererCallbackInstalled { slot } => {
                write!(
                    formatter,
                    "foreign Dear ImGui {slot} callback was installed"
                )
            }
            Self::PlatformMonitorsReplaced => {
                formatter.write_str("Dear ImGui PlatformIO.Monitors was replaced")
            }
            Self::ViewportFieldReplaced { field } => {
                write!(formatter, "Dear ImGui viewport {field} was replaced")
            }
            Self::CallbackContractUnavailable => formatter
                .write_str("Dear ImGui viewport callback ownership contract was unavailable"),
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::error::Error for ImguiViewportCallbackOwnershipError {}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn validate_platform_callback_install(
    context: &mut imgui::Context,
) -> Result<(), ImguiViewportCallbackInstallError> {
    if !context.io().backend_platform_user_data().is_null() {
        return Err(ImguiViewportCallbackInstallError::BackendPlatformUserData);
    }
    if context.io().backend_platform_name().is_some() {
        return Err(ImguiViewportCallbackInstallError::BackendPlatformName);
    }
    let flags = context.io().backend_flags();
    for (flag, name) in [
        (
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS,
            "PLATFORM_HAS_VIEWPORTS",
        ),
        (
            imgui::BackendFlags::RENDERER_HAS_VIEWPORTS,
            "RENDERER_HAS_VIEWPORTS",
        ),
        (
            imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
            "HAS_MOUSE_HOVERED_VIEWPORT",
        ),
    ] {
        if flags.contains(flag) {
            return Err(ImguiViewportCallbackInstallError::BackendFlag { flag: name });
        }
    }

    let main_viewport = context.main_viewport();
    for (occupied, field) in [
        (
            !main_viewport.platform_user_data().is_null(),
            "PlatformUserData",
        ),
        (!main_viewport.platform_handle().is_null(), "PlatformHandle"),
        (
            !main_viewport.platform_handle_raw().is_null(),
            "PlatformHandleRaw",
        ),
    ] {
        if occupied {
            return Err(ImguiViewportCallbackInstallError::MainViewportField { field });
        }
    }
    let raw = unsafe { &*context.platform_io().as_raw() };
    if !raw.Monitors.Data.is_null() || raw.Monitors.Size != 0 || raw.Monitors.Capacity != 0 {
        return Err(ImguiViewportCallbackInstallError::PlatformMonitors);
    }
    macro_rules! reject_occupied_slots {
        ($($slot:ident),+ $(,)?) => {
            $(
                if raw.$slot.is_some() {
                    return Err(ImguiViewportCallbackInstallError::CallbackSlot {
                        slot: stringify!($slot),
                    });
                }
            )+
        };
    }
    reject_occupied_slots!(
        Platform_CreateWindow,
        Platform_DestroyWindow,
        Platform_ShowWindow,
        Platform_SetWindowPos,
        Platform_GetWindowPos,
        Platform_SetWindowSize,
        Platform_GetWindowSize,
        Platform_GetWindowFramebufferScale,
        Platform_SetWindowFocus,
        Platform_GetWindowFocus,
        Platform_GetWindowMinimized,
        Platform_SetWindowTitle,
        Platform_SetWindowAlpha,
        Platform_UpdateWindow,
        Platform_RenderWindow,
        Platform_SwapBuffers,
        Platform_GetWindowDpiScale,
        Platform_OnChangedViewport,
        Platform_GetWindowWorkAreaInsets,
        Platform_CreateVkSurface,
        Renderer_CreateWindow,
        Renderer_DestroyWindow,
        Renderer_SetWindowSize,
        Renderer_RenderWindow,
        Renderer_SwapBuffers,
    );
    Ok(())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn preflight_owned_platform_callbacks(
    context: &mut imgui::Context,
) -> Result<(), ImguiViewportCallbackInstallError> {
    validate_platform_callback_install(context)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) unsafe fn install_owned_platform_callbacks(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackInstallError> {
    keepalive.set_context_id(context.id());
    validate_platform_callback_install(context)?;
    let bridge_ptr = Rc::as_ptr(keepalive).cast_mut().cast::<c_void>();
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_create_window_raw(Some(platform_create_window_raw_callback));
        platform_io.set_platform_destroy_window_raw(Some(platform_destroy_window_raw_callback));
        platform_io.set_platform_show_window_raw(Some(platform_show_window_raw_callback));
        platform_io.set_platform_set_window_pos_raw(Some(platform_set_window_pos_raw_callback));
        platform_io.set_platform_set_window_size_raw(Some(platform_set_window_size_raw_callback));
        platform_io.set_platform_set_window_focus_raw(Some(platform_set_window_focus_raw_callback));
        platform_io.set_platform_set_window_title_raw(Some(platform_set_window_title_raw_callback));
        platform_io.set_platform_update_window_raw(Some(platform_update_window_raw_callback));
        platform_io.set_platform_get_window_pos_raw(Some(platform_get_window_pos_raw_callback));
        platform_io.set_platform_get_window_size_raw(Some(platform_get_window_size_raw_callback));
        platform_io.set_platform_get_window_framebuffer_scale_raw(Some(
            platform_get_window_framebuffer_scale_raw_callback,
        ));
        platform_io.set_platform_get_window_dpi_scale_raw(Some(
            platform_get_window_dpi_scale_raw_callback,
        ));
        platform_io.set_platform_get_window_focus_raw(Some(platform_get_window_focus_raw_callback));
        platform_io.set_platform_get_window_minimized_raw(Some(
            platform_get_window_minimized_raw_callback,
        ));
        context.io_mut().set_backend_platform_user_data(bridge_ptr);
    }
    keepalive.record_callback_contract(context);
    ImguiViewportBridgeShared::register_context_owner(context, keepalive);
    Ok(())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn viewport_backend_flag_mask() -> i32 {
    (imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
        | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
        | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
        .bits()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn first_owned_flag_drift(
    expected: i32,
    actual: i32,
) -> Option<ImguiViewportCallbackOwnershipError> {
    for (flag, name) in [
        (
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS.bits(),
            "PLATFORM_HAS_VIEWPORTS",
        ),
        (
            imgui::BackendFlags::RENDERER_HAS_VIEWPORTS.bits(),
            "RENDERER_HAS_VIEWPORTS",
        ),
        (
            imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT.bits(),
            "HAS_MOUSE_HOVERED_VIEWPORT",
        ),
    ] {
        if expected & flag != actual & flag {
            return Some(ImguiViewportCallbackOwnershipError::BackendFlagReplaced { flag: name });
        }
    }
    None
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn validate_aggregate_callback_contract(
    platform_io: *mut sys::ImGuiPlatformIO,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    macro_rules! validate_aggregate_slot {
        ($clear:path, $set:path, $callback:path, $slot:literal) => {{
            if !unsafe { $clear(platform_io, $callback) } {
                return Err(
                    ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced { slot: $slot },
                );
            }
            unsafe { $set(platform_io, Some($callback)) };
        }};
    }

    validate_aggregate_slot!(
        sys::ImGuiPlatformIO_ClearPlatformSetWindowPosIfPointerParam,
        sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam,
        platform_set_window_pos_raw_callback,
        "Platform_SetWindowPos"
    );
    validate_aggregate_slot!(
        sys::ImGuiPlatformIO_ClearPlatformSetWindowSizeIfPointerParam,
        sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam,
        platform_set_window_size_raw_callback,
        "Platform_SetWindowSize"
    );
    Ok(())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn validate_hidden_callback_contract_raw(
    context_raw: *mut sys::ImGuiContext,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context_raw) };
    if platform_io.is_null() {
        return Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
    }
    // SAFETY: the caller enters through ContextPlatformWindowTeardown or ContextBinding, which
    // keeps this Context current and prevents concurrent PlatformIO access for the callback
    // contract transaction.
    let platform_io = unsafe { imgui::PlatformIo::from_raw_mut(platform_io) };
    unsafe {
        if !platform_io
            .clear_platform_set_window_pos_if_pointer_callback(platform_set_window_pos_raw_callback)
        {
            return Err(
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                    slot: "Platform_SetWindowPos",
                },
            );
        }
        platform_io.set_platform_set_window_pos_raw(Some(platform_set_window_pos_raw_callback));
        if !platform_io.clear_platform_set_window_size_if_pointer_callback(
            platform_set_window_size_raw_callback,
        ) {
            return Err(
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                    slot: "Platform_SetWindowSize",
                },
            );
        }
        platform_io.set_platform_set_window_size_raw(Some(platform_set_window_size_raw_callback));
        if !platform_io
            .clear_platform_get_window_pos_if_raw_callback(platform_get_window_pos_raw_callback)
        {
            return Err(
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                    slot: "Platform_GetWindowPos",
                },
            );
        }
        platform_io.set_platform_get_window_pos_raw(Some(platform_get_window_pos_raw_callback));
        if !platform_io
            .clear_platform_get_window_size_if_raw_callback(platform_get_window_size_raw_callback)
        {
            return Err(
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                    slot: "Platform_GetWindowSize",
                },
            );
        }
        platform_io.set_platform_get_window_size_raw(Some(platform_get_window_size_raw_callback));
        if !platform_io.clear_platform_get_window_framebuffer_scale_if_raw_callback(
            platform_get_window_framebuffer_scale_raw_callback,
        ) {
            return Err(
                ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                    slot: "Platform_GetWindowFramebufferScale",
                },
            );
        }
        platform_io.set_platform_get_window_framebuffer_scale_raw(Some(
            platform_get_window_framebuffer_scale_raw_callback,
        ));
    }
    Ok(())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn latch_platform_ownership_fault(
    context_raw: *mut sys::ImGuiContext,
    main_viewport: *const sys::ImGuiViewport,
    keepalive: &ImguiViewportBridgeKeepalive,
    error: ImguiViewportCallbackOwnershipError,
) -> ImguiViewportCallbackOwnershipError {
    keepalive.record_callback_fault(ImguiViewportRuntimeError::CallbackOwnership(error));
    revoke_platform_capabilities_if_still_owned_raw(context_raw, main_viewport, keepalive);
    error
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn validate_platform_contract_raw(
    context_raw: *mut sys::ImGuiContext,
    main_viewport: *const sys::ImGuiViewport,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let io = unsafe { sys::igGetIO_ContextPtr(context_raw) };
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context_raw) };
    let Some(io_ref) = (unsafe { io.as_ref() }) else {
        return Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
    };
    let expected_user_data = Rc::as_ptr(keepalive).cast_mut().cast::<c_void>();
    if io_ref.BackendPlatformUserData != expected_user_data {
        return Err(ImguiViewportCallbackOwnershipError::BackendPlatformUserDataReplaced);
    }
    let Some(expected_callbacks) = keepalive.callback_contract.get() else {
        return Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
    };
    let actual_callbacks = unsafe { ImguiViewportCallbackContract::capture_raw(platform_io) }
        .ok_or(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable)?;
    if let Some(error) = expected_callbacks.first_drift(actual_callbacks) {
        return Err(error);
    }
    let Some(expected_runtime) = keepalive.runtime_contract.get() else {
        return Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
    };
    let Some(main_viewport) = (unsafe { main_viewport.as_ref() }) else {
        return Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
    };
    for (actual, expected, field) in [
        (
            main_viewport.PlatformUserData,
            expected_runtime.main_viewport_platform_user_data,
            "PlatformUserData",
        ),
        (
            main_viewport.PlatformHandle,
            expected_runtime.main_viewport_platform_handle,
            "PlatformHandle",
        ),
        (
            main_viewport.PlatformHandleRaw,
            expected_runtime.main_viewport_platform_handle_raw,
            "PlatformHandleRaw",
        ),
    ] {
        if actual != expected {
            return Err(ImguiViewportCallbackOwnershipError::ViewportFieldReplaced { field });
        }
    }
    if io_ref.BackendPlatformName != expected_runtime.backend_platform_name {
        return Err(ImguiViewportCallbackOwnershipError::BackendPlatformNameReplaced);
    }
    if let Some(error) = first_owned_flag_drift(
        expected_runtime.owned_flags,
        io_ref.BackendFlags & viewport_backend_flag_mask(),
    ) {
        return Err(error);
    }
    if !keepalive.owns_raw_monitors(platform_io) {
        return Err(ImguiViewportCallbackOwnershipError::PlatformMonitorsReplaced);
    }
    unsafe { validate_aggregate_callback_contract(platform_io) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn platform_callback_ownership(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let binding = context.binding();
    binding.with_bound_context(|| {
        platform_callback_ownership_raw(
            context.as_raw(),
            unsafe { sys::igGetMainViewport() },
            keepalive,
        )
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn platform_callback_error(
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Option<ImguiViewportRuntimeError> {
    keepalive.callback_fault.get()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn platform_callback_ownership_raw(
    context_raw: *mut sys::ImGuiContext,
    main_viewport: *const sys::ImGuiViewport,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    if let Some(ImguiViewportRuntimeError::CallbackOwnership(error)) =
        keepalive.callback_fault.get()
    {
        return Err(error);
    }
    let validation = validate_platform_contract_raw(context_raw, main_viewport, keepalive)
        .and_then(|()| validate_hidden_callback_contract_raw(context_raw));
    match validation {
        Ok(()) => Ok(()),
        Err(error) => Err(latch_platform_ownership_fault(
            context_raw,
            main_viewport,
            keepalive,
            error,
        )),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn record_owned_platform_name(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) {
    keepalive.record_owned_platform_name(context);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn record_platform_runtime_contract_in_current_context(keepalive: &ImguiViewportBridgeKeepalive) {
    keepalive.record_runtime_contract_raw(unsafe { sys::igGetCurrentContext() });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn platform_capabilities_still_owned(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> bool {
    let main_viewport = context.main_viewport().as_raw();
    keepalive.retains_any_platform_identity_raw(context.as_raw(), main_viewport)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn revoke_platform_capabilities_if_still_owned_raw(
    context_raw: *mut sys::ImGuiContext,
    main_viewport: *const sys::ImGuiViewport,
    keepalive: &ImguiViewportBridgeKeepalive,
) {
    if !keepalive.retains_any_platform_identity_raw(context_raw, main_viewport) {
        return;
    }
    let Some(io) = (unsafe { sys::igGetIO_ContextPtr(context_raw).as_mut() }) else {
        return;
    };
    io.BackendFlags &= !viewport_backend_flag_mask();
    io.ConfigFlags &= !imgui::ConfigFlags::VIEWPORTS_ENABLE.bits();
}

/// Stop native viewport activity and enqueue destruction of the bridge-owned ECS entities.
///
/// Callback slots and pointer-bearing userdata deliberately remain installed until the ECS world
/// acknowledges every window and camera despawn. This keeps the platform handle allocations alive
/// for as long as native viewport fields can still refer to them.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn begin_owned_bridge_release(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let main_viewport_id = context.main_viewport().id();
    let mut ownership = platform_callback_ownership(context, keepalive);
    if ownership.is_ok() {
        // The complete contract was validated immediately above. Dear ImGui invokes the destroy
        // callbacks synchronously, and each successful callback invalidates part of that runtime
        // contract before the next viewport is visited. The explicit guard keeps direct unit
        // fixtures working without a Context attachment; production contexts additionally enter
        // the same scope through the core platform-window teardown observer.
        let _teardown = NativePlatformTeardownGuard::enter(&keepalive.native_teardown_in_progress);
        if context.destroy_platform_windows().is_err() {
            ownership = Err(ImguiViewportCallbackOwnershipError::CallbackContractUnavailable);
        }
    }

    keepalive.prepare_ecs_release(main_viewport_id);
    ownership
}

/// Clear native callback and userdata capabilities after the ECS viewport world has drained.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn finish_owned_bridge_release(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) {
    // Monitor ownership was validated before platform teardown. If another backend replaced it
    // while ECS release was pending, preserve the foreign storage and continue clearing only
    // capabilities that still belong to this bridge.
    let _ = keepalive.clear_monitors_if_owned(context);

    clear_owned_platform_callbacks(context);
    clear_imgui_viewport_platform_handles_for_keepalive(context, keepalive, false);
    clear_backend_platform_user_data_if_owned(context, keepalive);
    ImguiViewportBridgeShared::unregister_context_owner(context, keepalive);
}

/// Detach the bridge in one call for direct teardown paths that do not wait on an ECS world.
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn detach_owned_bridge(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let result = begin_owned_bridge_release(context, keepalive);
    finish_owned_bridge_release(context, keepalive);
    abandon_viewport_ecs_release(keepalive);
    result
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_ecs_release_pending(keepalive: &ImguiViewportBridgeKeepalive) -> bool {
    keepalive.has_tracked_ecs_entities()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn finish_viewport_ecs_release(keepalive: &ImguiViewportBridgeKeepalive) {
    keepalive.finish_ecs_release();
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn abandon_viewport_ecs_release(keepalive: &ImguiViewportBridgeKeepalive) {
    keepalive.clear_viewport_state();
    keepalive.callback_contract.set(None);
    keepalive.runtime_contract.set(None);
    keepalive.monitor_contract.borrow_mut().take();
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn clear_owned_platform_callbacks(context: &mut imgui::Context) {
    macro_rules! clear_direct_if_owned {
        ($platform_io:ident, $field:ident, $owned:path, $callback_type:ty, $setter:ident) => {{
            let owned = unsafe { &*$platform_io.as_raw() }
                .$field
                .is_some_and(|callback| std::ptr::fn_addr_eq(callback, $owned as $callback_type));
            if owned {
                unsafe { $platform_io.$setter(None) };
            }
        }};
    }

    let platform_io = context.platform_io_mut();
    clear_direct_if_owned!(
        platform_io,
        Platform_CreateWindow,
        platform_create_window_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport),
        set_platform_create_window_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_DestroyWindow,
        platform_destroy_window_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport),
        set_platform_destroy_window_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_ShowWindow,
        platform_show_window_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport),
        set_platform_show_window_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_SetWindowFocus,
        platform_set_window_focus_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport),
        set_platform_set_window_focus_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_SetWindowTitle,
        platform_set_window_title_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport, *const std::ffi::c_char),
        set_platform_set_window_title_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_UpdateWindow,
        platform_update_window_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport),
        set_platform_update_window_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_GetWindowDpiScale,
        platform_get_window_dpi_scale_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport) -> f32,
        set_platform_get_window_dpi_scale_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_GetWindowFocus,
        platform_get_window_focus_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool,
        set_platform_get_window_focus_raw
    );
    clear_direct_if_owned!(
        platform_io,
        Platform_GetWindowMinimized,
        platform_get_window_minimized_raw_callback,
        unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool,
        set_platform_get_window_minimized_raw
    );
    unsafe {
        platform_io.clear_platform_set_window_pos_if_pointer_callback(
            platform_set_window_pos_raw_callback,
        );
        platform_io.clear_platform_set_window_size_if_pointer_callback(
            platform_set_window_size_raw_callback,
        );
        platform_io
            .clear_platform_get_window_pos_if_raw_callback(platform_get_window_pos_raw_callback);
        platform_io
            .clear_platform_get_window_size_if_raw_callback(platform_get_window_size_raw_callback);
        platform_io.clear_platform_get_window_framebuffer_scale_if_raw_callback(
            platform_get_window_framebuffer_scale_raw_callback,
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn clear_backend_platform_user_data_if_owned(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> bool {
    let expected_user_data = Rc::as_ptr(keepalive).cast_mut().cast::<c_void>();
    if context.io().backend_platform_user_data() != expected_user_data {
        return false;
    }
    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
    }
    true
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_create_window_raw_callback(viewport: *mut sys::ImGuiViewport) {
    unsafe { platform_create_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_destroy_window_raw_callback(viewport: *mut sys::ImGuiViewport) {
    unsafe { platform_destroy_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_show_window_raw_callback(viewport: *mut sys::ImGuiViewport) {
    unsafe { platform_show_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_update_window_raw_callback(viewport: *mut sys::ImGuiViewport) {
    unsafe { platform_update_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_pos_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    pos: *const sys::ImVec2,
) {
    let Some(pos) = (unsafe { pos.as_ref() }) else {
        return;
    };
    unsafe { platform_set_window_pos(viewport.cast(), *pos) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_size_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    let Some(size) = (unsafe { size.as_ref() }) else {
        return;
    };
    unsafe { platform_set_window_size(viewport.cast(), *size) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_focus_raw_callback(viewport: *mut sys::ImGuiViewport) {
    unsafe { platform_set_window_focus(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_title_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    title: *const std::ffi::c_char,
) {
    unsafe { platform_set_window_title(viewport.cast(), title) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_pos_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_pos: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_pos(viewport.cast(), out_pos) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_size_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_size: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_size(viewport.cast(), out_size) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_framebuffer_scale_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_scale: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_framebuffer_scale(viewport.cast(), out_scale) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_dpi_scale_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> f32 {
    unsafe { platform_get_window_dpi_scale(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_focus_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    unsafe { platform_get_window_focus(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_minimized_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    unsafe { platform_get_window_minimized(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn with_current_bridge_mut<R>(
    f: impl FnOnce(&mut ImguiViewportBridgeState) -> R,
) -> Option<R> {
    let current_context = unsafe { sys::igGetCurrentContext() };
    if current_context.is_null() {
        return None;
    }
    let shared = VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
        registry
            .borrow()
            .get(&(current_context as usize))
            .and_then(Weak::upgrade)
    })?;
    if !shared.native_teardown_in_progress.get() {
        if let Some(error) = shared.callback_fault.get() {
            revoke_platform_capabilities_if_still_owned_raw(
                current_context,
                unsafe { sys::igGetMainViewport().cast_const() },
                &shared,
            );
            let _ = error;
            return None;
        }
        if let Err(error) = validate_platform_contract_raw(
            current_context,
            unsafe { sys::igGetMainViewport().cast_const() },
            &shared,
        ) {
            latch_platform_ownership_fault(
                current_context,
                unsafe { sys::igGetMainViewport().cast_const() },
                &shared,
                error,
            );
            return None;
        }
    }
    // The independent registry proves that this Context owns the shared allocation, and the full
    // callback/runtime validation above proves that Dear ImGui still publishes that capability.
    // The fault latch is outside the contested RefCell, so callback reentry records a deferred
    // Rust-side error without aliasing state or unwinding through C.
    let Ok(mut bridge) = shared.state.try_borrow_mut() else {
        shared.record_callback_fault(ImguiViewportRuntimeError::CallbackReentered);
        revoke_platform_capabilities_if_still_owned_raw(
            current_context,
            unsafe { sys::igGetMainViewport().cast_const() },
            &shared,
        );
        return None;
    };
    Some(f(&mut bridge))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_create_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let identity = ImguiViewportIdentity::capture(viewport);
    let Some(result) = (unsafe {
        with_current_bridge_mut(|bridge| {
            for (occupied, field) in [
                (!viewport.platform_user_data().is_null(), "PlatformUserData"),
                (!viewport.platform_handle().is_null(), "PlatformHandle"),
                (
                    !viewport.platform_handle_raw().is_null(),
                    "PlatformHandleRaw",
                ),
            ] {
                if occupied {
                    return Err(ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                        field,
                    });
                }
            }
            let handle = bridge.platform_handle(identity);
            let _ = bridge.set_viewport_flags(viewport.id(), viewport.flags());
            bridge.queue(ImguiViewportCommand::Create(
                ImguiViewportSnapshot::from_viewport(viewport),
            ));
            Ok(handle)
        })
    }) else {
        return;
    };
    let handle = match result {
        Ok(handle) => handle,
        Err(error) => {
            unsafe { latch_current_platform_ownership_fault(error) };
            return;
        }
    };
    // SAFETY: the bridge owns this stable handle until the matching destroy callback clears it.
    unsafe {
        viewport.set_platform_user_data(handle);
        viewport.set_platform_handle(handle);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn latch_current_platform_ownership_fault(error: ImguiViewportCallbackOwnershipError) {
    let current_context = unsafe { sys::igGetCurrentContext() };
    if current_context.is_null() {
        return;
    }
    let shared = VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
        registry
            .borrow()
            .get(&(current_context as usize))
            .and_then(Weak::upgrade)
    });
    if let Some(shared) = shared {
        latch_platform_ownership_fault(
            current_context,
            unsafe { sys::igGetMainViewport().cast_const() },
            &shared,
            error,
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_destroy_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let identity = ImguiViewportIdentity::capture(viewport);
    let viewport_id = identity.id;
    let owned_by_app = viewport
        .flags()
        .contains(imgui::ViewportFlags::OWNED_BY_APP);
    let Some(owned_handle) = (unsafe {
        with_current_bridge_mut(|bridge| {
            let owned_handle = bridge.take_platform_handle(identity);
            if !owned_by_app {
                bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });
            }
            bridge.viewport_flags.remove(&viewport_id);
            bridge.focus_next_frame.remove(&viewport_id);
            bridge.focus_ready.remove(&viewport_id);
            owned_handle
        })
    }) else {
        return;
    };
    let Some(owned_handle) = owned_handle else {
        return;
    };
    let owned_handle_ptr = (&*owned_handle as *const ImguiViewportPlatformHandle)
        .cast_mut()
        .cast::<c_void>();
    // SAFETY: the callback guard proved the bridge contract, and each field is cleared only if it
    // still contains the exact live handle allocation held above. Foreign replacements survive.
    unsafe {
        if viewport.platform_user_data() == owned_handle_ptr {
            viewport.set_platform_user_data(std::ptr::null_mut());
        }
        if viewport.platform_handle() == owned_handle_ptr {
            viewport.set_platform_handle(std::ptr::null_mut());
        }
        if viewport.platform_handle_raw() == owned_handle_ptr {
            viewport.set_platform_handle_raw(std::ptr::null_mut());
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_show_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            let _ = bridge.set_viewport_flags(viewport.id(), viewport.flags());
            bridge.queue(ImguiViewportCommand::Show { id: viewport.id() });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_update_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            let flags = viewport.flags();
            let previous_flags = bridge.set_viewport_flags(viewport.id(), flags);
            bridge.queue(ImguiViewportCommand::Update {
                id: viewport.id(),
                previous_flags,
                flags,
            });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn platform_set_window_pos(viewport: *mut imgui::Viewport, pos: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetPos {
                id: viewport.id(),
                pos: [pos.x, pos.y],
                dpi_scale: (*viewport.as_raw()).DpiScale,
            });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn platform_set_window_size(viewport: *mut imgui::Viewport, size: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetSize {
                id: viewport.id(),
                size: [size.x, size.y],
                dpi_scale: (*viewport.as_raw()).DpiScale,
            });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_focus(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetFocus { id: viewport.id() });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_title(
    viewport: *mut imgui::Viewport,
    title: *const std::ffi::c_char,
) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let title = if title.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(title) }
            .to_string_lossy()
            .into_owned()
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetTitle {
                id: viewport.id(),
                title,
            });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_pos(
    viewport: *mut imgui::Viewport,
    out_pos: *mut sys::ImVec2,
) {
    let Some(feedback) = feedback_for_viewport(viewport) else {
        return;
    };
    let pos = feedback.map(|feedback| feedback.pos).unwrap_or([0.0, 0.0]);
    if let Some(out_pos) = unsafe { out_pos.as_mut() } {
        *out_pos = sys::ImVec2 {
            x: pos[0],
            y: pos[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_size(
    viewport: *mut imgui::Viewport,
    out_size: *mut sys::ImVec2,
) {
    let Some(feedback) = feedback_for_viewport(viewport) else {
        return;
    };
    let size = feedback.map(|feedback| feedback.size).unwrap_or([0.0, 0.0]);
    if let Some(out_size) = unsafe { out_size.as_mut() } {
        *out_size = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_framebuffer_scale(
    viewport: *mut imgui::Viewport,
    out_scale: *mut sys::ImVec2,
) {
    let Some(feedback) = feedback_for_viewport(viewport) else {
        return;
    };
    let scale = feedback
        .map(|feedback| feedback.framebuffer_scale)
        .unwrap_or([1.0, 1.0]);
    if let Some(out_scale) = unsafe { out_scale.as_mut() } {
        *out_scale = sys::ImVec2 {
            x: scale[0],
            y: scale[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_dpi_scale(viewport: *mut imgui::Viewport) -> f32 {
    feedback_for_viewport(viewport)
        .flatten()
        .map(|feedback| feedback.dpi_scale)
        .unwrap_or(1.0)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_focus(viewport: *mut imgui::Viewport) -> bool {
    feedback_for_viewport(viewport)
        .flatten()
        .is_some_and(|feedback| feedback.focused)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_minimized(viewport: *mut imgui::Viewport) -> bool {
    feedback_for_viewport(viewport)
        .flatten()
        .is_some_and(|feedback| feedback.minimized)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn feedback_for_viewport(viewport: *mut imgui::Viewport) -> Option<Option<ImguiViewportFeedback>> {
    let viewport = unsafe { viewport.as_ref() }?;
    unsafe {
        with_current_bridge_mut(|bridge| bridge.viewport_feedback.get(&viewport.id()).copied())
    }
}

#[derive(SystemParam)]
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct OsViewportWindowEvents<'w, 's> {
    moved: MessageReader<'w, 's, WindowMoved>,
    resized: MessageReader<'w, 's, WindowResized>,
    close_requests: MessageReader<'w, 's, WindowCloseRequested>,
    occluded: MessageReader<'w, 's, WindowOccluded>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn sync_os_viewport_window_events(
    mut events: OsViewportWindowEvents,
    windows: Query<&Window>,
    viewport_windows: Query<(Entity, &ImguiViewportWindow, &ImguiViewportOwner)>,
    contexts: Option<NonSendMut<crate::ImguiContexts>>,
    bridge: NonSend<ImguiViewportBridge>,
) {
    let Some(mut contexts) = contexts else {
        events.moved.read().for_each(drop);
        events.resized.read().for_each(drop);
        events.close_requests.read().for_each(drop);
        events.occluded.read().for_each(drop);
        return;
    };
    let window_to_viewport = viewport_windows
        .iter()
        .filter_map(|(entity, marker, owner)| {
            if !owner.matches_window(marker)
                || bridge.viewport_window(marker.context_id, marker.viewport_id) != Some(entity)
            {
                return None;
            }
            Some((entity, (marker.context_id, marker.viewport_id)))
        })
        .collect::<HashMap<_, _>>();
    let mut moved_viewports = HashMap::<imgui::ContextId, HashSet<ImguiViewportId>>::new();
    let mut resized_viewports = HashMap::<imgui::ContextId, HashSet<ImguiViewportId>>::new();
    let mut closed_viewports = HashMap::<imgui::ContextId, HashSet<ImguiViewportId>>::new();

    for event in events.moved.read() {
        if let Some((context_id, viewport_id)) = window_to_viewport.get(&event.window).copied() {
            moved_viewports
                .entry(context_id)
                .or_default()
                .insert(viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let Some(context_bridge) = bridge.context(context_id) else {
                    continue;
                };
                let previous = context_bridge.viewport_feedback(viewport_id);
                let mut feedback =
                    feedback_from_window_for_entity(event.window, window, previous, None);
                if let Some(pos) = context_bridge.pending_client_position(viewport_id) {
                    feedback.pos = pos;
                }
                context_bridge.set_viewport_feedback(viewport_id, feedback);
            }
        }
    }

    for event in events.resized.read() {
        if let Some((context_id, viewport_id)) = window_to_viewport.get(&event.window).copied() {
            resized_viewports
                .entry(context_id)
                .or_default()
                .insert(viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let Some(context_bridge) = bridge.context(context_id) else {
                    continue;
                };
                let previous = context_bridge.viewport_feedback(viewport_id);
                let mut feedback =
                    feedback_from_window_for_entity(event.window, window, previous, None);
                if let Some(pos) = context_bridge.pending_client_position(viewport_id) {
                    feedback.pos = pos;
                }
                context_bridge.set_viewport_feedback(viewport_id, feedback);
            }
        }
    }

    for event in events.close_requests.read() {
        if let Some((context_id, viewport_id)) = window_to_viewport.get(&event.window).copied() {
            closed_viewports
                .entry(context_id)
                .or_default()
                .insert(viewport_id);
        }
    }

    for event in events.occluded.read() {
        if let Some((context_id, viewport_id)) = window_to_viewport.get(&event.window).copied()
            && let Ok(window) = windows.get(event.window)
        {
            let Some(context_bridge) = bridge.context(context_id) else {
                continue;
            };
            let previous = context_bridge.viewport_feedback(viewport_id);
            context_bridge.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(
                    event.window,
                    window,
                    previous,
                    Some(event.occluded),
                ),
            );
        }
    }

    let mut context_ids = moved_viewports.keys().copied().collect::<HashSet<_>>();
    context_ids.extend(resized_viewports.keys().copied());
    context_ids.extend(closed_viewports.keys().copied());
    let mut context_ids = context_ids.into_iter().collect::<Vec<_>>();
    context_ids.sort_by_key(|context_id| context_id.get().get());
    for context_id in context_ids {
        let result = contexts.configure(context_id, |context| {
            mark_platform_viewport_requests(
                context,
                moved_viewports
                    .get(&context_id)
                    .into_iter()
                    .flat_map(|ids| ids.iter().copied()),
                resized_viewports
                    .get(&context_id)
                    .into_iter()
                    .flat_map(|ids| ids.iter().copied()),
                closed_viewports
                    .get(&context_id)
                    .into_iter()
                    .flat_map(|ids| ids.iter().copied()),
            );
        });
        match result {
            Ok(()) => {}
            Err(
                crate::ImguiContextError::TeardownInProgress { .. }
                | crate::ImguiContextError::UnknownContext { .. },
            ) => {}
            Err(error) => {
                panic!("cannot apply Dear ImGui viewport requests for {context_id:?}: {error}")
            }
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn mark_platform_viewport_requests(
    context: &mut imgui::Context,
    moved_viewports: impl IntoIterator<Item = ImguiViewportId>,
    resized_viewports: impl IntoIterator<Item = ImguiViewportId>,
    closed_viewports: impl IntoIterator<Item = ImguiViewportId>,
) {
    let moved = moved_viewports.into_iter().collect::<HashSet<_>>();
    let resized = resized_viewports.into_iter().collect::<HashSet<_>>();
    let closed = closed_viewports.into_iter().collect::<HashSet<_>>();
    if moved.is_empty() && resized.is_empty() && closed.is_empty() {
        return;
    }

    let viewport_ids = moved
        .iter()
        .chain(resized.iter())
        .chain(closed.iter())
        .copied()
        .collect::<HashSet<_>>();
    let binding = context.binding();
    binding.with_bound_context(|| {
        for id in viewport_ids {
            // Dear ImGui filters hidden, inactive, and zero-sized viewports out of the public
            // list. Window events still belong to their live internal viewport.
            let viewport = unsafe { sys::igFindViewportByID(id.raw()) };
            if viewport.is_null() {
                continue;
            }
            // SAFETY: the current Context owns the viewport returned by Dear ImGui's lookup.
            let viewport = unsafe { imgui::Viewport::from_raw_mut(viewport) };
            if moved.contains(&id) {
                viewport.set_platform_request_move(true);
            }
            if resized.contains(&id) {
                viewport.set_platform_request_resize(true);
            }
            if closed.contains(&id) {
                viewport.set_platform_request_close(true);
            }
        }
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[cfg(feature = "render")]
type ViewportCameraComponentPresence = (
    Has<Camera2d>,
    Has<Camera>,
    Has<RenderTarget>,
    Has<CameraRenderGraph>,
    Has<RenderLayers>,
);

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[cfg(feature = "render")]
type ViewportCameraIdentity = (ImguiViewportId, Entity);

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[allow(unused_variables)]
fn apply_viewport_commands_system(
    mut ecs_commands: Commands,
    bridge: NonSend<ImguiViewportBridge>,
    backend_runtime: Res<crate::context::ownership::ImguiBackendRuntime>,
    winit_settings: Option<Res<WinitSettings>>,
    mut windows: Query<&mut Window>,
    mut cursor_options: Query<&mut CursorOptions>,
    viewport_windows: Query<
        (Entity, Option<&ImguiViewportWindow>, &ImguiViewportOwner),
        With<Window>,
    >,
    viewport_cameras: Query<(Entity, Option<&ImguiViewportCamera>, &ImguiViewportOwner)>,
    #[cfg(feature = "render")] viewport_camera_components: Query<ViewportCameraComponentPresence>,
) {
    let contexts = bridge.contexts();
    for context in contexts {
        #[cfg(feature = "render")]
        apply_viewport_commands_for_context(
            &mut ecs_commands,
            &context,
            backend_runtime.config(),
            winit_settings.is_some(),
            &mut windows,
            &mut cursor_options,
            &viewport_windows,
            &viewport_cameras,
            &viewport_camera_components,
        );
        #[cfg(not(feature = "render"))]
        apply_viewport_commands_for_context(
            &mut ecs_commands,
            &context,
            backend_runtime.config(),
            winit_settings.is_some(),
            &mut windows,
            &mut cursor_options,
            &viewport_windows,
            &viewport_cameras,
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[allow(unused_variables)]
fn apply_viewport_commands_for_context(
    ecs_commands: &mut Commands,
    context: &ImguiViewportBridgeContext,
    config: &crate::ImguiPluginConfig,
    uses_winit_window_lifecycle: bool,
    windows: &mut Query<&mut Window>,
    cursor_options: &mut Query<&mut CursorOptions>,
    viewport_windows: &Query<
        (Entity, Option<&ImguiViewportWindow>, &ImguiViewportOwner),
        With<Window>,
    >,
    viewport_cameras: &Query<(Entity, Option<&ImguiViewportCamera>, &ImguiViewportOwner)>,
    #[cfg(feature = "render")] viewport_camera_components: &Query<ViewportCameraComponentPresence>,
) {
    let Ok(queued) = context.drain_commands() else {
        return;
    };
    if context.ecs_release_pending() {
        for entity in context.take_all_ecs_entities_for_release() {
            native_window::release_pointer_capture_for(entity);
            ecs_commands.entity(entity).try_despawn();
        }
        return;
    }
    for entity in context.pending_ecs_despawns() {
        native_window::release_pointer_capture_for(entity);
        ecs_commands.entity(entity).try_despawn();
    }
    if uses_winit_window_lifecycle {
        settle_pending_client_placements(windows, context, winit_window_decoration_offset_desktop);
    }

    let viewport_window_config = config.viewport_window().validate().unwrap_or_else(|error| {
        panic!("invalid Dear ImGui viewport window configuration: {error}")
    });
    let mut feedback_candidates = HashSet::new();
    let mut pending_windows: HashMap<ImguiViewportId, Window> = HashMap::new();
    #[cfg(feature = "render")]
    let mut pending_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut scheduled_camera_despawns = HashSet::new();
    #[cfg(feature = "render")]
    let mut owned_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut recoverable_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let live_cameras = viewport_cameras
        .iter()
        .filter_map(|(entity, marker, owner)| {
            let (context_id, viewport_id) = owner.camera_identity()?;
            if context_id != context.context_id {
                return None;
            }
            owned_cameras.insert(entity);
            if context.viewport_camera(viewport_id) != Some(entity) {
                return None;
            }
            recoverable_cameras.insert((viewport_id, entity));
            if marker.is_none_or(|marker| !owner.matches_camera(marker)) {
                ecs_commands
                    .entity(entity)
                    .insert(ImguiViewportCamera::new(context_id, viewport_id));
            }
            viewport_camera_components
                .get(entity)
                .is_ok_and(
                    |(has_camera_2d, has_camera, has_target, has_graph, has_layers)| {
                        has_camera_2d && has_camera && has_target && has_graph && has_layers
                    },
                )
                .then_some((viewport_id, entity))
        })
        .collect::<HashSet<_>>();
    for command in queued {
        match command {
            ImguiViewportCommand::Create(snapshot) => {
                {
                    let mut state = context.inner.state.borrow_mut();
                    state.viewport_flags.insert(snapshot.id, snapshot.flags);
                    if uses_winit_window_lifecycle
                        && !snapshot.flags.contains(imgui::ViewportFlags::NO_DECORATION)
                    {
                        state.pending_client_placements.insert(
                            snapshot.id,
                            PendingClientPlacement {
                                pos: finite_desktop_pos(snapshot.pos),
                                dpi_scale: positive_finite_or(snapshot.dpi_scale, 1.0),
                                show_requested: false,
                                focus_requested: false,
                            },
                        );
                    } else {
                        state.pending_client_placements.remove(&snapshot.id);
                    }
                }
                let entity = if let Some(entity) = context.viewport_window(snapshot.id) {
                    entity
                } else {
                    let mut cursor_options = CursorOptions::default();
                    apply_viewport_flags_to_cursor_options(snapshot.flags, &mut cursor_options);
                    let entity = ecs_commands
                        .spawn((
                            window_from_snapshot_with_config(&snapshot, viewport_window_config)
                                .expect("the viewport window configuration was validated"),
                            cursor_options,
                            ImguiViewportWindow::new(context.context_id, snapshot.id),
                            ImguiViewportOwner::window(context.context_id, snapshot.id),
                        ))
                        .id();
                    context.set_viewport_window(snapshot.id, entity);
                    entity
                };
                context.set_viewport_feedback(snapshot.id, feedback_from_snapshot(&snapshot));
                #[cfg(feature = "render")]
                ensure_viewport_camera(
                    ecs_commands,
                    context,
                    snapshot.id,
                    entity,
                    viewport_window_config.transparent,
                    snapshot.flags,
                    ViewportCameraReconciliation {
                        live: &live_cameras,
                        recoverable: &recoverable_cameras,
                        pending: &mut pending_cameras,
                    },
                );
                if let Ok(mut window) = windows.get_mut(entity) {
                    apply_snapshot_to_window(&snapshot, entity, &mut window);
                } else {
                    pending_windows.insert(
                        snapshot.id,
                        window_from_snapshot_with_config(&snapshot, viewport_window_config)
                            .expect("the viewport window configuration was validated"),
                    );
                }
                feedback_candidates.insert(snapshot.id);
            }
            ImguiViewportCommand::Destroy { id } => {
                pending_windows.remove(&id);
                if let Some(entity) = context.remove_viewport_window(id) {
                    native_window::release_pointer_capture_for(entity);
                    context.track_ecs_despawn(entity);
                    ecs_commands.entity(entity).try_despawn();
                }
                context.remove_viewport_feedback(id);
                context.remove_viewport_flags(id);
                context
                    .inner
                    .state
                    .borrow_mut()
                    .pending_client_placements
                    .remove(&id);
                context.clear_focus_request(id);
                #[cfg(feature = "render")]
                {
                    pending_cameras.remove(&id);
                    if let Some(entity) = context.remove_viewport_camera(id) {
                        scheduled_camera_despawns.insert(entity);
                        context.track_ecs_despawn(entity);
                        ecs_commands.entity(entity).try_despawn();
                    }
                }
            }
            ImguiViewportCommand::Show { id } => {
                let should_focus = context.show_should_focus(id);
                let show_is_deferred = {
                    let mut state = context.inner.state.borrow_mut();
                    state
                        .pending_client_placements
                        .get_mut(&id)
                        .is_some_and(|placement| {
                            placement.show_requested = true;
                            placement.focus_requested |= should_focus;
                            true
                        })
                };
                if !show_is_deferred {
                    if let Some(window) = pending_windows.get_mut(&id) {
                        window.visible = true;
                        if should_focus {
                            window.focused = false;
                        }
                    } else {
                        with_window_mut(windows, context, id, |window| {
                            window.visible = true;
                            if should_focus {
                                window.focused = false;
                            }
                        });
                    }
                    if should_focus {
                        context.request_focus_next_frame(id);
                    }
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::Update {
                id,
                previous_flags,
                flags,
            } => {
                context
                    .inner
                    .state
                    .borrow_mut()
                    .viewport_flags
                    .insert(id, flags);
                let decoration_changed = previous_flags.is_some_and(|previous| {
                    previous.contains(imgui::ViewportFlags::NO_DECORATION)
                        != flags.contains(imgui::ViewportFlags::NO_DECORATION)
                });
                #[cfg(feature = "render")]
                let renderer_clear_changed = previous_flags.is_some_and(|previous| {
                    previous.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR)
                        != flags.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR)
                });
                let mut was_visible = false;
                if let Some(window) = pending_windows.get_mut(&id) {
                    was_visible = window.visible;
                    if decoration_changed && uses_winit_window_lifecycle {
                        window.visible = false;
                    }
                    apply_viewport_flags_to_window(flags, window);
                } else {
                    with_window_mut(windows, context, id, |window| {
                        was_visible = window.visible;
                        if decoration_changed && uses_winit_window_lifecycle {
                            window.visible = false;
                        }
                        apply_viewport_flags_to_window(flags, window);
                    });
                }
                if decoration_changed
                    && uses_winit_window_lifecycle
                    && let Some(feedback) = context.viewport_feedback(id)
                {
                    let mut state = context.inner.state.borrow_mut();
                    let placement = state.pending_client_placements.entry(id).or_insert(
                        PendingClientPlacement {
                            pos: feedback.pos,
                            dpi_scale: feedback.dpi_scale,
                            show_requested: false,
                            focus_requested: false,
                        },
                    );
                    placement.pos = feedback.pos;
                    placement.dpi_scale = feedback.dpi_scale;
                    placement.show_requested |= was_visible;
                }
                if let Some(entity) = context.viewport_window(id)
                    && let Ok(mut cursor_options) = cursor_options.get_mut(entity)
                {
                    apply_viewport_flags_to_cursor_options(flags, &mut cursor_options);
                } else if let Some(entity) = context.viewport_window(id) {
                    let mut cursor_options = CursorOptions::default();
                    apply_viewport_flags_to_cursor_options(flags, &mut cursor_options);
                    ecs_commands.entity(entity).insert(cursor_options);
                }
                #[cfg(feature = "render")]
                if renderer_clear_changed
                    && let Some(camera) = context.viewport_camera(id)
                    && (live_cameras.contains(&(id, camera))
                        || recoverable_cameras.contains(&(id, camera))
                        || pending_cameras.contains(&id))
                {
                    ecs_commands
                        .entity(camera)
                        .insert(viewport_camera(viewport_window_config.transparent, flags));
                }
            }
            ImguiViewportCommand::SetPos { id, pos, dpi_scale } => {
                if let Some(placement) = context
                    .inner
                    .state
                    .borrow_mut()
                    .pending_client_placements
                    .get_mut(&id)
                {
                    placement.pos = finite_desktop_pos(pos);
                    placement.dpi_scale = positive_finite_or(dpi_scale, 1.0);
                }
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.position = WindowPosition::At(physical_pos_from_desktop(pos, dpi_scale));
                } else {
                    if let Some(entity) = context.viewport_window(id)
                        && let Ok(mut window) = windows.get_mut(entity)
                    {
                        window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
                            entity, pos, dpi_scale,
                        ));
                    }
                }
                if let Some(mut feedback) = context.viewport_feedback(id) {
                    feedback.pos = finite_desktop_pos(pos);
                    context.set_viewport_feedback(id, feedback);
                }
            }
            ImguiViewportCommand::SetSize {
                id,
                size,
                dpi_scale,
            } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    set_window_desktop_size(window, size, dpi_scale);
                } else {
                    with_window_mut(windows, context, id, |window| {
                        set_window_desktop_size(window, size, dpi_scale);
                    });
                }
                if let Some(mut feedback) = context.viewport_feedback(id) {
                    feedback.size = finite_desktop_size(size);
                    context.set_viewport_feedback(id, feedback);
                }
            }
            ImguiViewportCommand::SetFocus { id } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.focused = false;
                } else {
                    with_window_mut(windows, context, id, |window| {
                        window.focused = false;
                    });
                }
                let focus_is_deferred = context
                    .inner
                    .state
                    .borrow_mut()
                    .pending_client_placements
                    .get_mut(&id)
                    .is_some_and(|placement| {
                        placement.focus_requested = true;
                        true
                    });
                if !focus_is_deferred {
                    context.request_focus_next_frame(id);
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetTitle { id, title } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.title = title;
                } else {
                    with_window_mut(windows, context, id, |window| {
                        window.title = title;
                    });
                }
                feedback_candidates.insert(id);
            }
        }
    }

    let pending_viewport_ids = pending_windows.keys().copied().collect::<HashSet<_>>();
    for (viewport_id, window) in pending_windows {
        if let Some(entity) = context.viewport_window(viewport_id) {
            let previous = context.viewport_feedback(viewport_id);
            context.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, &window, previous, None),
            );
            ecs_commands.entity(entity).insert(window);
        }
    }

    for viewport_id in feedback_candidates {
        if pending_viewport_ids.contains(&viewport_id)
            || context
                .inner
                .state
                .borrow()
                .pending_client_placements
                .contains_key(&viewport_id)
        {
            continue;
        }
        if let Some(entity) = context.viewport_window(viewport_id)
            && let Ok(window) = windows.get(entity)
        {
            let previous = context.viewport_feedback(viewport_id);
            context.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, window, previous, None),
            );
        }
    }

    apply_pending_viewport_focus_requests(windows, context);

    #[cfg(feature = "render")]
    for (window_entity, marker, owner) in viewport_windows.iter() {
        let Some((context_id, viewport_id)) = owner.window_identity() else {
            continue;
        };
        if context_id != context.context_id
            || context.viewport_window(viewport_id) != Some(window_entity)
        {
            continue;
        }
        if marker.is_none_or(|marker| !owner.matches_window(marker)) {
            ecs_commands
                .entity(window_entity)
                .insert(ImguiViewportWindow::new(context_id, viewport_id));
        }
        let flags = context
            .inner
            .state
            .borrow()
            .viewport_flags
            .get(&viewport_id)
            .copied()
            .unwrap_or_else(imgui::ViewportFlags::empty);
        ensure_viewport_camera(
            ecs_commands,
            context,
            viewport_id,
            window_entity,
            viewport_window_config.transparent,
            flags,
            ViewportCameraReconciliation {
                live: &live_cameras,
                recoverable: &recoverable_cameras,
                pending: &mut pending_cameras,
            },
        );
    }

    #[cfg(feature = "render")]
    cleanup_orphaned_viewport_cameras(
        ecs_commands,
        context,
        owned_cameras.into_iter(),
        &scheduled_camera_despawns,
    );
    #[cfg(not(feature = "render"))]
    let _ = viewport_cameras;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn acknowledge_viewport_ecs_despawns_system(
    bridge: NonSend<ImguiViewportBridge>,
    entities: Query<Entity>,
) {
    for context in bridge.contexts() {
        context.acknowledge_ecs_despawns(|entity| entities.get(entity).is_ok());
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_pending_viewport_focus_requests(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
) {
    let ready = std::mem::take(&mut bridge.inner.state.borrow_mut().focus_ready);
    for viewport_id in ready {
        if let Some(entity) = bridge.viewport_window(viewport_id)
            && let Ok(mut window) = windows.get_mut(entity)
        {
            window.focused = true;
        }
    }
    let mut state = bridge.inner.state.borrow_mut();
    let next_frame = std::mem::take(&mut state.focus_next_frame);
    state.focus_ready.extend(next_frame);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn settle_pending_client_placements(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
    mut decoration_offset: impl FnMut(Entity) -> Option<[f32; 2]>,
) {
    let pending = bridge
        .inner
        .state
        .borrow()
        .pending_client_placements
        .iter()
        .map(|(&viewport_id, &placement)| (viewport_id, placement))
        .collect::<Vec<_>>();
    let mut settled = Vec::new();

    for (viewport_id, placement) in pending {
        let Some(entity) = bridge.viewport_window(viewport_id) else {
            settled.push(viewport_id);
            continue;
        };
        let Some(offset) = decoration_offset(entity) else {
            continue;
        };
        let Ok(mut window) = windows.get_mut(entity) else {
            continue;
        };
        window.position = WindowPosition::At(physical_pos_from_desktop(
            [placement.pos[0] - offset[0], placement.pos[1] - offset[1]],
            placement.dpi_scale,
        ));
        if placement.show_requested {
            window.visible = true;
        }
        if placement.focus_requested {
            window.focused = false;
            bridge.request_focus_next_frame(viewport_id);
        }
        if let Some(mut feedback) = bridge.viewport_feedback(viewport_id) {
            feedback.pos = placement.pos;
            bridge.set_viewport_feedback(viewport_id, feedback);
        }
        settled.push(viewport_id);
    }

    if !settled.is_empty() {
        let mut state = bridge.inner.state.borrow_mut();
        for viewport_id in settled {
            state.pending_client_placements.remove(&viewport_id);
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(SystemParam)]
struct SecondaryViewportHostQueries<'w, 's> {
    primary: Query<'w, 's, Entity, With<PrimaryWindow>>,
    windows: Query<'w, 's, Entity, With<Window>>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn cleanup_secondary_viewports_when_host_is_unavailable(
    mut ecs_commands: Commands,
    mut close_requests: MessageReader<WindowCloseRequested>,
    host_queries: SecondaryViewportHostQueries,
    contexts: Option<NonSendMut<crate::ImguiContexts>>,
    bridge: NonSend<ImguiViewportBridge>,
    #[cfg(feature = "render")] input_metrics: Res<crate::input::ImguiContextInputMetrics>,
    #[cfg(feature = "render")] resolved_routes: Res<crate::route::ImguiResolvedRoutes>,
) {
    let Some(mut contexts) = contexts else {
        close_requests.read().for_each(drop);
        return;
    };
    let primary_window = host_queries.primary.single().ok();
    let close_requested = close_requests
        .read()
        .map(|event| event.window)
        .collect::<HashSet<_>>();
    #[cfg(feature = "render")]
    let primary_context = contexts.primary_id();

    for context_bridge in bridge.contexts() {
        let context_id = context_bridge.context_id;
        #[cfg(feature = "render")]
        let host_window = resolved_routes
            .render_route(context_id)
            .and_then(crate::route::ImguiResolvedRenderRoute::host_window)
            .or_else(|| {
                resolved_routes
                    .input_route(context_id)
                    .map(crate::route::ImguiResolvedInputRoute::host_window)
            })
            .or_else(|| {
                input_metrics
                    .get(context_id)
                    .map(|metrics| metrics.host_window)
            })
            .or_else(|| {
                (Some(context_id) == primary_context)
                    .then_some(primary_window)
                    .flatten()
            });
        #[cfg(not(feature = "render"))]
        let host_window = primary_window;
        let host_is_unavailable = host_window.is_none_or(|host_window| {
            host_queries.windows.get(host_window).is_err() || close_requested.contains(&host_window)
        });
        if !host_is_unavailable {
            continue;
        }

        let entities = context_bridge.mapped_ecs_entities();
        context_bridge.track_ecs_despawns(entities.iter().copied());
        for entity in entities {
            native_window::release_pointer_capture_for(entity);
            ecs_commands.entity(entity).try_despawn();
        }

        let result = contexts.configure(context_id, |context| {
            clear_imgui_viewport_platform_handles(context, &context_bridge);
        });
        let native_handles_cleared = match result {
            Ok(()) => true,
            Err(
                crate::ImguiContextError::TeardownInProgress { .. }
                | crate::ImguiContextError::UnknownContext { .. },
            ) => false,
            Err(error) => {
                panic!("cannot clear Dear ImGui viewport handles for {context_id:?}: {error}")
            }
        };
        if native_handles_cleared {
            context_bridge
                .inner
                .clear_viewport_state_preserving_pending_despawns();
        } else {
            // Teardown still owns the Context and must clear its raw fields before these boxes drop.
            context_bridge
                .inner
                .clear_viewport_state_preserving_native_handles();
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
) {
    clear_imgui_viewport_platform_handles_for_keepalive(context, &bridge.inner, true);
    // Host loss deliberately clears bridge-owned viewport fields. Publish that transition before
    // the next frame validates ownership, so recovery cannot mistake our cleanup for foreign
    // mutation and revoke the native viewport capabilities.
    bridge.inner.record_runtime_contract(context);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_imgui_viewport_platform_handles_for_keepalive(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
    recreate_platform_windows: bool,
) {
    let state = keepalive.state.borrow();
    let owned_handles = state
        .viewport_handles
        .values()
        .chain(state.retired_viewport_handles.values())
        .map(|handle| ImguiViewportHandleRef {
            identity: handle.identity,
            pointer: (&**handle as *const ImguiViewportPlatformHandle)
                .cast_mut()
                .cast::<c_void>(),
            recreate_platform_window: recreate_platform_windows,
        })
        .collect::<Vec<_>>();
    drop(state);
    clear_imgui_viewport_platform_handles_for_owned_handles(context, &owned_handles);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_stale_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
    live_viewports: &HashSet<ImguiViewportId>,
) {
    let owned_handles = bridge
        .inner
        .state
        .borrow()
        .viewport_handles
        .iter()
        .filter(|(viewport_id, _)| !live_viewports.contains(viewport_id))
        .map(|(_, handle)| ImguiViewportHandleRef {
            identity: handle.identity,
            pointer: (&**handle as *const ImguiViewportPlatformHandle)
                .cast_mut()
                .cast::<c_void>(),
            recreate_platform_window: true,
        })
        .collect::<Vec<_>>();
    clear_imgui_viewport_platform_handles_for_owned_handles(context, &owned_handles);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_imgui_viewport_platform_handles_for_owned_handles(
    context: &mut imgui::Context,
    owned_handles: &[ImguiViewportHandleRef],
) {
    if owned_handles.is_empty() {
        return;
    }

    let binding = context.binding();
    binding.with_bound_context(|| {
        let main_viewport = unsafe { sys::igGetMainViewport() };
        for owned_handle in owned_handles {
            // `PlatformIO.Viewports` intentionally omits hidden, inactive, and zero-sized
            // viewports. Resolve through Dear ImGui's full internal list instead, then require
            // the recorded address to prevent an ID-reused viewport from inheriting old state.
            let Some(viewport) = (unsafe { owned_handle.identity.resolve() }) else {
                continue;
            };
            // SAFETY: the internal lookup returned the exact still-live viewport for the bound
            // Context. Each field is cleared only when it still contains this bridge's handle.
            let viewport = unsafe { imgui::Viewport::from_raw_mut(viewport) };
            let platform_handle_is_owned = viewport.platform_handle() == owned_handle.pointer;
            let platform_user_data_is_owned = viewport.platform_user_data() == owned_handle.pointer;
            let platform_handle_raw_is_owned =
                viewport.platform_handle_raw() == owned_handle.pointer;
            let platform_handle_raw_is_unclaimed = viewport.platform_handle_raw().is_null();
            let can_recreate_platform_window = platform_handle_is_owned
                && platform_user_data_is_owned
                && (platform_handle_raw_is_unclaimed || platform_handle_raw_is_owned);
            unsafe {
                if platform_handle_is_owned {
                    viewport.set_platform_handle(std::ptr::null_mut());
                }
                if platform_user_data_is_owned {
                    viewport.set_platform_user_data(std::ptr::null_mut());
                }
                if platform_handle_raw_is_owned {
                    viewport.set_platform_handle_raw(std::ptr::null_mut());
                }
                if can_recreate_platform_window
                    && owned_handle.recreate_platform_window
                    && !std::ptr::eq(viewport.as_raw(), main_viewport)
                {
                    // The native viewport is still live, but its bridge-owned Bevy window
                    // disappeared outside the callback contract. Make Dear ImGui issue a fresh
                    // Platform_CreateWindow callback instead of retaining a handle-less viewport.
                    viewport.set_platform_window_created(false);
                }
            }
        }
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn prepare_platform_viewports_for_frame(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
    primary_window: Entity,
    window: &Window,
    monitors: &[sys::ImGuiPlatformMonitor],
    viewport_windows: impl Iterator<Item = (Entity, ImguiViewportId, ImguiViewportFeedback)>,
    enable_viewports: bool,
    desktop_position_support: native_window::DesktopPositionSupport,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    platform_callback_ownership(context, &bridge.inner)?;

    let mut live_feedback = HashSet::new();
    let main_viewport_identity = ImguiViewportIdentity::capture(context.main_viewport());
    let main_viewport_id = main_viewport_identity.id;
    bridge.set_viewport_window(main_viewport_id, primary_window);
    bridge.set_viewport_feedback(
        main_viewport_id,
        feedback_from_window_for_entity(
            primary_window,
            window,
            bridge.viewport_feedback(main_viewport_id),
            None,
        ),
    );
    live_feedback.insert(main_viewport_id);

    for (entity, viewport_id, feedback) in viewport_windows {
        bridge.set_viewport_window(viewport_id, entity);
        bridge.set_viewport_feedback(viewport_id, feedback);
        live_feedback.insert(viewport_id);
    }

    clear_stale_imgui_viewport_platform_handles(context, bridge, &live_feedback);

    let main_viewport_handle = {
        let mut state = bridge.inner.state.borrow_mut();
        state
            .viewport_feedback
            .retain(|viewport_id, _| live_feedback.contains(viewport_id));
        state
            .viewport_windows
            .retain(|viewport_id, _| live_feedback.contains(viewport_id));
        state
            .viewport_cameras
            .retain(|viewport_id, _| live_feedback.contains(viewport_id));
        state
            .viewport_flags
            .retain(|viewport_id, _| live_feedback.contains(viewport_id));
        state
            .focus_next_frame
            .retain(|viewport_id| live_feedback.contains(viewport_id));
        state
            .focus_ready
            .retain(|viewport_id| live_feedback.contains(viewport_id));
        state.retire_stale_platform_handles(&live_feedback);
        state.platform_handle(main_viewport_identity)
    };
    let main_viewport = context.main_viewport();
    // SAFETY: the bridge owns this stable handle and retains it for the complete viewport frame.
    unsafe {
        main_viewport.set_platform_handle(main_viewport_handle);
        main_viewport.set_platform_user_data(main_viewport_handle);
    }

    let fallback_monitor = monitor_from_window(window);
    let monitors = if monitors.is_empty() {
        std::slice::from_ref(&fallback_monitor)
    } else {
        monitors
    };
    // SAFETY: Bevy owns any monitor handles and keeps them valid until this list is replaced.
    unsafe { context.platform_io_mut().set_monitors(monitors) };
    bridge.inner.record_monitor_contract(context, monitors);

    let io = context.io_mut();
    let mut backend_flags = io.backend_flags();
    backend_flags.remove(
        imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
    );
    let native_viewports_available =
        enable_viewports && desktop_position_support.allows_native_viewports();
    if native_viewports_available {
        backend_flags |= imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS;
        if desktop_position_support.can_report_hovered_viewport() {
            backend_flags |= imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        }
    }
    io.set_backend_flags(backend_flags);

    let mut config_flags = io.config_flags();
    if native_viewports_available {
        config_flags.insert(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    } else {
        config_flags.remove(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    }
    io.set_config_flags(config_flags);
    bridge.inner.record_runtime_contract(context);
    Ok(())
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn cleanup_orphaned_viewport_cameras(
    ecs_commands: &mut Commands,
    bridge: &ImguiViewportBridgeContext,
    viewport_cameras: impl Iterator<Item = Entity>,
    scheduled_camera_despawns: &HashSet<Entity>,
) {
    let live_cameras = viewport_cameras.collect::<HashSet<_>>();
    let mapped_cameras = bridge
        .inner
        .state
        .borrow()
        .viewport_cameras
        .values()
        .copied()
        .collect::<HashSet<_>>();
    let orphaned_cameras = live_cameras
        .into_iter()
        .filter(|camera| {
            !mapped_cameras.contains(camera) && !scheduled_camera_despawns.contains(camera)
        })
        .collect::<Vec<_>>();
    for camera in orphaned_cameras {
        bridge.track_ecs_despawn(camera);
        ecs_commands.entity(camera).despawn();
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
struct ViewportCameraReconciliation<'a> {
    live: &'a HashSet<ViewportCameraIdentity>,
    recoverable: &'a HashSet<ViewportCameraIdentity>,
    pending: &'a mut HashSet<ImguiViewportId>,
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn ensure_viewport_camera(
    ecs_commands: &mut Commands,
    bridge: &ImguiViewportBridgeContext,
    viewport_id: ImguiViewportId,
    window_entity: Entity,
    transparent: bool,
    flags: imgui::ViewportFlags,
    cameras: ViewportCameraReconciliation<'_>,
) {
    if let Some(camera) = bridge.viewport_camera(viewport_id) {
        let camera_identity = (viewport_id, camera);
        if cameras.live.contains(&camera_identity) || cameras.pending.contains(&viewport_id) {
            return;
        }
        if cameras.recoverable.contains(&camera_identity) {
            cameras.pending.insert(viewport_id);
            ecs_commands.entity(camera).insert((
                Camera2d,
                viewport_camera(transparent, flags),
                RenderTarget::Window(WindowRef::Entity(window_entity)),
                CameraRenderGraph::new(Core2d),
                RenderLayers::none(),
                ImguiViewportCamera::new(bridge.context_id, viewport_id),
            ));
            return;
        }
        bridge.remove_viewport_camera(viewport_id);
    }
    if !cameras.pending.insert(viewport_id) {
        return;
    }

    let camera = ecs_commands
        .spawn((
            Camera2d,
            viewport_camera(transparent, flags),
            RenderTarget::Window(WindowRef::Entity(window_entity)),
            CameraRenderGraph::new(Core2d),
            RenderLayers::none(),
            ImguiViewportCamera::new(bridge.context_id, viewport_id),
            ImguiViewportOwner::camera(bridge.context_id, viewport_id),
        ))
        .id();
    bridge.set_viewport_camera(viewport_id, camera);
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn viewport_camera(transparent: bool, flags: imgui::ViewportFlags) -> Camera {
    let mut camera = Camera::default();
    let clear_color = if flags.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR) {
        ClearColorConfig::None
    } else if transparent {
        ClearColorConfig::Custom(bevy_color::Color::NONE)
    } else {
        ClearColorConfig::Default
    };
    camera.output_mode = CameraOutputMode::Write {
        blend_state: None,
        clear_color,
    };
    camera
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Exactly one variant is constructed on each native target.
enum DesktopCoordinateSpace {
    Physical,
    Logical,
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
const fn native_desktop_coordinate_space() -> DesktopCoordinateSpace {
    #[cfg(target_os = "macos")]
    {
        DesktopCoordinateSpace::Logical
    }
    #[cfg(not(target_os = "macos"))]
    {
        DesktopCoordinateSpace::Physical
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_position_from_physical(position: IVec2, scale_factor: f32) -> [f32; 2] {
    let position = [position.x as f32, position.y as f32];
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => position,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [position[0] / scale_factor, position[1] / scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_size_from_physical(size: [u32; 2], scale_factor: f32) -> [f32; 2] {
    let size = [size[0] as f32, size[1] as f32];
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [size[0] / scale_factor, size[1] / scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_framebuffer_scale(scale_factor: f32) -> [f32; 2] {
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => [1.0, 1.0],
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [scale_factor, scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn desktop_metrics_for_window(window: &Window) -> ([f32; 2], [f32; 2]) {
    let scale_factor = window.scale_factor();
    (
        desktop_size_from_physical(
            [window.physical_width(), window.physical_height()],
            scale_factor,
        ),
        desktop_framebuffer_scale(scale_factor),
    )
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn monitor_from_window(window: &Window) -> sys::ImGuiPlatformMonitor {
    let mut monitor = sys::ImGuiPlatformMonitor::default();
    let pos = match window.position {
        WindowPosition::At(pos) => desktop_position_from_physical(pos, window.scale_factor()),
        WindowPosition::Automatic | WindowPosition::Centered(_) => [0.0, 0.0],
    };
    let size = desktop_size_from_physical(
        [window.physical_width(), window.physical_height()],
        window.scale_factor(),
    );
    monitor.MainPos = sys::ImVec2 {
        x: pos[0],
        y: pos[1],
    };
    monitor.MainSize = sys::ImVec2 {
        x: size[0],
        y: size[1],
    };
    monitor.WorkPos = monitor.MainPos;
    monitor.WorkSize = monitor.MainSize;
    monitor.DpiScale = positive_finite_or(window.scale_factor(), 1.0);
    monitor
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn platform_monitors_from_bevy_monitors(
    monitors: impl IntoIterator<Item = (Monitor, bool)>,
) -> Vec<sys::ImGuiPlatformMonitor> {
    let mut monitors = monitors.into_iter().collect::<Vec<_>>();
    monitors.sort_by_key(|(monitor, is_primary)| {
        (
            !*is_primary,
            monitor.physical_position.x,
            monitor.physical_position.y,
        )
    });
    monitors
        .into_iter()
        .map(|(monitor, _)| platform_monitor_from_bevy_monitor(&monitor))
        .collect()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn platform_monitor_from_bevy_monitor(monitor: &Monitor) -> sys::ImGuiPlatformMonitor {
    let scale = positive_finite_or(monitor.scale_factor as f32, 1.0);
    let pos = desktop_position_from_physical(monitor.physical_position, scale);
    let size = desktop_size_from_physical([monitor.physical_width, monitor.physical_height], scale);
    let mut platform_monitor = sys::ImGuiPlatformMonitor::default();
    platform_monitor.MainPos = sys::ImVec2 {
        x: pos[0],
        y: pos[1],
    };
    platform_monitor.MainSize = sys::ImVec2 {
        x: size[0],
        y: size[1],
    };
    platform_monitor.WorkPos = platform_monitor.MainPos;
    platform_monitor.WorkSize = platform_monitor.MainSize;
    platform_monitor.DpiScale = scale;
    platform_monitor
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_feedback_from_window(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
) -> ImguiViewportFeedback {
    feedback_from_window_for_entity(entity, window, previous, None)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn feedback_from_window_for_entity(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
    minimized: Option<bool>,
) -> ImguiViewportFeedback {
    let pos = winit_window_client_origin_desktop(entity)
        .or_else(|| previous.map(|feedback| feedback.pos))
        .or_else(|| window_position_desktop(&window.position, window.scale_factor()))
        .unwrap_or([0.0, 0.0]);
    let scale_factor = window_client_scale_factor(entity, window);
    let size = winit_window_client_size_desktop(entity).unwrap_or_else(|| {
        desktop_size_from_physical(
            [window.physical_width(), window.physical_height()],
            scale_factor,
        )
    });
    ImguiViewportFeedback {
        pos,
        size,
        framebuffer_scale: desktop_framebuffer_scale(scale_factor),
        dpi_scale: scale_factor,
        focused: window.focused,
        minimized: minimized
            .or_else(|| previous.map(|feedback| feedback.minimized))
            .unwrap_or(false),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_origin_desktop(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
) -> Option<[f32; 2]> {
    if let Some(pos) = winit_window_client_origin_desktop(entity) {
        return Some(pos);
    }
    window_position_desktop(position, scale_factor)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_position_desktop(position: &WindowPosition, scale_factor: f32) -> Option<[f32; 2]> {
    match *position {
        WindowPosition::At(pos) => Some(desktop_position_from_physical(pos, scale_factor)),
        WindowPosition::Automatic | WindowPosition::Centered(_) => None,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn window_client_logical_to_desktop(
    entity: Entity,
    scale_factor: f32,
    cached_client_origin: Option<[f32; 2]>,
    client_position: [f32; 2],
) -> Option<[f32; 2]> {
    if !client_position.into_iter().all(f32::is_finite) {
        return None;
    }
    let origin = winit_window_client_origin_desktop(entity).or(cached_client_origin)?;
    let scale_factor =
        winit_window_scale_factor(entity).unwrap_or_else(|| positive_finite_or(scale_factor, 1.0));
    let client_position = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => [
            client_position[0] * scale_factor,
            client_position[1] * scale_factor,
        ],
        DesktopCoordinateSpace::Logical => client_position,
    };
    let position = [
        origin[0] + client_position[0],
        origin[1] + client_position[1],
    ];
    position.into_iter().all(f32::is_finite).then_some(position)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn desktop_to_window_client_logical(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
    desktop_position: [f32; 2],
) -> Option<[f32; 2]> {
    if !desktop_position.into_iter().all(f32::is_finite) {
        return None;
    }
    let origin = window_client_origin_desktop(entity, position, scale_factor)?;
    let mut client_position = [
        desktop_position[0] - origin[0],
        desktop_position[1] - origin[1],
    ];
    if native_desktop_coordinate_space() == DesktopCoordinateSpace::Physical {
        let scale_factor = winit_window_scale_factor(entity)
            .unwrap_or_else(|| positive_finite_or(scale_factor, 1.0));
        client_position[0] /= scale_factor;
        client_position[1] /= scale_factor;
    }
    client_position
        .into_iter()
        .all(f32::is_finite)
        .then_some(client_position)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_scale_factor(entity: Entity, window: &Window) -> f32 {
    winit_window_scale_factor(entity)
        .unwrap_or_else(|| positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_origin_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = positive_finite_or(window.scale_factor() as f32, 1.0);
        let pos_phys = window.inner_position().ok()?;
        Some(desktop_position_from_physical(
            IVec2::new(pos_phys.x, pos_phys.y),
            scale,
        ))
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_size_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let size = window.inner_size();
        Some(desktop_size_from_physical(
            [size.width, size.height],
            positive_finite_or(window.scale_factor() as f32, 1.0),
        ))
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_decoration_offset_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = positive_finite_or(window.scale_factor() as f32, 1.0);
        let inner = window.inner_position().ok()?;
        let outer = window.outer_position().ok()?;
        let inner = desktop_position_from_physical(IVec2::new(inner.x, inner.y), scale);
        let outer = desktop_position_from_physical(IVec2::new(outer.x, outer.y), scale);
        Some([inner[0] - outer[0], inner[1] - outer[1]])
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_scale_factor(entity: Entity) -> Option<f32> {
    WINIT_WINDOWS.with_borrow(|windows| {
        windows
            .get_window(entity)
            .map(|window| positive_finite_or(window.scale_factor() as f32, 1.0))
    })
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn with_window_mut(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
    id: ImguiViewportId,
    f: impl FnOnce(&mut Window),
) -> Option<()> {
    let entity = bridge.viewport_window(id)?;
    let Ok(mut window) = windows.get_mut(entity) else {
        return None;
    };
    f(&mut window);
    Some(())
}

#[must_use]
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(crate) fn window_from_snapshot(snapshot: &ImguiViewportSnapshot) -> Window {
    window_from_snapshot_with_config(snapshot, ImguiViewportWindowConfig::default())
        .expect("the default viewport window configuration is valid")
}

/// Build a secondary Bevy window after validating its presentation policy.
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(crate) fn window_from_snapshot_with_config(
    snapshot: &ImguiViewportSnapshot,
    config: ImguiViewportWindowConfig,
) -> Result<Window, ImguiViewportWindowConfigError> {
    let config = config.validate()?;
    let scale_factor = positive_finite_or(snapshot.dpi_scale, 1.0);
    let desktop_size = finite_desktop_size(snapshot.size);
    let mut window = Window {
        title: format!("Dear ImGui Viewport {}", snapshot.id.raw()),
        position: WindowPosition::At(physical_pos_from_desktop(snapshot.pos, scale_factor)),
        resolution: WindowResolution::new(1, 1),
        decorations: !snapshot.flags.contains(imgui::ViewportFlags::NO_DECORATION),
        skip_taskbar: snapshot
            .flags
            .contains(imgui::ViewportFlags::NO_TASK_BAR_ICON),
        window_level: if snapshot.flags.contains(imgui::ViewportFlags::TOP_MOST) {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        },
        visible: false,
        focused: false,
        ..Default::default()
    };
    window.resolution.set_scale_factor(scale_factor);
    set_window_desktop_size(&mut window, desktop_size, scale_factor);
    config.apply_to(&mut window);
    Ok(window)
}

#[cfg(test)]
mod window_config_tests {
    use super::*;

    #[test]
    fn secondary_window_inherits_complete_presentation_policy() {
        let snapshot = ImguiViewportSnapshot {
            id: ImguiViewportId::from(7_u32),
            pos: [0.0, 0.0],
            size: [320.0, 240.0],
            dpi_scale: 1.0,
            flags: imgui::ViewportFlags::empty(),
        };
        let config = ImguiViewportWindowConfig {
            present_mode: PresentMode::AutoNoVsync,
            composite_alpha_mode: CompositeAlphaMode::PostMultiplied,
            desired_maximum_frame_latency: std::num::NonZeroU32::new(3),
            window_theme: Some(WindowTheme::Dark),
            transparent: true,
        };

        let window = window_from_snapshot_with_config(&snapshot, config).unwrap();
        assert_eq!(window.present_mode, config.present_mode);
        assert_eq!(window.composite_alpha_mode, config.composite_alpha_mode);
        assert_eq!(
            window.desired_maximum_frame_latency,
            config.desired_maximum_frame_latency
        );
        assert_eq!(window.window_theme, config.window_theme);
        assert!(window.transparent);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_snapshot_to_window(snapshot: &ImguiViewportSnapshot, entity: Entity, window: &mut Window) {
    let next = window_from_snapshot(snapshot);
    window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
        entity,
        snapshot.pos,
        snapshot.dpi_scale,
    ));
    window.resolution = next.resolution;
    apply_viewport_flags_to_window(snapshot.flags, window);
    window.focused = false;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_viewport_flags_to_window(flags: imgui::ViewportFlags, window: &mut Window) {
    window.decorations = !flags.contains(imgui::ViewportFlags::NO_DECORATION);
    window.skip_taskbar = flags.contains(imgui::ViewportFlags::NO_TASK_BAR_ICON);
    window.window_level = if flags.contains(imgui::ViewportFlags::TOP_MOST) {
        WindowLevel::AlwaysOnTop
    } else {
        WindowLevel::Normal
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_viewport_flags_to_cursor_options(
    flags: imgui::ViewportFlags,
    cursor_options: &mut CursorOptions,
) {
    if native_window::supports_pointer_passthrough() {
        cursor_options.hit_test = !flags.contains(imgui::ViewportFlags::NO_INPUTS);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn feedback_from_snapshot(snapshot: &ImguiViewportSnapshot) -> ImguiViewportFeedback {
    let dpi_scale = positive_finite_or(snapshot.dpi_scale, 1.0);
    ImguiViewportFeedback {
        pos: finite_desktop_pos(snapshot.pos),
        size: finite_desktop_size(snapshot.size),
        framebuffer_scale: desktop_framebuffer_scale(dpi_scale),
        dpi_scale,
        focused: false,
        minimized: false,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn physical_outer_pos_for_client_pos(entity: Entity, pos: [f32; 2], dpi_scale: f32) -> IVec2 {
    let pos = if let Some(offset) = winit_window_decoration_offset_desktop(entity) {
        [pos[0] - offset[0], pos[1] - offset[1]]
    } else {
        pos
    };
    physical_pos_from_desktop(pos, dpi_scale)
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn physical_pos_from_desktop(pos: [f32; 2], scale_factor: f32) -> IVec2 {
    let pos = finite_desktop_pos(pos);
    let pos = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => pos,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [pos[0] * scale_factor, pos[1] * scale_factor]
        }
    };
    IVec2::new(pos[0].round() as i32, pos[1].round() as i32)
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn physical_extent(value: f32) -> u32 {
    value.round().max(1.0) as u32
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn finite_desktop_pos(pos: [f32; 2]) -> [f32; 2] {
    [finite_or(pos[0], 0.0), finite_or(pos[1], 0.0)]
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn finite_desktop_size(size: [f32; 2]) -> [f32; 2] {
    [
        positive_finite_or(size[0], 1.0),
        positive_finite_or(size[1], 1.0),
    ]
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
fn set_window_desktop_size(window: &mut Window, size: [f32; 2], scale_factor: f32) {
    let size = finite_desktop_size(size);
    let size = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [size[0] * scale_factor, size[1] * scale_factor]
        }
    };
    window
        .resolution
        .set_physical_resolution(physical_extent(size[0]), physical_extent(size[1]));
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[derive(Resource)]
    struct EcsReleaseBeforeDeferredProbe {
        entity: Entity,
        entity_was_live: bool,
        release_was_pending: bool,
    }

    fn observe_ecs_release_before_deferred(
        bridge: NonSend<ImguiViewportBridge>,
        entities: Query<Entity>,
        mut probe: ResMut<EcsReleaseBeforeDeferredProbe>,
    ) {
        probe.entity_was_live = entities.get(probe.entity).is_ok();
        probe.release_was_pending = bridge.inner.has_tracked_ecs_entities();
    }

    static FOREIGN_DESTROY_CALLS: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0);
    static FOREIGN_RENDERER_DESTROY_CALLS: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0);

    #[test]
    fn mixed_dpi_client_and_desktop_positions_round_trip() {
        let entity = Entity::from_raw_u32(1).expect("test entity index should be valid");
        let window_position = WindowPosition::At(IVec2::new(1920, -200));
        let client_position = [160.25, 48.0];
        let cached_origin = window_position_desktop(&window_position, 2.0);
        let desktop_position =
            window_client_logical_to_desktop(entity, 2.0, cached_origin, client_position)
                .expect("finite client geometry should map into desktop space");

        #[cfg(not(target_os = "macos"))]
        assert_eq!(desktop_position, [2240.5, -104.0]);
        #[cfg(target_os = "macos")]
        assert_eq!(desktop_position, [1120.25, -52.0]);

        assert_eq!(
            desktop_to_window_client_logical(entity, &window_position, 2.0, desktop_position,),
            Some(client_position)
        );
    }

    #[test]
    fn mixed_dpi_window_geometry_round_trips_through_platform_feedback() {
        let entity = Entity::from_raw_u32(1).expect("test entity index should be valid");
        let snapshot = ImguiViewportSnapshot {
            id: imgui::Id::from(0x430),
            pos: [1920.0, -200.0],
            size: [800.0, 600.0],
            dpi_scale: 2.0,
            flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
        };
        let window = window_from_snapshot(&snapshot);
        let feedback = feedback_from_window_for_entity(entity, &window, None, None);

        assert_eq!(feedback.pos, snapshot.pos);
        assert_eq!(feedback.size, snapshot.size);
        assert_eq!(feedback.dpi_scale, snapshot.dpi_scale);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(feedback.framebuffer_scale, [1.0, 1.0]);
        #[cfg(target_os = "macos")]
        assert_eq!(feedback.framebuffer_scale, [2.0, 2.0]);
    }

    unsafe extern "C" fn foreign_platform_destroy_window(_viewport: *mut sys::ImGuiViewport) {
        FOREIGN_DESTROY_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {
        FOREIGN_RENDERER_DESTROY_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    unsafe extern "C" fn foreign_renderer_set_window_size(
        _viewport: *mut sys::ImGuiViewport,
        _size: sys::ImVec2,
    ) {
    }

    unsafe extern "C" fn foreign_platform_set_window_pos(
        _viewport: *mut sys::ImGuiViewport,
        _pos: *const sys::ImVec2,
    ) {
    }

    unsafe extern "C" fn foreign_platform_alpha(_viewport: *mut sys::ImGuiViewport, _alpha: f32) {}

    unsafe extern "C" fn foreign_platform_render(
        _viewport: *mut sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
    }

    unsafe extern "C" fn foreign_platform_work_area(
        _viewport: *mut sys::ImGuiViewport,
    ) -> sys::ImVec4 {
        sys::ImVec4::default()
    }

    unsafe extern "C" fn foreign_platform_vk_surface(
        _viewport: *mut sys::ImGuiViewport,
        _instance: sys::ImU64,
        _allocators: *const c_void,
        _surface: *mut sys::ImU64,
    ) -> i32 {
        0
    }

    fn test_context_id() -> imgui::ContextId {
        imgui::Context::create().id()
    }

    fn assert_despawn_remains_tracked_until_deferred_application(release: bool) {
        let viewport_id = imgui::Id::from(0x7A0);
        let main_viewport_id = imgui::Id::from(0x7A1);
        let context_id = test_context_id();
        let mut world = World::new();
        let entity = world
            .spawn((
                Window::default(),
                ImguiViewportWindow::new(context_id, viewport_id),
                ImguiViewportOwner::window(context_id, viewport_id),
            ))
            .id();
        let mut bridge = ImguiViewportBridge::default();
        bridge.set_viewport_window(viewport_id, entity);
        let keepalive = bridge.keepalive();
        bridge.register_context(context_id, Rc::clone(&keepalive));
        if release {
            keepalive.prepare_ecs_release(main_viewport_id);
        } else {
            bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });
        }
        world.insert_non_send(bridge);
        world.insert_resource(crate::context::ownership::ImguiBackendRuntime::new(
            crate::ImguiPluginConfig::default(),
            true,
        ));
        world.insert_resource(EcsReleaseBeforeDeferredProbe {
            entity,
            entity_was_live: false,
            release_was_pending: false,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                apply_viewport_commands_system,
                observe_ecs_release_before_deferred,
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred(),
        );
        schedule.run(&mut world);

        let probe = world.resource::<EcsReleaseBeforeDeferredProbe>();
        assert!(
            probe.entity_was_live,
            "the probe must run before the deferred despawn is applied"
        );
        assert!(
            probe.release_was_pending,
            "despawn cannot be acknowledged while its ECS entity is still live"
        );
        assert!(world.get_entity(entity).is_err());
        assert!(
            !world
                .get_non_send::<ImguiViewportBridge>()
                .unwrap()
                .inner
                .has_tracked_ecs_entities(),
            "post-deferred acknowledgement must clear only entities proven absent"
        );
        if release {
            assert!(
                world
                    .get_non_send::<ImguiViewportBridge>()
                    .unwrap()
                    .ecs_release_pending(),
                "ECS acknowledgement must leave final release ownership with the Context owner"
            );
            keepalive.finish_ecs_release();
            assert!(
                !world
                    .get_non_send::<ImguiViewportBridge>()
                    .unwrap()
                    .ecs_release_pending(),
                "the Context owner must finish release after observing the drained ECS world"
            );
        }
    }

    #[test]
    fn explicit_release_remains_pending_until_deferred_despawn_is_applied() {
        assert_despawn_remains_tracked_until_deferred_application(true);
    }

    #[test]
    fn ordinary_destroy_remains_tracked_until_deferred_despawn_is_applied() {
        assert_despawn_remains_tracked_until_deferred_application(false);
    }

    fn test_viewport_snapshot(
        id: ImguiViewportId,
        flags: imgui::ViewportFlags,
    ) -> ImguiViewportSnapshot {
        ImguiViewportSnapshot {
            id,
            pos: [32.0, 48.0],
            size: [640.0, 360.0],
            dpi_scale: 1.0,
            flags,
        }
    }

    fn run_viewport_command_schedule(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                apply_viewport_commands_system,
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred(),
        );
        schedule.run(world);
    }

    #[test]
    fn pending_decorated_window_is_positioned_by_client_origin_before_show() {
        fn settle_with_test_decoration(
            mut windows: Query<&mut Window>,
            bridge: NonSend<ImguiViewportBridge>,
        ) {
            for context in bridge.contexts() {
                settle_pending_client_placements(&mut windows, &context, |_| Some([4.0, 15.0]));
            }
        }

        let context_id = test_context_id();
        let viewport_id = imgui::Id::from(0x7AF);
        let mut bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        bridge.register_context(context_id, Rc::clone(&keepalive));

        let mut world = World::new();
        let entity = world
            .spawn(Window {
                visible: false,
                ..Default::default()
            })
            .id();
        {
            let mut state = keepalive.state.borrow_mut();
            state.viewport_windows.insert(viewport_id, entity);
            state.viewport_feedback.insert(
                viewport_id,
                ImguiViewportFeedback {
                    pos: [100.0, 200.0],
                    size: [320.0, 180.0],
                    framebuffer_scale: [1.0, 1.0],
                    dpi_scale: 1.0,
                    focused: false,
                    minimized: false,
                },
            );
            state.pending_client_placements.insert(
                viewport_id,
                PendingClientPlacement {
                    pos: [100.0, 200.0],
                    dpi_scale: 1.0,
                    show_requested: true,
                    focus_requested: true,
                },
            );
        }
        world.insert_non_send(bridge);

        let mut schedule = Schedule::default();
        schedule.add_systems(settle_with_test_decoration);
        schedule.run(&mut world);

        let window = world
            .get::<Window>(entity)
            .expect("the pending viewport Window should remain live");
        assert_eq!(window.position, WindowPosition::At(IVec2::new(96, 185)));
        assert!(window.visible);
        let state = keepalive.state.borrow();
        assert!(!state.pending_client_placements.contains_key(&viewport_id));
        assert!(state.focus_next_frame.contains(&viewport_id));
        assert_eq!(
            state
                .viewport_feedback
                .get(&viewport_id)
                .expect("settlement should preserve platform feedback")
                .pos,
            [100.0, 200.0]
        );
    }

    #[test]
    fn command_application_scopes_equal_viewport_ids_to_their_contexts() {
        let context_a_id = test_context_id();
        let context_b_id = test_context_id();
        assert_ne!(context_a_id, context_b_id);

        let viewport_id = imgui::Id::from(0x7B0);
        let mut bridge = ImguiViewportBridge::default();
        let keepalive_a = bridge.keepalive();
        bridge.register_context(context_a_id, Rc::clone(&keepalive_a));
        let keepalive_b = Rc::new(ImguiViewportBridgeShared::default());
        bridge.register_context(context_b_id, Rc::clone(&keepalive_b));
        keepalive_a
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::Create(test_viewport_snapshot(
                viewport_id,
                imgui::ViewportFlags::IS_PLATFORM_WINDOW | imgui::ViewportFlags::TOP_MOST,
            )));
        keepalive_b
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::Create(test_viewport_snapshot(
                viewport_id,
                imgui::ViewportFlags::IS_PLATFORM_WINDOW
                    | imgui::ViewportFlags::NO_FOCUS_ON_APPEARING,
            )));

        let mut world = World::new();
        world.insert_resource(crate::context::ownership::ImguiBackendRuntime::new(
            crate::ImguiPluginConfig::default(),
            true,
        ));
        world.insert_non_send(bridge);
        run_viewport_command_schedule(&mut world);

        let (window_a, window_b) = {
            let bridge = world.non_send::<ImguiViewportBridge>();
            (
                bridge
                    .viewport_window(context_a_id, viewport_id)
                    .expect("Context A should own its viewport window"),
                bridge
                    .viewport_window(context_b_id, viewport_id)
                    .expect("Context B should own its viewport window"),
            )
        };
        assert_ne!(window_a, window_b);
        assert_eq!(
            world
                .get::<ImguiViewportWindow>(window_a)
                .expect("Context A window should carry a viewport marker")
                .context_id,
            context_a_id
        );
        assert_eq!(
            world
                .get::<ImguiViewportWindow>(window_b)
                .expect("Context B window should carry a viewport marker")
                .context_id,
            context_b_id
        );
        assert_eq!(
            world
                .get::<Window>(window_a)
                .expect("Context A window should exist")
                .window_level,
            WindowLevel::AlwaysOnTop
        );

        keepalive_a
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::SetPos {
                id: viewport_id,
                pos: [80.0, 96.0],
                dpi_scale: 1.0,
            });
        keepalive_a
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::SetSize {
                id: viewport_id,
                size: [320.0, 200.0],
                dpi_scale: 1.0,
            });
        keepalive_a
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::SetTitle {
                id: viewport_id,
                title: "Context A".to_owned(),
            });
        keepalive_a
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::Show { id: viewport_id });
        keepalive_b
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::Show { id: viewport_id });
        run_viewport_command_schedule(&mut world);

        let window_a_state = world
            .get::<Window>(window_a)
            .expect("Context A window should remain live");
        assert_eq!(
            window_a_state.position,
            WindowPosition::At(IVec2::new(80, 96))
        );
        assert_eq!(window_a_state.resolution.width(), 320.0);
        assert_eq!(window_a_state.resolution.height(), 200.0);
        assert_eq!(window_a_state.title, "Context A");
        assert!(window_a_state.visible);
        assert!(!window_a_state.focused);
        assert!(
            !world
                .get::<Window>(window_b)
                .expect("Context B window should remain live")
                .focused,
            "NoFocusOnAppearing must remain local to Context B"
        );

        run_viewport_command_schedule(&mut world);
        assert!(
            world
                .get::<Window>(window_a)
                .expect("Context A window should remain live")
                .focused,
            "Context A show must request focus on the following ECS pass"
        );
        assert!(
            !world
                .get::<Window>(window_b)
                .expect("Context B window should remain live")
                .focused,
            "Context B must honor NoFocusOnAppearing"
        );

        keepalive_b
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::SetFocus { id: viewport_id });
        run_viewport_command_schedule(&mut world);
        run_viewport_command_schedule(&mut world);
        assert!(
            world
                .get::<Window>(window_b)
                .expect("Context B window should remain live")
                .focused,
            "an explicit Context B focus request must not be blocked by its show policy"
        );

        keepalive_b
            .state
            .borrow_mut()
            .queue(ImguiViewportCommand::Destroy { id: viewport_id });
        run_viewport_command_schedule(&mut world);
        let bridge = world.non_send::<ImguiViewportBridge>();
        assert!(
            bridge.viewport_window(context_b_id, viewport_id).is_none(),
            "destroying Context B must remove only Context B's mapping"
        );
        assert_eq!(
            bridge.viewport_window(context_a_id, viewport_id),
            Some(window_a),
            "Context A's equal numeric viewport id must remain live"
        );
        keepalive_b.record_callback_fault(ImguiViewportRuntimeError::CallbackReentered);
        assert_eq!(
            bridge.callback_error_for(context_b_id),
            Some(ImguiViewportRuntimeError::CallbackReentered)
        );
        assert_eq!(
            bridge.callback_error_for(context_a_id),
            None,
            "a deferred callback failure must remain local to its owning Context"
        );
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    #[test]
    fn transparent_viewport_camera_clears_to_transparent() {
        let camera = viewport_camera(true, imgui::ViewportFlags::empty());

        assert!(matches!(
            camera.output_mode,
            CameraOutputMode::Write {
                clear_color: ClearColorConfig::Custom(color),
                ..
            } if color == bevy_color::Color::NONE
        ));
        assert!(matches!(
            viewport_camera(false, imgui::ViewportFlags::empty()).output_mode,
            CameraOutputMode::Write {
                clear_color: ClearColorConfig::Default,
                ..
            }
        ));
    }

    struct PlatformViewportsGuard {
        platform_io: *mut sys::ImGuiPlatformIO,
        original_viewports: sys::ImVector_ImGuiViewportPtr,
        owned_viewport: *mut sys::ImGuiViewport,
    }

    impl PlatformViewportsGuard {
        unsafe fn replace(
            context: &mut imgui::Context,
            viewports: &mut [*mut sys::ImGuiViewport],
            owned_viewport: *mut sys::ImGuiViewport,
        ) -> Self {
            let platform_io = context.platform_io_mut().as_raw_mut();
            let original_viewports = unsafe { (*platform_io).Viewports };
            unsafe {
                (*platform_io).Viewports = sys::ImVector_ImGuiViewportPtr {
                    Size: viewports
                        .len()
                        .try_into()
                        .expect("test viewport count should fit i32"),
                    Capacity: viewports
                        .len()
                        .try_into()
                        .expect("test viewport count should fit i32"),
                    Data: viewports.as_mut_ptr(),
                };
            }
            Self {
                platform_io,
                original_viewports,
                owned_viewport,
            }
        }
    }

    impl Drop for PlatformViewportsGuard {
        fn drop(&mut self) {
            unsafe {
                (*self.platform_io).Viewports = self.original_viewports;
                if !self.owned_viewport.is_null() {
                    sys::ImGuiViewport_destroy(self.owned_viewport);
                }
            }
        }
    }

    fn feedback() -> ImguiViewportFeedback {
        ImguiViewportFeedback {
            pos: [0.0, 0.0],
            size: [64.0, 64.0],
            framebuffer_scale: [1.0, 1.0],
            dpi_scale: 1.0,
            focused: false,
            minimized: false,
        }
    }

    #[test]
    fn platform_capabilities_follow_native_desktop_position_support() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let context_bridge = ImguiViewportBridgeContext {
            context_id: context.id(),
            inner: Rc::clone(&keepalive),
        };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");

        for support in [
            native_window::DesktopPositionSupport::PendingWindow,
            native_window::DesktopPositionSupport::Unavailable,
        ] {
            prepare_platform_viewports_for_frame(
                &mut context,
                &context_bridge,
                primary_window,
                &Window::default(),
                &[],
                std::iter::empty(),
                true,
                support,
            )
            .unwrap();
            assert!(
                !context
                    .io()
                    .config_flags()
                    .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
            );
            assert!(!context.io().backend_flags().intersects(
                imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                    | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
                    | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT
            ));
        }

        prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            true,
            native_window::DesktopPositionSupport::Available,
        )
        .unwrap();
        assert!(
            context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
        );
        assert!(context.io().backend_flags().contains(
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
        ));
        assert_eq!(
            context
                .io()
                .backend_flags()
                .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT),
            cfg!(target_os = "windows")
        );

        detach_owned_bridge(&mut context, &keepalive).unwrap();
    }

    #[test]
    fn owned_command_snapshot_is_isolated_from_later_queue_changes() {
        let mut bridge = ImguiViewportBridge::default();
        let viewport_id = imgui::Id::from(0x440);
        bridge.queue(ImguiViewportCommand::Show { id: viewport_id });

        let observed = bridge.commands();
        assert_eq!(
            bridge.drain_commands().unwrap(),
            [ImguiViewportCommand::Show { id: viewport_id }]
        );
        bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });

        assert_eq!(
            observed,
            [ImguiViewportCommand::Show { id: viewport_id }],
            "an observer must own an immutable snapshot of the queue"
        );
        assert_eq!(
            bridge.drain_commands().unwrap(),
            [ImguiViewportCommand::Destroy { id: viewport_id }]
        );
    }

    #[test]
    fn callback_contention_latches_without_unwinding_through_c() {
        let mut context = imgui::Context::create();
        let mut bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());
        unsafe {
            (*viewport).ID = 0x441;
        }

        let state_borrow = bridge.inner.state.borrow_mut();
        let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            platform_show_window(viewport.cast::<imgui::Viewport>());
        }));
        assert!(callback.is_ok(), "the C callback boundary must not unwind");
        drop(state_borrow);

        assert_eq!(
            bridge.callback_error(),
            Some(ImguiViewportRuntimeError::CallbackReentered)
        );
        assert_eq!(
            bridge.drain_commands(),
            Err(ImguiViewportRuntimeError::CallbackReentered)
        );
        assert_eq!(
            bridge.drain_commands(),
            Err(ImguiViewportRuntimeError::CallbackReentered),
            "the deferred callback fault must remain sticky"
        );

        bridge.clear_viewport_state();
        assert_eq!(bridge.callback_error(), None);
        assert!(bridge.drain_commands().unwrap().is_empty());

        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
            sys::ImGuiViewport_destroy(viewport);
        }
    }

    #[test]
    fn direct_callback_skips_foreign_userdata_without_dereferencing_it() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());

        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
            platform_show_window(viewport.cast::<imgui::Viewport>());
        }
        assert_eq!(
            bridge.callback_error(),
            Some(ImguiViewportRuntimeError::CallbackOwnership(
                ImguiViewportCallbackOwnershipError::BackendPlatformUserDataReplaced,
            )),
            "a callback must latch drift before casting foreign userdata"
        );
        assert!(bridge.commands().is_empty());

        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
            sys::ImGuiViewport_destroy(viewport);
        }
    }

    #[test]
    fn destroy_callback_never_clears_handles_when_dispatch_is_rejected() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());
        unsafe {
            (*viewport).ID = 0x445;
            platform_create_window_raw_callback(viewport);
        }
        let owned_handle = unsafe { (*viewport).PlatformHandle };
        assert!(!owned_handle.is_null());

        let state_borrow = bridge.inner.state.borrow_mut();
        unsafe { platform_destroy_window_raw_callback(viewport) };
        assert_eq!(unsafe { (*viewport).PlatformUserData }, owned_handle);
        assert_eq!(unsafe { (*viewport).PlatformHandle }, owned_handle);
        assert_eq!(
            bridge.callback_error(),
            Some(ImguiViewportRuntimeError::CallbackReentered)
        );
        drop(state_borrow);

        bridge.inner.callback_fault.set(None);
        unsafe { platform_destroy_window_raw_callback(viewport) };
        assert!(unsafe { (*viewport).PlatformUserData.is_null() });
        assert!(unsafe { (*viewport).PlatformHandle.is_null() });
        detach_owned_bridge(&mut context, &keepalive).unwrap();
        unsafe { sys::ImGuiViewport_destroy(viewport) };
    }

    #[test]
    fn destroy_callback_does_not_touch_viewport_for_the_wrong_current_context() {
        let mut context = imgui::Context::create();
        let raw_context = context.as_raw();
        let io = unsafe { sys::igGetIO_ContextPtr(raw_context) };
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());
        unsafe {
            (*viewport).ID = 0x446;
            platform_create_window_raw_callback(viewport);
        }
        let owned_handle = unsafe { (*viewport).PlatformHandle };
        let suspended = context.suspend();
        let other_context = imgui::Context::create();

        unsafe { platform_destroy_window_raw_callback(viewport) };
        assert_eq!(unsafe { (*viewport).PlatformUserData }, owned_handle);
        assert_eq!(unsafe { (*viewport).PlatformHandle }, owned_handle);
        assert_eq!(bridge.callback_error(), None);

        unsafe {
            (*viewport).PlatformUserData = std::ptr::null_mut();
            (*viewport).PlatformHandle = std::ptr::null_mut();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            sys::ImGuiViewport_destroy(viewport);
        }
        VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
            registry.borrow_mut().remove(&(raw_context as usize));
        });
        drop(other_context);
        drop(suspended);
    }

    #[test]
    fn callback_install_rejects_foreign_ownership_without_partial_mutation() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        unsafe {
            context
                .platform_io_mut()
                .set_platform_destroy_window_raw(Some(foreign_platform_destroy_window));
        }

        assert_eq!(
            unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
            Err(ImguiViewportCallbackInstallError::CallbackSlot {
                slot: "Platform_DestroyWindow",
            })
        );
        assert!(context.io().backend_platform_user_data().is_null());
        let raw = unsafe { &*context.platform_io().as_raw() };
        assert!(raw.Platform_CreateWindow.is_none());
        assert!(raw.Platform_DestroyWindow.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            )
        }));

        unsafe {
            context
                .platform_io_mut()
                .set_platform_destroy_window_raw(None);
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
        }
        assert_eq!(
            unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
            Err(ImguiViewportCallbackInstallError::BackendPlatformUserData)
        );
        assert!(
            unsafe { &*context.platform_io().as_raw() }
                .Platform_CreateWindow
                .is_none()
        );
        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
        }
    }

    #[test]
    fn callback_install_rejects_existing_platform_monitors_without_mutating_them() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let monitor = monitor_from_window(&Window::default());
        unsafe { context.platform_io_mut().set_monitors(&[monitor]) };
        let original = unsafe { (*context.platform_io().as_raw()).Monitors };

        assert_eq!(
            unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
            Err(ImguiViewportCallbackInstallError::PlatformMonitors)
        );
        let actual = unsafe { (*context.platform_io().as_raw()).Monitors };
        assert_eq!(actual, original);
        assert_eq!(unsafe { *actual.Data }, monitor);
        assert!(context.io().backend_platform_user_data().is_null());
        assert!(
            unsafe { &*context.platform_io().as_raw() }
                .Platform_CreateWindow
                .is_none()
        );

        unsafe { context.platform_io_mut().set_monitors(&[]) };
    }

    #[test]
    fn callback_install_rejects_foreign_name_and_main_viewport_handles_without_mutation() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        context.set_platform_name(Some("foreign-platform")).unwrap();
        let foreign_name = context.io().backend_platform_name().unwrap().as_ptr();
        assert_eq!(
            unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
            Err(ImguiViewportCallbackInstallError::BackendPlatformName)
        );
        assert_eq!(
            context.io().backend_platform_name().unwrap().as_ptr(),
            foreign_name
        );
        assert!(context.io().backend_platform_user_data().is_null());
        assert!(
            unsafe { &*context.platform_io().as_raw() }
                .Platform_CreateWindow
                .is_none()
        );
        context.set_platform_name::<String>(None).unwrap();
        drop(context);

        macro_rules! assert_main_field_rejected {
            ($field:ident) => {{
                let mut context = imgui::Context::create();
                let bridge = ImguiViewportBridge::default();
                let marker = std::ptr::dangling_mut::<u16>().cast::<c_void>();
                unsafe { (*context.main_viewport().as_raw_mut()).$field = marker };
                assert_eq!(
                    unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
                    Err(ImguiViewportCallbackInstallError::MainViewportField {
                        field: stringify!($field),
                    })
                );
                assert_eq!(
                    unsafe { (*context.main_viewport().as_raw()).$field },
                    marker
                );
                assert!(context.io().backend_platform_user_data().is_null());
                assert!(
                    unsafe { &*context.platform_io().as_raw() }
                        .Platform_CreateWindow
                        .is_none()
                );
                unsafe { (*context.main_viewport().as_raw_mut()).$field = std::ptr::null_mut() };
            }};
        }

        assert_main_field_rejected!(PlatformUserData);
        assert_main_field_rejected!(PlatformHandle);
        assert_main_field_rejected!(PlatformHandleRaw);
    }

    #[test]
    fn direct_callback_validates_every_platform_callback_slot_before_dispatch() {
        macro_rules! assert_removed_slot_drift {
            ($slot:ident) => {{
                let mut context = imgui::Context::create();
                let bridge = ImguiViewportBridge::default();
                let keepalive = bridge.keepalive();
                unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
                let flags =
                    context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
                context.io_mut().set_backend_flags(flags);
                keepalive.record_runtime_contract(&mut context);
                let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
                assert!(!viewport.is_null());
                unsafe {
                    (*context.platform_io_mut().as_raw_mut()).$slot = None;
                    platform_show_window_raw_callback(viewport);
                }
                assert_eq!(
                    bridge.callback_error(),
                    Some(ImguiViewportRuntimeError::CallbackOwnership(
                        ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                            slot: stringify!($slot),
                        },
                    ))
                );
                assert!(
                    !context
                        .io()
                        .backend_flags()
                        .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
                );
                let _ = detach_owned_bridge(&mut context, &keepalive);
                unsafe { sys::ImGuiViewport_destroy(viewport) };
            }};
        }

        assert_removed_slot_drift!(Platform_CreateWindow);
        assert_removed_slot_drift!(Platform_DestroyWindow);
        assert_removed_slot_drift!(Platform_ShowWindow);
        assert_removed_slot_drift!(Platform_SetWindowPos);
        assert_removed_slot_drift!(Platform_GetWindowPos);
        assert_removed_slot_drift!(Platform_SetWindowSize);
        assert_removed_slot_drift!(Platform_GetWindowSize);
        assert_removed_slot_drift!(Platform_GetWindowFramebufferScale);
        assert_removed_slot_drift!(Platform_SetWindowFocus);
        assert_removed_slot_drift!(Platform_GetWindowFocus);
        assert_removed_slot_drift!(Platform_GetWindowMinimized);
        assert_removed_slot_drift!(Platform_SetWindowTitle);
        assert_removed_slot_drift!(Platform_UpdateWindow);
        assert_removed_slot_drift!(Platform_GetWindowDpiScale);

        macro_rules! assert_installed_slot_drift {
            ($slot:ident, $callback:path) => {{
                let mut context = imgui::Context::create();
                let bridge = ImguiViewportBridge::default();
                let keepalive = bridge.keepalive();
                unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
                let flags =
                    context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
                context.io_mut().set_backend_flags(flags);
                keepalive.record_runtime_contract(&mut context);
                let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
                assert!(!viewport.is_null());
                unsafe {
                    (*context.platform_io_mut().as_raw_mut()).$slot = Some($callback);
                    platform_show_window_raw_callback(viewport);
                }
                assert_eq!(
                    bridge.callback_error(),
                    Some(ImguiViewportRuntimeError::CallbackOwnership(
                        ImguiViewportCallbackOwnershipError::PlatformCallbackInstalled {
                            slot: stringify!($slot),
                        },
                    ))
                );
                assert!(
                    !context
                        .io()
                        .backend_flags()
                        .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
                );
                let _ = detach_owned_bridge(&mut context, &keepalive);
                unsafe {
                    (*context.platform_io_mut().as_raw_mut()).$slot = None;
                    sys::ImGuiViewport_destroy(viewport);
                }
            }};
        }

        assert_installed_slot_drift!(Platform_SetWindowAlpha, foreign_platform_alpha);
        assert_installed_slot_drift!(Platform_RenderWindow, foreign_platform_render);
        assert_installed_slot_drift!(Platform_SwapBuffers, foreign_platform_render);
        assert_installed_slot_drift!(Platform_OnChangedViewport, foreign_platform_destroy_window);
        assert_installed_slot_drift!(Platform_GetWindowWorkAreaInsets, foreign_platform_work_area);
        assert_installed_slot_drift!(Platform_CreateVkSurface, foreign_platform_vk_surface);

        macro_rules! assert_renderer_slot_drift {
            ($slot:ident, $callback:path) => {{
                let mut context = imgui::Context::create();
                let bridge = ImguiViewportBridge::default();
                let keepalive = bridge.keepalive();
                unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
                let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
                assert!(!viewport.is_null());
                unsafe {
                    (*context.platform_io_mut().as_raw_mut()).$slot = Some($callback);
                    platform_show_window_raw_callback(viewport);
                }
                assert_eq!(
                    bridge.callback_error(),
                    Some(ImguiViewportRuntimeError::CallbackOwnership(
                        ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                            slot: stringify!($slot),
                        },
                    ))
                );
                let _ = detach_owned_bridge(&mut context, &keepalive);
                unsafe {
                    (*context.platform_io_mut().as_raw_mut()).$slot = None;
                    sys::ImGuiViewport_destroy(viewport);
                }
            }};
        }

        assert_renderer_slot_drift!(Renderer_CreateWindow, foreign_renderer_destroy_window);
        assert_renderer_slot_drift!(Renderer_DestroyWindow, foreign_renderer_destroy_window);
        assert_renderer_slot_drift!(Renderer_SetWindowSize, foreign_renderer_set_window_size);
        assert_renderer_slot_drift!(Renderer_RenderWindow, foreign_platform_render);
        assert_renderer_slot_drift!(Renderer_SwapBuffers, foreign_platform_render);
    }

    #[test]
    fn direct_callback_validates_platform_name_flags_and_monitor_storage() {
        fn invoke_and_assert(
            context: &mut imgui::Context,
            bridge: &ImguiViewportBridge,
            expected: ImguiViewportCallbackOwnershipError,
        ) {
            let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
            assert!(!viewport.is_null());
            unsafe { platform_show_window_raw_callback(viewport) };
            assert_eq!(
                bridge.callback_error(),
                Some(ImguiViewportRuntimeError::CallbackOwnership(expected))
            );
            assert!(bridge.commands().is_empty());
            unsafe { sys::ImGuiViewport_destroy(viewport) };
            let _ = context;
        }

        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        context.set_platform_name(Some("foreign-platform")).unwrap();
        invoke_and_assert(
            &mut context,
            &bridge,
            ImguiViewportCallbackOwnershipError::BackendPlatformNameReplaced,
        );
        let _ = detach_owned_bridge(&mut context, &keepalive);
        context.set_platform_name::<String>(None).unwrap();
        drop(context);

        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let flags = context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
        context.io_mut().set_backend_flags(flags);
        invoke_and_assert(
            &mut context,
            &bridge,
            ImguiViewportCallbackOwnershipError::BackendFlagReplaced {
                flag: "PLATFORM_HAS_VIEWPORTS",
            },
        );
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
        );
        let _ = detach_owned_bridge(&mut context, &keepalive);
        drop(context);

        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let monitor = monitor_from_window(&Window::default());
        unsafe { context.platform_io_mut().set_monitors(&[monitor]) };
        let foreign = unsafe { (*context.platform_io().as_raw()).Monitors };
        invoke_and_assert(
            &mut context,
            &bridge,
            ImguiViewportCallbackOwnershipError::PlatformMonitorsReplaced,
        );
        let actual = unsafe { (*context.platform_io().as_raw()).Monitors };
        assert_eq!(actual, foreign);
        assert_eq!(unsafe { *actual.Data }, monitor);
        let _ = detach_owned_bridge(&mut context, &keepalive);
        unsafe { context.platform_io_mut().set_monitors(&[]) };
    }

    #[test]
    fn owned_platform_name_rebase_preserves_every_other_runtime_contract_field() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };

        let before = keepalive
            .runtime_contract
            .get()
            .expect("callback installation records a runtime contract");
        context.set_platform_name(Some("dear-imgui-bevy")).unwrap();
        keepalive.record_owned_platform_name(&mut context);
        let after = keepalive
            .runtime_contract
            .get()
            .expect("rebasing the owned name retains a runtime contract");

        assert_ne!(
            before.backend_platform_name, after.backend_platform_name,
            "the backend name write must replace only its own baseline"
        );
        assert_eq!(
            (
                before.backend_platform_user_data,
                before.owned_flags,
                before.main_viewport_platform_user_data,
                before.main_viewport_platform_handle,
                before.main_viewport_platform_handle_raw,
            ),
            (
                after.backend_platform_user_data,
                after.owned_flags,
                after.main_viewport_platform_user_data,
                after.main_viewport_platform_handle,
                after.main_viewport_platform_handle_raw,
            ),
            "rebasing the backend name must not accept unrelated platform drift"
        );
        assert_eq!(
            platform_callback_ownership(&mut context, &keepalive),
            Ok(())
        );

        let foreign_handle = std::ptr::dangling_mut::<u16>().cast::<c_void>();
        unsafe {
            (*context.main_viewport().as_raw_mut()).PlatformHandle = foreign_handle;
        }
        assert_eq!(
            platform_callback_ownership(&mut context, &keepalive),
            Err(ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                field: "PlatformHandle",
            })
        );

        unsafe {
            (*context.main_viewport().as_raw_mut()).PlatformHandle = std::ptr::null_mut();
        }
        let _ = detach_owned_bridge(&mut context, &keepalive);
        context.set_platform_name::<String>(None).unwrap();
    }

    #[test]
    fn callback_ownership_drift_detaches_owned_handles_without_calling_foreign_destroy() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        FOREIGN_DESTROY_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        FOREIGN_RENDERER_DESTROY_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            install_owned_platform_callbacks(&mut context, &keepalive).unwrap();
        }
        assert_eq!(
            platform_callback_ownership(&mut context, &keepalive),
            Ok(())
        );

        let main_viewport = context.main_viewport().as_raw_mut();
        unsafe {
            platform_create_window_raw_callback(main_viewport);
            (*main_viewport).PlatformHandleRaw = (*main_viewport).PlatformHandle;
            (*main_viewport).PlatformWindowCreated = true;
        }
        keepalive.record_runtime_contract(&mut context);
        let owned_handle = unsafe { (*main_viewport).PlatformHandle };
        assert!(!owned_handle.is_null());
        let foreign_platform_handle = std::ptr::dangling_mut::<u16>().cast::<c_void>();

        unsafe {
            let platform_io = context.platform_io_mut();
            platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_destroy_window));
        }
        assert_eq!(
            platform_callback_ownership(&mut context, &keepalive),
            Err(
                ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                    slot: "Renderer_DestroyWindow",
                }
            )
        );
        unsafe {
            (*main_viewport).PlatformHandle = foreign_platform_handle;
        }
        unsafe {
            let platform_io = context.platform_io_mut();
            platform_io.set_platform_destroy_window_raw(Some(foreign_platform_destroy_window));
            platform_io.set_platform_set_window_pos_raw(Some(foreign_platform_set_window_pos));
        }

        assert_eq!(
            detach_owned_bridge(&mut context, &keepalive),
            Err(
                ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                    slot: "Renderer_DestroyWindow",
                }
            )
        );
        assert_eq!(
            FOREIGN_DESTROY_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a foreign destroy callback must not receive Bevy-owned viewport handles"
        );
        assert_eq!(
            FOREIGN_RENDERER_DESTROY_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a foreign renderer destroy callback must not receive Bevy-owned viewport handles"
        );
        unsafe {
            assert!((*main_viewport).PlatformUserData.is_null());
            assert_eq!(
                (*main_viewport).PlatformHandle,
                foreign_platform_handle,
                "direct detach must preserve a foreign viewport-field replacement"
            );
            assert!((*main_viewport).PlatformHandleRaw.is_null());
        }
        let state = bridge.inner.state.borrow();
        assert!(state.viewport_handles.is_empty());
        assert!(state.commands.is_empty());
        drop(state);
        assert!(context.io().backend_platform_user_data().is_null());

        let platform_io = context.platform_io_mut();
        let raw = unsafe { &*platform_io.as_raw() };
        assert!(raw.Platform_CreateWindow.is_none());
        assert!(raw.Platform_DestroyWindow.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            )
        }));
        assert!(unsafe {
            platform_io
                .clear_platform_set_window_pos_if_pointer_callback(foreign_platform_set_window_pos)
        });
        assert!(raw.Renderer_DestroyWindow.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            )
        }));
        unsafe {
            platform_io.set_platform_destroy_window_raw(None);
            platform_io.set_renderer_destroy_window_raw(None);
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
        }
    }

    #[test]
    fn prepare_platform_viewports_rejects_each_replaced_main_viewport_field() {
        macro_rules! assert_main_viewport_field_drift {
            ($field:ident) => {{
                let mut context = imgui::Context::create();
                let bridge = ImguiViewportBridge::default();
                let keepalive = bridge.keepalive();
                unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
                let context_bridge = ImguiViewportBridgeContext {
                    context_id: context.id(),
                    inner: Rc::clone(&keepalive),
                };
                let primary_window =
                    Entity::from_raw_u32(1).expect("test entity index should be valid");

                prepare_platform_viewports_for_frame(
                    &mut context,
                    &context_bridge,
                    primary_window,
                    &Window::default(),
                    &[],
                    std::iter::empty(),
                    true,
                    native_window::DesktopPositionSupport::Available,
                )
                .unwrap();

                let foreign = std::ptr::dangling_mut::<u16>().cast::<c_void>();
                unsafe {
                    (*context.main_viewport().as_raw_mut()).$field = foreign;
                }
                assert_eq!(
                    prepare_platform_viewports_for_frame(
                        &mut context,
                        &context_bridge,
                        primary_window,
                        &Window::default(),
                        &[],
                        std::iter::empty(),
                        true,
                        native_window::DesktopPositionSupport::Available,
                    ),
                    Err(ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                        field: stringify!($field),
                    })
                );
                assert_eq!(
                    unsafe { (*context.main_viewport().as_raw()).$field },
                    foreign,
                    "frame preparation must not overwrite a foreign main viewport field"
                );
                assert_eq!(
                    bridge.callback_error(),
                    Some(ImguiViewportRuntimeError::CallbackOwnership(
                        ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                            field: stringify!($field),
                        }
                    ))
                );
                assert!(
                    (context.io().backend_flags().bits() & viewport_backend_flag_mask()) == 0,
                    "a partial ownership drift must revoke the Bevy viewport capabilities"
                );
                assert!(
                    !context
                        .io()
                        .config_flags()
                        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE),
                    "a partial ownership drift must disable Bevy-managed viewport execution"
                );

                let _ = detach_owned_bridge(&mut context, &keepalive);
                unsafe {
                    (*context.main_viewport().as_raw_mut()).$field = std::ptr::null_mut();
                }
            }};
        }

        assert_main_viewport_field_drift!(PlatformUserData);
        assert_main_viewport_field_drift!(PlatformHandle);
        assert_main_viewport_field_drift!(PlatformHandleRaw);
    }

    #[test]
    fn direct_callback_preserves_complete_foreign_platform_takeover() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let context_bridge = ImguiViewportBridgeContext {
            context_id: context.id(),
            inner: Rc::clone(&keepalive),
        };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
        prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            true,
            native_window::DesktopPositionSupport::Available,
        )
        .unwrap();

        let foreign_user_data = std::ptr::dangling_mut::<u16>().cast::<c_void>();
        let foreign_main_user_data = std::ptr::dangling_mut::<u32>().cast::<c_void>();
        let foreign_main_handle = std::ptr::dangling_mut::<u64>().cast::<c_void>();
        let foreign_main_handle_raw = std::ptr::dangling_mut::<u8>().cast::<c_void>();
        let foreign_monitor = monitor_from_window(&Window::default());
        let foreign_flags = context.io().backend_flags()
            | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        let foreign_config_flags =
            context.io().config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE;
        unsafe {
            context
                .io_mut()
                .set_backend_platform_user_data(foreign_user_data);
            context.set_platform_name(Some("foreign-platform")).unwrap();
            let platform_io = context.platform_io_mut().as_raw_mut();
            sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
            (*platform_io).Platform_ShowWindow = Some(foreign_platform_destroy_window);
            (*context.main_viewport().as_raw_mut()).PlatformUserData = foreign_main_user_data;
            (*context.main_viewport().as_raw_mut()).PlatformHandle = foreign_main_handle;
            (*context.main_viewport().as_raw_mut()).PlatformHandleRaw = foreign_main_handle_raw;
        }
        unsafe { context.platform_io_mut().set_monitors(&[foreign_monitor]) };
        context.io_mut().set_backend_flags(foreign_flags);
        context.io_mut().set_config_flags(foreign_config_flags);

        let main_viewport = context.main_viewport().as_raw_mut();
        unsafe { platform_show_window_raw_callback(main_viewport) };

        assert_eq!(
            bridge.callback_error(),
            Some(ImguiViewportRuntimeError::CallbackOwnership(
                ImguiViewportCallbackOwnershipError::BackendPlatformUserDataReplaced,
            ))
        );
        assert_eq!(context.io().backend_flags(), foreign_flags);
        assert_eq!(context.io().config_flags(), foreign_config_flags);
        assert_eq!(context.io().backend_platform_user_data(), foreign_user_data);
        assert_eq!(
            context.io().backend_platform_name().unwrap().to_bytes(),
            b"foreign-platform"
        );
        unsafe {
            let main_viewport = context.main_viewport().as_raw();
            assert_eq!((*main_viewport).PlatformUserData, foreign_main_user_data);
            assert_eq!((*main_viewport).PlatformHandle, foreign_main_handle);
            assert_eq!((*main_viewport).PlatformHandleRaw, foreign_main_handle_raw);
            let platform_io = context.platform_io().as_raw();
            assert_eq!((*platform_io).Monitors.Size, 1);
            assert_eq!(*(*platform_io).Monitors.Data, foreign_monitor);
            assert!((*platform_io).Platform_ShowWindow.is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_platform_destroy_window
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            }));
        }

        let _ = detach_owned_bridge(&mut context, &keepalive);
        unsafe {
            let platform_io = context.platform_io_mut().as_raw_mut();
            sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
            context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
            context.set_platform_name::<String>(None).unwrap();
            (*context.main_viewport().as_raw_mut()).PlatformUserData = std::ptr::null_mut();
            (*context.main_viewport().as_raw_mut()).PlatformHandle = std::ptr::null_mut();
            (*context.main_viewport().as_raw_mut()).PlatformHandleRaw = std::ptr::null_mut();
        }
        unsafe { context.platform_io_mut().set_monitors(&[]) };
        let mut flags = context.io().backend_flags();
        flags.remove(
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
                | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
        );
        context.io_mut().set_backend_flags(flags);
        let mut config_flags = context.io().config_flags();
        config_flags.remove(imgui::ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_flags(config_flags);
    }

    #[test]
    fn prepare_platform_viewports_prunes_handles_for_missing_viewports() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let context_bridge = ImguiViewportBridgeContext {
            context_id: context.id(),
            inner: Rc::clone(&keepalive),
        };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
        let secondary_window = Entity::from_raw_u32(2).expect("test entity index should be valid");
        let stale_viewport = imgui::Id::from(0x500);
        let live_viewport = imgui::Id::from(0x501);

        bridge
            .inner
            .state
            .borrow_mut()
            .platform_handle(ImguiViewportIdentity {
                id: stale_viewport,
                address: 0,
            });
        bridge
            .inner
            .state
            .borrow_mut()
            .platform_handle(ImguiViewportIdentity {
                id: live_viewport,
                address: 0,
            });

        prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::once((secondary_window, live_viewport, feedback())),
            true,
            native_window::DesktopPositionSupport::Available,
        )
        .unwrap();

        let main_viewport_id = context.main_viewport().id();
        let state = bridge.inner.state.borrow();
        assert!(state.viewport_handles.contains_key(&main_viewport_id));
        assert!(state.viewport_handles.contains_key(&live_viewport));
        assert!(
            !state.viewport_handles.contains_key(&stale_viewport),
            "platform handles must not outlive viewports that disappeared from the Bevy mapping"
        );
        drop(state);

        detach_owned_bridge(&mut context, &keepalive).unwrap();
    }

    #[test]
    fn cleanup_clears_handles_filtered_from_the_public_viewport_snapshot() {
        let mut context = imgui::Context::create();
        let bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let context_bridge = ImguiViewportBridgeContext {
            context_id: context.id(),
            inner: Rc::clone(&keepalive),
        };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");

        prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            true,
            native_window::DesktopPositionSupport::Available,
        )
        .unwrap();

        let main_viewport = context.main_viewport().as_raw_mut();
        assert!(unsafe { !(*main_viewport).PlatformHandle.is_null() });
        let mut filtered_snapshot = [];
        {
            // Dear ImGui's internal list still includes the main viewport. This only models the
            // filtered public `PlatformIO.Viewports` snapshot that hides a live viewport.
            let _viewports_guard = unsafe {
                PlatformViewportsGuard::replace(
                    &mut context,
                    &mut filtered_snapshot,
                    std::ptr::null_mut(),
                )
            };
            clear_imgui_viewport_platform_handles(&mut context, &context_bridge);
            assert!(
                unsafe { (*main_viewport).PlatformHandle.is_null() },
                "cleanup must clear a hidden backend-owned PlatformHandle before dropping it"
            );
            assert!(
                unsafe { (*main_viewport).PlatformUserData.is_null() },
                "cleanup must clear a hidden backend-owned PlatformUserData before dropping it"
            );
        }

        // The direct cleanup above intentionally changed the main viewport's owned fields. Rebase
        // the test's runtime contract before exercising the ordinary bridge detach path.
        keepalive.record_runtime_contract(&mut context);
        detach_owned_bridge(&mut context, &keepalive).unwrap();
    }
}
