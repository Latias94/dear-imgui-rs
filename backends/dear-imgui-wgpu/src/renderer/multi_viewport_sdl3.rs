// Multi-viewport support (SDL3 platform backend + WGPU renderer)
//
// This mirrors the existing winit multi-viewport renderer callbacks, but reads
// ImGuiViewport::PlatformHandle as an SDL_WindowID and creates per-viewport
// WGPU surfaces from SDL3 native window handles.

use super::*;
use dear_imgui_rs::Context;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::{PlatformIo, Viewport};
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;

#[cfg(target_arch = "wasm32")]
compile_error!("`multi-viewport-sdl3` is not supported on wasm32 targets.");

#[allow(unused_macros)]
macro_rules! mvlog {
    ($($arg:tt)*) => {
        if cfg!(feature = "mv-log") { eprintln!($($arg)*); }
    };
}

/// Per-viewport WGPU data stored in ImGuiViewport::RendererUserData
struct ViewportWgpuData {
    pub device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub pending_frame: Option<wgpu::SurfaceTexture>,
    pub pending_reconfigure: bool,
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());
static VIEWPORT_DATA: Mutex<Vec<usize>> = Mutex::new(Vec::new());

struct ContextRendererState {
    ctx: usize,
    renderer: usize,
    borrowed: bool,
    global: Option<GlobalHandles>,
}

/// Failure to acquire the renderer callback table for WGPU multi-viewport rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallbackOwnershipError {
    /// Another renderer backend still owns at least one `Renderer_*` callback slot.
    #[error("ImGuiPlatformIO renderer callbacks are already owned by another backend")]
    RendererCallbacksOccupied,

    /// The current Dear ImGui artifact cannot safely bridge `Renderer_SetWindowSize`.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
}

struct CurrentContextGuard {
    previous: *mut dear_imgui_rs::sys::ImGuiContext,
    target: *mut dear_imgui_rs::sys::ImGuiContext,
}

impl CurrentContextGuard {
    unsafe fn bind(target: *mut dear_imgui_rs::sys::ImGuiContext) -> Self {
        let previous = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        if previous != target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.previous) };
        }
    }
}

#[derive(Clone)]
struct GlobalHandles {
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    queue: wgpu::Queue,
    render_target_format: wgpu::TextureFormat,
}

fn upsert_renderer_state(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
    global: Option<GlobalHandles>,
) {
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
        entry.global = global;
        return;
    }

    renderers.push(ContextRendererState {
        ctx,
        renderer,
        borrowed: false,
        global,
    });
}

fn remove_renderer_state_for_context(ctx: *mut dear_imgui_rs::sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.ctx != ctx);
}

fn remove_renderer_state_for_renderer(renderer: *mut WgpuRenderer) {
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.renderer != renderer);
}

fn register_viewport_data(ptr: *mut ViewportWgpuData) {
    if ptr.is_null() {
        return;
    }

    let ptr = ptr as usize;
    let mut items = VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !items.contains(&ptr) {
        items.push(ptr);
    }
}

fn unregister_viewport_data(ptr: *mut ViewportWgpuData) {
    if ptr.is_null() {
        return;
    }

    let ptr = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| *entry != ptr);
}

fn is_wgpu_viewport_data(ptr: *mut ViewportWgpuData) -> bool {
    if ptr.is_null() {
        return false;
    }

    let ptr = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .contains(&ptr)
}

fn global_handles() -> Option<GlobalHandles> {
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() } as usize;
    if ctx == 0 {
        return None;
    }

    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|entry| entry.ctx == ctx)
        .and_then(|entry| entry.global.clone())
}

fn has_renderer_state_for_context(ctx: *mut dear_imgui_rs::sys::ImGuiContext) -> bool {
    if ctx.is_null() {
        return false;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|entry| entry.ctx == ctx)
}

fn unary_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn render_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn renderer_callbacks_are_owned_by_wgpu(platform_io: &PlatformIo) -> bool {
    unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) && unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) && platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
        && render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        )
        && render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        )
}

