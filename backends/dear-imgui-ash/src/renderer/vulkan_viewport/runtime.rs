use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextBinding, ContextBindingError, ContextDestroyed, ContextId,
    ContextLifecycle, ContextTeardown, TextureData, TextureId,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, destroy_renderer_viewport_resources, detect_callback_drift,
    preflight_callbacks, release_callbacks,
};
use super::registry::{
    GlobalHandles, preflight_runtime, query_surface_support, register_runtime, take_viewport_data,
    unregister_runtime, validate_vulkan_config,
};
use super::{SurfaceAdapter, SurfaceCreateError, SurfaceSupportError, VulkanViewportConfig};
use crate::renderer::lifecycle::{DeviceIdleOutcome, classify_device_idle};
use crate::{AshRenderer, Options, RendererError, TextureRetirementBatch, TextureUpdateResult};

struct AshRendererAttachmentMarker;

/// Failure to attach or operate an owning Ash multi-viewport runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AshViewportError {
    /// The Dear ImGui Context rejected the renderer attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The originating Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// The underlying Ash renderer operation failed.
    #[error(transparent)]
    Renderer(#[from] RendererError),
    /// The runtime and supplied Context identities differ.
    #[error("Ash viewport runtime belongs to Context {expected:?}, not {actual:?}")]
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// A callback entry was not running under this runtime's Context.
    #[error("the current Dear ImGui Context does not match Ash runtime Context {expected:?}")]
    BoundContextMismatch { expected: ContextId },
    /// Renderer callbacks require an attached platform runtime.
    #[error("Ash multi-viewport requires an attached multi-viewport platform runtime")]
    PlatformBackendUnavailable,
    /// The selected surface adapter does not match the attached platform runtime.
    #[error("Ash viewport adapter requires `{expected}`, but Context reports `{actual}`")]
    PlatformBackendMismatch {
        expected: &'static str,
        actual: String,
    },
    /// A required platform callback is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// SDL3 did not install its Vulkan surface callback.
    #[error("Platform_CreateVkSurface is not set by the SDL3 platform backend")]
    PlatformCreateVkSurfaceUnavailable,
    /// Another renderer owns one renderer callback slot.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
    /// A callback claimed by this runtime was replaced while attached.
    #[error("Ash renderer callback `{callback}` was replaced while the runtime was attached")]
    RendererCallbackReplaced { callback: &'static str },
    /// A secondary viewport already contains renderer-owned user data.
    #[error("a secondary viewport already has RendererUserData owned by another backend")]
    RendererUserDataOccupied,
    /// A callback observed foreign or unregistered renderer user data.
    #[error("Ash callback `{callback}` observed foreign or unregistered RendererUserData")]
    RendererUserDataOwnershipLost { callback: &'static str },
    /// Existing platform windows would miss the renderer create callback.
    #[error("secondary platform windows already exist; destroy them before attaching Ash")]
    PlatformWindowsAlreadyCreated,
    /// The aggregate size callback cannot be bridged safely by this artifact.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// The registry already contains a live runtime for this Context.
    #[error("an Ash viewport runtime is already attached to this Context")]
    RuntimeAlreadyAttached,
    /// The runtime has shut down or Context-owned teardown has started.
    #[error("the Ash viewport runtime is no longer attached")]
    RuntimeDetached,
    /// The renderer is already mutably borrowed by another runtime entry.
    #[error("Ash renderer runtime is already active in `{callback}`")]
    CallbackReentered { callback: &'static str },
    /// A callback panic was contained at the C ABI boundary.
    #[error("Ash renderer callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// Dear ImGui passed an invalid viewport to a renderer callback.
    #[error("Ash renderer callback `{callback}` received an invalid argument")]
    InvalidCallbackArgument { callback: &'static str },
    /// Creating a secondary Vulkan surface failed.
    #[error(transparent)]
    SurfaceCreate(#[from] SurfaceCreateError),
    /// The validation surface cannot support the configured runtime.
    #[error(transparent)]
    SurfaceUnsupported(#[from] SurfaceSupportError),
    /// The physical device handle is null.
    #[error("VulkanViewportConfig::physical_device must be non-null")]
    NullPhysicalDevice,
    /// The presentation queue handle is null.
    #[error("VulkanViewportConfig::present_queue must be non-null")]
    NullPresentQueue,
    /// The graphics queue family is outside the physical-device table.
    #[error(
        "graphics queue family {queue_family_index} is out of range for {queue_family_count} queue families"
    )]
    GraphicsQueueFamilyOutOfRange {
        queue_family_index: u32,
        queue_family_count: usize,
    },
    /// The presentation queue family is outside the physical-device table.
    #[error(
        "present queue family {queue_family_index} is out of range for {queue_family_count} queue families"
    )]
    PresentQueueFamilyOutOfRange {
        queue_family_index: u32,
        queue_family_count: usize,
    },
    /// The graphics queue family cannot execute graphics commands.
    #[error("queue family {queue_family_index} does not support GRAPHICS commands")]
    GraphicsQueueFamilyUnsupported { queue_family_index: u32 },
    /// A configured queue family exposes no queues.
    #[error("queue family {queue_family_index} exposes no queues")]
    QueueFamilyEmpty { queue_family_index: u32 },
    /// Waiting for Vulkan completion failed and resources were retained for retry.
    #[error("Vulkan completion wait `{operation}` failed: {source:?}")]
    DeviceCompletionFailed {
        operation: &'static str,
        source: ash::vk::Result,
    },
    /// The logical device entered the terminal lost state during cleanup.
    #[error("Vulkan device was lost during `{operation}`; resources were reclaimed terminally")]
    DeviceLost { operation: &'static str },
}

/// Transactional attachment failure that returns the unchanged renderer.
pub struct AshViewportAttachError {
    error: AshViewportError,
    renderer: Box<AshRenderer>,
}

impl AshViewportAttachError {
    pub(crate) fn new(error: AshViewportError, renderer: AshRenderer) -> Self {
        Self {
            error,
            renderer: Box::new(renderer),
        }
    }

    /// Returns the reason attachment failed.
    pub fn error(&self) -> &AshViewportError {
        &self.error
    }

    /// Returns the renderer so the caller can retry, render one viewport, or destroy it.
    pub fn into_renderer(self) -> AshRenderer {
        *self.renderer
    }

    /// Returns both the typed failure and the unchanged renderer.
    pub fn into_parts(self) -> (AshViewportError, AshRenderer) {
        (self.error, *self.renderer)
    }
}

impl fmt::Debug for AshViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AshViewportAttachError")
            .field("error", &self.error)
            .field("renderer", &"returned to caller")
            .finish()
    }
}

impl fmt::Display for AshViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for AshViewportAttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Constructing,
    Attached,
    ShuttingDown,
    Detached,
    ResourceDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallbackState {
    Unclaimed,
    Claimed,
    Released,
}

enum ShutdownAction<'a> {
    Quiesce,
    Explicit(&'a mut Context),
    BestEffort,
    ContextResources,
}

enum RendererStorage {
    Real(Box<AshRenderer>),
    #[cfg(test)]
    Fake(Box<u8>),
}

impl RendererStorage {
    fn real_mut(&mut self) -> Option<&mut AshRenderer> {
        match self {
            Self::Real(renderer) => Some(renderer),
            #[cfg(test)]
            Self::Fake(_) => None,
        }
    }

    #[cfg(test)]
    fn address(&self) -> *const () {
        match self {
            Self::Real(renderer) => std::ptr::from_ref(renderer.as_ref()).cast(),
            Self::Fake(renderer) => std::ptr::from_ref(renderer.as_ref()).cast(),
        }
    }
}

pub(super) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    renderer: RefCell<Option<RendererStorage>>,
    globals: RefCell<Option<GlobalHandles>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_state: Cell<CallbackState>,
    prior_backend_flags: BackendFlags,
    renderer_flags_added: BackendFlags,
    retained_viewports: RefCell<Vec<Box<super::ViewportAshData>>>,
    faults: RefCell<Option<AshViewportError>>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    callback_probe_count: Cell<usize>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("has_renderer", &self.renderer.borrow().is_some())
            .field("callback_state", &self.callback_state.get())
            .finish_non_exhaustive()
    }
}

