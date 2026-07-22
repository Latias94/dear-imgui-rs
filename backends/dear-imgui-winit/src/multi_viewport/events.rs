use dear_imgui_rs::Context;
use winit::event::{Event, WindowEvent};

use super::registry::with_viewport_data;
use super::runtime::{RuntimeControl, WinitPlatformRuntime};
use super::viewport_data::classify_window_event_echo;
use crate::sanitize;

pub(super) fn route_secondary_event<T>(
    control: &RuntimeControl,
    context: &mut Context,
    event: &Event<T>,
) -> bool {
    let binding = context.binding();
    binding.with_bound_context(|| match event {
        Event::WindowEvent { window_id, event } => unsafe {
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw());
            if platform_io.is_null() {
                return false;
            }
            let viewports = &(*platform_io).Viewports;
            if viewports.Data.is_null() || viewports.Size <= 0 {
                return false;
            }

            for index in 0..viewports.Size {
                let viewport = *viewports.Data.add(index as usize);
                if viewport.is_null() {
                    continue;
                }
                let Some(consumed) = with_viewport_data(control, viewport, |data| {
                    if data.is_main() || data.window().id() != *window_id {
                        return None;
                    }

                    let window = data.window();
                    match event {
                        WindowEvent::Moved(position) => {
                            let frame = dear_imgui_rs::sys::igGetFrameCount();
                            let (report, pending) = classify_window_event_echo(
                                frame,
                                data.pending_window_pos_echo.get(),
                                [i64::from(position.x), i64::from(position.y)],
                            );
                            data.pending_window_pos_echo.set(pending);
                            if report {
                                (*viewport).PlatformRequestMove = true;
                            }
                        }
                        WindowEvent::Resized(size) => {
                            let frame = dear_imgui_rs::sys::igGetFrameCount();
                            let (report, pending) = classify_window_event_echo(
                                frame,
                                data.pending_window_size_echo.get(),
                                [i64::from(size.width), i64::from(size.height)],
                            );
                            data.pending_window_size_echo.set(pending);
                            if report {
                                (*viewport).PlatformRequestResize = true;
                            }
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            let scale =
                                sanitize::positive_finite_f32_or(window.scale_factor() as f32, 1.0);
                            (*viewport).DpiScale = scale;
                            (*viewport).FramebufferScale.x = scale;
                            (*viewport).FramebufferScale.y = scale;
                        }
                        WindowEvent::CloseRequested => {
                            (*viewport).PlatformRequestClose = true;
                        }
                        _ => {}
                    }

                    let consumed = match event {
                        WindowEvent::KeyboardInput { event, .. } => {
                            crate::events::handle_keyboard_input(event, context)
                        }
                        WindowEvent::ModifiersChanged(modifiers) => {
                            crate::events::handle_modifiers_changed(modifiers, context);
                            context.io().want_capture_keyboard()
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            crate::events::handle_mouse_wheel(*delta, context)
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            crate::events::handle_mouse_button(*button, *state, context)
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            let position = position.to_logical::<f64>(
                                sanitize::positive_finite_or(window.scale_factor(), 1.0),
                            );
                            let local = [position.x, position.y];
                            let Some(position) =
                                super::viewport_data::client_to_screen_pos(window, local)
                            else {
                                return Some(context.io().want_capture_mouse());
                            };
                            context
                                .io_mut()
                                .add_mouse_viewport_event(dear_imgui_rs::Id::from((*viewport).ID));
                            crate::events::handle_cursor_moved(
                                [position[0] as f64, position[1] as f64],
                                context,
                            )
                        }
                        WindowEvent::CursorLeft { .. } => {
                            let io = context.io_mut();
                            io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                            io.add_mouse_viewport_event(dear_imgui_rs::Id::default());
                            false
                        }
                        WindowEvent::Focused(focused) => {
                            crate::events::handle_focused(*focused, context)
                        }
                        WindowEvent::Ime(ime) => {
                            crate::events::handle_ime_event(ime, context);
                            context.io().want_capture_keyboard()
                        }
                        WindowEvent::Touch(touch) => {
                            let position = crate::events::touch_logical_position(touch, window)
                                .and_then(|local| {
                                    super::viewport_data::client_to_screen_pos(
                                        window,
                                        [f64::from(local[0]), f64::from(local[1])],
                                    )
                                });
                            let _ = crate::events::handle_touch_event_at(
                                touch,
                                position,
                                Some(dear_imgui_rs::Id::from((*viewport).ID)),
                                context,
                            );
                            context.io().want_capture_mouse()
                        }
                        _ => false,
                    };
                    Some(consumed)
                }) else {
                    continue;
                };

                if let Some(consumed) = consumed {
                    return consumed;
                }
            }
            false
        },
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
