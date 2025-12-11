//! dear-app: minimal Dear ImGui app runner for dear-imgui-rs
//!
//! Goals
//! - Hide boilerplate (Winit + WGPU + platform + renderer)
//! - Provide a simple per-frame closure API similar to `immapp::Run`
//! - Optionally initialize add-ons (ImPlot, ImNodes) and expose them to the UI callback
//!
//! Quickstart
//! ```no_run
//! use dear_app::{run_simple};
//! use dear_imgui_rs::*;
//!
//! fn main() {
//!     run_simple(|ui| {
//!         ui.window("Hello")
//!             .size([300.0, 120.0], Condition::FirstUseEver)
//!             .build(|| ui.text("Hello from dear-app!"));
//!     }).unwrap();
//! }
//! ```
use dear_imgui_rs as imgui;
use dear_imgui_rs::{ConfigFlags, DockFlags, Id, WindowFlags};
use dear_imgui_wgpu as imgui_wgpu;
use dear_imgui_winit as imgui_winit;
use pollster::block_on;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{error, info};
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[cfg(feature = "imnodes")]
use dear_imnodes as imnodes;
#[cfg(feature = "implot")]
use dear_implot as implot;
#[cfg(feature = "implot3d")]
use dear_implot3d as implot3d;

#[derive(Debug, Error)]
pub enum DearAppError {
    #[error("WGPU surface lost")]
    SurfaceLost,
    #[error("WGPU surface outdated")]
    SurfaceOutdated,
    #[error("WGPU surface timeout")]
    SurfaceTimeout,
    #[error("WGPU error: {0}")]
    Wgpu(#[from] wgpu::SurfaceError),
    #[error("Window creation error: {0}")]
    WindowCreation(#[from] winit::error::EventLoopError),
    #[error("Generic error: {0}")]
    Generic(String),
}

/// Add-ons to be initialized and provided to the UI callback
#[derive(Default, Clone, Copy)]
pub struct AddOnsConfig {
    pub with_implot: bool,
    pub with_imnodes: bool,
    pub with_implot3d: bool,
}

impl AddOnsConfig {
    /// Enable add-ons that are compiled into this crate via features.
    /// This does not fail if a given add-on is not enabled at compile time;
    /// missing ones are simply ignored during initialization.
    pub fn auto() -> Self {
        Self {
            with_implot: cfg!(feature = "implot"),
            with_imnodes: cfg!(feature = "imnodes"),
            with_implot3d: cfg!(feature = "implot3d"),
        }
    }
}

/// Mutable view to add-ons for per-frame rendering
pub struct AddOns<'a> {
    #[cfg(feature = "implot")]
    pub implot: Option<&'a implot::PlotContext>,
    #[cfg(not(feature = "implot"))]
    pub implot: Option<()>,

    #[cfg(feature = "imnodes")]
    pub imnodes: Option<&'a imnodes::Context>,
    #[cfg(not(feature = "imnodes"))]
    pub imnodes: Option<()>,

    #[cfg(feature = "implot3d")]
    pub implot3d: Option<&'a implot3d::Plot3DContext>,
    #[cfg(not(feature = "implot3d"))]
    pub implot3d: Option<()>,
    pub docking: DockingApi<'a>,
    pub gpu: GpuApi<'a>,
    _marker: PhantomData<&'a ()>,
}

/// Basic runner configuration
pub struct RunnerConfig {
    pub window_title: String,
    pub window_size: (f64, f64),
    pub present_mode: wgpu::PresentMode,
    pub clear_color: [f32; 4],
    pub docking: DockingConfig,
    pub ini_filename: Option<PathBuf>,
    pub restore_previous_geometry: bool,
    pub redraw: RedrawMode,
    /// Optional override for `Io::config_flags` in addition to docking flag.
    /// If `Some`, it will be merged with docking flag; if `None`, only docking is applied.
    pub io_config_flags: Option<ConfigFlags>,
    /// Optional built-in theme to apply at startup (before on_style callback)
    pub theme: Option<Theme>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            window_title: format!("Dear ImGui App - {}", env!("CARGO_PKG_VERSION")),
            window_size: (1280.0, 720.0),
            present_mode: wgpu::PresentMode::Fifo,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            docking: DockingConfig::default(),
            ini_filename: None,
            restore_previous_geometry: true,
            redraw: RedrawMode::Poll,
            io_config_flags: None,
            theme: None,
        }
    }
}

