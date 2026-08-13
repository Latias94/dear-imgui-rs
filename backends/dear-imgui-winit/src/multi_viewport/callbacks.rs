use super::coordinates::{
    decoration_offset, desktop_position_from_physical, framebuffer_scale_for_dpi_scale,
    outer_position_from_client, request_client_geometry, validate_viewport_position,
    validate_viewport_size, viewport_target_dpi_scale,
};
use super::native_cursor_hittest::{
    MouseCaptureTransfer, focus_and_raise_window, raise_window_without_activation,
    show_window_without_activation, transfer_mouse_capture,
};
use super::registry::{
    clear_failed_viewport, insert_viewport_data, preflight_viewport_ownership,
    record_failed_viewport, remove_viewport_data, with_current_runtime, with_viewport_data,
};
use super::runtime::RuntimeControl;
use super::viewport_data::{ViewportData, ViewportWindowPolicy};
use super::*;
use crate::sanitize;
use dear_imgui_rs::Context;
use std::ffi::{CStr, c_char, c_void};
use std::rc::Rc;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use winit::platform::windows::{WindowAttributesExtWindows, WindowExtWindows};
#[cfg(target_os = "linux")]
use winit::platform::x11::{WindowAttributesExtX11, WindowType};
use winit::window::{WindowAttributes, WindowLevel};

mod monitors;
mod window;

pub(super) use self::monitors::*;
pub(super) use self::window::*;

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

fn should_focus_on_show(policy: ViewportWindowPolicy) -> bool {
    !policy.no_focus_on_appearing
}

fn requires_client_geometry_reconciliation(
    current: ViewportWindowPolicy,
    next: ViewportWindowPolicy,
) -> bool {
    current.decorations != next.decorations
}

fn requires_async_client_geometry_reconciliation() -> bool {
    cfg!(target_os = "linux")
}

fn viewport_client_geometry(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Result<([f32; 2], [f32; 2]), WinitPlatformError> {
    let viewport = unsafe { viewport.as_ref() }.ok_or(WinitPlatformError::ContextMismatch)?;
    let position = validate_viewport_position([viewport.Pos.x, viewport.Pos.y]).ok_or(
        WinitPlatformError::InvalidViewportGeometry {
            operation: "client geometry reconciliation",
            reason: "position must be finite and representable by a native window",
        },
    )?;
    let size = validate_viewport_size([viewport.Size.x, viewport.Size.y]).ok_or(
        WinitPlatformError::InvalidViewportGeometry {
            operation: "client geometry reconciliation",
            reason: "size must be finite, positive, and representable by a native window",
        },
    )?;
    Ok((position, size))
}

fn request_platform_window_focus(
    control: &RuntimeControl,
    window: &winit::window::Window,
) -> Result<(), WinitPlatformError> {
    let window_id = window.id();
    control.request_platform_window_focus(window_id);
    if let Err(error) = focus_and_raise_window(window) {
        control.cancel_platform_window_focus(window_id);
        return Err(error);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkipTaskbarCapability {
    Inherent,
    CreateOnly,
    Dynamic,
    Unsupported,
}

fn skip_taskbar_capability() -> SkipTaskbarCapability {
    if cfg!(target_os = "macos") {
        SkipTaskbarCapability::Inherent
    } else if cfg!(target_os = "windows") {
        SkipTaskbarCapability::Dynamic
    } else if cfg!(target_os = "linux") {
        SkipTaskbarCapability::CreateOnly
    } else {
        SkipTaskbarCapability::Unsupported
    }
}

fn unsupported_viewport_flag(flag: &'static str, operation: &'static str) -> WinitPlatformError {
    WinitPlatformError::UnsupportedViewportFlag { flag, operation }
}

fn validate_policy_for_creation(
    policy: ViewportWindowPolicy,
    capability: SkipTaskbarCapability,
) -> Result<(), WinitPlatformError> {
    if policy.skip_taskbar && capability == SkipTaskbarCapability::Unsupported {
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
    capability: SkipTaskbarCapability,
) -> Result<(), WinitPlatformError> {
    if current.skip_taskbar != next.skip_taskbar
        && matches!(
            capability,
            SkipTaskbarCapability::CreateOnly | SkipTaskbarCapability::Unsupported
        )
    {
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
    record_failed_viewport(control, viewport);
}

fn sync_window_policy(
    control: &RuntimeControl,
    data: &ViewportData,
    next: ViewportWindowPolicy,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Result<(), WinitPlatformError> {
    let current = data.window_policy.get();
    let taskbar_capability = skip_taskbar_capability();
    validate_policy_transition(current, next, taskbar_capability)?;
    let client_geometry = requires_client_geometry_reconciliation(current, next)
        .then(|| viewport_client_geometry(viewport))
        .transpose()?;

    let window = data.window();
    let decoration_offset_before_policy_change = client_geometry
        .filter(|_| requires_async_client_geometry_reconciliation())
        .and_then(|_| decoration_offset(window));
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
    if let Some((position, size)) = client_geometry {
        window.set_decorations(next.decorations);
        let dpi_scale = unsafe { viewport_target_dpi_scale(viewport) };
        request_client_geometry(window, position, size, dpi_scale);
        if requires_async_client_geometry_reconciliation() {
            data.request_client_geometry_reconciliation(
                position,
                size,
                decoration_offset_before_policy_change,
            );
        } else {
            // ImGui clears request flags at the end of the current platform update pass. Queue
            // the refresh so the next frame observes the settled native client geometry.
            data.request_geometry_refresh(true, true);
            control.request_main_window_redraw();
        }
    }
    if current.top_most != next.top_most {
        window.set_window_level(if next.top_most {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });
    }
    if current.skip_taskbar != next.skip_taskbar
        && taskbar_capability == SkipTaskbarCapability::Dynamic
    {
        #[cfg(target_os = "windows")]
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
