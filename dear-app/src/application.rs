use std::{error::Error as StdError, fmt};

use dear_imgui_rs as imgui;
use dear_imgui_rs::{DockNodeFlags, TextureId};
use thiserror::Error;
use winit::{event::WindowEvent, window::Window};

use crate::AppConfig;

#[cfg(feature = "imnodes")]
use dear_imnodes as imnodes;
#[cfg(feature = "implot")]
use dear_implot as implot;
#[cfg(feature = "implot3d")]
use dear_implot3d as implot3d;

/// Monotonically increasing identity of the active GPU resource set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GpuGeneration(pub(crate) u64);

impl GpuGeneration {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    fn ensure_current(self, current: Self) -> Result<(), ExternalTextureError> {
        if self == current {
            Ok(())
        } else {
            Err(ExternalTextureError::StaleGeneration {
                handle_generation: self.get(),
                current_generation: current.get(),
            })
        }
    }
}

/// Opaque external texture identity bound to one GPU generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternalTextureHandle {
    id: dear_imgui_wgpu::ExternalTextureId,
    generation: GpuGeneration,
}

impl ExternalTextureHandle {
    #[must_use]
    pub const fn generation(self) -> GpuGeneration {
        self.generation
    }

    fn resolve_for_generation(
        self,
        current: GpuGeneration,
    ) -> Result<dear_imgui_wgpu::ExternalTextureId, ExternalTextureError> {
        self.generation.ensure_current(current)?;
        Ok(self.id)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExternalTextureError {
    #[error(
        "external texture belongs to GPU generation {handle_generation}, current generation is {current_generation}"
    )]
    StaleGeneration {
        handle_generation: u64,
        current_generation: u64,
    },
    #[error(transparent)]
    Renderer(#[from] dear_imgui_wgpu::RendererError),
}

/// Lifecycle hook that produced an [`Application`] error.
///
/// The runtime assigns this stage at the hook boundary. Applications therefore do not need to
/// encode callback names in strings, and callers can inspect failures without parsing messages.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ApplicationStage {
    ConfigureImgui,
    Initialized,
    Event,
    PrepareFrame,
    Frame,
    GpuLost,
    GpuRecreated,
    Shutdown,
}

impl ApplicationStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigureImgui => "configure_imgui",
            Self::Initialized => "initialized",
            Self::Event => "event",
            Self::PrepareFrame => "prepare_frame",
            Self::Frame => "frame",
            Self::GpuLost => "gpu_lost",
            Self::GpuRecreated => "gpu_recreated",
            Self::Shutdown => "shutdown",
        }
    }
}