/// Docking configuration
pub struct DockingConfig {
    /// Enable ImGui docking (sets `ConfigFlags::DOCKING_ENABLE`)
    pub enable: bool,
    /// Automatically create a fullscreen host window + dockspace over main viewport
    pub auto_dockspace: bool,
    /// Flags used for the created dockspace
    pub dockspace_flags: DockFlags,
    /// Host window flags (for the fullscreen dockspace host)
    pub host_window_flags: WindowFlags,
    /// Optional host window name (useful to persist ini settings)
    pub host_window_name: &'static str,
}

impl Default for DockingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            auto_dockspace: true,
            dockspace_flags: DockFlags::PASSTHRU_CENTRAL_NODE,
            host_window_flags: WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | WindowFlags::NO_NAV_FOCUS,
            host_window_name: "DockSpaceHost",
        }
    }
}

/// Redraw behavior for the event loop
#[derive(Clone, Copy, Debug)]
pub enum RedrawMode {
    /// Always redraw (ControlFlow::Poll)
    Poll,
    /// On-demand redraw (ControlFlow::Wait)
    Wait,
    /// Redraw at most `fps` per second using WaitUntil
    WaitUntil { fps: f32 },
}

/// Simple built-in themes for convenience
#[derive(Clone, Copy, Debug)]
pub enum Theme {
    Dark,
    Light,
    Classic,
}

fn apply_theme(_ctx: &mut imgui::Context, theme: Theme) {
    // Apply via ImGui global helpers; doesn't require a Ui
    unsafe {
        match theme {
            Theme::Dark => dear_imgui_rs::sys::igStyleColorsDark(std::ptr::null_mut()),
            Theme::Light => dear_imgui_rs::sys::igStyleColorsLight(std::ptr::null_mut()),
            Theme::Classic => dear_imgui_rs::sys::igStyleColorsClassic(std::ptr::null_mut()),
        }
    }
}

/// Runner lifecycle callbacks (all optional)
pub struct RunnerCallbacks {
    pub on_setup: Option<Box<dyn FnMut(&mut imgui::Context)>>,
    pub on_style: Option<Box<dyn FnMut(&mut imgui::Context)>>,
    pub on_fonts: Option<Box<dyn FnMut(&mut imgui::Context)>>,
    pub on_post_init: Option<Box<dyn FnMut(&mut imgui::Context)>>,
    pub on_event:
        Option<Box<dyn FnMut(&winit::event::Event<()>, &Arc<Window>, &mut imgui::Context)>>,
    pub on_exit: Option<Box<dyn FnMut(&mut imgui::Context)>>,
}

impl Default for RunnerCallbacks {
    fn default() -> Self {
        Self {
            on_setup: None,
            on_style: None,
            on_fonts: None,
            on_post_init: None,
            on_event: None,
            on_exit: None,
        }
    }
}

