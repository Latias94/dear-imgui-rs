//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

#![allow(unsafe_op_in_unsafe_fn)]

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

fn abort_on_panic<R>(name: &str, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-imgui-winit: panic in {}", name);
            std::process::abort();
        }
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
    // Last framebuffer scale we logged for this viewport (debug only).
    pub last_log_fb_scale: f32,
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
            last_log_fb_scale: 0.0,
        }
    }
}

// Convert client-area logical coordinates to screen coordinates (logical), per-window
pub(crate) fn client_to_screen_pos(window: &Window, logical: [f64; 2]) -> Option<[f32; 2]> {
    let scale = window.scale_factor();
    // Cross-platform: absolute screen = client top-left (logical) + client offset (logical)
    let base = window
        .inner_position()
        .ok()
        .map(|p| p.to_logical::<f64>(scale))
        .or_else(|| {
            window
                .outer_position()
                .ok()
                .map(|p| p.to_logical::<f64>(scale))
        });
    if let Some(base) = base {
        Some([(base.x + logical[0]) as f32, (base.y + logical[1]) as f32])
    } else {
        Some([logical[0] as f32, logical[1] as f32])
    }
}

/// Compute the decoration offset in logical pixels: inner_position - outer_position.
///
/// This lets us translate between ImGui's platform coordinate space (client origin)
/// and winit's outer-position APIs. Returns None if either position is unavailable.
fn decoration_offset_logical(window: &Window) -> Option<(f64, f64)> {
    let scale = window.scale_factor();
    let inner_phys = window.inner_position().ok()?;
    let outer_phys = window.outer_position().ok()?;
    let inner_log = inner_phys.to_logical::<f64>(scale);
    let outer_log = outer_phys.to_logical::<f64>(scale);
    Some((inner_log.x - outer_log.x, inner_log.y - outer_log.y))
}

