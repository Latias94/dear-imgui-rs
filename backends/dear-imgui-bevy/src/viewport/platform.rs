use super::*;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[cold]
pub(super) fn missing_platform_io_aggregate_hooks() -> ! {
    panic!("dear-imgui-bevy multi-viewport requires PlatformIO aggregate ABI hooks")
}

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
pub(super) fn viewport_backend_flag_mask() -> i32 {
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
pub(super) fn latch_platform_ownership_fault(
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
pub(super) fn validate_platform_contract_raw(
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

/// Validate the complete platform callback contract without latching a runtime fault or revoking
/// native viewport capabilities.
///
/// Terminal shutdown uses this before it changes either Context ownership or native window
/// mappings, so callers can repair an ownership drift and retry the same transaction.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn preflight_platform_callback_ownership(
    context: &imgui::Context,
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Result<(), ImguiViewportCallbackOwnershipError> {
    let binding = context.binding();
    binding.with_bound_context(|| {
        validate_platform_contract_raw(
            context.as_raw(),
            unsafe { sys::igGetMainViewport() },
            keepalive,
        )
        .and_then(|()| validate_hidden_callback_contract_raw(context.as_raw()))
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn platform_callback_error(
    keepalive: &ImguiViewportBridgeKeepalive,
) -> Option<ImguiViewportRuntimeError> {
    keepalive.callback_fault.get()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn platform_callback_ownership_raw(
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
pub(super) fn record_platform_runtime_contract_in_current_context(
    keepalive: &ImguiViewportBridgeKeepalive,
) {
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
pub(super) fn revoke_platform_capabilities_if_still_owned_raw(
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

/// Detach every plugin-owned secondary native window before terminal Context teardown.
///
/// Bevy normally performs this operation from its private `Last` system. Explicit Dear ImGui
/// shutdown deliberately cannot run arbitrary user schedules, so it must perform the equivalent
/// ownership transition itself while the viewport bridge still identifies its windows.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn retire_native_viewport_windows(
    world: &mut World,
) -> native_window::NativeWindowRetirements {
    let mut entities = world
        .get_non_send::<ImguiViewportBridge>()
        .map(|bridge| {
            bridge
                .contexts()
                .into_iter()
                .flat_map(|context| context.mapped_window_entities())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    entities.sort_unstable();

    for &entity in &entities {
        native_window::release_pointer_capture_for(entity);
        if let Some(mut messages) = world.get_resource_mut::<Messages<WindowClosing>>() {
            messages.write(WindowClosing { window: entity });
        }
    }

    native_window::retire_windows(entities)
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn track_viewport_ecs_despawn_for_test(
    keepalive: &ImguiViewportBridgeKeepalive,
    entity: Entity,
) {
    keepalive.track_ecs_despawn(entity);
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
