use super::GlowRenderer;
use dear_imgui_rs::{Context, ViewportFlags, internal::RawCast, render::DrawData, sys};
use glow::HasContext;
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());

struct ContextRendererState {
    ctx: usize,
    renderer: usize,
    borrowed: bool,
}

struct CurrentContextGuard {
    previous: *mut sys::ImGuiContext,
    target: *mut sys::ImGuiContext,
}

impl CurrentContextGuard {
    unsafe fn bind(target: *mut sys::ImGuiContext) -> Self {
        let previous = unsafe { sys::igGetCurrentContext() };
        if previous != target {
            unsafe { sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe { sys::igSetCurrentContext(self.previous) };
        }
    }
}

fn upsert_renderer_state(ctx: *mut sys::ImGuiContext, renderer: *mut GlowRenderer) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    let renderer = renderer as usize;
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(entry) = renderers.iter_mut().find(|entry| entry.ctx == ctx) {
        entry.renderer = renderer;
        return;
    }

    renderers.push(ContextRendererState {
        ctx,
        renderer,
        borrowed: false,
    });
}

fn remove_renderer_state_for_context(ctx: *mut sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.ctx != ctx);
}

fn remove_renderer_state_for_renderer(renderer: *mut GlowRenderer) {
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.renderer != renderer);
}

struct RendererBorrowGuard {
    ctx: usize,
    renderer: *mut GlowRenderer,
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        let mut renderers = RENDERERS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = renderers
            .iter_mut()
            .find(|entry| entry.ctx == self.ctx && entry.renderer == self.renderer as usize)
        {
            entry.borrowed = false;
        }
    }
}

impl Deref for RendererBorrowGuard {
    type Target = GlowRenderer;

    fn deref(&self) -> &Self::Target {
        // Safety: guarded by the context-local borrow flag and pointer validity contract of `enable`.
        unsafe { &*self.renderer }
    }
}

impl DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: guarded by the context-local borrow flag and pointer validity contract of `enable`.
        unsafe { &mut *self.renderer }
    }
}

#[inline]
fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let ctx = unsafe { sys::igGetCurrentContext() } as usize;
    if ctx == 0 {
        return None;
    }

    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(entry) = renderers.iter_mut().find(|entry| entry.ctx == ctx) else {
        return None;
    };
    if entry.renderer == 0 {
        return None;
    }
    if entry.borrowed {
        eprintln!(
            "dear-imgui-glow: GlowRenderer is already mutably borrowed (multi-viewport reentrancy?); skipping callback"
        );
        return None;
    }

    entry.borrowed = true;
    Some(RendererBorrowGuard {
        ctx,
        renderer: entry.renderer as *mut GlowRenderer,
    })
}

/// Clear stored renderer state for `renderer`.
///
/// This is used to make multi-viewport callbacks become a no-op when the
/// renderer is dropped without an explicit call to [`disable`].
pub(crate) fn clear_for_drop(renderer: *mut GlowRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

/// Enable Glow multi-viewport rendering for the given ImGui context and renderer.
///
/// This registers a `Renderer_RenderWindow` callback in `ImGuiPlatformIO`, which
/// Dear ImGui will call from `Context::render_platform_windows_default()` for each
/// secondary viewport.
///
/// The platform backend (e.g. SDL3) is expected to:
/// - create/destroy platform windows;
/// - set the appropriate OpenGL context current in `Platform_RenderWindow`;
/// - swap buffers in `Platform_SwapBuffers`.
///
/// This function assumes that `renderer` owns a `glow::Context` (the common case);
/// if `GlowRenderer` was created with an external context (`gl_context()` returns
/// `None`), the multi-viewport callback will early-return and do nothing.
pub fn enable(renderer: &mut GlowRenderer, imgui_context: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    // Install raw Renderer_RenderWindow callback. We don't need the typed
    // trampolines here, as we never expose Viewport typed wrappers.
    let platform_io = imgui_context.platform_io_mut();
    platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    upsert_renderer_state(imgui_context.as_raw(), renderer as *mut _);
}

/// Disable Glow multi-viewport rendering and clear the renderer callback.
pub fn disable(imgui_context: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    let platform_io = imgui_context.platform_io_mut();
    platform_io.set_renderer_render_window_raw(None);
    remove_renderer_state_for_context(imgui_context.as_raw());
}

/// Backwards-compatible helper mirroring older naming.
///
/// Prefer using [`enable`] directly so the renderer instance is clearly threaded
/// through your setup code.
#[deprecated(
    since = "0.6.0",
    note = "use multi_viewport::enable(renderer, imgui_context) instead"
)]
pub fn init_multi_viewport_support(_imgui_context: &mut Context) {
    // Kept only to avoid breaking existing code that might call this.
    // Without a renderer reference there is nothing useful to do here.
}