/// Initialize multi-viewport support following official ImGui backend pattern
pub fn init_multi_viewport_support(ctx: &mut Context, main_window: &Window) {
    // Set up platform callbacks using direct C API
    unsafe {
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

        // Also register framebuffer/DPI scale and work area insets callbacks
        let pio_sys = dear_imgui_rs::sys::igGetPlatformIO_Nil();
        // Install out-parameter getters via cimgui helpers (avoid struct-return ABI)
        dear_imgui_rs::sys::ImGuiPlatformIO_Set_Platform_GetWindowPos(
            pio_sys,
            Some(winit_get_window_pos_out_v2),
        );
        dear_imgui_rs::sys::ImGuiPlatformIO_Set_Platform_GetWindowSize(
            pio_sys,
            Some(winit_get_window_size_out_v2),
        );
        // Framebuffer scale callback.
        //
        // On MSVC, returning ImVec2 by value from a foreign callback can be ABI-fragile, so we
        // keep this disabled on Windows for now and rely on Viewport::FramebufferScale and
        // io.DisplayFramebufferScale instead.
        #[cfg(not(target_os = "windows"))]
        {
            // ImGui will use FramebufferScale when available, falling back to
            // DisplayFramebufferScale otherwise.
            (*pio_sys).Platform_GetWindowFramebufferScale =
                Some(winit_get_window_framebuffer_scale);
        }
        #[cfg(target_os = "windows")]
        {
            (*pio_sys).Platform_GetWindowFramebufferScale = None;
        }
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

    let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
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

/// Try to route a winit event to the correct ImGui viewport window
/// Returns true if the event was consumed by Dear ImGui
pub fn route_event_to_viewports<T>(imgui_ctx: &mut Context, event: &Event<T>) -> bool {
    match event {
        Event::WindowEvent { window_id, event } => {
            // Iterate ImGui viewports and find the window that matches this event's WindowId
            #[cfg(feature = "multi-viewport")]
            {
                unsafe {
                    let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
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
                                            let cur = dear_imgui_rs::sys::igGetFrameCount();
                                            if vd.ignore_window_pos_event_frame != cur {
                                                (*vp).PlatformRequestMove = true;
                                            }
                                        }
                                        WindowEvent::Resized(_) => {
                                            let cur = dear_imgui_rs::sys::igGetFrameCount();
                                            if vd.ignore_window_size_event_frame != cur {
                                                (*vp).PlatformRequestResize = true;
                                            }
                                        }
                                        WindowEvent::ScaleFactorChanged { .. } => {
                                            // Keep cached scales up-to-date immediately.
                                            let scale = window.scale_factor() as f32;
                                            if scale.is_finite() && scale > 0.0 {
                                                (*vp).DpiScale = scale;
                                                (*vp).FramebufferScale.x = scale;
                                                (*vp).FramebufferScale.y = scale;
                                            }
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
                                            // Mark the hovered viewport for Dear ImGui.
                                            imgui_ctx.io_mut().add_mouse_viewport_event(
                                                dear_imgui_rs::Id::from((*vp).ID),
                                            );
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
                                            {
                                                let io = imgui_ctx.io_mut();
                                                io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                                                // Mouse left this platform window; clear hovered viewport.
                                                io.add_mouse_viewport_event(
                                                    dear_imgui_rs::Id::default(),
                                                );
                                            }
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

/// Convenience helper: handle an event for both the main window and all ImGui-created viewports.
///
/// This mirrors the pattern used in `examples/02-docking/multi_viewport_wgpu.rs` and helps
/// avoid forgetting to route events to secondary viewports.
///
/// Usage:
///
/// ```ignore
/// let full = Event::WindowEvent { window_id, event: event.clone() };
/// let _ = multi_viewport::handle_event_with_multi_viewport(
///     &mut platform,
///     &mut imgui,
///     &main_window,
///     &full,
/// );
/// ```
#[cfg(feature = "multi-viewport")]
pub fn handle_event_with_multi_viewport<T>(
    platform: &mut crate::platform::WinitPlatform,
    imgui_ctx: &mut Context,
    main_window: &Window,
    event: &Event<T>,
) -> bool {
    let mut consumed = false;

    // Forward events that target the main window through the standard platform handler
    if let Event::WindowEvent { window_id, .. } = event {
        if *window_id == main_window.id() {
            if platform.handle_event(imgui_ctx, main_window, event) {
                consumed = true;
            }
        }
    }

    // Route events (including those for secondary windows) to ImGui viewports
    if route_event_to_viewports(imgui_ctx, event) {
        consumed = true;
    }

    consumed
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
        dear_imgui_rs::sys::igDestroyPlatformWindows();
        // Also clean up main viewport's PlatformUserData allocated by us
        let main_viewport = dear_imgui_rs::sys::igGetMainViewport();
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
    }
}

/// Store an `ActiveEventLoop` reference for viewport creation.
///
/// winit's `ActiveEventLoop` is only valid for the duration of the callback it is
/// passed into. Do **not** store it long-term. Call this right before invoking
/// `Context::update_platform_windows()` / `Context::render_platform_windows_default()`
/// (or any other ImGui call that may create viewports), and preferably clear it
/// afterwards via `set_event_loop_for_frame` or `clear_event_loop`.
pub fn set_event_loop(event_loop: &ActiveEventLoop) {
    EVENT_LOOP.with(|el| {
        *el.borrow_mut() = Some(event_loop as *const ActiveEventLoop);
    });
}

/// Clear any previously stored event loop pointer.
pub fn clear_event_loop() {
    EVENT_LOOP.with(|el| {
        *el.borrow_mut() = None;
    });
}

/// A guard that keeps the event loop pointer valid for the current callback.
///
/// Dropping the guard clears the stored pointer.
pub struct EventLoopFrameGuard;

impl Drop for EventLoopFrameGuard {
    fn drop(&mut self) {
        clear_event_loop();
    }
}

/// Set the event loop pointer for the duration of the current callback.
///
/// Recommended usage:
///
/// ```rust,no_run
/// # use dear_imgui_winit::multi_viewport;
/// # use winit::event_loop::ActiveEventLoop;
/// # fn on_redraw(event_loop: &ActiveEventLoop) {
/// let _guard = multi_viewport::set_event_loop_for_frame(event_loop);
/// // imgui.update_platform_windows();
/// // imgui.render_platform_windows_default();
/// # }
/// ```
pub fn set_event_loop_for_frame(event_loop: &ActiveEventLoop) -> EventLoopFrameGuard {
    set_event_loop(event_loop);
    EventLoopFrameGuard
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
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

        // Create ViewportData
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        let vp_ref = unsafe { &mut *vp };
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
                mvlog!(
                    "[winit-mv] Platform_CreateWindow id={} size=({}, {})",
                    vp_ref.ID,
                    vp_ref.Size.x,
                    vp_ref.Size.y
                );
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
                    let _ = Box::from_raw(vd);
                }
                vp_ref.PlatformUserData = std::ptr::null_mut();
            }
        }
    });
}

/// Destroy a viewport window
unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
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

        unsafe {
            if (*vd_ptr).window_owned && !(*vd_ptr).window.is_null() {
                // Clean up the window.
                let _ = Box::from_raw((*vd_ptr).window);
            }
            (*vd_ptr).window = std::ptr::null_mut();

            // Drop the allocation backing `ViewportData`. Avoid creating an `&mut ViewportData`
            // reference and then freeing it while that reference is still live.
            let _ = Box::from_raw(vd_ptr);
        }
        vp_ref.PlatformUserData = std::ptr::null_mut();
        vp_ref.PlatformHandle = std::ptr::null_mut();
    });
}

