use super::{
    ImguiViewportBridgeContext, ImguiViewportBridgeKeepalive, ImguiViewportHandleRef,
    ImguiViewportIdentity, ImguiViewportInstanceId, ImguiViewportPlatformHandle,
    ImguiViewportPlatformHandleState, ImguiViewportRuntimeError, desktop, geometry, native_window,
    platform_callback_ownership,
};
use bevy_ecs::prelude::Entity;
use bevy_window::Window;
use dear_imgui_rs as imgui;
use dear_imgui_rs::sys;
use std::{collections::HashSet, ffi::c_void};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PlatformViewportRequests {
    move_requested: bool,
    resize_requested: bool,
    close_requested: bool,
}

impl PlatformViewportRequests {
    pub(super) const fn from_geometry(
        reconciliation: geometry::ViewportGeometryReconciliation,
    ) -> Self {
        Self {
            move_requested: reconciliation.request_move,
            resize_requested: reconciliation.request_resize,
            close_requested: false,
        }
    }

    pub(super) const fn close_requested() -> Self {
        Self {
            move_requested: false,
            resize_requested: false,
            close_requested: true,
        }
    }

    pub(super) const fn is_empty(self) -> bool {
        !self.move_requested && !self.resize_requested && !self.close_requested
    }
}

pub(super) fn mark_platform_viewport_requests(
    context: &mut imgui::Context,
    requests: impl IntoIterator<Item = (super::ImguiViewportId, PlatformViewportRequests)>,
) {
    let binding = context.binding();
    binding.with_bound_context(|| {
        for (id, requests) in requests {
            // Dear ImGui filters hidden, inactive, and zero-sized viewports out of the public
            // list. Window events still belong to their live internal viewport.
            let viewport = unsafe { sys::igFindViewportByID(id.raw()) };
            if viewport.is_null() {
                continue;
            }
            // SAFETY: the current Context owns the viewport returned by Dear ImGui's lookup.
            let viewport = unsafe { imgui::Viewport::from_raw_mut(viewport) };
            if requests.move_requested {
                viewport.set_platform_request_move(true);
            }
            if requests.resize_requested {
                viewport.set_platform_request_resize(true);
            }
            if requests.close_requested {
                viewport.set_platform_request_close(true);
            }
        }
    });
}

pub(super) fn clear_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
) {
    clear_imgui_viewport_platform_handles_for_keepalive(context, &bridge.inner, true);
    // Host loss deliberately clears bridge-owned viewport fields. Publish that transition before
    // the next frame validates ownership, so recovery cannot mistake our cleanup for foreign
    // mutation and revoke the native viewport capabilities.
    bridge.inner.record_runtime_contract(context);
}

pub(super) fn clear_imgui_viewport_platform_handles_for_keepalive(
    context: &mut imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
    recreate_platform_windows: bool,
) {
    let state = keepalive.state.borrow();
    let owned_handles = state
        .viewports
        .values()
        .filter_map(|record| {
            let handle = match record.handle.as_ref()? {
                ImguiViewportPlatformHandleState::Active(handle)
                | ImguiViewportPlatformHandleState::Retired(handle) => handle,
            };
            Some(ImguiViewportHandleRef {
                identity: handle.identity,
                pointer: (&**handle as *const ImguiViewportPlatformHandle)
                    .cast_mut()
                    .cast::<c_void>(),
                recreate_platform_window: recreate_platform_windows,
            })
        })
        .collect::<Vec<_>>();
    drop(state);
    clear_imgui_viewport_platform_handles_for_owned_handles(context, &owned_handles);
}

fn clear_stale_imgui_viewport_platform_handles(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
    live_viewports: &HashSet<ImguiViewportInstanceId>,
) {
    let owned_handles = bridge
        .inner
        .state
        .borrow()
        .viewports
        .iter()
        .filter(|(instance_id, _)| !live_viewports.contains(instance_id))
        .filter_map(|(_, record)| {
            let ImguiViewportPlatformHandleState::Active(handle) = record.handle.as_ref()? else {
                return None;
            };
            Some(ImguiViewportHandleRef {
                identity: handle.identity,
                pointer: (&**handle as *const ImguiViewportPlatformHandle)
                    .cast_mut()
                    .cast::<c_void>(),
                recreate_platform_window: true,
            })
        })
        .collect::<Vec<_>>();
    clear_imgui_viewport_platform_handles_for_owned_handles(context, &owned_handles);
}

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

#[derive(Clone, Copy)]
pub(crate) struct NativeViewportFrameSupport {
    renderer_available: bool,
    desktop_position: native_window::DesktopPositionSupport,
}

impl NativeViewportFrameSupport {
    pub(crate) const fn new(
        renderer_available: bool,
        desktop_position: native_window::DesktopPositionSupport,
    ) -> Self {
        Self {
            renderer_available,
            desktop_position,
        }
    }

