use super::coordinates::{
    desktop_position_from_physical, monitor_from_physical, outer_position_from_client,
};
use super::native_cursor_hittest::{
    MouseCaptureTransfer, focus_and_raise_window, raise_window_without_activation,
    transfer_mouse_capture,
};
use super::registry::{
    insert_viewport_data, preflight_viewport_ownership, remove_viewport_data, with_current_runtime,
    with_viewport_data,
};
use super::runtime::RuntimeControl;
use super::viewport_data::{ViewportData, ViewportWindowPolicy};
use super::*;
use crate::sanitize;
use dear_imgui_rs::Context;
use std::ffi::{CStr, c_char, c_void};
use std::rc::Rc;
use std::sync::Arc;
use winit::dpi::PhysicalPosition;
#[cfg(target_os = "windows")]
use winit::platform::windows::{WindowAttributesExtWindows, WindowExtWindows};
#[cfg(target_os = "linux")]
use winit::platform::x11::{WindowAttributesExtX11, WindowType};
use winit::window::{WindowAttributes, WindowLevel};

// This is the callback lease owned by Winit. Do not include callbacks merely because they are
// present in ImGuiPlatformIO: unsupported callbacks remain available to another backend and are
// intentionally outside Winit's drift contract.
const PLATFORM_CALLBACK_SLOTS: [&str; 17] = [
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
    "Platform_UpdateWindow",
    "Platform_RenderWindow",
    "Platform_SwapBuffers",
    "Platform_GetWindowDpiScale",
    "Platform_OnChangedViewport",
];

#[derive(Clone, Copy)]
pub(super) struct PlatformCallbackContract {
    slots: [usize; PLATFORM_CALLBACK_SLOTS.len()],
}

impl PlatformCallbackContract {
    unsafe fn capture(raw: *const dear_imgui_rs::sys::ImGuiPlatformIO) -> Option<Self> {
        let raw = unsafe { raw.as_ref() }?;
        Some(Self {
            slots: [
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
            ],
        })
    }

    fn first_drift(self, actual: Self) -> Option<&'static str> {
        self.slots
            .into_iter()
            .zip(actual.slots)
            .zip(PLATFORM_CALLBACK_SLOTS)
            .find_map(|((expected, actual), slot)| (expected != actual).then_some(slot))
    }

    fn has_matching_installed_slot(self, actual: Self) -> bool {
        self.slots
            .into_iter()
            .zip(actual.slots)
            .any(|(expected, actual)| expected != 0 && expected == actual)
    }
}

fn supports_inactive_window_creation() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

fn supports_skip_taskbar_at_creation() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

fn supports_dynamic_skip_taskbar() -> bool {
    cfg!(target_os = "windows")
}

fn unsupported_viewport_flag(flag: &'static str, operation: &'static str) -> WinitPlatformError {
    WinitPlatformError::UnsupportedViewportFlag { flag, operation }
}

fn validate_policy_for_creation(
    policy: ViewportWindowPolicy,
    supports_skip_taskbar: bool,
) -> Result<(), WinitPlatformError> {
    if policy.skip_taskbar && !supports_skip_taskbar {
        return Err(unsupported_viewport_flag(
            "NoTaskBarIcon",
            "window creation",
        ));
    }
    Ok(())
}

fn validate_policy_transition(
    current: ViewportWindowPolicy,
    next: ViewportWindowPolicy,
    supports_dynamic_taskbar: bool,
) -> Result<(), WinitPlatformError> {
    if current.skip_taskbar != next.skip_taskbar && !supports_dynamic_taskbar {
        return Err(unsupported_viewport_flag("NoTaskBarIcon", "window update"));
    }
    Ok(())
}

pub(super) fn record_viewport_failure(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    error: WinitPlatformError,
) {
    control.record_fault(error);
    if !viewport.is_null() {
        // SAFETY: callback callers pass a live viewport from the currently bound Context.
        unsafe { (*viewport).PlatformRequestClose = true };
    }
}

fn sync_window_policy(
    data: &ViewportData,
    next: ViewportWindowPolicy,
) -> Result<(), WinitPlatformError> {
    let current = data.window_policy.get();
    validate_policy_transition(current, next, supports_dynamic_skip_taskbar())?;

    let window = data.window();
    if current.cursor_hittest != next.cursor_hittest {
        if !next.cursor_hittest {
            // A moving viewport becomes hit-test transparent so the backend can identify the
            // docking target underneath it. Keep the payload above that target without changing
            // keyboard focus or making it globally top-most.
            raise_window_without_activation(window)?;
        }
        data.set_cursor_hittest(next.cursor_hittest)?;
    }
    if current.no_focus_on_click != next.no_focus_on_click {
        data.set_no_focus_on_click(next.no_focus_on_click)?;
    }
    if current.decorations != next.decorations {
        window.set_decorations(next.decorations);
        data.request_geometry_refresh(true, true);
    }
    if current.top_most != next.top_most {
        window.set_window_level(if next.top_most {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });
    }
    #[cfg(target_os = "windows")]
    if current.skip_taskbar != next.skip_taskbar {
        window.set_skip_taskbar(next.skip_taskbar);
    }

    data.window_policy.set(next);
    Ok(())
}

pub(super) fn preflight_platform_callbacks(ctx: &Context) -> Result<(), WinitPlatformError> {
    if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
        return Err(WinitPlatformError::AggregateCallbackHooksUnavailable);
    }

    let binding = ctx.binding();
    binding.with_bound_context(|| {
        let flags = ctx.io().backend_flags();
        for (backend_flag, flag) in [
            (
                dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS,
                "PLATFORM_HAS_VIEWPORTS",
            ),
            (
                dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
                "HAS_MOUSE_HOVERED_VIEWPORT",
            ),
        ] {
            if crate::platform::WINIT_VIEWPORT_FLAGS.contains(backend_flag)
                && flags.contains(backend_flag)
            {
                return Err(WinitPlatformError::PlatformCapabilityOccupied { flag });
            }
        }

        let pio = ctx.platform_io();
        let pio = unsafe { &*pio.as_raw() };
        let occupied = [
            (pio.Platform_CreateWindow.is_some(), "Platform_CreateWindow"),
            (
                pio.Platform_DestroyWindow.is_some(),
                "Platform_DestroyWindow",
            ),
            (pio.Platform_ShowWindow.is_some(), "Platform_ShowWindow"),
            (pio.Platform_SetWindowPos.is_some(), "Platform_SetWindowPos"),
            (pio.Platform_GetWindowPos.is_some(), "Platform_GetWindowPos"),
            (
                pio.Platform_SetWindowSize.is_some(),
                "Platform_SetWindowSize",
            ),
            (
                pio.Platform_GetWindowSize.is_some(),
                "Platform_GetWindowSize",
            ),
            (
                pio.Platform_GetWindowFramebufferScale.is_some(),
                "Platform_GetWindowFramebufferScale",
            ),
            (
                pio.Platform_SetWindowFocus.is_some(),
                "Platform_SetWindowFocus",
            ),
            (
                pio.Platform_GetWindowFocus.is_some(),
                "Platform_GetWindowFocus",
            ),
            (
                pio.Platform_GetWindowMinimized.is_some(),
                "Platform_GetWindowMinimized",
            ),
            (
                pio.Platform_SetWindowTitle.is_some(),
                "Platform_SetWindowTitle",
            ),
            (pio.Platform_UpdateWindow.is_some(), "Platform_UpdateWindow"),
            (pio.Platform_RenderWindow.is_some(), "Platform_RenderWindow"),
            (pio.Platform_SwapBuffers.is_some(), "Platform_SwapBuffers"),
            (
                pio.Platform_GetWindowDpiScale.is_some(),
                "Platform_GetWindowDpiScale",
            ),
            (
                pio.Platform_OnChangedViewport.is_some(),
                "Platform_OnChangedViewport",
            ),
        ];
        occupied
            .into_iter()
            .find_map(|(occupied, callback)| {
                occupied.then_some(WinitPlatformError::PlatformCallbackOccupied { callback })
            })
            .map_or(Ok(()), Err)
    })
}