fn try_claim_renderer_callbacks(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    platform_io: &mut PlatformIo,
    aggregate_hooks_available: bool,
) -> Result<(), CallbackOwnershipError> {
    if !platform_io.renderer_callbacks_are_empty() {
        if renderer_callbacks_are_owned_by_wgpu(platform_io)
            && has_renderer_state_for_context(context)
        {
            return Ok(());
        }
        return Err(CallbackOwnershipError::RendererCallbacksOccupied);
    }

    if !aggregate_hooks_available {
        return Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable);
    }

    platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
    platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
    // Rendering is owned by the renderer backend. Platform_RenderWindow and
    // Platform_SwapBuffers remain reserved for the platform backend.
    platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
    Ok(())
}

/// Enable WGPU multi-viewport after claiming an empty or already-owned renderer callback table.
pub fn enable(
    renderer: &mut WgpuRenderer,
    imgui_context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    let raw_context = imgui_context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };

    try_claim_renderer_callbacks(
        raw_context,
        imgui_context.platform_io_mut(),
        dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
    )?;

    let global = renderer.backend_data.as_ref().map(|backend| GlobalHandles {
        instance: backend.instance.clone(),
        adapter: backend.adapter.clone(),
        device: backend.device.clone(),
        #[cfg(feature = "wgpu-30")]
        queue: backend.queue.clone(),
        render_target_format: backend.render_target_format,
    });
    upsert_renderer_state(raw_context, renderer as *mut _, global);
    Ok(())
}

pub(crate) fn clear_for_drop(renderer: *mut WgpuRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

/// Disable WGPU multi-viewport callbacks and clear stored globals (SDL3 platform).
pub fn disable(imgui_context: &mut Context) {
    let raw_context = imgui_context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };

    let platform_io = imgui_context.platform_io_mut();
    if unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) {
        platform_io.set_renderer_create_window_raw(None);
    }
    if unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) {
        platform_io.set_renderer_destroy_window_raw(None);
    }
    platform_io.clear_renderer_set_window_size_if_pointer_callback(renderer_set_window_size_sys);
    if render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys,
    ) {
        platform_io.set_renderer_render_window_raw(None);
    }
    if render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys,
    ) {
        platform_io.set_renderer_swap_buffers_raw(None);
    }
    remove_renderer_state_for_context(raw_context);
}

