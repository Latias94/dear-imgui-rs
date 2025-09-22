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
    // Projection and helpers
    use_orthographic: bool,
    fov_degrees: f32,
    ortho_height: f32,
    // Helpers drawing
    show_grid: bool,
    grid_size: f32,
    show_cubes: bool,
    cube_mats: [glam::Mat4; 3],
    // Snapping
    use_snap: bool,
    translate_snap: [f32; 3],
    rotate_snap_deg: f32,
    scale_snap: [f32; 3],
    universal_snap_mode: i32, // 0=Translate 1=Rotate 2=Scale
    // Bounds editing
    enable_bounds: bool,
    local_bounds: [f32; 6],
    bounds_snap: [f32; 3],
    // Visibility/behavior
    allow_axis_flip: bool,
    axis_limit: f32,
    plane_limit: f32,
    gizmo_size_clip: f32,
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
            projection_matrix: create_perspective_for_aspect(45.0, 1280.0 / 720.0, 0.1, 100.0),
            current_operation: Operation::UNIVERSAL,
            current_mode: Mode::World,
            use_orthographic: false,
            fov_degrees: 45.0,
            ortho_height: 10.0,
            show_grid: true,
            grid_size: 1.0,
            show_cubes: true,
            cube_mats: [
                glam::Mat4::from_translation(glam::vec3(-2.0, 0.0, -2.0)),
                glam::Mat4::from_translation(glam::vec3(2.0, 0.0, 2.0)),
                glam::Mat4::from_scale(glam::vec3(0.75, 0.75, 0.75))
                    * glam::Mat4::from_translation(glam::vec3(0.0, 1.0, -3.0)),
            ],
            use_snap: true,
            translate_snap: [0.5, 0.5, 0.5],
            rotate_snap_deg: 15.0,
            scale_snap: [0.1, 0.1, 0.1],
            universal_snap_mode: 1,
            enable_bounds: false,
            local_bounds: [-0.5, 0.5, -0.5, 0.5, -0.5, 0.5],
            bounds_snap: [0.1, 0.1, 0.1],
            allow_axis_flip: true,
            axis_limit: 0.0025,
            plane_limit: 0.02,
            gizmo_size_clip: 0.1,
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
            self.imgui.projection_matrix = if self.imgui.use_orthographic {
                create_orthographic_for_aspect(self.imgui.ortho_height, aspect, 0.1, 100.0)
            } else {
                create_perspective_for_aspect(self.imgui.fov_degrees, aspect, 0.1, 100.0)
            };
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

fn create_perspective_for_aspect(fov_degrees: f32, aspect: f32, near: f32, far: f32) -> glam::Mat4 {
    glam::Mat4::perspective_rh(fov_degrees.to_radians(), aspect, near, far)
}