pub(super) fn claim_platform_callbacks(ctx: &mut Context) -> PlatformCallbackContract {
    let binding = ctx.binding();
    binding.with_bound_context(|| {
        let pio = ctx.platform_io_mut();

        // SAFETY: these static callbacks use the exact sys ABI, reject foreign runtime state,
        // and remain installed until `release_platform_callbacks` quiesces the runtime.
        unsafe {
            pio.set_platform_create_window_raw(Some(winit_create_window));
            pio.set_platform_destroy_window_raw(Some(winit_destroy_window));
            pio.set_platform_show_window_raw(Some(winit_show_window));
            pio.set_platform_set_window_pos_raw(Some(winit_set_window_pos));
            // Avoid direct ImVec2 return; use out-parameter shims for all ImVec2 getters.
            pio.set_platform_get_window_pos_raw(Some(winit_get_window_pos_out));
            pio.set_platform_set_window_size_raw(Some(winit_set_window_size));
            pio.set_platform_get_window_size_raw(Some(winit_get_window_size_out));
            pio.set_platform_set_window_focus_raw(Some(winit_set_window_focus));
            pio.set_platform_get_window_focus_raw(Some(winit_get_window_focus));
            pio.set_platform_get_window_minimized_raw(Some(winit_get_window_minimized));
            pio.set_platform_set_window_title_raw(Some(winit_set_window_title));
            pio.set_platform_update_window_raw(Some(winit_update_window));

            // ImGui will use FramebufferScale when available, falling back to
            // DisplayFramebufferScale otherwise. Install through the out-parameter shim to avoid
            // the struct-return callback ABI.
            pio.set_platform_get_window_framebuffer_scale_raw(Some(
                winit_get_window_framebuffer_scale_out,
            ));
            pio.set_platform_get_window_dpi_scale_raw(Some(winit_get_window_dpi_scale));
            pio.set_platform_on_changed_viewport_raw(Some(winit_on_changed_viewport));
            pio.set_platform_render_window_raw(Some(winit_platform_render_window));
            pio.set_platform_swap_buffers_raw(Some(winit_platform_swap_buffers));
        }

        // SAFETY: `pio` belongs to the currently bound live Context and remains valid here.
        unsafe { PlatformCallbackContract::capture(pio.as_raw()) }
            .expect("a live Context always exposes ImGuiPlatformIO")
    })
}

pub(super) fn preflight_platform_window_destruction(
    control: &RuntimeControl,
) -> Result<(), WinitPlatformError> {
    unsafe {
        if dear_imgui_rs::sys::igGetCurrentContext() != control.context_raw() {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
        if platform_io.is_null() {
            return Err(WinitPlatformError::ContextMismatch);
        }

        let destroy_is_owned = (*platform_io)
            .Platform_DestroyWindow
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    winit_destroy_window
                        as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
                )
            });
        if !destroy_is_owned {
            return Err(WinitPlatformError::PlatformCallbackReplaced {
                callback: "Platform_DestroyWindow",
            });
        }
        let renderer_callback = [
            (
                (*platform_io).Renderer_CreateWindow.is_some(),
                "Renderer_CreateWindow",
            ),
            (
                (*platform_io).Renderer_DestroyWindow.is_some(),
                "Renderer_DestroyWindow",
            ),
            (
                (*platform_io).Renderer_SetWindowSize.is_some(),
                "Renderer_SetWindowSize",
            ),
            (
                (*platform_io).Renderer_RenderWindow.is_some(),
                "Renderer_RenderWindow",
            ),
            (
                (*platform_io).Renderer_SwapBuffers.is_some(),
                "Renderer_SwapBuffers",
            ),
        ]
        .into_iter()
        .find_map(|(installed, field)| installed.then_some(field));
        if let Some(field) = renderer_callback {
            return Err(WinitPlatformError::RendererShutdownRequired { field });
        }
        let viewports = &(*platform_io).Viewports;
        if viewports.Size < 0
            || viewports.Capacity < viewports.Size
            || (viewports.Size > 0 && viewports.Data.is_null())
        {
            return Err(WinitPlatformError::ForeignPlatformUserData);
        }
        for index in 0..viewports.Size {
            let viewport = *viewports.Data.add(index as usize);
            if viewport.is_null() {
                return Err(WinitPlatformError::ForeignPlatformUserData);
            }
            if !(*viewport).RendererUserData.is_null() {
                return Err(WinitPlatformError::RendererShutdownRequired {
                    field: "RendererUserData",
                });
            }
        }
        preflight_viewport_ownership(control, platform_io)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct MonitorVectorState {
    data: *mut dear_imgui_rs::sys::ImGuiPlatformMonitor,
    size: i32,
    capacity: i32,
}

impl MonitorVectorState {
    unsafe fn from_platform_io(raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) -> Self {
        let monitors = unsafe { &(*raw).Monitors };
        Self {
            data: monitors.Data,
            size: monitors.Size,
            capacity: monitors.Capacity,
        }
    }

    unsafe fn install_into(self, raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) {
        let monitors = unsafe { &mut (*raw).Monitors };
        monitors.Data = self.data;
        monitors.Size = self.size;
        monitors.Capacity = self.capacity;
    }

