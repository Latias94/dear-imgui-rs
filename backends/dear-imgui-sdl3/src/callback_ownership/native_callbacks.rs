use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use dear_imgui_rs::sys;

use crate::core::ffi;
#[cfg(feature = "sdlgpu3-renderer")]
use crate::runtime::NativeRendererKind;
use crate::runtime::{RuntimeControl, with_current_runtime};

use super::ViewportPlatformState;

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    crate::runtime::register_runtime(control);
}

pub(super) unsafe extern "C" fn sdl3_create_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Platform_CreateWindow", (), |control| unsafe {
        if viewport.is_null() {
            return;
        }
        if !(*viewport).PlatformUserData.is_null() {
            control.record_foreign_platform_user_data();
            (*viewport).PlatformRequestClose = true;
            return;
        }
        let Some(callback) = control.original_create_window() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Create, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport);
        let native_faults = transaction.finish();
        let state = ViewportPlatformState::capture(viewport);
        if !state.user_data.is_null() {
            control.remember_owned_viewport(viewport, state);
        }
        if state.user_data.is_null() || state.handle.is_null() || native_faults != 0 {
            control.record_viewport_creation_failed();
            control.mark_viewport_failed(viewport);
        }
    });
}

pub(super) unsafe extern "C" fn sdl3_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let invoked = run_callback("Platform_DestroyWindow", false, |control| unsafe {
        if viewport.is_null() {
            return false;
        }
        control.forget_failed_viewport(viewport);
        let Some(callback) = control.original_destroy_window() else {
            return false;
        };
        let actual = ViewportPlatformState::capture(viewport);
        let Some(expected) = control.take_owned_viewport(viewport) else {
            record_viewport_replacements(control, None, actual);
            control.defer_platform_viewport_restore(viewport, actual);
            ViewportPlatformState::clear(viewport);
            return true;
        };

        if viewport_platform_state_eq(actual, expected) {
            callback(viewport);
            ViewportPlatformState::clear(viewport);
            return true;
        }

        record_viewport_replacements(control, Some(expected), actual);
        expected.restore(viewport);
        callback(viewport);
        ViewportPlatformState::clear(viewport);
        control.defer_platform_viewport_restore(viewport, actual);
        true
    });
    if invoked && !viewport.is_null() {
        unsafe { ViewportPlatformState::clear(viewport) };
    }
}

pub(super) unsafe extern "C" fn sdl3_render_window(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Platform_RenderWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !validate_platform_viewport_state(control, viewport) {
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_render_window() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Render, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport, render_argument);
        if transaction.finish() != 0 {
            control.mark_viewport_failed(viewport);
        } else if is_secondary_viewport(viewport) {
            control.record_opengl_viewport_context_activated((*viewport).ID);
        }
    });
}

pub(super) unsafe extern "C" fn sdl3_swap_buffers(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Platform_SwapBuffers", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !validate_platform_viewport_state(control, viewport) {
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_swap_buffers() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Swap, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport, render_argument);
        if transaction.finish() != 0 {
            control.mark_viewport_failed(viewport);
        } else if is_secondary_viewport(viewport) {
            control.record_opengl_viewport_swapped((*viewport).ID);
        }
    });
}

unsafe fn is_secondary_viewport(viewport: *mut sys::ImGuiViewport) -> bool {
    let main_viewport = unsafe { sys::igGetMainViewport() };
    !main_viewport.is_null() && main_viewport != viewport
}

