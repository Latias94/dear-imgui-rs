//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};

use dear_imgui_rs::Context;
use dear_imgui_rs::platform_io::Viewport as IoViewport;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{
    ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowLevel};

// Note: We previously experimented with external C++ debug stubs to validate MSVC ABI when
// returning aggregates by value. These are no longer used; we register our Rust implementations
// directly to PlatformIO below.

// Thread-local storage for winit multi-viewport support
thread_local! {
    static EVENT_LOOP: RefCell<Option<*const ActiveEventLoop>> = const { RefCell::new(None) };
}

// Debug logging helper (off by default). Enable by building this crate with
// `--features mv-log`.
#[allow(unused_macros)]
macro_rules! mvlog {
    ($($arg:tt)*) => {
        if cfg!(feature = "mv-log") { eprintln!($($arg)*); }
    }
}

/// Helper structure stored in the void* PlatformUserData field of each ImGuiViewport
/// to easily retrieve our backend data. Following official ImGui backend pattern.
#[repr(C)]
pub struct ViewportData {
    pub window: *mut Window, // Stored in ImGuiViewport::PlatformHandle
    pub window_owned: bool,  // Set to false for main window
    pub ignore_window_pos_event_frame: i32,
    pub ignore_window_size_event_frame: i32,
}

impl Default for ViewportData {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportData {
    pub fn new() -> Self {
        Self {
            window: std::ptr::null_mut(),
            window_owned: false,
            ignore_window_pos_event_frame: -1,
            ignore_window_size_event_frame: -1,
        }
    }
}

// Convert client-area logical coordinates to screen coordinates (physical), per-window
pub(crate) fn client_to_screen_pos(window: &Window, logical: [f64; 2]) -> Option<[f32; 2]> {
    // Cross-platform: absolute screen = client top-left (physical) + client offset (physical)
    let scale = window.scale_factor();
    let offset_px_x = (logical[0] * scale) as f32;
    let offset_px_y = (logical[1] * scale) as f32;
    let base = window
        .inner_position()
        .ok()
        .map(|p| [p.x as f32, p.y as f32])
        .or_else(|| window.outer_position().ok().map(|p| [p.x as f32, p.y as f32]));
    if let Some([bx, by]) = base {
        Some([bx + offset_px_x, by + offset_px_y])
    } else {
        Some([logical[0] as f32, logical[1] as f32])
    }
}

/// Initialize multi-viewport support following official ImGui backend pattern
pub fn init_multi_viewport_support(ctx: &mut Context, main_window: &Window) {
    // Set up platform callbacks using direct C API
    unsafe {
<<<<<<< HEAD
        let pio = ctx.platform_io_mut();
        // Install platform callbacks using raw function pointers
        pio.set_platform_create_window_raw(Some(winit_create_window));
        pio.set_platform_destroy_window_raw(Some(winit_destroy_window));
        pio.set_platform_show_window_raw(Some(winit_show_window));
        pio.set_platform_set_window_pos_raw(Some(winit_set_window_pos));
        // Avoid direct ImVec2 return on MSVC; use cimgui setters with out-params below
        pio.set_platform_get_window_pos_raw(None);
        pio.set_platform_set_window_size_raw(Some(winit_set_window_size));
        pio.set_platform_get_window_size_raw(None);
        pio.set_platform_set_window_focus_raw(Some(winit_set_window_focus));
        pio.set_platform_get_window_focus_raw(Some(winit_get_window_focus));
        pio.set_platform_get_window_minimized_raw(Some(winit_get_window_minimized));
        pio.set_platform_set_window_title_raw(Some(winit_set_window_title));
        pio.set_platform_update_window_raw(Some(winit_update_window));
=======
        let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
>>>>>>> main

        // Also register framebuffer/DPI scale and work area insets callbacks
        let pio_sys = dear_imgui::sys::igGetPlatformIO_Nil();
        // Install out-parameter getters via cimgui helpers (avoid struct-return ABI)
        dear_imgui::sys::ImGuiPlatformIO_Set_Platform_GetWindowPos(
            pio_sys,
            Some(winit_get_window_pos_out_v2),
        );
        dear_imgui::sys::ImGuiPlatformIO_Set_Platform_GetWindowSize(
            pio_sys,
            Some(winit_get_window_size_out_v2),
        );
        // Avoid returning ImVec2 by value across MSVC ABI: keep this unset
        (*pio_sys).Platform_GetWindowFramebufferScale = None;
        (*pio_sys).Platform_GetWindowDpiScale = Some(winit_get_window_dpi_scale);
        (*pio_sys).Platform_GetWindowWorkAreaInsets = None;
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

    // Set up the main viewport
    init_main_viewport(main_window);

    // Set up monitors - required for multi-viewport (after main viewport exists)
    unsafe {
        setup_monitors_with_window(main_window, ctx);
    }
}

/// Set up monitors list for multi-viewport support using a reference window
unsafe fn setup_monitors_with_window(window: &Window, _ctx: &mut Context) {
    // Build monitor list from winit and feed into ImGuiPlatformIO.Monitors.
    // We allocate storage using ImGui's allocator to keep ownership consistent.
    let monitors: Vec<dear_imgui::sys::ImGuiPlatformMonitor> = {
        let mut out = Vec::new();
        let mut iter = window.available_monitors();
        while let Some(m) = iter.next() {
            let pos = m.position();
            let size = m.size();
            let scale = m.scale_factor() as f32;

            let mut monitor = dear_imgui::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui::sys::ImVec2 {
                x: pos.x as f32,
                y: pos.y as f32,
            };
            monitor.MainSize = dear_imgui::sys::ImVec2 {
                x: size.width as f32,
                y: size.height as f32,
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            monitor.PlatformHandle = std::ptr::null_mut();
            out.push(monitor);
        }

        if out.is_empty() {
            // Fallback using window bounds
            let size = window.inner_size();
            let scale = window.scale_factor() as f32;
            let mut monitor = dear_imgui::sys::ImGuiPlatformMonitor::default();
            monitor.MainPos = dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
            monitor.MainSize = dear_imgui::sys::ImVec2 {
                x: size.width as f32,
                y: size.height as f32,
            };
            monitor.WorkPos = monitor.MainPos;
            monitor.WorkSize = monitor.MainSize;
            monitor.DpiScale = scale;
            out.push(monitor);
        }
        out
    };

    let pio = dear_imgui::sys::igGetPlatformIO_Nil();
    let vec = unsafe { &mut (*pio).Monitors };

    // Free existing storage if any (owned by ImGui allocator)
    if vec.Capacity > 0 && !vec.Data.is_null() {
        dear_imgui::sys::igMemFree(vec.Data as *mut _);
        vec.Data = std::ptr::null_mut();
        vec.Size = 0;
        vec.Capacity = 0;
    }

    let count = monitors.len();
    let bytes = count * std::mem::size_of::<dear_imgui::sys::ImGuiPlatformMonitor>();
    let data_ptr = if bytes > 0 {
        dear_imgui::sys::igMemAlloc(bytes) as *mut dear_imgui::sys::ImGuiPlatformMonitor
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

/// Try to route a winit event to the correct ImGui viewport window
/// Returns true if the event was consumed by Dear ImGui
pub fn route_event_to_viewports<T>(imgui_ctx: &mut Context, event: &Event<T>) -> bool {
    match event {
        Event::WindowEvent { window_id, event } => {
            // Iterate ImGui viewports and find the window that matches this event's WindowId
            #[cfg(feature = "multi-viewport")]
            {
                unsafe {
                    let pio = dear_imgui::sys::igGetPlatformIO_Nil();
                    let viewports = &(*pio).Viewports;
                    if viewports.Data.is_null() || viewports.Size <= 0 {
                        return false;
                    }
                    for i in 0..viewports.Size {
                        let vp = *viewports.Data.add(i as usize);
                        if vp.is_null() {
                            continue;
                        }
                        let vd_ptr = (*vp).PlatformUserData as *mut ViewportData;
                        if vd_ptr.is_null() {
                            continue;
                        }
                        if let Some(vd) = vd_ptr.as_ref() {
                            // Skip main viewport: its events are handled by the primary platform path
                            if !vd.window_owned {
                                continue;
                            }
                            if let Some(window) = vd.window.as_ref() {
                                if &window.id() == window_id {
                                    // Handle OS-move/resize/DPI change/close notifications → set flags and scales
                                    match event {
                                        WindowEvent::Moved(_) => {
                                            let cur = dear_imgui::sys::igGetFrameCount();
                                            if vd.ignore_window_pos_event_frame != cur {
                                                (*vp).PlatformRequestMove = true;
                                            }
                                        }
                                        WindowEvent::Resized(_) => {
                                            let cur = dear_imgui::sys::igGetFrameCount();
                                            if vd.ignore_window_size_event_frame != cur {
                                                (*vp).PlatformRequestResize = true;
                                            }
                                        }
                                        WindowEvent::ScaleFactorChanged {
                                            scale_factor, ..
                                        } => {
                                            let s = *scale_factor as f32;
                                            (*vp).DpiScale = s;
                                            (*vp).FramebufferScale.x = s;
                                            (*vp).FramebufferScale.y = s;
                                        }
                                        WindowEvent::CloseRequested => {
                                            (*vp).PlatformRequestClose = true;
                                        }
                                        _ => {}
                                    }
                                    // Route specific events using existing handlers
                                    match event {
                                        WindowEvent::KeyboardInput { event, .. } => {
                                            return crate::events::handle_keyboard_input(
                                                event, imgui_ctx,
                                            );
                                        }
                                        WindowEvent::ModifiersChanged(mods) => {
                                            crate::events::handle_modifiers_changed(
                                                mods, imgui_ctx,
                                            );
                                            return imgui_ctx.io().want_capture_keyboard();
                                        }
                                        WindowEvent::MouseWheel { delta, .. } => {
                                            return crate::events::handle_mouse_wheel(
                                                *delta, imgui_ctx,
                                            );
                                        }
                                        WindowEvent::MouseInput { state, button, .. } => {
                                            return crate::events::handle_mouse_button(
                                                *button, *state, imgui_ctx,
                                            );
                                        }
                                        WindowEvent::CursorMoved { position, .. } => {
                                            // With multi-viewports, feed absolute/screen coordinates
                                            let pos_logical =
                                                position.to_logical(window.scale_factor());
                                            let logical = [pos_logical.x, pos_logical.y];
                                            if let Some(screen) =
                                                client_to_screen_pos(window, logical)
                                            {
                                                return crate::events::handle_cursor_moved(
                                                    [screen[0] as f64, screen[1] as f64],
                                                    imgui_ctx,
                                                );
                                            } else {
                                                let pos =
                                                    position.to_logical(window.scale_factor());
                                                return crate::events::handle_cursor_moved(
                                                    [pos.x, pos.y],
                                                    imgui_ctx,
                                                );
                                            }
                                        }
                                        WindowEvent::CursorLeft { .. } => {
                                            imgui_ctx
                                                .io_mut()
                                                .add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                                            return false;
                                        }
                                        WindowEvent::Focused(focused) => {
                                            return crate::events::handle_focused(
                                                *focused, imgui_ctx,
                                            );
                                        }
                                        WindowEvent::Ime(ime) => {
                                            crate::events::handle_ime_event(ime, imgui_ctx);
                                            return imgui_ctx.io().want_capture_keyboard();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Initialize the main viewport with proper ViewportData
fn init_main_viewport(main_window: &Window) {
    unsafe {
        let main_viewport = dear_imgui_rs::sys::igGetMainViewport();

        if !main_viewport.is_null() {
            let vp = &*main_viewport;
        }

        // Create ViewportData for main window
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        (*vd).window = main_window as *const Window as *mut Window;
        (*vd).window_owned = false; // Main window is owned by the application

        (*main_viewport).PlatformUserData = vd as *mut c_void;
        (*main_viewport).PlatformHandle = main_window as *const Window as *mut c_void;
    }
}

/// Shutdown multi-viewport support
pub fn shutdown_multi_viewport_support() {
    // Clean up any remaining viewports
    unsafe {
<<<<<<< HEAD
        dear_imgui::sys::igDestroyPlatformWindows();
        // Also clean up main viewport's PlatformUserData allocated by us
        let main_viewport = dear_imgui::sys::igGetMainViewport();
        if !main_viewport.is_null() {
            let vp = &mut *main_viewport;
            let vd_ptr = vp.PlatformUserData as *mut ViewportData;
            if !vd_ptr.is_null() {
                // Main window not owned by us: do not free the window itself
                (*vd_ptr).window = std::ptr::null_mut();
                let _ = Box::from_raw(vd_ptr);
                vp.PlatformUserData = std::ptr::null_mut();
            }
            // Clear handle to avoid dangling pointer
            vp.PlatformHandle = std::ptr::null_mut();
        }
=======
        dear_imgui_rs::sys::igDestroyPlatformWindows();
>>>>>>> main
    }
}

/// Store event loop reference for viewport creation
pub fn set_event_loop(event_loop: &ActiveEventLoop) {
    EVENT_LOOP.with(|el| {
        *el.borrow_mut() = Some(event_loop as *const ActiveEventLoop);
    });
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    // Get event loop reference
    let event_loop = EVENT_LOOP.with(|el| el.borrow().map(|ptr| unsafe { &*ptr }));

    let event_loop = match event_loop {
        Some(el) => el,
        None => return,
    };

    // Create ViewportData
    let vd = Box::into_raw(Box::new(ViewportData::new()));
    let vp_ref = unsafe { &mut *vp };
    vp_ref.PlatformUserData = vd as *mut c_void;

    // Handle viewport flags
    let viewport_flags = vp_ref.Flags;
    let mut window_attrs = WindowAttributes::default()
        .with_title("ImGui Viewport")
        .with_inner_size(LogicalSize::new(vp_ref.Size.x as f64, vp_ref.Size.y as f64))
        .with_position(winit::dpi::Position::Logical(LogicalPosition::new(
            vp_ref.Pos.x as f64,
            vp_ref.Pos.y as f64,
        )))
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
            mvlog!(
                "[winit-mv] Platform_CreateWindow id={} size=({}, {})",
                vp_ref.ID,
                vp_ref.Size.x,
                vp_ref.Size.y
            );
            let window_ptr = Box::into_raw(Box::new(window));
            unsafe {
                (*vd).window = window_ptr;
                (*vd).window_owned = true;
            }
            vp_ref.PlatformHandle = window_ptr as *mut c_void;

            // Initialize DPI/framebuffer scale immediately
            let window_ref: &Window = unsafe { &*window_ptr };
            let scale = window_ref.scale_factor() as f32;
            vp_ref.DpiScale = scale;
            vp_ref.FramebufferScale.x = scale;
            vp_ref.FramebufferScale.y = scale;

            // TODO: Set up event callbacks for this window
            // This is a critical missing piece - we need to route events from this window
            // back to ImGui. For now, this is a known limitation.
        }
        Err(_) => {
            // Clean up ViewportData on failure
            unsafe {
                let _ = Box::from_raw(vd);
            }
            vp_ref.PlatformUserData = std::ptr::null_mut();
        }
    }
}

/// Destroy a viewport window
unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &mut *vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_mut() } {
        if vd.window_owned && !vd.window.is_null() {
            // Clean up the window
            unsafe {
                let _ = Box::from_raw(vd.window);
            }
        }
        vd.window = std::ptr::null_mut();

        // Clean up ViewportData using the original raw pointer
        unsafe {
            let _ = Box::from_raw(vd_ptr);
        }
    }
    vp_ref.PlatformUserData = std::ptr::null_mut();
    vp_ref.PlatformHandle = std::ptr::null_mut();
}

/// Show a viewport window
unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            window.set_visible(true);
        }
    }
}

/// Get window position
unsafe extern "C" fn winit_get_window_pos(
<<<<<<< HEAD
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    mvlog!("[winit-mv] ENTER winit_get_window_pos vp={:?}", vp);
    if vp.is_null() {
        mvlog!("[winit-mv] LEAVE winit_get_window_pos (null vp) -> (0,0)");
        return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    let vp_ref = &*vp;

    // For main viewport (check by ID or lack of owned window data)
    // The main viewport might not always have ID == 0
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    let is_main_viewport = if vd_ptr.is_null() {
        true
    } else if let Some(vd) = vd_ptr.as_ref() {
        !vd.window_owned
    } else {
        true
    };

    if is_main_viewport {
        let result = dear_imgui::sys::ImVec2 {
            x: vp_ref.Pos.x,
            y: vp_ref.Pos.y,
        };
        mvlog!(
            "[winit-mv] LEAVE winit_get_window_pos (main) -> ({:.1}, {:.1})",
            result.x,
            result.y
        );
        return result;
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = vd_ptr.as_ref() {
        // Only query window position for windows we own
        if vd.window_owned && !vd.window.is_null() {
            if let Some(window) = vd.window.as_ref() {
                // Prefer client-area top-left in screen space
                if let Some([sx, sy]) = client_to_screen_pos(window, [0.0, 0.0]) {
                    let result = dear_imgui::sys::ImVec2 { x: sx, y: sy };
                    mvlog!(
                        "[winit-mv] LEAVE winit_get_window_pos (client->screen) -> ({:.1}, {:.1})",
                        result.x,
                        result.y
                    );
                    return result;
                }
                // Fallback
                match window.outer_position() {
                    Ok(pos) => {
                        let result = dear_imgui::sys::ImVec2 {
                            x: pos.x as f32,
                            y: pos.y as f32,
                        };
                        mvlog!(
                            "[winit-mv] LEAVE winit_get_window_pos (outer) -> ({:.1}, {:.1})",
                            result.x,
                            result.y
                        );
                        return result;
                    }
                    Err(e) => {
                        mvlog!("[winit-mv] outer_position error: {:?}", e);
                    }
                }
=======
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    // Special handling for viewport ID 0 (main viewport or ImGui internal viewport)
    let vp_ref = unsafe { &*vp };
    let viewport_id = vp_ref.ID;
    if viewport_id == 0 {
        // Return safe default for main viewport
        return dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            if let Ok(pos) = window.outer_position() {
                return dear_imgui_rs::sys::ImVec2 {
                    x: pos.x as f32,
                    y: pos.y as f32,
                };
>>>>>>> main
            }
        }
    }

<<<<<<< HEAD
    // Fallback to viewport's stored position
    let result = dear_imgui::sys::ImVec2 {
        x: vp_ref.Pos.x,
        y: vp_ref.Pos.y,
    };
    mvlog!(
        "[winit-mv] LEAVE winit_get_window_pos (fallback) -> ({:.1}, {:.1})",
        result.x,
        result.y
    );
    result
}

/// Get window position (out-parameter version to avoid MSVC small-aggregate return)
unsafe extern "C" fn winit_get_window_pos_out(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    out_pos: *mut dear_imgui::sys::ImVec2,
) {
    let mut r = dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    if !vp.is_null() {
        let vp_ref = &*vp;
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        // Heuristic: main viewport or missing data → use cached Pos
        let is_main = vd_ptr.is_null()
            || (|| {
                if let Some(vd) = unsafe { vd_ptr.as_ref() } {
                    !vd.window_owned
                } else {
                    true
                }
            })();
        if is_main {
            r.x = vp_ref.Pos.x;
            r.y = vp_ref.Pos.y;
        } else if let Some(vd) = unsafe { vd_ptr.as_ref() } {
            if !vd.window.is_null() {
                if let Some(window) = unsafe { vd.window.as_ref() } {
                    if let Some([sx, sy]) = client_to_screen_pos(window, [0.0, 0.0]) {
                        r.x = sx;
                        r.y = sy;
                    } else if let Ok(pos) = window.outer_position() {
                        r.x = pos.x as f32;
                        r.y = pos.y as f32;
                    } else {
                        r.x = vp_ref.Pos.x;
                        r.y = vp_ref.Pos.y;
                    }
                }
            }
        }
    }
    if !out_pos.is_null() {
        unsafe {
            *out_pos = r;
        }
    }
}

/// Get window position (v2: always prefer OS window position when available)
unsafe extern "C" fn winit_get_window_pos_out_v2(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    out_pos: *mut dear_imgui::sys::ImVec2,
) {
    let mut r = dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    if !vp.is_null() {
        let vp_ref = &*vp;
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = vd_ptr.as_ref() {
            if let Some(window) = vd.window.as_ref() {
                if let Some([sx, sy]) = client_to_screen_pos(window, [0.0, 0.0]) {
                    r.x = sx;
                    r.y = sy;
                } else if let Ok(pos) = window.outer_position() {
                    r.x = pos.x as f32;
                    r.y = pos.y as f32;
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
=======
    dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 }
>>>>>>> main
}

/// Set window position
unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    pos: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_mut() } {
        if let Some(window) = unsafe { vd.window.as_mut() } {
<<<<<<< HEAD
            // ImGui provides screen-space physical pixels; convert to logical for winit
            let phys = winit::dpi::PhysicalPosition::new(pos.x as f64, pos.y as f64);
            let logical: winit::dpi::LogicalPosition<f64> = phys.to_logical(window.scale_factor());
            window.set_outer_position(winit::dpi::Position::Logical(logical));
            vd.ignore_window_pos_event_frame = unsafe { dear_imgui::sys::igGetFrameCount() };
=======
            let position = LogicalPosition::new(pos.x as f64, pos.y as f64);
            window.set_outer_position(position);
            vd.ignore_window_pos_event_frame = dear_imgui_rs::sys::igGetFrameCount();
>>>>>>> main
        }
    }
}

/// Get window size
unsafe extern "C" fn winit_get_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    let vp_ref = unsafe { &*vp };

    // For main viewport, always use stored size since we don't own the window handle
    if vp_ref.ID == 0 || vp_ref.PlatformUserData.is_null() {
        let result = dear_imgui::sys::ImVec2 {
            x: vp_ref.Size.x,
            y: vp_ref.Size.y,
        };

        return result;
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
<<<<<<< HEAD
        // Only query window size for windows we own
        if vd.window_owned && !vd.window.is_null() {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let size = window.inner_size();
                let result = dear_imgui::sys::ImVec2 {
                    x: size.width as f32,
                    y: size.height as f32,
                };

                return result;
            }
        }
    }

    // Fallback to viewport's stored size
    let result = dear_imgui::sys::ImVec2 {
        x: vp_ref.Size.x,
        y: vp_ref.Size.y,
    };

    result
}

/// Get window size (out-parameter version to avoid MSVC small-aggregate return)
unsafe extern "C" fn winit_get_window_size_out(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    out_size: *mut dear_imgui::sys::ImVec2,
) {
    let mut r = dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    if !vp.is_null() {
        let vp_ref = &*vp;
        if vp_ref.ID == 0 || vp_ref.PlatformUserData.is_null() {
            r.x = vp_ref.Size.x;
            r.y = vp_ref.Size.y;
        } else {
            let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
            if let Some(vd) = unsafe { vd_ptr.as_ref() } {
                if vd.window_owned && !vd.window.is_null() {
                    if let Some(window) = unsafe { vd.window.as_ref() } {
                        let size = window.inner_size();
                        r.x = size.width as f32;
                        r.y = size.height as f32;
                    }
                } else {
                    r.x = vp_ref.Size.x;
                    r.y = vp_ref.Size.y;
                }
            }
        }
    }
    if !out_size.is_null() {
        unsafe {
            *out_size = r;
        }
    }
}

/// Get window size (v2: always prefer OS inner size when available)
unsafe extern "C" fn winit_get_window_size_out_v2(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    out_size: *mut dear_imgui::sys::ImVec2,
) {
    let mut r = dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    if !vp.is_null() {
        let vp_ref = &*vp;
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = vd_ptr.as_ref() {
            if let Some(window) = vd.window.as_ref() {
                let size = window.inner_size();
                r.x = size.width as f32;
                r.y = size.height as f32;
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
=======
        if let Some(window) = unsafe { vd.window.as_ref() } {
            let size = window.inner_size();
            return dear_imgui_rs::sys::ImVec2 {
                x: size.width as f32,
                y: size.height as f32,
            };
        }
    }

    dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 }
>>>>>>> main
}

/// Set window size
unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_mut() } {
        if let Some(window) = unsafe { vd.window.as_mut() } {
<<<<<<< HEAD
            // ImGui provides inner size in physical pixels; convert to logical for winit
            let scale = window.scale_factor();
            let logical: winit::dpi::LogicalSize<f64> =
                LogicalSize::new((size.x as f64) / scale, (size.y as f64) / scale);
            let _ = window.request_inner_size(winit::dpi::Size::Logical(logical));
            vd.ignore_window_size_event_frame = unsafe { dear_imgui::sys::igGetFrameCount() };
=======
            let new_size = LogicalSize::new(size.x as f64, size.y as f64);
            let _ = window.request_inner_size(new_size);
            vd.ignore_window_size_event_frame = dear_imgui_rs::sys::igGetFrameCount();
>>>>>>> main
        }
    }
}

/// Set window focus
unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            window.focus_window();
        }
    }
}

/// Get window focus
unsafe extern "C" fn winit_get_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }

    let vp_ref = unsafe { &*vp };
    // Query from actual OS window if available (main or secondary)
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            return window.has_focus();
        }
    }
    false
}

/// Get window minimized state
unsafe extern "C" fn winit_get_window_minimized(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    if vp.is_null() {
        return false;
    }

    let vp_ref = unsafe { &*vp };
    // Query from actual OS window if available (main or secondary)
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            return window.is_minimized().unwrap_or(false);
        }
    }
    false
}

/// Set window title
unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    title: *const c_char,
) {
    if vp.is_null() || title.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if let Some(window) = unsafe { vd.window.as_ref() } {
            if let Ok(title_str) = unsafe { CStr::from_ptr(title) }.to_str() {
                window.set_title(title_str);
            }
        }
    }
}

