use dear_imgui_rs::Context;
use winit::event::{Event, WindowEvent};

use super::registry::with_viewport_data_for_window;
use super::runtime::{RuntimeControl, WinitPlatformRuntime};

pub(super) fn route_secondary_event<T>(
    control: &RuntimeControl,
    context: &mut Context,
    event: &Event<T>,
) -> bool {
    let binding = context.binding();
    binding.with_bound_context(|| match event {
        Event::WindowEvent { window_id, event } => {
            let Some(consumed) =
                with_viewport_data_for_window(control, *window_id, |viewport, data| {
                    let window = data.window();
                    // Keep DPI and framebuffer values owned by Dear ImGui's platform update state
                    // machine. The event only invalidates geometry so the registered callbacks query
                    // the new values at the next update boundary.
                    match event {
                        WindowEvent::Moved(_) => unsafe {
                            data.request_geometry_refresh(true, false);
                            (*viewport).PlatformRequestMove = true;
                        },
                        WindowEvent::Resized(_) => unsafe {
                            data.request_geometry_refresh(false, true);
                            (*viewport).PlatformRequestResize = true;
                        },
                        WindowEvent::ScaleFactorChanged { .. } => unsafe {
                            data.request_geometry_refresh(true, true);
                            (*viewport).PlatformRequestMove = true;
                            (*viewport).PlatformRequestResize = true;
                        },
                        WindowEvent::CloseRequested => unsafe {
                            (*viewport).PlatformRequestClose = true;
                        },
                        _ => {}
                    }

                    match event {
                        WindowEvent::KeyboardInput { event, .. } => {
                            if let Some(key) = crate::input::winit_key_to_imgui_key(
                                &event.logical_key,
                                event.location,
                            ) {
                                control.note_key(*window_id, key, event.state.is_pressed());
                            }
                            crate::events::handle_keyboard_input(event, context)
                        }
                        WindowEvent::ModifiersChanged(modifiers) => {
                            for (key, pressed) in crate::events::modifier_key_events(modifiers) {
                                control.note_key(*window_id, key, pressed);
                            }
                            crate::events::handle_modifiers_changed(modifiers, context);
                            context.io().want_capture_keyboard()
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            crate::events::handle_mouse_wheel(*delta, context)
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            if let Some(button) = crate::input::to_imgui_mouse_button(*button) {
                                control.note_mouse_button(*window_id, button, state.is_pressed());
                            }
                            crate::events::handle_mouse_button(*button, *state, context)
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            control.note_cursor_available();
                            let Some(position) = super::client_physical_to_screen_pos(
                                window,
                                [position.x, position.y],
                            ) else {
                                return context.io().want_capture_mouse();
                            };
                            crate::events::handle_cursor_moved(
                                [position[0] as f64, position[1] as f64],
                                context,
                            )
                        }
                        WindowEvent::CursorLeft { .. } => {
                            control.note_cursor_left();
                            false
                        }
                        WindowEvent::CursorEntered { .. } => {
                            control.note_cursor_available();
                            false
                        }
                        WindowEvent::Focused(focused) => {
                            control.note_window_focus(*window_id, *focused, context);
                            false
                        }
                        WindowEvent::Ime(ime) => {
                            crate::events::handle_ime_event(ime, context);
                            context.io().want_capture_keyboard()
                        }
                        WindowEvent::Touch(touch) => {
                            let position = super::client_physical_to_screen_pos(
                                window,
                                [touch.location.x, touch.location.y],
                            );
                            let _ = crate::events::handle_touch_event_at(
                                touch,
                                position,
                                Some(dear_imgui_rs::Id::from(unsafe { (*viewport).ID })),
                                context,
                            );
                            context.io().want_capture_mouse()
                        }
                        _ => false,
                    }
                })
            else {
                return false;
            };
            consumed
        }
        _ => false,
    })
}

pub(super) fn handle_event<T>(
    runtime: &WinitPlatformRuntime,
    platform: &mut crate::WinitPlatform,
    context: &mut Context,
    event: &Event<T>,
) -> Result<bool, super::WinitPlatformError> {
    let mut consumed = false;
    let Some(main_window) = runtime.control().main_window() else {
        return Ok(false);
    };
    if let Event::WindowEvent { window_id, .. } = event
        && *window_id == main_window.id()
        && platform.handle_event(context, &main_window, event)?
    {
        consumed = true;
    }

    Ok(route_secondary_event(runtime.control(), context, event) || consumed)
}