pub(super) unsafe extern "C" fn sdl3_renderer_create_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Renderer_CreateWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        if !(*viewport).RendererUserData.is_null() {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_renderer_create_window() else {
            return;
        };
        #[cfg(not(feature = "sdlgpu3-renderer"))]
        {
            callback(viewport);
            control.remember_owned_renderer_viewport(viewport, (*viewport).RendererUserData);
        }

        #[cfg(feature = "sdlgpu3-renderer")]
        {
            if control.native_renderer() != NativeRendererKind::SdlGpu3 {
                callback(viewport);
                control.remember_owned_renderer_viewport(viewport, (*viewport).RendererUserData);
                return;
            }
            let Some(transaction) =
                NativeTransaction::begin(control, NativePhase::SdlGpuCreate, viewport)
            else {
                control.mark_viewport_failed(viewport);
                return;
            };
            callback(viewport);
            let native_faults = transaction.finish();
            finish_sdlgpu_renderer_create(control, viewport, native_faults);
        }
    });
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(crate) unsafe fn finish_sdlgpu_renderer_create(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
    native_faults: u64,
) {
    if native_faults != 0 {
        // Upstream assigns its sentinel even when SDL rejected claim/configuration. Clearing it
        // prevents DestroyWindow from releasing an unclaimed window or releasing a
        // configure-failure claim that the native transaction already rolled back.
        unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
        control.mark_viewport_failed(viewport);
        return;
    }
    let renderer_user_data = unsafe { (*viewport).RendererUserData };
    if renderer_user_data.is_null() {
        control.record_renderer_state_replaced("Viewport.RendererUserData(create)");
        control.mark_viewport_failed(viewport);
        return;
    }
    control.remember_owned_renderer_viewport(viewport, renderer_user_data);
}

pub(super) unsafe extern "C" fn sdl3_renderer_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let invoked = run_callback("Renderer_DestroyWindow", false, |control| unsafe {
        if viewport.is_null() {
            return false;
        }
        if !control.validate_renderer_ownership_bound() {
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        }
        let Some(callback) = control.original_renderer_destroy_window() else {
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };
        let Some(expected_platform) = control.owned_viewport(viewport) else {
            record_viewport_replacements(control, None, ViewportPlatformState::capture(viewport));
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };
        let actual_platform = ViewportPlatformState::capture(viewport);
        let expected_renderer = control.owned_renderer_viewport(viewport);
        let actual_renderer = (*viewport).RendererUserData;
        if expected_renderer.is_none() && actual_renderer.is_null() {
            return true;
        }
        let Some(expected_renderer) = expected_renderer else {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };

        let platform_was_replaced = !viewport_platform_state_eq(actual_platform, expected_platform);
        if platform_was_replaced {
            record_viewport_replacements(control, Some(expected_platform), actual_platform);
            control.defer_platform_viewport_restore(viewport, actual_platform);
        }
        if actual_renderer != expected_renderer {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
        }

        expected_platform.restore(viewport);
        (*viewport).RendererUserData = expected_renderer;
        callback(viewport);
        (*viewport).RendererUserData = std::ptr::null_mut();
        control.forget_owned_renderer_viewport(viewport);
        true
    });
    if invoked && !viewport.is_null() {
        unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
    }
}

pub(super) unsafe extern "C" fn sdl3_renderer_render_window(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Renderer_RenderWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        #[cfg(feature = "sdlgpu3-renderer")]
        if control.native_renderer() == NativeRendererKind::SdlGpu3 {
            let faults = ffi::dear_imgui_sdl3_backend_sdlgpu3_render_viewport(viewport);
            control.record_native_faults(faults);
            if faults != 0 {
                control.mark_viewport_failed(viewport);
            }
            return;
        }
        if let Some(callback) = control.original_renderer_render_window() {
            callback(viewport, render_argument);
        }
    });
}

pub(super) unsafe extern "C" fn sdl3_renderer_set_window_size(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    run_callback("Renderer_SetWindowSize", (), |control| unsafe {
        if viewport.is_null() || size.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        control.invoke_original_renderer_set_window_size(viewport, size);
    });
}

pub(super) unsafe extern "C" fn sdl3_renderer_swap_buffers(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Renderer_SwapBuffers", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        if let Some(callback) = control.original_renderer_swap_buffers() {
            callback(viewport, render_argument);
        }
    });
}

#[repr(u32)]
#[derive(Clone, Copy)]
enum NativePhase {
    Create = 1,
    Render = 2,
    Swap = 3,
    #[cfg(feature = "sdlgpu3-renderer")]
    SdlGpuCreate = 4,
}

struct NativeTransaction<'a> {
    control: &'a RuntimeControl,
    active: bool,
}

