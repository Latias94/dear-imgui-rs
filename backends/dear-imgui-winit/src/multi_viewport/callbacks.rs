use super::registry::{viewport_data_mut, viewport_data_ref};
use super::viewport_data::decoration_offset_logical;
use super::*;
use std::ffi::{CStr, c_char, c_void};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{WindowAttributes, WindowLevel};

pub(super) fn install_platform_callbacks(ctx: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(ctx.as_raw()) };

    // Set up platform callbacks using direct C API
    unsafe {
        let pio = ctx.platform_io_mut();
        let pio_sys = pio.as_raw_mut();

        // Install platform callbacks using raw function pointers
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

        // Also register framebuffer/DPI scale and work area insets callbacks.
        // ImGui will use FramebufferScale when available, falling back to
        // DisplayFramebufferScale otherwise. Install through the out-parameter shim to avoid the
        // struct-return callback ABI.
        pio.set_platform_get_window_framebuffer_scale_raw(Some(
            winit_get_window_framebuffer_scale_out,
        ));
        (*pio_sys).Platform_GetWindowDpiScale = Some(winit_get_window_dpi_scale);
        pio.set_platform_get_window_work_area_insets_raw(None);
        (*pio_sys).Platform_OnChangedViewport = Some(winit_on_changed_viewport);
        // Provide no-op implementations to avoid null-calls
        (*pio_sys).Platform_SetWindowAlpha = Some(winit_set_window_alpha);
        (*pio_sys).Platform_RenderWindow = Some(winit_platform_render_window);
        (*pio_sys).Platform_SwapBuffers = Some(winit_platform_swap_buffers);
        (*pio_sys).Platform_CreateVkSurface = Some(winit_platform_create_vk_surface);

        // Audit: print which callbacks are null/non-null to catch missing ones
        macro_rules! chk {
            ($name:expr, $ptr:expr) => {};
        }
        chk!("CreateWindow", (*pio_sys).Platform_CreateWindow);
        chk!("DestroyWindow", (*pio_sys).Platform_DestroyWindow);
        chk!("ShowWindow", (*pio_sys).Platform_ShowWindow);
        chk!("SetWindowPos", (*pio_sys).Platform_SetWindowPos);
        chk!("GetWindowPos", (*pio_sys).Platform_GetWindowPos);
        chk!("SetWindowSize", (*pio_sys).Platform_SetWindowSize);
        chk!("GetWindowSize", (*pio_sys).Platform_GetWindowSize);
        chk!(
            "GetWindowFramebufferScale",
            (*pio_sys).Platform_GetWindowFramebufferScale
        );
        chk!("SetWindowFocus", (*pio_sys).Platform_SetWindowFocus);
        chk!("GetWindowFocus", (*pio_sys).Platform_GetWindowFocus);
        chk!("GetWindowMinimized", (*pio_sys).Platform_GetWindowMinimized);
        chk!("SetWindowTitle", (*pio_sys).Platform_SetWindowTitle);
        chk!("SetWindowAlpha", (*pio_sys).Platform_SetWindowAlpha);
        chk!("UpdateWindow", (*pio_sys).Platform_UpdateWindow);
        chk!("RenderWindow", (*pio_sys).Platform_RenderWindow);
        chk!("SwapBuffers", (*pio_sys).Platform_SwapBuffers);
        chk!("GetWindowDpiScale", (*pio_sys).Platform_GetWindowDpiScale);
        chk!("OnChangedViewport", (*pio_sys).Platform_OnChangedViewport);
        chk!(
            "GetWindowWorkAreaInsets",
            (*pio_sys).Platform_GetWindowWorkAreaInsets
        );
        chk!("CreateVkSurface", (*pio_sys).Platform_CreateVkSurface);
    }
}

