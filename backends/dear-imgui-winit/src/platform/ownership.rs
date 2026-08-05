use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void};
use std::rc::Rc;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[cfg(feature = "multi-viewport")]
use dear_imgui_rs::ContextPlatformWindowTeardown;
use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentHandle, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding, ContextDestroyed,
    ContextTeardown,
};
use winit::window::Window;

use crate::{cursor::CursorSettings, sanitize};

use super::WinitPlatformError;
use super::frame::HiDpiMode;
use super::window_state::{SetImeDataCallback, ime_callback_eq, imgui_winit_set_ime_data};

struct WinitPlatformAttachmentMarker;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlatformState {
    Active,
    Faulted,
    ShuttingDown,
    Detached,
    ContextDestroyed,
}

pub(super) const WINIT_BASE_FLAGS: BackendFlags = BackendFlags::HAS_MOUSE_CURSORS;
#[cfg(all(feature = "multi-viewport", target_os = "windows"))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags =
    BackendFlags::PLATFORM_HAS_VIEWPORTS.union(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT);
#[cfg(all(feature = "multi-viewport", not(target_os = "windows")))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags = BackendFlags::PLATFORM_HAS_VIEWPORTS;
#[cfg(not(feature = "multi-viewport"))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags = BackendFlags::empty();
pub(super) const WINIT_RESERVED_FLAGS: BackendFlags = WINIT_BASE_FLAGS
    .union(BackendFlags::HAS_SET_MOUSE_POS)
    .union(WINIT_VIEWPORT_FLAGS)
    .union(BackendFlags::HAS_PARENT_VIEWPORT);

#[repr(C)]
pub(super) struct PlatformOwnerToken {
    marker: u8,
}

pub(super) fn winit_backend_name_ptr() -> *const c_char {
    concat!("dear-imgui-winit ", env!("CARGO_PKG_VERSION"), "\0")
        .as_ptr()
        .cast()
}

struct RegisteredPlatform {
    context_raw: usize,
    context_id: dear_imgui_rs::ContextId,
    control: std::rc::Weak<WinitPlatformControl>,
}

thread_local! {
    static PLATFORM_CONTROLS: RefCell<Vec<RegisteredPlatform>> = const { RefCell::new(Vec::new()) };
}

fn register_platform_control(control: &Rc<WinitPlatformControl>) {
    PLATFORM_CONTROLS.with(|controls| {
        let mut controls = controls.borrow_mut();
        controls.retain(|entry| entry.control.strong_count() > 0);
        controls.push(RegisteredPlatform {
            context_raw: control.context_raw as usize,
            context_id: control.binding.id(),
            control: Rc::downgrade(control),
        });
    });
}

fn unregister_platform_control(context_id: dear_imgui_rs::ContextId) {
    PLATFORM_CONTROLS.with(|controls| {
        controls
            .borrow_mut()
            .retain(|entry| entry.context_id != context_id);
    });
}

