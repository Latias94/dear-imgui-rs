//! Context-local owning runtime and viewport-data registries.

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::{ContextBinding, ContextId, ContextLifecycle};

use super::runtime::{RuntimeControl, WgpuViewportError};
use super::surface::ViewportWgpuData;
use crate::renderer::WgpuRenderer;

struct RegisteredRuntime {
    context_raw: usize,
    context_id: ContextId,
    control: Weak<RuntimeControl>,
}

struct ViewportDataState {
    context_raw: usize,
    binding: ContextBinding,
    pointer: usize,
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
    static RUNTIMES: RefCell<Vec<RegisteredRuntime>> = const { RefCell::new(Vec::new()) };
    static VIEWPORT_DATA: RefCell<Vec<ViewportDataState>> = const { RefCell::new(Vec::new()) };
    #[cfg(test)]
    static FAIL_NEXT_VIEWPORT_REGISTRATION: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn current_context() -> *mut dear_imgui_rs::sys::ImGuiContext {
    // SAFETY: this reads the current-context pointer without dereferencing it.
    unsafe { dear_imgui_rs::sys::igGetCurrentContext() }
}

pub(super) fn renderer_globals(
    renderer: &WgpuRenderer,
) -> Result<GlobalHandles, WgpuViewportError> {
    #[cfg(target_arch = "wasm32")]
    return Err(WgpuViewportError::UnsupportedTarget);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let backend = renderer
            .backend_data
            .as_ref()
            .ok_or(WgpuViewportError::RendererNotInitialized)?;
        Ok(GlobalHandles {
            instance: backend
                .instance
                .clone()
                .ok_or(WgpuViewportError::MissingInstance)?,
            adapter: backend
                .adapter
                .clone()
                .ok_or(WgpuViewportError::MissingAdapter)?,
            device: backend.device.clone(),
            #[cfg(feature = "wgpu-30")]
            queue: backend.queue.clone(),
            render_target_format: backend.render_target_format,
        })
    }
}

pub(super) fn preflight_runtime(context: ContextId) -> Result<(), WgpuViewportError> {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        if runtimes.iter().any(|entry| entry.context_id == context) {
            Err(WgpuViewportError::RuntimeAlreadyAttached)
        } else {
            Ok(())
        }
    })
}

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        debug_assert!(
            !runtimes
                .iter()
                .any(|entry| entry.context_id == control.binding().id()),
            "WGPU viewport runtime registered twice for one Context"
        );
        runtimes.push(RegisteredRuntime {
            context_raw: control.context_raw() as usize,
            context_id: control.binding().id(),
            control: Rc::downgrade(control),
        });
    });
}

pub(super) fn unregister_runtime(context: ContextId) {
    RUNTIMES.with(|runtimes| {
        runtimes
            .borrow_mut()
            .retain(|entry| entry.context_id != context);
    });
}

pub(super) fn runtime_for_context(
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<Rc<RuntimeControl>> {
    if context_raw.is_null() {
        return None;
    }
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        runtimes
            .iter()
            .find(|entry| entry.context_raw == context_raw as usize)
            .and_then(|entry| entry.control.upgrade())
    })
}

pub(super) fn with_current_runtime<R>(
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> Option<R> {
    let control = runtime_for_context(current_context())?;
    if !control.is_callback_accessible() {
        return None;
    }
    match control.binding().lifecycle() {
        ContextLifecycle::Alive => control
            .binding()
            .try_with_bound_context(|| callback(&control))
            .ok(),
        ContextLifecycle::Dropping | ContextLifecycle::NativeDestroyed => None,
        _ => None,
    }
}

fn binding_has_native_context(binding: &ContextBinding) -> bool {
    matches!(
        binding.lifecycle(),
        ContextLifecycle::Alive | ContextLifecycle::Dropping
    )
}

pub(super) fn register_viewport_data(
    context: &ContextBinding,
    pointer: *mut ViewportWgpuData,
) -> Result<(), WgpuViewportError> {
    if pointer.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "register null viewport data",
        });
    }
    #[cfg(test)]
    if FAIL_NEXT_VIEWPORT_REGISTRATION.with(|failure| failure.replace(false)) {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "injected viewport registration failure",
        });
    }
    let context_raw = context
        .try_with_bound_context(current_context)
        .map_err(WgpuViewportError::Context)?;
    if context_raw.is_null() {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "register viewport data without a bound Context",
        });
    }
    VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        data.retain(|state| binding_has_native_context(&state.binding));
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
    Ok(())
}

pub(super) fn unregister_viewport_data(pointer: *mut ViewportWgpuData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA.with(|data| {
        data.borrow_mut().retain(|state| state.pointer != pointer);
    });
}

fn owns_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) -> bool {
    !pointer.is_null()
        && VIEWPORT_DATA.with(|data| {
            data.borrow().iter().any(|state| {
                state.context_raw == context as usize
                    && binding_has_native_context(&state.binding)
                    && state.pointer == pointer as usize
            })
        })
}

pub(super) unsafe fn viewport_data_pointer(viewport: &Viewport) -> Option<*mut ViewportWgpuData> {
    let context = current_context();
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    owns_viewport_data(context, pointer).then_some(pointer)
}

pub(super) unsafe fn destroy_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &mut Viewport,
) -> bool {
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    if !owns_viewport_data(context, pointer) {
        return false;
    }
    unregister_viewport_data(pointer);
    // SAFETY: registry ownership proves that the renderer data belongs to this backend and is
    // cleared before the allocation is reclaimed below.
    unsafe { viewport.set_renderer_user_data(std::ptr::null_mut()) };
    // SAFETY: registry ownership proves this pointer came from `Box::into_raw` exactly once.
    let data = unsafe { Box::from_raw(pointer) };
    debug_assert_eq!(data.owner_context, context as usize);
    drop(data);
    true
}

pub(super) fn take_viewport_data(context: ContextId) -> Vec<*mut ViewportWgpuData> {
    VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        let mut owned = Vec::new();
        data.retain(|state| {
            if state.binding.id() == context {
                owned.push(state.pointer as *mut ViewportWgpuData);
                false
            } else {
                true
            }
        });
        owned
    })
}

#[cfg(test)]
pub(super) fn fail_next_viewport_registration() {
    FAIL_NEXT_VIEWPORT_REGISTRATION.with(|failure| failure.set(true));
}

#[cfg(test)]
pub(super) fn viewport_data_count(context: ContextId) -> usize {
    VIEWPORT_DATA.with(|data| {
        data.borrow()
            .iter()
            .filter(|state| state.binding.id() == context)
            .count()
    })
}