    unsafe fn matches(self, raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) -> bool {
        let monitors = unsafe { &(*raw).Monitors };
        monitors.Data == self.data
            && monitors.Size == self.size
            && monitors.Capacity == self.capacity
    }

    unsafe fn free(self) {
        if !self.data.is_null() {
            unsafe { dear_imgui_rs::sys::igMemFree(self.data.cast()) };
        }
    }
}

pub(super) struct PreparedMonitors {
    storage: Option<MonitorVectorState>,
}

impl PreparedMonitors {
    fn allocate(
        context: &Context,
        monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ) -> Result<Self, WinitPlatformError> {
        validate_monitors(monitors)?;
        let count =
            i32::try_from(monitors.len()).map_err(|_| WinitPlatformError::MonitorCountOverflow)?;
        let byte_len = std::mem::size_of_val(monitors);
        let data = context.binding().with_bound_context(|| unsafe {
            dear_imgui_rs::sys::igMemAlloc(byte_len)
                .cast::<dear_imgui_rs::sys::ImGuiPlatformMonitor>()
        });
        if data.is_null() {
            return Err(WinitPlatformError::MonitorStorageAllocationFailed);
        }
        unsafe { data.copy_from_nonoverlapping(monitors.as_ptr(), monitors.len()) };
        Ok(Self {
            storage: Some(MonitorVectorState {
                data,
                size: count,
                capacity: count,
            }),
        })
    }

    fn take_storage(&mut self) -> MonitorVectorState {
        self.storage
            .take()
            .expect("prepared monitor storage can only be published once")
    }
}

impl Drop for PreparedMonitors {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            unsafe { storage.free() };
        }
    }
}

pub(super) struct MonitorOwnership {
    prior: MonitorVectorState,
    installed: MonitorVectorState,
}

impl MonitorOwnership {
    pub(super) unsafe fn installed_matches(
        &self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
    ) -> bool {
        unsafe { self.installed.matches(raw) }
    }

    unsafe fn installed_equals(
        &self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
        monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ) -> Result<bool, WinitPlatformError> {
        if !unsafe { self.installed.matches(raw) } {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let count = usize::try_from(self.installed.size).map_err(|_| {
            WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            }
        })?;
        if count != monitors.len() {
            return Ok(false);
        }
        if count == 0 {
            return Ok(true);
        }
        if self.installed.data.is_null() {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let installed = unsafe { std::slice::from_raw_parts(self.installed.data, count) };
        Ok(installed == monitors)
    }

    unsafe fn replace_installed(
        &mut self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
        mut prepared: PreparedMonitors,
    ) -> Result<(), WinitPlatformError> {
        if !unsafe { self.installed.matches(raw) } {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let replacement = prepared.take_storage();
        unsafe { replacement.install_into(raw) };
        let previous = std::mem::replace(&mut self.installed, replacement);
        unsafe { previous.free() };
        Ok(())
    }

    pub(super) unsafe fn restore_if_owned(self, raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) {
        if unsafe { self.installed.matches(raw) } {
            unsafe { self.prior.install_into(raw) };
            unsafe { self.installed.free() };
        } else if unsafe { self.prior.matches(raw) } {
            // An allocator-aware foreign replacement may have freed Winit's allocation before
            // reproducing the prior state (most commonly the empty vector). It is therefore not
            // safe to free the detached pointer again. A direct raw replacement can leak Winit's
            // allocation, but never turns uncertain ownership into a double free.
        } else {
            // A foreign owner replaced the vector through the allocator-aware API. That operation
            // released our installed allocation, so only the detached prior allocation remains.
            unsafe { self.prior.free() };
        }
    }

    pub(super) unsafe fn context_destroyed(self) {
        // Dear ImGui released whichever vector remained installed. The prior allocation was
        // detached from native ownership when Winit published its monitor list.
        unsafe { self.prior.free() };
    }
}

pub(super) fn prepare_monitors(
    context: &Context,
    window: &winit::window::Window,
) -> Result<PreparedMonitors, WinitPlatformError> {
    let monitors = collect_monitors(window);
    PreparedMonitors::allocate(context, &monitors)
}

fn move_primary_to_front<T: Eq>(monitors: &mut Vec<T>, primary: Option<T>) {
    let Some(primary) = primary else {
        return;
    };
    if let Some(index) = monitors.iter().position(|monitor| *monitor == primary) {
        let primary = monitors.remove(index);
        monitors.insert(0, primary);
    } else {
        monitors.insert(0, primary);
    }
}

pub(super) fn collect_monitors(
    window: &winit::window::Window,
) -> Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor> {
    let mut monitor_handles = window.available_monitors().collect::<Vec<_>>();
    monitor_handles.sort_by_key(|monitor| {
        let position = monitor.position();
        let size = monitor.size();
        (
            position.x,
            position.y,
            size.width,
            size.height,
            monitor.name(),
        )
    });
    move_primary_to_front(&mut monitor_handles, window.primary_monitor());
    let mut monitors = monitor_handles
        .into_iter()
        .map(|monitor| {
            monitor_from_physical(monitor.position(), monitor.size(), monitor.scale_factor())
        })
        .collect::<Vec<_>>();
    if monitors.is_empty() {
        monitors.push(monitor_from_physical(
            PhysicalPosition::new(0, 0),
            window.inner_size(),
            window.scale_factor(),
        ));
    }
    monitors
}

pub(super) fn refresh_monitors(
    context: &Context,
    window: &winit::window::Window,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    let monitors = collect_monitors(window);
    refresh_published_monitors(context, &monitors, ownership)
}

fn refresh_published_monitors(
    context: &Context,
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    validate_monitors(monitors)?;
    let raw = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    if raw.is_null() {
        return Err(WinitPlatformError::ContextMismatch);
    }
    if unsafe { ownership.installed_equals(raw, monitors)? } {
        return Ok(false);
    }
    let prepared = PreparedMonitors::allocate(context, monitors)?;
    unsafe { ownership.replace_installed(raw, prepared)? };
    Ok(true)
}

#[cfg(test)]
pub(super) fn refresh_monitors_for_test(
    context: &Context,
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    refresh_published_monitors(context, monitors, ownership)
}

#[cfg(test)]
pub(super) fn prepare_monitors_for_test(
    context: &Context,
    monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
) -> Result<PreparedMonitors, WinitPlatformError> {
    PreparedMonitors::allocate(context, &monitors)
}

pub(super) fn publish_monitors(
    context: &mut Context,
    mut prepared: PreparedMonitors,
) -> MonitorOwnership {
    context.binding().with_bound_context(|| unsafe {
        let raw = context.platform_io_mut().as_raw_mut();
        let prior = MonitorVectorState::from_platform_io(raw);
        let installed = prepared.take_storage();
        installed.install_into(raw);
        MonitorOwnership { prior, installed }
    })
}

fn validate_monitors(
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
) -> Result<(), WinitPlatformError> {
    if monitors.is_empty() {
        return Err(WinitPlatformError::NoMonitors);
    }
    for (monitor, value) in monitors.iter().enumerate() {
        let values = [
            value.MainPos.x,
            value.MainPos.y,
            value.MainSize.x,
            value.MainSize.y,
            value.WorkPos.x,
            value.WorkPos.y,
            value.WorkSize.x,
            value.WorkSize.y,
            value.DpiScale,
        ];
        if !values.iter().all(|value| value.is_finite()) {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "geometry and DPI values must be finite",
            });
        }
        if value.MainSize.x <= 0.0 || value.MainSize.y <= 0.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "MainSize must be positive",
            });
        }
        if value.WorkSize.x < 0.0 || value.WorkSize.y < 0.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "WorkSize must not be negative",
            });
        }

        let main_max = [
            value.MainPos.x + value.MainSize.x,
            value.MainPos.y + value.MainSize.y,
        ];
        let work_max = [
            value.WorkPos.x + value.WorkSize.x,
            value.WorkPos.y + value.WorkSize.y,
        ];
        if !main_max
            .iter()
            .chain(work_max.iter())
            .all(|value| value.is_finite())
        {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "geometry bounds must not overflow",
            });
        }
        if value.WorkPos.x < value.MainPos.x
            || value.WorkPos.y < value.MainPos.y
            || work_max[0] > main_max[0]
            || work_max[1] > main_max[1]
        {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "work area must be contained within the main area",
            });
        }
        if value.DpiScale <= 0.0 || value.DpiScale >= 99.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "DpiScale must be greater than 0 and less than 99",
            });
        }
    }
    Ok(())
}

