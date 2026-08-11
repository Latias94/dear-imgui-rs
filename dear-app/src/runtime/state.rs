use std::sync::Arc;

use dear_imgui_rs as imgui;
use dear_imgui_rs::ConfigFlags;
use pollster::block_on;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::Window,
};

use crate::application::DockingController;
use crate::{
    AppConfig, Application, ApplicationStage, GpuApi, GpuContext, GpuGeneration, InitContext,
    RunError, ShutdownContext, Theme,
};

use super::recovery::OwnedGpuGeneration;

pub(crate) fn platform_error(
    operation: &'static str,
    error: dear_imgui_winit::WinitPlatformError,
) -> RunError {
    RunError::Platform {
        operation,
        source: error,
    }
}

pub(crate) fn run_application_frame_boundary<T>(
    context: &mut imgui::Context,
    stage: ApplicationStage,
    callback: impl FnOnce(&mut imgui::Context) -> Result<T, RunError>,
) -> Result<T, RunError> {
    let before = context.frame_lifecycle_stamp();
    let result = callback(context);
    let result = result.map_err(|error| error.during_application_stage(stage));
    let after = context.frame_lifecycle_stamp();
    if after == before {
        return result;
    }

    if after.state() == imgui::FrameLifecycleState::InFrame {
        context.end_frame();
    }
    let ownership_error = RunError::ImGuiFrameOwnership {
        stage,
        before,
        after,
    };
    match result {
        Ok(_) => Err(ownership_error),
        Err(primary) => {
            tracing::warn!(
                %primary,
                secondary = %ownership_error,
                "application callback failed after changing Dear ImGui frame ownership"
            );
            Err(primary)
        }
    }
}

pub(crate) fn run_application_shutdown<A: Application>(
    application: &mut A,
    context: &mut imgui::Context,
    window: &Window,
    generation: Option<GpuGeneration>,
) -> Result<(), RunError> {
    run_application_frame_boundary(context, ApplicationStage::Shutdown, |imgui| {
        let mut shutdown = ShutdownContext {
            imgui,
            window,
            generation,
        };
        application.shutdown(&mut shutdown)
    })
}

pub(crate) fn preserve_initialization_error(
    primary_error: RunError,
    cleanup: impl FnOnce() -> Result<(), RunError>,
) -> RunError {
    if let Err(cleanup_error) = cleanup() {
        tracing::warn!(
            primary = %primary_error,
            cleanup = %cleanup_error,
            "Dear App initialization cleanup failed; preserving the original initialization error"
        );
    }
    primary_error
}

fn shutdown_after_initialization_failure<A: Application>(
    application: &mut A,
    context: &mut imgui::Context,
    window: &Window,
) -> Result<(), RunError> {
    run_application_shutdown(application, context, window, None)
}

fn abort_context_initialization<A: Application>(
    application: &mut A,
    mut context: imgui::Context,
    window: &Window,
    primary_error: RunError,
) -> RunError {
    preserve_initialization_error(primary_error, || {
        shutdown_after_initialization_failure(application, &mut context, window)
    })
}

fn abort_platform_initialization<A: Application>(
    application: &mut A,
    mut context: imgui::Context,
    window: &Window,
    mut platform: dear_imgui_winit::WinitPlatform,
    primary_error: RunError,
) -> RunError {
    preserve_initialization_error(primary_error, move || {
        let application_result =
            shutdown_after_initialization_failure(application, &mut context, window);
        let platform_result = platform.shutdown(&mut context).map_err(|error| {
            platform_error("Winit platform cleanup after initialization failure", error)
        });

        match platform_result {
            Ok(()) => {
                drop(platform);
                drop(context);
                application_result
            }
            Err(error) => {
                // A failed explicit platform release leaves Context attachment state uncertain.
                // Quarantine both values rather than allowing Context drop to invoke the fallback.
                std::mem::forget(platform);
                std::mem::forget(context);
                application_result.and(Err(error))
            }
        }
    })
}

