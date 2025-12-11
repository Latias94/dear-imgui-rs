//! ImGuizmo Basic Example (wgpu + winit)
//!
//! - Toggle full-view vs window viewport
//! - Perspective/Orthographic camera, distance and FOV/width controls
//! - Choose operation (Translate/Rotate/Scale) and mode (Local/World)
//! - Optional snapping for translate/rotate/scale
//! - Draw grid and multiple cubes, manipulate with gizmos

use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_imguizmo as guizmo;
use dear_imguizmo::GuizmoExt;
use dear_imguizmo::graph::{Graph, GraphEditorExt, GraphView, Node, Pin, PinKind};
use glam::{Mat4, Vec3};
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,

    // Demo state
    state: DemoState,
}

#[derive(Clone, Copy)]
enum OpKind {
    Translate,
    Rotate,
    Scale,
}

struct DemoState {
    // Viewport control
    use_window: bool,
    gizmo_count: i32,
    gizmo_window_no_move: bool,

    // Camera
    is_perspective: bool,
    fov_deg: f32,
    ortho_width: f32,
    cam_distance: f32,
    cam_y_angle: f32,
    cam_x_angle: f32,
    first_frame: bool,

    camera_view: Mat4,
    camera_proj: Mat4,

    // Operation & mode
    current_op_kind: OpKind,
    current_mode: guizmo::Mode,
    use_snap: bool,
    snap: [f32; 3],

    // Objects
    last_using: i32,
    objects: [Mat4; 4],

    // Graph editor
    graph: Graph,
    graph_view: GraphView,
    graph_grid_visible: bool,
    graph_links_curves: bool,
    graph_scale_link_thickness: bool,
    graph_draw_io_on_hover: bool,
    graph_snap: f32,
    graph_minimap_enabled: bool,
    graph_show_editor: bool,
    graph_ctx_evt: Option<dear_imguizmo::graph::RightClickEvent>,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            use_window: true,
            gizmo_count: 1,
            gizmo_window_no_move: false,

            is_perspective: true,
            fov_deg: 27.0,
            ortho_width: 10.0,
            cam_distance: 8.0,
            cam_y_angle: 165.0_f32.to_radians(),
            cam_x_angle: 32.0_f32.to_radians(),
            first_frame: true,
            camera_view: Mat4::IDENTITY,
            camera_proj: Mat4::IDENTITY,

            current_op_kind: OpKind::Translate,
            current_mode: guizmo::Mode::World,
            use_snap: false,
            snap: [1.0, 1.0, 1.0],