impl fmt::Display for ApplicationStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
#[derive(Debug, Error)]
#[error("{0}")]
struct ApplicationMessage(String);

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RunError {
    #[error("event loop error: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("window creation failed: {0}")]
    WindowCreation(#[source] winit::error::OsError),
    #[error("WGPU surface creation failed: {0}")]
    SurfaceCreation(#[source] wgpu::CreateSurfaceError),
    #[error("no suitable WGPU adapter found: {0}")]
    AdapterUnavailable(#[source] wgpu::RequestAdapterError),
    #[error("the selected surface exposes no texture formats")]
    SurfaceFormatUnavailable,
    #[error(
        "the recreated surface changed format from {previous:?} to {replacement:?}; the active renderer cannot be reused"
    )]
    SurfaceFormatChanged {
        previous: wgpu::TextureFormat,
        replacement: wgpu::TextureFormat,
    },
    #[error("WGPU device request failed: {0}")]
    DeviceRequest(#[source] wgpu::RequestDeviceError),
    #[error("Dear ImGui context creation failed: {0}")]
    ImGuiContext(#[source] imgui::ImGuiError),
    #[error("Dear ImGui frame admission failed: {0}")]
    ImGuiFrame(#[source] imgui::render::RendererConsumerError),
    #[error(
        "application callback during {stage} changed Dear ImGui frame ownership from {before:?} to {after:?}"
    )]
    ImGuiFrameOwnership {
        stage: ApplicationStage,
        before: imgui::FrameLifecycleStamp,
        after: imgui::FrameLifecycleStamp,
    },
    #[error("WGPU renderer initialization failed: {0}")]
    RendererInit(#[source] dear_imgui_wgpu::RendererError),
    #[error("WGPU renderer draw failed: {0}")]
    Render(#[source] dear_imgui_wgpu::RendererError),
    #[error("WGPU resource invalidation failed: {0}")]
    GpuInvalidation(#[source] dear_imgui_wgpu::RendererError),
    #[error("WGPU renderer release failed: {0}")]
    RendererRelease(#[source] dear_imgui_wgpu::RendererError),
    #[error("Dear ImGui dockspace submission failed: {0}")]
    Dockspace(#[source] imgui::DockspaceError),
    #[error("Winit platform operation `{operation}` failed: {source}")]
    Platform {
        operation: &'static str,
        #[source]
        source: dear_imgui_winit::WinitPlatformError,
    },
    #[error(
        "dear-app does not support Dear ImGui platform viewports; remove ConfigFlags::VIEWPORTS_ENABLE"
    )]
    MultiViewportUnsupported,
    #[error("WGPU surface validation failed while acquiring the next frame")]
    SurfaceValidation,
    #[error("WGPU exhausted GPU memory: {message}")]
    GpuOutOfMemory { message: String },
    #[error("WGPU reported an uncaptured validation error: {message}")]
    GpuValidation { message: String },
    #[error("WGPU reported an uncaptured internal error: {message}")]
    GpuInternal { message: String },
    #[cfg(feature = "test-engine")]
    #[error("Test Engine frame {frame} failed: {source}")]
    TestEngineFrame {
        frame: u64,
        #[source]
        source: Box<dear_imgui_test_engine::FrameDriverError<RunError, RunError, RunError>>,
    },
    #[error("application callback failed during {stage}: {source}")]
    Application {
        stage: ApplicationStage,
        #[source]
        source: Box<dyn StdError + 'static>,
    },
    #[error("GPU generation recovery failed: {message}")]
    Recovery { message: String },
}

impl RunError {
    /// Wraps an application-owned error with the lifecycle stage that produced it.
    #[must_use]
    pub fn application<E>(stage: ApplicationStage, source: E) -> Self
    where
        E: StdError + 'static,
    {
        Self::Application {
            stage,
            source: Box::new(source),
        }
    }

    /// Returns the lifecycle stage for an application callback failure.
    #[must_use]
    pub const fn application_stage(&self) -> Option<ApplicationStage> {
        match self {
            Self::Application { stage, .. } | Self::ImGuiFrameOwnership { stage, .. } => {
                Some(*stage)
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn application_message(stage: ApplicationStage, message: impl Into<String>) -> Self {
        Self::application(stage, ApplicationMessage(message.into()))
    }

    pub(crate) fn during_application_stage(self, stage: ApplicationStage) -> Self {
        if self.application_stage() == Some(stage) {
            self
        } else {
            Self::application(stage, self)
        }
    }
}

/// Persistent user application. This value survives every GPU recreation.
///
/// The runtime wraps every hook failure with the matching [`ApplicationStage`] while retaining
/// the returned [`RunError`] as its source. Hooks therefore return the most specific error they
/// have instead of formatting callback names into messages.
pub trait Application {
    /// Configures the one stable Dear ImGui context before renderer initialization.
    fn configure_imgui(&mut self, _context: &mut InitContext<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Runs exactly once after the window, UI state, and first GPU generation are ready.
    fn initialized(
        &mut self,
        _context: &mut InitializedContext<'_>,
        _gpu: &mut GpuContext<'_>,
    ) -> Result<(), RunError> {
        Ok(())
    }

    /// Runs before the old GPU generation is invalidated and destroyed.
    fn gpu_lost(&mut self, _context: &mut GpuContext<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Runs after a replacement GPU generation has been committed.
    fn gpu_recreated(&mut self, _context: &mut GpuContext<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Receives events for the live main window only.
    fn event(&mut self, _context: &mut EventContext<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Mutates Context-owned resources before Dear ImGui opens the next frame.
    fn prepare_frame(&mut self, _context: &mut PrepareFrameContext<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Builds one Dear ImGui frame.
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError>;

    /// Returns the Test Engine attached to this application's Context, when enabled.
    ///
    /// The runtime drives this engine only for admitted surface frames and owns the complete
    /// `render -> pre-swap -> present -> post-swap` protocol. Applications may use the engine from
    /// [`Self::frame`] for UI and queue controls, but must not drive presentation independently.
    #[cfg(feature = "test-engine")]
    fn test_engine(&mut self) -> Option<&mut crate::test_engine::TestEngine> {
        None
    }

    /// Runs exactly once before add-ons and the Dear ImGui context are torn down.
    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        Ok(())
    }
}

pub struct InitContext<'a> {
    pub(crate) imgui: &'a mut imgui::Context,
    pub(crate) window: &'a Window,
    pub(crate) config: &'a AppConfig,
}

impl InitContext<'_> {
    /// Returns the Context before platform and renderer ownership is attached.
    ///
    /// This full capability exists for extension crates whose initialization API requires
    /// `&mut Context`. The runtime validates the Context identity and frame progress when the
    /// callback returns. Later lifecycle hooks expose only narrow capabilities.
    pub fn imgui(&mut self) -> &mut imgui::Context {
        self.imgui
    }

    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }

    #[must_use]
    pub fn config(&self) -> &AppConfig {
        self.config
    }
}

/// Post-attachment initialization capabilities without Context or frame ownership.
///
/// ```compile_fail
/// use dear_app::InitializedContext;
///
/// fn replace_context(context: &mut InitializedContext<'_>) {
///     let _ = std::mem::replace(context.imgui(), dear_app::imgui::Context::create());
/// }
/// ```
pub struct InitializedContext<'a> {
    pub(crate) imgui: &'a mut imgui::Context,
    pub(crate) window: &'a Window,
    pub(crate) config: &'a AppConfig,
}

impl InitializedContext<'_> {
    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }

    #[must_use]
    pub fn config(&self) -> &AppConfig {
        self.config
    }
}

/// Event-time capabilities that cannot open or render a Dear ImGui frame.
///
/// Frame lifecycle APIs are intentionally unavailable at this boundary:
///
/// ```compile_fail
/// use dear_app::EventContext;
///
/// fn steal_frame(context: &mut EventContext<'_>) {
///     context.imgui().begin_frame();
/// }
/// ```
pub struct EventContext<'a> {
    pub(crate) event: &'a WindowEvent,
    pub(crate) imgui: &'a mut imgui::Context,
    pub(crate) window: &'a Window,
    pub(crate) exit_requested: &'a mut bool,
}

impl EventContext<'_> {
    #[must_use]
    pub fn event(&self) -> &WindowEvent {
        self.event
    }

    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }

    /// Requests normal event-loop exit after the current event callback completes.
    pub fn request_exit(&mut self) {
        *self.exit_requested = true;
    }
}

/// Terminal resource capabilities without Context or frame ownership.
///
/// ```compile_fail
/// use dear_app::ShutdownContext;
///
/// fn replace_context(context: &mut ShutdownContext<'_>) {
///     let _ = std::mem::replace(context.imgui(), dear_app::imgui::Context::create());
/// }
/// ```
pub struct ShutdownContext<'a> {
    pub(crate) imgui: &'a mut imgui::Context,
    pub(crate) window: &'a Window,
    pub(crate) generation: Option<GpuGeneration>,
}

/// Pre-frame access to Context-owned resources without frame lifecycle authority.
///
/// Resource mutations are allowed here, but opening or rendering the frame remains owned by the
/// runtime:
///
/// ```compile_fail
/// use dear_app::PrepareFrameContext;
///
/// fn steal_frame(context: &mut PrepareFrameContext<'_>) {
///     context.imgui().begin_frame();
/// }
/// ```
pub struct PrepareFrameContext<'a> {
    pub(crate) imgui: &'a mut imgui::Context,
    pub(crate) window: &'a Window,
}

impl PrepareFrameContext<'_> {
    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }
}

