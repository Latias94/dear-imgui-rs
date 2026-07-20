use super::registry::{
    insert_viewport_data, remove_viewport_data, with_current_runtime, with_viewport_data,
};
use super::runtime::RuntimeControl;
use super::viewport_data::{ViewportData, decoration_offset_logical};
use super::*;
use crate::sanitize;
use dear_imgui_rs::Context;
use std::ffi::{CStr, c_char, c_void};
use std::rc::Rc;
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{WindowAttributes, WindowLevel};

pub(super) fn preflight_platform_callbacks(ctx: &Context) -> Result<(), WinitPlatformError> {
    if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
        return Err(WinitPlatformError::AggregateCallbackHooksUnavailable);
    }

    let binding = ctx.binding();
    binding.with_bound_context(|| {
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
            (
                pio.Platform_SetWindowAlpha.is_some(),
                "Platform_SetWindowAlpha",
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

pub(super) fn claim_platform_callbacks(ctx: &mut Context) {
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
            pio.set_platform_set_window_alpha_raw(Some(winit_set_window_alpha));
            pio.set_platform_render_window_raw(Some(winit_platform_render_window));
            pio.set_platform_swap_buffers_raw(Some(winit_platform_swap_buffers));
        }
    });
}

pub(super) fn setup_monitors(control: &RuntimeControl, ctx: &mut Context) {
    let Some(window) = control.main_window() else {
        return;
    };
    // Build monitor list from winit and let PlatformIo own its allocator contract.
    let monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor> = {
        let mut out = Vec::new();
        for m in window.available_monitors() {
            // Winit reports monitor geometry in physical pixels. Dear ImGui expects
            // monitor rectangles in the same coordinate space as viewport Pos/Size.
            // Our multi-viewport backend uses logical screen coordinates, so convert.
            let scale_f64 = sanitize::positive_finite_or(m.scale_factor(), 1.0);
            let scale = sanitize::positive_finite_f32_or(scale_f64 as f32, 1.0);
            let pos_logical = m.position().to_logical::<f64>(scale_f64);
            let size_logical = m.size().to_logical::<f64>(scale_f64);
            let pos = sanitize::finite_vec2_f64_to_f32([pos_logical.x, pos_logical.y])
                .unwrap_or([0.0, 0.0]);
            let size = sanitize::finite_non_negative_size(size_logical);

            let mut monitor = dear_imgui_rs::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui_rs::sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            };
            monitor.MainSize = dear_imgui_rs::sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            monitor.PlatformHandle = std::ptr::null_mut();
            out.push(monitor);
        }

        if out.is_empty() {
            // Fallback using window bounds
            let scale_f64 = sanitize::positive_finite_or(window.scale_factor(), 1.0);
            let scale = sanitize::positive_finite_f32_or(scale_f64 as f32, 1.0);
            let size_logical = window.inner_size().to_logical::<f64>(scale_f64);
            let size = sanitize::finite_non_negative_size(size_logical);
            let mut monitor = dear_imgui_rs::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
            monitor.MainSize = dear_imgui_rs::sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            out.push(monitor);
        }
        out
    };

    control
        .binding()
        .with_bound_context(|| unsafe { ctx.platform_io_mut().set_monitors(&monitors) });
}