fn create_orthographic_for_aspect(height: f32, aspect: f32, near: f32, far: f32) -> glam::Mat4 {
    let half_h = height * 0.5;
    let half_w = half_h * aspect;
    glam::Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
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
        let _ = gizmo_ui.set_rect(0.0, 0.0, window_size[0], window_size[1]);

        // Control panel
        ui.window("ImGuizmo Controls")
            .size([360.0, 560.0], Condition::FirstUseEver)
            .position([10.0, 10.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("ImGuizmo: common features demo");
                ui.separator();

                // Operation selection
                ui.text("Operation");
                if ui.radio_button(
                    "Universal",
                    self.imgui.current_operation == Operation::UNIVERSAL,
                ) {
                    self.imgui.current_operation = Operation::UNIVERSAL;
                }
                if ui.radio_button(
                    "Translate",
                    self.imgui.current_operation == Operation::TRANSLATE,
                ) {
                    self.imgui.current_operation = Operation::TRANSLATE;
                }
                if ui.radio_button("Rotate", self.imgui.current_operation == Operation::ROTATE) {
                    self.imgui.current_operation = Operation::ROTATE;
                }
                if ui.radio_button("Scale", self.imgui.current_operation == Operation::SCALE) {
                    self.imgui.current_operation = Operation::SCALE;
                }

                ui.separator();

                // Mode selection
                ui.text("Coordinate System");
                if ui.radio_button("World", matches!(self.imgui.current_mode, Mode::World)) {
                    self.imgui.current_mode = Mode::World;
                }
                if ui.radio_button("Local", matches!(self.imgui.current_mode, Mode::Local)) {
                    self.imgui.current_mode = Mode::Local;
                }

                ui.separator();

                // Projection
                ui.text("Projection");
                ui.checkbox("Orthographic", &mut self.imgui.use_orthographic);
                if self.imgui.use_orthographic {
                    ui.slider("Ortho Height", 2.0, 50.0, &mut self.imgui.ortho_height);
                } else {
                    ui.slider("FOV (deg)", 10.0, 120.0, &mut self.imgui.fov_degrees);
                }

                // Apply projection immediately
                let aspect = self.surface_desc.width as f32 / self.surface_desc.height as f32;
                self.imgui.projection_matrix = if self.imgui.use_orthographic {
                    create_orthographic_for_aspect(self.imgui.ortho_height, aspect, 0.1, 100.0)
                } else {
                    create_perspective_for_aspect(self.imgui.fov_degrees, aspect, 0.1, 100.0)
                };

                ui.separator();

                // Snap settings
                ui.checkbox("Enable Snap", &mut self.imgui.use_snap);
                if self.imgui.use_snap {
                    if self.imgui.current_operation == Operation::UNIVERSAL {
                        ui.text("Universal snap applies to:");
                        if ui.radio_button("Translate", self.imgui.universal_snap_mode == 0) {
                            self.imgui.universal_snap_mode = 0;
                        }
                        ui.same_line();
                        if ui.radio_button("Rotate", self.imgui.universal_snap_mode == 1) {
                            self.imgui.universal_snap_mode = 1;
                        }
                        ui.same_line();
                        if ui.radio_button("Scale", self.imgui.universal_snap_mode == 2) {
                            self.imgui.universal_snap_mode = 2;
                        }
                    }
                    ui.drag_float3("Translate snap", &mut self.imgui.translate_snap);
                    ui.slider(
                        "Rotate snap (deg)",
                        1.0,
                        90.0,
                        &mut self.imgui.rotate_snap_deg,
                    );
                    ui.drag_float3("Scale snap", &mut self.imgui.scale_snap);
                }

                ui.separator();

                // Behavior & size
                ui.text("Behavior");
                ui.checkbox("Allow axis flip", &mut self.imgui.allow_axis_flip);
                ui.slider(
                    "Gizmo size (clip)",
                    0.05,
                    0.3,
                    &mut self.imgui.gizmo_size_clip,
                );
                ui.slider("Axis limit", 0.0, 0.02, &mut self.imgui.axis_limit);
                ui.slider("Plane limit", 0.0, 0.2, &mut self.imgui.plane_limit);

                ui.separator();

                // Helpers
                ui.text("Helpers");
                ui.checkbox("Draw grid", &mut self.imgui.show_grid);
                if self.imgui.show_grid {
                    ui.slider("Grid size", 0.25, 5.0, &mut self.imgui.grid_size);
                }
                ui.checkbox("Draw sample cubes", &mut self.imgui.show_cubes);

                ui.separator();

                // Bounds
                ui.text("Bounds");
                ui.checkbox("Enable local bounds editing", &mut self.imgui.enable_bounds);
                if self.imgui.enable_bounds {
                    let mut bmin = [
                        self.imgui.local_bounds[0],
                        self.imgui.local_bounds[2],
                        self.imgui.local_bounds[4],
                    ];
                    let mut bmax = [
                        self.imgui.local_bounds[1],
                        self.imgui.local_bounds[3],
                        self.imgui.local_bounds[5],
                    ];
                    ui.drag_float3("Bounds min", &mut bmin);
                    ui.drag_float3("Bounds max", &mut bmax);
                    self.imgui.local_bounds =
                        [bmin[0], bmax[0], bmin[1], bmax[1], bmin[2], bmax[2]];
                    ui.drag_float3("Bounds snap", &mut self.imgui.bounds_snap);
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
            .size([920.0, 680.0], Condition::FirstUseEver)
            .position([380.0, 10.0], Condition::FirstUseEver)
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

                // Reserve space for the gizmo (we draw into the window background)
                ui.dummy(content_region);

                // Apply per-frame gizmo settings
                gizmo_ui.set_orthographic(self.imgui.use_orthographic);
                gizmo_ui.allow_axis_flip(self.imgui.allow_axis_flip);
                gizmo_ui.set_gizmo_size_clip_space(self.imgui.gizmo_size_clip);
                gizmo_ui.set_axis_limit(self.imgui.axis_limit);
                gizmo_ui.set_plane_limit(self.imgui.plane_limit);

                // Draw helpers first (these methods create their own DrawListMut internally)
                if self.imgui.show_grid {
                    gizmo_ui.draw_grid(
                        &self.imgui.view_matrix,
                        &self.imgui.projection_matrix,
                        &glam::Mat4::IDENTITY,
                        self.imgui.grid_size,
                    );
                }
                if self.imgui.show_cubes {
                    gizmo_ui.draw_cubes(
                        &self.imgui.view_matrix,
                        &self.imgui.projection_matrix,
                        &self.imgui.cube_mats,
                    );
                }

                // Get the window draw list for gizmo rendering (after helpers are done)
                let draw_list = ui.get_window_draw_list();

                // Build snap option according to operation
                let mut rot_snap_arr = [self.imgui.rotate_snap_deg, 0.0, 0.0];
                let snap_opt: Option<&[f32; 3]> = if self.imgui.use_snap {
                    let op = self.imgui.current_operation;
                    if op == Operation::UNIVERSAL {
                        match self.imgui.universal_snap_mode {
                            0 => Some(&self.imgui.translate_snap),
                            1 => Some(&rot_snap_arr),
                            _ => Some(&self.imgui.scale_snap),
                        }
                    } else if op.intersects(Operation::ROTATE) {
                        Some(&rot_snap_arr)
                    } else if op.intersects(Operation::TRANSLATE) {
                        Some(&self.imgui.translate_snap)
                    } else if op.intersects(Operation::SCALE | Operation::SCALE_UNIFORM) {
                        Some(&self.imgui.scale_snap)
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Bounds options
                let local_bounds_opt = if self.imgui.enable_bounds {
                    Some(&self.imgui.local_bounds)
                } else {
                    None
                };
                let bounds_snap_opt = if self.imgui.enable_bounds {
                    Some(&self.imgui.bounds_snap)
                } else {
                    None
                };

                // Perform the gizmo manipulation (full options)
                gizmo_ui.manipulate_with_options(
                    &draw_list,
                    &self.imgui.view_matrix,
                    &self.imgui.projection_matrix,
                    self.imgui.current_operation,
                    self.imgui.current_mode,
                    &mut self.imgui.object_matrix,
                    None,
                    snap_opt,
                    local_bounds_opt,
                    bounds_snap_opt,
                )
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
            .size([420.0, 260.0], Condition::FirstUseEver)
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
                ui.text(&format!("Over Any: {}", gizmo_ui.is_over()));
                ui.text(&format!(
                    "Over Op(Translate): {}",
                    gizmo_ui.is_over_operation(Operation::TRANSLATE)
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