pub(super) fn release_platform_callbacks(
    control: &RuntimeControl,
) -> Result<(), WinitPlatformError> {
    let mut replaced = None;
    let mut owned_callback = false;
    unsafe {
        if dear_imgui_rs::sys::igGetCurrentContext() != control.context_raw() {
            return Err(WinitPlatformError::ContextMismatch);
        }
        let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
        if pio.is_null() {
            return Ok(());
        }
        let pio = dear_imgui_rs::platform_io::PlatformIo::from_raw_mut(pio);
        let raw = pio.as_raw_mut();

        macro_rules! clear_unary {
            ($field:ident, $expected:path, $setter:ident, $name:literal) => {
                match (*raw).$field {
                    Some(actual)
                        if std::ptr::fn_addr_eq(
                            actual,
                            $expected
                                as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
                        ) =>
                    {
                        owned_callback = true;
                        pio.$setter(None);
                    }
                    None => {
                        replaced.get_or_insert($name);
                    }
                    Some(_) => {
                        replaced.get_or_insert($name);
                    }
                }
            };
        }
        macro_rules! clear_render {
            ($field:ident, $expected:path, $setter:ident, $name:literal) => {
                match (*raw).$field {
                    Some(actual)
                        if std::ptr::fn_addr_eq(
                            actual,
                            $expected
                                as unsafe extern "C" fn(
                                    *mut dear_imgui_rs::sys::ImGuiViewport,
                                    *mut c_void,
                                ),
                        ) =>
                    {
                        owned_callback = true;
                        pio.$setter(None);
                    }
                    None => {
                        replaced.get_or_insert($name);
                    }
                    Some(_) => {
                        replaced.get_or_insert($name);
                    }
                }
            };
        }

        clear_unary!(
            Platform_CreateWindow,
            winit_create_window,
            set_platform_create_window_raw,
            "Platform_CreateWindow"
        );
        clear_unary!(
            Platform_DestroyWindow,
            winit_destroy_window,
            set_platform_destroy_window_raw,
            "Platform_DestroyWindow"
        );
        clear_unary!(
            Platform_ShowWindow,
            winit_show_window,
            set_platform_show_window_raw,
            "Platform_ShowWindow"
        );
        // Aggregate callback slots are conditionally cleared through core owner helpers below.
        if pio.clear_platform_set_window_pos_if_pointer_callback(winit_set_window_pos) {
            owned_callback = true;
        } else {
            replaced.get_or_insert("Platform_SetWindowPos");
        }
        if pio.clear_platform_get_window_pos_if_raw_callback(winit_get_window_pos_out) {
            owned_callback = true;
        } else {
            replaced.get_or_insert("Platform_GetWindowPos");
        }
        if pio.clear_platform_set_window_size_if_pointer_callback(winit_set_window_size) {
            owned_callback = true;
        } else {
            replaced.get_or_insert("Platform_SetWindowSize");
        }
        if pio.clear_platform_get_window_size_if_raw_callback(winit_get_window_size_out) {
            owned_callback = true;
        } else {
            replaced.get_or_insert("Platform_GetWindowSize");
        }
        if pio.clear_platform_get_window_framebuffer_scale_if_raw_callback(
            winit_get_window_framebuffer_scale_out,
        ) {
            owned_callback = true;
        } else {
            replaced.get_or_insert("Platform_GetWindowFramebufferScale");
        }
        clear_unary!(
            Platform_SetWindowFocus,
            winit_set_window_focus,
            set_platform_set_window_focus_raw,
            "Platform_SetWindowFocus"
        );
        match (*raw).Platform_GetWindowFocus {
            Some(actual)
                if std::ptr::fn_addr_eq(
                    actual,
                    winit_get_window_focus
                        as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport) -> bool,
                ) =>
            {
                owned_callback = true;
                pio.set_platform_get_window_focus_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_GetWindowFocus");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_GetWindowFocus");
            }
        }
        match (*raw).Platform_GetWindowMinimized {
            Some(actual)
                if std::ptr::fn_addr_eq(
                    actual,
                    winit_get_window_minimized
                        as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport) -> bool,
                ) =>
            {
                owned_callback = true;
                pio.set_platform_get_window_minimized_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_GetWindowMinimized");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_GetWindowMinimized");
            }
        }
        match (*raw).Platform_SetWindowTitle {
            Some(actual)
                if std::ptr::fn_addr_eq(
                    actual,
                    winit_set_window_title
                        as unsafe extern "C" fn(
                            *mut dear_imgui_rs::sys::ImGuiViewport,
                            *const c_char,
                        ),
                ) =>
            {
                owned_callback = true;
                pio.set_platform_set_window_title_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_SetWindowTitle");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_SetWindowTitle");
            }
        }
        clear_unary!(
            Platform_UpdateWindow,
            winit_update_window,
            set_platform_update_window_raw,
            "Platform_UpdateWindow"
        );
        clear_render!(
            Platform_RenderWindow,
            winit_platform_render_window,
            set_platform_render_window_raw,
            "Platform_RenderWindow"
        );
        clear_render!(
            Platform_SwapBuffers,
            winit_platform_swap_buffers,
            set_platform_swap_buffers_raw,
            "Platform_SwapBuffers"
        );
        match (*raw).Platform_GetWindowDpiScale {
            Some(actual)
                if std::ptr::fn_addr_eq(
                    actual,
                    winit_get_window_dpi_scale
                        as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport) -> f32,
                ) =>
            {
                owned_callback = true;
                pio.set_platform_get_window_dpi_scale_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_GetWindowDpiScale");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_GetWindowDpiScale");
            }
        }
        clear_unary!(
            Platform_OnChangedViewport,
            winit_on_changed_viewport,
            set_platform_on_changed_viewport_raw,
            "Platform_OnChangedViewport"
        );

        let base_publication_owned = control
            .platform_control()
            .is_ok_and(|platform| platform.owns_base_publication_in_current_context());
        if owned_callback || base_publication_owned {
            clear_platform_capability_flags_in_current_context(
                owned_callback && !base_publication_owned,
            );
        }
    }
    control.clear_platform_callback_contract();
    match replaced {
        Some(callback) => Err(WinitPlatformError::PlatformCallbackReplaced { callback }),
        None => Ok(()),
    }
}