/// Set up monitors list for multi-viewport support using a reference window
pub(super) unsafe fn setup_monitors_with_window(window: &Window, ctx: &mut Context) {
    // Build monitor list from winit and feed into ImGuiPlatformIO.Monitors.
    // We allocate storage using ImGui's allocator to keep ownership consistent.
    let monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor> = {
        let mut out = Vec::new();
        let mut iter = window.available_monitors();
        while let Some(m) = iter.next() {
            // Winit reports monitor geometry in physical pixels. Dear ImGui expects
            // monitor rectangles in the same coordinate space as viewport Pos/Size.
            // Our multi-viewport backend uses logical screen coordinates, so convert.
            let scale_f64 = m.scale_factor();
            let scale = scale_f64 as f32;
            let pos_logical = m.position().to_logical::<f64>(scale_f64);
            let size_logical = m.size().to_logical::<f64>(scale_f64);

            let mut monitor = dear_imgui_rs::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui_rs::sys::ImVec2 {
                x: pos_logical.x as f32,
                y: pos_logical.y as f32,
            };
            monitor.MainSize = dear_imgui_rs::sys::ImVec2 {
                x: size_logical.width as f32,
                y: size_logical.height as f32,
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            monitor.PlatformHandle = std::ptr::null_mut();
            out.push(monitor);
        }

        if out.is_empty() {
            // Fallback using window bounds
            let scale_f64 = window.scale_factor();
            let scale = scale_f64 as f32;
            let size_logical = window.inner_size().to_logical::<f64>(scale_f64);
            let mut monitor = dear_imgui_rs::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
            monitor.MainSize = dear_imgui_rs::sys::ImVec2 {
                x: size_logical.width as f32,
                y: size_logical.height as f32,
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            out.push(monitor);
        }
        out
    };

    let pio = ctx.platform_io_mut().as_raw_mut();
    let vec = unsafe { &mut (*pio).Monitors };

    // Free existing storage if any (owned by ImGui allocator)
    if vec.Capacity > 0 && !vec.Data.is_null() {
        dear_imgui_rs::sys::igMemFree(vec.Data as *mut _);
        vec.Data = std::ptr::null_mut();
        vec.Size = 0;
        vec.Capacity = 0;
    }

    let count = monitors.len();
    let bytes = count * std::mem::size_of::<dear_imgui_rs::sys::ImGuiPlatformMonitor>();
    let data_ptr = if bytes > 0 {
        dear_imgui_rs::sys::igMemAlloc(bytes) as *mut dear_imgui_rs::sys::ImGuiPlatformMonitor
    } else {
        std::ptr::null_mut()
    };

    if !data_ptr.is_null() {
        for (i, m) in monitors.iter().enumerate() {
            *data_ptr.add(i) = *m;
        }
        vec.Data = data_ptr;
        vec.Size = count as i32;
        vec.Capacity = count as i32;
    }
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
pub(super) unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_CreateWindow", || {
        if vp.is_null() {
            return;
        }

        // Get event loop reference
        let event_loop = EVENT_LOOP.with(|el| el.borrow().map(|ptr| unsafe { &*ptr }));

        let event_loop = match event_loop {
            Some(el) => el,
            None => return,
        };

        let vp_ref = unsafe { &mut *vp };
        let existing = vp_ref.PlatformUserData as *mut ViewportData;
        if is_winit_viewport_data(existing) {
            return;
        }
        if !existing.is_null() {
            panic!("viewport PlatformUserData is already owned by another platform backend");
        }

        // Create ViewportData
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        register_viewport_data(vd);
        vp_ref.PlatformUserData = vd as *mut c_void;

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
        if viewport_flags & (dear_imgui_rs::sys::ImGuiViewportFlags_NoDecoration as i32) != 0 {
            window_attrs = window_attrs.with_decorations(false);
        }

        // Handle always on top
        if viewport_flags & (dear_imgui_rs::sys::ImGuiViewportFlags_TopMost as i32) != 0 {
            window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
        }

        // Create the window
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

                let window_ptr = Box::into_raw(Box::new(window));
                unsafe {
                    (*vd).window = window_ptr;
                    (*vd).window_owned = true;
                    (*vd).ignore_window_pos_event_frame = cur_frame;
                    (*vd).ignore_window_size_event_frame = cur_frame;
                }
                vp_ref.PlatformHandle = window_ptr as *mut c_void;

                // Initialize DPI/framebuffer scale immediately
                let window_ref: &Window = unsafe { &*window_ptr };
                let scale = window_ref.scale_factor() as f32;
                vp_ref.DpiScale = scale;
                vp_ref.FramebufferScale.x = scale;
                vp_ref.FramebufferScale.y = scale;

                // Note: winit does not allow registering per-window event callbacks here.
                // The application must forward `Event::WindowEvent` to
                // `handle_event_with_multi_viewport` (or `route_event_to_viewports`)
                // so secondary viewport windows receive input and OS move/resize notifications.
            }
            Err(_) => {
                // Clean up ViewportData on failure
                unsafe {
                    drop_viewport_data(vd);
                }
                vp_ref.PlatformUserData = std::ptr::null_mut();
            }
        }
    });
}

