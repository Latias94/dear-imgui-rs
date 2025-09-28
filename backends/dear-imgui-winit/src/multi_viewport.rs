//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};

use dear_imgui::Context;
use dear_imgui::platform_io::Viewport as IoViewport;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{
    ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowLevel};

// Debug trampolines defined in C++ (imgui.cpp) to avoid MSVC aggregate return ABI issues
// when callbacks return ImVec2/ImVec4 by value. Keep declarations once at module scope.
// Note: To avoid link-time dependencies on C++ debug stubs, only declare the ones we actually use here.
unsafe extern "C" {
    fn ImGui_DebugTest_ReturnZero_GetWindowPos(
        vp: *mut dear_imgui::sys::ImGuiViewport,
    ) -> dear_imgui::sys::ImVec2;
    fn ImGui_DebugTest_ReturnZero_GetWindowSize(
        vp: *mut dear_imgui::sys::ImGuiViewport,
    ) -> dear_imgui::sys::ImVec2;
    fn ImGui_DebugTest_False_GetWindowBool(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool;
}

// Thread-local storage for winit multi-viewport support
thread_local! {
    static EVENT_LOOP: RefCell<Option<*const ActiveEventLoop>> = const { RefCell::new(None) };
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
        // DEBUG: test C++-side stub to validate FFI return ABI
        pio.set_platform_get_window_pos_raw(Some(ImGui_DebugTest_ReturnZero_GetWindowPos));
        pio.set_platform_set_window_size_raw(Some(winit_set_window_size));
        pio.set_platform_get_window_size_raw(Some(ImGui_DebugTest_ReturnZero_GetWindowSize));
        pio.set_platform_set_window_focus_raw(Some(winit_set_window_focus));
        pio.set_platform_get_window_focus_raw(Some(winit_get_window_focus));
        pio.set_platform_get_window_minimized_raw(Some(winit_get_window_minimized));
        pio.set_platform_set_window_title_raw(Some(winit_set_window_title));
        pio.set_platform_update_window_raw(Some(winit_update_window));

        // Note: no typed setter for framebuffer scale in PlatformIo wrapper. Rely on defaults set in imgui.cpp.
        let pio_sys = dear_imgui::sys::igGetPlatformIO_Nil();
        (*pio_sys).Platform_GetWindowFramebufferScale = None;
        (*pio_sys).Platform_GetWindowDpiScale = None;
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
                            if let Some(window) = vd.window.as_ref() {
                                if &window.id() == window_id {
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
                                            // Convert from winit logical to ImGui logical using this window's scale factor
                                            let pos = position.to_logical(window.scale_factor());
                                            return crate::events::handle_cursor_moved(
                                                [pos.x, pos.y],
                                                imgui_ctx,
                                            );
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
        let main_viewport = dear_imgui::sys::igGetMainViewport();

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
        dear_imgui::sys::igDestroyPlatformWindows();
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
unsafe extern "C" fn winit_create_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
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
    if viewport_flags & (dear_imgui::sys::ImGuiViewportFlags_NoDecoration as i32) != 0 {
        window_attrs = window_attrs.with_decorations(false);
    }

    // Handle always on top
    if viewport_flags & (dear_imgui::sys::ImGuiViewportFlags_TopMost as i32) != 0 {
        window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
    }

    // Create the window
    match event_loop.create_window(window_attrs) {
        Ok(window) => {
            eprintln!(
                "[winit-mv] Platform_CreateWindow id={} size=({}, {})",
                vp_ref.ID, vp_ref.Size.x, vp_ref.Size.y
            );
            let window_ptr = Box::into_raw(Box::new(window));
            unsafe {
                (*vd).window = window_ptr;
                (*vd).window_owned = true;
            }
            vp_ref.PlatformHandle = window_ptr as *mut c_void;

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
unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
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

        // Clean up ViewportData
        unsafe {
            let _ = Box::from_raw(vd);
        }
    }
    vp_ref.PlatformUserData = std::ptr::null_mut();
    vp_ref.PlatformHandle = std::ptr::null_mut();
}

/// Show a viewport window
unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
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
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    eprintln!("[winit-mv] ENTER winit_get_window_pos vp={:?}", vp);
    if vp.is_null() {
        eprintln!("[winit-mv] LEAVE winit_get_window_pos (null vp) -> (0,0)");
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
        eprintln!(
            "[winit-mv] LEAVE winit_get_window_pos (main) -> ({:.1}, {:.1})",
            result.x, result.y
        );
        return result;
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = vd_ptr.as_ref() {
        // Only query window position for windows we own
        if vd.window_owned && !vd.window.is_null() {
            if let Some(window) = vd.window.as_ref() {
                match window.outer_position() {
                    Ok(pos) => {
                        let result = dear_imgui::sys::ImVec2 {
                            x: pos.x as f32,
                            y: pos.y as f32,
                        };
                        eprintln!(
                            "[winit-mv] LEAVE winit_get_window_pos (owned) -> ({:.1}, {:.1})",
                            result.x, result.y
                        );
                        return result;
                    }
                    Err(e) => {
                        eprintln!("[winit-mv] outer_position error: {:?}", e);
                    }
                }
            }
        }
    }

    // Fallback to viewport's stored position
    let result = dear_imgui::sys::ImVec2 {
        x: vp_ref.Pos.x,
        y: vp_ref.Pos.y,
    };
    eprintln!(
        "[winit-mv] LEAVE winit_get_window_pos (fallback) -> ({:.1}, {:.1})",
        result.x, result.y
    );
    result
}

/// Set window position
unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    pos: dear_imgui::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_mut() } {
        if let Some(window) = unsafe { vd.window.as_mut() } {
            let position = LogicalPosition::new(pos.x as f64, pos.y as f64);
            window.set_outer_position(position);
            vd.ignore_window_pos_event_frame = unsafe { dear_imgui::sys::igGetFrameCount() };
        }
    }
}

/// Get window size
unsafe extern "C" fn winit_get_window_size(
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
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

/// Set window size
unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    size: dear_imgui::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_mut() } {
        if let Some(window) = unsafe { vd.window.as_mut() } {
            let new_size = LogicalSize::new(size.x as f64, size.y as f64);
            let _ = window.request_inner_size(new_size);
            vd.ignore_window_size_event_frame = unsafe { dear_imgui::sys::igGetFrameCount() };
        }
    }
}

/// Set window focus
unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) {
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
unsafe extern "C" fn winit_get_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }

    let vp_ref = unsafe { &*vp };

    // For main viewport, return true (we assume it has focus when running)
    if vp_ref.ID == 0 || vp_ref.PlatformUserData.is_null() {
        return true;
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        // Only query window focus for windows we own
        if vd.window_owned && !vd.window.is_null() {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let result = window.has_focus();

                return result;
            }
        }
    }

    let result = false;

    result
}

