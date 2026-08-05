use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

#[cfg(test)]
use dear_imgui_rs::render::{FrameSnapshot, RendererConsumer};
use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextBinding, ContextBindingError, ContextId, ContextLifecycle,
    TextureData, TextureId,
};
use thiserror::Error;

use super::callbacks::{claim_callbacks, preflight_callbacks};
use super::registry::{
    GlobalHandles, ViewportIdentity, preflight_runtime, query_surface_support, register_runtime,
    validate_vulkan_config,
};
use super::trace::{AshViewportFrameReport, FrameTraceState};
use super::{SurfaceAdapter, SurfaceCreateError, SurfaceSupportError, VulkanViewportConfig};
use crate::{AshRenderer, Options, RendererError, TextureRetirementBatch, TextureUpdateResult};

mod cleanup;
mod fault;
mod lifecycle;
mod state;
mod viewport_registry;

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
    /// The typed platform owner belongs to another Dear ImGui Context.
    #[error("{backend} platform runtime belongs to Context {actual:?}, not {expected:?}")]
    PlatformOwnerContextMismatch {
        backend: &'static str,
        expected: ContextId,
        actual: ContextId,
    },
    /// The typed Winit platform owner rejected renderer attachment.
    #[cfg(feature = "multi-viewport-winit")]
    #[error(transparent)]
    WinitPlatform(#[from] dear_imgui_winit::WinitPlatformError),
    /// A callback entry was not running under this runtime's Context.
    #[error("the current Dear ImGui Context does not match Ash runtime Context {expected:?}")]
    BoundContextMismatch { expected: ContextId },
    /// Renderer callbacks require an attached platform runtime.
    #[error("Ash multi-viewport requires an attached multi-viewport platform runtime")]
    PlatformBackendUnavailable,
    /// A required platform callback is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// The typed SDL3 platform owner rejected attachment.
    #[cfg(feature = "multi-viewport-sdl3")]
    #[error(transparent)]
    Sdl3Platform(#[from] dear_imgui_sdl3::Sdl3BackendError),
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
    /// A secondary-viewport frame trace is already active for this runtime.
    #[error("an Ash viewport frame trace is already active")]
    FrameTraceAlreadyActive,
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
    failed_viewports: RefCell<HashSet<ViewportIdentity>>,
    // Native RendererUserData stores addresses into these boxes, so vector relocation must not
    // relocate the sidecar allocations themselves.
    #[allow(
        clippy::vec_box,
        reason = "native viewport callbacks retain sidecar addresses"
    )]
    retained_viewports: RefCell<Vec<Box<super::ViewportAshData>>>,
    faults: RefCell<RuntimeFaults>,
    frame_trace: RefCell<FrameTraceState>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    callback_probe_count: Cell<usize>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
    #[cfg(test)]
    renderer_contract_fault: Cell<Option<&'static str>>,
}

#[derive(Default)]
struct RuntimeFaults {
    terminal: Option<AshViewportError>,
    non_terminal: Option<AshViewportError>,
}

impl RuntimeFaults {
    fn record_terminal(&mut self, fault: AshViewportError) {
        if self.terminal.is_none() {
            self.terminal = Some(fault);
        }
    }

    fn record_non_terminal(&mut self, fault: AshViewportError) {
        if self.non_terminal.is_none() {
            self.non_terminal = Some(fault);
        }
    }

    fn has_pending(&self) -> bool {
        self.terminal.is_some() || self.non_terminal.is_some()
    }

    fn take_next(&mut self) -> Option<AshViewportError> {
        self.terminal.take().or_else(|| self.non_terminal.take())
    }
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

/// Backend-local owning runtime shared by the Winit and SDL3 typed wrappers.
pub(crate) struct OwningViewportRuntime {
    control: Rc<RuntimeControl>,
}

/// A non-nestable trace scope for one secondary-viewport Vulkan rendering pass.
///
/// Call [`Self::finish`] after `render_platform_windows_default` to obtain same-scope evidence of
/// successful render submission and presentation. Dropping the guard discards partial evidence.
#[must_use = "finish the frame trace to obtain its report"]
pub struct AshViewportFrameTrace<'runtime> {
    control: &'runtime RuntimeControl,
    active: bool,
}

impl AshViewportFrameTrace<'_> {
    /// Ends the trace and returns normalized secondary-viewport submission evidence.
    pub fn finish(mut self) -> AshViewportFrameReport {
        let report = self.control.finish_frame_trace();
        self.active = false;
        report
    }
}

impl Drop for AshViewportFrameTrace<'_> {
    fn drop(&mut self) {
        if self.active {
            self.control.abort_frame_trace();
        }
    }
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
    pub(crate) fn begin_frame_trace(&self) -> Result<AshViewportFrameTrace<'_>, AshViewportError> {
        self.control.begin_frame_trace()?;
        Ok(AshViewportFrameTrace {
            control: self.control.as_ref(),
            active: true,
        })
    }

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
            swapchain_image_usage: config.swapchain_image_usage,
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

    pub(crate) unsafe fn cmd_draw(
        &self,
        command_buffer: ash::vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.control
            .with_renderer_mut("cmd_draw", |renderer| unsafe {
                renderer.cmd_draw(command_buffer, frame).map_err(Into::into)
            })
    }

    pub(crate) fn prepare_frame(
        &self,
        frame: &mut RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, AshViewportError> {
        self.control.with_renderer_mut("prepare_frame", |renderer| {
            renderer.prepare_frame(frame).map_err(Into::into)
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

    pub(crate) unsafe fn register_external_texture(
        &self,
        image_view: ash::vk::ImageView,
        image_layout: ash::vk::ImageLayout,
    ) -> Result<TextureId, AshViewportError> {
        self.control
            .with_renderer_mut("register_external_texture", |renderer| unsafe {
                renderer
                    .register_external_texture(image_view, image_layout)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn update_external_texture(
        &self,
        texture: TextureId,
        image_view: ash::vk::ImageView,
        image_layout: ash::vk::ImageLayout,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture", |renderer| unsafe {
                renderer
                    .update_external_texture(texture, image_view, image_layout)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn update_external_texture_unchecked(
        &self,
        texture: TextureId,
        image_view: ash::vk::ImageView,
        image_layout: ash::vk::ImageLayout,
    ) -> Result<bool, AshViewportError> {
        self.control
            .with_renderer_mut("update_external_texture_unchecked", |renderer| unsafe {
                renderer
                    .update_external_texture_unchecked(texture, image_view, image_layout)
                    .map_err(Into::into)
            })
    }

    pub(crate) unsafe fn unregister_texture(
        &self,
        texture: TextureId,
    ) -> Result<(), AshViewportError> {
        self.control
            .with_renderer_mut("unregister_texture", |renderer| unsafe {
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
