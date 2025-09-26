//! Dear ImNodes Basic Example
//!
//! Minimal demo showing how to integrate ImNodes via dear-imnodes
//! following the same windowing/render pattern as other examples.

use dear_imgui::input::MouseButton;
use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_imnodes as imnodes;
use dear_imnodes::ImNodesExt;
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
    nodes_context: imnodes::Context,
    editor_context: imnodes::EditorContext,
    editor_context_b: imnodes::EditorContext,
    editor_context_shader: imnodes::EditorContext,
    saved_ini: Option<String>,
    clear_color: wgpu::Color,
    last_frame: Instant,
}

struct GraphState {
    next_link_id: i32,
    links: Vec<(i32, i32, i32)>, // (id, start_attr, end_attr)
    positions_initialized: bool,
    // Added nodes: (node_id, pending_screen_pos_for_first_frame)
    added_nodes: Vec<(i32, Option<[f32; 2]>)>,
    next_node_id: i32,
}

#[derive(Clone, Copy)]
struct NodeOptions {
    show_grid: bool,
    primary_lines: bool,
    grid_snapping: bool,
    grid_spacing: f32,
    link_thickness: f32,
    corner_round: f32,
    node_padding: [f32; 2],
    node_border_thickness: f32,
    link_segments_per_len: f32,
    link_hover_distance: f32,
    pin_circle_radius: f32,
    pin_quad_side: f32,
    pin_triangle_side: f32,
    pin_line_thickness: f32,
    pin_hover_radius: f32,
    pin_offset: f32,
    auto_panning_speed: f32,
    color_title: [f32; 4],
    color_node_bg: [f32; 4],
    color_link: [f32; 4],
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    graph: GraphState,
    graph_b: GraphState,
    graph_shader: GraphState,
    node_options: NodeOptions,
    ini_path: String,
    minimap_hovered: Option<i32>,
    minimap_size: f32,
    minimap_location: imnodes::MiniMapLocation,
}

