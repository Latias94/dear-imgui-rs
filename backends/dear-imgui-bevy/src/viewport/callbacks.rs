use super::*;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_create_window_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) {
    unsafe { platform_create_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_destroy_window_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) {
    unsafe { platform_destroy_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_show_window_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) {
    unsafe { platform_show_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_update_window_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) {
    unsafe { platform_update_window(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_set_window_pos_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    pos: *const sys::ImVec2,
) {
    let Some(pos) = (unsafe { pos.as_ref() }) else {
        return;
    };
    unsafe { platform_set_window_pos(viewport.cast(), *pos) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_set_window_size_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    let Some(size) = (unsafe { size.as_ref() }) else {
        return;
    };
    unsafe { platform_set_window_size(viewport.cast(), *size) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_set_window_focus_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) {
    unsafe { platform_set_window_focus(viewport.cast()) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_set_window_title_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    title: *const std::ffi::c_char,
) {
    unsafe { platform_set_window_title(viewport.cast(), title) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_pos_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_pos: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_pos(viewport.cast(), out_pos) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_size_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_size: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_size(viewport.cast(), out_size) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_framebuffer_scale_raw_callback(
    viewport: *mut sys::ImGuiViewport,
    out_scale: *mut sys::ImVec2,
) {
    unsafe { platform_get_window_framebuffer_scale(viewport.cast(), out_scale) };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_dpi_scale_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> f32 {
    unsafe { platform_get_window_dpi_scale(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_focus_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    unsafe { platform_get_window_focus(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_get_window_minimized_raw_callback(
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    unsafe { platform_get_window_minimized(viewport.cast()) }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn with_current_bridge_mut<R>(
    f: impl FnOnce(&mut ImguiViewportBridgeState, imgui::ContextId) -> R,
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
    let context_id = shared.context_id.get()?;
    Some(f(&mut bridge, context_id))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn observe_callback_viewport(
    bridge: &mut ImguiViewportBridgeState,
    viewport: &imgui::Viewport,
) -> Result<(ImguiViewportInstanceId, ImguiViewportId), ImguiViewportRuntimeError> {
    let current_id = viewport.id();
    let identity = ImguiViewportIdentity::capture(unsafe { sys::igGetCurrentContext() }, viewport);
    let instance_id = bridge
        .instances_by_native
        .get(&identity)
        .copied()
        .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
    bridge.validate_callback_handle(instance_id, viewport)?;
    bridge.bind_current_id(instance_id, current_id)?;
    Ok((instance_id, current_id))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_create_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let identity = ImguiViewportIdentity::capture(unsafe { sys::igGetCurrentContext() }, viewport);
    let Some(result) = (unsafe {
        with_current_bridge_mut(|bridge, context_id| {
            for (occupied, field) in [
                (!viewport.platform_user_data().is_null(), "PlatformUserData"),
                (!viewport.platform_handle().is_null(), "PlatformHandle"),
                (
                    !viewport.platform_handle_raw().is_null(),
                    "PlatformHandleRaw",
                ),
            ] {
                if occupied {
                    return Err(ImguiViewportRuntimeError::CallbackOwnership(
                        ImguiViewportCallbackOwnershipError::ViewportFieldReplaced { field },
                    ));
                }
            }
            let current_id = viewport.id();
            let instance_id = bridge.register_viewport(context_id, identity, current_id)?;
            let handle = bridge
                .platform_handle(instance_id)
                .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
            let _ = bridge.set_viewport_flags(instance_id, viewport.flags());
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::Create(ImguiViewportSnapshot::from_viewport(viewport)),
            )?;
            Ok(handle)
        })
    }) else {
        return;
    };
    let handle = match result {
        Ok(handle) => handle,
        Err(error) => {
            unsafe { latch_current_viewport_runtime_fault(error) };
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
unsafe fn latch_current_viewport_runtime_fault(error: ImguiViewportRuntimeError) {
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
        match error {
            ImguiViewportRuntimeError::CallbackOwnership(error) => {
                latch_platform_ownership_fault(
                    current_context,
                    unsafe { sys::igGetMainViewport().cast_const() },
                    &shared,
                    error,
                );
            }
            error => {
                shared.record_callback_fault(error);
                revoke_platform_capabilities_if_still_owned_raw(
                    current_context,
                    unsafe { sys::igGetMainViewport().cast_const() },
                    &shared,
                );
            }
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_destroy_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let owned_by_app = viewport
        .flags()
        .contains(imgui::ViewportFlags::OWNED_BY_APP);
    let Some(result) = (unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            let owned_handle = if owned_by_app {
                bridge
                    .retire_platform_handle(instance_id)
                    .map(|pointer| (pointer, None))
            } else {
                bridge.queue(
                    instance_id,
                    current_id,
                    ImguiViewportCommand::Destroy { id: current_id },
                )?;
                bridge.take_platform_handle(instance_id).map(|handle| {
                    let pointer = (&*handle as *const ImguiViewportPlatformHandle)
                        .cast_mut()
                        .cast::<c_void>();
                    (pointer, Some(handle))
                })
            };
            if let Some(record) = bridge.record_mut(instance_id) {
                record.native_policy.release();
                record.show_requested = false;
                record.flags = None;
                record.focus_next_frame = false;
                record.focus_ready = false;
            }
            Ok(owned_handle)
        })
    }) else {
        return;
    };
    let owned_handle = match result {
        Ok(handle) => handle,
        Err(error) => {
            unsafe { latch_current_viewport_runtime_fault(error) };
            return;
        }
    };
    let Some((owned_handle_ptr, _owned_handle)) = owned_handle else {
        return;
    };
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
    if owned_by_app {
        let current_context = unsafe { sys::igGetCurrentContext() };
        let shared = VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
            registry
                .borrow()
                .get(&(current_context as usize))
                .and_then(Weak::upgrade)
        });
        if let Some(shared) = shared {
            shared.record_runtime_contract_raw(current_context);
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) unsafe extern "C" fn platform_show_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            let _ = bridge.set_viewport_flags(instance_id, viewport.flags());
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::Show { id: current_id },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_update_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            let flags = viewport.flags();
            let previous_flags = bridge.set_viewport_flags(instance_id, flags);
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::Update {
                    id: current_id,
                    previous_flags,
                    flags,
                },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn platform_set_window_pos(viewport: *mut imgui::Viewport, pos: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::SetPos {
                    id: current_id,
                    pos: [pos.x, pos.y],
                    dpi_scale: (*viewport.as_raw()).DpiScale,
                },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn platform_set_window_size(viewport: *mut imgui::Viewport, size: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::SetSize {
                    id: current_id,
                    size: [size.x, size.y],
                    dpi_scale: (*viewport.as_raw()).DpiScale,
                },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_focus(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::SetFocus { id: current_id },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
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
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, current_id) = observe_callback_viewport(bridge, viewport)?;
            bridge.queue(
                instance_id,
                current_id,
                ImguiViewportCommand::SetTitle {
                    id: current_id,
                    title,
                },
            )
        })
    };
    if let Some(Err(error)) = result {
        unsafe { latch_current_viewport_runtime_fault(error) };
    }
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
    let result = unsafe {
        with_current_bridge_mut(|bridge, _| {
            let (instance_id, _) = observe_callback_viewport(bridge, viewport)?;
            Ok(bridge
                .record(instance_id)
                .and_then(|record| record.feedback))
        })
    }?;
    match result {
        Ok(feedback) => Some(feedback),
        Err(error) => {
            unsafe { latch_current_viewport_runtime_fault(error) };
            None
        }
    }
}