/// Convenience helper that destroys all platform windows and disables callbacks.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    context.destroy_platform_windows();
    disable(context);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex as TestMutex, OnceLock};

    fn lock_context() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<TestMutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| TestMutex::new(())).lock().unwrap()
    }

    unsafe extern "C" fn platform_slot_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
    }

    unsafe extern "C" fn renderer_slot_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
    }

    unsafe extern "C" fn renderer_unary_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    ) {
    }

    unsafe extern "C" fn renderer_size_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _size: *const dear_imgui_rs::sys::ImVec2,
    ) {
    }

    #[test]
    fn enable_targets_passed_context() {
        let _guard = lock_context();
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let pio_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };

        unsafe {
            (*pio_a).Platform_RenderWindow = Some(platform_slot_sentinel);
            (*pio_a).Platform_SwapBuffers = Some(platform_slot_sentinel);
        }

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let pio_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };

        let mut renderer = WgpuRenderer::empty();
        enable(&mut renderer, &mut ctx_a).expect("empty renderer callback table");

        unsafe {
            assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_CreateWindow.is_some());
            assert!((*pio_a).Renderer_DestroyWindow.is_some());
            assert!((*pio_a).Renderer_SetWindowSize.is_some());
            assert!(render_callback_matches(
                (*pio_a).Renderer_RenderWindow,
                renderer_render_window_sys
            ));
            assert!(render_callback_matches(
                (*pio_a).Renderer_SwapBuffers,
                renderer_swap_buffers_sys
            ));
            assert!(render_callback_matches(
                (*pio_a).Platform_RenderWindow,
                platform_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*pio_a).Platform_SwapBuffers,
                platform_slot_sentinel
            ));

            assert!((*pio_b).Renderer_CreateWindow.is_none());
            assert!((*pio_b).Renderer_DestroyWindow.is_none());
            assert!((*pio_b).Renderer_SetWindowSize.is_none());
            assert!((*pio_b).Renderer_RenderWindow.is_none());
            assert!((*pio_b).Renderer_SwapBuffers.is_none());
            assert!((*pio_b).Platform_RenderWindow.is_none());
            assert!((*pio_b).Platform_SwapBuffers.is_none());
        }

        disable(&mut ctx_a);

        unsafe {
            assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_CreateWindow.is_none());
            assert!((*pio_a).Renderer_DestroyWindow.is_none());
            assert!((*pio_a).Renderer_SetWindowSize.is_none());
            assert!((*pio_a).Renderer_RenderWindow.is_none());
            assert!((*pio_a).Renderer_SwapBuffers.is_none());
            assert!(render_callback_matches(
                (*pio_a).Platform_RenderWindow,
                platform_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*pio_a).Platform_SwapBuffers,
                platform_slot_sentinel
            ));

            (*pio_a).Platform_RenderWindow = None;
            (*pio_a).Platform_SwapBuffers = None;
        }

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        }
        drop(ctx_a);
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
        }
        drop(ctx_b);
        drop(renderer);
    }

    #[test]
    fn renderer_callback_conflict_preserves_foreign_slots() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(ctx.as_raw()) };
        let mut renderer = WgpuRenderer::empty();

        unsafe {
            (*platform_io).Renderer_RenderWindow = Some(renderer_slot_sentinel);
            (*platform_io).Renderer_SwapBuffers = Some(renderer_slot_sentinel);
        }

        assert_eq!(
            enable(&mut renderer, &mut ctx),
            Err(CallbackOwnershipError::RendererCallbacksOccupied)
        );

        unsafe {
            assert!(render_callback_matches(
                (*platform_io).Renderer_RenderWindow,
                renderer_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*platform_io).Renderer_SwapBuffers,
                renderer_slot_sentinel
            ));
        }

        disable(&mut ctx);

        unsafe {
            assert!(render_callback_matches(
                (*platform_io).Renderer_RenderWindow,
                renderer_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*platform_io).Renderer_SwapBuffers,
                renderer_slot_sentinel
            ));
            (*platform_io).Renderer_RenderWindow = None;
            (*platform_io).Renderer_SwapBuffers = None;
        }
    }

    #[test]
    fn missing_aggregate_hooks_reject_install_without_mutation() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let raw = ctx.as_raw();

        assert_eq!(
            try_claim_renderer_callbacks(raw, ctx.platform_io_mut(), false),
            Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable)
        );
        assert!(ctx.platform_io().renderer_callbacks_are_empty());
        assert!(!has_renderer_state_for_context(raw));
    }

    #[test]
    fn disable_preserves_renderer_callbacks_replaced_after_enable() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let mut renderer = WgpuRenderer::empty();

        enable(&mut renderer, &mut ctx).expect("empty renderer callback table");
        {
            let platform_io = ctx.platform_io_mut();
            platform_io.set_renderer_create_window_raw(Some(renderer_unary_sentinel));
            platform_io.set_renderer_destroy_window_raw(Some(renderer_unary_sentinel));
            platform_io.set_renderer_set_window_size_raw(Some(renderer_size_sentinel));
            platform_io.set_renderer_render_window_raw(Some(renderer_slot_sentinel));
            platform_io.set_renderer_swap_buffers_raw(Some(renderer_slot_sentinel));
        }

        disable(&mut ctx);

        let platform_io = ctx.platform_io_mut();
        assert!(unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            renderer_unary_sentinel
        ));
        assert!(unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            renderer_unary_sentinel
        ));
        assert!(
            platform_io.renderer_set_window_size_matches_pointer_callback(renderer_size_sentinel)
        );
        assert!(render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_slot_sentinel
        ));
        assert!(render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_slot_sentinel
        ));
        platform_io.clear_renderer_handlers();
    }

    #[test]
    fn same_backend_rebind_updates_renderer_state() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let raw = ctx.as_raw();
        let mut renderer_before_move = WgpuRenderer::empty();
        let mut renderer_after_move = WgpuRenderer::empty();

        enable(&mut renderer_before_move, &mut ctx).expect("empty renderer callback table");
        enable(&mut renderer_after_move, &mut ctx).expect("same backend owns callback table");

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw);
            let borrowed = borrow_renderer().expect("renderer for rebound context");
            assert_eq!(
                (&*borrowed.renderer) as *const _,
                &renderer_after_move as *const _
            );
        }

        disable(&mut ctx);
    }

    #[test]
    fn renderer_state_is_context_local() {
        let _guard = lock_context();
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let mut renderer_a = WgpuRenderer::empty();
        enable(&mut renderer_a, &mut ctx_a).expect("empty renderer callback table");

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let mut ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let mut renderer_b = WgpuRenderer::empty();
        enable(&mut renderer_b, &mut ctx_b).expect("empty renderer callback table");

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
            {
                let borrowed = borrow_renderer().expect("renderer for context A");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_a as *const _);
            }

            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
            {
                let borrowed = borrow_renderer().expect("renderer for context B");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_b as *const _);
            }
        }

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
        }
        disable(&mut ctx_b);
        drop(ctx_b);
        drop(renderer_b);

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        }
        disable(&mut ctx_a);
        drop(ctx_a);
        drop(renderer_a);
    }

    #[test]
    fn renderer_shutdown_clears_renderer_state() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let raw = ctx.as_raw();
        let mut renderer = WgpuRenderer::empty();

        enable(&mut renderer, &mut ctx).expect("empty renderer callback table");

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw);
            assert!(borrow_renderer().is_some());
        }

        renderer.shutdown();

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw);
            assert!(borrow_renderer().is_none());
        }

        disable(&mut ctx);
        drop(ctx);
        drop(renderer);
    }

    #[test]
    fn renderer_destroy_window_ignores_foreign_renderer_user_data() {
        let _guard = lock_context();
        let mut viewport = dear_imgui_rs::sys::ImGuiViewport::default();
        let foreign = 0x1234usize as *mut c_void;
        viewport.RendererUserData = foreign;

        unsafe {
            renderer_destroy_window((&mut viewport as *mut _) as *mut Viewport);
        }

        assert_eq!(viewport.RendererUserData, foreign);
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() } as usize;
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
        eprintln!("[wgpu-mv-sdl3] renderer already mutably borrowed; skipping callback");
        return None;
    }

    entry.borrowed = true;
    Some(RendererBorrowGuard {
        ctx,
        renderer: entry.renderer as *mut WgpuRenderer,
    })
}