pub(super) fn platform_control_for_context(
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<Rc<WinitPlatformControl>> {
    if context_raw.is_null() {
        return None;
    }
    PLATFORM_CONTROLS.with(|controls| {
        let mut controls = controls.borrow_mut();
        controls.retain(|entry| entry.control.strong_count() > 0);
        controls
            .iter()
            .find(|entry| entry.context_raw == context_raw as usize)
            .and_then(|entry| entry.control.upgrade())
    })
}

pub(crate) struct WinitPlatformControl {
    pub(super) context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    pub(super) binding: ContextBinding,
    pub(super) state: Cell<PlatformState>,
    pub(super) token: Box<PlatformOwnerToken>,
    pub(super) installed_name: Cell<*const c_char>,
    pub(super) baseline_ime_callback: Cell<Option<SetImeDataCallback>>,
    pub(super) baseline_ime_user_data: Cell<*mut c_void>,
    pub(super) attached_window: RefCell<Option<Arc<Window>>>,
    pub(super) ime_allowed: Cell<bool>,
    pub(super) active_touch: Cell<Option<u64>>,
    pub(super) terminal_fault: RefCell<Option<WinitPlatformError>>,
    pub(super) attachment_handle: RefCell<Option<ContextAttachmentHandle>>,
    #[cfg(feature = "multi-viewport")]
    pub(super) runtime: RefCell<Option<Rc<crate::multi_viewport::RuntimeControl>>>,
    #[cfg(feature = "multi-viewport")]
    pub(super) cursor_settings: Cell<Option<CursorSettings>>,
}

impl WinitPlatformControl {
    fn preflight(
        context: &Context,
    ) -> Result<(Option<SetImeDataCallback>, *mut c_void), WinitPlatformError> {
        context.binding().with_bound_context(|| unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            for (occupied, field) in [
                (!(*io).BackendPlatformName.is_null(), "BackendPlatformName"),
                (
                    !(*io).BackendPlatformUserData.is_null(),
                    "BackendPlatformUserData",
                ),
                (
                    !(*platform_io).Platform_ImeUserData.is_null(),
                    "Platform_ImeUserData",
                ),
            ] {
                if occupied {
                    return Err(WinitPlatformError::PlatformStateOccupied { field });
                }
            }
            let occupied_flags =
                BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS;
            if !occupied_flags.is_empty() {
                return Err(WinitPlatformError::PlatformStateOccupied {
                    field: "BackendFlags",
                });
            }
            Ok((
                (*platform_io).Platform_SetImeDataFn,
                (*platform_io).Platform_ImeUserData,
            ))
        })
    }

    fn claim(
        context: &mut Context,
    ) -> Result<(Rc<Self>, ContextAttachmentLease), WinitPlatformError> {
        let (baseline_ime_callback, baseline_ime_user_data) = Self::preflight(context)?;
        let control = Rc::new(Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(PlatformState::Active),
            token: Box::new(PlatformOwnerToken { marker: 1 }),
            installed_name: Cell::new(std::ptr::null()),
            baseline_ime_callback: Cell::new(baseline_ime_callback),
            baseline_ime_user_data: Cell::new(baseline_ime_user_data),
            attached_window: RefCell::new(None),
            ime_allowed: Cell::new(false),
            active_touch: Cell::new(None),
            terminal_fault: RefCell::new(None),
            attachment_handle: RefCell::new(None),
            #[cfg(feature = "multi-viewport")]
            runtime: RefCell::new(None),
            #[cfg(feature = "multi-viewport")]
            cursor_settings: Cell::new(None),
        });
        let attachment = context.register_attachment::<WinitPlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        )?;
        control
            .attachment_handle
            .borrow_mut()
            .replace(attachment.handle());

        context.binding().with_bound_context(|| unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            (*io).BackendPlatformName = winit_backend_name_ptr();
            control.installed_name.set(winit_backend_name_ptr());
            (*io).BackendPlatformUserData = control.token_ptr();
            (*io).BackendFlags |= WINIT_BASE_FLAGS.bits();
        });
        register_platform_control(&control);
        Ok((control, attachment))
    }

    pub(super) fn token_ptr(&self) -> *mut c_void {
        std::ptr::from_ref::<PlatformOwnerToken>(&self.token)
            .cast_mut()
            .cast()
    }

    #[cfg(test)]
    pub(crate) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    fn expected_owned_flags(&self) -> BackendFlags {
        #[cfg(feature = "multi-viewport")]
        let viewport_flags = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .map_or(BackendFlags::empty(), |_| WINIT_VIEWPORT_FLAGS);
        #[cfg(not(feature = "multi-viewport"))]
        let viewport_flags = BackendFlags::empty();
        WINIT_BASE_FLAGS | viewport_flags
    }

    pub(crate) fn ensure_context(&self, context: &Context) -> Result<(), WinitPlatformError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(WinitPlatformError::ContextMismatch)
        }
    }

    fn ensure_active(&self) -> Result<(), WinitPlatformError> {
        if let Some(error) = self.terminal_fault.borrow().clone() {
            return Err(error);
        }
        match self.state.get() {
            PlatformState::Active => Ok(()),
            PlatformState::Faulted
            | PlatformState::ShuttingDown
            | PlatformState::Detached
            | PlatformState::ContextDestroyed => Err(WinitPlatformError::RuntimeDetached),
        }
    }

    fn validate_base_contract_in_current_context(&self) -> Result<(), WinitPlatformError> {
        self.ensure_active()?;
        unsafe {
            if dear_imgui_rs::sys::igGetCurrentContext() != self.context_raw {
                return Err(WinitPlatformError::ContextMismatch);
            }
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            for (matches, field) in [
                (
                    (*io).BackendPlatformName == self.installed_name.get(),
                    "BackendPlatformName",
                ),
                (
                    (*io).BackendPlatformUserData == self.token_ptr(),
                    "BackendPlatformUserData",
                ),
                (
                    (BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS)
                        == self.expected_owned_flags(),
                    "BackendFlags",
                ),
            ] {
                if !matches {
                    return Err(WinitPlatformError::PlatformStateReplaced { field });
                }
            }

            let attached_window = self.attached_window.borrow();
            let expected_window = attached_window.as_ref().map(Arc::as_ptr);
            let callback_matches = match ((*platform_io).Platform_SetImeDataFn, expected_window) {
                (actual, None) => ime_callback_eq(actual, self.baseline_ime_callback.get()),
                (Some(actual), Some(_)) => {
                    std::ptr::fn_addr_eq(actual, imgui_winit_set_ime_data as SetImeDataCallback)
                }
                _ => false,
            };
            if !callback_matches {
                return Err(WinitPlatformError::PlatformStateReplaced {
                    field: "Platform_SetImeDataFn",
                });
            }
            let expected_user_data = expected_window
                .map_or(self.baseline_ime_user_data.get(), |window| {
                    window.cast_mut().cast()
                });
            if (*platform_io).Platform_ImeUserData != expected_user_data {
                return Err(WinitPlatformError::PlatformStateReplaced {
                    field: "Platform_ImeUserData",
                });
            }
        }
        Ok(())
    }

    pub(crate) fn validate_complete_contract_in_current_context(
        &self,
    ) -> Result<(), WinitPlatformError> {
        self.validate_base_contract_in_current_context()?;
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.validate_publication_contract_in_current_context()?;
        }
        Ok(())
    }

    pub(crate) fn validate_operational_contract(&self) -> Result<(), WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            let result = self.validate_complete_contract_in_current_context();
            if let Err(error) = &result {
                self.fail_current_contract(error.clone());
            }
            result
        })?
    }

    pub(crate) fn terminal_fault(&self) -> Option<WinitPlatformError> {
        if matches!(
            self.state.get(),
            PlatformState::Detached | PlatformState::ContextDestroyed
        ) {
            None
        } else {
            self.terminal_fault.borrow().clone()
        }
    }

    pub(crate) fn owns_base_publication_in_current_context(&self) -> bool {
        unsafe {
            if dear_imgui_rs::sys::igGetCurrentContext() != self.context_raw {
                return false;
            }
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return false;
            }
            if (*io).BackendPlatformName == self.installed_name.get()
                || (*io).BackendPlatformUserData == self.token_ptr()
            {
                return true;
            }
            let attached_window = self.attached_window.borrow();
            let Some(window) = attached_window.as_ref() else {
                return false;
            };
            let expected_window = Arc::as_ptr(window).cast_mut().cast::<c_void>();
            (*platform_io)
                .Platform_SetImeDataFn
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(callback, imgui_winit_set_ime_data as SetImeDataCallback)
                })
                || (*platform_io).Platform_ImeUserData == expected_window
        }
    }

    fn owns_any_publication_or_callback_in_current_context(&self) -> bool {
        if self.owns_base_publication_in_current_context() {
            return true;
        }
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            return runtime.owns_any_platform_callback_in_current_context();
        }
        false
    }

    pub(crate) fn fail_current_contract(&self, error: WinitPlatformError) {
        let owns_winit_state = self.owns_any_publication_or_callback_in_current_context();
        if self.terminal_fault.borrow().is_none() {
            *self.terminal_fault.borrow_mut() = Some(error);
        }
        self.state.set(PlatformState::Faulted);
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.mark_faulted();
        }
        unsafe {
            if owns_winit_state && dear_imgui_rs::sys::igGetCurrentContext() == self.context_raw {
                let io = dear_imgui_rs::sys::igGetIO_Nil();
                if !io.is_null() {
                    (*io).BackendFlags &= !(WINIT_BASE_FLAGS | WINIT_VIEWPORT_FLAGS).bits();
                }
            }
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn has_live_runtime(&self) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| !runtime.is_released())
    }

    pub(crate) fn attachment_handle(&self) -> Result<ContextAttachmentHandle, WinitPlatformError> {
        self.attachment_handle
            .borrow()
            .clone()
            .filter(ContextAttachmentHandle::is_attached)
            .ok_or(WinitPlatformError::RuntimeDetached)
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn install_runtime(
        &self,
        runtime: Rc<crate::multi_viewport::RuntimeControl>,
    ) -> Result<(), WinitPlatformError> {
        let mut slot = self.runtime.borrow_mut();
        if slot.is_some() {
            return Err(WinitPlatformError::RuntimeAlreadyAttached);
        }
        *slot = Some(runtime);
        Ok(())
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn clear_runtime(&self, runtime: &Rc<crate::multi_viewport::RuntimeControl>) {
        self.clear_runtime_by_control(runtime.as_ref());
    }

    #[cfg(feature = "multi-viewport")]
    fn clear_runtime_by_control(&self, runtime: &crate::multi_viewport::RuntimeControl) {
        let mut slot = self.runtime.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|installed| std::ptr::eq(installed.as_ref(), runtime))
        {
            slot.take();
        }
    }

    fn release_base_in_current_context(&self) -> Result<(), WinitPlatformError> {
        let mut replaced = None;
        let owns_base_publication = self.owns_base_publication_in_current_context();
        unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            self.release_single_touch_in_current_context();
            let attached_window = self
                .attached_window
                .borrow()
                .as_ref()
                .map(|window| Arc::as_ptr(window).cast_mut().cast::<c_void>());
            if let Some(expected_window) = attached_window {
                let callback_is_ours =
                    (*platform_io)
                        .Platform_SetImeDataFn
                        .is_some_and(|callback| {
                            std::ptr::fn_addr_eq(
                                callback,
                                imgui_winit_set_ime_data as SetImeDataCallback,
                            )
                        });
                if callback_is_ours {
                    (*platform_io).Platform_SetImeDataFn = self.baseline_ime_callback.get();
                } else {
                    replaced.get_or_insert("Platform_SetImeDataFn");
                }
                if (*platform_io).Platform_ImeUserData == expected_window {
                    (*platform_io).Platform_ImeUserData = self.baseline_ime_user_data.get();
                } else {
                    replaced.get_or_insert("Platform_ImeUserData");
                }
            } else {
                if !ime_callback_eq(
                    (*platform_io).Platform_SetImeDataFn,
                    self.baseline_ime_callback.get(),
                ) {
                    replaced.get_or_insert("Platform_SetImeDataFn");
                }
                if (*platform_io).Platform_ImeUserData != self.baseline_ime_user_data.get() {
                    replaced.get_or_insert("Platform_ImeUserData");
                }
            }
            let expected_flags = self.expected_owned_flags();
            if owns_base_publication
                && self.terminal_fault.borrow().is_none()
                && (BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS)
                    != expected_flags
            {
                replaced.get_or_insert("BackendFlags");
            }
            if (*io).BackendPlatformUserData == self.token_ptr() {
                (*io).BackendPlatformUserData = std::ptr::null_mut();
            } else {
                replaced.get_or_insert("BackendPlatformUserData");
            }
            if (*io).BackendPlatformName == self.installed_name.get() {
                (*io).BackendPlatformName = std::ptr::null();
            } else {
                replaced.get_or_insert("BackendPlatformName");
            }
            if owns_base_publication {
                (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
            }
        }
        if let Some(window) = self.attached_window.borrow_mut().take() {
            window.set_ime_allowed(false);
        }
        replaced.map_or(Ok(()), |field| {
            Err(WinitPlatformError::PlatformStateReplaced { field })
        })
    }
}