/// Get window framebuffer scale
unsafe extern "C" fn winit_get_window_framebuffer_scale(
<<<<<<< HEAD
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    mvlog!(
        "[winit-mv] ENTER winit_get_window_framebuffer_scale vp={:?}",
        vp
    );
    if vp.is_null() {
        mvlog!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (null) -> (1,1)");
        return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
=======
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
>>>>>>> main
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if vd_ptr.is_null() {
        mvlog!(
            "[winit-mv] LEAVE winit_get_window_framebuffer_scale (no PlatformUserData) -> (1,1)"
        );
        return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
    }
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if vd.window.is_null() {
            mvlog!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (no window) -> (1,1)");
            return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
        }
        if let Some(window) = unsafe { vd.window.as_ref() } {
            let scale = window.scale_factor() as f32;
<<<<<<< HEAD
            mvlog!(
                "[winit-mv] LEAVE winit_get_window_framebuffer_scale -> ({:.2}, {:.2})",
                scale,
                scale
            );
            return dear_imgui::sys::ImVec2 { x: scale, y: scale };
        }
    }
    mvlog!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (fallback) -> (1,1)");
    dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 }
=======
            return dear_imgui_rs::sys::ImVec2 { x: scale, y: scale };
        }
    }

    dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 }
>>>>>>> main
}

