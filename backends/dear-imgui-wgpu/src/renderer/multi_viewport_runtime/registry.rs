//! Context-local renderer and viewport-data registries.

use super::CallbackOwnershipError;
use super::surface::ViewportWgpuData;
use crate::renderer::WgpuRenderer;
use dear_imgui_rs::platform_io::Viewport;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::thread::ThreadId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ViewportDataState {
    context: usize,
    pointer: usize,
}

struct ContextRendererState {
    context: usize,
    renderer: usize,
    borrowed: bool,
    thread: ThreadId,
    globals: Option<GlobalHandles>,
}

#[derive(Clone)]
pub(super) struct GlobalHandles {
    pub(super) instance: wgpu::Instance,
    pub(super) adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    pub(super) queue: wgpu::Queue,
    pub(super) render_target_format: wgpu::TextureFormat,
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());
static VIEWPORT_DATA: Mutex<Vec<ViewportDataState>> = Mutex::new(Vec::new());

pub(super) struct CurrentContextGuard {
    previous: *mut dear_imgui_rs::sys::ImGuiContext,
    target: *mut dear_imgui_rs::sys::ImGuiContext,
}

impl CurrentContextGuard {
    /// # Safety
    ///
    /// `target` must be null or a live context for the current thread. Neither it nor the context
    /// that is current on entry may be destroyed before the returned guard is dropped.
    pub(super) unsafe fn bind(target: *mut dear_imgui_rs::sys::ImGuiContext) -> Self {
        // SAFETY: reading the current context does not dereference it; the caller owns context
        // lifetime and thread-affinity for the duration of the guard.
        let previous = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        if previous != target {
            // SAFETY: `target` satisfies the caller contract above and remains live until drop.
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            // SAFETY: `bind` requires the previously current context to outlive this guard.
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.previous) };
        }
    }
}

pub(super) fn current_context() -> *mut dear_imgui_rs::sys::ImGuiContext {
    // SAFETY: this reads the current-context pointer without dereferencing it.
    unsafe { dear_imgui_rs::sys::igGetCurrentContext() }
}

pub(super) fn renderer_globals(
    renderer: &WgpuRenderer,
) -> Result<GlobalHandles, CallbackOwnershipError> {
    #[cfg(target_arch = "wasm32")]
    return Err(CallbackOwnershipError::UnsupportedTarget);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let backend = renderer
            .backend_data
            .as_ref()
            .ok_or(CallbackOwnershipError::RendererNotInitialized)?;
        Ok(GlobalHandles {
            instance: backend
                .instance
                .clone()
                .ok_or(CallbackOwnershipError::MissingInstance)?,
            adapter: backend
                .adapter
                .clone()
                .ok_or(CallbackOwnershipError::MissingAdapter)?,
            device: backend.device.clone(),
            #[cfg(feature = "wgpu-30")]
            queue: backend.queue.clone(),
            render_target_format: backend.render_target_format,
        })
    }
}

pub(super) fn insert_renderer_state(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
    globals: Option<GlobalHandles>,
) {
    let context = context as usize;
    let renderer = renderer as usize;
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    debug_assert!(!renderers.iter().any(|state| state.context == context));
    renderers.push(ContextRendererState {
        context,
        renderer,
        borrowed: false,
        thread: std::thread::current().id(),
        globals,
    });
}

pub(super) fn remove_renderer_state_for_context(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
) -> bool {
    let context = context as usize;
    let mut removed = false;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| {
            let keep = state.context != context;
            removed |= !keep;
            keep
        });
    removed
}

pub(super) fn remove_renderer_state_for_renderer(renderer: *mut WgpuRenderer) {
    // SAFETY: this function is called from `WgpuRenderer::drop` while `renderer` still points to
    // the live renderer being destroyed.
    unsafe { &*renderer }
        .multi_viewport_active
        .store(false, Ordering::Release);
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| state.renderer != renderer);
}

pub(super) fn has_renderer_state(context: *mut dear_imgui_rs::sys::ImGuiContext) -> bool {
    let context = context as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|state| state.context == context)
}