impl ContextAttachment for WinitPlatformControl {
    #[cfg(feature = "multi-viewport")]
    fn begin_platform_window_teardown(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let runtime = self.runtime.borrow().clone();
        context
            .with_bound_context(|| -> Result<(), WinitPlatformError> {
                // `WinitPlatformRuntime::shutdown` already performed its typed preflight and
                // opened this callback guard before it entered the core transaction. Preserve
                // that error path instead of wrapping it through the generic attachment error.
                if runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.teardown_callbacks_active())
                {
                    return Ok(());
                }

                self.validate_base_contract_in_current_context()?;
                if let Some(runtime) = runtime.as_ref() {
                    runtime.begin_context_platform_window_teardown()?;
                }
                Ok(())
            })
            .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    #[cfg(feature = "multi-viewport")]
    fn end_platform_window_teardown(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let runtime = self.runtime.borrow().clone();
        let Some(runtime) = runtime else {
            return Ok(());
        };

        let result = context.with_bound_context(|| -> Result<(), WinitPlatformError> {
            runtime.finish_context_platform_window_teardown()?;
            Ok(())
        });
        if runtime.is_released() {
            self.clear_runtime_by_control(&runtime);
        }
        result.map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    fn quiesce(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.state.set(PlatformState::ShuttingDown);
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.quiesce_from_platform();
        }
        Ok(())
    }

    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.release_from_platform_teardown(context)?;
        }
        context
            .with_bound_context(|| self.release_base_in_current_context())
            .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow_mut().take() {
            runtime.context_destroyed_from_platform(_context);
        }
        self.attached_window.borrow_mut().take();
        self.active_touch.set(None);
        unregister_platform_control(self.binding.id());
        self.attachment_handle.borrow_mut().take();
        self.state.set(PlatformState::ContextDestroyed);
    }
}

