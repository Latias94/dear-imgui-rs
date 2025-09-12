use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_imguizmo::*;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    demo_open: bool,
    last_frame: Instant,
    // ImGuizmo state
    gizmo_context: GuizmoContext,
    object_matrix: glam::Mat4,
    view_matrix: glam::Mat4,
    projection_matrix: glam::Mat4,
    current_operation: Operation,
    current_mode: Mode,
    use_snap: bool,
    snap_values: [f32; 3],
    show_view_manipulate: bool,
    view_manipulate_size: f32,
    camera_distance: f32,
    camera_angle_x: f32,
    camera_angle_y: f32,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let size = LogicalSize::new(1280.0, 720.0);

            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(&format!("Dear ImGuizmo Demo - {version}"))
                        .with_inner_size(size),
                )?,
            )
        };

        let surface = instance.create_surface(window.clone())?;

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let size = LogicalSize::new(1280.0, 720.0);
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // Setup ImGui immediately
        let mut context = Context::create_or_panic();
        context.set_ini_filename_or_panic(None::<String>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Initialize the renderer with one-step initialization
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("Failed to initialize WGPU renderer");

        // Log successful initialization
        dear_imgui::logging::log_context_created();
        dear_imgui::logging::log_platform_init("Winit");
        dear_imgui::logging::log_renderer_init("WGPU");

        // Initialize ImGuizmo
        let gizmo_context = GuizmoContext::new();

        // Initialize matrices
        let object_matrix = glam::Mat4::IDENTITY;

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            demo_open: false,
            last_frame: Instant::now(),
            gizmo_context,
            object_matrix,
            view_matrix: create_view_matrix(8.0, 0.0, 0.0),
            projection_matrix: create_projection_matrix(45.0, 1280.0 / 720.0, 0.1, 100.0),
            current_operation: Operation::TRANSLATE,
            current_mode: Mode::World,
            use_snap: false,
            snap_values: [1.0, 1.0, 1.0],
            show_view_manipulate: true,
            view_manipulate_size: 128.0,
            camera_distance: 8.0,
            camera_angle_x: 0.0,
            camera_angle_y: 0.0,
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);

            // Update projection matrix for new aspect ratio
            let aspect = new_size.width as f32 / new_size.height as f32;
            self.imgui.projection_matrix = create_projection_matrix(45.0, aspect, 0.1, 100.0);
        }
    }
}

// Helper functions for matrix creation
fn create_view_matrix(distance: f32, angle_x: f32, angle_y: f32) -> glam::Mat4 {
    // Simple look-at matrix pointing towards origin
    let eye_x = distance * angle_y.cos() * angle_x.cos();
    let eye_y = distance * angle_x.sin();
    let eye_z = distance * angle_y.sin() * angle_x.cos();

    // Use glam's look_at_rh function
    let eye = glam::Vec3::new(eye_x, eye_y, eye_z);
    let center = glam::Vec3::ZERO;
    let up = glam::Vec3::Y;
    glam::Mat4::look_at_rh(eye, center, up)
}

fn create_projection_matrix(fov_degrees: f32, aspect: f32, near: f32, far: f32) -> glam::Mat4 {
    glam::Mat4::perspective_rh(fov_degrees.to_radians(), aspect, near, far)
}

