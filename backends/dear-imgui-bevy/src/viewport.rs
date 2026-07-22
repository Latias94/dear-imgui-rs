//! Dear ImGui platform-viewport bridge for Bevy-owned windows.
//!
//! PlatformIO callbacks installed here only capture intent into an engine-owned queue. Bevy systems
//! drain that queue and mutate ECS-owned [`Window`] entities outside the C ABI callback boundary.

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
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::SystemParam;
use bevy_math::IVec2;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_window::WindowRef;
use bevy_window::{
    CompositeAlphaMode, PresentMode, Window, WindowLevel, WindowPosition, WindowResolution,
    WindowTheme,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{
    ExitSystems, Monitor, PrimaryWindow, WindowCloseRequested, WindowMoved, WindowOccluded,
    WindowResized,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::WINIT_WINDOWS;
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ffi::{CStr, c_char, c_void};
use std::rc::Rc;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Weak;

/// Policy applied to every Bevy window created for a secondary Dear ImGui viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiViewportWindowConfig {
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub desired_maximum_frame_latency: Option<std::num::NonZeroU32>,
    pub window_theme: Option<WindowTheme>,
    pub transparent: bool,
}

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
#[derive(Clone, Debug, PartialEq)]
pub struct ImguiViewportSnapshot {
    pub id: ImguiViewportId,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub dpi_scale: f32,
    pub flags: imgui::ViewportFlags,
}

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

/// Intent captured from Dear ImGui PlatformIO callbacks.
#[derive(Clone, Debug, PartialEq)]
pub enum ImguiViewportCommand {
    Create(ImguiViewportSnapshot),
    Destroy { id: ImguiViewportId },
    Show { id: ImguiViewportId },
    SetPos { id: ImguiViewportId, pos: [f32; 2] },
    SetSize { id: ImguiViewportId, size: [f32; 2] },
    SetFocus { id: ImguiViewportId },
    SetTitle { id: ImguiViewportId, title: String },
}

/// Last Bevy-observed platform state for a Dear ImGui viewport window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImguiViewportFeedback {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub dpi_scale: f32,
    pub focused: bool,
    pub minimized: bool,
}

/// Marker on Bevy `Window` entities created for Dear ImGui secondary platform viewports.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportWindow {
    pub viewport_id: ImguiViewportId,
}

/// Marker on Bevy camera entities created to render Dear ImGui secondary platform viewports.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportCamera {
    pub viewport_id: ImguiViewportId,
}

pub(crate) type ImguiViewportBridgeKeepalive = Rc<ImguiViewportBridgeShared>;

/// Backend-local queue and viewport-to-window map for Dear ImGui platform windows.
#[derive(Default)]
pub struct ImguiViewportBridge {
    inner: ImguiViewportBridgeKeepalive,
}

#[derive(Default)]
pub(crate) struct ImguiViewportBridgeShared {
    state: RefCell<ImguiViewportBridgeState>,
    callback_fault: Cell<Option<ImguiViewportBridgeError>>,
    ecs_release_pending: Cell<bool>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    native_teardown_in_progress: Cell<bool>,
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
impl ImguiViewportBridgeShared {
    fn clear_viewport_state_preserving_pending_despawns(&self) {
        let mut state = self.state.borrow_mut();
        state.viewport_windows.clear();
        state.viewport_cameras.clear();
        state.viewport_feedback.clear();
        state.viewport_flags.clear();
        state.viewport_handles.clear();
        state.retired_viewport_handles.clear();
        state.commands.clear();
        state.focus_next_frame.clear();
        state.focus_ready.clear();
        drop(state);
        self.callback_fault.set(None);
    }

    fn clear_viewport_state(&self) {
        self.clear_viewport_state_preserving_pending_despawns();
        self.state.borrow_mut().pending_ecs_despawns.clear();
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
        state.viewport_handles.clear();
        state.retired_viewport_handles.clear();
        state.focus_next_frame.clear();
        state.focus_ready.clear();
        drop(state);

        self.callback_fault.set(None);
        self.ecs_release_pending.set(true);
    }

    fn ecs_release_pending(&self) -> bool {
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
        state.pending_ecs_despawns.clone()
    }