pub(super) fn release_platform_callbacks(
    control: &RuntimeControl,
) -> Result<(), WinitPlatformError> {
    let mut replaced = None;
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

        if let Some(main_window) = control.main_window() {
            main_window.set_ime_allowed(false);
            crate::platform::clear_ime_callback_if_owned(raw, Arc::as_ptr(&main_window));
        }

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
        if !pio.clear_platform_set_window_pos_if_pointer_callback(winit_set_window_pos) {
            replaced.get_or_insert("Platform_SetWindowPos");
        }
        if !pio.clear_platform_get_window_pos_if_raw_callback(winit_get_window_pos_out) {
            replaced.get_or_insert("Platform_GetWindowPos");
        }
        if !pio.clear_platform_set_window_size_if_pointer_callback(winit_set_window_size) {
            replaced.get_or_insert("Platform_SetWindowSize");
        }
        if !pio.clear_platform_get_window_size_if_raw_callback(winit_get_window_size_out) {
            replaced.get_or_insert("Platform_GetWindowSize");
        }
        if !pio.clear_platform_get_window_framebuffer_scale_if_raw_callback(
            winit_get_window_framebuffer_scale_out,
        ) {
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
                pio.set_platform_set_window_title_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_SetWindowTitle");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_SetWindowTitle");
            }
        }
        match (*raw).Platform_SetWindowAlpha {
            Some(actual)
                if std::ptr::fn_addr_eq(
                    actual,
                    winit_set_window_alpha
                        as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, f32),
                ) =>
            {
                pio.set_platform_set_window_alpha_raw(None)
            }
            None => {
                replaced.get_or_insert("Platform_SetWindowAlpha");
            }
            Some(_) => {
                replaced.get_or_insert("Platform_SetWindowAlpha");
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

        let owned_table_is_empty = (*raw).Platform_CreateWindow.is_none()
            && (*raw).Platform_DestroyWindow.is_none()
            && (*raw).Platform_ShowWindow.is_none()
            && (*raw).Platform_SetWindowPos.is_none()
            && (*raw).Platform_GetWindowPos.is_none()
            && (*raw).Platform_SetWindowSize.is_none()
            && (*raw).Platform_GetWindowSize.is_none()
            && (*raw).Platform_GetWindowFramebufferScale.is_none()
            && (*raw).Platform_SetWindowFocus.is_none()
            && (*raw).Platform_GetWindowFocus.is_none()
            && (*raw).Platform_GetWindowMinimized.is_none()
            && (*raw).Platform_SetWindowTitle.is_none()
            && (*raw).Platform_SetWindowAlpha.is_none()
            && (*raw).Platform_UpdateWindow.is_none()
            && (*raw).Platform_RenderWindow.is_none()
            && (*raw).Platform_SwapBuffers.is_none()
            && (*raw).Platform_GetWindowDpiScale.is_none()
            && (*raw).Platform_OnChangedViewport.is_none();
        if owned_table_is_empty {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            if !io.is_null() {
                let owned_flags = (dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
                    | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
                    .bits();
                (*io).BackendFlags = ((*io).BackendFlags & !owned_flags)
                    | (control.prior_backend_flags().bits() & owned_flags);
            }
        }
    }
    match replaced {
        Some(callback) => Err(WinitPlatformError::PlatformCallbackReplaced { callback }),
        None => Ok(()),
    }
}

pub(super) fn run_callback<R>(
    name: &'static str,
    fallback: R,
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> R {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_runtime(callback)
    }));
    match result {
        Ok(Some(value)) => value,
        Ok(None) => fallback,
        Err(_) => {
            let _ = with_current_runtime(|control| {
                control.record_fault(WinitPlatformError::CallbackPanicked { callback: name });
            });
            fallback
        }
    }
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
pub(super) unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_CreateWindow", (), |control| {
        if vp.is_null() {
            return;
        }

        let Some(event_loop) = control.active_event_loop() else {
            control.record_fault(WinitPlatformError::EventLoopUnavailable);
            unsafe { (*vp).PlatformRequestClose = true };
            return;
        };

        let vp_ref = unsafe { &mut *vp };
        if super::viewport_data::viewport_data_is_owned(control, vp) {
            return;
        }
        if !vp_ref.PlatformUserData.is_null() || !vp_ref.PlatformHandle.is_null() {
            control.record_fault(WinitPlatformError::ForeignPlatformUserData);
            return;
        }

        // Handle viewport flags
        let viewport_flags = vp_ref.Flags;
        // ImGui provides screen-space *logical* coordinates for Pos/Size, even with multi-viewport.
        // Winit expects logical positions/sizes and applies DPI scaling internally.
        let mut pos_x = vp_ref.Pos.x as f64;
        let mut pos_y = vp_ref.Pos.y as f64;
        if !pos_x.is_finite() {
            pos_x = 0.0;
        }
        if !pos_y.is_finite() {
            pos_y = 0.0;
        }
        let mut size_x = vp_ref.Size.x as f64;
        let mut size_y = vp_ref.Size.y as f64;
        if !size_x.is_finite() || size_x <= 0.0 {
            size_x = 128.0;
        }
        if !size_y.is_finite() || size_y <= 0.0 {
            size_y = 128.0;
        }

        let pos_logical = LogicalPosition::new(pos_x, pos_y);
        let size_logical = LogicalSize::new(size_x, size_y);
        let mut window_attrs = WindowAttributes::default()
            .with_title("ImGui Viewport")
            .with_inner_size(size_logical)
            .with_position(pos_logical)
            .with_visible(false); // Start hidden, will be shown by show_window callback

        // Handle decorations
        if viewport_flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoDecoration != 0 {
            window_attrs = window_attrs.with_decorations(false);
        }

        // Handle always on top
        if viewport_flags & dear_imgui_rs::sys::ImGuiViewportFlags_TopMost != 0 {
            window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
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
                let cur_frame = unsafe { dear_imgui_rs::sys::igGetFrameCount() };
                let outer_target = if let Some((dx, dy)) = decoration_offset_logical(&window) {
                    LogicalPosition::new(pos_logical.x - dx, pos_logical.y - dy)
                } else {
                    pos_logical
                };
                window.set_outer_position(winit::dpi::Position::Logical(outer_target));

                let window = Arc::new(window);
                let data = ViewportData::new(Arc::clone(&window), false);
                data.ignore_window_pos_event_frame.set(cur_frame);
                data.ignore_window_size_event_frame.set(cur_frame);
                let data = match insert_viewport_data(control, vp, data) {
                    Ok(data) => data,
                    Err(error) => {
                        control.record_fault(error);
                        vp_ref.PlatformRequestClose = true;
                        return;
                    }
                };
                vp_ref.PlatformUserData = data.cast::<c_void>();
                vp_ref.PlatformHandle = Arc::as_ptr(&window).cast_mut().cast();

                // Initialize DPI/framebuffer scale immediately
                let scale = sanitize::positive_finite_f32_or(window.scale_factor() as f32, 1.0);
                vp_ref.DpiScale = scale;
                vp_ref.FramebufferScale.x = scale;
                vp_ref.FramebufferScale.y = scale;

                // Note: winit does not allow registering per-window event callbacks here.
                // The application forwards events through `WinitPlatformRuntime::handle_event`.
            }
            Err(error) => {
                control.record_fault(WinitPlatformError::WindowCreation {
                    message: error.to_string(),
                });
                vp_ref.PlatformRequestClose = true;
            }
        }
    });
}

