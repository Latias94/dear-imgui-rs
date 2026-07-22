use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

#[cfg(test)]
use dear_imgui_rs::render::FrameSnapshot;
use dear_imgui_rs::render::{RenderedFrame, RendererConsumer};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding, ContextBindingError,
    ContextDestroyed, ContextId, ContextLifecycle, ContextTeardown, Id, TextureData, TextureId,
    platform_io::{PlatformIo, Viewport},
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, destroy_renderer_viewport_resources, detect_runtime_contract_drift,
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
    /// Another renderer already advertises multi-viewport renderer support.
    #[error("RENDERER_HAS_VIEWPORTS is already advertised by another renderer")]
    RendererViewportCapabilityOccupied,
    /// This runtime's renderer capability bit was cleared while it remained attached.
    #[error("RENDERER_HAS_VIEWPORTS was cleared while the Ash runtime was attached")]
    RendererViewportCapabilityLost,
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
    ContextTeardown,
}

enum RendererStorage {
    Real(Box<AshRenderer>),
    #[cfg(test)]
    Fake {
        probe: Box<u8>,
        consumer: Option<RendererConsumer>,
    },
}

impl RendererStorage {
    fn real_mut(&mut self) -> Option<&mut AshRenderer> {
        match self {
            Self::Real(renderer) => Some(renderer),
            #[cfg(test)]
            Self::Fake { .. } => None,
        }
    }

    fn ensure_operational(&self) -> Result<(), AshViewportError> {
        match self {
            Self::Real(renderer) => renderer.ensure_operational().map_err(Into::into),
            #[cfg(test)]
            Self::Fake { .. } => Ok(()),
        }
    }

    #[cfg(test)]
    fn address(&self) -> *const () {
        match self {
            Self::Real(renderer) => std::ptr::from_ref(renderer.as_ref()).cast(),
            Self::Fake { probe, .. } => std::ptr::from_ref(probe.as_ref()).cast(),
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
    // ImGui clears PlatformRequestClose at the end of UpdatePlatformWindows, including failures
    // raised by Renderer_CreateWindow in that same call.
    failed_viewports: RefCell<HashSet<Id>>,
    retained_viewports: RefCell<Vec<Box<super::ViewportAshData>>>,
    faults: RefCell<Option<AshViewportError>>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    callback_probe_count: Cell<usize>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
    #[cfg(test)]
    renderer_contract_fault: Cell<Option<&'static str>>,
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
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(renderer)),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            failed_viewports: RefCell::new(HashSet::new()),
            retained_viewports: RefCell::new(Vec::new()),
            faults: RefCell::new(None),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            callback_probe_count: Cell::new(0),
            #[cfg(test)]
            transitions: RefCell::new(Vec::new()),
            #[cfg(test)]
            renderer_contract_fault: Cell::new(None),
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

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(super) fn can_enter_callback(&self) -> bool {
        self.is_callback_accessible()
            && self
                .faults
                .try_borrow()
                .is_ok_and(|faults| faults.is_none())
    }

    pub(super) fn should_validate_runtime_contract(&self) -> bool {
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

    pub(super) fn record_runtime_contract_fault(&self, fault: AshViewportError) {
        self.record_fault(fault);
        self.begin_shutdown();
    }

    pub(super) fn validate_renderer_contract(&self) -> Result<(), AshViewportError> {
        #[cfg(test)]
        if let Some(field) = self.renderer_contract_fault.get() {
            return Err(RendererError::RendererStateReplaced { field }.into());
        }
        let renderer =
            self.renderer
                .try_borrow()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "validate renderer runtime contract",
                })?;
        renderer
            .as_ref()
            .ok_or(AshViewportError::RuntimeDetached)?
            .ensure_operational()
    }

    /// Returns whether the current Context still has an exact core renderer publication owned by
    /// this runtime. A callback-table takeover may replace every viewport callback while leaving
    /// a partially owned core lease behind; in that case failure handling must still revoke the
    /// viewport capability. If ownership cannot be proven, callers preserve the shared bit.
    pub(super) fn owns_core_renderer_publication(&self, platform_io: &PlatformIo) -> bool {
        let Ok(renderer) = self.renderer.try_borrow() else {
            return false;
        };
        match renderer.as_ref() {
            Some(RendererStorage::Real(renderer)) => renderer
                .context_state
                .owns_core_publication_bound(platform_io),
            #[cfg(test)]
            Some(RendererStorage::Fake { .. }) | None => false,
            #[cfg(not(test))]
            None => false,
        }
    }