/// Get window minimized state
unsafe extern "C" fn winit_get_window_minimized(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }

    let vp_ref = unsafe { &*vp };

    // For main viewport, return false (we assume it's not minimized when running)
    if vp_ref.ID == 0 || vp_ref.PlatformUserData.is_null() {
        return false;
    }

    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        // Only query window state for windows we own
        if vd.window_owned && !vd.window.is_null() {
            if let Some(window) = unsafe { vd.window.as_ref() } {
                let result = window.is_minimized().unwrap_or(false);

                return result;
            }
        }
    }

    let result = false;

    result
}

/// Set window title
unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui::sys::ImGuiViewport,
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
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    eprintln!(
        "[winit-mv] ENTER winit_get_window_framebuffer_scale vp={:?}",
        vp
    );
    if vp.is_null() {
        eprintln!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (null) -> (1,1)");
        return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
    }

    let vp_ref = unsafe { &*vp };
    let vd_ptr = vp_ref.PlatformUserData as *mut ViewportData;
    if vd_ptr.is_null() {
        eprintln!(
            "[winit-mv] LEAVE winit_get_window_framebuffer_scale (no PlatformUserData) -> (1,1)"
        );
        return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
    }
    if let Some(vd) = unsafe { vd_ptr.as_ref() } {
        if vd.window.is_null() {
            eprintln!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (no window) -> (1,1)");
            return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
        }
        if let Some(window) = unsafe { vd.window.as_ref() } {
            let scale = window.scale_factor() as f32;
            eprintln!(
                "[winit-mv] LEAVE winit_get_window_framebuffer_scale -> ({:.2}, {:.2})",
                scale, scale
            );
            return dear_imgui::sys::ImVec2 { x: scale, y: scale };
        }
    }
    eprintln!("[winit-mv] LEAVE winit_get_window_framebuffer_scale (fallback) -> (1,1)");
    dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 }
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
unsafe extern "C" fn winit_update_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    // For now, this is a no-op. In GLFW implementation, this is used for
    // platform-specific window updates. Winit handles most of this automatically.
    // We might need to add specific logic here later for things like:
    // - Window state synchronization
    // - Platform-specific optimizations
    // - Event processing
}
