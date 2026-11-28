use dear_imgui_rs::Context;
use winit::dpi::PhysicalPosition;
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowId};

use crate::events;
use crate::sanitize;

use super::ownership::{WinitPlatform, WinitPlatformControl};
use super::{
    WinitCaptureIntent, WinitEventForwarding, WinitEventOutcome, WinitEventTarget,
    WinitPlatformAction, WinitPlatformError,
};

impl WinitPlatformControl {
    fn activate_ime_source(
        &self,
        context: &mut Context,
        source: dear_imgui_rs::NativeWindowInstanceId,
    ) {
        if self
            .ime_event_source
            .replace(Some(source))
            .is_some_and(|previous| previous != source)
        {
            // An already enabled second window need not emit another Enabled.
            // Its events cannot be commits from the cancelled native source.
            context.notify_ime_enabled(true);
        }
    }

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

    fn handle_single_window_focus(
        &self,
        focused: bool,
        context: &mut Context,
    ) -> WinitEventForwarding {
        let released_touch = if !focused {
            self.last_local_pointer.set(None);
            context
                .binding()
                .with_bound_context(|| self.release_single_touch_in_current_context())
        } else {
            false
        };
        let mut forwarded = events::handle_focused(focused, context);
        forwarded.pointer |= released_touch;
        forwarded
    }