/// Get window DPI scale (float)
unsafe extern "C" fn winit_get_window_dpi_scale(vp: *mut dear_imgui::sys::ImGuiViewport) -> f32 {
    if vp.is_null() {
        return 1.0;
    }
    let vp_ref = &*vp;
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = vd_ptr.as_ref() {
        if let Some(window) = vd.window.as_ref() {
            return window.scale_factor() as f32;
        }
    }
    1.0
}

/// Get window work area insets (ImVec4: left, top, right, bottom)
unsafe extern "C" fn winit_get_window_work_area_insets(
    _vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec4 {
    dear_imgui::sys::ImVec4 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    }
}

/// Notify viewport changed (no-op)
unsafe extern "C" fn winit_on_changed_viewport(_vp: *mut dear_imgui::sys::ImGuiViewport) {}

/// Set window alpha (no-op for winit)
unsafe extern "C" fn winit_set_window_alpha(vp: *mut dear_imgui::sys::ImGuiViewport, alpha: f32) {
    if vp.is_null() {
        return;
    }
}

/// Platform render window (no-op; renderer handles rendering)
unsafe extern "C" fn winit_platform_render_window(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
}

/// Platform swap buffers (no-op; renderer handles present)
unsafe extern "C" fn winit_platform_swap_buffers(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
}

/// Platform create Vulkan surface (not used; return failure)
unsafe extern "C" fn winit_platform_create_vk_surface(
    _vp: *mut dear_imgui::sys::ImGuiViewport,
    _vk_inst: u64,
    _vk_allocators: *const c_void,
    out_vk_surface: *mut u64,
) -> ::std::os::raw::c_int {
    if !out_vk_surface.is_null() {
        *out_vk_surface = 0;
    }
    -1 // Not supported
}

/// Update window - called by ImGui for platform-specific updates
<<<<<<< HEAD
unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
=======
unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {}
>>>>>>> main

    // For now, this is a no-op. In GLFW implementation, this is used for
    // platform-specific window updates. Winit handles most of this automatically.
    // We might need to add specific logic here later for things like:
    // - Window state synchronization
    // - Platform-specific optimizations
    // - Event processing
}
