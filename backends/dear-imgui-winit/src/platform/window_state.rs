use std::sync::Arc;

use dear_imgui_rs::{ConfigFlags, Context};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::Window;

use crate::cursor::CursorSettings;

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl, platform_control_for_context};

pub(super) type SetImeDataCallback = unsafe extern "C" fn(
    *mut dear_imgui_rs::sys::ImGuiContext,
    *mut dear_imgui_rs::sys::ImGuiViewport,
    *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
);

/// IME hook: Dear ImGui calls this when the text input caret moves. We forward
/// the position to winit so platforms that support it can position the IME
/// candidate/composition window near the caret.
pub(super) unsafe extern "C" fn imgui_winit_set_ime_data(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
) {
    use dear_imgui_rs::sys::{ImGuiPlatformImeData, ImGuiViewport};

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if viewport.is_null() || data.is_null() {
            return;
        }
        let Some(control) = platform_control_for_context(ctx) else {
            return;
        };
        let _ = control.binding.try_with_bound_context(|| {
            if let Err(error) = control.validate_complete_contract_in_current_context() {
                control.fail_current_contract(error);
                return;
            }
            #[cfg(feature = "multi-viewport")]
            let window = crate::multi_viewport::window_for_viewport(ctx, viewport)
                .or_else(|| control.attached_window().ok());
            #[cfg(not(feature = "multi-viewport"))]
            let window = control.attached_window().ok();
            let Some(window) = window else {
                return;
            };

            let ime: &ImGuiPlatformImeData = &*data;
            let vp: &ImGuiViewport = &*viewport;
            if !ime.WantVisible && !ime.WantTextInput {
                return;
            }
            let line_height = if ime.InputLineHeight > 0.0 {
                ime.InputLineHeight
            } else {
                16.0_f32
            };
            #[cfg(feature = "multi-viewport")]
            let area = if control.has_live_runtime() {
                crate::multi_viewport::ime_cursor_area_for_viewport(
                    &window,
                    [ime.InputPos.x, ime.InputPos.y],
                    [vp.Pos.x, vp.Pos.y],
                    line_height,
                )
            } else {
                Some((
                    LogicalPosition::new(
                        f64::from(ime.InputPos.x - vp.Pos.x),
                        f64::from(ime.InputPos.y - vp.Pos.y),
                    ),
                    LogicalSize::new(f64::from(line_height), f64::from(line_height)),
                ))
            };
            #[cfg(not(feature = "multi-viewport"))]
            let area = Some((
                LogicalPosition::new(
                    f64::from(ime.InputPos.x - vp.Pos.x),
                    f64::from(ime.InputPos.y - vp.Pos.y),
                ),
                LogicalSize::new(f64::from(line_height), f64::from(line_height)),
            ));
            if let Some((position, size)) = area {
                window.set_ime_cursor_area(position, size);
            }
        });
    }));
    if res.is_err()
        && let Some(control) = platform_control_for_context(ctx)
    {
        let _ = control.binding.try_with_bound_context(|| {
            control.fail_current_contract(WinitPlatformError::CallbackPanicked {
                callback: "Platform_SetImeDataFn",
            });
        });
    }
}

pub(super) fn ime_callback_eq(
    left: Option<SetImeDataCallback>,
    right: Option<SetImeDataCallback>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

impl WinitPlatformControl {
    pub(crate) fn attached_window(&self) -> Result<Arc<Window>, WinitPlatformError> {
        self.attached_window
            .borrow()
            .clone()
            .ok_or(WinitPlatformError::WindowNotAttached)
    }

    pub(crate) fn set_ime_allowed_for_owned_windows(&self, allowed: bool) {
        self.ime_allowed.set(allowed);
        if let Some(window) = self.attached_window.borrow().as_ref() {
            window.set_ime_allowed(allowed);
        }
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.set_ime_allowed(allowed);
        }
    }

    #[cfg(feature = "multi-viewport")]
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn set_cursor_settings(&self, settings: Option<CursorSettings>) {
        self.cursor_settings.set(settings);
        let Some(settings) = settings else {
            return;
        };
        if let Some(window) = self.attached_window.borrow().as_ref() {
            settings.apply(window);
        }
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.apply_cursor_settings(settings);
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn apply_current_window_state(&self, window: &Window) {
        window.set_ime_allowed(self.ime_allowed.get());
        if let Some(settings) = self.cursor_settings.get() {
            settings.apply(window);
        }
    }

    fn ensure_window(&self, window: &Window) -> Result<Arc<Window>, WinitPlatformError> {
        let attached = self.attached_window()?;
        if std::ptr::eq(Arc::as_ptr(&attached), window) {
            Ok(attached)
        } else {
            Err(WinitPlatformError::WindowMismatch)
        }
    }

    pub(super) fn validate_entry(
        &self,
        context: &Context,
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.ensure_window(window)?;
        self.validate_operational_contract()
    }

    pub(super) fn validate_window_entry(&self, window: &Window) -> Result<(), WinitPlatformError> {
        self.ensure_window(window)?;
        self.validate_operational_contract()
    }

    pub(super) fn attach_window(
        &self,
        context: &mut Context,
        window: Arc<Window>,
    ) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.validate_operational_contract()?;
        self.binding.try_with_bound_context(|| {
            if let Some(existing) = self.attached_window.borrow().as_ref()
                && !Arc::ptr_eq(existing, &window)
            {
                return Err(WinitPlatformError::WindowMismatch);
            }
            unsafe {
                let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
                (*platform_io).Platform_SetImeDataFn = Some(imgui_winit_set_ime_data);
                (*platform_io).Platform_ImeUserData = Arc::as_ptr(&window).cast_mut().cast();
            }
            self.attached_window.borrow_mut().replace(window);
            Ok(())
        })?
    }

    pub(super) fn detach_window(
        &self,
        context: &mut Context,
        window: &Arc<Window>,
    ) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.validate_operational_contract()?;
        self.binding.try_with_bound_context(|| {
            let attached = self.attached_window()?;
            if !Arc::ptr_eq(&attached, window) {
                return Err(WinitPlatformError::WindowMismatch);
            }
            self.release_single_touch_in_current_context();
            unsafe {
                let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
                (*platform_io).Platform_SetImeDataFn = self.baseline_ime_callback.get();
                (*platform_io).Platform_ImeUserData = self.baseline_ime_user_data.get();
            }
            self.attached_window.borrow_mut().take();
            Ok(())
        })?
    }
}