/// Destroy a viewport window
pub(super) unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_DestroyWindow", (), |control| {
        if vp.is_null() {
            return;
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
        with_viewport_data(control, vp, |data| data.window().set_visible(true));
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
                let scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
                window
                    .inner_position()
                    .or_else(|_| window.outer_position())
                    .ok()
                    .and_then(|position| {
                        let position = position.to_logical::<f64>(scale);
                        sanitize::finite_vec2_f64_to_f32([position.x, position.y])
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

        with_viewport_data(control, vp, |data| {
            let Some([x, y]) = sanitize::finite_vec2_f32([pos.x, pos.y]) else {
                return;
            };
            let window = data.window();
            let desired_client = LogicalPosition::new(x as f64, y as f64);
            let outer_target = if let Some((dx, dy)) = decoration_offset_logical(window) {
                LogicalPosition::new(desired_client.x - dx, desired_client.y - dy)
            } else {
                desired_client
            };
            window.set_outer_position(winit::dpi::Position::Logical(outer_target));
            data.ignore_window_pos_event_frame
                .set(unsafe { dear_imgui_rs::sys::igGetFrameCount() });
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
            let size = with_viewport_data(control, vp, |data| {
                let window = data.window();
                let logical: LogicalSize<f64> = window
                    .inner_size()
                    .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                sanitize::finite_non_negative_size(logical)
            })
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

        with_viewport_data(control, vp, |data| {
            let size = sanitize::finite_vec2_f32([size.x, size.y]).unwrap_or([0.0, 0.0]);
            let logical = LogicalSize::new(size[0].max(0.0) as f64, size[1].max(0.0) as f64);
            let _ = data
                .window()
                .request_inner_size(winit::dpi::Size::Logical(logical));
            data.ignore_window_size_event_frame
                .set(unsafe { dear_imgui_rs::sys::igGetFrameCount() });
        });
    });
}

/// Set window focus
pub(super) unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("winit_set_window_focus", (), |control| {
        if vp.is_null() {
            return;
        }
        with_viewport_data(control, vp, |data| data.window().focus_window());
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
            let scale = sanitize::positive_finite_f32_or(window.scale_factor() as f32, 1.0);
            if cfg!(feature = "mv-log") && (scale - data.last_log_fb_scale.get()).abs() > 0.01 {
                mvlog(format_args!(
                    "[winit-mv] fb_scale changed id={} -> {:.2}",
                    vp_ref.ID, scale
                ));
                data.last_log_fb_scale.set(scale);
            }
            result = dear_imgui_rs::sys::ImVec2 { x: scale, y: scale };
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

/// Set window alpha (no-op for winit)
pub(super) unsafe extern "C" fn winit_set_window_alpha(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
    run_callback("Platform_SetWindowAlpha", (), |_| {});
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

/// Update window - called by ImGui for platform-specific updates
pub(super) unsafe extern "C" fn winit_update_window(_vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    run_callback("Platform_UpdateWindow", (), |_| {});
}