macro_rules! impl_context_resource_access {
    ($context:ident) => {
        impl $context<'_> {
            /// Borrows Dear ImGui IO without exposing frame lifecycle operations.
            #[must_use]
            pub fn io(&self) -> &imgui::Io {
                self.imgui.io()
            }

            /// Mutates Dear ImGui IO without exposing frame lifecycle operations.
            pub fn io_mut(&mut self) -> &mut imgui::Io {
                self.imgui.io_mut()
            }

            /// Borrows the global Dear ImGui style.
            #[must_use]
            pub fn style(&self) -> &imgui::Style {
                self.imgui.style()
            }

            /// Mutates the global Dear ImGui style.
            pub fn style_mut(&mut self) -> &mut imgui::Style {
                self.imgui.style_mut()
            }

            /// Borrows the Context-owned font atlas.
            #[must_use]
            pub fn font_atlas(&self) -> &imgui::FontAtlas {
                self.imgui.font_atlas()
            }

            /// Transfers an owned texture into the Context-managed texture registry.
            pub fn register_texture(
                &mut self,
                texture: imgui::OwnedTextureData,
            ) -> imgui::ManagedTextureId {
                self.imgui.register_texture(texture)
            }

            /// Reads a managed texture through a non-escaping capability.
            pub fn with_texture<R>(
                &self,
                id: imgui::ManagedTextureId,
                operation: impl for<'texture> FnOnce(imgui::ManagedTextureRef<'texture>) -> R,
            ) -> Result<R, imgui::ManagedTextureError> {
                self.imgui.with_texture(id, operation)
            }

            /// Mutates a managed texture through a non-escaping capability.
            pub fn with_texture_mut<R>(
                &mut self,
                id: imgui::ManagedTextureId,
                operation: impl for<'texture> FnOnce(imgui::ManagedTextureMut<'texture>) -> R,
            ) -> Result<R, imgui::ManagedTextureError> {
                self.imgui.with_texture_mut(id, operation)
            }

            /// Mutates a managed texture while flattening access and data-validation errors.
            pub fn try_with_texture_mut<R>(
                &mut self,
                id: imgui::ManagedTextureId,
                operation: impl for<'texture> FnOnce(
                    imgui::ManagedTextureMut<'texture>,
                ) -> Result<R, imgui::TextureDataError>,
            ) -> Result<R, imgui::ManagedTextureMutationError> {
                self.imgui.try_with_texture_mut(id, operation)
            }

            /// Retires a managed texture after outstanding renderer work completes.
            pub fn remove_texture(
                &mut self,
                id: imgui::ManagedTextureId,
            ) -> Result<(), imgui::ManagedTextureError> {
                self.imgui.remove_texture(id)
            }
        }
    };
}