/// Main platform backend for Dear ImGui with winit integration
pub struct WinitPlatform {
    pub(super) control: Rc<WinitPlatformControl>,
    pub(super) attachment: Option<ContextAttachmentLease>,
    pub(super) hidpi_mode: HiDpiMode,
    pub(super) hidpi_factor: f64,
    pub(super) cursor_cache: Option<CursorSettings>,
    pub(super) ime_enabled: bool,
    pub(super) ime_auto_manage: bool,
    pub(super) last_frame: Instant,
}

impl WinitPlatform {
    /// Create a new winit platform backend
    ///
    /// # Example
    ///
    /// ```
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_winit::WinitPlatform;
    ///
    /// let mut imgui_ctx = Context::create();
    /// let mut platform = WinitPlatform::new(&mut imgui_ctx).unwrap();
    /// ```
    pub fn new(imgui_ctx: &mut Context) -> Result<Self, WinitPlatformError> {
        let (control, attachment) = WinitPlatformControl::claim(imgui_ctx)?;
        Ok(Self {
            control,
            attachment: Some(attachment),
            hidpi_mode: HiDpiMode::default(),
            hidpi_factor: 1.0,
            cursor_cache: None,
            ime_enabled: false,
            ime_auto_manage: true,
            last_frame: Instant::now(),
        })
    }