fn clear_platform_capability_flags_in_current_context(clear_base: bool) {
    let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
    if !io.is_null() {
        let mut owned_flags = crate::platform::WINIT_VIEWPORT_FLAGS.bits();
        if clear_base {
            owned_flags |= dear_imgui_rs::BackendFlags::HAS_MOUSE_CURSORS.bits();
        }
        unsafe { (*io).BackendFlags &= !owned_flags };
    }
}

pub(super) fn has_owned_platform_callback_in_current_context(control: &RuntimeControl) -> bool {
    if unsafe { dear_imgui_rs::sys::igGetCurrentContext() } != control.context_raw() {
        return false;
    }
    let Some(expected) = control.platform_callback_contract() else {
        return false;
    };
    let Some(actual) =
        (unsafe { PlatformCallbackContract::capture(dear_imgui_rs::sys::igGetPlatformIO_Nil()) })
    else {
        return false;
    };
    expected.has_matching_installed_slot(actual)
}

pub(super) fn validate_platform_callback_contract(
    control: &RuntimeControl,
) -> Result<(), WinitPlatformError> {
    if unsafe { dear_imgui_rs::sys::igGetCurrentContext() } != control.context_raw() {
        return Err(WinitPlatformError::ContextMismatch);
    }
    if let Some(callback) = control.platform_callback_drift() {
        return Err(WinitPlatformError::PlatformCallbackReplaced { callback });
    }
    let expected = control
        .platform_callback_contract()
        .ok_or(WinitPlatformError::RuntimeDetached)?;
    // SAFETY: the runtime registry and current-context check prove this is the live PlatformIO
    // whose callback contract was captured during attachment.
    let actual =
        unsafe { PlatformCallbackContract::capture(dear_imgui_rs::sys::igGetPlatformIO_Nil()) }
            .ok_or(WinitPlatformError::ContextMismatch)?;
    match expected.first_drift(actual) {
        Some(callback) => {
            control.record_platform_callback_drift(callback);
            Err(WinitPlatformError::PlatformCallbackReplaced { callback })
        }
        None => Ok(()),
    }
}

enum CallbackDispatch<R> {
    Completed(R),
    Rejected,
}

