//! Dear ImGui platform-viewport bridge for Bevy-owned windows.
//!
//! PlatformIO callbacks installed here only capture intent into an engine-owned queue. Bevy systems
//! drain that queue and mutate ECS-owned [`Window`] entities outside the C ABI callback boundary.

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod callbacks;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
mod camera;
mod capability;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
mod desktop;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod ecs;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod error;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod frame;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod geometry;
mod identity;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) mod native_window;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod platform;
mod protocol;
mod window;

use bevy_app::App;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_app::{Last, PreUpdate};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_camera::{Camera, Camera2d, RenderTarget, visibility::RenderLayers};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::message::{MessageReader, Messages};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::SystemParam;
#[cfg(test)]
use bevy_math::IVec2;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_render::camera::CameraRenderGraph;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use bevy_window::Window;
#[cfg(test)]
use bevy_window::WindowLevel;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::WindowPosition;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{
    CursorOptions, ExitSystems, PrimaryWindow, WindowCloseRequested, WindowClosing, WindowOccluded,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::WinitSettings;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::cell::{Cell, RefCell};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::HashMap;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::HashSet;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ffi::{CStr, c_char, c_void};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Rc;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Weak;

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
use callbacks::platform_show_window;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use callbacks::{
    platform_create_window_raw_callback, platform_destroy_window_raw_callback,
    platform_get_window_dpi_scale_raw_callback, platform_get_window_focus_raw_callback,
    platform_get_window_framebuffer_scale_raw_callback, platform_get_window_minimized_raw_callback,
    platform_get_window_pos_raw_callback, platform_get_window_size_raw_callback,
    platform_set_window_focus_raw_callback, platform_set_window_pos_raw_callback,
    platform_set_window_size_raw_callback, platform_set_window_title_raw_callback,
    platform_show_window_raw_callback, platform_update_window_raw_callback,
};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use camera::{
    ViewportCameraReconciliation, cleanup_orphaned_viewport_cameras, ensure_viewport_camera,
    viewport_camera,
};
pub use capability::{ImguiNativeViewportStatus, ImguiNativeViewportSupport};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use desktop::{
    desktop_metrics_for_window, desktop_to_window_client_logical,
    platform_monitors_from_bevy_monitors, viewport_feedback_from_window,
    window_client_logical_to_desktop,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use desktop::{
    feedback_from_window_for_entity, finite_desktop_pos, finite_desktop_size,
    physical_outer_pos_for_client_pos, physical_pos_from_desktop, positive_finite_or,
    set_window_desktop_size, winit_window_decoration_offset_desktop,
};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
use desktop::{monitor_from_window, window_position_desktop};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use ecs::settle_pending_client_placements;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use ecs::{
    acknowledge_viewport_ecs_despawns_system, apply_viewport_commands_system,
    cleanup_secondary_viewports_when_host_is_unavailable, sync_os_viewport_lifecycle_events,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use error::ImguiViewportCallbackInstallError;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub use error::{ImguiViewportCallbackOwnershipError, ImguiViewportRuntimeError};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
use frame::clear_imgui_viewport_platform_handles;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use frame::clear_imgui_viewport_platform_handles_for_keepalive;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use frame::{NativeViewportFrameSupport, prepare_platform_viewports_for_frame};
pub(crate) use identity::ImguiViewportOwner;
pub use identity::{ImguiViewportCamera, ImguiViewportWindow};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use platform::{
    begin_owned_bridge_release, finish_owned_bridge_release, finish_viewport_ecs_release,
    install_owned_platform_callbacks, platform_callback_error, platform_callback_ownership,
    platform_capabilities_still_owned, preflight_owned_platform_callbacks,
    preflight_platform_callback_ownership, record_owned_platform_name,
    retire_native_viewport_windows, viewport_ecs_release_pending,
};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use platform::{detach_owned_bridge, track_viewport_ecs_despawn_for_test};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use platform::{
    latch_platform_ownership_fault, missing_platform_io_aggregate_hooks,
    platform_callback_ownership_raw, record_platform_runtime_contract_in_current_context,
    revoke_platform_capabilities_if_still_owned_raw, validate_platform_contract_raw,
    viewport_backend_flag_mask,
};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(crate) use protocol::ImguiViewportSnapshot;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use protocol::{ImguiViewportCommand, ImguiViewportFeedback};
pub use protocol::{ImguiViewportId, ImguiViewportInstanceId};
#[cfg(test)]
pub(crate) use window::window_from_snapshot;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use window::window_from_snapshot_with_config;
pub use window::{ImguiViewportWindowConfig, ImguiViewportWindowConfigError};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use window::{
    apply_snapshot_to_window, apply_viewport_flags_to_cursor_options,
    apply_viewport_flags_to_window, feedback_from_snapshot,
};

#[cfg(test)]
#[path = "viewport/tests/viewport.rs"]
mod viewport_tests;

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
        for record in state.viewports.values_mut() {
            record.clear_ecs_state();
        }
        state.commands.clear();
    }

    fn clear_viewport_state_preserving_pending_despawns(&self) {
        self.clear_viewport_state_preserving_native_handles();
        let mut state = self.state.borrow_mut();
        state.viewports.clear();
        state.instances_by_id.clear();
        state.instances_by_native.clear();
    }

    fn clear_viewport_state(&self) {
        self.clear_viewport_state_preserving_pending_despawns();
        self.state.borrow_mut().pending_ecs_despawns.clear();
        self.callback_fault.set(None);
        self.ecs_release_pending.set(false);
    }

    fn prepare_ecs_release(&self, main_viewport_id: ImguiViewportId) {
        let mut state = self.state.borrow_mut();
        let main_instance = state.instance_for_id(main_viewport_id);
        if let Some(record) = main_instance.and_then(|instance_id| state.record_mut(instance_id)) {
            record.window = None;
            record.camera = None;
        }

        let mut secondary_viewports = state
            .viewports
            .iter()
            .filter(|(instance_id, record)| {
                Some(**instance_id) != main_instance
                    && (record.window.is_some() || record.camera.is_some())
            })
            .map(|(&instance_id, record)| (instance_id, record.current_id))
            .collect::<Vec<_>>();
        secondary_viewports.sort_by_key(|(_, viewport_id)| viewport_id.raw());
        state.commands.clear();
        state.commands.extend(
            secondary_viewports
                .into_iter()
                .map(|(instance_id, current_id)| QueuedImguiViewportCommand {
                    instance_id,
                    command: ImguiViewportCommand::Destroy { id: current_id },
                }),
        );
        for record in state.viewports.values_mut() {
            record.feedback = None;
            record.flags = None;
            record.pending_client_placement = None;
            record.geometry = geometry::ViewportGeometryReconciler::default();
            record.focus_next_frame = false;
            record.focus_ready = false;
        }
        drop(state);

        self.callback_fault.set(None);
        self.ecs_release_pending.set(true);
    }

    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.ecs_release_pending.get()
    }

    fn has_tracked_ecs_entities(&self) -> bool {
        let state = self.state.borrow();
        state
            .viewports
            .values()
            .any(|record| record.window.is_some() || record.camera.is_some())
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
            .viewports
            .values()
            .flat_map(|record| record.window.into_iter().chain(record.camera))
            .collect::<Vec<_>>();
        state.pending_ecs_despawns.extend(mapped);
        for record in state.viewports.values_mut() {
            record.window = None;
            record.camera = None;
            record.pending_client_placement = None;
            record.geometry = geometry::ViewportGeometryReconciler::default();
        }
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

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct ImguiViewportBridgeState {
    commands: Vec<QueuedImguiViewportCommand>,
    viewports: HashMap<ImguiViewportInstanceId, ImguiViewportRecord>,
    instances_by_id: HashMap<ImguiViewportId, ImguiViewportInstanceId>,
    instances_by_native: HashMap<ImguiViewportIdentity, ImguiViewportInstanceId>,
    pending_ecs_despawns: HashSet<Entity>,
    next_instance_generation: u64,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, PartialEq)]
struct QueuedImguiViewportCommand {
    instance_id: ImguiViewportInstanceId,
    command: ImguiViewportCommand,
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
/// Numeric viewport IDs are deliberately excluded: docking may change them in place. Native code
/// validates the retained integer address against the owning Context's complete live registry
/// before Rust creates a reference from it.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ImguiViewportIdentity {
    context_address: usize,
    address: usize,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportIdentity {
    fn capture(context: *mut sys::ImGuiContext, viewport: &imgui::Viewport) -> Self {
        Self {
            context_address: context as usize,
            address: viewport.as_raw() as usize,
        }
    }

    unsafe fn resolve(self) -> Option<*mut sys::ImGuiViewport> {
        let viewport = unsafe {
            sys::ImGuiContext_FindLiveViewportByAddress(
                self.context_address as *mut sys::ImGuiContext,
                self.address,
            )
        };
        (!viewport.is_null()).then_some(viewport)
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Debug)]
struct ImguiViewportPlatformHandle {
    instance_id: ImguiViewportInstanceId,
    identity: ImguiViewportIdentity,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Debug)]
enum ImguiViewportPlatformHandleState {
    Active(Box<ImguiViewportPlatformHandle>),
    Retired(Box<ImguiViewportPlatformHandle>),
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct ImguiViewportRecord {
    identity: ImguiViewportIdentity,
    current_id: ImguiViewportId,
    window: Option<Entity>,
    camera: Option<Entity>,
    feedback: Option<ImguiViewportFeedback>,
    flags: Option<imgui::ViewportFlags>,
    pending_client_placement: Option<PendingClientPlacement>,
    geometry: geometry::ViewportGeometryReconciler,
    handle: Option<ImguiViewportPlatformHandleState>,
    focus_next_frame: bool,
    focus_ready: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportRecord {
    fn new(identity: ImguiViewportIdentity, current_id: ImguiViewportId) -> Self {
        Self {
            identity,
            current_id,
            window: None,
            camera: None,
            feedback: None,
            flags: None,
            pending_client_placement: None,
            geometry: geometry::ViewportGeometryReconciler::default(),
            handle: None,
            focus_next_frame: false,
            focus_ready: false,
        }
    }

    fn clear_ecs_state(&mut self) {
        self.window = None;
        self.camera = None;
        self.feedback = None;
        self.flags = None;
        self.pending_client_placement = None;
        self.geometry = geometry::ViewportGeometryReconciler::default();
        self.focus_next_frame = false;
        self.focus_ready = false;
    }
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
    fn next_instance_id(
        &mut self,
        context_id: imgui::ContextId,
    ) -> Result<ImguiViewportInstanceId, ImguiViewportRuntimeError> {
        let next_generation = self
            .next_instance_generation
            .checked_add(1)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceGenerationExhausted)?;
        let generation = std::num::NonZeroU64::new(next_generation)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceGenerationExhausted)?;
        self.next_instance_generation = next_generation;
        Ok(ImguiViewportInstanceId {
            context_id,
            generation,
        })
    }

    fn remove_instance(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportRecord> {
        let record = self.viewports.remove(&instance_id)?;
        if self.instances_by_id.get(&record.current_id) == Some(&instance_id) {
            self.instances_by_id.remove(&record.current_id);
        }
        if self.instances_by_native.get(&record.identity) == Some(&instance_id) {
            self.instances_by_native.remove(&record.identity);
        }
        Some(record)
    }

    fn evict_dead_id_owner(
        &mut self,
        current_id: ImguiViewportId,
        incoming: ImguiViewportInstanceId,
    ) -> Result<(), ImguiViewportRuntimeError> {
        let Some(existing) = self.instances_by_id.get(&current_id).copied() else {
            return Ok(());
        };
        if existing == incoming {
            return Ok(());
        }
        let existing_is_live_or_claimed = self.viewports.get(&existing).is_some_and(|record| {
            matches!(
                record.handle.as_ref(),
                Some(ImguiViewportPlatformHandleState::Active(_))
            ) || unsafe { record.identity.resolve().is_some() }
        });
        if existing_is_live_or_claimed {
            return Err(ImguiViewportRuntimeError::ViewportIdCollision {
                viewport_id: current_id,
            });
        }
        if let Some(record) = self.remove_instance(existing) {
            self.pending_ecs_despawns
                .extend(record.window.into_iter().chain(record.camera));
        }
        Ok(())
    }

    fn bind_current_id(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        current_id: ImguiViewportId,
    ) -> Result<(), ImguiViewportRuntimeError> {
        self.evict_dead_id_owner(current_id, instance_id)?;
        let Some(previous_id) = self
            .viewports
            .get(&instance_id)
            .map(|record| record.current_id)
        else {
            return Err(ImguiViewportRuntimeError::ViewportInstanceUnavailable);
        };
        if previous_id == current_id {
            self.instances_by_id.insert(current_id, instance_id);
            return Ok(());
        }
        if self.instances_by_id.get(&previous_id) == Some(&instance_id) {
            self.instances_by_id.remove(&previous_id);
        }
        self.viewports
            .get_mut(&instance_id)
            .expect("the viewport record was checked above")
            .current_id = current_id;
        self.instances_by_id.insert(current_id, instance_id);
        Ok(())
    }

    fn register_viewport(
        &mut self,
        context_id: imgui::ContextId,
        identity: ImguiViewportIdentity,
        current_id: ImguiViewportId,
    ) -> Result<ImguiViewportInstanceId, ImguiViewportRuntimeError> {
        if let Some(instance_id) = self.instances_by_native.get(&identity).copied() {
            let retains_sidecar = self
                .record(instance_id)
                .is_some_and(|record| record.handle.is_some());
            if retains_sidecar {
                self.bind_current_id(instance_id, current_id)?;
                return Ok(instance_id);
            }
            if let Some(record) = self.remove_instance(instance_id) {
                self.pending_ecs_despawns
                    .extend(record.window.into_iter().chain(record.camera));
            }
        }
        let instance_id = self.next_instance_id(context_id)?;
        self.evict_dead_id_owner(current_id, instance_id)?;
        self.viewports
            .insert(instance_id, ImguiViewportRecord::new(identity, current_id));
        self.instances_by_native.insert(identity, instance_id);
        self.instances_by_id.insert(current_id, instance_id);
        Ok(instance_id)
    }

    fn queue(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        current_id: ImguiViewportId,
        command: ImguiViewportCommand,
    ) -> Result<(), ImguiViewportRuntimeError> {
        self.bind_current_id(instance_id, current_id)?;
        self.commands.push(QueuedImguiViewportCommand {
            instance_id,
            command,
        });
        Ok(())
    }

    #[cfg(test)]
    fn queue_for_test(&mut self, context_id: imgui::ContextId, command: ImguiViewportCommand) {
        let current_id = command.current_id();
        let instance_id = self.instance_for_id(current_id).unwrap_or_else(|| {
            self.register_viewport(
                context_id,
                ImguiViewportIdentity {
                    context_address: 0,
                    address: current_id.raw() as usize + 1,
                },
                current_id,
            )
            .expect("a synthetic test viewport route should be registerable")
        });
        self.queue(instance_id, current_id, command)
            .expect("a synthetic test viewport command should be queueable");
    }

    fn instance_for_id(&self, viewport_id: ImguiViewportId) -> Option<ImguiViewportInstanceId> {
        self.instances_by_id.get(&viewport_id).copied()
    }

    fn record(&self, instance_id: ImguiViewportInstanceId) -> Option<&ImguiViewportRecord> {
        self.viewports.get(&instance_id)
    }

    fn record_mut(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<&mut ImguiViewportRecord> {
        self.viewports.get_mut(&instance_id)
    }

    fn platform_handle(&mut self, instance_id: ImguiViewportInstanceId) -> Option<*mut c_void> {
        let record = self.record_mut(instance_id)?;
        let handle = match record.handle.take() {
            Some(ImguiViewportPlatformHandleState::Active(handle))
            | Some(ImguiViewportPlatformHandleState::Retired(handle)) => handle,
            None => Box::new(ImguiViewportPlatformHandle {
                instance_id,
                identity: record.identity,
            }),
        };
        debug_assert_eq!(handle.instance_id, instance_id);
        debug_assert_eq!(handle.identity, record.identity);
        let pointer = (&*handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        record.handle = Some(ImguiViewportPlatformHandleState::Active(handle));
        Some(pointer)
    }

    fn take_platform_handle(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Box<ImguiViewportPlatformHandle>> {
        match self.record_mut(instance_id)?.handle.take()? {
            ImguiViewportPlatformHandleState::Active(handle)
            | ImguiViewportPlatformHandleState::Retired(handle) => Some(handle),
        }
    }

    fn retire_platform_handle(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<*mut c_void> {
        let record = self.record_mut(instance_id)?;
        let handle = match record.handle.take()? {
            ImguiViewportPlatformHandleState::Active(handle)
            | ImguiViewportPlatformHandleState::Retired(handle) => handle,
        };
        let pointer = (&*handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        record.handle = Some(ImguiViewportPlatformHandleState::Retired(handle));
        Some(pointer)
    }

    fn validate_callback_handle(
        &self,
        instance_id: ImguiViewportInstanceId,
        viewport: &imgui::Viewport,
    ) -> Result<(), ImguiViewportRuntimeError> {
        let record = self
            .record(instance_id)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
        let state = record
            .handle
            .as_ref()
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
        let (handle, active) = match state {
            ImguiViewportPlatformHandleState::Active(handle) => (handle, true),
            ImguiViewportPlatformHandleState::Retired(handle) => (handle, false),
        };
        if handle.instance_id != instance_id || handle.identity != record.identity {
            return Err(ImguiViewportRuntimeError::ViewportInstanceUnavailable);
        }
        let expected = (&**handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        let expected_claim = active.then_some(expected).unwrap_or(std::ptr::null_mut());
        for (actual, field) in [
            (viewport.platform_user_data(), "PlatformUserData"),
            (viewport.platform_handle(), "PlatformHandle"),
        ] {
            if actual != expected_claim {
                return Err(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::ViewportFieldReplaced { field },
                ));
            }
        }
        let raw = viewport.platform_handle_raw();
        if raw != std::ptr::null_mut() && raw != expected_claim {
            return Err(ImguiViewportRuntimeError::CallbackOwnership(
                ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                    field: "PlatformHandleRaw",
                },
            ));
        }
        Ok(())
    }

    fn retire_stale_platform_handles(&mut self, live_viewports: &HashSet<ImguiViewportInstanceId>) {
        for (instance_id, record) in &mut self.viewports {
            if live_viewports.contains(instance_id) {
                continue;
            }
            if let Some(ImguiViewportPlatformHandleState::Active(handle)) = record.handle.take() {
                record.handle = Some(ImguiViewportPlatformHandleState::Retired(handle));
            }
        }
    }

    fn set_viewport_flags(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        flags: imgui::ViewportFlags,
    ) -> Option<imgui::ViewportFlags> {
        self.record_mut(instance_id)?.flags.replace(flags)
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridge {
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn commands(&self) -> Vec<ImguiViewportCommand> {
        self.inner
            .state
            .borrow()
            .commands
            .iter()
            .map(|queued| queued.command.clone())
            .collect()
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn queue(&mut self, command: ImguiViewportCommand) {
        let context_id = self
            .inner
            .context_id
            .get()
            .expect("the test viewport bridge must have a Context before queueing commands");
        self.inner
            .state
            .borrow_mut()
            .queue_for_test(context_id, command);
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
        context
            .inner
            .state
            .borrow_mut()
            .queue_for_test(context_id, command);
        true
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn drain_commands(
        &mut self,
    ) -> Result<Vec<ImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self
            .inner
            .state
            .borrow_mut()
            .commands
            .drain(..)
            .map(|queued| queued.command)
            .collect())
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
        let context = self
            .context(context_id)
            .expect("the test viewport Context must remain registered");
        let instance_id = context
            .instance_for_id(viewport_id)
            .expect("the test viewport route must remain registered");
        context.set_viewport_feedback(instance_id, feedback);
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
        let context_id = self
            .inner
            .context_id
            .get()
            .expect("the test viewport bridge must have a Context before mapping windows");
        let mut state = self.inner.state.borrow_mut();
        let instance_id = state.instance_for_id(viewport_id).unwrap_or_else(|| {
            state
                .register_viewport(
                    context_id,
                    ImguiViewportIdentity {
                        context_address: 0,
                        address: viewport_id.raw() as usize + 1,
                    },
                    viewport_id,
                )
                .expect("a synthetic test viewport route should be registerable")
        });
        state
            .record_mut(instance_id)
            .expect("the synthetic viewport record should exist")
            .window = Some(entity);
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

    pub(crate) fn unregister_context(
        &self,
        context_id: imgui::ContextId,
        owner: &ImguiViewportBridgeKeepalive,
    ) {
        let mut contexts = self.contexts.borrow_mut();
        let is_current_owner = contexts
            .get(&context_id)
            .is_some_and(|registered| Rc::ptr_eq(registered, owner));
        if is_current_owner {
            contexts.remove(&context_id);
        }
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
    fn drain_commands(&self) -> Result<Vec<QueuedImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self.inner.state.borrow_mut().commands.drain(..).collect())
    }

    fn pending_create_instances(&self) -> HashSet<ImguiViewportInstanceId> {
        self.inner
            .state
            .borrow()
            .commands
            .iter()
            .filter_map(|queued| {
                matches!(&queued.command, ImguiViewportCommand::Create(_))
                    .then_some(queued.instance_id)
            })
            .collect()
    }

    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(test)]
    fn callback_error(&self) -> Option<ImguiViewportRuntimeError> {
        self.inner.callback_fault.get()
    }

    fn instance_for_id(&self, viewport_id: ImguiViewportId) -> Option<ImguiViewportInstanceId> {
        self.inner.state.borrow().instance_for_id(viewport_id)
    }

    pub(crate) fn viewport_id(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportId> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .map(|record| record.current_id)
    }

    fn set_viewport_window(&self, instance_id: ImguiViewportInstanceId, entity: Entity) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.window = Some(entity);
        }
    }

    pub(crate) fn viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        let state = self.inner.state.borrow();
        state
            .instance_for_id(viewport_id)
            .and_then(|instance_id| state.record(instance_id))
            .and_then(|record| record.window)
    }

    pub(crate) fn viewport_window_for_instance(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.window)
    }

    pub(crate) fn viewport_for_window(&self, entity: Entity) -> Option<ImguiViewportId> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .find_map(|record| (record.window == Some(entity)).then_some(record.current_id))
    }

    fn remove_viewport_window(&self, instance_id: ImguiViewportInstanceId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .record_mut(instance_id)
            .and_then(|record| record.window.take())
    }

    #[cfg(feature = "render")]
    fn set_viewport_camera(&self, instance_id: ImguiViewportInstanceId, entity: Entity) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.camera = Some(entity);
        }
    }

    #[cfg(all(test, feature = "render"))]
    fn viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        let state = self.inner.state.borrow();
        state
            .instance_for_id(viewport_id)
            .and_then(|instance_id| state.record(instance_id))
            .and_then(|record| record.camera)
    }

    #[cfg(feature = "render")]
    fn viewport_camera_for_instance(&self, instance_id: ImguiViewportInstanceId) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.camera)
    }

    #[cfg(feature = "render")]
    fn remove_viewport_camera(&self, instance_id: ImguiViewportInstanceId) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .record_mut(instance_id)
            .and_then(|record| record.camera.take())
    }

    pub(crate) fn viewport_feedback(
        &self,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportFeedback> {
        let instance_id = self.instance_for_id(viewport_id)?;
        self.viewport_feedback_for_instance(instance_id)
    }

    pub(crate) fn viewport_feedback_for_instance(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportFeedback> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.feedback)
    }

    fn set_viewport_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.feedback = Some(feedback);
        }
    }

    fn observe_viewport_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) -> geometry::ViewportGeometryReconciliation {
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return geometry::ViewportGeometryReconciliation::default();
        };
        let previous = record.feedback.unwrap_or(feedback);
        let geometry = std::mem::take(&mut record.geometry);
        let reconciliation = geometry.reconcile(previous, feedback);
        record.feedback = Some(feedback);
        reconciliation
    }

    fn record_position_request(
        &self,
        instance_id: ImguiViewportInstanceId,
        pos: [f32; 2],
        dpi_scale: f32,
    ) {
        let pos = finite_desktop_pos(pos);
        let dpi_scale = positive_finite_or(dpi_scale, 1.0);
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return;
        };
        if let Some(placement) = record.pending_client_placement.as_mut() {
            placement.pos = pos;
            placement.dpi_scale = dpi_scale;
            record.geometry.clear_position();
            if record.geometry.is_empty() {
                record.geometry = geometry::ViewportGeometryReconciler::default();
            }
            return;
        }
        record.geometry.record_position(pos, dpi_scale);
    }

    fn record_size_request(
        &self,
        instance_id: ImguiViewportInstanceId,
        size: [f32; 2],
        dpi_scale: f32,
    ) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record
                .geometry
                .record_size(finite_desktop_size(size), dpi_scale);
        }
    }

    fn remove_viewport_feedback(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.feedback = None;
            record.geometry = geometry::ViewportGeometryReconciler::default();
        }
    }

    fn client_placement_is_pending(&self, instance_id: ImguiViewportInstanceId) -> bool {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .is_some_and(|record| record.pending_client_placement.is_some())
    }

    fn remove_pending_client_placement(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.pending_client_placement = None;
        }
    }

    fn refresh_viewport_non_geometry_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) {
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return;
        };
        if let Some(cached) = record.feedback.as_mut() {
            let pos = cached.pos;
            let size = cached.size;
            *cached = ImguiViewportFeedback {
                pos,
                size,
                ..feedback
            };
        } else {
            record.feedback = Some(feedback);
        }
    }

    fn remove_viewport_flags(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.flags = None;
        }
    }

    fn show_should_focus(&self, instance_id: ImguiViewportInstanceId) -> bool {
        !self
            .inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.flags)
            .is_some_and(|flags| flags.contains(imgui::ViewportFlags::NO_FOCUS_ON_APPEARING))
    }

    fn request_focus_next_frame(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.focus_next_frame = true;
        }
    }

    fn clear_focus_request(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.focus_next_frame = false;
            record.focus_ready = false;
        }
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
            .viewports
            .values()
            .flat_map(|record| record.window.into_iter().chain(record.camera))
            .collect()
    }

    fn mapped_window_entities(&self) -> HashSet<Entity> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .filter_map(|record| record.window)
            .collect()
    }

    #[cfg(feature = "render")]
    fn mapped_camera_entities(&self) -> HashSet<Entity> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .filter_map(|record| record.camera)
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
        app.add_message::<WindowCloseRequested>();
        app.add_message::<WindowOccluded>();
        app.add_systems(
            PreUpdate,
            sync_os_viewport_lifecycle_events.before(crate::input::ImguiInputSystems),
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

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "viewport/tests/internal.rs"]
mod tests;