impl_context_resource_access!(EventContext);
impl_context_resource_access!(InitializedContext);
impl_context_resource_access!(PrepareFrameContext);
impl_context_resource_access!(ShutdownContext);

impl ShutdownContext<'_> {
    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }

    #[must_use]
    pub const fn gpu_generation(&self) -> Option<GpuGeneration> {
        self.generation
    }
}

pub struct DockingController {
    pub(crate) flags: DockNodeFlags,
}

pub struct DockingApi<'a> {
    pub(crate) controller: &'a mut DockingController,
}

impl DockingApi<'_> {
    #[must_use]
    pub fn flags(&self) -> DockNodeFlags {
        DockNodeFlags::from_bits_retain(self.controller.flags.bits())
    }

    pub fn set_flags(&mut self, flags: DockNodeFlags) {
        self.controller.flags = flags;
    }
}

pub struct AddOns<'a> {
    #[cfg(feature = "implot")]
    pub(crate) implot: Option<&'a implot::PlotContext>,
    #[cfg(feature = "imnodes")]
    pub(crate) imnodes: Option<&'a imnodes::Context>,
    #[cfg(feature = "implot3d")]
    pub(crate) implot3d: Option<&'a implot3d::Plot3DContext>,
    pub(crate) docking: DockingApi<'a>,
}

impl<'a> AddOns<'a> {
    #[cfg(feature = "implot")]
    #[must_use]
    pub fn implot(&self) -> Option<&'a implot::PlotContext> {
        self.implot
    }

    #[cfg(feature = "imnodes")]
    #[must_use]
    pub fn imnodes(&self) -> Option<&'a imnodes::Context> {
        self.imnodes
    }

    #[cfg(feature = "implot3d")]
    #[must_use]
    pub fn implot3d(&self) -> Option<&'a implot3d::Plot3DContext> {
        self.implot3d
    }

    pub fn docking(&mut self) -> &mut DockingApi<'a> {
        &mut self.docking
    }
}

pub struct GpuApi<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
    pub(crate) generation: GpuGeneration,
}

impl GpuApi<'_> {
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        self.device
    }

    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        self.queue
    }

    #[must_use]
    pub const fn generation(&self) -> GpuGeneration {
        self.generation
    }

    pub fn register_external_texture(
        &mut self,
        view: &wgpu::TextureView,
    ) -> Result<ExternalTextureHandle, ExternalTextureError> {
        Ok(ExternalTextureHandle {
            id: self.renderer.register_external_texture(view)?,
            generation: self.generation,
        })
    }

    pub fn resolve_external_texture(
        &self,
        handle: ExternalTextureHandle,
    ) -> Result<TextureId, ExternalTextureError> {
        handle
            .resolve_for_generation(self.generation)
            .map(dear_imgui_wgpu::ExternalTextureId::texture_id)
    }

    pub fn update_external_texture(
        &mut self,
        handle: ExternalTextureHandle,
        view: &wgpu::TextureView,
    ) -> Result<(), ExternalTextureError> {
        let texture = handle.resolve_for_generation(self.generation)?;
        self.renderer.update_external_texture(texture, view)?;
        Ok(())
    }

    pub fn unregister_external_texture(
        &mut self,
        handle: ExternalTextureHandle,
    ) -> Result<(), ExternalTextureError> {
        let texture = handle.resolve_for_generation(self.generation)?;
        self.renderer.unregister_external_texture(texture)?;
        Ok(())
    }
}