/// Show a viewport window
unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_ShowWindow", || {
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
    });
}

/// Get window position
unsafe extern "C" fn winit_get_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    abort_on_panic("Platform_GetWindowPos", || unsafe {
        mvlog!("[winit-mv] ENTER winit_get_window_pos vp={:?}", vp);
        if vp.is_null() {
            mvlog!("[winit-mv] LEAVE winit_get_window_pos (null vp) -> (0,0)");
            return dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
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
            let result = dear_imgui_rs::sys::ImVec2 {
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
                        let result = dear_imgui_rs::sys::ImVec2 { x: sx, y: sy };
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
                            let result = dear_imgui_rs::sys::ImVec2 {
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
                }
            }
        }

        // Fallback to viewport's stored position
        let result = dear_imgui_rs::sys::ImVec2 {
            x: vp_ref.Pos.x,
            y: vp_ref.Pos.y,
        };
        mvlog!(
            "[winit-mv] LEAVE winit_get_window_pos (fallback) -> ({:.1}, {:.1})",
            result.x,
            result.y
        );
        result
    })
}

/// Get window position (out-parameter version to avoid MSVC small-aggregate return)
unsafe extern "C" fn winit_get_window_pos_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_pos: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_pos_out", || unsafe {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
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
                // For main viewport, prefer OS client (inner) position if available.
                if let Some(vd) = unsafe { vd_ptr.as_ref() } {
                    if let Some(window) = unsafe { vd.window.as_ref() } {
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
            } else if let Some(vd) = unsafe { vd_ptr.as_ref() } {
                if !vd.window.is_null() {
                    if let Some(window) = unsafe { vd.window.as_ref() } {
                        // Platform_GetWindowPos is expected to return the OS client (inner) position.
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
                    }
                }
            }
        }
        if !out_pos.is_null() {
            unsafe {
                *out_pos = r;
            }
        }
    });
}