impl RuntimeControl {
    fn new(context: &Context, renderer: AshRenderer, globals: GlobalHandles) -> Self {
        Self::new_with_storage(
            context,
            RendererStorage::Real(Box::new(renderer)),
            Some(globals),
        )
    }

    fn new_with_storage(
        context: &Context,
        renderer: RendererStorage,
        globals: Option<GlobalHandles>,
    ) -> Self {
        let renderer_flags_added = match &renderer {
            RendererStorage::Real(renderer) => renderer.renderer_flags_added,
            #[cfg(test)]
            RendererStorage::Fake(_) => BackendFlags::empty(),
        };
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(renderer)),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            prior_backend_flags: context.io().backend_flags(),
            renderer_flags_added,
            retained_viewports: RefCell::new(Vec::new()),
            faults: RefCell::new(None),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            callback_probe_count: Cell::new(0),
            #[cfg(test)]
            transitions: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(super) fn globals(&self) -> Option<GlobalHandles> {
        self.globals.borrow().clone()
    }

    pub(super) fn prior_backend_flags(&self) -> BackendFlags {
        self.prior_backend_flags
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(super) fn should_detect_callback_drift(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() == CallbackState::Claimed
    }

    pub(super) fn callback_released(&self) -> bool {
        self.callback_state.get() == CallbackState::Released
    }

    pub(super) fn mark_callback_claimed(&self) {
        self.callback_state.set(CallbackState::Claimed);
    }

    pub(super) fn mark_callback_released(&self) {
        self.callback_state.set(CallbackState::Released);
        unregister_runtime(self.binding.id());
    }

    fn set_state(&self, state: RuntimeState) {
        #[cfg(test)]
        let previous = self.state.replace(state);
        #[cfg(not(test))]
        self.state.set(state);
        #[cfg(test)]
        if previous != state {
            match state {
                RuntimeState::ShuttingDown => self.transitions.borrow_mut().push("ShuttingDown"),
                RuntimeState::Detached => self.transitions.borrow_mut().push("Detached"),
                RuntimeState::ResourceDropped => {
                    self.transitions.borrow_mut().push("ResourceDropped");
                }
                RuntimeState::Constructing | RuntimeState::Attached => {}
            }
        }
    }

    pub(super) fn begin_shutdown(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Constructing | RuntimeState::Attached
        ) {
            self.set_state(RuntimeState::ShuttingDown);
        }
    }