impl WinitPlatform {
    /// Enable or disable IME events for the attached window.
    ///
    /// Winit does not deliver `WindowEvent::Ime` events unless IME is explicitly
    /// allowed on the window. When `ime_auto_manage` is enabled (default), the
    /// backend will override this based on `io.want_text_input()` every frame.
    /// Use this helper for immediate overrides (e.g. when auto-management is
    /// disabled or you want to force a specific state for a while).
    pub fn set_ime_allowed(&mut self, allowed: bool) -> Result<(), WinitPlatformError> {
        let window = self.control.attached_window()?;
        self.control.validate_window_entry(&window)?;
        self.control.set_ime_allowed_for_owned_windows(allowed);
        self.ime_enabled = allowed;
        Ok(())
    }

    /// Returns whether IME is currently allowed for the attached window.
    ///
    /// This reflects the last state set via `set_ime_allowed` or IME
    /// `WindowEvent::Ime(Enabled/Disabled)` notifications.
    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    /// Enable or disable automatic IME management.
    ///
    /// When enabled (default), the backend will call `set_ime_allowed` based on
    /// Dear ImGui's `io.want_text_input()` flag each frame, turning IME on
    /// while text widgets are active and off otherwise. When disabled, IME
    /// state is left entirely under application control.
    pub fn set_ime_auto_management(&mut self, enabled: bool) {
        self.ime_auto_manage = enabled;
    }

    /// Toggle Dear ImGui software-drawn cursor.
    /// When enabled, the OS cursor is hidden and ImGui draws the cursor in draw data.
    pub fn set_software_cursor_enabled(
        &mut self,
        imgui_ctx: &mut Context,
        enabled: bool,
    ) -> Result<(), WinitPlatformError> {
        self.control.ensure_context(imgui_ctx)?;
        self.control.validate_operational_contract()?;
        imgui_ctx.io_mut().set_mouse_draw_cursor(enabled);
        // Invalidate cursor cache so the next `prepare_render` applies the visibility change.
        self.cursor_cache = None;
        Ok(())
    }

    /// Update cursor and IME state after UI construction and before rendering.
    pub fn prepare_render(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        if ui.context_id() != self.control.binding.id() {
            return Err(WinitPlatformError::ContextMismatch);
        }
        self.control.validate_window_entry(window)?;
        // Auto-manage IME allowed state based on Dear ImGui's intent. This lets
        // the platform show/hide IME (and soft keyboards on mobile) only when
        // text input widgets are active.
        if self.ime_auto_manage {
            let want_text = ui.io().want_text_input();
            if want_text && !self.ime_enabled {
                self.control.set_ime_allowed_for_owned_windows(true);
                self.ime_enabled = true;
            } else if !want_text && self.ime_enabled {
                self.control.set_ime_allowed_for_owned_windows(false);
                self.ime_enabled = false;
            }
        }

        // Only change OS cursor if not disabled by config flags
        if !ui
            .io()
            .config_flags()
            .contains(ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            // Our Io wrapper does not currently expose MouseDrawCursor, assume false (OS cursor)
            let cursor = CursorSettings {
                cursor: ui.mouse_cursor(),
                draw_cursor: ui.io().mouse_draw_cursor(),
            };
            if self.cursor_cache != Some(cursor) {
                #[cfg(feature = "multi-viewport")]
                self.control.set_cursor_settings(Some(cursor));
                #[cfg(not(feature = "multi-viewport"))]
                cursor.apply(window);
                self.cursor_cache = Some(cursor);
            }
        } else {
            self.cursor_cache = None;
            #[cfg(feature = "multi-viewport")]
            self.control.set_cursor_settings(None);
        }
        Ok(())
    }
}