pub(super) fn run_callback<R>(
    name: &'static str,
    fallback: R,
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> R {
    run_callback_with_failure(name, fallback, || {}, callback)
}

fn run_callback_with_failure<R>(
    name: &'static str,
    fallback: R,
    failure: impl FnOnce(),
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> R {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_runtime(|control| {
            let authorized_destroy =
                name == "Platform_DestroyWindow" && control.teardown_callbacks_active();
            if !authorized_destroy {
                let contract = control
                    .platform_control()
                    .and_then(|platform| platform.validate_complete_contract_in_current_context());
                if let Err(error) = contract {
                    control.record_terminal_fault(error);
                    return CallbackDispatch::Rejected;
                }
            }
            CallbackDispatch::Completed(callback(control))
        })
    }));
    match result {
        Ok(Some(CallbackDispatch::Completed(value))) => value,
        Ok(Some(CallbackDispatch::Rejected)) | Ok(None) => {
            failure();
            fallback
        }
        Err(_) => {
            let _ = with_current_runtime(|control| {
                control
                    .record_terminal_fault(WinitPlatformError::CallbackPanicked { callback: name });
            });
            failure();
            fallback
        }
    }
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
pub(super) unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback_with_failure(
        "Platform_CreateWindow",
        (),
        || {
            if !vp.is_null() {
                // SAFETY: Dear ImGui keeps the callback viewport alive for the call duration.
                unsafe { (*vp).PlatformRequestClose = true };
            }
        },
        |control| {
            if vp.is_null() {
                return;
            }

            let Some(event_loop) = control.active_event_loop() else {
                record_viewport_failure(control, vp, WinitPlatformError::EventLoopUnavailable);
                return;
            };

            let vp_ref = unsafe { &mut *vp };
            if super::viewport_data::viewport_data_is_owned(control, vp) {
                return;
            }
            // Winit's lease covers all three platform fields. It intentionally leaves
            // PlatformHandleRaw null, but a foreign value there still makes the viewport
            // unavailable and must be rejected before allocating a native window or publishing
            // either of the fields Winit does own.
            if !vp_ref.PlatformUserData.is_null()
                || !vp_ref.PlatformHandle.is_null()
                || !vp_ref.PlatformHandleRaw.is_null()
            {
                record_viewport_failure(control, vp, WinitPlatformError::ForeignPlatformUserData);
                return;
            }

            // Handle viewport flags
            let viewport_flags = vp_ref.Flags;
            let window_policy = ViewportWindowPolicy::from_flags(viewport_flags);
            if let Err(error) =
                validate_policy_for_creation(window_policy, supports_skip_taskbar_at_creation())
            {
                record_viewport_failure(control, vp, error);
                return;
            }
            // ImGui positions and sizes are in the native desktop coordinate space. The shared
            // coordinate bridge keeps that space physical on Windows/X11 and Cocoa logical on
            // macOS without applying the target window's scale to a global desktop coordinate.
            let position =
                sanitize::finite_vec2_f32([vp_ref.Pos.x, vp_ref.Pos.y]).unwrap_or([0.0, 0.0]);
            let mut size =
                sanitize::finite_vec2_f32([vp_ref.Size.x, vp_ref.Size.y]).unwrap_or([128.0, 128.0]);
            if size[0] <= 0.0 {
                size[0] = 128.0;
            }
            if size[1] <= 0.0 {
                size[1] = 128.0;
            }
            let mut window_attrs = WindowAttributes::default()
                .with_title("ImGui Viewport")
                .with_inner_size(window_size_from_desktop(size))
                .with_position(window_position_from_desktop(position))
                .with_visible(false)
                .with_decorations(window_policy.decorations);

            // Inactive creation is guaranteed only on the platforms where Winit exposes that
            // contract. Other window managers control focus themselves, but the advisory flag
            // must not block an otherwise valid viewport from being created.
            if supports_inactive_window_creation() {
                window_attrs = window_attrs.with_active(false);
            }

            if window_policy.top_most {
                window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
            }

            if window_policy.skip_taskbar {
                #[cfg(target_os = "windows")]
                {
                    window_attrs = window_attrs.with_skip_taskbar(true);
                }
                #[cfg(target_os = "linux")]
                {
                    window_attrs = window_attrs.with_x11_window_type(vec![WindowType::Utility]);
                }
            }

            match event_loop.create_window(window_attrs) {
                Ok(window) => {
                    mvlog(format_args!(
                        "[winit-mv] Platform_CreateWindow id={} size=({}, {})",
                        vp_ref.ID, vp_ref.Size.x, vp_ref.Size.y
                    ));
                    // Ensure outer position matches ImGui expectation.
                    //
                    // ImGui platform coordinates are relative to the *client* origin, while winit only lets us
                    // position by outer window coordinates. Adjust by decoration offset when available.
                    let dpi_scale = unsafe { viewport_target_dpi_scale(vp, position) };
                    let outer_target = outer_position_from_client(&window, position, dpi_scale);
                    window.set_outer_position(window_position_from_desktop(outer_target));

                    let window = Arc::new(window);
                    let data = match ViewportData::new(Arc::clone(&window), false) {
                        Ok(data) => data,
                        Err(error) => {
                            record_viewport_failure(control, vp, error);
                            return;
                        }
                    };
                    if let Err(error) = data.set_cursor_hittest(window_policy.cursor_hittest) {
                        record_viewport_failure(control, vp, error);
                        return;
                    }
                    if let Err(error) = data.set_no_focus_on_click(window_policy.no_focus_on_click)
                    {
                        record_viewport_failure(control, vp, error);
                        return;
                    }
                    if let Ok(platform) = control.platform_control() {
                        platform.apply_current_window_state(&window);
                    }
                    data.window_policy.set(window_policy);
                    let data = match insert_viewport_data(control, vp, data) {
                        Ok(data) => data,
                        Err(error) => {
                            record_viewport_failure(control, vp, error);
                            return;
                        }
                    };
                    vp_ref.PlatformUserData = data.cast::<c_void>();
                    vp_ref.PlatformHandle = Arc::as_ptr(&window).cast_mut().cast();

                    // DPI controls UI scaling while framebuffer scale converts this backend's
                    // desktop coordinate unit into render-target pixels.
                    let scale = sanitize::positive_finite_f32_or(window.scale_factor() as f32, 1.0);
                    vp_ref.DpiScale = scale;
                    let framebuffer_scale = framebuffer_scale_for_window(&window);
                    vp_ref.FramebufferScale.x = framebuffer_scale[0];
                    vp_ref.FramebufferScale.y = framebuffer_scale[1];

                    // Note: winit does not allow registering per-window event callbacks here.
                    // The application forwards events through `WinitPlatformRuntime::handle_event`.
                }
                Err(error) => {
                    record_viewport_failure(
                        control,
                        vp,
                        WinitPlatformError::WindowCreation {
                            message: error.to_string(),
                        },
                    );
                }
            }
        },
    );
}

/// Destroy a viewport window
pub(super) unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_DestroyWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        if let Some(Err(error)) = with_viewport_data(control, vp, |data| {
            if data.is_main() {
                return Ok(());
            }
            let source_window = data.window();
            let source_id = source_window.id();
            let Some(main_window) = control.main_window() else {
                return control.retire_window_input(source_id, None);
            };
            let main_id = main_window.id();
            match transfer_mouse_capture(source_window, &main_window) {
                Ok(MouseCaptureTransfer::Transferred) => {
                    control.retire_window_input(source_id, Some(main_id))
                }
                Ok(MouseCaptureTransfer::NotOwned) => control.retire_window_input(source_id, None),
                Err(error) => {
                    control.retire_window_input(source_id, None)?;
                    Err(error)
                }
            }
        }) {
            control.record_fault(error);
        }
        if !remove_viewport_data(control, vp) && unsafe { !(*vp).PlatformUserData.is_null() } {
            control.record_fault(WinitPlatformError::ForeignPlatformUserData);
        }
    });
}

/// Show a viewport window
pub(super) unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_ShowWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        let policy = ViewportWindowPolicy::from_flags(unsafe { (*vp).Flags });
        with_viewport_data(control, vp, |data| {
            if let Err(error) = sync_window_policy(data, policy) {
                record_viewport_failure(control, vp, error);
                return;
            }
            data.window().set_visible(true);
            if supports_inactive_window_creation() && !policy.no_focus_on_appearing {
                if let Err(error) = focus_and_raise_window(data.window()) {
                    record_viewport_failure(control, vp, error);
                }
            } else if !policy.cursor_hittest
                && let Err(error) = raise_window_without_activation(data.window())
            {
                record_viewport_failure(control, vp, error);
            }
        });
    });
}

/// Get window position through an out-parameter to avoid MSVC small-aggregate returns.
pub(super) unsafe extern "C" fn winit_get_window_pos_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_pos: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_get_window_pos_out", (), |control| {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = unsafe { &*vp };
            let position = with_viewport_data(control, vp, |data| {
                let window = data.window();
                window.inner_position().ok().and_then(|position| {
                    desktop_position_from_physical(position, window.scale_factor())
                })
            })
            .flatten()
            .unwrap_or([vp_ref.Pos.x, vp_ref.Pos.y]);
            r.x = position[0];
            r.y = position[1];
        }
        if !out_pos.is_null() {
            unsafe { *out_pos = r };
        }
    });
}

fn validate_viewport_position(position: [f32; 2]) -> Option<[f32; 2]> {
    let position = sanitize::finite_vec2_f32(position)?;
    let minimum = f64::from(i32::MIN);
    let maximum = f64::from(i32::MAX);
    position
        .into_iter()
        .map(f64::from)
        .all(|value| value >= minimum && value <= maximum)
        .then_some(position)
}

