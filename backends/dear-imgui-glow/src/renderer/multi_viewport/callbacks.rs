use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::{BackendFlags, Context, ViewportFlags, render::DrawData, sys};
use glow::HasContext;

use super::registry::{runtime_for_context, with_current_runtime};
use super::runtime::{GlowViewportError, RuntimeControl};

pub(super) fn preflight_callbacks(context: &Context) -> Result<(), GlowViewportError> {
    let binding = context.binding();
    binding.with_bound_context(|| {
        if !context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
        {
            return Err(GlowViewportError::PlatformBackendUnavailable);
        }

        if let Some(name) = context.io().backend_platform_name() {
            let name = name.to_string_lossy();
            if name.starts_with("dear-imgui-winit") {
                return Err(GlowViewportError::PlatformGlContextUnsupported {
                    backend: name.into_owned(),
                });
            }
        }

        let platform_io = context.platform_io();
        // SAFETY: PlatformIo owns this table for the duration of the borrow.
        let raw = unsafe { &*platform_io.as_raw() };
        for (available, callback) in [
            (raw.Platform_CreateWindow.is_some(), "Platform_CreateWindow"),
            (
                raw.Platform_DestroyWindow.is_some(),
                "Platform_DestroyWindow",
            ),
            (raw.Platform_RenderWindow.is_some(), "Platform_RenderWindow"),
            (raw.Platform_SwapBuffers.is_some(), "Platform_SwapBuffers"),
        ] {
            if !available {
                return Err(GlowViewportError::PlatformCallbackUnavailable { callback });
            }
        }

        if let Some(callback) = first_occupied_renderer_callback(raw) {
            return Err(GlowViewportError::RendererCallbackOccupied { callback });
        }
        Ok(())
    })
}

pub(super) fn claim_callbacks(control: &RuntimeControl, context: &mut Context) {
    let binding = context.binding();
    binding.with_bound_context(|| {
        unsafe {
            context
                .platform_io_mut()
                .set_renderer_render_window_raw(Some(renderer_render_window_sys));
        }
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
    });
    control.mark_callback_claimed();
}

pub(super) fn detect_callback_drift(control: &RuntimeControl) {
    if !control.should_detect_callback_drift() {
        return;
    }
    let result = control.binding().try_with_bound_context(|| {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Some("Renderer_RenderWindow");
        }
        // SAFETY: the ContextBinding keeps the current Context and its PlatformIO alive.
        let raw = unsafe { &*platform_io };
        first_renderer_callback_drift(raw)
    });
    if let Ok(Some(callback)) = result {
        control.record_callback_replaced(callback);
    }
}

pub(super) fn release_callbacks(control: &RuntimeControl) -> Result<(), GlowViewportError> {
    if control.callback_released() {
        return Ok(());
    }

    let current = unsafe { sys::igGetCurrentContext() };
    if current != control.context_raw() {
        return Err(GlowViewportError::BoundContextMismatch {
            expected: control.binding().id(),
        });
    }
    let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
    if platform_io.is_null() {
        return Err(GlowViewportError::PlatformIoUnavailable);
    }

    // SAFETY: the caller bound this control's live Context for the entire operation.
    let platform_io = unsafe { dear_imgui_rs::platform_io::PlatformIo::from_raw_mut(platform_io) };
    // SAFETY: PlatformIo owns this callback table for the duration of the mutable borrow.
    let raw = unsafe { &*platform_io.as_raw() };
    let drift = first_renderer_callback_drift(raw);
    if render_callback_matches(platform_io.renderer_render_window_raw()) {
        unsafe { platform_io.set_renderer_render_window_raw(None) };
    }

    if platform_io.renderer_callbacks_are_empty() {
        let io = unsafe { sys::igGetIO_Nil() };
        if !io.is_null() {
            let viewport_bit = BackendFlags::RENDERER_HAS_VIEWPORTS.bits();
            unsafe {
                (*io).BackendFlags = ((*io).BackendFlags & !viewport_bit)
                    | (control.prior_backend_flags().bits() & viewport_bit);
            }
        }
    }
    control.mark_callback_released();

    drift.map_or(Ok(()), |callback| {
        Err(GlowViewportError::RendererCallbackReplaced { callback })
    })
}

fn first_occupied_renderer_callback(raw: &sys::ImGuiPlatformIO) -> Option<&'static str> {
    [
        (raw.Renderer_CreateWindow.is_some(), "Renderer_CreateWindow"),
        (
            raw.Renderer_DestroyWindow.is_some(),
            "Renderer_DestroyWindow",
        ),
        (
            raw.Renderer_SetWindowSize.is_some(),
            "Renderer_SetWindowSize",
        ),
        (raw.Renderer_RenderWindow.is_some(), "Renderer_RenderWindow"),
        (raw.Renderer_SwapBuffers.is_some(), "Renderer_SwapBuffers"),
    ]
    .into_iter()
    .find_map(|(occupied, name)| occupied.then_some(name))
}

fn first_renderer_callback_drift(raw: &sys::ImGuiPlatformIO) -> Option<&'static str> {
    if raw.Renderer_CreateWindow.is_some() {
        return Some("Renderer_CreateWindow");
    }
    if raw.Renderer_DestroyWindow.is_some() {
        return Some("Renderer_DestroyWindow");
    }
    if raw.Renderer_SetWindowSize.is_some() {
        return Some("Renderer_SetWindowSize");
    }
    if !render_callback_matches(raw.Renderer_RenderWindow) {
        return Some("Renderer_RenderWindow");
    }
    raw.Renderer_SwapBuffers
        .is_some()
        .then_some("Renderer_SwapBuffers")
}

fn render_callback_matches(
    callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
) -> bool {
    callback.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            renderer_render_window_sys
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
        )
    })
}

fn render_viewport(control: &RuntimeControl, viewport: *mut sys::ImGuiViewport) {
    #[cfg(test)]
    control.maybe_panic_callback_for_test();

    if viewport.is_null() {
        control.record_fault(GlowViewportError::InvalidViewport);
        return;
    }

    control.with_renderer_callback("Renderer_RenderWindow", |renderer, gl| {
        // SAFETY: Dear ImGui supplied this live viewport for the current callback.
        let viewport = unsafe { &*viewport };
        let flags = ViewportFlags::from_bits_truncate(viewport.Flags);
        if !flags.contains(ViewportFlags::NO_RENDERER_CLEAR) {
            let color = renderer.viewport_clear_color;
            unsafe {
                gl.clear_color(color[0], color[1], color[2], color[3]);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        if viewport.DrawData.is_null() {
            return Ok(());
        }
        // SAFETY: DrawData is owned by this live viewport for the callback duration.
        let raw_draw_data = unsafe { &*viewport.DrawData };
        let draw_data = unsafe { DrawData::from_raw(raw_draw_data) };
        renderer.render_draw_data(gl, draw_data)
    });
}

/// Dear ImGui renderer callback for one secondary viewport.
///
/// # Safety
///
/// Dear ImGui must call this with a live viewport owned by the currently bound Context.
pub(crate) unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut sys::ImGuiViewport,
    _render_argument: *mut c_void,
) {
    let context_raw = unsafe { sys::igGetCurrentContext() };
    let Some(control) = runtime_for_context(context_raw) else {
        return;
    };
    if !control.is_callback_accessible() {
        return;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_current_runtime(|active| render_viewport(active, viewport));
    }));
    if result.is_err() {
        control.record_fault(GlowViewportError::CallbackPanicked {
            callback: "Renderer_RenderWindow",
        });
    }
}
