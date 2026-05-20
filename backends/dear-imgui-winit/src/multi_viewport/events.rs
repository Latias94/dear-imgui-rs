use super::registry::viewport_data_ref;
use super::viewport_data::client_to_screen_pos;
use super::*;
use winit::event::{Event, WindowEvent};

pub fn route_event_to_viewports<T>(imgui_ctx: &mut Context, event: &Event<T>) -> bool {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_ctx.as_raw()) };

    match event {
        Event::WindowEvent { window_id, event } => {
            // Iterate ImGui viewports and find the window that matches this event's WindowId
            #[cfg(feature = "multi-viewport")]
            {
                unsafe {
                    let pio = dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(imgui_ctx.as_raw());
                    let viewports = &(*pio).Viewports;
                    if viewports.Data.is_null() || viewports.Size <= 0 {
                        return false;
                    }
                    for i in 0..viewports.Size {
                        let vp = *viewports.Data.add(i as usize);
                        if vp.is_null() {
                            continue;
                        }
                        if let Some(vd) = viewport_data_ref(vp) {
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