impl AppWindow {
    fn render(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        let delta_secs = delta_time.as_secs_f32();

        self.imgui.context.io_mut().set_delta_time(delta_secs);
        self.imgui.last_frame = now;

        let frame = self.surface.get_current_texture()?;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Update view matrix based on camera controls
        self.imgui.view_matrix = create_view_matrix(
            self.imgui.camera_distance,
            self.imgui.camera_angle_x,
            self.imgui.camera_angle_y,
        );

        // Begin ImGuizmo frame
        let gizmo_ui = self.imgui.gizmo_context.get_ui(&ui);

        // Set the viewport for ImGuizmo (full window)
        let window_size = ui.io().display_size();
        gizmo_ui.set_rect(0.0, 0.0, window_size[0], window_size[1]);

        // Control panel
        ui.window("ImGuizmo Controls")
            .size([300.0, 400.0], Condition::FirstUseEver)
            .position([10.0, 10.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("3D Gizmo Manipulation Demo");
                ui.separator();

                // Test drawing in ImGui window
                let draw_list = ui.get_window_draw_list();
                let cursor_pos = ui.cursor_screen_pos();

                // Draw test lines in the window
                draw_list
                    .add_line(
                        [cursor_pos[0], cursor_pos[1]],
                        [cursor_pos[0] + 100.0, cursor_pos[1] + 50.0],
                        0xFF00FF00, // Green
                    )
                    .thickness(3.0)
                    .build();

                draw_list
                    .add_line(
                        [cursor_pos[0], cursor_pos[1]],
                        [cursor_pos[0] + 50.0, cursor_pos[1] + 100.0],
                        0xFF0000FF, // Red
                    )
                    .thickness(3.0)
                    .build();

                ui.dummy([120.0, 120.0]); // Make space for the lines

                // Operation selection
                ui.text("Operation:");
                let mut op_translate = self.imgui.current_operation.contains(Operation::TRANSLATE);
                let mut op_rotate = self.imgui.current_operation.contains(Operation::ROTATE);
                let mut op_scale = self.imgui.current_operation.contains(Operation::SCALE);

                if ui.radio_button("Translate", op_translate && !op_rotate && !op_scale) {
                    self.imgui.current_operation = Operation::TRANSLATE;
                }
                if ui.radio_button("Rotate", op_rotate && !op_translate && !op_scale) {
                    self.imgui.current_operation = Operation::ROTATE;
                }
                if ui.radio_button("Scale", op_scale && !op_translate && !op_rotate) {
                    self.imgui.current_operation = Operation::SCALE;
                }

                ui.separator();

                // Mode selection
                ui.text("Coordinate System:");
                let mut is_world = matches!(self.imgui.current_mode, Mode::World);
                if ui.radio_button("World", is_world) {
                    self.imgui.current_mode = Mode::World;
                }
                if ui.radio_button("Local", !is_world) {
                    self.imgui.current_mode = Mode::Local;
                }

                ui.separator();

                // Snap settings
                ui.checkbox("Enable Snap", &mut self.imgui.use_snap);
                if self.imgui.use_snap {
                    ui.drag_float3("Snap Values", &mut self.imgui.snap_values);
                }

                ui.separator();

                // View manipulation settings
                ui.checkbox("Show View Manipulate", &mut self.imgui.show_view_manipulate);
                if self.imgui.show_view_manipulate {
                    ui.slider(
                        "View Cube Size",
                        64.0,
                        256.0,
                        &mut self.imgui.view_manipulate_size,
                    );
                }

                ui.separator();

                // Camera controls
                ui.text("Camera Controls:");
                ui.slider("Distance", 2.0, 20.0, &mut self.imgui.camera_distance);
                ui.slider(
                    "Angle X",
                    -std::f32::consts::PI,
                    std::f32::consts::PI,
                    &mut self.imgui.camera_angle_x,
                );
                ui.slider(
                    "Angle Y",
                    -std::f32::consts::PI,
                    std::f32::consts::PI,
                    &mut self.imgui.camera_angle_y,
                );

                if ui.button("Reset Camera") {
                    self.imgui.camera_distance = 8.0;
                    self.imgui.camera_angle_x = 0.0;
                    self.imgui.camera_angle_y = 0.0;
                }

                if ui.button("Reset Object") {
                    self.imgui.object_matrix = glam::Mat4::IDENTITY;
                }

                ui.separator();

                // Show demo window toggle
                ui.checkbox("Show ImGui Demo", &mut self.imgui.demo_open);

                ui.separator();

                // Frame rate
                ui.text(&format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });

        // Main 3D manipulation in a dedicated window
        let manipulation_result = ui
            .window("3D Gizmo Viewport")
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position([320.0, 10.0], Condition::FirstUseEver)
            .build(|| {
                // Set the gizmo viewport to this window's content area
                let content_region = ui.content_region_avail();
                let cursor_pos = ui.cursor_screen_pos();

                // Update gizmo viewport to match this window
                let _ = gizmo_ui.set_rect(
                    cursor_pos[0],
                    cursor_pos[1],
                    content_region[0],
                    content_region[1],
                );

                ui.text("3D Gizmo should appear here");
                ui.text(format!(
                    "Viewport: [{:.0}, {:.0}] size: [{:.0}, {:.0}]",
                    cursor_pos[0], cursor_pos[1], content_region[0], content_region[1]
                ));

                // Reserve space for the gizmo
                ui.dummy(content_region);

                // Get the window draw list for gizmo rendering
                let draw_list = ui.get_window_draw_list();

                // Now perform the gizmo manipulation
                if self.imgui.use_snap {
                    gizmo_ui.manipulate_with_snap(
                        &draw_list,
                        &self.imgui.view_matrix,
                        &self.imgui.projection_matrix,
                        self.imgui.current_operation,
                        self.imgui.current_mode,
                        &mut self.imgui.object_matrix,
                        Some(&self.imgui.snap_values),
                    )
                } else {
                    gizmo_ui.manipulate(
                        &draw_list,
                        &self.imgui.view_matrix,
                        &self.imgui.projection_matrix,
                        self.imgui.current_operation,
                        self.imgui.current_mode,
                        &mut self.imgui.object_matrix,
                    )
                }
            })
            .unwrap_or(Ok(false));

        // Handle manipulation result
        match manipulation_result {
            Ok(modified) => {
                if modified {
                    info!(
                        "Object manipulated with operation: {:?}",
                        self.imgui.current_operation
                    );
                }
            }
            Err(e) => {
                error!("Manipulation error: {:?}", e);
            }
        }

        // View manipulation (camera controls)
        if self.imgui.show_view_manipulate {
            let window_size = ui.io().display_size();
            let view_pos = [
                window_size[0] - self.imgui.view_manipulate_size - 10.0,
                10.0,
            ];
            let view_size = [
                self.imgui.view_manipulate_size,
                self.imgui.view_manipulate_size,
            ];

            let view_modified = gizmo_ui.view_manipulate(
                &mut self.imgui.view_matrix,
                8.0,
                view_pos,
                view_size,
                0x10101050,
            );

            if view_modified {
                info!("View manipulated");
            }
        }

        // Matrix information window
        ui.window("Matrix Information")
            .size([350.0, 200.0], Condition::FirstUseEver)
            .position(
                [window_size[0] - 360.0, window_size[1] - 210.0],
                Condition::FirstUseEver,
            )
            .build(|| {
                ui.text("Object Transformation Matrix:");
                ui.separator();

                // Display matrix in a readable format
                let matrix_array = self.imgui.object_matrix.to_cols_array();
                for row in 0..4 {
                    let start_idx = row * 4;
                    ui.text(&format!(
                        "[{:6.2}, {:6.2}, {:6.2}, {:6.2}]",
                        matrix_array[start_idx],
                        matrix_array[start_idx + 1],
                        matrix_array[start_idx + 2],
                        matrix_array[start_idx + 3]
                    ));
                }

                ui.separator();

                // Show gizmo state
                ui.text(&format!("Operation: {:?}", self.imgui.current_operation));
                ui.text(&format!("Mode: {:?}", self.imgui.current_mode));
                ui.text(&format!("Using: {}", gizmo_ui.is_using()));
                ui.text(&format!(
                    "Over Operation: {}",
                    gizmo_ui.is_over_operation(self.imgui.current_operation)
                ));
            });

        // Show demo window if requested
        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
        }

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
                        load: wgpu::LoadOp::Clear(self.imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Call new_frame before rendering
            self.imgui
                .renderer
                .new_frame()
                .expect("Failed to prepare new frame");

            if let Err(e) = self.imgui.renderer.render_draw_data(&draw_data, &mut rpass) {
                error!("Failed to render draw data: {}", e);
                return Err(Box::new(e));
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // For compatibility with older winit versions and mobile platforms
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                    info!("Window created successfully in resumed");
                }
                Err(e) => {
                    error!("Failed to create window in resumed: {e}");
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
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        // Handle the event with ImGui first
        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        window
            .imgui
            .platform
            .handle_event(&mut window.imgui.context, &window.window, &full_event);

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                info!("Close requested");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    error!("Render error: {e}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw if we have a window
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn main() {
    // Initialize tracing with custom filter for demo
    dear_imgui::logging::init_tracing_with_filter("dear_imgui=debug,imguizmo_basic=info,wgpu=warn");

    info!("Starting Dear ImGuizmo Basic Example");
    info!("This example demonstrates:");
    info!("  - 3D gizmo manipulation (translate, rotate, scale)");
    info!("  - View manipulation with camera controls");
    info!("  - Real-time matrix display");
    info!("  - Coordinate system switching (World/Local)");
    info!("  - Snap-to-grid functionality");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();

    info!("Starting event loop...");
    event_loop.run_app(&mut app).unwrap();

    info!("Dear ImGuizmo Basic Example finished");
}