fn nid_to_pin_in(nid: i32) -> i32 {
    match nid {
        1 => 10,
        2 => 20,
        _ => nid * 100 + 2,
    }
}
fn nid_to_pin_out(nid: i32) -> i32 {
    match nid {
        1 => 11,
        2 => 21,
        _ => nid * 100 + 1,
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
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
                        .with_title(format!("Dear ImGui + ImNodes Example - {version}"))
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

        // Initialize the renderer with one-step initialization
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("Failed to initialize WGPU renderer");
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        // Setup ImNodes
        let nodes_context = imnodes::Context::create(&context);
        let editor_context = imnodes::EditorContext::create();
        let editor_context_b = imnodes::EditorContext::create();

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            nodes_context,
            editor_context,
            editor_context_b,
            editor_context_shader: imnodes::EditorContext::create(),
            saved_ini: None,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            last_frame: Instant::now(),
        };

        let graph = GraphState {
            next_link_id: 1,
            links: Vec::new(),
            positions_initialized: false,
            added_nodes: Vec::new(),
            next_node_id: 3,
        };
        let graph_b = GraphState {
            next_link_id: 1000,
            links: Vec::new(),
            positions_initialized: false,
            added_nodes: Vec::new(),
            next_node_id: 200,
        };
        let node_options = NodeOptions {
            show_grid: true,
            primary_lines: true,
            grid_snapping: false,
            grid_spacing: 24.0,
            link_thickness: 3.0,
            corner_round: 4.0,
            node_padding: [8.0, 8.0],
            node_border_thickness: 1.0,
            link_segments_per_len: 0.1,
            link_hover_distance: 10.0,
            pin_circle_radius: 6.0,
            pin_quad_side: 7.0,
            pin_triangle_side: 8.0,
            pin_line_thickness: 1.0,
            pin_hover_radius: 10.0,
            pin_offset: 0.0,
            auto_panning_speed: 200.0,
            color_title: [0.20, 0.40, 0.70, 1.0],
            color_node_bg: [0.15, 0.15, 0.15, 1.0],
            color_link: [0.60, 0.80, 1.00, 1.0],
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            graph,
            graph_b,
            graph_shader: GraphState {
                next_link_id: 10000,
                links: Vec::new(),
                positions_initialized: false,
                added_nodes: Vec::new(),
                next_node_id: 3000,
            },
            node_options,
            ini_path: String::from("imnodes_state.ini"),
            minimap_hovered: None,
            minimap_size: 0.25,
            minimap_location: imnodes::MiniMapLocation::BottomRight,
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

        // Build a simple node editor UI
        ui.window("ImNodes Demo")
            .size([1100.0, 800.0], Condition::FirstUseEver)
            .position([50.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                if let Some(tab_bar) = ui.tab_bar("imnodes_tabs") {
                    if let Some(tab) = ui.tab_item("Hello") {
                        let editor = ui.imnodes_editor(
                            &self.imgui.nodes_context,
                            Some(&self.imgui.editor_context),
                        );
                        editor.enable_link_detach_with_ctrl();
                        editor.enable_multiple_select_with_ctrl();
                        if !self.graph.positions_initialized {
                            editor.set_node_pos_grid(1, [100.0, 100.0]);
                            editor.set_node_pos_grid(2, [400.0, 220.0]);
                            self.graph.positions_initialized = true;
                        }

                        // Initialize positions once
                        // Node 1
                        let _n1 = editor.node(1);
                        _n1.title_bar(|| ui.text("Node A"));
                        {
                            let _in = editor.input_attr(10, imnodes::PinShape::CircleFilled);
                            ui.text("In");
                            _in.end();
                        }
                        {
                            let _out = editor.output_attr(11, imnodes::PinShape::QuadFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        _n1.end();

                        // Node 2
                        let _n2 = editor.node(2);
                        _n2.title_bar(|| ui.text("Node B"));
                        {
                            let _in = editor.input_attr(20, imnodes::PinShape::Circle);
                            ui.text("In");
                            _in.end();
                        }
                        {
                            let _out = editor.output_attr(21, imnodes::PinShape::TriangleFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        _n2.end();

                        // Added nodes rendering
                        for (nid, pending_pos) in &mut self.graph.added_nodes {
                            let _n = editor.node(*nid);
                            _n.title_bar(|| ui.text(format!("Node {}", nid)));
                            {
                                let _in = editor
                                    .input_attr(nid_to_pin_in(*nid), imnodes::PinShape::Circle);
                                ui.text("In");
                                _in.end();
                            }
                            {
                                let _out = editor.output_attr(
                                    nid_to_pin_out(*nid),
                                    imnodes::PinShape::CircleFilled,
                                );
                                ui.text("Out");
                                _out.end();
                            }
                            _n.end();
                            if let Some(pos) = pending_pos.take() {
                                editor.set_node_pos_screen(*nid, pos);
                            }
                        }

                        // Existing links (render)
                        for (id, a, b) in &self.graph.links {
                            editor.link(*id, *a, *b);
                        }

                        // Show link count only; style toggles moved to Style tab to avoid interaction conflicts
                        ui.separator();
                        ui.text(format!("Links: {}", self.graph.links.len()));
                        // Ensure NodeEditor ends before closing the TabItem to keep ImGui ID stack balanced
                        editor.end();
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Multi-Editor") {
                        let editor = ui.imnodes_editor(
                            &self.imgui.nodes_context,
                            Some(&self.imgui.editor_context_b),
                        );
                        editor.enable_link_detach_with_ctrl();
                        editor.enable_multiple_select_with_ctrl();
                        if !self.graph_b.positions_initialized {
                            editor.set_node_pos_grid(101, [180.0, 140.0]);
                            editor.set_node_pos_grid(102, [420.0, 260.0]);
                            self.graph_b.positions_initialized = true;
                        }
                        let _n1 = editor.node(101);
                        _n1.title_bar(|| ui.text("Node X"));
                        {
                            let _out = editor.output_attr(111, imnodes::PinShape::CircleFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        _n1.end();
                        let _n2 = editor.node(102);
                        _n2.title_bar(|| ui.text("Node Y"));
                        {
                            let _in = editor.input_attr(120, imnodes::PinShape::Circle);
                            ui.text("In");
                            _in.end();
                        }
                        _n2.end();
                        for (id, a, b) in &self.graph_b.links {
                            editor.link(*id, *a, *b);
                        }
                        editor.end();
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Style") {
                        ui.text("Style & Colors");
                        ui.separator();
                        ui.text("A few common style variables");
                        if ui.slider_f32(
                            "Grid Spacing",
                            &mut self.node_options.grid_spacing,
                            8.0,
                            64.0,
                        ) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_grid_spacing(self.node_options.grid_spacing);
                            editor.end();
                        }
                        if ui.slider_f32(
                            "Link Thickness",
                            &mut self.node_options.link_thickness,
                            1.0,
                            8.0,
                        ) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_link_thickness(self.node_options.link_thickness);
                            editor.end();
                        }
                        if ui.slider_f32(
                            "Node Rounding",
                            &mut self.node_options.corner_round,
                            0.0,
                            12.0,
                        ) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_node_corner_rounding(self.node_options.corner_round);
                            editor.end();
                        }
                        if ui.slider_f32(
                            "Auto Pan Speed (LMB Box)",
                            &mut self.node_options.auto_panning_speed,
                            0.0,
                            2000.0,
                        ) {
                            // 0 disables auto-panning while box selecting near edges
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_auto_panning_speed(self.node_options.auto_panning_speed);
                            editor.end();
                        }
                        if ui.slider_f32(
                            "Node Border Thickness",
                            &mut self.node_options.node_border_thickness,
                            0.0,
                            8.0,
                        ) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor
                                .set_node_border_thickness(self.node_options.node_border_thickness);
                            editor.end();
                        }
                        if ui.slider_f32(
                            "Node Padding X",
                            &mut self.node_options.node_padding[0],
                            0.0,
                            20.0,
                        ) || ui.slider_f32(
                            "Node Padding Y",
                            &mut self.node_options.node_padding[1],
                            0.0,
                            20.0,
                        ) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_node_padding(self.node_options.node_padding);
                            editor.end();
                        }

                        ui.separator();
                        ui.text("Colors (RGBA)");
                        if ui.color_edit4("TitleBar", &mut self.node_options.color_title) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_color(
                                dear_imnodes::ColorElement::TitleBar,
                                self.node_options.color_title,
                            );
                            editor.end();
                        }
                        if ui.color_edit4("Node Background", &mut self.node_options.color_node_bg) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_color(
                                dear_imnodes::ColorElement::NodeBackground,
                                self.node_options.color_node_bg,
                            );
                            editor.end();
                        }
                        if ui.color_edit4("Link", &mut self.node_options.color_link) {
                            let editor = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            editor.set_color(
                                dear_imnodes::ColorElement::Link,
                                self.node_options.color_link,
                            );
                            editor.end();
                        }

                        ui.separator();
                        ui.text("Save/Load State (file)");
                        ui.input_text("INI Path", &mut self.ini_path).build();
                        if ui.button("Save To File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            post.save_state_to_ini_file(&self.ini_path);
                        }
                        ui.same_line();
                        if ui.button("Load From File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            post.load_state_from_ini_file(&self.ini_path);
                        }
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Save/Load") {
                        ui.text("Current Editor A (string)");
                        if ui.button("Save A (string)") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            self.imgui.saved_ini = Some(post.save_state_to_ini_string());
                        }
                        ui.same_line();
                        if ui.button("Load A (string)")
                            && let Some(ref s) = self.imgui.saved_ini
                        {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            post.load_state_from_ini_string(s);
                        }
                        ui.separator();
                        ui.text("INI File path");
                        ui.input_text("Path", &mut self.ini_path).build();
                        if ui.button("Save A to File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            post.save_state_to_ini_file(&self.ini_path);
                        }
                        ui.same_line();
                        if ui.button("Load A from File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context),
                                )
                                .end();
                            post.load_state_from_ini_file(&self.ini_path);
                        }
                        ui.separator();
                        ui.text("Editor B (file)");
                        if ui.button("Save B to File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context_b),
                                )
                                .end();
                            post.save_state_to_ini_file(&self.ini_path);
                        }
                        ui.same_line();
                        if ui.button("Load B from File") {
                            let post = ui
                                .imnodes_editor(
                                    &self.imgui.nodes_context,
                                    Some(&self.imgui.editor_context_b),
                                )
                                .end();
                            post.load_state_from_ini_file(&self.ini_path);
                        }
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Color Editor") {
                        ui.text("Scoped color overrides using push/pop");
                        let editor = ui.imnodes_editor(
                            &self.imgui.nodes_context,
                            Some(&self.imgui.editor_context_b),
                        );
                        let _c_title = editor.push_color(
                            dear_imnodes::ColorElement::TitleBar,
                            [0.90, 0.50, 0.30, 1.0],
                        );
                        let _c_link = editor
                            .push_color(dear_imnodes::ColorElement::Link, [0.10, 0.80, 0.40, 1.0]);
                        let _n1 = editor.node(201);
                        _n1.title_bar(|| ui.text("Scoped Node 201"));
                        {
                            let _out = editor.output_attr(211, imnodes::PinShape::CircleFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        _n1.end();
                        let _n2 = editor.node(202);
                        _n2.title_bar(|| ui.text("Scoped Node 202"));
                        {
                            let _in = editor.input_attr(220, imnodes::PinShape::Circle);
                            ui.text("In");
                            _in.end();
                        }
                        _n2.end();
                        editor.link(5000, 211, 220);
                        editor.end();
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Advanced Style") {
                        ui.text("Link & Pin tuning");
                        if ui.slider_f32(
                            "Link Segments/Len",
                            &mut self.node_options.link_segments_per_len,
                            0.0,
                            2.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_link_line_segments_per_length(
                                self.node_options.link_segments_per_len,
                            );
                            e.end();
                        }
                        if ui.slider_f32(
                            "Link Hover Dist",
                            &mut self.node_options.link_hover_distance,
                            0.0,
                            50.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_link_hover_distance(self.node_options.link_hover_distance);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Circle Radius",
                            &mut self.node_options.pin_circle_radius,
                            1.0,
                            20.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_circle_radius(self.node_options.pin_circle_radius);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Quad Side",
                            &mut self.node_options.pin_quad_side,
                            1.0,
                            30.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_quad_side_length(self.node_options.pin_quad_side);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Triangle Side",
                            &mut self.node_options.pin_triangle_side,
                            1.0,
                            30.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_triangle_side_length(self.node_options.pin_triangle_side);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Line Thickness",
                            &mut self.node_options.pin_line_thickness,
                            0.0,
                            8.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_line_thickness(self.node_options.pin_line_thickness);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Hover Radius",
                            &mut self.node_options.pin_hover_radius,
                            0.0,
                            30.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_hover_radius(self.node_options.pin_hover_radius);
                            e.end();
                        }
                        if ui.slider_f32(
                            "Pin Offset",
                            &mut self.node_options.pin_offset,
                            -10.0,
                            10.0,
                        ) {
                            let e = ui.imnodes_editor(
                                &self.imgui.nodes_context,
                                Some(&self.imgui.editor_context),
                            );
                            e.set_pin_offset(self.node_options.pin_offset);
                            e.end();
                        }
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("Shader Graph") {
                        let editor = ui.imnodes_editor(
                            &self.imgui.nodes_context,
                            Some(&self.imgui.editor_context_shader),
                        );
                        // One-time layout and links
                        if !self.graph_shader.positions_initialized {
                            // Nodes: 3001 Texture, 3002 UV, 3003 Multiply, 3004 Add, 3005 Output
                            editor.set_node_pos_grid(3001, [100.0, 100.0]);
                            editor.set_node_pos_grid(3002, [100.0, 260.0]);
                            editor.set_node_pos_grid(3003, [360.0, 140.0]);
                            editor.set_node_pos_grid(3004, [580.0, 160.0]);
                            editor.set_node_pos_grid(3005, [820.0, 180.0]);
                            // Links initial (attr ids):
                            // 30021 (UV out) -> 30011 (Texture UV in)
                            // 30012 (Texture color out) -> 30032 (Multiply in A)
                            // 30022 (Color const out) -> 30033 (Multiply in B)
                            // 30031 (Multiply out) -> 30042 (Add in A)
                            // 30023 (Color const2 out) -> 30043 (Add in B)
                            // 30041 (Add out) -> 30052 (Output in)
                            let mut push_link = |a: i32, b: i32| {
                                let lid = self.graph_shader.next_link_id;
                                self.graph_shader.next_link_id += 1;
                                self.graph_shader.links.push((lid, a, b));
                            };
                            push_link(30021, 30011);
                            push_link(30012, 30032);
                            push_link(30022, 30033);
                            push_link(30031, 30042);
                            push_link(30023, 30043);
                            push_link(30041, 30052);
                            self.graph_shader.positions_initialized = true;
                        }
                        // Nodes
                        // UV Node (3002)
                        let n_uv = editor.node(3002);
                        n_uv.title_bar(|| ui.text("UV"));
                        {
                            let _out = editor.output_attr(30021, imnodes::PinShape::CircleFilled);
                            ui.text("UV");
                            _out.end();
                        }
                        n_uv.end();
                        // Texture Node (3001)
                        let n_tex = editor.node(3001);
                        n_tex.title_bar(|| ui.text("Texture2D"));
                        {
                            let _in = editor.input_attr(30011, imnodes::PinShape::Circle);
                            ui.text("UV");
                            _in.end();
                        }
                        {
                            let _out = editor.output_attr(30012, imnodes::PinShape::QuadFilled);
                            ui.text("Color");
                            _out.end();
                        }
                        n_tex.end();
                        // Multiply Node (3003)
                        let n_mul = editor.node(3003);
                        n_mul.title_bar(|| ui.text("Multiply"));
                        {
                            let _in = editor.input_attr(30032, imnodes::PinShape::Circle);
                            ui.text("A");
                            _in.end();
                        }
                        {
                            let _in = editor.input_attr(30033, imnodes::PinShape::Circle);
                            ui.text("B");
                            _in.end();
                        }
                        {
                            let _out = editor.output_attr(30031, imnodes::PinShape::TriangleFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        n_mul.end();
                        // Color Const nodes (3006/3007) as outputs
                        let n_c1 = editor.node(3006);
                        n_c1.title_bar(|| ui.text("ColorConst A"));
                        {
                            let _out = editor.output_attr(30022, imnodes::PinShape::QuadFilled);
                            ui.text("Color");
                            _out.end();
                        }
                        n_c1.end();
                        let n_c2 = editor.node(3007);
                        n_c2.title_bar(|| ui.text("ColorConst B"));
                        {
                            let _out = editor.output_attr(30023, imnodes::PinShape::QuadFilled);
                            ui.text("Color");
                            _out.end();
                        }
                        n_c2.end();
                        // Add Node (3004)
                        let n_add = editor.node(3004);
                        n_add.title_bar(|| ui.text("Add"));
                        {
                            let _in = editor.input_attr(30042, imnodes::PinShape::Circle);
                            ui.text("A");
                            _in.end();
                        }
                        {
                            let _in = editor.input_attr(30043, imnodes::PinShape::Circle);
                            ui.text("B");
                            _in.end();
                        }
                        {
                            let _out = editor.output_attr(30041, imnodes::PinShape::TriangleFilled);
                            ui.text("Out");
                            _out.end();
                        }
                        n_add.end();
                        // Output Node (3005)
                        let n_out = editor.node(3005);
                        n_out.title_bar(|| ui.text("Output"));
                        {
                            let _in = editor.input_attr(30052, imnodes::PinShape::Circle);
                            ui.text("Color");
                            _in.end();
                        }
                        n_out.end();

                        // Draw links
                        for (id, a, b) in &self.graph_shader.links {
                            editor.link(*id, *a, *b);
                        }

                        editor.end();
                        tab.end();
                    }
                    if let Some(tab) = ui.tab_item("MiniMap Callback") {
                        ui.text("Hover a node in the minimap to see its id.");
                        ui.slider_f32("MiniMap Size", &mut self.minimap_size, 0.1, 0.5);
                        if let Some(_combo) = ui.begin_combo(
                            "Location",
                            match self.minimap_location {
                                imnodes::MiniMapLocation::BottomLeft => "BottomLeft",
                                imnodes::MiniMapLocation::BottomRight => "BottomRight",
                                imnodes::MiniMapLocation::TopLeft => "TopLeft",
                                imnodes::MiniMapLocation::TopRight => "TopRight",
                            },
                        ) {
                            if ui.selectable_config("BottomLeft").build() {
                                self.minimap_location = imnodes::MiniMapLocation::BottomLeft;
                            }
                            if ui.selectable_config("BottomRight").build() {
                                self.minimap_location = imnodes::MiniMapLocation::BottomRight;
                            }
                            if ui.selectable_config("TopLeft").build() {
                                self.minimap_location = imnodes::MiniMapLocation::TopLeft;
                            }
                            if ui.selectable_config("TopRight").build() {
                                self.minimap_location = imnodes::MiniMapLocation::TopRight;
                            }
                        }
                        // Draw Editor A minimap with a callback capturing hovered node id
                        let editor = ui.imnodes_editor(
                            &self.imgui.nodes_context,
                            Some(&self.imgui.editor_context),
                        );
                        {
                            let mut cb = |node_id: i32| {
                                self.minimap_hovered = Some(node_id);
                            };
                            editor.minimap_with_callback(
                                self.minimap_size,
                                self.minimap_location,
                                &mut cb,
                            );
                        }
                        editor.end();
                        match self.minimap_hovered {
                            Some(id) => ui.text(format!("Hovered node: {}", id)),
                            None => ui.text("Hovered node: (none)"),
                        }
                        tab.end();
                    }
                    tab_bar.end();
                }

                // Minimap
                // Use A context for minimap
                let editor_a =
                    ui.imnodes_editor(&self.imgui.nodes_context, Some(&self.imgui.editor_context));
                editor_a.minimap(self.minimap_size, self.minimap_location);
                let post = editor_a.end();
                // Selection counts (Editor A)
                let sel_nodes = post.selected_nodes();
                let sel_links = post.selected_links();
                ui.text(format!(
                    "Selected: {} nodes, {} links",
                    sel_nodes.len(),
                    sel_links.len()
                ));

                // New link event (must be after EndNodeEditor)
                if let Some(link) = post.is_link_created() {
                    self.graph.links.push((
                        self.graph.next_link_id,
                        link.start_attr,
                        link.end_attr,
                    ));
                    self.graph.next_link_id += 1;
                }
                if let Some(link_id) = post.is_link_destroyed() {
                    self.graph.links.retain(|(id, _, _)| *id != link_id);
                }
                if let Some(start_attr) = post.is_link_dropped(true) {
                    ui.text(format!(
                        "Link dropped (including detached) from attr #{start_attr}"
                    ));
                }

                // Controls that require EndNodeEditor
                if ui.button("Delete Selected Links") {
                    let selected = post.selected_links();
                    self.graph.links.retain(|(id, _, _)| !selected.contains(id));
                    post.clear_selection();
                }
                ui.same_line();
                if ui.button("Clear Selection") {
                    post.clear_selection();
                }

                // Right-click context menu on Editor A
                if post.is_editor_hovered() && ui.get_mouse_clicked_count(MouseButton::Right) > 0 {
                    ui.open_popup("editor_a_ctx");
                }
                ui.popup("editor_a_ctx", || {
                    let hovered_link = post.hovered_link();
                    let hovered_node = post.hovered_node();
                    if let Some(lid) = hovered_link {
                        if ui.selectable_config(&format!("Delete Link #{lid}")).build() {
                            self.graph.links.retain(|(id, _, _)| *id != lid);
                            ui.close_current_popup();
                        }
                    } else if let Some(nid) = hovered_node {
                        if ui.selectable_config(&format!("Delete Node #{nid}")).build() {
                            // Remove links attached to this node (by attribute ids)
                            let pin_in = nid_to_pin_in(nid);
                            let pin_out = nid_to_pin_out(nid);
                            self.graph.links.retain(|(_, a, b)| {
                                *a != pin_in && *a != pin_out && *b != pin_in && *b != pin_out
                            });
                            // Remove from added nodes if present
                            self.graph.added_nodes.retain(|(id, _)| *id != nid);
                            ui.close_current_popup();
                        }
                    } else if ui.selectable_config("Add Node Here").build() {
                        let pos = ui.get_mouse_pos_on_opening_current_popup();
                        let nid = self.graph.next_node_id;
                        self.graph.next_node_id += 1;
                        self.graph.added_nodes.push((nid, Some(pos)));
                        ui.close_current_popup();
                    }
                });
            });

        // Render
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            let draw_data = self.imgui.context.render();
            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut render_pass)?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(window) = &mut self.window {
            let winit_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                window_id: id,
                event: event.clone(),
            };
            window.imgui.platform.handle_event(
                &mut window.imgui.context,
                &window.window,
                &winit_event,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