    fn mark_detached(&self) {
        if !matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.set_state(RuntimeState::Detached);
        }
        unregister_runtime(self.binding.id());
    }

    pub(super) fn record_fault(&self, fault: AshViewportError) {
        let mut faults = self.faults.borrow_mut();
        if faults.is_none() {
            *faults = Some(fault);
        }
    }

    pub(super) fn record_callback_replaced(&self, callback: &'static str) {
        self.record_fault(AshViewportError::RendererCallbackReplaced { callback });
        self.begin_shutdown();
    }

    fn detect_and_take_fault(&self) -> Option<AshViewportError> {
        detect_callback_drift(self);
        self.faults.borrow_mut().take()
    }

    fn ensure_context(&self, context: &Context) -> Result<(), AshViewportError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(AshViewportError::ContextMismatch {
                expected: self.binding.id(),
                actual: context.id(),
            })
        }
    }

    fn ensure_entry(&self) -> Result<(), AshViewportError> {
        if let Some(fault) = self.detect_and_take_fault() {
            return Err(fault);
        }
        if self.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(AshViewportError::RuntimeDetached)
        }
    }

    fn finish_entry(&self) -> Result<(), AshViewportError> {
        self.detect_and_take_fault().map_or(Ok(()), Err)
    }

    fn with_renderer_mut<R>(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut AshRenderer) -> Result<R, AshViewportError>,
    ) -> Result<R, AshViewportError> {
        self.ensure_entry()?;
        let result = {
            let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: callback_name,
                }
            })?;
            let renderer = renderer
                .as_mut()
                .and_then(RendererStorage::real_mut)
                .ok_or(AshViewportError::RuntimeDetached)?;
            callback(renderer)
        }?;
        self.finish_entry()?;
        Ok(result)
    }

    fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&AshRenderer) -> R,
    ) -> Result<R, AshViewportError> {
        self.ensure_entry()?;
        let result = {
            let renderer =
                self.renderer
                    .try_borrow()
                    .map_err(|_| AshViewportError::CallbackReentered {
                        callback: "Rust runtime entry",
                    })?;
            let renderer = match renderer.as_ref() {
                Some(RendererStorage::Real(renderer)) => renderer.as_ref(),
                #[cfg(test)]
                Some(RendererStorage::Fake(_)) | None => {
                    return Err(AshViewportError::RuntimeDetached);
                }
                #[cfg(not(test))]
                None => return Err(AshViewportError::RuntimeDetached),
            };
            callback(renderer)
        };
        self.finish_entry()?;
        Ok(result)
    }

    pub(super) fn with_renderer_callback<R>(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut AshRenderer, &GlobalHandles) -> Result<R, AshViewportError>,
    ) -> Result<R, AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: callback_name,
                })?;
        let renderer = renderer
            .as_mut()
            .and_then(RendererStorage::real_mut)
            .ok_or(AshViewportError::RuntimeDetached)?;
        let globals = self.globals().ok_or(AshViewportError::RuntimeDetached)?;
        callback(renderer, &globals)
    }

    pub(super) fn with_renderer_teardown<R>(
        &self,
        callback: impl FnOnce(&mut AshRenderer, &GlobalHandles) -> Result<R, AshViewportError>,
    ) -> Result<Option<R>, AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        let Some(storage) = storage.as_mut() else {
            return Ok(None);
        };
        #[cfg(test)]
        if matches!(storage, RendererStorage::Fake(_)) {
            return Ok(None);
        }
        let renderer = storage
            .real_mut()
            .ok_or(AshViewportError::RuntimeDetached)?;
        let globals = self.globals().ok_or(AshViewportError::RuntimeDetached)?;
        callback(renderer, &globals).map(Some)
    }

    pub(super) fn wait_device_idle(
        &self,
        renderer: &AshRenderer,
        operation: &'static str,
    ) -> Result<(), AshViewportError> {
        match classify_device_idle(unsafe { renderer.device.device_wait_idle() }) {
            Ok(DeviceIdleOutcome::Complete) => Ok(()),
            Ok(DeviceIdleOutcome::DeviceLost) => {
                self.record_fault(AshViewportError::DeviceLost { operation });
                Ok(())
            }
            Err(source) => Err(AshViewportError::DeviceCompletionFailed { operation, source }),
        }
    }

    fn release_renderer_explicit(&self, context: &mut Context) -> Result<(), AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        let Some(storage) = renderer.as_mut() else {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        };
        let shutdown_result = match storage {
            RendererStorage::Real(renderer) => renderer.shutdown(context),
            #[cfg(test)]
            RendererStorage::Fake(_) => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake(_) => true,
        };
        if !may_release {
            return shutdown_result.map_err(Into::into);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        map_renderer_shutdown_result(shutdown_result, "renderer shutdown")
    }

    fn release_renderer_without_context_reset(&self) -> Result<(), AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        let Some(storage) = renderer.as_mut() else {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        };
        let shutdown_result = match storage {
            RendererStorage::Real(renderer) => renderer.shutdown_without_context_reset(),
            #[cfg(test)]
            RendererStorage::Fake(_) => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake(_) => true,
        };
        if !may_release {
            return shutdown_result.map_err(Into::into);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        map_renderer_shutdown_result(shutdown_result, "renderer teardown")
    }

    fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), AshViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            if !matches!(action, ShutdownAction::ContextResources) {
                self.detach_attachment();
            }
            return Ok(());
        }

        self.begin_shutdown();
        if matches!(action, ShutdownAction::Quiesce) {
            return release_callbacks(self);
        }

        let viewport_error = match destroy_renderer_viewport_resources(self) {
            Ok(()) => None,
            Err(error) if matches!(action, ShutdownAction::Explicit(_)) => return Err(error),
            Err(error) => Some(error),
        };
        let callback_result = release_callbacks(self);

        match action {
            ShutdownAction::Quiesce => unreachable!(),
            ShutdownAction::Explicit(context) => {
                self.mark_detached();
                let renderer_result = self.release_renderer_explicit(context);
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                first_error([viewport_error, callback_result.err(), renderer_result.err()])
            }
            ShutdownAction::BestEffort => {
                if viewport_error.is_some() {
                    return first_error([viewport_error, callback_result.err()]);
                }
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.clear_bound_renderer_configuration();
                    self.detach_attachment();
                }
                first_error([callback_result.err(), renderer_result.err()])
            }
            ShutdownAction::ContextResources => {
                if viewport_error.is_some() {
                    return first_error([viewport_error, callback_result.err()]);
                }
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                first_error([callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn clear_bound_renderer_configuration(&self) {
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
        let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
        if io.is_null() || platform_io.is_null() {
            return;
        }
        let platform_io =
            unsafe { dear_imgui_rs::platform_io::PlatformIo::from_raw_mut(platform_io) };
        let renderer_name = unsafe {
            (!(*io).BackendRendererName.is_null())
                .then(|| std::ffi::CStr::from_ptr((*io).BackendRendererName))
        };
        let renderer_name_is_ours = AshRenderer::renderer_name_is_ours(renderer_name);
        let draw_callbacks_are_ours = AshRenderer::owned_draw_callbacks_match(platform_io);
        unsafe {
            if renderer_name_is_ours {
                (*io).BackendRendererName = std::ptr::null();
            }
            if renderer_name_is_ours && draw_callbacks_are_ours {
                (*io).BackendFlags &= !self.renderer_flags_added.bits();
            }
        }
        AshRenderer::clear_owned_draw_callbacks(platform_io);
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        match self.binding.lifecycle() {
            ContextLifecycle::Alive | ContextLifecycle::Dropping => {
                let _ = self.binding.try_with_bound_context(|| {
                    if let Err(error) = self.shutdown_once(ShutdownAction::BestEffort) {
                        self.record_fault(error);
                    }
                });
            }
            ContextLifecycle::NativeDestroyed => {
                if let Err(error) = self.retry_detached_cleanup() {
                    self.record_fault(error);
                }
            }
            _ => {}
        }
    }

    fn store_attachment(&self, attachment: ContextAttachmentLease) {
        self.attachment.borrow_mut().replace(attachment);
    }

    fn detach_attachment(&self) {
        if let Some(mut attachment) = self.attachment.borrow_mut().take() {
            attachment.detach();
        }
    }

    fn recover_renderer(&self) -> AshRenderer {
        self.globals.borrow_mut().take();
        match self
            .renderer
            .borrow_mut()
            .take()
            .expect("failed Ash runtime construction lost its renderer")
        {
            RendererStorage::Real(renderer) => *renderer,
            #[cfg(test)]
            RendererStorage::Fake(_) => unreachable!("test runtime does not recover AshRenderer"),
        }
    }

    fn mark_context_destroyed(&self) {
        unregister_runtime(self.binding.id());
        self.retained_viewports
            .borrow_mut()
            .extend(take_viewport_data(self.binding.id()));
        self.attachment.borrow_mut().take();
        if self.renderer.borrow().is_none() {
            self.set_state(RuntimeState::ResourceDropped);
        } else {
            self.mark_detached();
            if let Err(error) = self.retry_detached_cleanup() {
                self.record_fault(error);
            }
        }
    }

    fn retry_detached_cleanup(&self) -> Result<(), AshViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            return Ok(());
        }
        if !self.retained_viewports.borrow().is_empty() {
            self.with_renderer_teardown(|renderer, globals| {
                self.wait_device_idle(renderer, "retained viewport cleanup")?;
                let surface_loader =
                    super::khr_surface::Instance::new(&globals.entry, &globals.instance);
                let retained = std::mem::take(&mut *self.retained_viewports.borrow_mut());
                for data in retained {
                    data.destroy_after_device_idle(renderer, &surface_loader)?;
                }
                Ok(())
            })?;
        }
        self.release_renderer_without_context_reset()
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    #[cfg(test)]
    fn renderer_address_for_test(&self) -> *const () {
        self.renderer
            .borrow()
            .as_ref()
            .map_or(std::ptr::null(), RendererStorage::address)
    }

    #[cfg(test)]
    fn panic_next_callback_for_test(&self) {
        self.panic_next_callback.set(true);
    }

    #[cfg(test)]
    pub(super) fn maybe_panic_callback_for_test(&self) {
        assert!(
            !self.panic_next_callback.replace(false),
            "injected Ash viewport callback panic"
        );
    }

    #[cfg(test)]
    pub(super) fn probe_renderer_storage_for_test(&self) -> Result<(), AshViewportError> {
        let storage =
            self.renderer
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "injected stable-storage probe",
                })?;
        let storage = storage.as_ref().ok_or(AshViewportError::RuntimeDetached)?;
        assert!(!storage.address().is_null());
        self.callback_probe_count
            .set(self.callback_probe_count.get() + 1);
        Ok(())
    }

    #[cfg(test)]
    fn trigger_reentrant_entry_for_test(&self) {
        let _borrow = self.renderer.borrow_mut();
        let error = self
            .with_renderer_callback("injected reentry", |_renderer, _globals| Ok(()))
            .unwrap_err();
        self.record_fault(error);
    }

    #[cfg(test)]
    fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.transitions.borrow().clone()
    }

    #[cfg(test)]
    fn callback_probe_count_for_test(&self) -> usize {
        self.callback_probe_count.get()
    }
}

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(|| {
            if let Err(error) = self.shutdown_once(ShutdownAction::Quiesce) {
                self.record_fault(error);
            }
        });
    }

    fn release_renderer_resources(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(|| {
            if let Err(error) = self.shutdown_once(ShutdownAction::ContextResources) {
                self.record_fault(error);
            }
        });
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.mark_context_destroyed();
    }
}