    pub(super) fn release_single_touch_in_current_context(&self) -> bool {
        if self.active_touch.take().is_none() {
            return false;
        }
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
        if io.is_null() {
            return false;
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
        true
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
    ) -> Option<bool> {
        let runtime = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .cloned()?;
        Some(runtime.note_window_focus(window_id, focused, context))
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
    /// Routes a full event by its native WindowId. Raw device events remain host-owned.
    pub fn handle_event<T>(
        &mut self,
        imgui_ctx: &mut Context,
        event: &Event<T>,
    ) -> Result<WinitEventOutcome, WinitPlatformError> {
        match event {
            Event::WindowEvent { window_id, event } => {
                self.handle_window_event(imgui_ctx, *window_id, event)
            }
            _ => {
                self.control.ensure_context(imgui_ctx)?;
                self.control.validate_operational_contract()?;
                Ok(WinitEventOutcome::default())
            }
        }
    }

    /// Routes an ApplicationHandler event without requiring a Window borrow or synthetic event.
    ///
    /// Foreign window identifiers return an unowned outcome. Capture intent describes the most
    /// recent UI frame and never means that this individual event was consumed.
    pub fn handle_window_event(
        &mut self,
        imgui_ctx: &mut Context,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> Result<WinitEventOutcome, WinitPlatformError> {
        self.control.ensure_context(imgui_ctx)?;
        let Some(snapshot) = self.window_snapshot(window_id)? else {
            return Ok(WinitEventOutcome::default());
        };
        #[cfg(feature = "multi-viewport")]
        let runtime = if self.control.has_live_runtime() {
            let runtime = self.control.runtime_control()?;
            runtime.poll_fault()?;
            Some(runtime)
        } else {
            None
        };
        let capture = WinitCaptureIntent::snapshot(imgui_ctx);
        let pointer_release = super::pointer_control::release_reason(event).and_then(|reason| {
            self.control
                .release_pointer_control(reason, Some(snapshot.target.instance_id))
        });
        let owns_text_input = imgui_ctx.text_input_viewport() == Some(snapshot.target.viewport_id);
        let owns_ime_session =
            imgui_ctx.ime_session_viewport() == Some(snapshot.target.viewport_id);
        if (owns_text_input || owns_ime_session)
            && matches!(event, WindowEvent::Focused(false) | WindowEvent::Destroyed)
        {
            imgui_ctx.clear_ime_preedit();
        }
        let forwarded = if let WindowEvent::Ime(ime) = event {
            let accepts_ime = {
                let mut enabled = self.control.ime_enabled_windows.borrow_mut();
                match ime {
                    winit::event::Ime::Enabled => {
                        if self.control.ime_allowed.get() {
                            enabled.insert(snapshot.target.instance_id);
                            true
                        } else {
                            enabled.remove(&snapshot.target.instance_id);
                            false
                        }
                    }
                    winit::event::Ime::Disabled => {
                        enabled.remove(&snapshot.target.instance_id);
                        true
                    }
                    winit::event::Ime::Preedit(..) | winit::event::Ime::Commit(_) => {
                        self.control.ime_allowed.get()
                            && enabled.contains(&snapshot.target.instance_id)
                    }
                }
            };
            let session_notice = matches!(
                ime,
                winit::event::Ime::Enabled | winit::event::Ime::Disabled
            );
            let fresh_enable = matches!(ime, winit::event::Ime::Enabled)
                && imgui_ctx.ime_session_viewport().is_none();
            if accepts_ime
                && (owns_text_input || session_notice && (owns_ime_session || fresh_enable))
            {
                if !matches!(ime, winit::event::Ime::Disabled) {
                    self.control
                        .activate_ime_source(imgui_ctx, snapshot.target.instance_id);
                }
                self.ime_enabled = !matches!(ime, winit::event::Ime::Disabled);
                events::handle_ime_event(ime, imgui_ctx)
            } else {
                WinitEventForwarding::default()
            }
        } else if snapshot.target.is_main {
            let window = self.control.attached_window()?;
            self.control.validate_entry(imgui_ctx, &window)?;
            self.handle_window_event_internal(imgui_ctx, &window, event)
        } else {
            #[cfg(feature = "multi-viewport")]
            {
                let runtime = runtime
                    .as_ref()
                    .ok_or(WinitPlatformError::RuntimeDetached)?;
                crate::multi_viewport::events::route_secondary_window_event(
                    runtime, imgui_ctx, window_id, event,
                )?
            }
            #[cfg(not(feature = "multi-viewport"))]
            {
                WinitEventForwarding::default()
            }
        };
        let needs_frame = !matches!(event, WindowEvent::Destroyed)
            && (!matches!(event, WindowEvent::RedrawRequested) || !snapshot.target.is_main);
        if needs_frame && let Ok(main) = self.control.attached_window() {
            main.request_redraw();
        }
        let platform_action = match event {
            WindowEvent::CloseRequested if snapshot.target.is_main => {
                WinitPlatformAction::MainCloseRequested
            }
            WindowEvent::CloseRequested => WinitPlatformAction::ViewportCloseRequested,
            WindowEvent::Resized(_)
            | WindowEvent::Moved(_)
            | WindowEvent::ScaleFactorChanged { .. } => WinitPlatformAction::GeometryChanged,
            WindowEvent::Destroyed => WinitPlatformAction::NativeDestroyed,
            _ => WinitPlatformAction::None,
        };
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = runtime {
            runtime.poll_fault()?;
        }
        Ok(WinitEventOutcome {
            target: Some(WinitEventTarget {
                window_id,
                native: snapshot.target,
            }),
            forwarded,
            capture: Some(capture),
            needs_frame,
            platform_action,
            pointer_release,
        })
    }

    /// Internal implementation for window event handling.
    fn handle_window_event_internal(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> WinitEventForwarding {
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
                    return WinitEventForwarding::default();
                }
                self.update_single_window_geometry(
                    imgui_ctx,
                    *physical_size,
                    self.hidpi_factor_for_window(window),
                );
                WinitEventForwarding::default()
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => {
                #[cfg(not(feature = "multi-viewport"))]
                let _ = inner_size_writer;
                let new_hidpi = self.hidpi_factor_for_scale(*scale_factor);
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    if let Some(size) = crate::multi_viewport::scale_factor_inner_size_override(
                        imgui_ctx.io().config_dpi_scale_viewports(),
                        window.inner_size(),
                    ) {
                        let mut inner_size_writer = inner_size_writer.clone();
                        let _ = inner_size_writer.request_inner_size(size);
                    }
                    self.control
                        .note_runtime_window_geometry(window.id(), true, true);
                    // Native desktop coordinates do not change when a viewport crosses a DPI
                    // boundary. Only its UI DPI and framebuffer relation change.
                    self.control.hidpi_factor.set(new_hidpi);
                    let io = imgui_ctx.io_mut();
                    io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                    io.set_display_framebuffer_scale(
                        crate::multi_viewport::framebuffer_scale_for_window(window),
                    );
                    return WinitEventForwarding::default();
                }
                self.update_single_window_geometry(imgui_ctx, window.inner_size(), new_hidpi);
                WinitEventForwarding::default()
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
                            return WinitEventForwarding::default();
                        };
                        return events::handle_cursor_moved(
                            [f64::from(position[0]), f64::from(position[1])],
                            imgui_ctx,
                        );
                    }
                }
                self.handle_local_cursor_moved(*position, imgui_ctx)
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
                    return WinitEventForwarding::default();
                }
                self.control.last_local_pointer.set(None);
                {
                    let io = imgui_ctx.io_mut();
                    io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                }
                WinitEventForwarding {
                    pointer: true,
                    ..Default::default()
                }
            }
            #[cfg(feature = "multi-viewport")]
            WindowEvent::Moved(_) if self.control.has_live_runtime() => {
                self.control
                    .note_runtime_window_geometry(window.id(), true, false);
                WinitEventForwarding::default()
            }
            WindowEvent::CursorEntered { .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_cursor_available();
                }
                WinitEventForwarding::default()
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_modifiers(window.id(), modifiers);
                }
                events::handle_modifiers_changed(modifiers, imgui_ctx)
            }
            WindowEvent::Touch(touch) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    let mut forwarded = WinitEventForwarding::default();
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
                            forwarded = events::handle_touch_event_at(
                                action,
                                position,
                                Some(imgui_ctx.main_viewport().id()),
                                imgui_ctx,
                            );
                        }
                    }
                    return forwarded;
                }
                self.handle_local_touch(touch, imgui_ctx)
            }
            WindowEvent::Focused(focused) => {
                #[cfg(feature = "multi-viewport")]
                if let Some(forwarded) =
                    self.control
                        .note_runtime_window_focus(window.id(), *focused, imgui_ctx)
                {
                    return WinitEventForwarding {
                        focus: forwarded,
                        ..Default::default()
                    };
                }
                self.control.handle_single_window_focus(*focused, imgui_ctx)
            }
            _ => WinitEventForwarding::default(),
        }
    }

    fn handle_local_cursor_moved(
        &self,
        physical: PhysicalPosition<f64>,
        context: &mut Context,
    ) -> WinitEventForwarding {
        let logical = physical.to_logical::<f64>(self.hidpi_factor());
        let forwarded = events::handle_cursor_moved([logical.x, logical.y], context);
        if forwarded.pointer {
            self.control.last_local_pointer.set(Some(physical));
        }
        forwarded
    }

    fn handle_local_touch(
        &self,
        touch: &winit::event::Touch,
        context: &mut Context,
    ) -> WinitEventForwarding {
        let position = sanitize::finite_position(touch.location.to_logical(self.hidpi_factor()));
        if position.is_none()
            && matches!(
                touch.phase,
                winit::event::TouchPhase::Started | winit::event::TouchPhase::Moved
            )
        {
            return WinitEventForwarding::default();
        }
        let Some(action) = self.control.note_single_touch(touch) else {
            return WinitEventForwarding::default();
        };
        if position.is_some() {
            self.control.last_local_pointer.set(Some(touch.location));
        }
        events::handle_touch_event_at(action, position, None, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::test_sync::lock_context;
    use dear_imgui_rs::input::{MouseButton, MouseSource};
    use winit::{
        dpi::PhysicalSize,
        event::{DeviceId, Touch, TouchPhase},
    };

    #[test]
    fn dpi_change_keeps_queued_click_at_its_own_position_and_preserves_drag_origin() {
        let _guard = lock_context();
        let mut context = Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _consumer = context.create_synchronous_renderer_consumer().unwrap();
        let platform = WinitPlatform::new(&mut context).unwrap();
        let size = PhysicalSize::new(640, 480);
        platform.handle_local_cursor_moved(PhysicalPosition::new(120.0, 80.0), &mut context);
        context
            .io_mut()
            .add_mouse_button_event(MouseButton::Left, true);
        platform.handle_local_cursor_moved(PhysicalPosition::new(122.0, 82.0), &mut context);
        platform.update_single_window_geometry(&mut context, size, 2.0);

        drop(context.begin_frame());
        assert!(context.io().mouse_down(MouseButton::Left));
        assert_eq!(context.io().mouse_pos(), [60.0, 40.0]);
        drop(context.begin_frame());
        assert_eq!(context.io().mouse_pos(), [61.0, 41.0]);

        // The same physical position across a second scale change must not start a drag.
        platform.update_single_window_geometry(&mut context, size, 1.0);
        let frame = context.begin_frame();
        assert_eq!(frame.ui().io().mouse_pos(), [122.0, 82.0]);
        assert_eq!(frame.ui().io().mouse_delta(), [0.0, 0.0]);
        assert!(!frame.ui().is_mouse_dragging(MouseButton::Left));
    }

    #[test]
    fn dpi_change_reprojects_queued_mouse_and_touch_in_physical_pixels() {
        let _guard = lock_context();
        let mut context = Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _consumer = context.create_synchronous_renderer_consumer().unwrap();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let size = PhysicalSize::new(640, 480);

        // Both events arrive before Dear ImGui has consumed the first position.
        platform.handle_local_cursor_moved(PhysicalPosition::new(120.0, 80.0), &mut context);
        platform.update_single_window_geometry(&mut context, size, 2.0);
        drop(context.begin_frame());
        assert_eq!(context.io().mouse_pos(), [60.0, 40.0]);
        assert_eq!(context.io().display_size(), [320.0, 240.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [2.0, 2.0]);

        platform
            .set_hidpi_mode(crate::HiDpiMode::Locked(2.0))
            .unwrap();
        let hidpi = platform.hidpi_factor_for_scale(1.25);
        platform.update_single_window_geometry(&mut context, size, hidpi);
        let touch = Touch {
            device_id: DeviceId::dummy(),
            phase: TouchPhase::Started,
            location: PhysicalPosition::new(160.0, 90.0),
            force: None,
            id: 1,
        };
        assert!(platform.handle_local_touch(&touch, &mut context).pointer);
        let other_touch = Touch {
            id: 2,
            location: PhysicalPosition::new(10.0, 20.0),
            ..touch
        };
        assert!(
            !platform
                .handle_local_touch(&other_touch, &mut context)
                .pointer
        );
        drop(context.begin_frame());
        assert_eq!(context.io().mouse_pos(), [80.0, 45.0]);
        assert_eq!(context.io().mouse_source(), MouseSource::TouchScreen);
        // Dear ImGui trickles the touch press into the frame following the position event.
        drop(context.begin_frame());

        platform.set_hidpi_mode(crate::HiDpiMode::Rounded).unwrap();
        let hidpi = platform.hidpi_factor_for_scale(1.25);
        platform.update_single_window_geometry(&mut context, size, hidpi);
        drop(context.begin_frame());
        assert_eq!(context.io().mouse_pos(), [160.0, 90.0]);
        assert_eq!(context.io().mouse_source(), MouseSource::TouchScreen);
        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    fn already_enabled_native_source_can_take_over_a_cancelled_session() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform = WinitPlatform::new(&mut context).unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .unwrap();
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let first = dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap();
        let second = dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap();
        let mut text = String::new();
        for activate in [true, false] {
            let ui = context.frame();
            ui.window("native IME sources").build(|| {
                if activate {
                    ui.set_keyboard_focus_here();
                }
                ui.input_text("text", &mut text).build();
            });
            let _ = context.render_legacy();
        }
        platform.control.activate_ime_source(&mut context, first);
        context.set_ime_preedit("old", None).unwrap();
        context.clear_ime_preedit();
        platform.control.activate_ime_source(&mut context, first);
        assert!(
            !context.commit_ime_text("old"),
            "the cancelled source stays blocked"
        );
        platform.control.activate_ime_source(&mut context, second);
        context.set_ime_preedit("new", None).unwrap();
        assert!(
            context.has_ime_preedit(),
            "another enabled source can begin composition"
        );
        context.notify_ime_enabled(false);
        platform.control.activate_ime_source(&mut context, first);
        assert!(
            context.commit_ime_text("new"),
            "a different source can also leave Disabled"
        );
    }

    #[test]
    fn single_window_focus_loss_releases_touch_ownership() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let touch = |id| Touch {
            device_id: DeviceId::dummy(),
            phase: TouchPhase::Started,
            location: PhysicalPosition::new(40.0, 60.0),
            force: None,
            id,
        };

        assert_eq!(
            platform.control.note_single_touch(&touch(7)),
            Some(events::TouchAction::Press)
        );
        let released = platform
            .control
            .handle_single_window_focus(false, &mut context);
        assert!(released.focus && released.pointer);
        let repeated = platform
            .control
            .handle_single_window_focus(false, &mut context);
        assert!(!repeated.pointer);
        platform
            .control
            .handle_single_window_focus(true, &mut context);
        assert_eq!(
            platform.control.note_single_touch(&touch(8)),
            Some(events::TouchAction::Press),
            "a missing touch end before focus loss must not block a new finger"
        );
        platform.shutdown(&mut context).unwrap();
    }
}
