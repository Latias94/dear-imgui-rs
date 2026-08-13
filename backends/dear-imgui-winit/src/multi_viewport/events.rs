use dear_imgui_rs::Context;
use winit::event::{Event, WindowEvent};

use super::registry::with_viewport_data_for_window;
use super::runtime::{RuntimeControl, RuntimeState};

fn validate_runtime(
    control: &RuntimeControl,
    context: &Context,
) -> Result<(), super::WinitPlatformError> {
    control.ensure_context(context)?;
    control.poll_fault()?;
    if control.state() != RuntimeState::Attached {
        return control
            .poll_fault()
            .and(Err(super::WinitPlatformError::RuntimeDetached));
    }
    control.platform_control()?.validate_operational_contract()
}

fn secondary_event_requests_main_redraw(
    event: &WindowEvent,
    has_client_geometry_reconciliation: bool,
) -> bool {
    matches!(
        event,
        WindowEvent::Moved(_)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::CloseRequested
            | WindowEvent::RedrawRequested
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::Ime(_)
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::Focused(_)
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::Touch(_)
    ) || has_client_geometry_reconciliation && matches!(event, WindowEvent::Occluded(_))
}

pub(crate) fn route_secondary_window_event(
    control: &RuntimeControl,
    context: &mut Context,
    window_id: winit::window::WindowId,
    event: &WindowEvent,
) -> Result<bool, super::WinitPlatformError> {
    control.ensure_context(context)?;
    let binding = context.binding();
    let mut request_main_redraw = false;
    let consumed = binding.with_bound_context(|| {
        with_viewport_data_for_window(control, window_id, |viewport, data| {
            let window = data.window();
            let has_client_geometry_reconciliation = data.has_client_geometry_reconciliation();
            request_main_redraw |=
                secondary_event_requests_main_redraw(event, has_client_geometry_reconciliation);
            // Keep DPI and framebuffer values owned by Dear ImGui's platform update state
            // machine. The event only invalidates geometry so the registered callbacks query
            // the new values at the next update boundary.
            match event {
                WindowEvent::Moved(_) => {
                    data.note_geometry_event();
                    data.request_geometry_refresh(true, false);
                }
                WindowEvent::Resized(_) => {
                    data.note_geometry_event();
                    data.request_geometry_refresh(false, true);
                }
                WindowEvent::ScaleFactorChanged {
                    inner_size_writer, ..
                } => {
                    if let Some(size) = super::scale_factor_inner_size_override(
                        context.io().config_dpi_scale_viewports(),
                        window.inner_size(),
                    ) {
                        let mut inner_size_writer = inner_size_writer.clone();
                        let _ = inner_size_writer.request_inner_size(size);
                    }
                    data.note_geometry_event();
                    data.request_geometry_refresh(true, true);
                }
                WindowEvent::Occluded(_) if has_client_geometry_reconciliation => {
                    // X11 reports its first visibility state after the map request, including
                    // fully obscured windows. Winit updates its internal visibility after this
                    // callback returns, so schedule another observation without treating the
                    // visibility notification as authoritative geometry.
                    data.request_geometry_refresh(true, true);
                }
                WindowEvent::CloseRequested => unsafe {
                    (*viewport).PlatformRequestClose = true;
                },
                _ => {}
            }

            match event {
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Some(key) =
                        crate::input::winit_key_to_imgui_key(&event.logical_key, event.physical_key)
                    {
                        control.note_key(window_id, key, event.state.is_pressed());
                    }
                    crate::events::handle_keyboard_input(event, context)
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    for (key, pressed) in crate::events::modifier_key_events(modifiers) {
                        control.note_key(window_id, key, pressed);
                    }
                    crate::events::handle_modifiers_changed(modifiers, context);
                    context.io().want_capture_keyboard()
                }
                WindowEvent::MouseWheel { delta, phase, .. } => crate::events::handle_mouse_wheel(
                    *delta,
                    *phase,
                    window.scale_factor(),
                    context,
                ),
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(button) = crate::input::to_imgui_mouse_button(*button) {
                        control.note_mouse_button(window_id, button, state.is_pressed());
                    }
                    crate::events::handle_mouse_button(*button, *state, context)
                }
                WindowEvent::CursorMoved { position, .. } => {
                    control.note_cursor_available();
                    let Some(position) =
                        super::client_physical_to_screen_pos(window, [position.x, position.y])
                    else {
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
                    control.note_window_focus(window_id, *focused, context);
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
                    if (position.is_some()
                        || !matches!(
                            touch.phase,
                            winit::event::TouchPhase::Started | winit::event::TouchPhase::Moved
                        ))
                        && let Some(action) = control.note_touch(window_id, touch.id, touch.phase)
                    {
                        let _ = crate::events::handle_touch_event_at(
                            action,
                            position,
                            Some(dear_imgui_rs::Id::from(unsafe { (*viewport).ID })),
                            context,
                        );
                    }
                    context.io().want_capture_mouse()
                }
                _ => false,
            }
        })
        .unwrap_or(false)
    });
    if request_main_redraw {
        control.request_main_window_redraw();
    }
    Ok(consumed)
}

pub(super) fn route_secondary_event<T>(
    control: &RuntimeControl,
    context: &mut Context,
    event: &Event<T>,
) -> Result<bool, super::WinitPlatformError> {
    match event {
        Event::WindowEvent { window_id, event } => {
            route_secondary_window_event(control, context, *window_id, event)
        }
        _ => {
            control.ensure_context(context)?;
            Ok(false)
        }
    }
}

pub(crate) fn handle_event<T>(
    control: &RuntimeControl,
    platform: &mut crate::WinitPlatform,
    context: &mut Context,
    event: &Event<T>,
) -> Result<bool, super::WinitPlatformError> {
    validate_runtime(control, context)?;
    let mut consumed = false;
    let Some(main_window) = control.main_window() else {
        return Ok(false);
    };
    match event {
        Event::WindowEvent { window_id, .. } if *window_id == main_window.id() => {
            consumed = platform.handle_main_event(context, &main_window, event)?;
        }
        Event::WindowEvent { .. } => {}
        _ => consumed = platform.handle_main_event(context, &main_window, event)?,
    }

    let consumed = route_secondary_event(control, context, event)? || consumed;
    control.poll_fault()?;
    Ok(consumed)
}

#[cfg(test)]
mod tests {
    use super::secondary_event_requests_main_redraw;
    use winit::event::WindowEvent;

    #[test]
    fn secondary_close_and_redraw_wake_the_main_window() {
        assert!(secondary_event_requests_main_redraw(
            &WindowEvent::CloseRequested,
            false,
        ));
        assert!(secondary_event_requests_main_redraw(
            &WindowEvent::RedrawRequested,
            false,
        ));
    }

    #[test]
    fn either_x11_occlusion_state_wakes_pending_reconciliation() {
        for occluded in [false, true] {
            assert!(secondary_event_requests_main_redraw(
                &WindowEvent::Occluded(occluded),
                true,
            ));
            assert!(!secondary_event_requests_main_redraw(
                &WindowEvent::Occluded(occluded),
                false,
            ));
        }
    }
}