unsafe fn viewport_user_data_mut<'a>(vpm: &'a mut Viewport) -> Option<&'a mut ViewportWgpuData> {
    let data = vpm.renderer_user_data();
    let data = data as *mut ViewportWgpuData;
    if !is_wgpu_viewport_data(data) {
        None
    } else {
        Some(unsafe { &mut *data })
    }
}

struct RendererBorrowGuard {
    ctx: usize,
    renderer: *mut WgpuRenderer,
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
    type Target = WgpuRenderer;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.renderer }
    }
}

impl DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.renderer }
    }
}

fn sdl_window_id_from_viewport(vp: &Viewport) -> Option<sdl3_sys::video::SDL_WindowID> {
    let handle = vp.platform_handle();
    if handle.is_null() {
        None
    } else {
        Some(sdl3_sys::video::SDL_WindowID(handle as usize as u32))
    }
}

fn clamp_i32_pixels_to_u32(pixels: i32) -> u32 {
    if pixels > 0 { pixels as u32 } else { 1 }
}

/// Renderer: create per-viewport resources (surface + config)
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv-sdl3] Renderer_CreateWindow");

        let Some(global) = global_handles() else {
            return;
        };

        let vpm = &mut *vp;
        let window_id = match sdl_window_id_from_viewport(vpm) {
            Some(id) => id,
            None => return,
        };

        let target = match Sdl3SurfaceTarget::from_window_id(window_id) {
            Some(t) => t,
            None => return,
        };

        let instance = match &global.instance {
            Some(i) => i.clone(),
            None => return,
        };

        let surface: wgpu::Surface<'static> = {
            // SAFETY: the underlying SDL window and display remain valid for the lifetime of the
            // corresponding ImGui viewport, and Dear ImGui calls `Renderer_DestroyWindow` before
            // the platform destroys the window. Therefore the raw handles remain valid until this
            // surface is dropped.
            let surface_target =
                match wgpu::SurfaceTargetUnsafe::from_display_and_window(&target, &target) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("[wgpu-mv-sdl3] create_surface handle error: {:?}", e);
                        return;
                    }
                };
            match instance.create_surface_unsafe(surface_target) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[wgpu-mv-sdl3] create_surface error: {:?}", e);
                    return;
                }
            }
        };

        // Query initial pixel size from SDL3.
        let mut w: i32 = 0;
        let mut h: i32 = 0;
        let raw_window = target.raw_window();
        let ok = sdl3_sys::video::SDL_GetWindowSizeInPixels(raw_window, &mut w, &mut h);
        if !ok {
            return;
        }
        let width = clamp_i32_pixels_to_u32(w);
        let height = clamp_i32_pixels_to_u32(h);

        let config = if let Some(adapter) = &global.adapter {
            let caps = surface.get_capabilities(adapter);
            let format = if caps.formats.contains(&global.render_target_format) {
                global.render_target_format
            } else {
                eprintln!(
                    "[wgpu-mv-sdl3] Surface doesn't support pipeline format {:?}; supported: {:?}. Skipping configure.",
                    global.render_target_format, caps.formats
                );
                return;
            };
            let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
                caps.present_modes
                    .get(0)
                    .cloned()
                    .unwrap_or(wgpu::PresentMode::Fifo)
            };
            let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
                wgpu::CompositeAlphaMode::Auto
            } else {
                caps.alpha_modes
                    .get(0)
                    .cloned()
                    .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
            };
            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                #[cfg(feature = "wgpu-30")]
                color_space: wgpu::SurfaceColorSpace::Auto,
                width,
                height,
                present_mode,
                alpha_mode,
                view_formats: vec![format],
                desired_maximum_frame_latency: 1,
            }
        } else {
            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: global.render_target_format,
                #[cfg(feature = "wgpu-30")]
                color_space: wgpu::SurfaceColorSpace::Auto,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![global.render_target_format],
                desired_maximum_frame_latency: 1,
            }
        };

        surface.configure(&global.device, &config);

        let data = ViewportWgpuData {
            device: global.device.clone(),
            #[cfg(feature = "wgpu-30")]
            queue: global.queue.clone(),
            surface,
            config,
            pending_frame: None,
            pending_reconfigure: false,
        };
        let ptr = Box::into_raw(Box::new(data));
        register_viewport_data(ptr);
        vpm.set_renderer_user_data(ptr as *mut c_void);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_CreateWindow");
        std::process::abort();
    }
}