impl<'a> NativeTransaction<'a> {
    unsafe fn begin(
        control: &'a RuntimeControl,
        phase: NativePhase,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<Self> {
        let (swap_interval_policy, explicit_swap_interval) = control.native_gl_swap_interval();
        let faults = unsafe {
            ffi::dear_imgui_sdl3_native_begin(
                phase as u32,
                u32::from(control.expects_opengl()),
                swap_interval_policy,
                explicit_swap_interval,
                viewport,
            )
        };
        control.record_native_faults(faults);
        (faults == 0).then_some(Self {
            control,
            active: true,
        })
    }

    unsafe fn finish(mut self) -> u64 {
        let faults = unsafe { ffi::dear_imgui_sdl3_native_end() };
        self.active = false;
        self.control.record_native_faults(faults);
        faults
    }
}

impl Drop for NativeTransaction<'_> {
    fn drop(&mut self) {
        if self.active {
            let faults = unsafe { ffi::dear_imgui_sdl3_native_end() };
            self.control.record_native_faults(faults);
        }
    }
}

pub(crate) unsafe fn validate_platform_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    let actual = unsafe { ViewportPlatformState::capture(viewport) };
    let expected = control.owned_viewport(viewport);
    if expected.is_some_and(|expected| viewport_platform_state_eq(actual, expected)) {
        return true;
    }
    record_viewport_replacements(control, expected, actual);
    false
}

unsafe fn validate_renderer_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    let actual = unsafe { (*viewport).RendererUserData };
    match control.owned_renderer_viewport(viewport) {
        Some(expected) if expected == actual => true,
        None if actual.is_null() => true,
        _ => {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            false
        }
    }
}

fn viewport_platform_state_eq(left: ViewportPlatformState, right: ViewportPlatformState) -> bool {
    left.user_data == right.user_data
        && left.handle == right.handle
        && left.handle_raw == right.handle_raw
}

fn record_viewport_replacements(
    control: &RuntimeControl,
    expected: Option<ViewportPlatformState>,
    actual: ViewportPlatformState,
) {
    if expected.map_or(!actual.user_data.is_null(), |expected| {
        expected.user_data != actual.user_data
    }) {
        if actual.user_data.is_null() {
            control.record_platform_state_replaced("Viewport.PlatformUserData");
        } else {
            control.record_foreign_platform_user_data();
        }
    }
    if expected.map_or(!actual.handle.is_null(), |expected| {
        expected.handle != actual.handle
    }) {
        control.record_platform_state_replaced("Viewport.PlatformHandle");
    }
    if expected.map_or(!actual.handle_raw.is_null(), |expected| {
        expected.handle_raw != actual.handle_raw
    }) {
        control.record_platform_state_replaced("Viewport.PlatformHandleRaw");
    }
}

fn run_callback<R: Copy>(
    name: &'static str,
    fallback: R,
    callback: impl FnOnce(&RuntimeControl) -> R,
) -> R {
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_current_runtime(|control| {
            if control.validate_platform_ownership_bound() || control.callback_teardown_active() {
                callback(control)
            } else {
                fallback
            }
        })
        .unwrap_or(fallback)
    }));
    match result {
        Ok(result) => result,
        Err(_) => {
            let _ = with_current_runtime(|control| control.record_callback_panicked(name));
            fallback
        }
    }
}

#[cfg(test)]
pub(crate) unsafe fn create_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_create_window(viewport) }
}

#[cfg(test)]
pub(crate) unsafe fn destroy_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_destroy_window(viewport) }
}

#[cfg(test)]
pub(crate) unsafe fn render_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_render_window(viewport, std::ptr::null_mut()) }
}

#[cfg(test)]
pub(crate) unsafe fn swap_buffers_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_swap_buffers(viewport, std::ptr::null_mut()) }
}

#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
pub(crate) unsafe fn renderer_render_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_renderer_render_window(viewport, std::ptr::null_mut()) }
}

#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
pub(crate) unsafe fn renderer_set_window_size_callback_for_test(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    unsafe { sdl3_renderer_set_window_size(viewport, size) }
}
