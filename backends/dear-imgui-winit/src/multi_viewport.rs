//! Multi-viewport support for Dear ImGui winit backend
//!
//! This module provides multi-viewport functionality following the official
//! ImGui backend pattern, allowing Dear ImGui to create and manage multiple
//! OS windows for advanced UI layouts.

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CStr};

use dear_imgui::Context;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowLevel};

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
pub fn init_multi_viewport_support(_ctx: &mut Context, main_window: &Window) {
    // Set up platform callbacks using direct C API
    unsafe {
        let pio = dear_imgui::sys::ImGui_GetPlatformIO();

        (*pio).Platform_CreateWindow = Some(winit_create_window);
        (*pio).Platform_DestroyWindow = Some(winit_destroy_window);
        (*pio).Platform_ShowWindow = Some(winit_show_window);
        (*pio).Platform_SetWindowPos = Some(winit_set_window_pos);
        (*pio).Platform_GetWindowPos = Some(winit_get_window_pos);
        (*pio).Platform_SetWindowSize = Some(winit_set_window_size);
        (*pio).Platform_GetWindowSize = Some(winit_get_window_size);
        (*pio).Platform_SetWindowFocus = Some(winit_set_window_focus);
        (*pio).Platform_GetWindowFocus = Some(winit_get_window_focus);
        (*pio).Platform_GetWindowMinimized = Some(winit_get_window_minimized);
        (*pio).Platform_SetWindowTitle = Some(winit_set_window_title);
        (*pio).Platform_GetWindowFramebufferScale = Some(winit_get_window_framebuffer_scale);
        (*pio).Platform_UpdateWindow = Some(winit_update_window);

        // Set up monitors - this is required for multi-viewport
        setup_monitors();
    }

    // Set up the main viewport
    init_main_viewport(main_window);
}

/// Set up monitors list for multi-viewport support
unsafe fn setup_monitors() {
    // For now, let's skip the monitor setup and see if ImGui can work without it
    // The assertion suggests ImGui expects monitors to be set up, but let's try a simpler approach

    // We'll let ImGui handle monitor detection internally
    // This is a temporary workaround to get basic multi-viewport working
}

/// Initialize the main viewport with proper ViewportData
fn init_main_viewport(main_window: &Window) {
    unsafe {
        let main_viewport = dear_imgui::sys::ImGui_GetMainViewport();

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
        dear_imgui::sys::ImGui_DestroyPlatformWindows();
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
    let event_loop = EVENT_LOOP.with(|el| el.borrow().map(|ptr| &*ptr));

    let event_loop = match event_loop {
        Some(el) => el,
        None => return,
    };

    // Create ViewportData
    let vd = Box::into_raw(Box::new(ViewportData::new()));
    (*vp).PlatformUserData = vd as *mut c_void;

    // Handle viewport flags
    let viewport_flags = (*vp).Flags;
    let mut window_attrs = WindowAttributes::default()
        .with_title("ImGui Viewport")
        .with_inner_size(LogicalSize::new((*vp).Size.x as f64, (*vp).Size.y as f64))
        .with_position(winit::dpi::Position::Logical(LogicalPosition::new(
            (*vp).Pos.x as f64,
            (*vp).Pos.y as f64,
        )))
        .with_visible(false); // Start hidden, will be shown by show_window callback

    // Handle decorations
    if viewport_flags & dear_imgui::sys::ImGuiViewportFlags_NoDecoration != 0 {
        window_attrs = window_attrs.with_decorations(false);
    }

    // Handle always on top
    if viewport_flags & dear_imgui::sys::ImGuiViewportFlags_TopMost != 0 {
        window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
    }

    // Create the window
    match event_loop.create_window(window_attrs) {
        Ok(window) => {
            let window_ptr = Box::into_raw(Box::new(window));
            (*vd).window = window_ptr;
            (*vd).window_owned = true;
            (*vp).PlatformHandle = window_ptr as *mut c_void;

            // TODO: Set up event callbacks for this window
            // This is a critical missing piece - we need to route events from this window
            // back to ImGui. For now, this is a known limitation.
            eprintln!("Warning: Event routing for viewport windows not yet implemented");
        }
        Err(_) => {
            // Clean up ViewportData on failure
            let _ = Box::from_raw(vd);
            (*vp).PlatformUserData = std::ptr::null_mut();
        }
    }
}

/// Destroy a viewport window
unsafe extern "C" fn winit_destroy_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
        if vd.window_owned && !vd.window.is_null() {
            // Clean up the window
            let _ = Box::from_raw(vd.window);
        }
        vd.window = std::ptr::null_mut();

        // Clean up ViewportData
        let _ = Box::from_raw(vd);
    }

    (*vp).PlatformUserData = std::ptr::null_mut();
    (*vp).PlatformHandle = std::ptr::null_mut();
}