/// Completes the fallible portion of UI initialization after Context configuration.
///
/// The two cleanup callbacks consume every value that was successfully published before the
/// failing operation. Keeping those ownership transfers in one place makes it impossible for a
/// later platform-construction failure to bypass the application shutdown hook.
fn initialize_configured_ui<A, P>(
    application: &mut A,
    context: imgui::Context,
    configure: impl FnOnce(&mut A, &mut imgui::Context) -> Result<(), RunError>,
    create_platform: impl FnOnce(&mut A, &mut imgui::Context) -> Result<P, RunError>,
    attach_window: impl FnOnce(&mut A, &mut P, &mut imgui::Context) -> Result<(), RunError>,
    abort_without_platform: impl FnOnce(&mut A, imgui::Context, RunError) -> RunError,
    abort_with_platform: impl FnOnce(&mut A, imgui::Context, P, RunError) -> RunError,
) -> Result<(imgui::Context, P), RunError> {
    let mut context = context;
    if let Err(error) = configure(application, &mut context) {
        return Err(abort_without_platform(application, context, error));
    }

    let mut platform = match create_platform(application, &mut context) {
        Ok(platform) => platform,
        Err(error) => return Err(abort_without_platform(application, context, error)),
    };

    if let Err(error) = attach_window(application, &mut platform, &mut context) {
        return Err(abort_with_platform(application, context, platform, error));
    }

    Ok((context, platform))
}

pub(crate) fn validate_supported_imgui_config(context: &imgui::Context) -> Result<(), RunError> {
    if context
        .io()
        .config_flags()
        .contains(ConfigFlags::VIEWPORTS_ENABLE)
    {
        return Err(RunError::MultiViewportUnsupported);
    }
    Ok(())
}

#[cfg(feature = "imnodes")]
use dear_imnodes as imnodes;
#[cfg(feature = "implot")]
use dear_implot as implot;
#[cfg(feature = "implot3d")]
use dear_implot3d as implot3d;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GpuFaultKind {
    OutOfMemory,
    Validation,
    Internal,
}

#[derive(Debug)]
pub(crate) enum RuntimeEvent {
    DeviceLost {
        generation: GpuGeneration,
        message: String,
    },
    GpuFault {
        generation: GpuGeneration,
        kind: GpuFaultKind,
        message: String,
    },
}

pub(crate) struct WindowState {
    instance: wgpu::Instance,
    pub(crate) window: Arc<Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) surface_config: Option<wgpu::SurfaceConfiguration>,
}

impl WindowState {
    pub(crate) fn new(event_loop: &ActiveEventLoop, config: &AppConfig) -> Result<Self, RunError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: config.wgpu.backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let size = LogicalSize::new(config.window_size.0, config.window_size.1);
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(config.window_title.clone())
                        .with_inner_size(size),
                )
                .map_err(RunError::WindowCreation)?,
        );
        let surface = instance
            .create_surface(window.clone())
            .map_err(RunError::SurfaceCreation)?;

        Ok(Self {
            instance,
            window,
            surface,
            surface_config: None,
        })
    }

    pub(crate) fn configure_surface(
        &mut self,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        config: &AppConfig,
    ) -> Result<wgpu::TextureFormat, RunError> {
        let capabilities = self.surface.get_capabilities(adapter);
        let current_format = self.surface_config.as_ref().map(|config| config.format);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = current_format
            .filter(|format| capabilities.formats.contains(format))
            .or_else(|| {
                preferred_srgb
                    .iter()
                    .copied()
                    .find(|format| capabilities.formats.contains(format))
            })
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(RunError::SurfaceFormatUnavailable)?;
        let size = self.window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: config.present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        self.surface.configure(device, &surface_config);
        self.surface_config = Some(surface_config);
        Ok(format)
    }

    pub(crate) fn reconfigure(&self, device: &wgpu::Device) {
        if let Some(surface_config) = self.surface_config.as_ref() {
            self.surface.configure(device, surface_config);
        }
    }

    pub(crate) fn recreate_surface(
        &mut self,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        config: &AppConfig,
    ) -> Result<(), RunError> {
        let previous_format = self.surface_config().map(|config| config.format)?;
        self.surface = self
            .instance
            .create_surface(self.window.clone())
            .map_err(RunError::SurfaceCreation)?;
        let replacement_format = self.configure_surface(adapter, device, config)?;
        if replacement_format != previous_format {
            return Err(RunError::SurfaceFormatChanged {
                previous: previous_format,
                replacement: replacement_format,
            });
        }
        Ok(())
    }

    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>, device: &wgpu::Device) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if let Some(surface_config) = self.surface_config.as_mut() {
            surface_config.width = size.width;
            surface_config.height = size.height;
            self.surface.configure(device, surface_config);
        }
    }

    pub(crate) fn surface_config(&self) -> Result<&wgpu::SurfaceConfiguration, RunError> {
        self.surface_config
            .as_ref()
            .ok_or_else(|| RunError::Recovery {
                message: "GPU context requested before surface configuration".to_owned(),
            })
    }
}