/// Destroy a viewport window
pub(super) unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_DestroyWindow", || {
        if vp.is_null() {
            return;
        }

        let vp_ref = unsafe { &mut *vp };
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if vd_ptr.is_null() {
            vp_ref.PlatformUserData = std::ptr::null_mut();
            vp_ref.PlatformHandle = std::ptr::null_mut();
            return;
        }
        if !is_winit_viewport_data(vd_ptr) {
            return;
        }

        unsafe {
            if (*vd_ptr).window_owned && !(*vd_ptr).window.is_null() {
                // Clean up the window.
                let _ = Box::from_raw((*vd_ptr).window);
            }
            (*vd_ptr).window = std::ptr::null_mut();

            // Drop the allocation backing `ViewportData`. Avoid creating an `&mut ViewportData`
            // reference and then freeing it while that reference is still live.
            drop_viewport_data(vd_ptr);
        }
        vp_ref.PlatformUserData = std::ptr::null_mut();
        vp_ref.PlatformHandle = std::ptr::null_mut();
    });
}

/// Show a viewport window
pub(super) unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_ShowWindow", || {
        if vp.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        if let Some(vd) = unsafe { viewport_data_ref(vp_ref) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                window.set_visible(true);
            }
        }
    });
}

/// Get window position through an out-parameter to avoid MSVC small-aggregate returns.
pub(super) unsafe extern "C" fn winit_get_window_pos_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_pos: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_pos_out", || {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = &*vp;
            if let Some(vd) = viewport_data_ref(vp) {
                if let Some(window) = vd.window.as_ref() {
                    let scale = window.scale_factor();
                    if let Ok(pos_phys) = window.inner_position() {
                        let pos_logical = pos_phys.to_logical::<f64>(scale);
                        r.x = pos_logical.x as f32;
                        r.y = pos_logical.y as f32;
                    } else if let Ok(pos_phys) = window.outer_position() {
                        let pos_logical = pos_phys.to_logical::<f64>(scale);
                        r.x = pos_logical.x as f32;
                        r.y = pos_logical.y as f32;
                    } else {
                        r.x = vp_ref.Pos.x;
                        r.y = vp_ref.Pos.y;
                    }
                } else {
                    r.x = vp_ref.Pos.x;
                    r.y = vp_ref.Pos.y;
                }
            } else {
                r.x = vp_ref.Pos.x;
                r.y = vp_ref.Pos.y;
            }
        }
        if !out_pos.is_null() {
            *out_pos = r;
        }
    });
}

/// Set window position
pub(super) unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    pos: dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_set_window_pos", || {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = unsafe { viewport_data_mut(vp) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                // ImGui provides screen-space logical coordinates relative to client origin.
                // Convert to outer coordinates for winit by subtracting decoration offset.
                let desired_client = LogicalPosition::new(pos.x as f64, pos.y as f64);
                let outer_target = if let Some((dx, dy)) = decoration_offset_logical(window) {
                    LogicalPosition::new(desired_client.x - dx, desired_client.y - dy)
                } else {
                    desired_client
                };
                window.set_outer_position(winit::dpi::Position::Logical(outer_target));
                vd.ignore_window_pos_event_frame = unsafe { dear_imgui_rs::sys::igGetFrameCount() };
            }
        }
    });
}

/// Get window size through an out-parameter to avoid MSVC small-aggregate returns.
pub(super) unsafe extern "C" fn winit_get_window_size_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_size: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_size_out", || {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = &*vp;
            if let Some(vd) = viewport_data_ref(vp) {
                if let Some(window) = vd.window.as_ref() {
                    let size_phys = window.inner_size();
                    let size_logical: LogicalSize<f64> =
                        size_phys.to_logical(window.scale_factor());
                    r.x = size_logical.width as f32;
                    r.y = size_logical.height as f32;
                } else {
                    r.x = vp_ref.Size.x;
                    r.y = vp_ref.Size.y;
                }
            } else {
                r.x = vp_ref.Size.x;
                r.y = vp_ref.Size.y;
            }
        }
        if !out_size.is_null() {
            *out_size = r;
        }
    });
}

/// Set window size
pub(super) unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_set_window_size", || {
        if vp.is_null() {
            return;
        }

        if let Some(vd) = unsafe { viewport_data_mut(vp) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                // ImGui provides inner size in logical pixels; pass through directly.
                let logical: LogicalSize<f64> = LogicalSize::new(size.x as f64, size.y as f64);
                let _ = window.request_inner_size(winit::dpi::Size::Logical(logical));
                vd.ignore_window_size_event_frame =
                    unsafe { dear_imgui_rs::sys::igGetFrameCount() };
            }
        }
    });
}

/// Set window focus
pub(super) unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("winit_set_window_focus", || {
        if vp.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        if let Some(vd) = unsafe { viewport_data_ref(vp_ref) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                window.focus_window();
            }
        }
    });
}