/// Shutdown helper that destroys platform windows and clears callbacks.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    context.destroy_platform_windows();
    disable(context);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        shaders::Shaders, state::GlStateBackup, texture::SimpleTextureMap, versions::GlVersion,
    };

    fn make_test_renderer() -> GlowRenderer {
        GlowRenderer {
            shaders: Shaders {
                program: None,
                attrib_location_tex: None,
                attrib_location_proj_mtx: None,
                attrib_location_color_gamma: None,
                attrib_location_vtx_pos: 0,
                attrib_location_vtx_uv: 0,
                attrib_location_vtx_color: 0,
            },
            state_backup: GlStateBackup::default(),
            vbo_handle: None,
            ebo_handle: None,
            font_atlas_texture: None,
            font_atlas_texture_data: std::ptr::null_mut(),
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
            is_destroyed: false,
            gl_context: None,
            texture_map: Some(Box::new(SimpleTextureMap::default())),
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn enable_targets_passed_context() {
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let pio_a = unsafe { sys::igGetPlatformIO_ContextPtr(raw_a) };

        unsafe {
            sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let pio_b = unsafe { sys::igGetPlatformIO_ContextPtr(raw_b) };

        let mut renderer = make_test_renderer();
        enable(&mut renderer, &mut ctx_a);

        unsafe {
            assert_eq!(sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_RenderWindow.is_some());
            assert!((*pio_b).Renderer_RenderWindow.is_none());
        }

        disable(&mut ctx_a);

        unsafe {
            assert_eq!(sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_RenderWindow.is_none());
        }

        unsafe {
            sys::igSetCurrentContext(raw_a);
        }
        drop(ctx_a);
        unsafe {
            sys::igSetCurrentContext(raw_b);
        }
        drop(ctx_b);
        drop(renderer);
    }

    #[test]
    fn renderer_state_is_context_local() {
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let mut renderer_a = make_test_renderer();
        enable(&mut renderer_a, &mut ctx_a);

        unsafe {
            sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let mut ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let mut renderer_b = make_test_renderer();
        enable(&mut renderer_b, &mut ctx_b);

        unsafe {
            sys::igSetCurrentContext(raw_a);
            {
                let borrowed = borrow_renderer().expect("renderer for context A");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_a as *const _);
            }

            sys::igSetCurrentContext(raw_b);
            {
                let borrowed = borrow_renderer().expect("renderer for context B");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_b as *const _);
            }
        }

        unsafe {
            sys::igSetCurrentContext(raw_b);
        }
        disable(&mut ctx_b);
        drop(ctx_b);
        drop(renderer_b);

        unsafe {
            sys::igSetCurrentContext(raw_a);
        }
        disable(&mut ctx_a);
        drop(ctx_a);
        drop(renderer_a);
    }

    #[test]
    fn renderer_destroy_clears_renderer_state() {
        let mut ctx = Context::create();
        let raw = ctx.as_raw();
        let mut renderer = make_test_renderer();

        enable(&mut renderer, &mut ctx);

        unsafe {
            sys::igSetCurrentContext(raw);
            assert!(borrow_renderer().is_some());
        }

        unsafe extern "system" fn fake_gl_get_string(_name: u32) -> *const u8 {
            b"4.6\0".as_ptr()
        }
        unsafe extern "system" fn fake_gl_get_string_i(_name: u32, _index: u32) -> *const u8 {
            b"\0".as_ptr()
        }
        unsafe extern "system" fn fake_gl_get_integer_v(_name: u32, data: *mut i32) {
            if !data.is_null() {
                unsafe { *data = 4 };
            }
        }

        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                if name == "glGetString" {
                    fake_gl_get_string as *const () as *const c_void
                } else if name == "glGetIntegerv" {
                    fake_gl_get_integer_v as *const () as *const c_void
                } else if name == "glGetStringi" {
                    fake_gl_get_string_i as *const () as *const c_void
                } else {
                    std::ptr::null()
                }
            })
        };
        renderer.destroy(&gl);

        unsafe {
            sys::igSetCurrentContext(raw);
            assert!(borrow_renderer().is_none());
        }

        disable(&mut ctx);
        drop(ctx);
        drop(renderer);
    }
}

/// Renderer callback used by Dear ImGui for each secondary viewport.
///
/// This corresponds to `ImGuiPlatformIO::Renderer_RenderWindow`.
///
/// Safety: called from C with a valid `ImGuiViewport*` while the ImGui
/// context and registered renderer are still alive.
pub unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut renderer = match borrow_renderer() {
            Some(r) => r,
            None => return,
        };

        // We currently only support the common case where GlowRenderer owns the
        // GL context. If the context is externally managed, the application
        // should render viewports by calling `render_with_context` manually.
        let gl_rc = match renderer.gl_context() {
            Some(rc) => rc.clone(),
            None => return,
        };
        let gl = &*gl_rc;

        // Safety: viewport was checked for null above.
        let vp_ref = unsafe { &*viewport };

        // Clear the viewport if needed using Dear ImGui's ViewportFlags.
        let flags = ViewportFlags::from_bits_truncate(vp_ref.Flags);
        if !flags.contains(ViewportFlags::NO_RENDERER_CLEAR) {
            let c = renderer.viewport_clear_color;
            unsafe {
                gl.clear_color(c[0], c[1], c[2], c[3]);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        // Render the draw data for this viewport, if present.
        if !vp_ref.DrawData.is_null() {
            // Safety: DrawData pointer is owned by Dear ImGui for the duration
            // of this callback.
            let raw_dd: &mut sys::ImDrawData = unsafe { &mut *vp_ref.DrawData };
            let draw_data: &mut DrawData = unsafe { DrawData::from_raw_mut(raw_dd) };

            if let Err(err) = renderer.render_with_context(gl, draw_data) {
                eprintln!("dear-imgui-glow: error rendering viewport: {:?}", err);
            }
        }
    }));

    if res.is_err() {
        eprintln!("dear-imgui-glow: panic in Renderer_RenderWindow callback");
        std::process::abort();
    }
}