    const fn allows_native_viewports(self) -> bool {
        self.renderer_available && self.desktop_position.allows_native_viewports()
    }

    const fn can_report_hovered_viewport(self) -> bool {
        self.allows_native_viewports() && self.desktop_position.can_report_hovered_viewport()
    }
}

pub(crate) fn prepare_platform_viewports_for_frame(
    context: &mut imgui::Context,
    bridge: &ImguiViewportBridgeContext,
    primary_window: Entity,
    window: &Window,
    monitor_publication: Option<&super::desktop::ImguiMonitorPublication>,
    viewport_windows: impl Iterator<
        Item = (
            Entity,
            ImguiViewportInstanceId,
            super::ImguiViewportFeedback,
        ),
    >,
    support: NativeViewportFrameSupport,
) -> Result<(), ImguiViewportRuntimeError> {
    if let Some(error) = bridge.inner.callback_fault.get() {
        return Err(error);
    }
    platform_callback_ownership(context, &bridge.inner)
        .map_err(ImguiViewportRuntimeError::CallbackOwnership)?;

    // A create callback may run before the ECS command projection in the same application tick.
    // Keep its sidecar active until that queued create has had a chance to publish the Window.
    let mut live_instances = bridge.pending_create_instances();
    let main_viewport_identity =
        ImguiViewportIdentity::capture(context.as_raw(), context.main_viewport());
    let main_viewport_id = context.main_viewport().id();
    let main_instance_id = bridge.inner.state.borrow_mut().register_viewport(
        bridge.context_id,
        main_viewport_identity,
        main_viewport_id,
    )?;
    bridge.set_viewport_window(main_instance_id, primary_window);
    bridge.set_viewport_feedback(
        main_instance_id,
        desktop::feedback_from_window_for_entity(
            primary_window,
            window,
            bridge.viewport_feedback_for_instance(main_instance_id),
            None,
        ),
    );
    live_instances.insert(main_instance_id);

    let mut platform_requests = Vec::new();
    for (entity, instance_id, feedback) in viewport_windows {
        let Some(viewport_id) = bridge.viewport_id(instance_id) else {
            continue;
        };
        bridge.set_viewport_window(instance_id, entity);
        live_instances.insert(instance_id);
        if bridge.client_placement_is_pending(instance_id) || feedback.minimized {
            // A decorated window initially reports its outer-window origin. Until the deferred
            // client-origin placement has settled, feeding that transient decoration offset back
            // to Dear ImGui would turn it into a persistent docking-coordinate error. Minimized
            // windows may likewise expose unavailable or transient native geometry; retain their
            // latest request until a restored frame can observe an authoritative client rectangle.
            bridge.refresh_viewport_non_geometry_feedback(instance_id, feedback);
            continue;
        }
        let reconciliation = bridge.observe_viewport_feedback(instance_id, feedback);
        let requests = PlatformViewportRequests::from_geometry(reconciliation);
        if !requests.is_empty() {
            platform_requests.push((viewport_id, requests));
        }
    }

    // Reconcile geometry at the exact NewFrame boundary instead of depending on Bevy window
    // messages. Some window managers coalesce acknowledgements or emit no move event when a
    // requested position is clamped at a screen edge. Dear ImGui must still receive the native
    // client geometry as authoritative before it builds docking previews for this frame.
    mark_platform_viewport_requests(context, platform_requests);

    clear_stale_imgui_viewport_platform_handles(context, bridge, &live_instances);

    let main_viewport_handle = {
        let mut state = bridge.inner.state.borrow_mut();
        for (instance_id, record) in &mut state.viewports {
            if !live_instances.contains(instance_id) {
                record.clear_ecs_state();
            }
        }
        state.retire_stale_platform_handles(&live_instances);
        state
            .platform_handle(main_instance_id)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?
    };
    let main_viewport = context.main_viewport();
    // SAFETY: the bridge owns this stable handle and retains it for the complete viewport frame.
    unsafe {
        main_viewport.set_platform_handle(main_viewport_handle);
        main_viewport.set_platform_user_data(main_viewport_handle);
    }

    if let Some(publication) = monitor_publication {
        bridge
            .inner
            .publish_monitor_publication(context, publication)
            .map_err(ImguiViewportRuntimeError::CallbackOwnership)?;
    }

    let io = context.io_mut();
    let mut backend_flags = io.backend_flags();
    backend_flags.remove(
        imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
    );
    // A desktop capability alone is insufficient: the host monitor batch is the proof that the
    // exact native display source was resolved for this frame. Keep in-window docking available
    // when the batch is pending or failed, but never enable native top-level viewports from a
    // guessed or stale monitor list.
    let native_viewports_available =
        monitor_publication.is_some() && support.allows_native_viewports();
    if native_viewports_available {
        backend_flags |= imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS;
        if support.can_report_hovered_viewport() {
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