/// App builder for ergonomic configuration
pub struct AppBuilder {
    cfg: RunnerConfig,
    addons: AddOnsConfig,
    cbs: RunnerCallbacks,
    on_frame: Option<Box<dyn FnMut(&imgui::Ui, &mut AddOns) + 'static>>,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            cfg: RunnerConfig::default(),
            addons: AddOnsConfig::default(),
            cbs: RunnerCallbacks::default(),
            on_frame: None,
        }
    }
    pub fn with_config(mut self, cfg: RunnerConfig) -> Self {
        self.cfg = cfg;
        self
    }
    pub fn with_addons(mut self, addons: AddOnsConfig) -> Self {
        self.addons = addons;
        self
    }
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.cfg.theme = Some(theme);
        self
    }
    pub fn on_setup<F: FnMut(&mut imgui::Context) + 'static>(mut self, f: F) -> Self {
        self.cbs.on_setup = Some(Box::new(f));
        self
    }
    pub fn on_style<F: FnMut(&mut imgui::Context) + 'static>(mut self, f: F) -> Self {
        self.cbs.on_style = Some(Box::new(f));
        self
    }
    pub fn on_fonts<F: FnMut(&mut imgui::Context) + 'static>(mut self, f: F) -> Self {
        self.cbs.on_fonts = Some(Box::new(f));
        self
    }
    pub fn on_post_init<F: FnMut(&mut imgui::Context) + 'static>(mut self, f: F) -> Self {
        self.cbs.on_post_init = Some(Box::new(f));
        self
    }
    pub fn on_event<
        F: FnMut(&winit::event::Event<()>, &Arc<Window>, &mut imgui::Context) + 'static,
    >(
        mut self,
        f: F,
    ) -> Self {
        self.cbs.on_event = Some(Box::new(f));
        self
    }
    pub fn on_frame<F: FnMut(&imgui::Ui, &mut AddOns) + 'static>(mut self, f: F) -> Self {
        self.on_frame = Some(Box::new(f));
        self
    }
    pub fn on_exit<F: FnMut(&mut imgui::Context) + 'static>(mut self, f: F) -> Self {
        self.cbs.on_exit = Some(Box::new(f));
        self
    }
    pub fn run(mut self) -> Result<(), DearAppError> {
        let frame_fn = self
            .on_frame
            .take()
            .ok_or_else(|| DearAppError::Generic("on_frame not set in AppBuilder".into()))?;
        run_with_callbacks(self.cfg, self.addons, self.cbs, frame_fn)
    }
}

/// Simple helper to run an app with a per-frame UI callback.
///
/// - Initializes Winit + WGPU + Dear ImGui
/// - Optionally initializes add-ons (ImPlot, ImNodes)
/// - Calls `gui` every frame with `Ui` and available add-ons
pub fn run_simple<F>(mut gui: F) -> Result<(), DearAppError>
where
    F: FnMut(&imgui::Ui) + 'static,
{
    run(
        RunnerConfig::default(),
        AddOnsConfig::default(),
        move |ui, _addons| gui(ui),
    )
}

/// Run an app with configuration and add-ons.
///
/// The `gui` callback is called every frame with access to ImGui `Ui` and the initialized add-ons.
pub fn run<F>(
    runner: RunnerConfig,
    addons_cfg: AddOnsConfig,
    mut gui: F,
) -> Result<(), DearAppError>
where
    F: FnMut(&imgui::Ui, &mut AddOns) + 'static,
{
    run_with_callbacks(runner, addons_cfg, RunnerCallbacks::default(), gui)
}

/// Run with explicit lifecycle callbacks (used by the builder-style API)
pub fn run_with_callbacks<F>(
    runner: RunnerConfig,
    addons_cfg: AddOnsConfig,
    cbs: RunnerCallbacks,
    gui: F,
) -> Result<(), DearAppError>
where
    F: FnMut(&imgui::Ui, &mut AddOns) + 'static,
{
    let event_loop = EventLoop::new()?;
    match runner.redraw {
        RedrawMode::Poll => event_loop.set_control_flow(ControlFlow::Poll),
        RedrawMode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
        RedrawMode::WaitUntil { .. } => event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        )),
    }

    let mut app = App::new(runner, addons_cfg, cbs, gui);
    info!("Starting Dear App event loop");
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct ImguiState {
    context: imgui::Context,
    platform: imgui_winit::WinitPlatform,
    renderer: imgui_wgpu::WgpuRenderer,
    last_frame: Instant,
}

// Runtime docking flags controller
pub struct DockingController {
    flags: DockFlags,
}

pub struct DockingApi<'a> {
    ctrl: &'a mut DockingController,
}

impl<'a> DockingApi<'a> {
    pub fn flags(&self) -> DockFlags {
        DockFlags::from_bits_retain(self.ctrl.flags.bits())
    }
    pub fn set_flags(&mut self, flags: DockFlags) {
        self.ctrl.flags = flags;
    }
}

// Minimal textures API to allow explicit texture updates from UI code
/// GPU access API for real-time scenarios (game view, image browser, atlas editor)
pub struct GpuApi<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    renderer: &'a mut imgui_wgpu::WgpuRenderer,
}

