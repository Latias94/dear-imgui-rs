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
    AppConfig, Application, GpuApi, GpuContext, GpuGeneration, InitContext, RunError,
    ShutdownContext, Theme,
};

use super::recovery::OwnedGpuGeneration;

#[cfg(feature = "imnodes")]
use dear_imnodes as imnodes;
#[cfg(feature = "implot")]
use dear_implot as implot;
#[cfg(feature = "implot3d")]
use dear_implot3d as implot3d;

#[derive(Debug)]
pub(crate) enum RuntimeEvent {
    DeviceLost {
        generation: GpuGeneration,
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
        let mut context = imgui::Context::try_create().map_err(RunError::ImGuiContext)?;
        configure_context(&mut context, config);
        {
            let mut init = InitContext {
                imgui: &mut context,
                window: &window.window,
                config,
            };
            if let Err(error) = application.configure_imgui(&mut init) {
                let mut shutdown = ShutdownContext {
                    imgui: init.imgui,
                    window: init.window,
                    generation: None,
                };
                let _ = application.shutdown(&mut shutdown);
                return Err(error);
            }
        }

        let mut platform = dear_imgui_winit::WinitPlatform::new(&mut context);
        platform.attach_window(
            &window.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

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

    pub(crate) fn teardown(self) {
        #[cfg(feature = "implot3d")]
        drop(self.implot3d);
        #[cfg(feature = "imnodes")]
        drop(self.imnodes);
        #[cfg(feature = "implot")]
        drop(self.implot);
        drop(self.context);
    }
}

pub(crate) struct GpuState {
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) renderer: dear_imgui_wgpu::WgpuRenderer,
}

impl GpuState {
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