/// Backend-local owning runtime shared by the Winit and SDL3 typed wrappers.
pub(crate) struct OwningViewportRuntime {
    control: Rc<RuntimeControl>,
}

impl fmt::Debug for OwningViewportRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwningViewportRuntime")
            .field("control", &self.control)
            .finish()
    }
}

impl OwningViewportRuntime {
    pub(crate) unsafe fn attach(
        context: &mut Context,
        renderer: AshRenderer,
        config: VulkanViewportConfig,
        surface_adapter: Arc<dyn SurfaceAdapter>,
    ) -> Result<Self, AshViewportAttachError> {
        if let Err(error) = renderer.ensure_context_matches(context) {
            return Err(AshViewportAttachError::new(error.into(), renderer));
        }
        if let Err(error) = preflight_callbacks(context) {
            return Err(AshViewportAttachError::new(error, renderer));
        }
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(AshViewportAttachError::new(error, renderer));
        }

        let validation_surface = config.validation_surface;
        let globals = GlobalHandles {
            entry: config.entry,
            instance: config.instance,
            physical_device: config.physical_device,
            present_queue: config.present_queue,
            graphics_queue_family_index: config.graphics_queue_family_index,
            present_queue_family_index: config.present_queue_family_index,
            in_flight_frames: renderer.options.in_flight_frames.max(1),
            surface_adapter,
        };
        let validation = validate_vulkan_config(&globals).and_then(|()| {
            query_surface_support(&globals, validation_surface)
                .map(|_| ())
                .map_err(Into::into)
        });
        if let Err(error) = validation {
            return Err(AshViewportAttachError::new(error, renderer));
        }

