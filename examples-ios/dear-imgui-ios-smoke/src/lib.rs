use std::sync::Arc;
use std::time::Instant;

use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(target_os = "ios")]
use winit::platform::ios::{ScreenEdge, ValidOrientations, WindowAttributesExtIOS, WindowExtIOS};
use winit::window::{Window, WindowId};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
    show_demo_window: bool,
    touch_label: String,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    clear_color: wgpu::Color,
    tap_counter: u32,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let window_attributes = Window::default_attributes()
            .with_title("Dear ImGui iOS Smoke")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

        #[cfg(not(target_os = "ios"))]
        let window_attributes = window_attributes;

        #[cfg(target_os = "ios")]
        let window_attributes = window_attributes
            .with_valid_orientations(ValidOrientations::LandscapeAndPortrait)
            .with_prefers_status_bar_hidden(true)
            .with_prefers_home_indicator_hidden(true)
            .with_preferred_screen_edges_deferring_system_gestures(ScreenEdge::ALL);

        let window = Arc::new(event_loop.create_window(window_attributes)?);

        #[cfg(target_os = "ios")]
        if let Some(monitor) = window.current_monitor() {
            window.set_scale_factor(monitor.scale_factor());
        }

        let surface = instance.create_surface(window.clone())?;

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("failed to acquire a WGPU adapter for the smoke window");

        let required_limits = wgpu::Limits::downlevel_defaults()
            .using_resolution(adapter.limits())
            .using_alignment(adapter.limits());
        let device_desc = wgpu::DeviceDescriptor {
            required_limits: required_limits.clone(),
            ..Default::default()
        };
        let adapter_limits = adapter.limits();
        let (device, queue) = block_on(adapter.request_device(&device_desc)).map_err(|err| {
            format!(
                "request_device failed: {err}; required_limits={required_limits:?}; adapter_limits={adapter_limits:?}"
            )
        })?;

        let physical_size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .copied()
            .find(|candidate| caps.formats.contains(candidate))
            .unwrap_or(caps.formats[0]);

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: physical_size.width.max(1),
            height: physical_size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_desc);

        let mut context = Context::create();
        context
            .io_mut()
            .set_config_flags(ConfigFlags::DOCKING_ENABLE | ConfigFlags::NAV_ENABLE_KEYBOARD);
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);

        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = WgpuRenderer::new(init_info, &mut context)
            .map_err(|err| format!("WgpuRenderer::new failed: {err}"))?;
        renderer.set_gamma_mode(GammaMode::Auto);

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui: ImguiState {
                context,
                platform,
                renderer,
                last_frame: Instant::now(),
                show_demo_window: true,
                touch_label: "Tap here and focus the input field to validate touch + IME."
                    .to_string(),
            },
            clear_color: wgpu::Color {
                r: 0.08,
                g: 0.11,
                b: 0.16,
                a: 1.0,
            },
            tap_counter: 0,
        })
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.surface_desc.width = size.width;
            self.surface_desc.height = size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());
        self.imgui.last_frame = now;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("Dear ImGui iOS Smoke")
            .size([460.0, 320.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Reference path: winit + wgpu");
                ui.separator();
                ui.text(format!(
                    "Framebuffer: {} x {}",
                    self.surface_desc.width, self.surface_desc.height
                ));
                ui.text(format!("FPS: {:.1}", ui.io().framerate()));

                if ui.button("Count Tap") {
                    self.tap_counter = self.tap_counter.saturating_add(1);
                }
                ui.same_line();
                ui.text(format!("Taps: {}", self.tap_counter));

                ui.input_text("Input", &mut self.imgui.touch_label).build();

                let mut color = [
                    self.clear_color.r as f32,
                    self.clear_color.g as f32,
                    self.clear_color.b as f32,
                    self.clear_color.a as f32,
                ];
                if ui.color_edit4("Clear Color", &mut color) {
                    self.clear_color.r = color[0] as f64;
                    self.clear_color.g = color[1] as f64;
                    self.clear_color.b = color[2] as f64;
                    self.clear_color.a = color[3] as f64;
                }

                ui.text_wrapped(
                    "Focus the input field to validate the iOS soft keyboard path handled by dear-imgui-winit.",
                );
            });

        if self.imgui.show_demo_window {
            ui.show_demo_window(&mut self.imgui.show_demo_window);
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        let (frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dear ImGui iOS Smoke Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Dear ImGui iOS Smoke Pass"),
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
                multiview_mask: None,
            });

            self.imgui.renderer.new_frame()?;
            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut render_pass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.surface_desc);
        }

        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                    if let Some(window) = &self.window {
                        window.window.request_redraw();
                    }
                }
                Err(err) => {
                    panic!("failed to initialize Dear ImGui iOS smoke app: {err}");
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );

        match event {
            WindowEvent::Resized(size) => {
                window.resize(size);
                window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                window.resize(window.window.inner_size());
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = window.render() {
                    panic!("Dear ImGui iOS smoke render failed: {err}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn start_inner() {
    let event_loop = EventLoop::new().expect("failed to create winit event loop for iOS smoke");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .expect("iOS smoke event loop terminated unexpectedly");
}

#[unsafe(no_mangle)]
pub extern "C" fn start_winit_app() {
    start_inner();
}