pub(crate) struct UiState {
    pub(crate) context: imgui::Context,
    pub(crate) platform: dear_imgui_winit::WinitPlatform,
    #[cfg(feature = "implot")]
    pub(crate) implot: Option<implot::PlotContext>,
    #[cfg(feature = "imnodes")]
    pub(crate) imnodes: Option<imnodes::Context>,
    #[cfg(feature = "implot3d")]
    pub(crate) implot3d: Option<implot3d::Plot3DContext>,
    pub(crate) docking: DockingController,
}

impl UiState {
    pub(crate) fn new<A: Application>(
        window: &WindowState,
        config: &AppConfig,
        application: &mut A,
    ) -> Result<Self, RunError> {
        let context = imgui::Context::try_create().map_err(RunError::ImGuiContext)?;
        let (context, platform) = initialize_configured_ui(
            application,
            context,
            |application, context| {
                configure_context(context, config);
                run_application_frame_boundary(
                    context,
                    ApplicationStage::ConfigureImgui,
                    |context| {
                        let mut init = InitContext {
                            imgui: context,
                            window: &window.window,
                            config,
                        };
                        application.configure_imgui(&mut init)
                    },
                )?;
                validate_supported_imgui_config(context)
            },
            |_, context| {
                dear_imgui_winit::WinitPlatform::new(context)
                    .map_err(|error| platform_error("Winit platform initialization", error))
            },
            |_, platform, context| {
                platform
                    .attach_window(
                        Arc::clone(&window.window),
                        dear_imgui_winit::HiDpiMode::Default,
                        context,
                    )
                    .map_err(|error| platform_error("Winit main-window attachment", error))
            },
            |application, context, error| {
                abort_context_initialization(application, context, &window.window, error)
            },
            |application, context, platform, error| {
                abort_platform_initialization(application, context, &window.window, platform, error)
            },
        )?;

        #[cfg(feature = "implot")]
        let implot = config
            .addons
            .with_implot
            .then(|| implot::PlotContext::create(&context));
        #[cfg(feature = "imnodes")]
        let imnodes = config
            .addons
            .with_imnodes
            .then(|| imnodes::Context::create(&context));
        #[cfg(feature = "implot3d")]
        let implot3d = config
            .addons
            .with_implot3d
            .then(|| implot3d::Plot3DContext::create(&context));

        Ok(Self {
            context,
            platform,
            #[cfg(feature = "implot")]
            implot,
            #[cfg(feature = "imnodes")]
            imnodes,
            #[cfg(feature = "implot3d")]
            implot3d,
            docking: DockingController {
                flags: config.docking.dockspace_flags(),
            },
        })
    }

    pub(crate) fn release_platform(&mut self) -> Result<(), RunError> {
        self.platform
            .shutdown(&mut self.context)
            .map_err(|error| platform_error("Winit platform shutdown", error))
    }

    pub(crate) fn release_platform_then_teardown_or_quarantine(mut self) -> Result<(), RunError> {
        if let Err(error) = self.release_platform() {
            // Platform shutdown can report an ownership conflict after partially observing Context
            // state. Keep the complete graph alive rather than falling back to Context drop.
            std::mem::forget(self);
            return Err(error);
        }
        self.teardown_after_platform_release();
        Ok(())
    }

