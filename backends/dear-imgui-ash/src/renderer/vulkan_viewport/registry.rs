use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::{ContextBinding, ContextId, ContextLifecycle};

use super::runtime::{AshViewportError, RuntimeControl};
use super::{SurfaceAdapter, ViewportAshData, khr_surface, sys, vk};

#[derive(Clone)]
pub(super) struct GlobalHandles {
    pub(super) entry: ash::Entry,
    pub(super) instance: ash::Instance,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) present_queue: vk::Queue,
    pub(super) graphics_queue_family_index: u32,
    pub(super) present_queue_family_index: u32,
    pub(super) in_flight_frames: usize,
    pub(super) surface_adapter: Arc<dyn SurfaceAdapter>,
}

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

thread_local! {
    static RUNTIMES: RefCell<Vec<RegisteredRuntime>> = const { RefCell::new(Vec::new()) };
    static VIEWPORT_DATA: RefCell<Vec<ViewportDataState>> = const { RefCell::new(Vec::new()) };
    #[cfg(test)]
    static FAIL_NEXT_VIEWPORT_REGISTRATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn current_context() -> *mut sys::ImGuiContext {
    // SAFETY: this reads the process-global current Context pointer without dereferencing it.
    unsafe { sys::igGetCurrentContext() }
}

pub(super) fn preflight_runtime(context: ContextId) -> Result<(), AshViewportError> {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        if runtimes.iter().any(|entry| entry.context_id == context) {
            Err(AshViewportError::RuntimeAlreadyAttached)
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
            "Ash viewport runtime registered twice for one Context"
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
    context_raw: *mut sys::ImGuiContext,
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
    pointer: *mut ViewportAshData,
) -> Result<(), AshViewportError> {
    if pointer.is_null() {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "register RendererUserData",
        });
    }
    #[cfg(test)]
    if FAIL_NEXT_VIEWPORT_REGISTRATION.with(|failure| failure.replace(false)) {
        return Err(AshViewportError::InvalidCallbackArgument {
            callback: "injected RendererUserData registration",
        });
    }
    let context_raw = context.try_with_bound_context(current_context)?;
    if context_raw.is_null() {
        return Err(AshViewportError::BoundContextMismatch {
            expected: context.id(),
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

pub(super) fn unregister_viewport_data(pointer: *mut ViewportAshData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA.with(|data| {
        data.borrow_mut().retain(|state| state.pointer != pointer);
    });
}

fn owns_viewport_data(context: *mut sys::ImGuiContext, pointer: *mut ViewportAshData) -> bool {
    !context.is_null()
        && !pointer.is_null()
        && VIEWPORT_DATA.with(|data| {
            data.borrow().iter().any(|state| {
                state.context_raw == context as usize
                    && binding_has_native_context(&state.binding)
                    && state.pointer == pointer as usize
            })
        })
}

pub(super) unsafe fn viewport_user_data_mut(
    context: *mut sys::ImGuiContext,
    viewport: &mut Viewport,
) -> Option<&mut ViewportAshData> {
    let pointer = viewport.renderer_user_data().cast::<ViewportAshData>();
    owns_viewport_data(context, pointer).then(|| unsafe { &mut *pointer })
}

pub(super) unsafe fn take_viewport_data_from_viewport(
    context: *mut sys::ImGuiContext,
    viewport: &mut Viewport,
) -> Option<Box<ViewportAshData>> {
    let pointer = viewport.renderer_user_data().cast::<ViewportAshData>();
    if !owns_viewport_data(context, pointer) {
        return None;
    }
    unregister_viewport_data(pointer);
    viewport.set_renderer_user_data(std::ptr::null_mut());
    Some(unsafe { Box::from_raw(pointer) })
}

pub(super) fn take_viewport_data(context: ContextId) -> Vec<Box<ViewportAshData>> {
    VIEWPORT_DATA.with(|data| {
        let mut data = data.borrow_mut();
        let mut pointers = Vec::new();
        data.retain(|state| {
            if state.binding.id() == context {
                pointers.push(unsafe { Box::from_raw(state.pointer as *mut ViewportAshData) });
                false
            } else {
                true
            }
        });
        pointers
    })
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

#[cfg(test)]
pub(super) fn fail_next_viewport_registration() {
    FAIL_NEXT_VIEWPORT_REGISTRATION.with(|failure| failure.set(true));
}

/// Vulkan surface capability required by the viewport runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SurfaceSupportError {
    #[error("the validation Vulkan surface must be non-null")]
    NullSurface,
    #[error("querying present support failed: {0:?}")]
    PresentSupportQuery(vk::Result),
    #[error("queue family {queue_family_index} cannot present to the validation surface")]
    PresentUnsupported { queue_family_index: u32 },
    #[error("querying surface capabilities failed: {0:?}")]
    CapabilitiesQuery(vk::Result),
    #[error("the surface does not support COLOR_ATTACHMENT swapchain images")]
    ColorAttachmentUnsupported,
    #[error("querying surface formats failed: {0:?}")]
    FormatsQuery(vk::Result),
    #[error("the surface reports no formats")]
    NoFormats,
    #[error("querying surface present modes failed: {0:?}")]
    PresentModesQuery(vk::Result),
    #[error("the surface reports no present modes")]
    NoPresentModes,
}

pub(super) struct SurfaceSupport {
    pub(super) capabilities: vk::SurfaceCapabilitiesKHR,
    pub(super) formats: Vec<vk::SurfaceFormatKHR>,
    pub(super) present_modes: Vec<vk::PresentModeKHR>,
}

pub(super) fn validate_vulkan_handles(
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
) -> Result<(), AshViewportError> {
    if physical_device == vk::PhysicalDevice::null() {
        return Err(AshViewportError::NullPhysicalDevice);
    }
    if present_queue == vk::Queue::null() {
        return Err(AshViewportError::NullPresentQueue);
    }
    Ok(())
}

pub(super) fn validate_queue_family_selection(
    properties: &[vk::QueueFamilyProperties],
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
) -> Result<(), AshViewportError> {
    let Some(graphics) = properties.get(graphics_queue_family_index as usize) else {
        return Err(AshViewportError::GraphicsQueueFamilyOutOfRange {
            queue_family_index: graphics_queue_family_index,
            queue_family_count: properties.len(),
        });
    };
    let Some(present) = properties.get(present_queue_family_index as usize) else {
        return Err(AshViewportError::PresentQueueFamilyOutOfRange {
            queue_family_index: present_queue_family_index,
            queue_family_count: properties.len(),
        });
    };
    if graphics.queue_count == 0 {
        return Err(AshViewportError::QueueFamilyEmpty {
            queue_family_index: graphics_queue_family_index,
        });
    }
    if present.queue_count == 0 {
        return Err(AshViewportError::QueueFamilyEmpty {
            queue_family_index: present_queue_family_index,
        });
    }
    if !graphics.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        return Err(AshViewportError::GraphicsQueueFamilyUnsupported {
            queue_family_index: graphics_queue_family_index,
        });
    }
    Ok(())
}

pub(super) fn validate_vulkan_config(global: &GlobalHandles) -> Result<(), AshViewportError> {
    validate_vulkan_handles(global.physical_device, global.present_queue)?;
    let queue_families = unsafe {
        global
            .instance
            .get_physical_device_queue_family_properties(global.physical_device)
    };
    validate_queue_family_selection(
        &queue_families,
        global.graphics_queue_family_index,
        global.present_queue_family_index,
    )
}

pub(super) fn query_surface_support(
    global: &GlobalHandles,
    surface: vk::SurfaceKHR,
) -> Result<SurfaceSupport, SurfaceSupportError> {
    if surface == vk::SurfaceKHR::null() {
        return Err(SurfaceSupportError::NullSurface);
    }

    let loader = khr_surface::Instance::new(&global.entry, &global.instance);
    let present_supported = unsafe {
        loader.get_physical_device_surface_support(
            global.physical_device,
            global.present_queue_family_index,
            surface,
        )
    }
    .map_err(SurfaceSupportError::PresentSupportQuery)?;
    if !present_supported {
        return Err(SurfaceSupportError::PresentUnsupported {
            queue_family_index: global.present_queue_family_index,
        });
    }

    let capabilities =
        unsafe { loader.get_physical_device_surface_capabilities(global.physical_device, surface) }
            .map_err(SurfaceSupportError::CapabilitiesQuery)?;
    if !capabilities
        .supported_usage_flags
        .contains(vk::ImageUsageFlags::COLOR_ATTACHMENT)
    {
        return Err(SurfaceSupportError::ColorAttachmentUnsupported);
    }

    let formats =
        unsafe { loader.get_physical_device_surface_formats(global.physical_device, surface) }
            .map_err(SurfaceSupportError::FormatsQuery)?;
    if formats.is_empty() {
        return Err(SurfaceSupportError::NoFormats);
    }

    let present_modes = unsafe {
        loader.get_physical_device_surface_present_modes(global.physical_device, surface)
    }
    .map_err(SurfaceSupportError::PresentModesQuery)?;
    if present_modes.is_empty() {
        return Err(SurfaceSupportError::NoPresentModes);
    }

    Ok(SurfaceSupport {
        capabilities,
        formats,
        present_modes,
    })
}