    /// Attach the platform to a window.
    ///
    /// The platform keeps shared ownership of the exact window allocation until detach or
    /// Context teardown, so the native IME callback cannot outlive the Winit window.
    pub fn attach_window(
        &mut self,
        window: Arc<Window>,
        hidpi_mode: HiDpiMode,
        imgui_ctx: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        // This must precede the IME attachment and every cached scale update. Reattaching the
        // same window with a different coordinate mode is still a contract violation once
        // secondary viewports exist.
        self.ensure_runtime_configuration_mutable()?;
        self.control.attach_window(imgui_ctx, Arc::clone(&window))?;
        self.hidpi_mode = hidpi_mode;
        self.hidpi_factor = self.hidpi_factor_for_window(&window);

        // Convert via winit scale then adapt to our active HiDPI mode
        let logical_size = window
            .inner_size()
            .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
        let logical_size = self.scale_size_from_winit(&window, logical_size);
        let io = imgui_ctx.io_mut();

        io.set_display_size(sanitize::finite_non_negative_size(logical_size));
        io.set_display_framebuffer_scale(sanitize::framebuffer_scale(self.hidpi_factor, 1.0));

        // Enable IME by default so WindowEvent::Ime events and IME composition
        // are available on desktop platforms. Auto-management (when enabled)
        // will further refine this for text widgets.
        self.set_ime_allowed(true)?;
        Ok(())
    }