/// Get window focus
pub(super) unsafe extern "C" fn winit_get_window_focus(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    abort_on_panic("winit_get_window_focus", || {
        if vp.is_null() {
            return false;
        }

        let vp_ref = unsafe { &*vp };
        // Query from actual OS window if available (main or secondary)
        if let Some(vd) = unsafe { viewport_data_ref(vp_ref) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                return window.has_focus();
            }
        }
        false
    })
}

/// Get window minimized state
pub(super) unsafe extern "C" fn winit_get_window_minimized(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    abort_on_panic("Platform_GetWindowMinimized", || {
        if vp.is_null() {
            return false;
        }

        let vp_ref = unsafe { &*vp };
        // Query from actual OS window if available (main or secondary)
        if let Some(vd) = unsafe { viewport_data_ref(vp_ref) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                return window.is_minimized().unwrap_or(false);
            }
        }
        false
    })
}

/// Set window title
pub(super) unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    title: *const c_char,
) {
    abort_on_panic("Platform_SetWindowTitle", || {
        if vp.is_null() || title.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        if let Some(vd) = unsafe { viewport_data_ref(vp_ref) } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let title = unsafe { CStr::from_ptr(title) }.to_string_lossy();
                window.set_title(title.as_ref());
            }
        }
    });
}

/// Get window framebuffer scale
pub(super) unsafe extern "C" fn winit_get_window_framebuffer_scale_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_scale: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("Platform_GetWindowFramebufferScale", || {
        if out_scale.is_null() {
            return;
        }

        let mut result = dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
        if vp.is_null() {
            unsafe { *out_scale = result };
            return;
        }

        let vp_ref = unsafe { &*vp };
        if let Some(vd) = unsafe { viewport_data_mut(vp) } {
            // Always report actual framebuffer scale for this viewport, including the main one.
            // Dear ImGui relies on this to compute correct scaling when windows move between
            // viewports. Upstream backends (GLFW/SDL) do the same.
            if vd.window.is_null() {
                unsafe { *out_scale = result };
                return;
            }
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let scale = window.scale_factor() as f32;
                if cfg!(feature = "mv-log") && (scale - vd.last_log_fb_scale).abs() > 0.01 {
                    mvlog(format_args!(
                        "[winit-mv] fb_scale changed id={} -> {:.2}",
                        vp_ref.ID, scale
                    ));
                    vd.last_log_fb_scale = scale;
                }
                result = dear_imgui_rs::sys::ImVec2 { x: scale, y: scale };
            }
        }
        unsafe { *out_scale = result };
    })
}

/// Get window DPI scale (float)
pub(super) unsafe extern "C" fn winit_get_window_dpi_scale(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> f32 {
    abort_on_panic("Platform_GetWindowDpiScale", || {
        if vp.is_null() {
            return 1.0;
        }
        let vp_ref = &mut *vp;
        let mut scale = 1.0f32;
        if let Some(vd) = viewport_data_ref(vp_ref) {
            if let Some(window) = vd.window.as_ref() {
                scale = window.scale_factor() as f32;
            }
        }
        if !scale.is_finite() || scale <= 0.0 {
            scale = 1.0;
        }
        scale
    })
}

/// Notify viewport changed.
///
/// Dear ImGui calls this when a viewport changes monitor or ownership. We use it
/// for targeted debug output to diagnose DPI/scale transitions without per-frame spam.
pub(super) unsafe extern "C" fn winit_on_changed_viewport(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    abort_on_panic("Platform_OnChangedViewport", || {
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
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
    abort_on_panic("Platform_SetWindowAlpha", || {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform render window (no-op; renderer handles rendering)
pub(super) unsafe extern "C" fn winit_platform_render_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    abort_on_panic("Platform_RenderWindow", || {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform swap buffers (no-op; renderer handles present)
pub(super) unsafe extern "C" fn winit_platform_swap_buffers(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    abort_on_panic("Platform_SwapBuffers", || {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform create Vulkan surface (not used; return failure)
pub(super) unsafe extern "C" fn winit_platform_create_vk_surface(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _vk_inst: u64,
    _vk_allocators: *const c_void,
    out_vk_surface: *mut u64,
) -> ::std::os::raw::c_int {
    abort_on_panic("Platform_CreateVkSurface", || {
        if !out_vk_surface.is_null() {
            *out_vk_surface = 0;
        }
        -1 // Not supported
    })
}

/// Update window - called by ImGui for platform-specific updates
pub(super) unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_UpdateWindow", || {
        if vp.is_null() {
            return;
        }

        // For now, this is a no-op. In GLFW implementation, this is used for
        // platform-specific window updates. Winit handles most of this automatically.
        // We might need to add specific logic here later for things like:
        // - Window state synchronization
        // - Platform-specific optimizations
        // - Event processing
    });
}