        let control = Rc::new(RuntimeControl::new(context, renderer, globals));
        let attachment = match context.register_attachment::<AshRendererAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        ) {
            Ok(attachment) => attachment,
            Err(error) => {
                let renderer = control.recover_renderer();
                return Err(AshViewportAttachError::new(error.into(), renderer));
            }
        };
        control.store_attachment(attachment);
        register_runtime(&control);
        claim_callbacks(&control, context);
        control.set_state(RuntimeState::Attached);
        Ok(Self { control })
    }

    #[cfg(test)]
    pub(super) fn attach_for_test(context: &mut Context) -> Result<Self, AshViewportError> {
        preflight_callbacks(context)?;
        preflight_runtime(context.id())?;
        let control = Rc::new(RuntimeControl::new_with_storage(
            context,
            RendererStorage::Fake(Box::new(0)),
            None,
        ));
        let attachment = context.register_attachment::<AshRendererAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        )?;
        control.store_attachment(attachment);
        register_runtime(&control);
        claim_callbacks(&control, context);
        control.set_state(RuntimeState::Attached);
        Ok(Self { control })
    }

    pub(crate) fn poll_fault(&self) -> Result<(), AshViewportError> {
        if let Some(fault) = self.control.detect_and_take_fault() {
            return Err(fault);
        }
        if self.control.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(AshViewportError::RuntimeDetached)
        }
    }

    pub(crate) fn cmd_draw(
        &self,
        command_buffer: ash::vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.control.with_renderer_mut("cmd_draw", |renderer| {
            renderer.cmd_draw(command_buffer, frame).map_err(Into::into)
        })
    }

    pub(crate) fn pending_texture_retirement(
        &self,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.control
            .with_renderer(AshRenderer::pending_texture_retirement)
    }

    pub(crate) fn wait_for_texture_retirements(
        &self,
        batch: TextureRetirementBatch,
    ) -> Result<usize, AshViewportError> {
        self.control
            .with_renderer_mut("wait_for_texture_retirements", |renderer| {
                renderer
                    .wait_for_texture_retirements(batch)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn complete_texture_retirements_with_fences(
        &self,
        batch: TextureRetirementBatch,
        fences: &[ash::vk::Fence],
    ) -> Result<usize, AshViewportError> {
        self.control.with_renderer_mut(
            "complete_texture_retirements_with_fences",
            |renderer| unsafe {
                renderer
                    .complete_texture_retirements_with_fences(batch, fences)
                    .map_err(Into::into)
            },
        )
    }

    pub(crate) fn set_viewport_clear_color(&self, color: [f32; 4]) -> Result<(), AshViewportError> {
        self.control
            .with_renderer_mut("set_viewport_clear_color", |renderer| {
                renderer.set_viewport_clear_color(color);
                Ok(())
            })
    }

    pub(crate) fn viewport_clear_color(&self) -> Result<[f32; 4], AshViewportError> {
        self.control
            .with_renderer(AshRenderer::viewport_clear_color)
    }

    pub(crate) fn options(&self) -> Result<Options, AshViewportError> {
        self.control.with_renderer(AshRenderer::options)
    }

    pub(crate) fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&AshRenderer) -> R,
    ) -> Result<R, AshViewportError> {
        self.control.with_renderer(callback)
    }

    pub(crate) fn register_texture_descriptor_set(
        &self,
        set: ash::vk::DescriptorSet,
    ) -> Result<TextureId, AshViewportError> {
        self.control
            .with_renderer_mut("register_texture_descriptor_set", |renderer| {
                Ok(renderer.register_texture_descriptor_set(set))
            })
    }

    pub(crate) fn register_external_texture_with_sampler(
        &self,
        image_view: ash::vk::ImageView,
        sampler: ash::vk::Sampler,
    ) -> Result<TextureId, AshViewportError> {
        self.control
            .with_renderer_mut("register_external_texture_with_sampler", |renderer| {
                renderer
                    .register_external_texture_with_sampler(image_view, sampler)
                    .map_err(Into::into)
            })
    }

    pub(crate) fn remove_texture_descriptor_set(
        &self,
        texture: TextureId,
    ) -> Result<(), AshViewportError> {
        self.control
            .with_renderer_mut("remove_texture_descriptor_set", |renderer| {
                renderer
                    .remove_texture_descriptor_set(texture)
                    .map_err(Into::into)
            })
    }

    pub(crate) fn update_external_texture_view(
        &self,
        texture: TextureId,
        image_view: ash::vk::ImageView,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture_view", |renderer| {
                renderer
                    .update_external_texture_view(texture, image_view)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn update_external_texture_view_unchecked(
        &self,
        texture: TextureId,
        image_view: ash::vk::ImageView,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture_view_unchecked", |renderer| {
                Ok(unsafe { renderer.update_external_texture_view_unchecked(texture, image_view) })
            })
    }

    pub(crate) fn update_external_texture_sampler(
        &self,
        texture: TextureId,
        sampler: ash::vk::Sampler,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture_sampler", |renderer| {
                renderer
                    .update_external_texture_sampler(texture, sampler)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn update_external_texture_sampler_unchecked(
        &self,
        texture: TextureId,
        sampler: ash::vk::Sampler,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture_sampler_unchecked", |renderer| {
                Ok(unsafe { renderer.update_external_texture_sampler_unchecked(texture, sampler) })
            })
    }

    pub(crate) fn unregister_texture(&self, texture: TextureId) -> Result<(), AshViewportError> {
        self.control
            .with_renderer_mut("unregister_texture", |renderer| {
                renderer.unregister_texture(texture).map_err(Into::into)
            })
    }

    pub(crate) unsafe fn unregister_texture_unchecked(
        &self,
        texture: TextureId,
    ) -> Result<(), AshViewportError> {
        self.control
            .with_renderer_mut("unregister_texture_unchecked", |renderer| {
                unsafe { renderer.unregister_texture_unchecked(texture) };
                Ok(())
            })
    }

    pub(crate) fn update_texture(
        &self,
        texture: &TextureData,
    ) -> Result<TextureUpdateResult, AshViewportError> {
        self.control
            .with_renderer_mut("update_texture", |renderer| {
                renderer.update_texture(texture).map_err(Into::into)
            })
    }

    pub(crate) unsafe fn update_texture_unchecked(
        &self,
        texture: &TextureData,
    ) -> Result<TextureUpdateResult, AshViewportError> {
        self.control
            .with_renderer_mut("update_texture_unchecked", |renderer| unsafe {
                renderer
                    .update_texture_unchecked(texture)
                    .map_err(Into::into)
            })
    }

    pub(crate) fn shutdown(&mut self, context: &mut Context) -> Result<(), AshViewportError> {
        self.control.ensure_context(context)?;
        let pending = self.control.detect_and_take_fault();
        let binding = self.control.binding.clone();
        let result = binding
            .try_with_bound_context(|| {
                self.control
                    .shutdown_once(ShutdownAction::Explicit(context))
            })
            .map_err(Into::into)
            .and_then(|result| result);
        match (pending, result) {
            (Some(fault), Err(shutdown_error)) => {
                self.control.record_fault(shutdown_error);
                Err(fault)
            }
            (Some(fault), Ok(())) => Err(fault),
            (None, result) => result,
        }
    }

    pub(crate) fn retry_retained_cleanup(&mut self) -> Result<(), AshViewportError> {
        if self.control.binding.lifecycle() != ContextLifecycle::NativeDestroyed {
            return Err(AshViewportError::RuntimeDetached);
        }
        self.control.retry_detached_cleanup()
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const () {
        self.control.renderer_address_for_test()
    }

    #[cfg(test)]
    pub(super) fn state_for_test(&self) -> RuntimeState {
        self.control.state()
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.control.transition_log_for_test()
    }

    #[cfg(test)]
    pub(super) fn control_for_test(&self) -> Rc<RuntimeControl> {
        Rc::clone(&self.control)
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.control.panic_next_callback_for_test();
    }

    #[cfg(test)]
    pub(super) fn trigger_reentrant_entry_for_test(&self) {
        self.control.trigger_reentrant_entry_for_test();
    }

    #[cfg(test)]
    pub(super) fn callback_probe_count_for_test(&self) -> usize {
        self.control.callback_probe_count_for_test()
    }
}

impl Drop for OwningViewportRuntime {
    fn drop(&mut self) {
        self.control.owner_dropped();
    }
}

pub(crate) unsafe fn attach_with_adapter(
    renderer: AshRenderer,
    context: &mut Context,
    config: VulkanViewportConfig,
    surface_adapter: Arc<dyn SurfaceAdapter>,
) -> Result<OwningViewportRuntime, AshViewportAttachError> {
    unsafe { OwningViewportRuntime::attach(context, renderer, config, surface_adapter) }
}

fn first_error<const N: usize>(
    errors: [Option<AshViewportError>; N],
) -> Result<(), AshViewportError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}

fn map_renderer_shutdown_result(
    result: Result<(), RendererError>,
    operation: &'static str,
) -> Result<(), AshViewportError> {
    match result {
        Ok(()) => Ok(()),
        Err(RendererError::Vulkan(ash::vk::Result::ERROR_DEVICE_LOST)) => {
            Err(AshViewportError::DeviceLost { operation })
        }
        Err(error) => Err(error.into()),
    }
}
