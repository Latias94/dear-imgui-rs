use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use dear_imgui_rs::sys;

use crate::core::ffi;
#[cfg(feature = "sdlgpu3-renderer")]
use crate::runtime::NativeRendererKind;
use crate::runtime::{RuntimeControl, with_current_runtime};

use super::{PlatformCallbackSlot, PlatformCallbacks, ViewportPlatformState};

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    crate::runtime::register_runtime(control);
}

pub(super) unsafe extern "C" fn sdl3_create_window(viewport: *mut sys::ImGuiViewport) {
    run_platform_callback(
        PlatformCallbackSlot::CreateWindow,
        (),
        |control, callbacks| unsafe {
            if viewport.is_null() || !control.validate_platform_ownership_bound() {
                return;
            }
            let Some(live) = control.live_viewport(viewport) else {
                control.record_platform_state_replaced("Viewport liveness");
                return;
            };
            let Some(initial_state) = control.capture_viewport_platform_state(live) else {
                control.record_platform_state_replaced("Viewport liveness");
                return;
            };
            if !initial_state.user_data.is_null() {
                control.record_foreign_platform_user_data();
                (*live).PlatformRequestClose = true;
                return;
            }
            let Some(callback) = callbacks.create_window() else {
                return;
            };
            let Some(transaction) =
                NativeTransaction::begin(control, NativePhase::Create, viewport)
            else {
                control.mark_viewport_failed(viewport);
                return;
            };
            callback(viewport);
            let native_faults = transaction.finish();
            let Some(live) = control.live_viewport(viewport) else {
                control.record_viewport_creation_failed();
                return;
            };
            let Some(state) = control.capture_viewport_platform_state(live) else {
                control.record_viewport_creation_failed();
                return;
            };
            if !state.user_data.is_null() {
                control.remember_owned_viewport(viewport, state);
            }
            if state.user_data.is_null() || state.handle.is_null() || native_faults != 0 {
                control.record_viewport_creation_failed();
                control.mark_viewport_failed(viewport);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let _ = run_platform_callback(
        PlatformCallbackSlot::DestroyWindow,
        false,
        |control, callbacks| unsafe {
            if viewport.is_null() {
                return false;
            }
            let Some(live) = control.live_viewport(viewport) else {
                control.record_platform_state_replaced("Viewport liveness");
                return false;
            };
            let Some(actual) = control.capture_viewport_platform_state(live) else {
                control.record_platform_state_replaced("Viewport liveness");
                return false;
            };
            control.forget_failed_viewport(viewport);
            let Some(callback) = callbacks.destroy_window() else {
                return false;
            };
            let Some(expected) = control.take_owned_viewport(viewport) else {
                record_viewport_replacements(control, None, actual);
                control.defer_platform_viewport_restore(viewport, actual);
                control.clear_viewport_platform_state(viewport);
                return true;
            };

            if viewport_platform_state_eq(actual, expected) {
                callback(viewport);
                control.clear_viewport_platform_state(viewport);
                return true;
            }

            record_viewport_replacements(control, Some(expected), actual);
            if !control.restore_viewport_platform_state(viewport, expected) {
                control.record_platform_state_replaced("Viewport liveness");
                return false;
            }
            callback(viewport);
            control.clear_viewport_platform_state(viewport);
            control.defer_platform_viewport_restore(viewport, actual);
            true
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_show_window(viewport: *mut sys::ImGuiViewport) {
    run_owned_platform_callback(
        PlatformCallbackSlot::ShowWindow,
        viewport,
        (),
        |_, callbacks| unsafe {
            if let Some(callback) = callbacks.show_window() {
                callback(viewport);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_update_window(viewport: *mut sys::ImGuiViewport) {
    run_owned_platform_callback(
        PlatformCallbackSlot::UpdateWindow,
        viewport,
        (),
        |_, callbacks| unsafe {
            if let Some(callback) = callbacks.update_window() {
                callback(viewport);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_set_window_pos(
    viewport: *mut sys::ImGuiViewport,
    pos: *const sys::ImVec2,
) {
    if pos.is_null() {
        return;
    }
    run_owned_platform_callback(
        PlatformCallbackSlot::SetWindowPos,
        viewport,
        (),
        |_, callbacks| {
            let _ = unsafe { callbacks.invoke_set_window_pos(viewport, pos) };
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_get_window_pos(
    viewport: *mut sys::ImGuiViewport,
    out_pos: *mut sys::ImVec2,
) {
    if out_pos.is_null() {
        return;
    }
    let fallback = sys::ImVec2 { x: 0.0, y: 0.0 };
    unsafe { out_pos.write(fallback) };
    run_owned_platform_callback(
        PlatformCallbackSlot::GetWindowPos,
        viewport,
        (),
        |control, callbacks| unsafe {
            let Some(live) = control.live_viewport(viewport) else {
                return;
            };
            let mut result = (*live).Pos;
            if callbacks.invoke_get_window_pos(viewport, &mut result) {
                out_pos.write(result);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_set_window_size(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    if size.is_null() {
        return;
    }
    run_owned_platform_callback(
        PlatformCallbackSlot::SetWindowSize,
        viewport,
        (),
        |_, callbacks| {
            let _ = unsafe { callbacks.invoke_set_window_size(viewport, size) };
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_get_window_size(
    viewport: *mut sys::ImGuiViewport,
    out_size: *mut sys::ImVec2,
) {
    if out_size.is_null() {
        return;
    }
    let fallback = sys::ImVec2 { x: 0.0, y: 0.0 };
    unsafe { out_size.write(fallback) };
    run_owned_platform_callback(
        PlatformCallbackSlot::GetWindowSize,
        viewport,
        (),
        |control, callbacks| unsafe {
            let Some(live) = control.live_viewport(viewport) else {
                return;
            };
            let mut result = (*live).Size;
            if callbacks.invoke_get_window_size(viewport, &mut result) {
                out_size.write(result);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_get_window_framebuffer_scale(
    viewport: *mut sys::ImGuiViewport,
    out_scale: *mut sys::ImVec2,
) {
    if out_scale.is_null() {
        return;
    }
    let fallback = sys::ImVec2 { x: 0.0, y: 0.0 };
    unsafe { out_scale.write(fallback) };
    run_owned_platform_callback(
        PlatformCallbackSlot::GetWindowFramebufferScale,
        viewport,
        (),
        |control, callbacks| unsafe {
            let Some(live) = control.live_viewport(viewport) else {
                return;
            };
            let mut result = (*live).FramebufferScale;
            if callbacks.invoke_get_window_framebuffer_scale(viewport, &mut result) {
                out_scale.write(result);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_set_window_focus(viewport: *mut sys::ImGuiViewport) {
    run_owned_platform_callback(
        PlatformCallbackSlot::SetWindowFocus,
        viewport,
        (),
        |_, callbacks| unsafe {
            if let Some(callback) = callbacks.set_window_focus() {
                callback(viewport);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_get_window_focus(viewport: *mut sys::ImGuiViewport) -> bool {
    run_owned_platform_callback(
        PlatformCallbackSlot::GetWindowFocus,
        viewport,
        false,
        |control, callbacks| unsafe {
            let Some(live) = control.live_viewport(viewport) else {
                return false;
            };
            callbacks.get_window_focus().map_or(
                (*live).Flags & sys::ImGuiViewportFlags_IsFocused != 0,
                |callback| callback(viewport),
            )
        },
    )
}

pub(super) unsafe extern "C" fn sdl3_get_window_minimized(
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    run_owned_platform_callback(
        PlatformCallbackSlot::GetWindowMinimized,
        viewport,
        false,
        |control, callbacks| unsafe {
            let Some(live) = control.live_viewport(viewport) else {
                return false;
            };
            callbacks.get_window_minimized().map_or(
                (*live).Flags & sys::ImGuiViewportFlags_IsMinimized != 0,
                |callback| callback(viewport),
            )
        },
    )
}

pub(super) unsafe extern "C" fn sdl3_set_window_title(
    viewport: *mut sys::ImGuiViewport,
    title: *const std::ffi::c_char,
) {
    if title.is_null() {
        return;
    }
    run_owned_platform_callback(
        PlatformCallbackSlot::SetWindowTitle,
        viewport,
        (),
        |_, callbacks| unsafe {
            if let Some(callback) = callbacks.set_window_title() {
                callback(viewport, title);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_render_window(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_platform_callback(
        PlatformCallbackSlot::RenderWindow,
        (),
        |control, callbacks| unsafe {
            if viewport.is_null() || control.viewport_failed(viewport) {
                return;
            }
            if !validate_platform_viewport_state(control, viewport) {
                control.mark_viewport_failed(viewport);
                return;
            }
            let Some(callback) = callbacks.render_window() else {
                return;
            };
            let Some(transaction) =
                NativeTransaction::begin(control, NativePhase::Render, viewport)
            else {
                control.mark_viewport_failed(viewport);
                return;
            };
            callback(viewport, render_argument);
            if transaction.finish() != 0 {
                control.mark_viewport_failed(viewport);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_swap_buffers(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_platform_callback(
        PlatformCallbackSlot::SwapBuffers,
        (),
        |control, callbacks| unsafe {
            if viewport.is_null() || control.viewport_failed(viewport) {
                return;
            }
            if !validate_platform_viewport_state(control, viewport) {
                control.mark_viewport_failed(viewport);
                return;
            }
            let Some(callback) = callbacks.swap_buffers() else {
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
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_set_window_alpha(
    viewport: *mut sys::ImGuiViewport,
    alpha: f32,
) {
    run_owned_platform_callback(
        PlatformCallbackSlot::SetWindowAlpha,
        viewport,
        (),
        |_, callbacks| unsafe {
            if let Some(callback) = callbacks.set_window_alpha() {
                callback(viewport, alpha);
            }
        },
    );
}

pub(super) unsafe extern "C" fn sdl3_create_vk_surface(
    viewport: *mut sys::ImGuiViewport,
    instance: sys::ImU64,
    allocators: *const c_void,
    out_surface: *mut sys::ImU64,
) -> std::os::raw::c_int {
    if out_surface.is_null() {
        return 1;
    }
    unsafe { out_surface.write(0) };
    let result = run_owned_platform_callback(
        PlatformCallbackSlot::CreateVkSurface,
        viewport,
        1,
        |_, callbacks| unsafe {
            callbacks.create_vk_surface().map_or(1, |callback| {
                callback(viewport, instance, allocators, out_surface)
            })
        },
    );
    if result != 0 {
        unsafe { out_surface.write(0) };
    }
    result
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
        let actual_renderer = control
            .viewport_renderer_user_data(viewport)
            .unwrap_or(std::ptr::null_mut());
        if !actual_renderer.is_null() {
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
            if let Some(renderer_user_data) = control.viewport_renderer_user_data(viewport) {
                control.remember_owned_renderer_viewport(viewport, renderer_user_data);
            } else {
                control.record_renderer_state_replaced("Viewport liveness(create)");
                control.mark_viewport_failed(viewport);
            }
        }

        #[cfg(feature = "sdlgpu3-renderer")]
        {
            if control.native_renderer() != NativeRendererKind::SdlGpu3 {
                callback(viewport);
                if let Some(renderer_user_data) = control.viewport_renderer_user_data(viewport) {
                    control.remember_owned_renderer_viewport(viewport, renderer_user_data);
                } else {
                    control.record_renderer_state_replaced("Viewport liveness(create)");
                    control.mark_viewport_failed(viewport);
                }
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
        let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
        control.mark_viewport_failed(viewport);
        return;
    }
    let Some(renderer_user_data) = control.viewport_renderer_user_data(viewport) else {
        control.record_renderer_state_replaced("Viewport liveness(create)");
        control.mark_viewport_failed(viewport);
        return;
    };
    if renderer_user_data.is_null() {
        control.record_renderer_state_replaced("Viewport.RendererUserData(create)");
        control.mark_viewport_failed(viewport);
        return;
    }
    control.remember_owned_renderer_viewport(viewport, renderer_user_data);
}

pub(super) unsafe extern "C" fn sdl3_renderer_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let _ = run_callback("Renderer_DestroyWindow", false, |control| unsafe {
        if viewport.is_null() {
            return false;
        }
        if control.live_viewport(viewport).is_none() {
            control.record_renderer_state_replaced("Viewport liveness");
            return false;
        }
        let actual_renderer = control
            .viewport_renderer_user_data(viewport)
            .unwrap_or(std::ptr::null_mut());
        if !control.validate_renderer_ownership_bound() {
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
            return true;
        }
        let Some(callback) = control.original_renderer_destroy_window() else {
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
            return true;
        };
        let Some((expected_platform, actual_platform)) = control.inspect_owned_viewport(viewport)
        else {
            let Some(actual_platform) = control.capture_viewport_platform_state(viewport) else {
                control.record_platform_state_replaced("Viewport liveness");
                return false;
            };
            record_viewport_replacements(control, None, actual_platform);
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
            return true;
        };
        let expected_renderer = control.owned_renderer_viewport(viewport);
        if expected_renderer.is_none() && actual_renderer.is_null() {
            return true;
        }
        let Some(expected_renderer) = expected_renderer else {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
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

        if !control.restore_viewport_platform_state(viewport, expected_platform) {
            control.record_platform_state_replaced("Viewport liveness");
            return false;
        }
        if !control.set_viewport_renderer_user_data(viewport, expected_renderer) {
            return false;
        }
        callback(viewport);
        let _ = control.set_viewport_renderer_user_data(viewport, std::ptr::null_mut());
        control.forget_owned_renderer_viewport(viewport);
        true
    });
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
            let mut first_fault = 0;
            let faults =
                ffi::dear_imgui_sdl3_backend_sdlgpu3_render_viewport(viewport, &mut first_fault);
            control.record_native_faults(faults, first_fault);
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
        control.record_native_faults(faults, faults);
        (faults == 0).then_some(Self {
            control,
            active: true,
        })
    }

    unsafe fn finish(mut self) -> u64 {
        let mut first_fault = 0;
        let faults = unsafe { ffi::dear_imgui_sdl3_native_end(&mut first_fault) };
        self.active = false;
        self.control.record_native_faults(faults, first_fault);
        faults
    }
}

impl Drop for NativeTransaction<'_> {
    fn drop(&mut self) {
        if self.active {
            let mut first_fault = 0;
            let faults = unsafe { ffi::dear_imgui_sdl3_native_end(&mut first_fault) };
            self.control.record_native_faults(faults, first_fault);
        }
    }
}

pub(crate) unsafe fn validate_platform_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    if viewport.is_null() {
        return false;
    }
    let Some((expected, actual)) = control.inspect_owned_viewport(viewport) else {
        control.record_platform_state_replaced("Viewport liveness");
        return false;
    };
    if viewport_platform_state_eq(actual, expected) {
        return true;
    }
    record_viewport_replacements(control, Some(expected), actual);
    false
}

unsafe fn validate_renderer_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    let Some(actual) = (unsafe { control.viewport_renderer_user_data(viewport) }) else {
        control.record_renderer_state_replaced("Viewport liveness");
        return false;
    };
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

fn run_platform_callback<R: Copy>(
    slot: PlatformCallbackSlot,
    fallback: R,
    callback: impl FnOnce(&RuntimeControl, &PlatformCallbacks) -> R,
) -> R {
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_current_runtime(|control| {
            if !control.validate_platform_callback_slot(slot) {
                return fallback;
            }
            let Some(callbacks) = control.original_platform_callbacks() else {
                control.record_platform_state_replaced("platform callback ownership");
                return fallback;
            };
            callback(control, &callbacks)
        })
        .unwrap_or(fallback)
    }));
    match result {
        Ok(result) => result,
        Err(_) => {
            let _ = with_current_runtime(|control| control.record_callback_panicked(slot.name()));
            fallback
        }
    }
}

fn run_owned_platform_callback<R: Copy>(
    slot: PlatformCallbackSlot,
    viewport: *mut sys::ImGuiViewport,
    fallback: R,
    callback: impl FnOnce(&RuntimeControl, &PlatformCallbacks) -> R,
) -> R {
    run_platform_callback(slot, fallback, |control, callbacks| {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return fallback;
        }
        if unsafe { !validate_platform_viewport_state(control, viewport) } {
            control.mark_viewport_failed(viewport);
            return fallback;
        }
        callback(control, callbacks)
    })
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