/// Renderer: destroy per-viewport resources
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv-sdl3] Renderer_DestroyWindow");
        let vpm = &mut *vp;
        let data = vpm.renderer_user_data() as *mut ViewportWgpuData;
        if is_wgpu_viewport_data(data) {
            unregister_viewport_data(data);
            let _boxed: Box<ViewportWgpuData> = Box::from_raw(data);
            vpm.set_renderer_user_data(std::ptr::null_mut());
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_DestroyWindow");
        std::process::abort();
    }
}

/// Renderer: notify new size
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!(
            "[wgpu-mv-sdl3] Renderer_SetWindowSize to ({}, {})",
            size.x,
            size.y
        );
        let Some(global) = global_handles() else {
            return;
        };
        let vpm = &mut *vp;
        let scale = vpm.framebuffer_scale();
        if let Some(data) = viewport_user_data_mut(vpm) {
            let sx = if scale[0].is_finite() && scale[0] > 0.0 {
                scale[0]
            } else {
                1.0
            };
            let sy = if scale[1].is_finite() && scale[1] > 0.0 {
                scale[1]
            } else {
                1.0
            };
            let new_w = (size.x * sx).max(1.0).round() as u32;
            let new_h = (size.y * sy).max(1.0).round() as u32;
            if data.config.width != new_w || data.config.height != new_h {
                data.config.width = new_w;
                data.config.height = new_h;
                data.surface.configure(&global.device, &data.config);
            }
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_SetWindowSize");
        std::process::abort();
    }
}