    pub(super) fn mark_viewport_create_failed(&self, viewport: &mut Viewport) {
        self.failed_viewports.borrow_mut().insert(viewport.id());
        viewport.set_platform_request_close(true);
    }

    pub(super) fn clear_viewport_create_failure(&self, viewport: &Viewport) {
        self.failed_viewports.borrow_mut().remove(&viewport.id());
    }

    pub(super) fn clear_viewport_create_failures(&self) {
        self.failed_viewports.borrow_mut().clear();
    }

    pub(super) fn reassert_failed_viewport_closures(&self) {
        let failed_viewports = self
            .failed_viewports
            .borrow()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if failed_viewports.is_empty() {
            return;
        }
        for id in failed_viewports {
            // A failed secondary viewport may be hidden from `PlatformIO.Viewports` while Dear
            // ImGui still owns it internally. Reassert the close request through the complete
            // internal lookup so its platform/renderer sidecars can be destroyed normally.
            let viewport = unsafe { sys::igFindViewportByID(id.raw()) };
            if !viewport.is_null() {
                // SAFETY: this runtime's Context is current while contract drift is checked.
                unsafe { Viewport::from_raw_mut(viewport) }.set_platform_request_close(true);
            }
        }
    }

    fn detect_and_take_fault(&self) -> Option<AshViewportError> {
        detect_runtime_contract_drift(self);
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
            renderer.ensure_operational()?;
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
                Some(RendererStorage::Fake { .. }) | None => {
                    return Err(AshViewportError::RuntimeDetached);
                }
                #[cfg(not(test))]
                None => return Err(AshViewportError::RuntimeDetached),
            };
            renderer.ensure_operational()?;
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
        renderer.ensure_operational()?;
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
        if matches!(storage, RendererStorage::Fake { .. }) {
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

    fn shutdown_explicit(&self, context: &mut Context) -> Result<(), AshViewportError> {
        // Validate snapshot completion before mutating any viewport, callback, or runtime state.
        // A failed permit preparation must leave the entire multi-viewport runtime retryable.
        let consumer = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            let storage = storage.as_mut().ok_or(AshViewportError::RuntimeDetached)?;
            match storage {
                RendererStorage::Real(renderer) => renderer.take_shutdown_consumer()?,
                #[cfg(test)]
                RendererStorage::Fake { consumer, .. } => {
                    consumer.take().ok_or(RendererError::RendererNotAttached)?
                }
            }
        };

        let permit = match context.prepare_renderer_texture_reset(&consumer) {
            Ok(permit) => permit,
            Err(error) => {
                self.restore_explicit_shutdown_consumer(consumer)?;
                return Err(RendererError::from(error).into());
            }
        };

        self.begin_shutdown();
        if let Err(error) = destroy_renderer_viewport_resources(self) {
            drop(permit);
            self.restore_explicit_shutdown_consumer(consumer)?;
            return Err(error);
        }
        let callback_result = release_callbacks(self);
        self.mark_detached();

        let (shutdown_result, destroyed) = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
                RendererStorage::Real(renderer) => {
                    let shutdown_result = renderer.destroy_internal();
                    (shutdown_result, renderer.destroyed)
                }
                #[cfg(test)]
                RendererStorage::Fake { .. } => (Ok(()), true),
            }
        };

        if !destroyed {
            drop(permit);
            self.restore_explicit_shutdown_consumer(consumer)?;
            return first_error([
                callback_result.err(),
                map_renderer_shutdown_result(shutdown_result, "renderer shutdown").err(),
            ]);
        }

        // The renderer's complete texture map is gone, so the already-validated reset can now
        // invalidate Context-owned bindings before we publish the renderer teardown.
        let _ = permit.commit();
        let renderer = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
                RendererStorage::Real(renderer) => renderer.finalize_shutdown_after_reset(context),
                #[cfg(test)]
                RendererStorage::Fake { .. } => {}
            }
            storage.take()
        };
        drop(renderer);
        drop(consumer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        self.detach_attachment();
        first_error([
            callback_result.err(),
            map_renderer_shutdown_result(shutdown_result, "renderer shutdown").err(),
        ])
    }

    fn restore_explicit_shutdown_consumer(
        &self,
        consumer: RendererConsumer,
    ) -> Result<(), AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
            RendererStorage::Real(renderer) => renderer.restore_shutdown_consumer(consumer),
            #[cfg(test)]
            RendererStorage::Fake {
                consumer: stored_consumer,
                ..
            } => {
                debug_assert!(stored_consumer.is_none());
                *stored_consumer = Some(consumer);
            }
        }
        Ok(())
    }

    fn take_context_teardown_consumer(&self) -> Result<RendererConsumer, AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
            RendererStorage::Real(renderer) => {
                renderer.take_shutdown_consumer().map_err(Into::into)
            }
            #[cfg(test)]
            RendererStorage::Fake { consumer, .. } => consumer
                .take()
                .ok_or(RendererError::RendererNotAttached)
                .map_err(Into::into),
        }
    }

    /// Releases the renderer after Context entered its terminal teardown phase.
    ///
    /// This may run only from the release closure of `ContextTeardown::with_renderer_texture_reset`.
    fn release_renderer_during_context_teardown(&self) -> Result<(), AshViewportError> {
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
            RendererStorage::Real(renderer) => renderer.shutdown_during_context_teardown(),
            #[cfg(test)]
            RendererStorage::Fake { .. } => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake { .. } => true,
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

    /// Releases remaining renderer resources after native Context destruction.
    ///
    /// A previous Context teardown can only reach this retry path after a retryable Vulkan wait
    /// failure. Native ImGui state is gone, so the renderer must not attempt a texture reset or
    /// touch current-context global pointers.
    fn release_renderer_after_context_destroyed(&self) -> Result<(), AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "destroyed Context renderer-resource cleanup",
                })?;
        let Some(storage) = renderer.as_mut() else {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        };
        let shutdown_result = match storage {
            RendererStorage::Real(renderer) => renderer.shutdown_after_context_destroyed(),
            #[cfg(test)]
            RendererStorage::Fake { .. } => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake { .. } => true,
        };
        if !may_release {
            return shutdown_result.map_err(Into::into);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        map_renderer_shutdown_result(
            shutdown_result,
            "renderer cleanup after Context destruction",
        )
    }

    fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), AshViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            if !matches!(action, ShutdownAction::ContextTeardown) {
                self.detach_attachment();
            }
            return Ok(());
        }

        match action {
            ShutdownAction::Quiesce => {
                self.begin_shutdown();
                release_callbacks(self)
            }
            ShutdownAction::Explicit(context) => self.shutdown_explicit(context),
            ShutdownAction::ContextTeardown => {
                self.begin_shutdown();
                // A failed ownership preflight leaves every sidecar and callback publication
                // intact. Do not clear `Renderer_DestroyWindow` while it is still the only safe
                // way to reclaim a live foreign-replaced slot.
                destroy_renderer_viewport_resources(self)?;
                let callback_result = release_callbacks(self);
                self.mark_detached();
                let renderer_result = self.release_renderer_during_context_teardown();
                first_error([callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        match self.binding.lifecycle() {
            // `Drop` has no exclusive `&mut Context`, so it cannot prepare and commit the
            // renderer-texture reset transaction. Leave the attachment owned by Context instead
            // of releasing Vulkan resources behind still-live managed texture bindings.
            ContextLifecycle::Alive => self.defer_attachment_to_context(),
            // Context has already begun its ordered teardown and still owns this attachment.
            // Its RendererResources phase performs the terminal cleanup with the Context bound.
            ContextLifecycle::Dropping => {}
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

    fn defer_attachment_to_context(&self) {
        if let Some(attachment) = self.attachment.borrow_mut().take() {
            attachment.defer_to_context();
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
            RendererStorage::Fake { .. } => {
                unreachable!("test runtime does not recover AshRenderer")
            }
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
        self.release_renderer_after_context_destroyed()
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
    fn snapshot_for_shutdown_test(&self, context: &mut Context) -> FrameSnapshot {
        let renderer = self.renderer.borrow();
        let consumer = match renderer.as_ref() {
            Some(RendererStorage::Fake {
                consumer: Some(consumer),
                ..
            }) => consumer,
            Some(RendererStorage::Real(_))
            | Some(RendererStorage::Fake { consumer: None, .. })
            | None => panic!("test runtime has no active renderer consumer"),
        };
        context
            .begin_frame()
            .render_snapshot(consumer)
            .expect("test runtime consumer must capture a snapshot")
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
    pub(super) fn replace_renderer_contract_for_test(&self, field: &'static str) {
        self.renderer_contract_fault.set(Some(field));
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
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| match self.shutdown_once(ShutdownAction::Quiesce) {
            Ok(()) => Ok(()),
            Err(error) => {
                let teardown_error = ContextAttachmentTeardownError::new(error.to_string());
                self.record_fault(error);
                Err(teardown_error)
            }
        })
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            let consumer = self
                .take_context_teardown_consumer()
                .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))?;
            let mut terminal_error = None;
            let reset = context.with_renderer_texture_reset(&consumer, || {
                match self.shutdown_once(ShutdownAction::ContextTeardown) {
                    Ok(()) => Ok(()),
                    Err(error) if self.state.get() == RuntimeState::ResourceDropped => {
                        terminal_error = Some(error);
                        Ok(())
                    }
                    Err(error) => Err(ContextAttachmentTeardownError::new(error.to_string())),
                }
            });
            if let Err(error) = reset {
                if let Err(restore_error) = self.restore_explicit_shutdown_consumer(consumer) {
                    self.record_fault(restore_error);
                }
                return Err(error);
            }
            drop(consumer);

            if let Some(error) = terminal_error {
                let teardown_error = ContextAttachmentTeardownError::new(error.to_string());
                self.record_fault(error);
                Err(teardown_error)
            } else {
                Ok(())
            }
        })
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
        if let Err(error) = preflight_attachment_with(context, || {
            renderer.ensure_context_matches(context)?;
            renderer.ensure_operational()?;
            Ok(())
        }) {
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
            swapchain_policy: config.swapchain_policy,
            surface_adapter,
        };
        let validation = validate_vulkan_config(&globals).and_then(|()| {
            query_surface_support(&globals, validation_surface)
                .and_then(|support| {
                    super::swapchain::resolve_swapchain_policy(
                        globals.swapchain_policy,
                        &support.formats,
                        &support.present_modes,
                    )
                    .map(|_| ())
                })
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
        let consumer = context
            .create_renderer_consumer()
            .map_err(RendererError::from)?;
        // The fake renderer below has no Vulkan texture map and cannot have submitted an epoch.
        // Commit the empty transaction before the test runtime claims callbacks.
        let reset = context
            .prepare_renderer_texture_reset(&consumer)
            .map_err(RendererError::from)?;
        let _ = reset.commit();
        let control = Rc::new(RuntimeControl::new_with_storage(
            context,
            RendererStorage::Fake {
                probe: Box::new(0),
                consumer: Some(consumer),
            },
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
            .and_then(|result| result.map_err(Into::into))
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
                renderer
                    .register_texture_descriptor_set(set)
                    .map_err(Into::into)
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
        self.control.with_renderer_mut(
            "update_external_texture_view_unchecked",
            |renderer| unsafe {
                renderer
                    .update_external_texture_view_unchecked(texture, image_view)
                    .map_err(Into::into)
            },
        )
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
        self.control.with_renderer_mut(
            "update_external_texture_sampler_unchecked",
            |renderer| unsafe {
                renderer
                    .update_external_texture_sampler_unchecked(texture, sampler)
                    .map_err(Into::into)
            },
        )
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
            .with_renderer_mut("unregister_texture_unchecked", |renderer| unsafe {
                renderer
                    .unregister_texture_unchecked(texture)
                    .map_err(Into::into)
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
    pub(super) fn snapshot_for_shutdown_test(&self, context: &mut Context) -> FrameSnapshot {
        self.control.snapshot_for_shutdown_test(context)
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

pub(super) fn preflight_attachment_with(
    context: &Context,
    validate_renderer: impl FnOnce() -> Result<(), AshViewportError>,
) -> Result<(), AshViewportError> {
    validate_renderer()?;
    preflight_callbacks(context)?;
    preflight_runtime(context.id())
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