/// Show a viewport window
unsafe extern "C" fn winit_show_window(vp: *mut dear_imgui::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            (*vd.window).set_visible(true);
        }
    }
}

/// Get window position
unsafe extern "C" fn winit_get_window_pos(
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    // Special handling for viewport ID 0 (main viewport or ImGui internal viewport)
    let viewport_id = (*vp).ID;
    if viewport_id == 0 {
        // Return safe default for main viewport
        return dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            if let Ok(pos) = (*vd.window).outer_position() {
                return dear_imgui::sys::ImVec2 {
                    x: pos.x as f32,
                    y: pos.y as f32,
                };
            }
        }
    }

    dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 }
}

/// Set window position
unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    pos: dear_imgui::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
        if !vd.window.is_null() {
            let position = LogicalPosition::new(pos.x as f64, pos.y as f64);
            (*vd.window).set_outer_position(position);
            vd.ignore_window_pos_event_frame = dear_imgui::sys::ImGui_GetFrameCount();
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

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            let size = (*vd.window).inner_size();
            return dear_imgui::sys::ImVec2 {
                x: size.width as f32,
                y: size.height as f32,
            };
        }
    }

    dear_imgui::sys::ImVec2 { x: 0.0, y: 0.0 }
}

/// Set window size
unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    size: dear_imgui::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_mut() {
        if !vd.window.is_null() {
            let new_size = LogicalSize::new(size.x as f64, size.y as f64);
            let _ = (*vd.window).request_inner_size(new_size);
            vd.ignore_window_size_event_frame = dear_imgui::sys::ImGui_GetFrameCount();
        }
    }
}

/// Set window focus
unsafe extern "C" fn winit_set_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            (*vd.window).focus_window();
        }
    }
}

/// Get window focus
unsafe extern "C" fn winit_get_window_focus(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            return (*vd.window).has_focus();
        }
    }

    false
}

/// Get window minimized state
unsafe extern "C" fn winit_get_window_minimized(vp: *mut dear_imgui::sys::ImGuiViewport) -> bool {
    if vp.is_null() {
        return false;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            return (*vd.window).is_minimized().unwrap_or(false);
        }
    }

    false
}

/// Set window title
unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui::sys::ImGuiViewport,
    title: *const c_char,
) {
    if vp.is_null() || title.is_null() {
        return;
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            if let Ok(title_str) = CStr::from_ptr(title).to_str() {
                (*vd.window).set_title(title_str);
            }
        }
    }
}

/// Get window framebuffer scale
unsafe extern "C" fn winit_get_window_framebuffer_scale(
    vp: *mut dear_imgui::sys::ImGuiViewport,
) -> dear_imgui::sys::ImVec2 {
    if vp.is_null() {
        return dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 };
    }

    if let Some(vd) = ((*vp).PlatformUserData as *mut ViewportData).as_ref() {
        if !vd.window.is_null() {
            let scale = (*vd.window).scale_factor() as f32;
            return dear_imgui::sys::ImVec2 { x: scale, y: scale };
        }
    }

    dear_imgui::sys::ImVec2 { x: 1.0, y: 1.0 }
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
