//! Context-local renderer and viewport-data registries.

use super::CallbackOwnershipError;
use super::surface::ViewportWgpuData;
use crate::renderer::WgpuRenderer;
use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::{Context, ContextBinding, ContextId};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

struct ViewportDataState {
    context_raw: usize,
    binding: ContextBinding,
    pointer: usize,
}

struct ContextRendererState {
    context: usize,
    binding: ContextBinding,
    renderer: usize,
    borrowed: bool,
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

thread_local! {
    static RENDERERS: RefCell<Vec<ContextRendererState>> = const { RefCell::new(Vec::new()) };
    static VIEWPORT_DATA: RefCell<Vec<ViewportDataState>> = const { RefCell::new(Vec::new()) };
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
    context: &Context,
    renderer: *mut WgpuRenderer,
    globals: Option<GlobalHandles>,
) {
    let context_raw = context.as_raw() as usize;
    let binding = context.binding();
    let renderer = renderer as usize;
    RENDERERS.with(|renderers| {
        let mut renderers = renderers.borrow_mut();
        renderers.retain(|state| state.binding.is_alive());
        debug_assert!(
            !renderers
                .iter()
                .any(|state| state.binding.id() == binding.id())
        );
        renderers.push(ContextRendererState {
            context: context_raw,
            binding,
            renderer,
            borrowed: false,
            globals,
        });
    });
}

pub(super) fn remove_renderer_state_for_context(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
) -> bool {
    let context = context as usize;
    let mut removed = false;
    RENDERERS.with(|renderers| {
        renderers.borrow_mut().retain(|state| {
            let keep = state.context != context;
            removed |= !keep;
            keep
        });
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
    RENDERERS.with(|renderers| {
        renderers
            .borrow_mut()
            .retain(|state| state.renderer != renderer);
    });
}

pub(super) fn has_renderer_state(context: *mut dear_imgui_rs::sys::ImGuiContext) -> bool {
    let context = context as usize;
    RENDERERS.with(|renderers| {
        renderers
            .borrow()
            .iter()
            .any(|state| state.context == context && state.binding.is_alive())
    })
}

pub(super) fn registered_renderer_for_context(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<*mut WgpuRenderer> {
    RENDERERS.with(|renderers| {
        renderers
            .borrow()
            .iter()
            .find(|state| state.context == context as usize && state.binding.is_alive())
            .map(|state| state.renderer as *mut WgpuRenderer)
    })
}

pub(super) fn validate_new_registration(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
) -> Result<(), CallbackOwnershipError> {
    let context = context as usize;
    let renderer = renderer as usize;
    let registration = RENDERERS.with(|renderers| {
        let mut renderers = renderers.borrow_mut();
        renderers.retain(|state| state.binding.is_alive());
        let Some(state) = renderers.iter().find(|state| state.context == context) else {
            // SAFETY: callers pass the live renderer they are attempting to register.
            return if renderers.iter().any(|state| state.renderer == renderer)
                || unsafe { &*(renderer as *const WgpuRenderer) }
                    .multi_viewport_active
                    .load(Ordering::Acquire)
            {
                Err(CallbackOwnershipError::RendererAlreadyRegistered)
            } else {
                Ok(())
            };
        };
        if state.borrowed {
            Err(CallbackOwnershipError::RendererCallbackActive)
        } else {
            Err(CallbackOwnershipError::AlreadyEnabled)
        }
    });

    if matches!(
        registration,
        Err(CallbackOwnershipError::RendererAlreadyRegistered)
            | Err(CallbackOwnershipError::RendererCallbackActive)
    ) {
        registration
    } else if has_viewport_data(context) {
        Err(CallbackOwnershipError::LiveViewportRendererRebind)
    } else {
        registration
    }
}

pub(super) fn globals_for_current_context() -> Option<GlobalHandles> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    RENDERERS.with(|renderers| {
        renderers
            .borrow()
            .iter()
            .find(|state| state.context == context && state.binding.is_alive())
            .and_then(|state| state.globals.clone())
    })
}

pub(super) fn binding_for_current_context() -> Option<ContextBinding> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }

    RENDERERS.with(|renderers| {
        renderers
            .borrow()
            .iter()
            .find(|state| state.context == context && state.binding.is_alive())
            .map(|state| state.binding.clone())
    })
}

pub(super) fn register_viewport_data(context: &ContextBinding, pointer: *mut ViewportWgpuData) {
    if pointer.is_null() {
        return;
    }
    let Ok(context_raw) = context.try_with_bound_context(current_context) else {
        return;
    };
    VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        data.retain(|state| state.binding.is_alive());
        if !data
            .iter()
            .any(|state| state.binding.id() == context.id() && state.pointer == pointer as usize)
        {
            data.push(ViewportDataState {
                context_raw: context_raw as usize,
                binding: context.clone(),
                pointer: pointer as usize,
            });
        }
    });
}

pub(super) fn unregister_viewport_data(pointer: *mut ViewportWgpuData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA.with(|data| {
        data.borrow_mut().retain(|state| state.pointer != pointer);
    });
}

fn has_viewport_data(context: usize) -> bool {
    VIEWPORT_DATA.with(|data| {
        data.borrow()
            .iter()
            .any(|state| state.context_raw == context && state.binding.is_alive())
    })
}

fn owns_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) -> bool {
    !pointer.is_null()
        && VIEWPORT_DATA.with(|data| {
            data.borrow().iter().any(|state| {
                state.context_raw == context as usize
                    && state.binding.is_alive()
                    && state.pointer == pointer as usize
            })
        })
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
    context: ContextId,
    pub(super) renderer: *mut WgpuRenderer,
}

pub(super) unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    RENDERERS.with(|renderers| {
        let mut renderers = renderers.borrow_mut();
        let state = renderers
            .iter_mut()
            .find(|state| state.context == context && state.binding.is_alive())?;
        if state.renderer == 0 || state.borrowed {
            return None;
        }
        state.borrowed = true;
        Some(RendererBorrowGuard {
            context: state.binding.id(),
            renderer: state.renderer as *mut WgpuRenderer,
        })
    })
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        RENDERERS.with(|renderers| {
            if let Some(state) = renderers.borrow_mut().iter_mut().find(|state| {
                state.binding.id() == self.context && state.renderer == self.renderer as usize
            }) {
                state.borrowed = false;
            }
        });
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