            last_using: 0,
            objects: [
                Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, // col0
                    0.0, 1.0, 0.0, 0.0, // col1
                    0.0, 0.0, 1.0, 0.0, // col2
                    0.0, 0.0, 0.0, 1.0, // col3
                ]),
                Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0,
                ]),
                Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 2.0, 1.0,
                ]),
                Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 1.0,
                ]),
            ],

            graph: Graph::new(),
            graph_view: GraphView::default(),
            graph_grid_visible: true,
            graph_links_curves: true,
            graph_scale_link_thickness: false,
            graph_draw_io_on_hover: false,
            graph_snap: 0.0,
            graph_minimap_enabled: true,
            graph_show_editor: true,
            graph_ctx_evt: None,
        }
    }
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
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
                        .with_title(&format!("Dear ImGui + ImGuizmo Example - {version}"))
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
        // Pick an sRGB surface format when available for consistent visuals
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
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // Setup ImGui
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Initialize renderer
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("Failed to initialize WGPU renderer");
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

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
            last_frame: Instant::now(),
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            state: DemoState::default(),
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui.last_frame = now;

        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);

        let ui = self.imgui.context.frame();

        // Build UI
        {
            // Camera projection
            let io = ui.io();
            let ds = io.display_size();
            let aspect = if ds[1] > 0.0 { ds[0] / ds[1] } else { 1.0 };
            if self.state.is_perspective {
                self.state.camera_proj = perspective(self.state.fov_deg, aspect, 0.1, 100.0);
            } else {
                let view_height = self.state.ortho_width * (ds[1] / ds[0]).max(0.0001);
                self.state.camera_proj = orthographic(
                    -self.state.ortho_width,
                    self.state.ortho_width,
                    -view_height,
                    view_height,
                    1000.0,
                    -1000.0,
                );
            }

            let giz = ui.guizmo();
            giz.set_orthographic(!self.state.is_perspective);

            // Control window
            ui.window("Editor")
                .size([320.0, 340.0], Condition::FirstUseEver)
                .position([10.0, 10.0], Condition::FirstUseEver)
                .build(|| {
                    if ui.radio_button("Full view", !self.state.use_window) {
                        self.state.use_window = false;
                    }
                    ui.same_line();
                    if ui.radio_button("Window", self.state.use_window) {
                        self.state.use_window = true;
                    }

                    ui.text("Camera");
                    if ui.radio_button("Perspective", self.state.is_perspective) {
                        self.state.is_perspective = true;
                    }
                    ui.same_line();
                    if ui.radio_button("Orthographic", !self.state.is_perspective) {
                        self.state.is_perspective = false;
                    }
                    if self.state.is_perspective {
                        ui.slider_f32("Fov", &mut self.state.fov_deg, 20.0, 110.0);
                    } else {
                        ui.slider_f32("Ortho width", &mut self.state.ortho_width, 1.0, 20.0);
                    }
                    let mut view_dirty = false;
                    view_dirty |=
                        ui.slider_f32("Distance", &mut self.state.cam_distance, 1.0, 10.0);
                    ui.slider_i32("Gizmo count", &mut self.state.gizmo_count, 1, 4);

                    if view_dirty || self.state.first_frame {
                        let eye = Vec3::new(
                            self.state.cam_y_angle.cos()
                                * self.state.cam_x_angle.cos()
                                * self.state.cam_distance,
                            self.state.cam_x_angle.sin() * self.state.cam_distance,
                            self.state.cam_y_angle.sin()
                                * self.state.cam_x_angle.cos()
                                * self.state.cam_distance,
                        );
                        self.state.camera_view = look_at(eye, Vec3::ZERO, Vec3::Y);
                        self.state.first_frame = false;
                    }

                    ui.separator();
                    // Operation selection
                    match self.state.current_op_kind {
                        OpKind::Translate => if ui.radio_button("Translate", true) {},
                        _ => {
                            if ui.radio_button("Translate", false) {
                                self.state.current_op_kind = OpKind::Translate;
                            }
                        }
                    }
                    ui.same_line();
                    match self.state.current_op_kind {
                        OpKind::Rotate => if ui.radio_button("Rotate", true) {},
                        _ => {
                            if ui.radio_button("Rotate", false) {
                                self.state.current_op_kind = OpKind::Rotate;
                            }
                        }
                    }
                    ui.same_line();
                    match self.state.current_op_kind {
                        OpKind::Scale => if ui.radio_button("Scale", true) {},
                        _ => {
                            if ui.radio_button("Scale", false) {
                                self.state.current_op_kind = OpKind::Scale;
                            }
                        }
                    }

                    // Mode selection (not available for Scale in original demo, but allowed here)
                    if let OpKind::Scale = self.state.current_op_kind {
                        // leave as-is, user can still toggle mode but ImGuizmo ignores it for Scale
                    }
                    match self.state.current_mode {
                        guizmo::Mode::Local => if ui.radio_button("Local", true) {},
                        guizmo::Mode::World => {
                            if ui.radio_button("Local", false) {
                                self.state.current_mode = guizmo::Mode::Local;
                            }
                        }
                    }
                    ui.same_line();
                    match self.state.current_mode {
                        guizmo::Mode::World => if ui.radio_button("World", true) {},
                        guizmo::Mode::Local => {
                            if ui.radio_button("World", false) {
                                self.state.current_mode = guizmo::Mode::World;
                            }
                        }
                    }

                    // Snap controls
                    ui.checkbox("Enable snap", &mut self.state.use_snap);
                    if self.state.use_snap {
                        match self.state.current_op_kind {
                            OpKind::Translate => {
                                let _ = ui.input_scalar_n("Snap (Tr)", &mut self.state.snap);
                            }
                            OpKind::Rotate => {
                                ui.input_float("Angle Snap", &mut self.state.snap[0]);
                            }
                            OpKind::Scale => {
                                ui.input_float("Scale Snap", &mut self.state.snap[0]);
                            }
                        }
                    }
                });

            // Initialize a tiny graph on first frame
            if self.state.first_frame && self.state.graph.nodes.is_empty() {
                let n1 = self.state.graph.alloc_node_id();
                let n2 = self.state.graph.alloc_node_id();
                // Node1: two colored outputs
                let p_out0 = self.state.graph.alloc_pin_id();
                let p_out1 = self.state.graph.alloc_pin_id();
                let mut node1 = Node::new(n1, (-150.0_f32, -40.0), "Source");
                node1.outputs.push(Pin::colored(
                    p_out0,
                    "Out0",
                    PinKind::Output,
                    [0.78, 0.39, 0.39, 1.0],
                ));
                node1.outputs.push(Pin::colored(
                    p_out1,
                    "Out1",
                    PinKind::Output,
                    [0.39, 0.78, 0.39, 1.0],
                ));
                // Node2: three colored inputs
                let p_in0 = self.state.graph.alloc_pin_id();
                let p_in1 = self.state.graph.alloc_pin_id();
                let p_in2 = self.state.graph.alloc_pin_id();
                let mut node2 = Node::new(n2, (150.0_f32, -40.0), "Sink");
                node2.inputs.push(Pin::colored(
                    p_in0,
                    "In0",
                    PinKind::Input,
                    [0.78, 0.39, 0.39, 1.0],
                ));
                node2.inputs.push(Pin::colored(
                    p_in1,
                    "In1",
                    PinKind::Input,
                    [0.39, 0.78, 0.39, 1.0],
                ));
                node2.inputs.push(Pin::colored(
                    p_in2,
                    "In2",
                    PinKind::Input,
                    [0.39, 0.39, 0.78, 1.0],
                ));
                self.state.graph.nodes.push(node1);
                self.state.graph.nodes.push(node2);
                // initial link
                let lid = self.state.graph.alloc_link_id();
                self.state.graph.links.push(dear_imguizmo::graph::Link {
                    id: lid,
                    from: p_out0,
                    to: p_in0,
                });
            }

            // Graph Editor window (toggle like C++ sample)
            if self.state.graph_show_editor {
                ui.window("Graph Editor")
                    .size([420.0, 300.0], Condition::FirstUseEver)
                    .position([10.0, 360.0], Condition::FirstUseEver)
                    .build(|| {
                        if ui.button("Delete Selected") {
                            dear_imguizmo::graph::delete_selected(
                                &mut self.state.graph,
                                &mut self.state.graph_view,
                            );
                        }
                        ui.same_line();
                        if ui.button("Fit all nodes") {
                            let [wx, wy] = ui.window_pos();
                            let [ww, wh] = ui.window_size();
                            dear_imguizmo::graph::fit_all_nodes(
                                &self.state.graph,
                                &mut self.state.graph_view,
                                [wx, wy],
                                [ww, wh],
                            );
                        }
                        ui.same_line();
                        if ui.button("Fit selected nodes") {
                            let [wx, wy] = ui.window_pos();
                            let [ww, wh] = ui.window_size();
                            dear_imguizmo::graph::fit_selected_nodes(
                                &self.state.graph,
                                &mut self.state.graph_view,
                                [wx, wy],
                                [ww, wh],
                            );
                        }
                        ui.same_line();
                        ui.checkbox("Show grid", &mut self.state.graph_grid_visible);
                        ui.same_line();
                        ui.checkbox("Curved links", &mut self.state.graph_links_curves);
                        ui.same_line();
                        ui.checkbox(
                            "Scale thickness",
                            &mut self.state.graph_scale_link_thickness,
                        );
                        ui.same_line();
                        ui.checkbox("IO text on hover", &mut self.state.graph_draw_io_on_hover);
                        ui.same_line();
                        ui.checkbox("Minimap", &mut self.state.graph_minimap_enabled);
                        ui.same_line();
                        ui.set_next_item_width(100.0);
                        let _ = ui.slider("Snap", 0.0, 50.0, &mut self.state.graph_snap);
                        ui.same_line();
                        ui.text(format!(
                            "Selected: {} nodes",
                            self.state.graph_view.selected_nodes.len()
                        ));
                        let resp = ui
                            .graph_editor_config()
                            .graph(&mut self.state.graph)
                            .view(&mut self.state.graph_view)
                            .grid_visible(self.state.graph_grid_visible)
                            .display_links_as_curves(self.state.graph_links_curves)
                            .scale_link_thickness_with_zoom(self.state.graph_scale_link_thickness)
                            .draw_io_name_on_hover(self.state.graph_draw_io_on_hover)
                            .snap(self.state.graph_snap)
                            .minimap_enabled(self.state.graph_minimap_enabled)
                            .grid_spacing(28.0)
                            .build();
                        if let Some(evt) = resp.right_click {
                            self.state.graph_ctx_evt = Some(evt);
                            ui.open_popup("GraphCtx");
                        }
                        if let Some(_popup) = ui.begin_popup("GraphCtx") {
                            if let Some(evt) = self.state.graph_ctx_evt {
                                if let Some(nid) = evt.node {
                                    if ui.menu_item("Delete Node") {
                                        self.state.graph_view.selected_nodes.clear();
                                        self.state.graph_view.selected_nodes.insert(nid);
                                        dear_imguizmo::graph::delete_selected(
                                            &mut self.state.graph,
                                            &mut self.state.graph_view,
                                        );
                                        self.state.graph_ctx_evt = None;
                                    }
                                } else {
                                    if ui.menu_item("Add Node Here") {
                                        // convert screen pos to world
                                        let [wx, wy] = ui.window_pos();
                                        let mp = evt.mouse_pos;
                                        let wpos = [
                                            (mp[0] - wx - self.state.graph_view.pan[0])
                                                / self.state.graph_view.zoom,
                                            (mp[1] - wy - self.state.graph_view.pan[1])
                                                / self.state.graph_view.zoom,
                                        ];
                                        let nid = self.state.graph.alloc_node_id();
                                        let mut node =
                                            Node::new(nid, (wpos[0], wpos[1]), "New Node");
                                        let pin_in = self.state.graph.alloc_pin_id();
                                        let pin_out = self.state.graph.alloc_pin_id();
                                        node.inputs.push(Pin::colored(
                                            pin_in,
                                            "In",
                                            PinKind::Input,
                                            [0.78, 0.39, 0.39, 1.0],
                                        ));
                                        node.outputs.push(Pin::colored(
                                            pin_out,
                                            "Out",
                                            PinKind::Output,
                                            [0.39, 0.78, 0.39, 1.0],
                                        ));
                                        self.state.graph.nodes.push(node);
                                        self.state.graph_ctx_evt = None;
                                    }
                                }
                                if ui.menu_item("Delete Selected") {
                                    dear_imguizmo::graph::delete_selected(
                                        &mut self.state.graph,
                                        &mut self.state.graph_view,
                                    );
                                    self.state.graph_ctx_evt = None;
                                }
                            }
                        }
                    });
            }

            // Gizmo viewport window (or fullscreen)
            let op_bits = match self.state.current_op_kind {
                OpKind::Translate => guizmo::Operation::TRANSLATE,
                OpKind::Rotate => guizmo::Operation::ROTATE,
                OpKind::Scale => guizmo::Operation::SCALE,
            };
            let snap_opt = if self.state.use_snap {
                Some(&self.state.snap)
            } else {
                None
            };

            if self.state.use_window {
                let mut flags = WindowFlags::empty();
                if self.state.gizmo_window_no_move {
                    flags |= WindowFlags::NO_MOVE;
                }
                ui.window("Gizmo")
                    .size([800.0, 400.0], Condition::FirstUseEver)
                    .position([400.0, 20.0], Condition::FirstUseEver)
                    .flags(flags)
                    .build(|| {
                        // Bind draw list to current window and set rect
                        let [wx, wy] = ui.window_pos();
                        let [ww, wh] = ui.window_size();
                        giz.set_drawlist_window();
                        giz.set_rect(wx, wy, ww, wh);

                        // Draw grid + cubes
                        let identity = Mat4::IDENTITY;
                        giz.draw_grid(
                            &self.state.camera_view,
                            &self.state.camera_proj,
                            &identity,
                            100.0,
                        );
                        let count = self.state.gizmo_count.clamp(1, 4) as usize;
                        giz.draw_cubes(
                            &self.state.camera_view,
                            &self.state.camera_proj,
                            &self.state.objects[0..count],
                        );

                        // Manipulate each cube (window drawlist already set)
                        for i in 0..count {
                            let _id = giz.push_id(i as i32);
                            let mut m = giz
                                .manipulate_config(
                                    &self.state.camera_view,
                                    &self.state.camera_proj,
                                    &mut self.state.objects[i],
                                )
                                .operation(op_bits)
                                .mode(self.state.current_mode);
                            if let Some(snap) = snap_opt {
                                m = m.snap(*snap);
                            }
                            let _used = m.build();
                            if giz.is_using() {
                                self.state.last_using = i as i32;
                            }
                        }

                        // View manipulator on the top-right of the window
                        let pos = [wx + ww - 128.0, wy];
                        let size = [128.0, 128.0];
                        giz.view_manipulate(
                            &mut self.state.camera_view,
                            self.state.cam_distance,
                            pos,
                            size,
                            0x10101010,
                        );

                        // Lock window move when interacting with gizmo
                        self.state.gizmo_window_no_move = giz.is_over() || giz.is_using();
                    });
            } else {
                // Full view
                let ds = ui.io().display_size();
                giz.set_drawlist_background();
                giz.set_rect(0.0, 0.0, ds[0], ds[1]);
                let identity = Mat4::IDENTITY;
                giz.draw_grid(
                    &self.state.camera_view,
                    &self.state.camera_proj,
                    &identity,
                    100.0,
                );
                let count = self.state.gizmo_count.clamp(1, 4) as usize;
                giz.draw_cubes(
                    &self.state.camera_view,
                    &self.state.camera_proj,
                    &self.state.objects[0..count],
                );
                // Manipulate (foreground drawlist already set)
                for i in 0..count {
                    let _id = giz.push_id(i as i32);
                    let mut m = giz
                        .manipulate_config(
                            &self.state.camera_view,
                            &self.state.camera_proj,
                            &mut self.state.objects[i],
                        )
                        .operation(op_bits)
                        .mode(self.state.current_mode);
                    if let Some(snap) = snap_opt {
                        m = m.snap(*snap);
                    }
                    let _used = m.build();
                    if giz.is_using() {
                        self.state.last_using = i as i32;
                    }
                }
                // View manipulator on the top-right of the screen
                let pos = [ds[0] - 128.0, 0.0];
                let size = [128.0, 128.0];
                giz.view_manipulate(
                    &mut self.state.camera_view,
                    self.state.cam_distance,
                    pos,
                    size,
                    0x10101010,
                );
            }

            // Inspector for last selected matrix (decompose/recompose)
            ui.window("Gizmo Inspector")
                .size([320.0, 260.0], Condition::FirstUseEver)
                .position([10.0, 360.0], Condition::FirstUseEver)
                .build(|| {
                    let idx = self
                        .state
                        .last_using
                        .clamp(0, (self.state.gizmo_count - 1).max(0))
                        as usize;
                    ui.text(format!("Editing object #{idx}"));
                    let (mut tr, mut rt, mut sc) =
                        guizmo::decompose_matrix(&self.state.objects[idx]);
                    let _ = ui.input_scalar_n("Tr", &mut tr);
                    let _ = ui.input_scalar_n("Rt", &mut rt);
                    let _ = ui.input_scalar_n("Sc", &mut sc);
                    self.state.objects[idx] = guizmo::recompose_matrix(&tr, &rt, &sc);
                });

            // Additional controls like C++ sample: toggle Graph Editor visibility
            ui.window("Graph Controls")
                .size([300.0, 120.0], Condition::FirstUseEver)
                .position([10.0, 680.0], Condition::FirstUseEver)
                .build(|| {
                    if ui.collapsing_header("Graph Editor", dear_imgui_rs::TreeNodeFlags::empty()) {
                        ui.checkbox("Show GraphEditor", &mut self.state.graph_show_editor);
                    }
                });
        }

        // Rendering
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("imgui encoder"),
            });
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("imgui render pass"),
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

        let draw_data = self.imgui.context.render();
        self.imgui
            .renderer
            .render_draw_data(draw_data, &mut rpass)?;
        drop(rpass);
        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }

    // (build_ui removed; content moved inline into render())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                }
                Err(e) => {
                    eprintln!("Failed to create window: {}", e);
                    event_loop.exit();
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
        if let Some(window) = &mut self.window {
            window.imgui.platform.handle_window_event(
                &mut window.imgui.context,
                &window.window,
                &event,
            );

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(new_size) => {
                    window.resize(new_size);
                }
                WindowEvent::RedrawRequested => {
                    if let Err(e) = window.render() {
                        eprintln!("Render error: {}", e);
                    }
                    window.window.request_redraw();
                }
                _ => {}
            }
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}

