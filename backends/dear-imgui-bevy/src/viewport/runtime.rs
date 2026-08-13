#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::desktop::ImguiMonitorPublication;
use super::*;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "callbacks.rs"]
mod callbacks;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[path = "camera.rs"]
mod camera;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "ecs.rs"]
mod ecs;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "frame.rs"]
mod frame;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "native_policy.rs"]
mod native_policy;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "platform.rs"]
mod platform;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "state.rs"]
mod state;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use native_policy::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use state::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "bridge.rs"]
mod bridge;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use bridge::ImguiViewportBridgeContext;

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
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use ecs::settle_pending_client_placements;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use ecs::{
    acknowledge_viewport_ecs_despawns_system, apply_viewport_commands_system,
    cleanup_secondary_viewports_when_host_is_unavailable, sync_os_viewport_lifecycle_events,
};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
use frame::clear_imgui_viewport_platform_handles;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use frame::clear_imgui_viewport_platform_handles_for_keepalive;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use frame::{NativeViewportFrameSupport, prepare_platform_viewports_for_frame};
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
    publication: ImguiMonitorPublication,
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
            record.native_policy.release();
            record.show_requested = false;
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
            record.native_policy.release();
            record.show_requested = false;
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
            record.native_policy.release();
            record.show_requested = false;
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
        publication: ImguiMonitorPublication,
    ) {
        let raw = unsafe { &(*context.platform_io().as_raw()).Monitors };
        debug_assert_eq!(raw.Size, i32::try_from(publication.values.len()).unwrap());
        debug_assert_eq!(raw.Capacity, raw.Size);
        debug_assert_eq!(
            unsafe { std::slice::from_raw_parts(raw.Data, publication.values.len()) },
            publication.values.as_slice()
        );
        self.monitor_contract
            .replace(Some(ImguiViewportMonitorContract {
                data: raw.Data,
                size: raw.Size,
                capacity: raw.Capacity,
                publication,
            }));
    }

    fn publish_monitor_publication(
        &self,
        context: &mut imgui::Context,
        publication: &ImguiMonitorPublication,
    ) -> Result<bool, ImguiViewportCallbackOwnershipError> {
        if publication.values.is_empty() {
            return Ok(false);
        }
        if !self.owns_current_monitors(context) {
            return Err(ImguiViewportCallbackOwnershipError::PlatformMonitorsReplaced);
        }
        if self
            .monitor_contract
            .borrow()
            .as_ref()
            .is_some_and(|current| current.publication.equivalent_to(publication))
        {
            return Ok(false);
        }
        // SAFETY: `owns_current_monitors` proves that the existing allocation is either empty or
        // the bridge-owned allocation. Dear ImGui owns the replacement storage after this call.
        unsafe { context.platform_io_mut().set_monitors(&publication.values) };
        self.record_monitor_contract(context, publication.clone());
        Ok(true)
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
        let actual =
            unsafe { std::slice::from_raw_parts(raw.Data, expected.publication.values.len()) };
        actual == expected.publication.values.as_slice()
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
            sync_os_viewport_lifecycle_events
                .in_set(crate::input::ImguiInputPipelineSystems::PlatformLifecycle),
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

#[cfg(test)]
#[path = "tests/viewport.rs"]
mod viewport_tests;

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
#[path = "tests/internal.rs"]
mod tests;