    pub(crate) fn teardown_after_platform_release(self) {
        let Self {
            context,
            platform,
            #[cfg(feature = "implot")]
            implot,
            #[cfg(feature = "imnodes")]
            imnodes,
            #[cfg(feature = "implot3d")]
            implot3d,
            docking: _,
        } = self;
        drop(platform);
        #[cfg(feature = "implot3d")]
        drop(implot3d);
        #[cfg(feature = "imnodes")]
        drop(imnodes);
        #[cfg(feature = "implot")]
        drop(implot);
        drop(context);
    }
}

pub(crate) struct GpuState {
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) renderer: dear_imgui_wgpu::WgpuRenderer,
}

impl GpuState {
    pub(crate) fn release_renderer(
        &mut self,
        context: &mut imgui::Context,
    ) -> Result<(), RunError> {
        self.renderer
            .shutdown(context)
            .map_err(RunError::RendererRelease)
    }

    pub(crate) fn teardown(self) {
        let Self {
            adapter,
            device,
            queue,
            renderer,
        } = self;
        drop(renderer);
        device.destroy();
        drop(queue);
        drop(device);
        drop(adapter);
    }
}

pub(crate) struct RuntimeGeneration {
    pub(crate) id: GpuGeneration,
    pub(crate) gpu: GpuState,
}

impl RuntimeGeneration {
    pub(crate) fn api(&mut self) -> GpuApi<'_> {
        GpuApi {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            renderer: &mut self.gpu.renderer,
            generation: self.id,
        }
    }

    pub(crate) fn context<'a>(
        &'a mut self,
        window: &'a WindowState,
    ) -> Result<GpuContext<'a>, RunError> {
        Ok(GpuContext {
            window: &window.window,
            surface_config: window.surface_config()?,
            gpu: self.api(),
        })
    }

    pub(crate) fn teardown(self) {
        self.gpu.teardown();
    }
}

impl OwnedGpuGeneration for RuntimeGeneration {
    fn generation(&self) -> GpuGeneration {
        self.id
    }

    fn teardown(self) {
        RuntimeGeneration::teardown(self);
    }
}

pub(crate) struct WgpuRuntimeFactory;

impl WgpuRuntimeFactory {
    pub(crate) fn create(
        window: &mut WindowState,
        ui: &mut UiState,
        config: &AppConfig,
        event_proxy: EventLoopProxy<RuntimeEvent>,
        generation: GpuGeneration,
    ) -> Result<RuntimeGeneration, RunError> {
        let adapter = block_on(
            window
                .instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: config.wgpu.power_preference,
                    compatible_surface: Some(&window.surface),
                    apply_limit_buckets: false,
                    force_fallback_adapter: config.wgpu.force_fallback_adapter,
                }),
        )
        .map_err(RunError::AdapterUnavailable)?;
        let descriptor = wgpu::DeviceDescriptor {
            label: config.wgpu.device_label.as_deref(),
            required_features: config.wgpu.required_features,
            required_limits: config.wgpu.required_limits.clone(),
            memory_hints: config.wgpu.memory_hints.clone(),
            ..Default::default()
        };
        let (device, queue) =
            block_on(adapter.request_device(&descriptor)).map_err(RunError::DeviceRequest)?;

        let fault_proxy = event_proxy.clone();
        device.on_uncaptured_error(std::sync::Arc::new(move |error| {
            let kind = match &error {
                wgpu::Error::OutOfMemory { .. } => GpuFaultKind::OutOfMemory,
                wgpu::Error::Validation { .. } => GpuFaultKind::Validation,
                wgpu::Error::Internal { .. } => GpuFaultKind::Internal,
            };
            let _ = fault_proxy.send_event(RuntimeEvent::GpuFault {
                generation,
                kind,
                message: error.to_string(),
            });
        }));
        device.set_device_lost_callback(move |reason, message| {
            let _ = event_proxy.send_event(RuntimeEvent::DeviceLost {
                generation,
                message: format!("{reason:?}: {message}"),
            });
        });

        let format = window.configure_surface(&adapter, &device, config)?;
        let init_info = dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), format);
        let mut renderer = dear_imgui_wgpu::WgpuRenderer::new(init_info, &mut ui.context)
            .map_err(RunError::RendererInit)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        Ok(RuntimeGeneration {
            id: generation,
            gpu: GpuState {
                adapter,
                device,
                queue,
                renderer,
            },
        })
    }
}