fn validate_viewport_size(size: [f32; 2]) -> Option<[f32; 2]> {
    let size = sanitize::finite_vec2_f32(size)?;
    let maximum = f64::from(i32::MAX);
    size.into_iter()
        .map(f64::from)
        .all(|value| value > 0.0 && value <= maximum)
        .then_some(size)
}

fn target_monitor_dpi_scale(
    viewport_dpi_scale: f32,
    position: [f32; 2],
    size: [f32; 2],
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
) -> f32 {
    let fallback = sanitize::positive_finite_f32_or(viewport_dpi_scale, 1.0);
    if monitors.is_empty() {
        return fallback;
    }

    let viewport_min = position;
    let viewport_max = [
        position[0] + size[0].max(0.0),
        position[1] + size[1].max(0.0),
    ];
    let surface_threshold = (size[0].max(0.0) * size[1].max(0.0) * 0.5).max(1.0);
    let mut best_index = 0;
    let mut best_surface = 0.001;

    for (index, monitor) in monitors.iter().enumerate() {
        let monitor_min = [monitor.MainPos.x, monitor.MainPos.y];
        let monitor_max = [
            monitor.MainPos.x + monitor.MainSize.x,
            monitor.MainPos.y + monitor.MainSize.y,
        ];
        let contains = viewport_min[0] >= monitor_min[0]
            && viewport_min[1] >= monitor_min[1]
            && viewport_max[0] <= monitor_max[0]
            && viewport_max[1] <= monitor_max[1];
        if contains {
            best_index = index;
            break;
        }

        let overlap_width =
            (viewport_max[0].min(monitor_max[0]) - viewport_min[0].max(monitor_min[0])).max(0.0);
        let overlap_height =
            (viewport_max[1].min(monitor_max[1]) - viewport_min[1].max(monitor_min[1])).max(0.0);
        let overlap_surface = overlap_width * overlap_height;
        if overlap_surface >= best_surface {
            best_surface = overlap_surface;
            best_index = index;
        }
        if best_surface >= surface_threshold {
            break;
        }
    }

    sanitize::positive_finite_f32_or(monitors[best_index].DpiScale, fallback)
}

unsafe fn viewport_target_dpi_scale(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    position: [f32; 2],
) -> f32 {
    let Some(viewport) = (unsafe { vp.as_ref() }) else {
        return 1.0;
    };
    let fallback = sanitize::positive_finite_f32_or(viewport.DpiScale, 1.0);
    let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    let Some(platform_io) = (unsafe { platform_io.as_ref() }) else {
        return fallback;
    };
    let native_monitors = &platform_io.Monitors;
    let Ok(count) = usize::try_from(native_monitors.Size) else {
        return fallback;
    };
    if native_monitors.Capacity < native_monitors.Size
        || count > 0 && native_monitors.Data.is_null()
    {
        return fallback;
    }
    let monitors = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(native_monitors.Data, count) }
    };
    target_monitor_dpi_scale(
        viewport.DpiScale,
        position,
        [viewport.Size.x, viewport.Size.y],
        monitors,
    )
}

/// Set window position
pub(super) unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    pos: *const dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_set_window_pos", (), |control| {
        if vp.is_null() || pos.is_null() {
            return;
        }
        let pos = unsafe { *pos };
        let Some(requested_position) = validate_viewport_position([pos.x, pos.y]) else {
            record_viewport_failure(
                control,
                vp,
                WinitPlatformError::InvalidViewportGeometry {
                    operation: "window positioning",
                    reason: "position must be finite and representable by a native window",
                },
            );
            return;
        };

        with_viewport_data(control, vp, |data| {
            let [x, y] = requested_position;
            let dpi_scale = unsafe { viewport_target_dpi_scale(vp, [x, y]) };
            let window = data.window();
            let desired_client = [x, y];
            let outer_target = outer_position_from_client(window, desired_client, dpi_scale);
            window.set_outer_position(window_position_from_desktop(outer_target));
            data.request_geometry_refresh(true, false);
        });
    });
}

/// Get window size through an out-parameter to avoid MSVC small-aggregate returns.
pub(super) unsafe extern "C" fn winit_get_window_size_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_size: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_get_window_size_out", (), |control| {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = unsafe { &*vp };
            let size =
                with_viewport_data(control, vp, |data| desktop_size_for_window(data.window()))
                    .unwrap_or([vp_ref.Size.x, vp_ref.Size.y]);
            r.x = size[0];
            r.y = size[1];
        }
        if !out_size.is_null() {
            unsafe { *out_size = r };
        }
    });
}

/// Set window size
pub(super) unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_set_window_size", (), |control| {
        if vp.is_null() || size.is_null() {
            return;
        }
        let size = unsafe { *size };
        let Some(size) = validate_viewport_size([size.x, size.y]) else {
            record_viewport_failure(
                control,
                vp,
                WinitPlatformError::InvalidViewportGeometry {
                    operation: "window resizing",
                    reason: "size must be finite, positive, and representable by a native window",
                },
            );
            return;
        };

        with_viewport_data(control, vp, |data| {
            let window = data.window();
            let _ = window.request_inner_size(window_size_from_desktop(size));
            data.request_geometry_refresh(false, true);
        });
    });
}

/// Set window focus
pub(super) unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("winit_set_window_focus", (), |control| {
        if vp.is_null() {
            return;
        }
        if let Some(Err(error)) =
            with_viewport_data(control, vp, |data| focus_and_raise_window(data.window()))
        {
            control.record_fault(error);
        }
    });
}

/// Get window focus
pub(super) unsafe extern "C" fn winit_get_window_focus(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    run_callback("winit_get_window_focus", false, |control| {
        if vp.is_null() {
            return false;
        }
        with_viewport_data(control, vp, |data| data.window().has_focus()).unwrap_or(false)
    })
}

/// Get window minimized state
pub(super) unsafe extern "C" fn winit_get_window_minimized(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    run_callback("Platform_GetWindowMinimized", false, |control| {
        if vp.is_null() {
            return false;
        }
        with_viewport_data(control, vp, |data| {
            data.window().is_minimized().unwrap_or(false)
        })
        .unwrap_or(false)
    })
}

/// Set window title
pub(super) unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    title: *const c_char,
) {
    run_callback("Platform_SetWindowTitle", (), |control| {
        if vp.is_null() || title.is_null() {
            return;
        }
        let title = unsafe { CStr::from_ptr(title) }.to_string_lossy();
        with_viewport_data(control, vp, |data| data.window().set_title(title.as_ref()));
    });
}