    fn acknowledge_ecs_despawns(&self, mut entity_is_live: impl FnMut(Entity) -> bool) {
        {
            let mut state = self.state.borrow_mut();
            state
                .pending_ecs_despawns
                .retain(|entity| entity_is_live(*entity));
        }
        if self.ecs_release_pending() && !self.has_tracked_ecs_entities() {
            self.finish_ecs_release();
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
        let io = unsafe { &*sys::igGetIO_ContextPtr(context.as_raw()) };
        let main_viewport = context.main_viewport();
        self.runtime_contract
            .set(Some(ImguiViewportRuntimeContract {
                backend_platform_user_data: io.BackendPlatformUserData,
                backend_platform_name: io.BackendPlatformName,
                owned_flags: io.BackendFlags & viewport_backend_flag_mask(),
                main_viewport_platform_user_data: main_viewport.platform_user_data(),
                main_viewport_platform_handle: main_viewport.platform_handle(),
                main_viewport_platform_handle_raw: main_viewport.platform_handle_raw(),
            }));
    }

    fn record_callback_fault(&self, error: ImguiViewportBridgeError) {
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
        if expected_runtime.main_viewport_platform_user_data != std::ptr::null_mut()
            && main_viewport.PlatformUserData == expected_runtime.main_viewport_platform_user_data
        {
            return true;
        }
        if expected_runtime.main_viewport_platform_handle != std::ptr::null_mut()
            && main_viewport.PlatformHandle == expected_runtime.main_viewport_platform_handle
        {
            return true;
        }
        if expected_runtime.main_viewport_platform_handle_raw != std::ptr::null_mut()
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

/// Deferred failure reported by a Dear ImGui PlatformIO callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportBridgeError {
    /// The callback attempted to re-enter the bridge while its command queue was borrowed.
    CallbackQueueBusy,
    /// A native backend field changed after the Bevy viewport bridge claimed it.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    CallbackOwnership(ImguiViewportCallbackOwnershipError),
}

impl std::fmt::Display for ImguiViewportBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CallbackQueueBusy => formatter.write_str(
                "a Dear ImGui viewport callback could not borrow the Bevy command queue",
            ),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::CallbackOwnership(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ImguiViewportBridgeError {}

#[derive(Default)]
pub(crate) struct ImguiViewportBridgeState {
    commands: Vec<ImguiViewportCommand>,
    viewport_windows: HashMap<ImguiViewportId, Entity>,
    viewport_cameras: HashMap<ImguiViewportId, Entity>,
    pending_ecs_despawns: HashSet<Entity>,
    viewport_feedback: HashMap<ImguiViewportId, ImguiViewportFeedback>,
    viewport_flags: HashMap<ImguiViewportId, imgui::ViewportFlags>,
    viewport_handles: HashMap<ImguiViewportId, Box<ImguiViewportPlatformHandle>>,
    retired_viewport_handles: HashMap<ImguiViewportId, Box<ImguiViewportPlatformHandle>>,
    focus_next_frame: HashSet<ImguiViewportId>,
    focus_ready: HashSet<ImguiViewportId>,
}

/// Identifies one exact Dear ImGui viewport without retaining a dereferenceable native pointer.
///
/// Dear ImGui may omit a still-live viewport from `PlatformIO.Viewports`, and can later reuse its
/// numeric ID. Cleanup therefore resolves the ID through Dear ImGui's internal registry and
/// verifies the address before touching native fields.
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

#[derive(Debug)]
struct ImguiViewportPlatformHandle {
    identity: ImguiViewportIdentity,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy)]
struct ImguiViewportHandleRef {
    identity: ImguiViewportIdentity,
    pointer: *mut c_void,
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
        {
            if handle.identity == identity {
                self.viewport_handles.insert(viewport_id, handle);
            }
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

    fn remove_platform_handle(&mut self, viewport_id: ImguiViewportId) {
        let active = self.viewport_handles.remove(&viewport_id);
        let retired = self.retired_viewport_handles.remove(&viewport_id);
        debug_assert!(active.is_none() || retired.is_none());
        drop(active.or(retired));
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

    fn set_viewport_flags(&mut self, viewport_id: ImguiViewportId, flags: imgui::ViewportFlags) {
        self.viewport_flags.insert(viewport_id, flags);
    }
}

impl ImguiViewportBridge {
    /// Returns an owned snapshot of the currently queued viewport commands.
    ///
    /// The returned commands no longer borrow the bridge, and reading them does not remove them
    /// from the queue. Use [`Self::drain_commands`] to consume queued commands.
    #[must_use]
    pub fn commands(&self) -> Vec<ImguiViewportCommand> {
        self.inner.state.borrow().commands.clone()
    }

    #[must_use]
    pub fn viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .viewport_windows
            .get(&viewport_id)
            .copied()
    }

    #[must_use]
    pub fn viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .viewport_cameras
            .get(&viewport_id)
            .copied()
    }

    #[must_use]
    pub fn viewport_feedback(&self, viewport_id: ImguiViewportId) -> Option<ImguiViewportFeedback> {
        self.inner
            .state
            .borrow()
            .viewport_feedback
            .get(&viewport_id)
            .copied()
    }

    pub fn queue(&mut self, command: ImguiViewportCommand) {
        self.inner.state.borrow_mut().commands.push(command);
    }

    /// Removes and returns all currently queued viewport commands.
    ///
    /// A deferred native callback fault returns its precise error without consuming any commands.
    /// That fault remains sticky until the viewport bridge is torn down and rebuilt.
    pub fn drain_commands(
        &mut self,
    ) -> Result<Vec<ImguiViewportCommand>, ImguiViewportBridgeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self.inner.state.borrow_mut().commands.drain(..).collect())
    }

