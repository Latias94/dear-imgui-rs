use std::cell::Cell;
use std::ffi::c_void;
use std::sync::Arc;

use winit::window::Window;

use super::WinitPlatformError;
use super::native_cursor_hittest::NativeCursorHitTest;
use super::registry::{insert_viewport_data, owns_viewport_data};
use super::runtime::RuntimeControl;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewportWindowPolicy {
    pub(super) decorations: bool,
    pub(super) top_most: bool,
    pub(super) skip_taskbar: bool,
    pub(super) cursor_hittest: bool,
    pub(super) no_focus_on_appearing: bool,
    pub(super) no_focus_on_click: bool,
}

impl Default for ViewportWindowPolicy {
    fn default() -> Self {
        Self {
            decorations: true,
            top_most: false,
            skip_taskbar: false,
            cursor_hittest: true,
            no_focus_on_appearing: false,
            no_focus_on_click: false,
        }
    }
}

impl ViewportWindowPolicy {
    pub(super) fn from_flags(flags: dear_imgui_rs::sys::ImGuiViewportFlags) -> Self {
        Self {
            decorations: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoDecoration == 0,
            top_most: flags & dear_imgui_rs::sys::ImGuiViewportFlags_TopMost != 0,
            skip_taskbar: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoTaskBarIcon != 0,
            cursor_hittest: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoInputs == 0,
            no_focus_on_appearing: flags
                & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnAppearing
                != 0,
            no_focus_on_click: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnClick != 0,
        }
    }
}

/// Runtime-owned sidecar stored in `ImGuiViewport::PlatformUserData`.
#[repr(C)]
pub(super) struct ViewportData {
    // Keep the native subclass ahead of the Window so it is removed before the final Arc can
    // destroy the HWND.
    cursor_hittest: NativeCursorHitTest,
    window: Arc<Window>,
    main: bool,
    pub(super) window_policy: Cell<ViewportWindowPolicy>,
    pub(super) last_log_fb_scale: Cell<f32>,
}

impl ViewportData {
    pub(super) fn new(window: Arc<Window>, main: bool) -> Result<Self, WinitPlatformError> {
        let cursor_hittest = NativeCursorHitTest::install(&window)?;
        Ok(Self {
            cursor_hittest,
            window,
            main,
            window_policy: Cell::new(ViewportWindowPolicy::default()),
            last_log_fb_scale: Cell::new(0.0),
        })
    }

    pub(super) fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub(super) fn set_cursor_hittest(&self, enabled: bool) -> Result<(), WinitPlatformError> {
        self.cursor_hittest.set_enabled(&self.window, enabled)
    }

    #[cfg(target_os = "windows")]
    pub(super) fn native_window_id(&self) -> usize {
        self.cursor_hittest.native_window_id()
    }

    pub(super) fn window_ptr(&self) -> *const Window {
        Arc::as_ptr(&self.window)
    }

    pub(super) fn is_main(&self) -> bool {
        self.main
    }
}

pub(super) fn preflight_main_viewport(
    context: &dear_imgui_rs::Context,
) -> Result<(), WinitPlatformError> {
    let binding = context.binding();
    binding.with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null() {
            Err(WinitPlatformError::ContextMismatch)
        } else if (*viewport).PlatformUserData.is_null()
            && (*viewport).PlatformHandle.is_null()
            && (*viewport).PlatformHandleRaw.is_null()
        {
            Ok(())
        } else {
            Err(WinitPlatformError::ForeignPlatformUserData)
        }
    })
}

pub(super) fn init_main_viewport(
    control: &RuntimeControl,
    main_window: Arc<Window>,
) -> Result<(), WinitPlatformError> {
    control.binding().try_with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null()
            || !(*viewport).PlatformUserData.is_null()
            || !(*viewport).PlatformHandle.is_null()
            || !(*viewport).PlatformHandleRaw.is_null()
        {
            return Err(WinitPlatformError::ForeignPlatformUserData);
        }

        let data = insert_viewport_data(
            control,
            viewport,
            ViewportData::new(Arc::clone(&main_window), true)?,
        )?;
        (*viewport).PlatformUserData = data.cast::<c_void>();
        (*viewport).PlatformHandle = Arc::as_ptr(&main_window).cast_mut().cast();
        Ok(())
    })?
}

pub(super) fn viewport_data_is_owned(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    if viewport.is_null() {
        return false;
    }
    // SAFETY: callers invoke this only for a live viewport in the current Context.
    let data = unsafe { (*viewport).PlatformUserData.cast::<ViewportData>() };
    owns_viewport_data(control, viewport, data)
}