impl<'a> GpuApi<'a> {
    /// Access the WGPU device
    pub fn device(&self) -> &wgpu::Device {
        self.device
    }
    /// Access the default WGPU queue
    pub fn queue(&self) -> &wgpu::Queue {
        self.queue
    }
    /// Register an external texture + view and obtain an ImGui TextureId (u64)
    pub fn register_texture(&mut self, texture: &wgpu::Texture, view: &wgpu::TextureView) -> u64 {
        self.renderer.register_external_texture(texture, view)
    }
    /// Update the view for an existing registered texture
    pub fn update_texture_view(&mut self, tex_id: u64, view: &wgpu::TextureView) -> bool {
        self.renderer.update_external_texture_view(tex_id, view)
    }
    /// Unregister a previously registered texture
    pub fn unregister_texture(&mut self, tex_id: u64) {
        self.renderer.unregister_texture(tex_id)
    }
    /// Optional: directly drive managed TextureData create/update without waiting for draw pass
    pub fn update_texture_data(
        &mut self,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> Result<(), String> {
        let res = self
            .renderer
            .update_texture(texture_data)
            .map_err(|e| format!("update_texture failed: {e}"))?;
        // Important: apply result so TexID/Status are written back
        res.apply_to(texture_data);
        Ok(())
    }
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,

    // add-ons
    #[cfg(feature = "implot")]
    implot_ctx: Option<implot::PlotContext>,
    #[cfg(feature = "imnodes")]
    imnodes_ctx: Option<imnodes::Context>,
    #[cfg(feature = "implot3d")]
    implot3d_ctx: Option<implot3d::Plot3DContext>,

    // config for rendering
    clear_color: wgpu::Color,
    docking_ctrl: DockingController,
}

impl AppWindow {
    fn new(
        event_loop: &ActiveEventLoop,
        cfg: &RunnerConfig,
        addons: &AddOnsConfig,
        cbs: &mut RunnerCallbacks,
    ) -> Result<Self, DearAppError> {
        // WGPU instance and window
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let size = LogicalSize::new(cfg.window_size.0, cfg.window_size.1);
            Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(cfg.window_title.clone())
                            .with_inner_size(size),
                    )
                    .map_err(|e| DearAppError::Generic(format!("Window creation failed: {e}")))?,
            )
        };

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| DearAppError::Generic(format!("Failed to create surface: {e}")))?;

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("No suitable GPU adapter found");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| DearAppError::Generic(format!("request_device failed: {e}")))?;

        // Surface config
        let physical_size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .cloned()
            .find(|f| caps.formats.contains(f))
            .unwrap_or(caps.formats[0]);

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: cfg.present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // ImGui setup
        let mut context = imgui::Context::create();
        // ini setup before fonts
        if let Some(p) = &cfg.ini_filename {
            let _ = context.set_ini_filename(Some(p.clone()));
        } else {
            let _ = context.set_ini_filename(None::<String>);
        }

        // lifecycle: on_setup/style/fonts before renderer init
        if let Some(cb) = cbs.on_setup.as_mut() {
            cb(&mut context);
        }
        // Apply optional theme from config before user style tweak
        if let Some(theme) = cfg.theme {
            apply_theme(&mut context, theme);
        }
        if let Some(cb) = cbs.on_style.as_mut() {
            cb(&mut context);
        }
        if let Some(cb) = cbs.on_fonts.as_mut() {
            cb(&mut context);
        }

        let mut platform = imgui_winit::WinitPlatform::new(&mut context);
        platform.attach_window(&window, imgui_winit::HiDpiMode::Default, &mut context);

        let init_info =
            imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = imgui_wgpu::WgpuRenderer::new(init_info, &mut context)
            .map_err(|e| DearAppError::Generic(format!("Failed to init renderer: {e}")))?;
        renderer.set_gamma_mode(imgui_wgpu::GammaMode::Auto);

        // Configure IO flags & docking (never enable multi-viewport here)
        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            if cfg.docking.enable {
                flags.insert(ConfigFlags::DOCKING_ENABLE);
            }
            if let Some(extra) = &cfg.io_config_flags {
                let merged = flags.bits() | extra.bits();
                flags = ConfigFlags::from_bits_retain(merged);
            }
            io.set_config_flags(flags);
        }

        #[cfg(feature = "implot")]
        let implot_ctx = if addons.with_implot {
            Some(implot::PlotContext::create(&context))
        } else {
            None
        };

        #[cfg(feature = "imnodes")]
        let imnodes_ctx = if addons.with_imnodes {
            Some(imnodes::Context::create(&context))
        } else {
            None
        };

        #[cfg(feature = "implot3d")]
        let implot3d_ctx = if addons.with_implot3d {
            Some(implot3d::Plot3DContext::create(&context))
        } else {
            None
        };

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            #[cfg(feature = "implot")]
            implot_ctx,
            #[cfg(feature = "imnodes")]
            imnodes_ctx,
            #[cfg(feature = "implot3d")]
            implot3d_ctx,
            clear_color: wgpu::Color {
                r: cfg.clear_color[0] as f64,
                g: cfg.clear_color[1] as f64,
                b: cfg.clear_color[2] as f64,
                a: cfg.clear_color[3] as f64,
            },
            docking_ctrl: DockingController {
                flags: DockFlags::from_bits_retain(cfg.docking.dockspace_flags.bits()),
            },
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render<F>(&mut self, gui: &mut F, docking: &DockingConfig) -> Result<(), DearAppError>
    where
        F: FnMut(&imgui::Ui, &mut AddOns),
    {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            Err(SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(e) => return Err(DearAppError::from(e)),
        };

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Optional fullscreen dockspace
        if docking.enable && docking.auto_dockspace {
            let viewport = ui.main_viewport();
            // Host window always covering the main viewport
            ui.set_next_window_viewport(Id::from(viewport.id()));
            let pos = viewport.pos();
            let size = viewport.size();
            // NO_BACKGROUND if passthru central node
            let current_flags = DockFlags::from_bits_retain(self.docking_ctrl.flags.bits());
            let mut win_flags = docking.host_window_flags;
            if current_flags.contains(DockFlags::PASSTHRU_CENTRAL_NODE) {
                win_flags |= WindowFlags::NO_BACKGROUND;
            }
            ui.window(docking.host_window_name)
                .flags(win_flags)
                .position([pos[0], pos[1]], imgui::Condition::Always)
                .size([size[0], size[1]], imgui::Condition::Always)
                .build(|| {
                    let ds_flags = DockFlags::from_bits_retain(current_flags.bits());
                    let _ = ui.dockspace_over_main_viewport_with_flags(Id::from(0u32), ds_flags);
                });
        }

        // Build add-ons view
        let mut addons = AddOns {
            #[cfg(feature = "implot")]
            implot: self.implot_ctx.as_ref(),
            #[cfg(not(feature = "implot"))]
            implot: None,
            #[cfg(feature = "imnodes")]
            imnodes: self.imnodes_ctx.as_ref(),
            #[cfg(not(feature = "imnodes"))]
            imnodes: None,
            #[cfg(feature = "implot3d")]
            implot3d: self.implot3d_ctx.as_ref(),
            #[cfg(not(feature = "implot3d"))]
            implot3d: None,
            docking: DockingApi {
                ctrl: &mut self.docking_ctrl,
            },
            gpu: GpuApi {
                device: &self.device,
                queue: &self.queue,
                renderer: &mut self.imgui.renderer,
            },
            _marker: PhantomData,
        };

        // Call user GUI
        gui(&ui, &mut addons);

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let draw_data = self.imgui.context.render();

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.imgui
                .renderer
                .new_frame()
                .map_err(|e| DearAppError::Generic(format!("new_frame failed: {e}")))?;
            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut rpass)
                .map_err(|e| DearAppError::Generic(format!("render_draw_data failed: {e}")))?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