    /// Returns a deferred callback failure from the native callback boundary.
    ///
    /// Reading the error does not clear it. The failure remains sticky until the viewport bridge is
    /// torn down and rebuilt, so callers cannot accidentally resume from a partially observed
    /// callback sequence.
    #[must_use]
    pub fn callback_error(&self) -> Option<ImguiViewportBridgeError> {
        self.inner.callback_fault.get()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    fn clear_viewport_state(&mut self) {
        self.inner.clear_viewport_state();
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn keepalive(&self) -> ImguiViewportBridgeKeepalive {
        Rc::clone(&self.inner)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn set_viewport_window(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner
            .state
            .borrow_mut()
            .viewport_windows
            .insert(viewport_id, entity);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_window(&mut self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .viewport_windows
            .remove(&viewport_id)
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    fn set_viewport_camera(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner
            .state
            .borrow_mut()
            .viewport_cameras
            .insert(viewport_id, entity);
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    fn remove_viewport_camera(&mut self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .viewport_cameras
            .remove(&viewport_id)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_feedback(&mut self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .viewport_feedback
            .remove(&viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_flags(&mut self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .viewport_flags
            .remove(&viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_platform_handle(&mut self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .remove_platform_handle(viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn set_viewport_feedback(
        &mut self,
        viewport_id: ImguiViewportId,
        feedback: ImguiViewportFeedback,
    ) {
        self.inner
            .state
            .borrow_mut()
            .viewport_feedback
            .insert(viewport_id, feedback);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn show_should_focus(&self, viewport_id: ImguiViewportId) -> bool {
        !self
            .inner
            .state
            .borrow()
            .viewport_flags
            .get(&viewport_id)
            .is_some_and(|flags| flags.contains(imgui::ViewportFlags::NO_FOCUS_ON_APPEARING))
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn request_focus_next_frame(&mut self, viewport_id: ImguiViewportId) {
        self.inner
            .state
            .borrow_mut()
            .focus_next_frame
            .insert(viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn clear_focus_request(&mut self, viewport_id: ImguiViewportId) {
        let mut state = self.inner.state.borrow_mut();
        state.focus_next_frame.remove(&viewport_id);
        state.focus_ready.remove(&viewport_id);
    }
}

pub(crate) fn install_viewport_bridge(_app: &mut App) {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        let app = _app;
        let multi_viewport_requested = app
            .world()
            .get_resource::<crate::ImguiBackendConfig>()
            .is_some_and(|config| config.multi_viewport);
        if !multi_viewport_requested {
            return;
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
        attach_bridge_to_imgui_context(app.world_mut());
        app.add_systems(
            crate::ImguiEndFrame,
            (
                apply_viewport_commands_system.after(crate::context::end_primary_frame_system),
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred(),
        );
        app.add_systems(
            Last,
            (
                cleanup_secondary_viewports_when_primary_is_unavailable,
                ApplyDeferred,
                acknowledge_viewport_ecs_despawns_system,
            )
                .chain_ignore_deferred()
                .before(ExitSystems),
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn attach_bridge_to_imgui_context(world: &mut World) {
    assert!(
        sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
        "dear-imgui-bevy multi-viewport requires PlatformIO aggregate ABI hooks"
    );

    let bridge_keepalive = {
        let Some(bridge) = world.get_non_send::<ImguiViewportBridge>() else {
            return;
        };
        bridge.keepalive()
    };

    let Some(mut imgui_context) = world.get_non_send_mut::<crate::ImguiContext>() else {
        return;
    };
    // SAFETY: the bridge resource keeps the allocation stable while installation publishes its
    // pointer, and the context retains the cloned Rc immediately after installation succeeds.
    unsafe { install_owned_platform_callbacks(imgui_context.context_mut(), &bridge_keepalive) }
        .unwrap_or_else(|error| panic!("cannot install Dear ImGui viewport callbacks: {error}"));
    imgui_context.attach_viewport_bridge(bridge_keepalive);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImguiViewportCallbackInstallError {
    BackendPlatformUserDataOccupied,
    BackendPlatformNameOccupied,
    BackendFlagOccupied { flag: &'static str },
    CallbackSlotOccupied { slot: &'static str },
    MainViewportFieldOccupied { field: &'static str },
    PlatformMonitorsOccupied,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl std::fmt::Display for ImguiViewportCallbackInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendPlatformUserDataOccupied => {
                formatter.write_str("Dear ImGui BackendPlatformUserData is already owned")
            }
            Self::BackendPlatformNameOccupied => {
                formatter.write_str("Dear ImGui BackendPlatformName is already owned")
            }
            Self::BackendFlagOccupied { flag } => {
                write!(
                    formatter,
                    "Dear ImGui backend flag `{flag}` is already owned"
                )
            }
            Self::CallbackSlotOccupied { slot } => {
                write!(formatter, "Dear ImGui {slot} callback is already owned")
            }
            Self::MainViewportFieldOccupied { field } => {
                write!(
                    formatter,
                    "Dear ImGui main viewport {field} is already owned"
                )
            }
            Self::PlatformMonitorsOccupied => {
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
        return Err(ImguiViewportCallbackInstallError::BackendPlatformUserDataOccupied);
    }
    if context.io().backend_platform_name().is_some() {
        return Err(ImguiViewportCallbackInstallError::BackendPlatformNameOccupied);
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
            return Err(ImguiViewportCallbackInstallError::BackendFlagOccupied { flag: name });
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
            return Err(ImguiViewportCallbackInstallError::MainViewportFieldOccupied { field });
        }
    }
    let raw = unsafe { &*context.platform_io().as_raw() };
    if !raw.Monitors.Data.is_null() || raw.Monitors.Size != 0 || raw.Monitors.Capacity != 0 {
        return Err(ImguiViewportCallbackInstallError::PlatformMonitorsOccupied);
    }
    macro_rules! reject_occupied_slots {
        ($($slot:ident),+ $(,)?) => {
            $(
                if raw.$slot.is_some() {
                    return Err(ImguiViewportCallbackInstallError::CallbackSlotOccupied {
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
pub(crate) unsafe fn install_owned_platform_callbacks(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackInstallError> {
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
fn validate_hidden_callback_contract(
    context: &mut imgui::Context,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let platform_io = context.platform_io_mut();
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
    keepalive.record_callback_fault(ImguiViewportBridgeError::CallbackOwnership(error));
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
    if let Some(ImguiViewportBridgeError::CallbackOwnership(error)) = keepalive.callback_fault.get()
    {
        return Err(error);
    }
    let main_viewport = context.main_viewport().as_raw();
    let validation = validate_platform_contract_raw(context.as_raw(), main_viewport, keepalive)
        .and_then(|()| validate_hidden_callback_contract(context));
    match validation {
        Ok(()) => Ok(()),
        Err(error) => Err(latch_platform_ownership_fault(
            context.as_raw(),
            main_viewport,
            keepalive,
            error,
        )),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn record_platform_runtime_contract(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) {
    keepalive.record_runtime_contract(context);
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

/// Detach the Bevy viewport bridge without invoking callbacks whose ownership has drifted.
///
/// On an intact callback lineage this asks Dear ImGui to destroy secondary platform windows first.
/// If either destroy callback is foreign, it instead clears only Bevy-owned viewport pointers
/// directly. Both paths release owned callback slots, backend data, handles, and queued bridge
/// state before returning the detected ownership result.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn detach_owned_bridge(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let main_viewport_id = context.main_viewport().id();
    let mut ownership = platform_callback_ownership(context, keepalive);
    if ownership.is_ok() {
        // The complete contract was validated immediately above. Dear ImGui invokes the destroy
        // callbacks synchronously, and each successful callback invalidates part of that runtime
        // contract before the next viewport is visited. Keep registry-based callback access live
        // for this native teardown transaction without treating those expected changes as drift.
        let _teardown = NativePlatformTeardownGuard::enter(&keepalive.native_teardown_in_progress);
        context.destroy_platform_windows();
    }

    if !keepalive.clear_monitors_if_owned(context) && ownership.is_ok() {
        ownership = Err(ImguiViewportCallbackOwnershipError::PlatformMonitorsReplaced);
    }

    clear_owned_platform_callbacks(context);
    clear_imgui_viewport_platform_handles_for_keepalive(context, keepalive);
    clear_backend_platform_user_data_if_owned(context, keepalive);
    ImguiViewportBridgeShared::unregister_context_owner(context, keepalive);
    keepalive.prepare_ecs_release(main_viewport_id);
    ownership
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_ecs_release_pending(keepalive: &ImguiViewportBridgeKeepalive) -> bool {
    keepalive.has_tracked_ecs_entities()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn finish_viewport_ecs_release(keepalive: &ImguiViewportBridgeKeepalive) {
    keepalive.finish_ecs_release();
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
        shared.record_callback_fault(ImguiViewportBridgeError::CallbackQueueBusy);
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
            bridge.set_viewport_flags(viewport.id(), viewport.flags());
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
            bridge.set_viewport_flags(viewport.id(), viewport.flags());
            bridge.queue(ImguiViewportCommand::Show { id: viewport.id() });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_pos(viewport: *mut imgui::Viewport, pos: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetPos {
                id: viewport.id(),
                pos: [pos.x, pos.y],
            });
        })
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_size(viewport: *mut imgui::Viewport, size: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let _ = unsafe {
        with_current_bridge_mut(|bridge| {
            bridge.queue(ImguiViewportCommand::SetSize {
                id: viewport.id(),
                size: [size.x, size.y],
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
    viewport_windows: Query<(Entity, &ImguiViewportWindow)>,
    mut imgui_context: NonSendMut<crate::ImguiContext>,
    mut bridge: NonSendMut<ImguiViewportBridge>,
) {
    let window_to_viewport = viewport_windows.iter().collect::<HashMap<_, _>>();
    let mut moved_viewports = HashSet::new();
    let mut resized_viewports = HashSet::new();
    let mut closed_viewports = HashSet::new();

    for event in events.moved.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            moved_viewports.insert(viewport_window.viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let previous = bridge.viewport_feedback(viewport_window.viewport_id);
                bridge.set_viewport_feedback(
                    viewport_window.viewport_id,
                    feedback_from_window_for_entity(event.window, window, previous, None),
                );
            }
        }
    }

    for event in events.resized.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            resized_viewports.insert(viewport_window.viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let previous = bridge.viewport_feedback(viewport_window.viewport_id);
                bridge.set_viewport_feedback(
                    viewport_window.viewport_id,
                    feedback_from_window_for_entity(event.window, window, previous, None),
                );
            }
        }
    }

    for event in events.close_requests.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            closed_viewports.insert(viewport_window.viewport_id);
        }
    }

    for event in events.occluded.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied()
            && let Ok(window) = windows.get(event.window)
        {
            let previous = bridge.viewport_feedback(viewport_window.viewport_id);
            bridge.set_viewport_feedback(
                viewport_window.viewport_id,
                feedback_from_window_for_entity(
                    event.window,
                    window,
                    previous,
                    Some(event.occluded),
                ),
            );
        }
    }

    mark_platform_viewport_requests(
        imgui_context.context_mut(),
        moved_viewports.iter().copied(),
        resized_viewports.iter().copied(),
        closed_viewports.iter().copied(),
    );
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
#[allow(unused_variables)]
fn apply_viewport_commands_system(
    mut ecs_commands: Commands,
    mut bridge: NonSendMut<ImguiViewportBridge>,
    config: Res<crate::ImguiBackendConfig>,
    mut windows: Query<&mut Window>,
    viewport_windows: Query<(Entity, &ImguiViewportWindow)>,
    viewport_cameras: Query<(Entity, &ImguiViewportCamera)>,
) {
    let queued = bridge
        .drain_commands()
        .unwrap_or_else(|error| panic!("Dear ImGui viewport callback failed: {error}"));
    if bridge.ecs_release_pending() {
        for entity in bridge.inner.take_all_ecs_entities_for_release() {
            ecs_commands.entity(entity).try_despawn();
        }
        return;
    }
    for entity in bridge.inner.pending_ecs_despawns() {
        ecs_commands.entity(entity).try_despawn();
    }

    let viewport_window_config = config.viewport_window.validate().unwrap_or_else(|error| {
        panic!("invalid Dear ImGui viewport window configuration: {error}")
    });
    let mut feedback_candidates = HashSet::new();
    let mut pending_windows: HashMap<ImguiViewportId, Window> = HashMap::new();
    #[cfg(feature = "render")]
    let mut pending_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut scheduled_camera_despawns = HashSet::new();
    #[cfg(feature = "render")]
    let live_cameras = viewport_cameras
        .iter()
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();
    for command in queued {
        match command {
            ImguiViewportCommand::Create(snapshot) => {
                bridge
                    .inner
                    .state
                    .borrow_mut()
                    .viewport_flags
                    .insert(snapshot.id, snapshot.flags);
                let entity = if let Some(entity) = bridge.viewport_window(snapshot.id) {
                    entity
                } else {
                    let entity = ecs_commands
                        .spawn((
                            window_from_snapshot_with_config(&snapshot, viewport_window_config)
                                .expect("the viewport window configuration was validated"),
                            ImguiViewportWindow {
                                viewport_id: snapshot.id,
                            },
                        ))
                        .id();
                    bridge.set_viewport_window(snapshot.id, entity);
                    entity
                };
                #[cfg(feature = "render")]
                ensure_viewport_camera(
                    &mut ecs_commands,
                    &mut bridge,
                    snapshot.id,
                    entity,
                    viewport_window_config.transparent,
                    snapshot.flags,
                    &live_cameras,
                    &mut pending_cameras,
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
                if let Some(entity) = bridge.remove_viewport_window(id) {
                    bridge.inner.track_ecs_despawn(entity);
                    ecs_commands.entity(entity).despawn();
                }
                bridge.remove_viewport_feedback(id);
                bridge.remove_viewport_flags(id);
                bridge.remove_platform_handle(id);
                bridge.clear_focus_request(id);
                #[cfg(feature = "render")]
                {
                    pending_cameras.remove(&id);
                    if let Some(entity) = bridge.remove_viewport_camera(id) {
                        scheduled_camera_despawns.insert(entity);
                        bridge.inner.track_ecs_despawn(entity);
                        ecs_commands.entity(entity).despawn();
                    }
                }
            }
            ImguiViewportCommand::Show { id } => {
                let should_focus = bridge.show_should_focus(id);
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.visible = true;
                    if should_focus {
                        window.focused = false;
                    }
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.visible = true;
                        if should_focus {
                            window.focused = false;
                        }
                    });
                }
                if should_focus {
                    bridge.request_focus_next_frame(id);
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetPos { id, pos } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.position = WindowPosition::At(physical_pos_for_window(pos, window));
                } else {
                    if let Some(entity) = bridge.viewport_window(id)
                        && let Ok(mut window) = windows.get_mut(entity)
                    {
                        window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
                            entity, pos, &window,
                        ));
                    }
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetSize { id, size } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    set_window_logical_size(window, size);
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        set_window_logical_size(window, size);
                    });
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetFocus { id } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.focused = false;
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.focused = false;
                    });
                }
                bridge.request_focus_next_frame(id);
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetTitle { id, title } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.title = title;
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.title = title;
                    });
                }
                feedback_candidates.insert(id);
            }
        }
    }

    let pending_viewport_ids = pending_windows.keys().copied().collect::<HashSet<_>>();
    for (viewport_id, window) in pending_windows {
        if let Some(entity) = bridge.viewport_window(viewport_id) {
            let previous = bridge.viewport_feedback(viewport_id);
            bridge.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, &window, previous, None),
            );
            ecs_commands.entity(entity).insert(window);
        }
    }

    for viewport_id in feedback_candidates {
        if pending_viewport_ids.contains(&viewport_id) {
            continue;
        }
        if let Some(entity) = bridge.viewport_window(viewport_id)
            && let Ok(window) = windows.get(entity)
        {
            let previous = bridge.viewport_feedback(viewport_id);
            bridge.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, window, previous, None),
            );
        }
    }

    apply_pending_viewport_focus_requests(&mut windows, &mut bridge);

    #[cfg(feature = "render")]
    for (window_entity, viewport_window) in viewport_windows.iter() {
        if bridge.viewport_window(viewport_window.viewport_id) != Some(window_entity) {
            continue;
        }
        let flags = bridge
            .inner
            .state
            .borrow()
            .viewport_flags
            .get(&viewport_window.viewport_id)
            .copied()
            .unwrap_or_else(imgui::ViewportFlags::empty);
        ensure_viewport_camera(
            &mut ecs_commands,
            &mut bridge,
            viewport_window.viewport_id,
            window_entity,
            viewport_window_config.transparent,
            flags,
            &live_cameras,
            &mut pending_cameras,
        );
    }

    #[cfg(feature = "render")]
    cleanup_orphaned_viewport_cameras(
        &mut ecs_commands,
        &mut bridge,
        live_cameras.into_iter(),
        &scheduled_camera_despawns,
    );
    #[cfg(not(feature = "render"))]
    cleanup_orphaned_viewport_cameras(
        &mut ecs_commands,
        &mut bridge,
        viewport_cameras.iter().map(|(entity, _)| entity),
    );
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn acknowledge_viewport_ecs_despawns_system(
    bridge: NonSend<ImguiViewportBridge>,
    entities: Query<Entity>,
) {
    bridge
        .inner
        .acknowledge_ecs_despawns(|entity| entities.get(entity).is_ok());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_pending_viewport_focus_requests(
    windows: &mut Query<&mut Window>,
    bridge: &mut ImguiViewportBridge,
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
fn cleanup_secondary_viewports_when_primary_is_unavailable(
    mut ecs_commands: Commands,
    mut close_requests: MessageReader<WindowCloseRequested>,
    primary_windows: Query<Entity, With<PrimaryWindow>>,
    viewport_windows: Query<Entity, With<ImguiViewportWindow>>,
    viewport_cameras: Query<Entity, With<ImguiViewportCamera>>,
    mut imgui_context: NonSendMut<crate::ImguiContext>,
    bridge: NonSendMut<ImguiViewportBridge>,
) {
    let primary_window = primary_windows.single().ok();
    let primary_close_requested = primary_window.is_some_and(|primary_window| {
        close_requests
            .read()
            .any(|event| event.window == primary_window)
    });

    if primary_window.is_some() && !primary_close_requested {
        return;
    }

    let viewport_entities = viewport_windows
        .iter()
        .chain(viewport_cameras.iter())
        .collect::<HashSet<_>>();
    bridge
        .inner
        .track_ecs_despawns(viewport_entities.iter().copied());
    for entity in viewport_entities {
        ecs_commands.entity(entity).despawn();
    }
    clear_imgui_viewport_platform_handles(imgui_context.context_mut(), &bridge);
    bridge
        .inner
        .clear_viewport_state_preserving_pending_despawns();
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridge,
) {
    clear_imgui_viewport_platform_handles_for_keepalive(context, &bridge.inner);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_imgui_viewport_platform_handles_for_keepalive(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
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
        })
        .collect::<Vec<_>>();
    drop(state);
    clear_imgui_viewport_platform_handles_for_owned_handles(context, &owned_handles);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_stale_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridge,
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
            unsafe {
                if viewport.platform_handle() == owned_handle.pointer {
                    viewport.set_platform_handle(std::ptr::null_mut());
                }
                if viewport.platform_user_data() == owned_handle.pointer {
                    viewport.set_platform_user_data(std::ptr::null_mut());
                }
                if viewport.platform_handle_raw() == owned_handle.pointer {
                    viewport.set_platform_handle_raw(std::ptr::null_mut());
                }
            }
        }
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn prepare_platform_viewports_for_frame(
    context: &mut imgui::Context,
    bridge: &mut ImguiViewportBridge,
    primary_window: Entity,
    window: &Window,
    monitors: &[sys::ImGuiPlatformMonitor],
    viewport_windows: impl Iterator<Item = (Entity, ImguiViewportId, ImguiViewportFeedback)>,
    enable_viewports: bool,
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
    if enable_viewports {
        backend_flags |= imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    }
    io.set_backend_flags(backend_flags);

    let mut config_flags = io.config_flags();
    if enable_viewports {
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
    bridge: &mut ImguiViewportBridge,
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
        bridge.inner.track_ecs_despawn(camera);
        ecs_commands.entity(camera).despawn();
    }
}

#[cfg(all(
    not(feature = "render"),
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn cleanup_orphaned_viewport_cameras(
    _ecs_commands: &mut Commands,
    _bridge: &mut ImguiViewportBridge,
    _viewport_cameras: impl Iterator<Item = Entity>,
) {
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn ensure_viewport_camera(
    ecs_commands: &mut Commands,
    bridge: &mut ImguiViewportBridge,
    viewport_id: ImguiViewportId,
    window_entity: Entity,
    transparent: bool,
    flags: imgui::ViewportFlags,
    live_cameras: &HashSet<Entity>,
    pending_cameras: &mut HashSet<ImguiViewportId>,
) {
    if let Some(camera) = bridge.viewport_camera(viewport_id) {
        if live_cameras.contains(&camera) || pending_cameras.contains(&viewport_id) {
            return;
        }
        bridge.remove_viewport_camera(viewport_id);
    }
    if !pending_cameras.insert(viewport_id) {
        return;
    }

    let camera = ecs_commands
        .spawn((
            Camera2d,
            viewport_camera(transparent, flags),
            RenderTarget::Window(WindowRef::Entity(window_entity)),
            RenderLayers::none(),
            crate::render::ImguiOverlayCamera,
            ImguiViewportCamera { viewport_id },
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

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn monitor_from_window(window: &Window) -> sys::ImGuiPlatformMonitor {
    let mut monitor = sys::ImGuiPlatformMonitor::default();
    let pos = match window.position {
        WindowPosition::At(pos) => logical_pos(pos, window),
        WindowPosition::Automatic | WindowPosition::Centered(_) => [0.0, 0.0],
    };
    let size = [window.width().max(1.0), window.height().max(1.0)];
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
pub fn platform_monitors_from_bevy_monitors(
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
    let pos = monitor.physical_position.as_vec2() / scale;
    let size = bevy_math::Vec2::new(
        monitor.physical_width as f32,
        monitor.physical_height as f32,
    ) / scale;
    let mut platform_monitor = sys::ImGuiPlatformMonitor::default();
    platform_monitor.MainPos = sys::ImVec2 { x: pos.x, y: pos.y };
    platform_monitor.MainSize = sys::ImVec2 {
        x: size.x,
        y: size.y,
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
    let pos = window_client_origin_logical(entity, &window.position, window.scale_factor())
        .or_else(|| previous.map(|feedback| feedback.pos))
        .unwrap_or([0.0, 0.0]);
    let scale_factor = window_client_scale_factor(entity, window);
    ImguiViewportFeedback {
        pos,
        size: [window.width().max(0.0), window.height().max(0.0)],
        framebuffer_scale: [scale_factor, scale_factor],
        dpi_scale: scale_factor,
        focused: window.focused,
        minimized: minimized
            .or_else(|| previous.map(|feedback| feedback.minimized))
            .unwrap_or(false),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn window_client_origin_logical(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
) -> Option<[f32; 2]> {
    if let Some(pos) = winit_window_client_origin_logical(entity) {
        return Some(pos);
    }
    match *position {
        WindowPosition::At(pos) => Some(logical_pos_with_scale(pos, scale_factor)),
        WindowPosition::Automatic | WindowPosition::Centered(_) => None,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_scale_factor(entity: Entity, window: &Window) -> f32 {
    winit_window_scale_factor(entity)
        .unwrap_or_else(|| positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_origin_logical(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = window.scale_factor();
        if let Ok(pos_phys) = window.inner_position() {
            let pos_logical = pos_phys.to_logical::<f64>(scale);
            Some([pos_logical.x as f32, pos_logical.y as f32])
        } else if let Ok(pos_phys) = window.outer_position() {
            let pos_logical = pos_phys.to_logical::<f64>(scale);
            Some([pos_logical.x as f32, pos_logical.y as f32])
        } else {
            None
        }
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_decoration_offset_logical(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = window.scale_factor();
        let inner = window.inner_position().ok()?.to_logical::<f64>(scale);
        let outer = window.outer_position().ok()?.to_logical::<f64>(scale);
        Some([(inner.x - outer.x) as f32, (inner.y - outer.y) as f32])
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
    bridge: &ImguiViewportBridge,
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
pub fn window_from_snapshot(snapshot: &ImguiViewportSnapshot) -> Window {
    window_from_snapshot_with_config(snapshot, ImguiViewportWindowConfig::default())
        .expect("the default viewport window configuration is valid")
}

/// Build a secondary Bevy window after validating its presentation policy.
pub fn window_from_snapshot_with_config(
    snapshot: &ImguiViewportSnapshot,
    config: ImguiViewportWindowConfig,
) -> Result<Window, ImguiViewportWindowConfigError> {
    let config = config.validate()?;
    let scale_factor = positive_finite_or(snapshot.dpi_scale, 1.0);
    let logical_size = finite_logical_size(snapshot.size);
    let mut window = Window {
        title: format!("Dear ImGui Viewport {}", snapshot.id.raw()),
        position: WindowPosition::At(physical_pos(snapshot.pos, scale_factor)),
        resolution: WindowResolution::new(
            physical_extent(logical_size[0]),
            physical_extent(logical_size[1]),
        ),
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
    set_window_logical_size(&mut window, logical_size);
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
        &next,
    ));
    window.resolution = next.resolution;
    window.decorations = next.decorations;
    window.skip_taskbar = next.skip_taskbar;
    window.window_level = next.window_level;
    window.focused = false;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn physical_pos_for_window(pos: [f32; 2], window: &Window) -> IVec2 {
    physical_pos(pos, positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn physical_outer_pos_for_client_pos(entity: Entity, pos: [f32; 2], window: &Window) -> IVec2 {
    let pos = if let Some(offset) = winit_window_decoration_offset_logical(entity) {
        [pos[0] - offset[0], pos[1] - offset[1]]
    } else {
        pos
    };
    physical_pos(pos, window_client_scale_factor(entity, window))
}

fn physical_pos(pos: [f32; 2], scale_factor: f32) -> IVec2 {
    let pos = finite_logical_pos(pos);
    IVec2::new(
        (pos[0] * scale_factor).round() as i32,
        (pos[1] * scale_factor).round() as i32,
    )
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn logical_pos(pos: IVec2, window: &Window) -> [f32; 2] {
    logical_pos_with_scale(pos, window.scale_factor())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn logical_pos_with_scale(pos: IVec2, scale_factor: f32) -> [f32; 2] {
    let scale_factor = positive_finite_or(scale_factor, 1.0);
    [pos.x as f32 / scale_factor, pos.y as f32 / scale_factor]
}

fn physical_extent(value: f32) -> u32 {
    value.round().max(1.0) as u32
}

fn finite_logical_pos(pos: [f32; 2]) -> [f32; 2] {
    [finite_or(pos[0], 0.0), finite_or(pos[1], 0.0)]
}

fn finite_logical_size(size: [f32; 2]) -> [f32; 2] {
    [
        positive_finite_or(size[0], 1.0),
        positive_finite_or(size[1], 1.0),
    ]
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn set_window_logical_size(window: &mut Window, size: [f32; 2]) {
    let [width, height] = finite_logical_size(size);
    window.resolution.set(width, height);
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

    fn assert_despawn_remains_tracked_until_deferred_application(release: bool) {
        let viewport_id = imgui::Id::from(0x7A0);
        let main_viewport_id = imgui::Id::from(0x7A1);
        let mut world = World::new();
        let entity = world
            .spawn((Window::default(), ImguiViewportWindow { viewport_id }))
            .id();
        let mut bridge = ImguiViewportBridge::default();
        bridge.set_viewport_window(viewport_id, entity);
        let keepalive = bridge.keepalive();
        if release {
            keepalive.prepare_ecs_release(main_viewport_id);
        } else {
            bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });
        }
        world.insert_non_send(bridge);
        world.insert_resource(crate::ImguiBackendConfig::default());
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
                !world
                    .get_non_send::<ImguiViewportBridge>()
                    .unwrap()
                    .ecs_release_pending(),
                "post-deferred acknowledgement must finish release without a surviving wrapper"
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
            Some(ImguiViewportBridgeError::CallbackQueueBusy)
        );
        assert_eq!(
            bridge.drain_commands(),
            Err(ImguiViewportBridgeError::CallbackQueueBusy)
        );
        assert_eq!(
            bridge.drain_commands(),
            Err(ImguiViewportBridgeError::CallbackQueueBusy),
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
            Some(ImguiViewportBridgeError::CallbackOwnership(
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
            Some(ImguiViewportBridgeError::CallbackQueueBusy)
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
            Err(ImguiViewportCallbackInstallError::CallbackSlotOccupied {
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
            Err(ImguiViewportCallbackInstallError::BackendPlatformUserDataOccupied)
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
            Err(ImguiViewportCallbackInstallError::PlatformMonitorsOccupied)
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
            Err(ImguiViewportCallbackInstallError::BackendPlatformNameOccupied)
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
                    Err(
                        ImguiViewportCallbackInstallError::MainViewportFieldOccupied {
                            field: stringify!($field),
                        }
                    )
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
                    Some(ImguiViewportBridgeError::CallbackOwnership(
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
                    Some(ImguiViewportBridgeError::CallbackOwnership(
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
        assert_installed_slot_drift!(Platform_UpdateWindow, foreign_platform_destroy_window);
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
                    Some(ImguiViewportBridgeError::CallbackOwnership(
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
                Some(ImguiViewportBridgeError::CallbackOwnership(expected))
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

        let secondary_viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!secondary_viewport.is_null());
        let main_viewport = context.main_viewport().as_raw_mut();
        let mut viewport_ptrs = [main_viewport, secondary_viewport];
        unsafe {
            (*secondary_viewport).ID = 0x442;
            platform_create_window_raw_callback(secondary_viewport);
            (*secondary_viewport).PlatformHandleRaw = (*secondary_viewport).PlatformHandle;
            (*secondary_viewport).PlatformWindowCreated = true;
        }
        let owned_handle = unsafe { (*secondary_viewport).PlatformHandle };
        assert!(!owned_handle.is_null());
        let foreign_platform_handle = std::ptr::dangling_mut::<u16>().cast::<c_void>();
        unsafe {
            (*secondary_viewport).PlatformHandle = foreign_platform_handle;
        }
        let _viewports_guard = unsafe {
            PlatformViewportsGuard::replace(&mut context, &mut viewport_ptrs, secondary_viewport)
        };

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
            assert!((*secondary_viewport).PlatformUserData.is_null());
            assert_eq!(
                (*secondary_viewport).PlatformHandle,
                foreign_platform_handle,
                "direct detach must preserve a foreign viewport-field replacement"
            );
            assert!((*secondary_viewport).PlatformHandleRaw.is_null());
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
            (*secondary_viewport).PlatformHandle = std::ptr::null_mut();
        }
    }

    #[test]
    fn prepare_platform_viewports_rejects_each_replaced_main_viewport_field() {
        macro_rules! assert_main_viewport_field_drift {
            ($field:ident) => {{
                let mut context = imgui::Context::create();
                let mut bridge = ImguiViewportBridge::default();
                let keepalive = bridge.keepalive();
                unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
                let primary_window =
                    Entity::from_raw_u32(1).expect("test entity index should be valid");

                prepare_platform_viewports_for_frame(
                    &mut context,
                    &mut bridge,
                    primary_window,
                    &Window::default(),
                    &[],
                    std::iter::empty(),
                    true,
                )
                .unwrap();

                let foreign = std::ptr::dangling_mut::<u16>().cast::<c_void>();
                unsafe {
                    (*context.main_viewport().as_raw_mut()).$field = foreign;
                }
                assert_eq!(
                    prepare_platform_viewports_for_frame(
                        &mut context,
                        &mut bridge,
                        primary_window,
                        &Window::default(),
                        &[],
                        std::iter::empty(),
                        true,
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
                    Some(ImguiViewportBridgeError::CallbackOwnership(
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
        let mut bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
        prepare_platform_viewports_for_frame(
            &mut context,
            &mut bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            true,
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
            Some(ImguiViewportBridgeError::CallbackOwnership(
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
        let mut bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
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
            &mut bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::once((secondary_window, live_viewport, feedback())),
            true,
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
        let mut bridge = ImguiViewportBridge::default();
        let keepalive = bridge.keepalive();
        unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");

        prepare_platform_viewports_for_frame(
            &mut context,
            &mut bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            true,
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
            clear_imgui_viewport_platform_handles(&mut context, &bridge);
            assert!(
                unsafe { (*main_viewport).PlatformHandle.is_null() },
                "cleanup must clear a hidden backend-owned PlatformHandle before dropping it"
            );
            assert!(
                unsafe { (*main_viewport).PlatformUserData.is_null() },
                "cleanup must clear a hidden backend-owned PlatformUserData before dropping it"
            );
        }

        detach_owned_bridge(&mut context, &keepalive).unwrap();
    }
}