    /// Detach the platform from a window and clear winit-owned IME hooks.
    ///
    /// Multi-viewport support must be shut down before detaching the shared main window.
    pub fn detach_window(
        &mut self,
        imgui_ctx: &mut Context,
    ) -> Result<Arc<Window>, WinitPlatformError> {
        #[cfg(feature = "multi-viewport")]
        if self.control.runtime.borrow().is_some() {
            return Err(WinitPlatformError::RuntimeAlreadyAttached);
        }
        let window = self.control.attached_window()?;
        self.control.detach_window(imgui_ctx, &window)?;
        window.set_ime_allowed(false);
        self.ime_enabled = false;
        Ok(window)
    }

    /// Explicitly releases every Winit-owned field from the bound Context.
    ///
    /// Multi-viewport state, when present, is released first. The operation preserves foreign
    /// replacements and reports the first ownership violation after clearing fields that still
    /// have Winit's exact pointer identity. An active renderer attachment rejects shutdown before
    /// any frame or native state changes; shut down that renderer and retry.
    pub fn shutdown(&mut self, imgui_ctx: &mut Context) -> Result<(), WinitPlatformError> {
        self.control.ensure_context(imgui_ctx)?;
        if matches!(
            self.control.state.get(),
            PlatformState::Detached | PlatformState::ContextDestroyed
        ) {
            return Ok(());
        }
        let attachment = self.control.attachment_handle()?;
        let mut release = imgui_ctx.prepare_platform_attachment_release(&attachment)?;
        let _imgui_ctx = release.context_mut();
        let terminal_fault = self.control.terminal_fault();
        #[cfg(feature = "multi-viewport")]
        let pending_error = {
            let mut pending_error = None;
            if let Some(runtime) = self.control.runtime.borrow().clone() {
                if let Err(error) = runtime.shutdown_from_platform(_imgui_ctx) {
                    if !runtime.is_released() {
                        return Err(error);
                    }
                    pending_error = Some(error);
                }
                self.control.clear_runtime(&runtime);
            }
            pending_error
        };
        #[cfg(not(feature = "multi-viewport"))]
        let pending_error = None;

        let result = self
            .control
            .binding
            .try_with_bound_context(|| self.control.release_base_in_current_context())?;
        unregister_platform_control(self.control.binding.id());
        self.control.state.set(PlatformState::Detached);
        self.control.attachment_handle.borrow_mut().take();
        release.commit();
        self.attachment.take();
        match (terminal_fault.or(pending_error), result) {
            (Some(error), _) => Err(error),
            (None, result) => result,
        }
    }

    /// Returns the Context-bound owner shared with the multi-viewport runtime.
    #[cfg(any(feature = "multi-viewport", test))]
    pub(crate) fn control(&self) -> Rc<WinitPlatformControl> {
        Rc::clone(&self.control)
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn ensure_runtime_configuration_mutable(&self) -> Result<(), WinitPlatformError> {
        if self.control.runtime.borrow().is_some() {
            Err(WinitPlatformError::RuntimeConfigurationLocked)
        } else {
            Ok(())
        }
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub(crate) fn ensure_runtime_configuration_mutable(&self) -> Result<(), WinitPlatformError> {
        Ok(())
    }
}

impl Drop for WinitPlatform {
    fn drop(&mut self) {
        #[cfg(feature = "multi-viewport")]
        let runtime_attached = self.control.runtime.borrow().is_some();
        #[cfg(not(feature = "multi-viewport"))]
        let runtime_attached = false;
        let renderer_attached = self
            .attachment
            .as_ref()
            .is_some_and(|attachment| attachment.handle().has_active_renderer_dependency());
        if !runtime_attached
            && !renderer_attached
            && self
                .control
                .binding
                .try_with_bound_context(|| self.control.release_base_in_current_context())
                .is_ok()
        {
            unregister_platform_control(self.control.binding.id());
            self.control.state.set(PlatformState::Detached);
            if let Some(mut attachment) = self.attachment.take() {
                let _ = attachment
                    .detach()
                    .expect("Winit verified that no renderer attachment blocks platform detach");
            }
            return;
        }
        if let Some(attachment) = self.attachment.take() {
            attachment.defer_to_context();
        }
    }
}