struct App<F>
where
    F: FnMut(&imgui::Ui, &mut AddOns) + 'static,
{
    cfg: RunnerConfig,
    addons_cfg: AddOnsConfig,
    window: Option<AppWindow>,
    gui: F,
    cbs: RunnerCallbacks,
    last_wake: Instant,
}

impl<F> App<F>
where
    F: FnMut(&imgui::Ui, &mut AddOns) + 'static,
{
    fn new(cfg: RunnerConfig, addons_cfg: AddOnsConfig, cbs: RunnerCallbacks, gui: F) -> Self {
        Self {
            cfg,
            addons_cfg,
            window: None,
            gui,
            cbs,
            last_wake: Instant::now(),
        }
    }
}

impl<F> ApplicationHandler for App<F>
where
    F: FnMut(&imgui::Ui, &mut AddOns) + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop, &self.cfg, &self.addons_cfg, &mut self.cbs) {
                Ok(window) => {
                    self.window = Some(window);
                    info!("Window created successfully");
                    if let Some(cb) = self.cbs.on_post_init.as_mut() {
                        if let Some(w) = self.window.as_mut() {
                            cb(&mut w.imgui.context);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // We may recreate the window/gpu stack on fatal GPU errors, so we avoid
        // holding a mutable borrow of self.window across the whole match.
        match event {
            WindowEvent::RedrawRequested => {
                // Render and, on fatal errors, attempt a full GPU/window rebuild.
                let mut need_recreate = false;
                if let Some(window) = self.window.as_mut() {
                    let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                        window_id,
                        event: event.clone(),
                    };
                    if let Some(cb) = self.cbs.on_event.as_mut() {
                        cb(&full_event, &window.window, &mut window.imgui.context);
                    }
                    window.imgui.platform.handle_event(
                        &mut window.imgui.context,
                        &window.window,
                        &full_event,
                    );

                    if let Err(e) = window.render(&mut self.gui, &self.cfg.docking) {
                        error!("Render error: {e}; attempting to recover by recreating GPU state");
                        need_recreate = true;
                    } else {
                        window.window.request_redraw();
                    }
                }

                if need_recreate {
                    // Drop the existing window and try to rebuild the whole stack.
                    self.window = None;
                    if let Err(e) =
                        AppWindow::new(event_loop, &self.cfg, &self.addons_cfg, &mut self.cbs)
                    {
                        error!("Failed to recreate window after GPU error: {e}");
                        if let Some(cb) = self.cbs.on_exit.as_mut() {
                            // Best-effort: give user a chance to clean up the old context if any.
                            if let Some(w) = self.window.as_mut() {
                                cb(&mut w.imgui.context);
                            }
                        }
                        event_loop.exit();
                    } else if let Some(window) = self.window.as_mut() {
                        info!("Successfully recreated window and GPU state after error");
                        if let Some(cb) = self.cbs.on_post_init.as_mut() {
                            cb(&mut window.imgui.context);
                        }
                        window.window.request_redraw();
                    }
                }
            }
            _ => {
                let window = match self.window.as_mut() {
                    Some(window) => window,
                    None => return,
                };

                let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                    window_id,
                    event: event.clone(),
                };
                if let Some(cb) = self.cbs.on_event.as_mut() {
                    cb(&full_event, &window.window, &mut window.imgui.context);
                }
                window.imgui.platform.handle_event(
                    &mut window.imgui.context,
                    &window.window,
                    &full_event,
                );

                match event {
                    WindowEvent::Resized(physical_size) => {
                        window.resize(physical_size);
                        window.window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        let new_size = window.window.inner_size();
                        window.resize(new_size);
                        window.window.request_redraw();
                    }
                    WindowEvent::CloseRequested => {
                        if let Some(cb) = self.cbs.on_exit.as_mut() {
                            if let Some(w) = self.window.as_mut() {
                                cb(&mut w.imgui.context);
                            }
                        }
                        event_loop.exit();
                    }
                    _ => {}
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            match self.cfg.redraw {
                RedrawMode::Poll => {
                    window.window.request_redraw();
                }
                RedrawMode::Wait => {
                    // On-demand: redraw only on events; still keep UI alive
                }
                RedrawMode::WaitUntil { fps } => {
                    let frame = (1.0f32 / fps.max(1.0)) as f32;
                    if self.last_wake.elapsed() >= Duration::from_secs_f32(frame) {
                        window.window.request_redraw();
                        self.last_wake = Instant::now();
                    }
                }
            }
        }
    }
}
