//! Context-local owning runtime and viewport-data registries.

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::{ContextBinding, ContextId, ContextLifecycle};

use super::runtime::{RuntimeControl, WgpuViewportError};
use super::surface::ViewportWgpuData;
use crate::WgpuViewportSurfaceConfig;
use crate::renderer::WgpuRenderer;

struct RegisteredRuntime {
    context_raw: usize,
    context_id: ContextId,
    control: Weak<RuntimeControl>,
}

struct ViewportDataState {
    context_raw: usize,
    binding: ContextBinding,
    viewport: ViewportIdentity,
    pointer: usize,
    drop_allocation: unsafe fn(usize),
    is_wgpu_data: bool,
}

impl ViewportDataState {
    unsafe fn drop_allocation(self) {
        unsafe { (self.drop_allocation)(self.pointer) };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewportIdentity {
    address: usize,
    id: u32,
}

impl ViewportIdentity {
    pub(super) fn capture(viewport: &Viewport) -> Self {
        Self {
            address: viewport.as_raw() as usize,
            id: viewport.id().raw(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportDataLookup {
    Absent,
    Owned(*mut ViewportWgpuData),
    OwnershipLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportDataDestroy {
    Absent,
    Destroyed,
    OwnershipLost,
}

#[derive(Clone)]
pub(super) struct GlobalHandles {
    pub(super) instance: wgpu::Instance,
    pub(super) adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    pub(super) queue: wgpu::Queue,
    pub(super) render_target_format: wgpu::TextureFormat,
    pub(super) viewport_surface_config: WgpuViewportSurfaceConfig,
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
            viewport_surface_config: backend.init_info.viewport_surface_config,
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

unsafe fn drop_boxed_allocation<T>(pointer: usize) {
    drop(unsafe { Box::from_raw(pointer as *mut T) });
}

fn register_viewport_allocation<T>(
    context: &ContextBinding,
    viewport: ViewportIdentity,
    pointer: *mut T,
    is_wgpu_data: bool,
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
        if data.iter().any(|state| {
            state.binding.id() == context.id()
                && (state.viewport == viewport || state.pointer == pointer as usize)
        }) {
            return Err(WgpuViewportError::RendererUserDataOccupied);
        }
        data.push(ViewportDataState {
            context_raw: context_raw as usize,
            binding: context.clone(),
            viewport,
            pointer: pointer as usize,
            drop_allocation: drop_boxed_allocation::<T>,
            is_wgpu_data,
        });
        Ok(())
    })
}

pub(super) fn register_viewport_data(
    context: &ContextBinding,
    viewport: ViewportIdentity,
    pointer: *mut ViewportWgpuData,
) -> Result<(), WgpuViewportError> {
    register_viewport_allocation(context, viewport, pointer, true)
}

#[cfg(test)]
pub(super) fn register_test_viewport_data<T>(
    context: &ContextBinding,
    viewport: ViewportIdentity,
    pointer: *mut T,
) -> Result<(), WgpuViewportError> {
    register_viewport_allocation(context, viewport, pointer, false)
}

#[cfg(test)]
pub(super) fn unregister_viewport_data(pointer: *mut ViewportWgpuData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA.with(|data| {
        data.borrow_mut().retain(|state| state.pointer != pointer);
    });
}

pub(super) fn viewport_data_lookup(viewport: &Viewport) -> ViewportDataLookup {
    let context = current_context();
    let identity = ViewportIdentity::capture(viewport);
    let slot = viewport.renderer_user_data() as usize;
    VIEWPORT_DATA.with(|data| {
        let data = data.borrow();
        let Some(state) = data.iter().find(|state| {
            state.context_raw == context as usize
                && binding_has_native_context(&state.binding)
                && state.viewport == identity
        }) else {
            return if slot == 0 {
                ViewportDataLookup::Absent
            } else {
                ViewportDataLookup::OwnershipLost
            };
        };
        if state.pointer == slot && state.is_wgpu_data {
            ViewportDataLookup::Owned(state.pointer as *mut ViewportWgpuData)
        } else {
            ViewportDataLookup::OwnershipLost
        }
    })
}

/// Verifies that every renderer-owned slot reachable from the bound Context still points at the
/// exact allocation recorded by this runtime.
///
/// This is intentionally read-only. Teardown must reject a partial foreign takeover before it
/// drops any sidecar or releases `Renderer_DestroyWindow`; otherwise Dear ImGui can later reach a
/// foreign `RendererUserData` value through an empty destroy callback slot.
pub(super) fn preflight_viewport_data_ownership(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: &ContextBinding,
    reachable_viewports: &[(ViewportIdentity, usize)],
) -> Result<(), WgpuViewportError> {
    VIEWPORT_DATA.with(|data| {
        let data = data.borrow();
        for (viewport, slot) in reachable_viewports {
            let state = data.iter().find(|state| {
                state.context_raw == context as usize
                    && state.binding.id() == binding.id()
                    && state.viewport == *viewport
            });
            match state {
                Some(state) => {
                    if state.pointer != *slot {
                        return Err(WgpuViewportError::RendererUserDataOwnershipLost {
                            callback: "Renderer_DestroyWindow",
                        });
                    }
                }
                None if *slot != 0 => {
                    return Err(WgpuViewportError::RendererUserDataOwnershipLost {
                        callback: "Renderer_DestroyWindow",
                    });
                }
                None => {}
            }
        }
        Ok(())
    })
}

pub(super) unsafe fn destroy_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &mut Viewport,
) -> ViewportDataDestroy {
    let identity = ViewportIdentity::capture(viewport);
    let state = VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        data.iter()
            .position(|state| state.context_raw == context as usize && state.viewport == identity)
            .map(|position| data.remove(position))
    });
    let Some(state) = state else {
        return if viewport.renderer_user_data().is_null() {
            ViewportDataDestroy::Absent
        } else {
            ViewportDataDestroy::OwnershipLost
        };
    };

    let slot_is_owned = viewport.renderer_user_data() as usize == state.pointer;
    if slot_is_owned {
        // SAFETY: the registry proves that this exact slot is ours; clear it before drop.
        unsafe { viewport.set_renderer_user_data(std::ptr::null_mut()) };
    }
    // SAFETY: registry removal transfers the sole allocation ownership into this function.
    unsafe { state.drop_allocation() };
    if slot_is_owned {
        ViewportDataDestroy::Destroyed
    } else {
        ViewportDataDestroy::OwnershipLost
    }
}

pub(super) fn drop_orphaned_viewport_data(context: ContextId) {
    VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        let mut owned = Vec::new();
        let mut index = 0;
        while index < data.len() {
            if data[index].binding.id() == context {
                owned.push(data.remove(index));
            } else {
                index += 1;
            }
        }
        drop(data);
        for state in owned {
            // SAFETY: removing each registry entry transfers its sole allocation ownership.
            unsafe { state.drop_allocation() };
        }
    });
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