/// Get window position (v2: always prefer OS window position when available)
unsafe extern "C" fn winit_get_window_pos_out_v2(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_pos: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_pos_out_v2", || unsafe {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = &*vp;
            let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
            if let Some(vd) = vd_ptr.as_ref() {
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
unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    pos: dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_set_window_pos", || unsafe {
        if vp.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = unsafe { vd_ptr.as_mut() } {
            if let Some(window) = unsafe { vd.window.as_mut() } {
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

/// Get window size
unsafe extern "C" fn winit_get_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    abort_on_panic("winit_get_window_size", || unsafe {
        if vp.is_null() {
            return dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        }

        let vp_ref = unsafe { &*vp };

        // For main viewport, always use stored size since we don't own the window handle
        if vp_ref.ID == 0 || vp_ref.PlatformUserData.is_null() {
            let result = dear_imgui_rs::sys::ImVec2 {
                x: vp_ref.Size.x,
                y: vp_ref.Size.y,
            };

            return result;
        }

        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = unsafe { vd_ptr.as_ref() } {
            // Only query window size for windows we own
            if vd.window_owned && !vd.window.is_null() {
                if let Some(window) = unsafe { vd.window.as_ref() } {
                    let size_phys = window.inner_size();
                    let size_logical: LogicalSize<f64> =
                        size_phys.to_logical(window.scale_factor());
                    let result = dear_imgui_rs::sys::ImVec2 {
                        x: size_logical.width as f32,
                        y: size_logical.height as f32,
                    };

                    return result;
                }
            }
        }

        // Fallback to viewport's stored size
        let result = dear_imgui_rs::sys::ImVec2 {
            x: vp_ref.Size.x,
            y: vp_ref.Size.y,
        };

        result
    })
}

/// Get window size (out-parameter version to avoid MSVC small-aggregate return)
unsafe extern "C" fn winit_get_window_size_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_size: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_size_out", || unsafe {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
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
                            let size_phys = window.inner_size();
                            let size_logical: LogicalSize<f64> =
                                size_phys.to_logical(window.scale_factor());
                            r.x = size_logical.width as f32;
                            r.y = size_logical.height as f32;
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
    });
}