/// Renderer: render viewport draw data into its surface
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv-sdl3] Renderer_RenderWindow");

        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let (device, queue) = match renderer.backend_data.as_ref() {
            Some(b) => (b.device.clone(), b.queue.clone()),
            None => return,
        };

        let vpm = &mut *vp;
        let window_id = sdl_window_id_from_viewport(vpm);
        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            return;
        }
        // SAFETY: Dear ImGui guarantees `raw_dd` is valid during render callbacks.
        let draw_data: &mut dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw_mut(&mut *raw_dd);

        if let Some(data) = viewport_user_data_mut(vpm) {
            let fb_w = data.config.width;
            let fb_h = data.config.height;
            #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
            let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    // Reconfigure from actual SDL3 pixel size and retry next frame.
                    if let Some(window_id) = window_id {
                        if let Some(target) = Sdl3SurfaceTarget::from_window_id(window_id) {
                            let raw_window = target.raw_window();
                            let mut w: i32 = 0;
                            let mut h: i32 = 0;
                            if sdl3_sys::video::SDL_GetWindowSizeInPixels(
                                raw_window, &mut w, &mut h,
                            ) {
                                let w = clamp_i32_pixels_to_u32(w);
                                let h = clamp_i32_pixels_to_u32(h);
                                data.config.width = w;
                                data.config.height = h;
                                data.surface.configure(&device, &data.config);
                            }
                        }
                    }
                    return;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return;
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    eprintln!("[wgpu-mv-sdl3] get_current_texture failed with a validation error");
                    return;
                }
            };
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
            let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
                Ok(frame) => (frame, false),
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    // Reconfigure from actual SDL3 pixel size and retry next frame.
                    if let Some(window_id) = window_id {
                        if let Some(target) = Sdl3SurfaceTarget::from_window_id(window_id) {
                            let raw_window = target.raw_window();
                            let mut w: i32 = 0;
                            let mut h: i32 = 0;
                            if sdl3_sys::video::SDL_GetWindowSizeInPixels(
                                raw_window, &mut w, &mut h,
                            ) {
                                let w = clamp_i32_pixels_to_u32(w);
                                let h = clamp_i32_pixels_to_u32(h);
                                data.config.width = w;
                                data.config.height = h;
                                data.surface.configure(&data.device, &data.config);
                            }
                        }
                    }
                    return;
                }
                Err(wgpu::SurfaceError::Timeout) => return,
                Err(e) => {
                    eprintln!("[wgpu-mv-sdl3] get_current_texture error: {:?}", e);
                    return;
                }
            };

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let render_block = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("dear-imgui-wgpu::viewport-encoder"),
                });
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("dear-imgui-wgpu::viewport-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(renderer.viewport_clear_color()),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                        multiview_mask: None,
                        timestamp_writes: None,
                    });

                    if let Err(e) = renderer.render_draw_data_with_fb_size_ex(
                        draw_data,
                        &mut render_pass,
                        fb_w,
                        fb_h,
                        false,
                        dear_imgui_rs::sys::igGetPlatformIO_Nil(),
                    ) {
                        eprintln!("[wgpu-mv-sdl3] render_draw_data(with_fb) error: {:?}", e);
                    }
                }
                queue.submit(std::iter::once(encoder.finish()));
            }));

            if render_block.is_err() {
                eprintln!(
                    "[wgpu-mv-sdl3] panic during viewport render block; skipping present for this viewport"
                );
                return;
            }

            data.pending_frame = Some(frame);
            data.pending_reconfigure = reconfigure_after_present;
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_RenderWindow");
        std::process::abort();
    }
}

/// Renderer: present frame for viewport surface
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv-sdl3] Renderer_SwapBuffers");
        let vpm = &mut *vp;
        if let Some(data) = viewport_user_data_mut(vpm) {
            if let Some(frame) = data.pending_frame.take() {
                #[cfg(feature = "wgpu-30")]
                data.queue.present(frame);
                #[cfg(not(feature = "wgpu-30"))]
                frame.present();
                if data.pending_reconfigure {
                    data.surface.configure(&data.device, &data.config);
                    data.pending_reconfigure = false;
                }
            }
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}

// Raw renderer callback adapters avoid the typed trampoline registry and give this backend stable
// callback identities for ownership checks.
//
// # Safety
//
// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
pub unsafe extern "C" fn renderer_create_window_sys(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    unsafe { renderer_create_window(vp.cast()) };
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
pub unsafe extern "C" fn renderer_destroy_window_sys(vp: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if vp.is_null() {
        return;
    }
    unsafe { renderer_destroy_window(vp.cast()) };
}

/// # Safety
///
/// Called by the repository-owned C++ aggregate ABI hook with a valid viewport and vector pointer.
pub unsafe extern "C" fn renderer_set_window_size_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() || size.is_null() {
        return;
    }
    let size = unsafe { *size };
    unsafe { renderer_set_window_size(vp.cast(), size) };
}

/// Raw renderer callback adapter to avoid typed trampolines.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_render_window(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_RenderWindow");
        std::process::abort();
    }
}

/// Raw renderer callback adapter to avoid typed trampolines.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_swap_buffers(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}
