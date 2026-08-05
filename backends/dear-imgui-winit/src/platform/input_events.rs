use dear_imgui_rs::Context;
use winit::event::{Event, WindowEvent};
use winit::window::Window;

use crate::events;
use crate::sanitize;

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl};

impl WinitPlatformControl {
    #[cfg(feature = "multi-viewport")]
    fn note_runtime_key(&self, window_id: winit::window::WindowId, event: &winit::event::KeyEvent) {
        let Some(key) =
            crate::input::winit_key_to_imgui_key(&event.logical_key, event.physical_key)
        else {
            return;
        };
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_key(window_id, key, event.state.is_pressed());
        }
    }

    fn note_single_touch(&self, touch: &winit::event::Touch) -> Option<events::TouchAction> {
        let (next_active, action) =
            events::touch_transition(self.active_touch.get(), touch.id, touch.phase);
        self.active_touch.set(next_active);
        action
    }

    pub(super) fn release_single_touch_in_current_context(&self) {
        if self.active_touch.take().is_none() {
            return;
        }
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
        if io.is_null() {
            return;
        }
        unsafe {
            dear_imgui_rs::sys::ImGuiIO_AddMouseSourceEvent(
                io,
                dear_imgui_rs::input::MouseSource::TouchScreen.into(),
            );
            dear_imgui_rs::sys::ImGuiIO_AddMouseButtonEvent(
                io,
                dear_imgui_rs::input::MouseButton::Left.into(),
                false,
            );
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_modifiers(
        &self,
        window_id: winit::window::WindowId,
        modifiers: &winit::event::Modifiers,
    ) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            for (key, pressed) in crate::events::modifier_key_events(modifiers) {
                runtime.note_key(window_id, key, pressed);
            }
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_mouse_button(
        &self,
        window_id: winit::window::WindowId,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        let Some(button) = crate::input::to_imgui_mouse_button(button) else {
            return;
        };
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_mouse_button(window_id, button, state.is_pressed());
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_touch(
        &self,
        window_id: winit::window::WindowId,
        touch: &winit::event::Touch,
    ) -> Option<events::TouchAction> {
        self.runtime
            .borrow()
            .as_ref()
            .and_then(|runtime| runtime.note_touch(window_id, touch.id, touch.phase))
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_cursor_left(&self) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_cursor_left();
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_cursor_available(&self) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_cursor_available();
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_window_focus(
        &self,
        window_id: winit::window::WindowId,
        focused: bool,
        context: &mut Context,
    ) -> bool {
        let runtime = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .cloned();
        let Some(runtime) = runtime else {
            return false;
        };
        runtime.note_window_focus(window_id, focused, context);
        true
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_window_geometry(
        &self,
        window_id: winit::window::WindowId,
        position: bool,
        size: bool,
    ) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_window_geometry(window_id, position, size);
        }
    }
}

impl WinitPlatform {
    /// Handle a winit event.
    ///
    /// This is the most general entry point: pass the full `Event<T>` from
    /// your event loop and the backend will dispatch to the appropriate
    /// handlers. For `ApplicationHandler::window_event`, where you already
    /// receive a `WindowEvent` for a specific window, you can use
    /// `handle_window_event` instead and avoid constructing a synthetic
    /// `Event::WindowEvent`.
    pub fn handle_event<T>(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &Event<T>,
    ) -> Result<bool, WinitPlatformError> {
        if !event_targets_window(window.id(), event) {
            return Ok(false);
        }
        self.control.validate_entry(imgui_ctx, window)?;
        Ok(match event {
            Event::WindowEvent { event, .. } => {
                self.handle_window_event_internal(imgui_ctx, window, event)
            }
            Event::DeviceEvent { event, .. } => {
                events::handle_device_event(event);
                false
            }
            _ => false,
        })
    }

    /// Handle a single window event for a given window.
    ///
    /// This is a convenience wrapper for frameworks that already route
    /// window-local events, such as winit's `ApplicationHandler::window_event`,
    /// and don't need to build a full `Event::WindowEvent` value.
    pub fn handle_window_event(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> Result<bool, WinitPlatformError> {
        self.control.validate_entry(imgui_ctx, window)?;
        Ok(self.handle_window_event_internal(imgui_ctx, window, event))
    }

    /// Internal implementation for window event handling.
    fn handle_window_event_internal(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::Resized(physical_size) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_window_geometry(window.id(), false, true);
                    let io = imgui_ctx.io_mut();
                    io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                    io.set_display_framebuffer_scale(
                        crate::multi_viewport::framebuffer_scale_for_window(window),
                    );
                    return false;
                }
                let logical_size = physical_size
                    .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let logical_size = self.scale_size_from_winit(window, logical_size);
                imgui_ctx
                    .io_mut()
                    .set_display_size(sanitize::finite_non_negative_size(logical_size));
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let new_hidpi = self.hidpi_factor_for_scale(*scale_factor);
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_window_geometry(window.id(), true, true);
                    // Native desktop coordinates do not change when a viewport crosses a DPI
                    // boundary. Only its UI DPI and framebuffer relation change.
                    self.hidpi_factor = new_hidpi;
                    let io = imgui_ctx.io_mut();
                    io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                    io.set_display_framebuffer_scale(
                        crate::multi_viewport::framebuffer_scale_for_window(window),
                    );
                    return false;
                }
                // Adjust mouse position proportionally when DPI factor changes
                {
                    let io = imgui_ctx.io_mut();
                    let mouse = io.mouse_pos();
                    if let Some(scaled) =
                        rescale_mouse_pos_for_hidpi_change(mouse, self.hidpi_factor, new_hidpi)
                    {
                        io.set_mouse_pos(scaled);
                    }
                }
                self.hidpi_factor = new_hidpi;

                let logical_size = window
                    .inner_size()
                    .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let logical_size = self.scale_size_from_winit(window, logical_size);
                let io = imgui_ctx.io_mut();
                io.set_display_size(sanitize::finite_non_negative_size(logical_size));
                io.set_display_framebuffer_scale(sanitize::framebuffer_scale(
                    self.hidpi_factor,
                    1.0,
                ));
                false
            }
            WindowEvent::KeyboardInput { event, .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_key(window.id(), event);
                }
                events::handle_keyboard_input(event, imgui_ctx)
            }
            WindowEvent::CursorMoved { position, .. } => {
                #[cfg(feature = "multi-viewport")]
                {
                    if self.control.has_live_runtime() {
                        self.control.note_runtime_cursor_available();
                        let Some(position) = crate::multi_viewport::client_physical_to_screen_pos(
                            window,
                            [position.x, position.y],
                        ) else {
                            return imgui_ctx.io().want_capture_mouse();
                        };
                        return events::handle_cursor_moved(
                            [f64::from(position[0]), f64::from(position[1])],
                            imgui_ctx,
                        );
                    }
                }
                // Fallback: local logical coordinates
                let position =
                    position.to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let position = self.scale_pos_from_winit(window, position);
                events::handle_cursor_moved([position.x, position.y], imgui_ctx)
            }
            WindowEvent::MouseInput { button, state, .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_mouse_button(window.id(), *button, *state);
                }
                events::handle_mouse_button(*button, *state, imgui_ctx)
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                events::handle_mouse_wheel(*delta, *phase, window.scale_factor(), imgui_ctx)
            }
            // Single-window mode invalidates immediately. Multi-viewport mode delays the leave
            // so an in-flight drag can enter another owned native window without losing position.
            WindowEvent::CursorLeft { .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_cursor_left();
                    return false;
                }
                {
                    let io = imgui_ctx.io_mut();
                    io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                }
                false
            }
            #[cfg(feature = "multi-viewport")]
            WindowEvent::Moved(_) if self.control.has_live_runtime() => {
                self.control
                    .note_runtime_window_geometry(window.id(), true, false);
                false
            }
            WindowEvent::CursorEntered { .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_cursor_available();
                }
                false
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_modifiers(window.id(), modifiers);
                }
                events::handle_modifiers_changed(modifiers, imgui_ctx);
                false
            }
            WindowEvent::Ime(ime) => {
                events::handle_ime_event(ime, imgui_ctx);
                // Track IME enabled/disabled state based on winit notifications.
                self.ime_enabled = !matches!(ime, winit::event::Ime::Disabled);
                imgui_ctx.io().want_capture_keyboard()
            }
            WindowEvent::Touch(touch) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    let position = crate::multi_viewport::client_physical_to_screen_pos(
                        window,
                        [touch.location.x, touch.location.y],
                    );
                    if position.is_some()
                        || !matches!(
                            touch.phase,
                            winit::event::TouchPhase::Started | winit::event::TouchPhase::Moved
                        )
                    {
                        let action = self.control.note_runtime_touch(window.id(), touch);
                        if let Some(action) = action {
                            let _ = events::handle_touch_event_at(
                                action,
                                position,
                                Some(imgui_ctx.main_viewport().id()),
                                imgui_ctx,
                            );
                        }
                    }
                    return imgui_ctx.io().want_capture_mouse();
                }
                let position = events::touch_logical_position(touch, window);
                if position.is_some()
                    || !matches!(
                        touch.phase,
                        winit::event::TouchPhase::Started | winit::event::TouchPhase::Moved
                    )
                {
                    if let Some(action) = self.control.note_single_touch(touch) {
                        let _ = events::handle_touch_event_at(action, position, None, imgui_ctx);
                    }
                }
                imgui_ctx.io().want_capture_mouse()
            }
            WindowEvent::Focused(focused) => {
                #[cfg(feature = "multi-viewport")]
                if self
                    .control
                    .note_runtime_window_focus(window.id(), *focused, imgui_ctx)
                {
                    return false;
                }
                events::handle_focused(*focused, imgui_ctx)
            }
            _ => false,
        }
    }
}

pub(super) fn event_targets_window<T>(
    window_id: winit::window::WindowId,
    event: &Event<T>,
) -> bool {
    !matches!(
        event,
        Event::WindowEvent {
            window_id: event_window_id,
            ..
        } if *event_window_id != window_id
    )
}

pub(super) fn rescale_mouse_pos_for_hidpi_change(
    mouse: [f32; 2],
    old_hidpi: f64,
    new_hidpi: f64,
) -> Option<[f32; 2]> {
    let mouse = sanitize::finite_vec2_f32(mouse)?;
    let old_hidpi = sanitize::positive_finite_or(old_hidpi, 1.0);
    let scale = sanitize::positive_finite_or(new_hidpi / old_hidpi, 1.0);
    sanitize::finite_vec2_f32([mouse[0] * scale as f32, mouse[1] * scale as f32])
}