/// Get window size (v2: always prefer OS inner size when available)
unsafe extern "C" fn winit_get_window_size_out_v2(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_size: *mut dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_get_window_size_out_v2", || unsafe {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = &*vp;
            let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
            if let Some(vd) = vd_ptr.as_ref() {
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
unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    abort_on_panic("winit_set_window_size", || unsafe {
        if vp.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = unsafe { vd_ptr.as_mut() } {
            if let Some(window) = unsafe { vd.window.as_mut() } {
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
unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("winit_set_window_focus", || unsafe {
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
    });
}

/// Get window focus
unsafe extern "C" fn winit_get_window_focus(vp: *mut dear_imgui_rs::sys::ImGuiViewport) -> bool {
    abort_on_panic("winit_get_window_focus", || unsafe {
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
    })
}

/// Get window minimized state
unsafe extern "C" fn winit_get_window_minimized(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    abort_on_panic("Platform_GetWindowMinimized", || unsafe {
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
    })
}

/// Set window title
unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    title: *const c_char,
) {
    abort_on_panic("Platform_SetWindowTitle", || unsafe {
        if vp.is_null() || title.is_null() {
            return;
        }

        let vp_ref = unsafe { &*vp };
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if let Some(vd) = unsafe { vd_ptr.as_ref() } {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let title = unsafe { CStr::from_ptr(title) }.to_string_lossy();
                window.set_title(title.as_ref());
            }
        }
    });
}

/// Get window framebuffer scale
unsafe extern "C" fn winit_get_window_framebuffer_scale(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    abort_on_panic("Platform_GetWindowFramebufferScale", || unsafe {
        if vp.is_null() {
            return dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
        }

        let vp_ref = unsafe { &*vp };
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        if vd_ptr.is_null() {
            return dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
        }
        if let Some(vd) = unsafe { vd_ptr.as_mut() } {
            // Always report actual framebuffer scale for this viewport, including the main one.
            // Dear ImGui relies on this to compute correct scaling when windows move between
            // viewports. Upstream backends (GLFW/SDL) do the same.
            if vd.window.is_null() {
                return dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
            }
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let scale = window.scale_factor() as f32;
                if cfg!(feature = "mv-log") && (scale - vd.last_log_fb_scale).abs() > 0.01 {
                    mvlog!(
                        "[winit-mv] fb_scale changed id={} -> {:.2}",
                        vp_ref.ID,
                        scale
                    );
                    vd.last_log_fb_scale = scale;
                }
                return dear_imgui_rs::sys::ImVec2 { x: scale, y: scale };
            }
        }
        dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 }
    })
}

/// Get window DPI scale (float)
unsafe extern "C" fn winit_get_window_dpi_scale(vp: *mut dear_imgui_rs::sys::ImGuiViewport) -> f32 {
    abort_on_panic("Platform_GetWindowDpiScale", || unsafe {
        if vp.is_null() {
            return 1.0;
        }
        let vp_ref = &mut *vp;
        let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
        let mut scale = 1.0f32;
        if let Some(vd) = vd_ptr.as_ref() {
            if let Some(window) = vd.window.as_ref() {
                scale = window.scale_factor() as f32;
            }
        }
        if !scale.is_finite() || scale <= 0.0 {
            scale = 1.0;
        }
        // On Windows we keep Platform_GetWindowFramebufferScale disabled (ABI concerns).
        // Keep the per-viewport cached framebuffer scale in sync via this callback.
        #[cfg(target_os = "windows")]
        {
            vp_ref.FramebufferScale.x = scale;
            vp_ref.FramebufferScale.y = scale;
        }
        scale
    })
}

/// Get window work area insets (ImVec4: left, top, right, bottom)
unsafe extern "C" fn winit_get_window_work_area_insets(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec4 {
    abort_on_panic("Platform_GetWindowWorkAreaInsets", || {
        dear_imgui_rs::sys::ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    })
}

/// Notify viewport changed.
///
/// Dear ImGui calls this when a viewport changes monitor or ownership. We use it
/// for targeted debug output to diagnose DPI/scale transitions without per-frame spam.
unsafe extern "C" fn winit_on_changed_viewport(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_OnChangedViewport", || unsafe {
        if vp.is_null() {
            return;
        }
        let vp_ref = &*vp;
        mvlog!(
            "[winit-mv] OnChangedViewport id={} pos=({:.1},{:.1}) size=({:.1},{:.1}) dpi_scale={:.2} fb_scale=({:.2},{:.2})",
            vp_ref.ID,
            vp_ref.Pos.x,
            vp_ref.Pos.y,
            vp_ref.Size.x,
            vp_ref.Size.y,
            vp_ref.DpiScale,
            vp_ref.FramebufferScale.x,
            vp_ref.FramebufferScale.y
        );
    });
}

/// Set window alpha (no-op for winit)
unsafe extern "C" fn winit_set_window_alpha(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
    abort_on_panic("Platform_SetWindowAlpha", || unsafe {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform render window (no-op; renderer handles rendering)
unsafe extern "C" fn winit_platform_render_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    abort_on_panic("Platform_RenderWindow", || unsafe {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform swap buffers (no-op; renderer handles present)
unsafe extern "C" fn winit_platform_swap_buffers(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    abort_on_panic("Platform_SwapBuffers", || unsafe {
        if vp.is_null() {
            return;
        }
    });
}

/// Platform create Vulkan surface (not used; return failure)
unsafe extern "C" fn winit_platform_create_vk_surface(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _vk_inst: u64,
    _vk_allocators: *const c_void,
    out_vk_surface: *mut u64,
) -> ::std::os::raw::c_int {
    abort_on_panic("Platform_CreateVkSurface", || unsafe {
        if !out_vk_surface.is_null() {
            *out_vk_surface = 0;
        }
        -1 // Not supported
    })
}

/// Update window - called by ImGui for platform-specific updates
unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    abort_on_panic("Platform_UpdateWindow", || unsafe {
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