/// Get window framebuffer scale
pub(super) unsafe extern "C" fn winit_get_window_framebuffer_scale_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_scale: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("Platform_GetWindowFramebufferScale", (), |control| {
        if out_scale.is_null() {
            return;
        }

        let mut result = dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
        if vp.is_null() {
            unsafe { *out_scale = result };
            return;
        }

        let vp_ref = unsafe { &*vp };
        with_viewport_data(control, vp, |data| {
            let window = data.window();
            let scale = framebuffer_scale_for_window(window);
            if cfg!(feature = "mv-log") && (scale[0] - data.last_log_fb_scale.get()).abs() > 0.01 {
                mvlog(format_args!(
                    "[winit-mv] fb_scale changed id={} -> {:.2}",
                    vp_ref.ID, scale[0]
                ));
                data.last_log_fb_scale.set(scale[0]);
            }
            result = dear_imgui_rs::sys::ImVec2 {
                x: scale[0],
                y: scale[1],
            };
        });
        unsafe { *out_scale = result };
    })
}

/// Get window DPI scale (float)
pub(super) unsafe extern "C" fn winit_get_window_dpi_scale(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> f32 {
    run_callback("Platform_GetWindowDpiScale", 1.0, |control| {
        if vp.is_null() {
            return 1.0;
        }
        with_viewport_data(control, vp, |data| {
            sanitize::positive_finite_f32_or(data.window().scale_factor() as f32, 1.0)
        })
        .unwrap_or(1.0)
    })
}

/// Notify viewport changed.
///
/// Dear ImGui calls this when a viewport changes monitor or ownership. We use it
/// for targeted debug output to diagnose DPI/scale transitions without per-frame spam.
pub(super) unsafe extern "C" fn winit_on_changed_viewport(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Platform_OnChangedViewport", (), |_| {
        if vp.is_null() {
            return;
        }
        let vp_ref = &*vp;
        mvlog(format_args!(
            "[winit-mv] OnChangedViewport id={} pos=({:.1},{:.1}) size=({:.1},{:.1}) dpi_scale={:.2} fb_scale=({:.2},{:.2})",
            vp_ref.ID,
            vp_ref.Pos.x,
            vp_ref.Pos.y,
            vp_ref.Size.x,
            vp_ref.Size.y,
            vp_ref.DpiScale,
            vp_ref.FramebufferScale.x,
            vp_ref.FramebufferScale.y
        ));
    });
}

/// Platform render window (no-op; renderer handles rendering)
pub(super) unsafe extern "C" fn winit_platform_render_window(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Platform_RenderWindow", (), |_| {});
}

/// Platform swap buffers (no-op; renderer handles present)
pub(super) unsafe extern "C" fn winit_platform_swap_buffers(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Platform_SwapBuffers", (), |_| {});
}

/// Apply flags that can change while a viewport is alive.
pub(super) unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_UpdateWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        let policy = ViewportWindowPolicy::from_flags(unsafe { (*vp).Flags });
        with_viewport_data(control, vp, |data| {
            if let Err(error) = sync_window_policy(data, policy) {
                record_viewport_failure(control, vp, error);
            }
        });
    });
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn primary_monitor_is_first_without_duplication() {
        let mut monitors = vec![2, 1, 3];
        move_primary_to_front(&mut monitors, Some(1));
        assert_eq!(monitors, vec![1, 2, 3]);

        move_primary_to_front(&mut monitors, Some(4));
        assert_eq!(monitors, vec![4, 1, 2, 3]);

        move_primary_to_front(&mut monitors, Some(3));
        assert_eq!(monitors, vec![3, 4, 1, 2]);
    }

    #[test]
    fn viewport_geometry_rejects_values_native_windows_cannot_represent() {
        assert_eq!(
            validate_viewport_position([-1920.0, 32.0]),
            Some([-1920.0, 32.0])
        );
        assert_eq!(validate_viewport_position([f32::NAN, 32.0]), None);
        assert_eq!(validate_viewport_position([f32::MAX, 32.0]), None);

        assert_eq!(
            validate_viewport_size([1280.0, 720.0]),
            Some([1280.0, 720.0])
        );
        assert_eq!(validate_viewport_size([0.0, 720.0]), None);
        assert_eq!(validate_viewport_size([f32::INFINITY, 720.0]), None);
        assert_eq!(validate_viewport_size([f32::MAX, 720.0]), None);
    }

    #[test]
    fn focus_click_and_appearance_are_best_effort_but_taskbar_requires_capability() {
        let no_focus_on_click = ViewportWindowPolicy {
            no_focus_on_click: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(validate_policy_for_creation(no_focus_on_click, true).is_ok());

        let no_focus_on_appearing = ViewportWindowPolicy {
            no_focus_on_appearing: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(validate_policy_for_creation(no_focus_on_appearing, true).is_ok());

        let no_taskbar = ViewportWindowPolicy {
            skip_taskbar: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(matches!(
            validate_policy_for_creation(no_taskbar, false),
            Err(WinitPlatformError::UnsupportedViewportFlag {
                flag: "NoTaskBarIcon",
                ..
            })
        ));
    }

    #[test]
    fn late_focus_and_unsupported_taskbar_changes_fail_closed() {
        let current = ViewportWindowPolicy::default();
        let late_no_focus = ViewportWindowPolicy {
            no_focus_on_click: true,
            ..current
        };
        assert!(validate_policy_transition(current, late_no_focus, true).is_ok());

        let taskbar_change = ViewportWindowPolicy {
            skip_taskbar: true,
            ..current
        };
        assert!(matches!(
            validate_policy_transition(current, taskbar_change, false),
            Err(WinitPlatformError::UnsupportedViewportFlag {
                flag: "NoTaskBarIcon",
                ..
            })
        ));
    }

    #[test]
    fn window_positioning_uses_the_destination_monitor_dpi() {
        fn monitor(dpi_scale: f32) -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
            dear_imgui_rs::sys::ImGuiPlatformMonitor {
                MainPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
                MainSize: dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 },
                WorkPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
                WorkSize: dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 },
                DpiScale: dpi_scale,
                PlatformHandle: std::ptr::null_mut(),
            }
        }
        let mut monitors = [monitor(1.0), monitor(1.5)];
        monitors[0].MainSize = dear_imgui_rs::sys::ImVec2 { x: 100.0, y: 100.0 };
        monitors[1].MainPos.x = 100.0;
        monitors[1].MainSize = dear_imgui_rs::sys::ImVec2 { x: 100.0, y: 100.0 };

        assert_eq!(
            target_monitor_dpi_scale(1.0, [20.0, 20.0], [20.0, 20.0], &monitors),
            1.0
        );
        assert_eq!(
            target_monitor_dpi_scale(1.0, [120.0, 20.0], [20.0, 20.0], &monitors),
            1.5
        );
        assert_eq!(
            target_monitor_dpi_scale(1.25, [0.0, 0.0], [10.0, 10.0], &[]),
            1.25
        );
    }
}