// --- Math helpers (match ImGuizmo demo behavior) ---
fn frustum(left: f32, right: f32, bottom: f32, top: f32, znear: f32, zfar: f32) -> Mat4 {
    let temp = 2.0 * znear;
    let temp2 = right - left;
    let temp3 = top - bottom;
    let temp4 = zfar - znear;
    Mat4::from_cols_array(&[
        temp / temp2,
        0.0,
        0.0,
        0.0,
        0.0,
        temp / temp3,
        0.0,
        0.0,
        (right + left) / temp2,
        (top + bottom) / temp3,
        (-zfar - znear) / temp4,
        -1.0,
        0.0,
        0.0,
        (-temp * zfar) / temp4,
        0.0,
    ])
}

fn perspective(fovy_degrees: f32, aspect_ratio: f32, znear: f32, zfar: f32) -> Mat4 {
    let ymax = znear * (fovy_degrees.to_radians()).tan();
    let xmax = ymax * aspect_ratio.max(0.0001);
    frustum(-xmax, xmax, -ymax, ymax, znear, zfar)
}

fn orthographic(l: f32, r: f32, b: f32, t: f32, zn: f32, zf: f32) -> Mat4 {
    Mat4::from_cols_array(&[
        2.0 / (r - l),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (t - b),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / (zf - zn),
        0.0,
        (l + r) / (l - r),
        (t + b) / (b - t),
        zn / (zn - zf),
        1.0,
    ])
}

fn look_at(eye: Vec3, at: Vec3, up: Vec3) -> Mat4 {
    let mut z = (eye - at).normalize_or_zero();
    let mut y = up.normalize_or_zero();
    let mut x = y.cross(z).normalize_or_zero();
    y = z.cross(x).normalize_or_zero();

    Mat4::from_cols_array(&[
        x.x,
        y.x,
        z.x,
        0.0,
        x.y,
        y.y,
        z.y,
        0.0,
        x.z,
        y.z,
        z.z,
        0.0,
        -x.dot(eye),
        -y.dot(eye),
        -z.dot(eye),
        1.0,
    ])
}