fn configure_context(context: &mut imgui::Context, config: &AppConfig) {
    if config.restore_previous_geometry {
        let _ = context.set_ini_filename(config.ini_filename.clone());
    } else {
        let _ = context.set_ini_filename(None::<String>);
    }

    if let Some(theme) = config.theme {
        let preset = match theme {
            Theme::Dark => imgui::ThemePreset::Dark,
            Theme::Light => imgui::ThemePreset::Light,
            Theme::Classic => imgui::ThemePreset::Classic,
        };
        let theme = imgui::Theme {
            preset,
            ..Default::default()
        };
        theme.apply_to_context(context);
    }

    let io = context.io_mut();
    let mut flags = io.config_flags();
    if config.docking.is_enabled() {
        flags.insert(ConfigFlags::DOCKING_ENABLE);
    }
    if let Some(extra) = config.io_config_flags {
        flags = ConfigFlags::from_bits_retain(flags.bits() | extra.bits());
    }
    io.set_config_flags(flags);
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use dear_imgui_rs::{FrameLifecycleState, FramePrepareOptions};

    use super::{
        initialize_configured_ui, platform_error, preserve_initialization_error,
        run_application_frame_boundary,
    };
    use crate::{ApplicationStage, RunError};

    #[derive(Debug)]
    struct TestPlatform(Rc<Cell<usize>>);

    impl Drop for TestPlatform {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[derive(Default)]
    struct InitializationProbe {
        configured: usize,
        shutdown_calls: usize,
    }

    fn prepared_context() -> (
        dear_imgui_rs::Context,
        dear_imgui_rs::render::SynchronousRendererConsumer,
    ) {
        let mut context = dear_imgui_rs::Context::create();
        context.prepare_frame(
            FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let consumer = context
            .create_synchronous_renderer_consumer()
            .expect("the test renderer consumer must attach");
        (context, consumer)
    }

    fn leak_open_frame(context: &mut dear_imgui_rs::Context) {
        let frame = context.begin_frame();
        std::mem::forget(frame);
    }

    #[test]
    fn application_boundary_rejects_and_closes_a_leaked_frame() {
        let _guard = crate::runtime::imgui_test_guard();
        let (mut context, _consumer) = prepared_context();
        let error = run_application_frame_boundary(
            &mut context,
            ApplicationStage::PrepareFrame,
            |context| {
                leak_open_frame(context);
                Ok(())
            },
        )
        .expect_err("frame ownership changes must be rejected");

        assert!(matches!(
            error,
            RunError::ImGuiFrameOwnership {
                stage: ApplicationStage::PrepareFrame,
                before,
                after,
            } if before.state() == FrameLifecycleState::Idle
                && after.state() == FrameLifecycleState::InFrame
        ));
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
    }

    #[test]
    fn application_boundary_rejects_an_idle_to_idle_frame_round_trip() {
        let _guard = crate::runtime::imgui_test_guard();
        let (mut context, _consumer) = prepared_context();
        let error = run_application_frame_boundary(
            &mut context,
            ApplicationStage::ConfigureImgui,
            |context| {
                drop(context.begin_frame());
                Ok(())
            },
        )
        .expect_err("opening and ending a frame inside a callback must remain observable");

        assert!(matches!(
            error,
            RunError::ImGuiFrameOwnership { before, after, .. }
                if before.state() == FrameLifecycleState::Idle
                    && after.state() == FrameLifecycleState::Idle
                    && before.context_id() == after.context_id()
                    && before != after
        ));
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
    }

    #[test]
    fn application_boundary_preserves_the_callback_error_and_closes_a_leaked_frame() {
        let _guard = crate::runtime::imgui_test_guard();
        let (mut context, _consumer) = prepared_context();
        let error = run_application_frame_boundary(
            &mut context,
            ApplicationStage::ConfigureImgui,
            |context| {
                leak_open_frame(context);
                Err::<(), _>(RunError::application_message(
                    ApplicationStage::ConfigureImgui,
                    "primary failure",
                ))
            },
        )
        .expect_err("the original callback failure must remain primary");

        assert_eq!(
            error.application_stage(),
            Some(ApplicationStage::ConfigureImgui)
        );
        assert_eq!(
            error.to_string(),
            "application callback failed during configure_imgui: primary failure"
        );
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
    }

    #[test]
    fn initialization_cleanup_runs_once_and_preserves_the_primary_error() {
        let cleanup_calls = Cell::new(0);
        let primary =
            RunError::application_message(ApplicationStage::ConfigureImgui, "primary failure");

        let error = preserve_initialization_error(primary, || {
            cleanup_calls.set(cleanup_calls.get() + 1);
            Err(RunError::application_message(
                ApplicationStage::Shutdown,
                "cleanup failure",
            ))
        });

        assert_eq!(cleanup_calls.get(), 1);
        assert_eq!(
            error.to_string(),
            "application callback failed during configure_imgui: primary failure"
        );
    }

    #[test]
    fn successful_initialization_does_not_run_cleanup() {
        let cleanup_calls = Cell::new(0);
        let value = 41_u8;

        let result = Ok(value).map_err(|error: RunError| {
            preserve_initialization_error(error, || {
                cleanup_calls.set(cleanup_calls.get() + 1);
                Ok(())
            })
        });

        assert!(matches!(result, Ok(actual) if actual == value));
        assert_eq!(cleanup_calls.get(), 0);
    }

    #[test]
    fn platform_constructor_failure_after_configuration_runs_shutdown_once() {
        let _guard = crate::runtime::imgui_test_guard();
        let mut probe = InitializationProbe::default();
        let mut context = dear_imgui_rs::Context::create();
        let mut existing_platform = Some(
            dear_imgui_winit::WinitPlatform::new(&mut context)
                .expect("the first platform attachment must succeed"),
        );

        let result = initialize_configured_ui(
            &mut probe,
            context,
            |state, _| {
                state.configured += 1;
                Ok(())
            },
            |_, context| {
                dear_imgui_winit::WinitPlatform::new(context)
                    .map_err(|error| platform_error("Winit platform initialization", error))
            },
            |_, _, _| unreachable!("attach must not run after constructor failure"),
            |state, mut context, primary| {
                preserve_initialization_error(primary, || {
                    state.shutdown_calls += 1;
                    existing_platform
                        .take()
                        .expect("the original platform must still own its attachment")
                        .shutdown(&mut context)
                        .expect("the original platform shutdown must succeed");
                    drop(context);
                    Err(RunError::application_message(
                        ApplicationStage::Shutdown,
                        "injected cleanup failure",
                    ))
                })
            },
            |_, _, _, _| unreachable!("platform cleanup requires a constructed platform"),
        );
        let error = result
            .err()
            .expect("the second platform attachment must be rejected");

        assert_eq!(probe.configured, 1);
        assert_eq!(probe.shutdown_calls, 1);
        assert!(matches!(
            error,
            RunError::Platform {
                operation: "Winit platform initialization",
                ..
            }
        ));
    }

    #[test]
    fn main_window_attachment_failure_after_configuration_runs_shutdown_once() {
        let _guard = crate::runtime::imgui_test_guard();
        let dropped_platforms = Rc::new(Cell::new(0));
        let mut probe = InitializationProbe::default();

        let error = initialize_configured_ui(
            &mut probe,
            dear_imgui_rs::Context::create(),
            |state, _| {
                state.configured += 1;
                Ok(())
            },
            |_, _| Ok(TestPlatform(Rc::clone(&dropped_platforms))),
            |_, _, _| {
                Err(RunError::Recovery {
                    message: "injected attachment failure".to_owned(),
                })
            },
            |_, _, _| unreachable!("constructor cleanup must not run after attachment failure"),
            |state, context, platform, primary| {
                preserve_initialization_error(primary, || {
                    state.shutdown_calls += 1;
                    drop(platform);
                    drop(context);
                    Err(RunError::application_message(
                        ApplicationStage::Shutdown,
                        "injected cleanup failure",
                    ))
                })
            },
        )
        .expect_err("the attachment failure must be reported");

        assert_eq!(probe.configured, 1);
        assert_eq!(probe.shutdown_calls, 1);
        assert_eq!(dropped_platforms.get(), 1);
        assert_eq!(
            error.to_string(),
            "GPU generation recovery failed: injected attachment failure"
        );
    }
}