pub struct GpuContext<'a> {
    pub(crate) window: &'a Window,
    pub(crate) surface_config: &'a wgpu::SurfaceConfiguration,
    pub(crate) gpu: GpuApi<'a>,
}

impl<'a> GpuContext<'a> {
    #[must_use]
    pub fn window(&self) -> &Window {
        self.window
    }

    #[must_use]
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        self.surface_config
    }

    pub fn gpu(&mut self) -> &mut GpuApi<'a> {
        &mut self.gpu
    }

    #[must_use]
    pub const fn generation(&self) -> GpuGeneration {
        self.gpu.generation
    }
}

pub struct FrameContext<'a> {
    pub(crate) ui: &'a imgui::Ui,
    pub(crate) addons: AddOns<'a>,
    pub(crate) gpu: GpuApi<'a>,
    pub(crate) exit_requested: &'a mut bool,
}

impl<'a> FrameContext<'a> {
    #[must_use]
    pub fn ui(&self) -> &'a imgui::Ui {
        self.ui
    }

    pub fn addons(&mut self) -> &mut AddOns<'a> {
        &mut self.addons
    }

    pub fn gpu(&mut self) -> &mut GpuApi<'a> {
        &mut self.gpu
    }

    /// Requests normal event-loop exit after the current frame is presented.
    ///
    /// This is a control signal, not an error. If the same callback returns an error, that error
    /// remains primary and shutdown still runs exactly once.
    pub fn request_exit(&mut self) {
        *self.exit_requested = true;
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use thiserror::Error;

    use super::{ApplicationStage, ExternalTextureError, GpuGeneration, RunError};

    #[derive(Debug, Error)]
    #[error("injected user failure")]
    struct UserError;

    #[test]
    fn stale_generation_error_names_both_epochs() {
        let error = ExternalTextureError::StaleGeneration {
            handle_generation: 2,
            current_generation: 3,
        };

        assert_eq!(
            error.to_string(),
            "external texture belongs to GPU generation 2, current generation is 3"
        );
    }

    #[test]
    fn stale_gpu_generation_is_rejected_after_recovery() {
        assert!(matches!(
            GpuGeneration(2).ensure_current(GpuGeneration(3)),
            Err(ExternalTextureError::StaleGeneration {
                handle_generation: 2,
                current_generation: 3,
            })
        ));
        assert!(GpuGeneration(3).ensure_current(GpuGeneration(3)).is_ok());
    }

    #[test]
    fn application_error_retains_typed_stage_and_original_source() {
        let error = RunError::application(ApplicationStage::Frame, UserError);

        assert!(matches!(
            &error,
            RunError::Application {
                stage: ApplicationStage::Frame,
                ..
            }
        ));
        assert!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<UserError>())
                .is_some()
        );
    }

    #[test]
    fn assigning_the_same_stage_does_not_duplicate_the_source_chain() {
        let error = RunError::application(ApplicationStage::Frame, UserError)
            .during_application_stage(ApplicationStage::Frame);

        assert!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<UserError>())
                .is_some()
        );
    }

    #[test]
    fn frame_ownership_error_reports_its_application_stage() {
        let _guard = crate::runtime::imgui_test_guard();
        let context = dear_imgui_rs::Context::create();
        let stamp = context.frame_lifecycle_stamp();
        let error = RunError::ImGuiFrameOwnership {
            stage: ApplicationStage::PrepareFrame,
            before: stamp,
            after: stamp,
        };

        assert_eq!(
            error.application_stage(),
            Some(ApplicationStage::PrepareFrame)
        );
    }

    #[test]
    fn runtime_assigned_stage_retains_the_hook_run_error_as_source() {
        let error = RunError::GpuValidation {
            message: "injected validation failure".to_owned(),
        }
        .during_application_stage(ApplicationStage::GpuRecreated);

        assert!(matches!(
            &error,
            RunError::Application {
                stage: ApplicationStage::GpuRecreated,
                ..
            }
        ));
        assert!(matches!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<RunError>()),
            Some(RunError::GpuValidation { message })
                if message == "injected validation failure"
        ));
    }
}