pub(super) fn registered_renderer_for_context(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<*mut WgpuRenderer> {
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|state| state.context == context as usize)
        .map(|state| state.renderer as *mut WgpuRenderer)
}

pub(super) fn validate_new_registration(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
) -> Result<(), CallbackOwnershipError> {
    let context = context as usize;
    let renderer = renderer as usize;
    let renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(state) = renderers.iter().find(|state| state.context == context) else {
        // SAFETY: callers pass the live renderer they are attempting to register.
        if renderers.iter().any(|state| state.renderer == renderer)
            || unsafe { &*(renderer as *const WgpuRenderer) }
                .multi_viewport_active
                .load(Ordering::Acquire)
        {
            return Err(CallbackOwnershipError::RendererAlreadyRegistered);
        }
        drop(renderers);
        return if has_viewport_data(context) {
            Err(CallbackOwnershipError::LiveViewportRendererRebind)
        } else {
            Ok(())
        };
    };
    if state.borrowed {
        return Err(CallbackOwnershipError::RendererCallbackActive);
    }
    drop(renderers);
    if has_viewport_data(context) {
        Err(CallbackOwnershipError::LiveViewportRendererRebind)
    } else {
        Err(CallbackOwnershipError::AlreadyEnabled)
    }
}

pub(super) fn globals_for_current_context() -> Option<GlobalHandles> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|state| state.context == context && state.thread == std::thread::current().id())
        .and_then(|state| state.globals.clone())
}

pub(super) fn register_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) {
    if pointer.is_null() {
        return;
    }
    let state = ViewportDataState {
        context: context as usize,
        pointer: pointer as usize,
    };
    let mut data = VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !data.contains(&state) {
        data.push(state);
    }
}

pub(super) fn unregister_viewport_data(pointer: *mut ViewportWgpuData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| state.pointer != pointer);
}

fn has_viewport_data(context: usize) -> bool {
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|state| state.context == context)
}

fn owns_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) -> bool {
    let expected = ViewportDataState {
        context: context as usize,
        pointer: pointer as usize,
    };
    !pointer.is_null()
        && VIEWPORT_DATA
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .contains(&expected)
}

pub(super) unsafe fn viewport_data_pointer(viewport: &Viewport) -> Option<*mut ViewportWgpuData> {
    let context = current_context();
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    if owns_viewport_data(context, pointer) {
        Some(pointer)
    } else {
        None
    }
}

pub(super) unsafe fn destroy_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &mut Viewport,
) {
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    if !owns_viewport_data(context, pointer) {
        return;
    }
    unregister_viewport_data(pointer);
    viewport.set_renderer_user_data(std::ptr::null_mut());
    // SAFETY: registry ownership proves this pointer came from `Box::into_raw` exactly once.
    let data = unsafe { Box::from_raw(pointer) };
    debug_assert_eq!(data.owner_context, context as usize);
    drop(data);
}

pub(super) struct RendererBorrowGuard {
    context: usize,
    pub(super) renderer: *mut WgpuRenderer,
}

pub(super) unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let state = renderers
        .iter_mut()
        .find(|state| state.context == context && state.thread == std::thread::current().id())?;
    if state.renderer == 0 || state.borrowed {
        return None;
    }
    state.borrowed = true;
    Some(RendererBorrowGuard {
        context,
        renderer: state.renderer as *mut WgpuRenderer,
    })
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        let mut renderers = RENDERERS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(state) = renderers
            .iter_mut()
            .find(|state| state.context == self.context && state.renderer == self.renderer as usize)
        {
            state.borrowed = false;
        }
    }
}

impl Deref for RendererBorrowGuard {
    type Target = WgpuRenderer;

    fn deref(&self) -> &Self::Target {
        // SAFETY: registration keeps the renderer at a stable address and the guard owns the
        // context-local callback borrow until it is dropped.
        unsafe { &*self.renderer }
    }
}

impl DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the registry permits only one active mutable callback borrow per context.
        unsafe { &mut *self.renderer }
    }
}
