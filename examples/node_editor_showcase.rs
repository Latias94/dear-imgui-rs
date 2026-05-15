//! Blueprints-style imgui-node-editor example for the `dear-node-editor` safe API.

#[path = "support/node_editor_blueprint.rs"]
mod node_editor_blueprint;
#[path = "support/wgpu_init.rs"]
mod wgpu_init;

use dear_imgui_rs::*;
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use dear_node_editor::{
    CanvasSizeMode, EditorConfig, EditorContext, FlowDirection, LinkId, NodeEditorFrame,
    NodeEditorUiExt, NodeId, PinId, PinKind, StyleColor,
};
use node_editor_blueprint::{BlueprintNodeBuilder, IconType};
use std::{ffi::c_void, path::PathBuf, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const COMMENT_NODE: NodeId = NodeId::new(9000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeKind {
    Blueprint,
    Simple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinType {
    Flow,
    Bool,
    Float,
    String,
    Object,
    Delegate,
}

impl PinType {
    fn color(self) -> [f32; 4] {
        match self {
            Self::Flow => [0.92, 0.94, 0.96, 1.0],
            Self::Bool => [0.86, 0.24, 0.24, 1.0],
            Self::Float => [0.55, 0.84, 0.32, 1.0],
            Self::String => [0.72, 0.36, 0.86, 1.0],
            Self::Object => [0.30, 0.62, 0.92, 1.0],
            Self::Delegate => [1.00, 0.72, 0.28, 1.0],
        }
    }

    fn short_name(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Bool => "bool",
            Self::Float => "float",
            Self::String => "string",
            Self::Object => "object",
            Self::Delegate => "delegate",
        }
    }

    fn icon_type(self) -> IconType {
        match self {
            Self::Flow => IconType::Flow,
            Self::Bool => IconType::Circle,
            Self::Float => IconType::Circle,
            Self::String => IconType::Square,
            Self::Object => IconType::Grid,
            Self::Delegate => IconType::Square,
        }
    }
}

#[derive(Clone, Copy)]
struct PinSpec {
    id: PinId,
    node: NodeId,
    name: &'static str,
    kind: PinKind,
    ty: PinType,
}

#[derive(Clone, Copy)]
struct NodeSpec {
    id: NodeId,
    name: &'static str,
    kind: NodeKind,
    color: [f32; 4],
    position: [f32; 2],
    inputs: &'static [PinSpec],
    outputs: &'static [PinSpec],
}

#[derive(Clone, Copy)]
struct Link {
    id: LinkId,
    start_pin: PinId,
    end_pin: PinId,
}

#[derive(Clone, Copy, Debug)]
enum ContextPopup {
    Node(NodeId),
    Pin(PinId),
    Link(LinkId),
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StylePreset {
    Blueprint,
    Compact,
}

struct GraphState {
    links: Vec<Link>,
    next_link_id: usize,
    positions_initialized: bool,
    pending_fit_content: bool,
    pending_select_flow: bool,
    shortcuts_enabled: bool,
    show_inspector: bool,
    animate_flow: bool,
    style_preset: StylePreset,
    context_popup: Option<ContextPopup>,
    last_action: String,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            links: vec![
                Link {
                    id: LinkId::new(1000),
                    start_pin: PIN_ON_PRESSED,
                    end_pin: PIN_BRANCH_EXEC,
                },
                Link {
                    id: LinkId::new(1001),
                    start_pin: PIN_CONDITION,
                    end_pin: PIN_BRANCH_CONDITION,
                },
                Link {
                    id: LinkId::new(1002),
                    start_pin: PIN_BRANCH_TRUE,
                    end_pin: PIN_PRINT_EXEC,
                },
                Link {
                    id: LinkId::new(1003),
                    start_pin: PIN_PRINT_THEN,
                    end_pin: PIN_OUTPUT_EXEC,
                },
            ],
            next_link_id: 1004,
            positions_initialized: false,
            pending_fit_content: false,
            pending_select_flow: false,
            shortcuts_enabled: true,
            show_inspector: true,
            animate_flow: false,
            style_preset: StylePreset::Blueprint,
            context_popup: None,
            last_action: "Ready".to_owned(),
        }
    }
}

const NODE_INPUT: NodeId = NodeId::new(1);
const NODE_BRANCH: NodeId = NodeId::new(2);
const NODE_PRINT: NodeId = NodeId::new(3);
const NODE_OUTPUT: NodeId = NodeId::new(4);
const NODE_CONDITION: NodeId = NodeId::new(5);
const NODE_DELAY: NodeId = NodeId::new(6);

const PIN_ACTION: PinId = PinId::new(101);
const PIN_ON_PRESSED: PinId = PinId::new(102);
const PIN_BRANCH_EXEC: PinId = PinId::new(201);
const PIN_BRANCH_CONDITION: PinId = PinId::new(202);
const PIN_BRANCH_TRUE: PinId = PinId::new(203);
const PIN_BRANCH_FALSE: PinId = PinId::new(204);
const PIN_PRINT_EXEC: PinId = PinId::new(301);
const PIN_PRINT_MESSAGE: PinId = PinId::new(302);
const PIN_PRINT_THEN: PinId = PinId::new(303);
const PIN_OUTPUT_EXEC: PinId = PinId::new(401);
const PIN_CONDITION: PinId = PinId::new(501);
const PIN_DELAY_EXEC: PinId = PinId::new(601);
const PIN_DELAY_DURATION: PinId = PinId::new(602);
const PIN_DELAY_THEN: PinId = PinId::new(603);

const INPUT_OUTPUTS: [PinSpec; 2] = [
    PinSpec {
        id: PIN_ACTION,
        node: NODE_INPUT,
        name: "Action",
        kind: PinKind::Output,
        ty: PinType::String,
    },
    PinSpec {
        id: PIN_ON_PRESSED,
        node: NODE_INPUT,
        name: "Pressed",
        kind: PinKind::Output,
        ty: PinType::Flow,
    },
];
const BRANCH_INPUTS: [PinSpec; 2] = [
    PinSpec {
        id: PIN_BRANCH_EXEC,
        node: NODE_BRANCH,
        name: "",
        kind: PinKind::Input,
        ty: PinType::Flow,
    },
    PinSpec {
        id: PIN_BRANCH_CONDITION,
        node: NODE_BRANCH,
        name: "Condition",
        kind: PinKind::Input,
        ty: PinType::Bool,
    },
];
const BRANCH_OUTPUTS: [PinSpec; 2] = [
    PinSpec {
        id: PIN_BRANCH_TRUE,
        node: NODE_BRANCH,
        name: "True",
        kind: PinKind::Output,
        ty: PinType::Flow,
    },
    PinSpec {
        id: PIN_BRANCH_FALSE,
        node: NODE_BRANCH,
        name: "False",
        kind: PinKind::Output,
        ty: PinType::Flow,
    },
];
const PRINT_INPUTS: [PinSpec; 2] = [
    PinSpec {
        id: PIN_PRINT_EXEC,
        node: NODE_PRINT,
        name: "",
        kind: PinKind::Input,
        ty: PinType::Flow,
    },
    PinSpec {
        id: PIN_PRINT_MESSAGE,
        node: NODE_PRINT,
        name: "Message",
        kind: PinKind::Input,
        ty: PinType::String,
    },
];
const PRINT_OUTPUTS: [PinSpec; 1] = [PinSpec {
    id: PIN_PRINT_THEN,
    node: NODE_PRINT,
    name: "Then",
    kind: PinKind::Output,
    ty: PinType::Flow,
}];
const OUTPUT_INPUTS: [PinSpec; 1] = [PinSpec {
    id: PIN_OUTPUT_EXEC,
    node: NODE_OUTPUT,
    name: "",
    kind: PinKind::Input,
    ty: PinType::Flow,
}];
const CONDITION_OUTPUTS: [PinSpec; 1] = [PinSpec {
    id: PIN_CONDITION,
    node: NODE_CONDITION,
    name: "Value",
    kind: PinKind::Output,
    ty: PinType::Bool,
}];
const DELAY_INPUTS: [PinSpec; 2] = [
    PinSpec {
        id: PIN_DELAY_EXEC,
        node: NODE_DELAY,
        name: "",
        kind: PinKind::Input,
        ty: PinType::Flow,
    },
    PinSpec {
        id: PIN_DELAY_DURATION,
        node: NODE_DELAY,
        name: "Duration",
        kind: PinKind::Input,
        ty: PinType::Float,
    },
];
const DELAY_OUTPUTS: [PinSpec; 1] = [PinSpec {
    id: PIN_DELAY_THEN,
    node: NODE_DELAY,
    name: "Then",
    kind: PinKind::Output,
    ty: PinType::Flow,
}];
const NO_PINS: [PinSpec; 0] = [];

const NODES: [NodeSpec; 6] = [
    NodeSpec {
        id: NODE_INPUT,
        name: "InputAction",
        kind: NodeKind::Blueprint,
        color: [0.24, 0.52, 0.86, 1.0],
        position: [-280.0, 160.0],
        inputs: &NO_PINS,
        outputs: &INPUT_OUTPUTS,
    },
    NodeSpec {
        id: NODE_BRANCH,
        name: "Branch",
        kind: NodeKind::Blueprint,
        color: [0.70, 0.44, 0.90, 1.0],
        position: [40.0, 180.0],
        inputs: &BRANCH_INPUTS,
        outputs: &BRANCH_OUTPUTS,
    },
    NodeSpec {
        id: NODE_PRINT,
        name: "Print String",
        kind: NodeKind::Blueprint,
        color: [0.88, 0.56, 0.24, 1.0],
        position: [360.0, 150.0],
        inputs: &PRINT_INPUTS,
        outputs: &PRINT_OUTPUTS,
    },
    NodeSpec {
        id: NODE_OUTPUT,
        name: "OutputAction",
        kind: NodeKind::Simple,
        color: [0.26, 0.72, 0.45, 1.0],
        position: [690.0, 168.0],
        inputs: &OUTPUT_INPUTS,
        outputs: &NO_PINS,
    },
    NodeSpec {
        id: NODE_CONDITION,
        name: "Can Jump",
        kind: NodeKind::Simple,
        color: [0.56, 0.78, 0.26, 1.0],
        position: [-190.0, 360.0],
        inputs: &NO_PINS,
        outputs: &CONDITION_OUTPUTS,
    },
    NodeSpec {
        id: NODE_DELAY,
        name: "Delay",
        kind: NodeKind::Blueprint,
        color: [0.32, 0.67, 0.76, 1.0],
        position: [360.0, 350.0],
        inputs: &DELAY_INPUTS,
        outputs: &DELAY_OUTPUTS,
    },
];

struct ImguiState {
    #[allow(dead_code)]
    registered_user_textures: Vec<RegisteredUserTexture>,
    node_editor: EditorContext,
    renderer: WgpuRenderer,
    platform: WinitPlatform,
    context: Context,
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
    header_background: texture::OwnedTextureData,
    graph: GraphState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Dear ImGui Node Editor Blueprints")
                    .with_inner_size(LogicalSize::new(1360.0, 820.0)),
            )?,
        );

        let (device, queue, surface, surface_desc) = wgpu_init::init_wgpu_for_window(&window)?;

        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);

        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("failed to initialize WGPU renderer");
        renderer.set_gamma_mode(GammaMode::Auto);

        let config = EditorConfig::new()
            .no_settings_file()
            .canvas_size_mode(CanvasSizeMode::FitVerticalView)
            .custom_zoom_levels(vec![0.5, 0.75, 1.0, 1.25, 1.5, 2.0])
            .drag_button(MouseButton::Left)
            .select_button(MouseButton::Left)
            .navigate_button(MouseButton::Right)
            .context_menu_button(MouseButton::Right)
            .smooth_zoom(true, 1.18);
        let node_editor = EditorContext::create_with_config(&context, config);
        apply_style_preset(&node_editor, StylePreset::Blueprint);

        let mut header_background = load_header_background_texture()?;
        let registered_user_textures =
            vec![context.register_user_texture_token(&mut header_background)];

        let imgui = ImguiState {
            registered_user_textures,
            node_editor,
            renderer,
            platform,
            context,
            clear_color: wgpu::Color {
                r: 0.075,
                g: 0.080,
                b: 0.086,
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
            header_background,
            graph: GraphState::default(),
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        wgpu_init::reconfigure_surface(
            &self.surface,
            &self.device,
            &mut self.surface_desc,
            new_size,
        );
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

        draw_blueprints_window(
            &ui,
            &self.imgui.node_editor,
            &mut self.header_background,
            &mut self.graph,
        );

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        let (output, reconfigure_after_present) = match self.surface.get_current_texture() {
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

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Node Editor Blueprints Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Node Editor Blueprints Render Pass"),
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
                multiview_mask: None,
            });
            self.imgui
                .renderer
                .render_draw_data(draw_data, &mut render_pass)?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
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
                Ok(window) => self.window = Some(window),
                Err(e) => {
                    eprintln!("failed to create window: {e}");
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
        if let Some(window) = &mut self.window {
            if window.window.id() != window_id {
                return;
            }

            window.imgui.platform.handle_window_event(
                &mut window.imgui.context,
                &window.window,
                &event,
            );

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(new_size) => {
                    window.resize(new_size);
                    window.window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    window.resize(window.window.inner_size());
                    window.window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    if let Err(e) = window.render() {
                        eprintln!("render error: {e}");
                    }
                    window.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn load_header_background_texture() -> Result<texture::OwnedTextureData, Box<dyn std::error::Error>>
{
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("examples crate has workspace parent")
        .join("extensions/dear-node-editor-sys/third-party/cimnodes_editor/imgui-node-editor/examples/blueprints-example/data/BlueprintBackground.png");

    let image = ::image::ImageReader::open(&path)?.decode()?.to_rgba8();
    let (width, height) = image.dimensions();
    let mut texture = texture::TextureData::new();
    texture.create(
        texture::TextureFormat::RGBA32,
        i32::try_from(width).expect("header texture width fits i32"),
        i32::try_from(height).expect("header texture height fits i32"),
    );
    texture.set_data(image.as_raw());
    texture.set_status(texture::TextureStatus::WantCreate);
    Ok(texture)
}

fn draw_blueprints_window(
    ui: &Ui,
    node_editor: &EditorContext,
    header_background: &mut texture::OwnedTextureData,
    graph: &mut GraphState,
) {
    ui.window("Blueprints Example")
        .size([1260.0, 760.0], Condition::FirstUseEver)
        .position([32.0, 32.0], Condition::FirstUseEver)
        .build(|| {
            ui.group(|| {
                draw_left_pane(ui, node_editor, graph);
            });

            ui.same_line_with_spacing(0.0, 12.0);

            ui.group(|| {
                draw_editor_pane(ui, node_editor, header_background, graph);
            });
        });
}

fn draw_left_pane(ui: &Ui, node_editor: &EditorContext, graph: &mut GraphState) {
    ui.child_window("blueprints_left_pane")
        .size([300.0, 0.0])
        .border(true)
        .build(ui, || {
            ui.text("Blueprints");
            ui.separator();
            if ui.button("Fit Content") {
                graph.pending_fit_content = true;
                graph.last_action = "Queued fit content".to_owned();
            }
            ui.same_line();
            if ui.button("Select Flow") {
                graph.pending_select_flow = true;
                graph.last_action = "Queued flow selection".to_owned();
            }

            ui.checkbox("Shortcuts", &mut graph.shortcuts_enabled);
            ui.same_line();
            ui.checkbox("Flow", &mut graph.animate_flow);
            ui.checkbox("Inspector", &mut graph.show_inspector);

            let mut preset_changed = false;
            if ui.radio_button("Blueprint", graph.style_preset == StylePreset::Blueprint) {
                graph.style_preset = StylePreset::Blueprint;
                preset_changed = true;
            }
            ui.same_line();
            if ui.radio_button("Compact", graph.style_preset == StylePreset::Compact) {
                graph.style_preset = StylePreset::Compact;
                preset_changed = true;
            }
            if preset_changed {
                apply_style_preset(node_editor, graph.style_preset);
            }

            ui.separator_with_text("Pins");
            for ty in [
                PinType::Flow,
                PinType::Bool,
                PinType::Float,
                PinType::String,
                PinType::Object,
                PinType::Delegate,
            ] {
                ui.text_colored(ty.color(), format!("{} pin", ty.short_name()));
            }

            ui.separator_with_text("State");
            ui.text(format!("Links: {}", graph.links.len()));
            ui.text_wrapped(format!("Last action: {}", graph.last_action));
        });
}

fn draw_editor_pane(
    ui: &Ui,
    node_editor: &EditorContext,
    header_background: &mut texture::OwnedTextureData,
    graph: &mut GraphState,
) {
    let editor = ui.node_editor(node_editor, "blueprints_editor", [0.0, 610.0]);
    editor.set_shortcuts_enabled(graph.shortcuts_enabled);

    if !graph.positions_initialized {
        initialize_graph_layout(&editor);
        graph.positions_initialized = true;
    }
    if graph.pending_select_flow {
        editor.clear_selection();
        for node in [NODE_INPUT, NODE_BRANCH, NODE_PRINT, NODE_OUTPUT] {
            editor.add_node_to_selection(node);
        }
        for link in &graph.links {
            if let Some(start) = pin_spec(link.start_pin) {
                if start.ty == PinType::Flow {
                    editor.add_link_to_selection(link.id);
                }
            }
        }
        graph.pending_select_flow = false;
        graph.last_action = "Selected flow path".to_owned();
    }
    if graph.pending_fit_content {
        editor.navigate_to_content(0.35);
        graph.pending_fit_content = false;
        graph.last_action = "Navigated to content".to_owned();
    }

    for node in &NODES {
        draw_blueprint_node(&editor, ui, graph, node, header_background);
    }
    draw_comment_node(&editor, ui);

    for link in &graph.links {
        let color = pin_spec(link.start_pin)
            .map(|pin| pin.ty.color())
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        editor.link_colored(link.id, link.start_pin, link.end_pin, color, 2.0);
        if graph.animate_flow {
            editor.flow(link.id, FlowDirection::Forward);
        }
    }

    draw_comment_hint(&editor);
    handle_create_session(&editor, graph);
    handle_delete_session(&editor, graph);
    handle_shortcuts(&editor, graph);
    handle_context_popups(ui, &editor, graph);
    let summary = collect_summary(&editor, ui, graph);
    editor.end();

    if graph.show_inspector {
        ui.separator();
        draw_inspector(ui, &summary, graph);
    }
}

fn initialize_graph_layout(editor: &NodeEditorFrame<'_>) {
    for node in &NODES {
        editor.set_node_position(node.id, node.position);
        editor.set_node_z_position(node.id, 0.0);
    }
    editor.set_node_position(COMMENT_NODE, [-320.0, 118.0]);
    editor.set_group_size(COMMENT_NODE, [1130.0, 380.0]);
    editor.set_node_z_position(COMMENT_NODE, -1.0);
    editor.navigate_to_content(0.0);
}

fn draw_comment_node(editor: &NodeEditorFrame<'_>, ui: &Ui) {
    let _alpha = ui.push_style_var(StyleVar::Alpha(0.75));
    let _node_bg =
        editor.push_style_color(StyleColor::NodeBackground, [1.0, 1.0, 1.0, 64.0 / 255.0]);
    let _node_border =
        editor.push_style_color(StyleColor::NodeBorder, [1.0, 1.0, 1.0, 64.0 / 255.0]);

    let node = editor.begin_node(COMMENT_NODE);
    let _id = ui.push_id(COMMENT_NODE.raw() as *const c_void);
    let content = ui.begin_vertical("content", [0.0, 0.0], -1.0);
    let horizontal = ui.begin_horizontal("horizontal", [0.0, 0.0], -1.0);
    ui.spring(1.0, -1.0);
    ui.text("Input Action Pipeline");
    ui.spring(1.0, -1.0);
    horizontal.end();
    editor.group([1130.0, 380.0]);
    content.end();
    node.end();
}

fn draw_blueprint_node(
    editor: &NodeEditorFrame<'_>,
    ui: &Ui,
    graph: &mut GraphState,
    node: &NodeSpec,
    header_background: &mut texture::OwnedTextureData,
) {
    let _node_bg = editor.push_style_color(
        StyleColor::NodeBackground,
        [
            node.color[0] * 0.13,
            node.color[1] * 0.13,
            node.color[2] * 0.13,
            0.96,
        ],
    );
    let _node_border = editor.push_style_color(StyleColor::NodeBorder, node.color);

    let is_simple = node.kind == NodeKind::Simple;
    let mut builder = BlueprintNodeBuilder::begin(editor, ui, node.id);

    if !is_simple {
        builder.header(node.color, || {
            ui.spring(0.0, -1.0);
            ui.text(node.name);
            ui.spring(1.0, -1.0);
            ui.dummy([0.0, 28.0]);
            ui.spring(0.0, -1.0);
        });
        builder.end_header();
    }

    for input in node.inputs {
        builder.input(input.id, |pin_token| {
            draw_pin_icon(ui, graph, *input);
            ui.spring(0.0, -1.0);
            if !input.name.is_empty() {
                ui.text(input.name);
                ui.spring(0.0, -1.0);
            }
            pin_token.pivot_size([0.0, 0.0]);
        });
    }

    if is_simple {
        builder.middle(|| {
            ui.spring(1.0, 0.0);
            ui.text(node.name);
            ui.spring(1.0, 0.0);
        });
    }

    for output in node.outputs {
        builder.output(output.id, |pin_token| {
            if !output.name.is_empty() {
                ui.spring(0.0, -1.0);
                ui.text(output.name);
            }
            ui.spring(0.0, -1.0);
            draw_pin_icon(ui, graph, *output);
            pin_token.pivot_size([0.0, 0.0]);
        });
    }

    builder.end(header_background);
}

fn pin_is_connected(graph: &GraphState, pin: PinId) -> bool {
    graph
        .links
        .iter()
        .any(|link| link.start_pin == pin || link.end_pin == pin)
}

fn draw_pin_icon(ui: &Ui, graph: &GraphState, pin: PinSpec) {
    let color = pin.ty.color();
    let alpha = ui.clone_style().alpha();
    node_editor_blueprint::icon(
        ui,
        [24.0, 24.0],
        pin.ty.icon_type(),
        pin_is_connected(graph, pin.id),
        [color[0], color[1], color[2], alpha],
        [0.08, 0.08, 0.08, alpha],
    );
}

fn draw_comment_hint(editor: &NodeEditorFrame<'_>) {
    if let Some(hint) = editor.begin_group_hint(COMMENT_NODE) {
        let min = hint.min();
        let label_min = [min[0] + 12.0, min[1] - 24.0];
        let label_max = [label_min[0] + 176.0, label_min[1] + 22.0];
        hint.background_draw_list()
            .add_rect(label_min, label_max, [0.12, 0.14, 0.16, 0.88])
            .filled(true)
            .rounding(4.0)
            .build();
        hint.foreground_draw_list().add_text(
            [label_min[0] + 8.0, label_min[1] + 4.0],
            [0.72, 0.83, 0.92, 1.0],
            "Blueprint comment",
        );
        hint.end();
    }
}

fn handle_create_session(editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    if let Some(create) = editor.begin_create([1.0, 1.0, 1.0, 1.0], 2.0) {
        if let Some((a, b)) = create.query_new_link_styled([0.55, 0.82, 1.0, 1.0], 2.0) {
            match normalize_link(a, b) {
                Some((start_pin, end_pin))
                    if !graph
                        .links
                        .iter()
                        .any(|link| link.start_pin == start_pin && link.end_pin == end_pin) =>
                {
                    if create.accept_new_item_styled([0.35, 0.92, 0.55, 1.0], 3.0) {
                        graph.links.push(Link {
                            id: LinkId::new(graph.next_link_id),
                            start_pin,
                            end_pin,
                        });
                        graph.next_link_id += 1;
                        graph.last_action =
                            format!("Created link {} -> {}", start_pin.raw(), end_pin.raw());
                    }
                }
                _ => create.reject_new_item_styled([0.95, 0.24, 0.25, 1.0], 2.0),
            }
        } else if let Some(pin) = create.query_new_node_styled([0.95, 0.65, 0.24, 1.0], 2.0) {
            create.reject_new_item_styled([0.95, 0.65, 0.24, 1.0], 2.0);
            graph.last_action = format!("New-node request from pin {} rejected", pin.raw());
        }
    }
}

fn handle_delete_session(editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    if let Some(delete) = editor.begin_delete() {
        while let Some((link_id, _, _)) = delete.query_deleted_link() {
            if delete.accept_deleted_item(true) {
                graph.links.retain(|link| link.id != link_id);
                graph.last_action = format!("Deleted link {}", link_id.raw());
            }
        }

        while let Some(node_id) = delete.query_deleted_node() {
            delete.reject_deleted_item();
            graph.last_action = format!("Rejected node delete for {}", node_id.raw());
        }
    }
}

fn handle_shortcuts(editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    if let Some(shortcut) = editor.begin_shortcut() {
        if shortcut.accept_copy() {
            graph.last_action = format!(
                "Copy shortcut on {} nodes and {} links",
                shortcut.action_context_nodes().len(),
                shortcut.action_context_links().len()
            );
        }
        if shortcut.accept_cut() {
            graph.last_action = "Cut shortcut rejected by demo graph".to_owned();
        }
        if shortcut.accept_duplicate() {
            graph.last_action = "Duplicate shortcut acknowledged".to_owned();
        }
        if shortcut.accept_create_node() {
            graph.last_action = "Create-node shortcut acknowledged".to_owned();
        }
        if shortcut.accept_paste() {
            graph.last_action = "Paste shortcut acknowledged".to_owned();
        }
    }
}

fn handle_context_popups(ui: &Ui, editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    let suspension = editor.suspend();
    if let Some(node) = editor.show_node_context_menu() {
        graph.context_popup = Some(ContextPopup::Node(node));
        ui.open_popup("Blueprint Context");
    } else if let Some(pin) = editor.show_pin_context_menu() {
        graph.context_popup = Some(ContextPopup::Pin(pin));
        ui.open_popup("Blueprint Context");
    } else if let Some(link) = editor.show_link_context_menu() {
        graph.context_popup = Some(ContextPopup::Link(link));
        ui.open_popup("Blueprint Context");
    } else if editor.show_background_context_menu() {
        graph.context_popup = Some(ContextPopup::Background);
        ui.open_popup("Blueprint Context");
    }

    draw_context_popup(ui, graph);
    debug_assert!(editor.is_suspended());
    suspension.resume();
}

fn draw_context_popup(ui: &Ui, graph: &mut GraphState) {
    if let Some(_popup) = ui.begin_popup("Blueprint Context") {
        match graph.context_popup {
            Some(ContextPopup::Node(node)) => {
                ui.text(format!("Node {}", node.raw()));
                if ui.menu_item("Select") {
                    graph.last_action = format!("Context selected node {}", node.raw());
                    ui.close_current_popup();
                }
            }
            Some(ContextPopup::Pin(pin)) => {
                ui.text(format!("Pin {}", pin.raw()));
                if let Some(spec) = pin_spec(pin) {
                    ui.text_disabled(format!(
                        "{} {}",
                        spec.ty.short_name(),
                        match spec.kind {
                            PinKind::Input => "input",
                            PinKind::Output => "output",
                        }
                    ));
                }
                if ui.menu_item("Break Links") {
                    graph
                        .links
                        .retain(|link| link.start_pin != pin && link.end_pin != pin);
                    graph.last_action = format!("Removed model links for pin {}", pin.raw());
                    ui.close_current_popup();
                }
            }
            Some(ContextPopup::Link(link)) => {
                ui.text(format!("Link {}", link.raw()));
                if ui.menu_item("Delete") {
                    graph.links.retain(|entry| entry.id != link);
                    graph.last_action = format!("Deleted link {}", link.raw());
                    ui.close_current_popup();
                }
            }
            Some(ContextPopup::Background) => {
                ui.text("Background");
                if ui.menu_item("Fit Content") {
                    graph.positions_initialized = false;
                    graph.last_action = "Requested fit content".to_owned();
                    ui.close_current_popup();
                }
            }
            None => ui.text("No context"),
        }
    }

    if !ui.is_popup_open("Blueprint Context") {
        graph.context_popup = None;
    }
}

struct FrameSummary {
    selected_nodes: Vec<NodeId>,
    selected_links: Vec<LinkId>,
    hovered_node: Option<NodeId>,
    hovered_pin: Option<PinId>,
    hovered_link: Option<LinkId>,
    double_clicked_node: Option<NodeId>,
    zoom: f32,
    screen_size: [f32; 2],
    mouse_canvas: Option<[f32; 2]>,
}

fn collect_summary(editor: &NodeEditorFrame<'_>, ui: &Ui, _graph: &GraphState) -> FrameSummary {
    FrameSummary {
        selected_nodes: editor.selected_nodes(),
        selected_links: editor.selected_links(),
        hovered_node: editor.hovered_node(),
        hovered_pin: editor.hovered_pin(),
        hovered_link: editor.hovered_link(),
        double_clicked_node: editor.double_clicked_node(),
        zoom: editor.current_zoom(),
        screen_size: editor.screen_size(),
        mouse_canvas: ui
            .is_mouse_pos_valid()
            .then(|| editor.screen_to_canvas(ui.mouse_pos())),
    }
}

fn draw_inspector(ui: &Ui, summary: &FrameSummary, graph: &GraphState) {
    ui.text(format!(
        "Selected nodes [{}] | selected links [{}]",
        format_node_ids(&summary.selected_nodes),
        format_link_ids(&summary.selected_links)
    ));
    ui.text(format!(
        "Hovered node/pin/link: {:?} / {:?} / {:?} | double-clicked node: {:?}",
        summary.hovered_node.map(NodeId::raw),
        summary.hovered_pin.map(PinId::raw),
        summary.hovered_link.map(LinkId::raw),
        summary.double_clicked_node.map(NodeId::raw)
    ));
    let mouse_canvas = summary
        .mouse_canvas
        .map(|pos| format!("{:.1}, {:.1}", pos[0], pos[1]))
        .unwrap_or_else(|| "n/a".to_owned());
    ui.text(format!(
        "Links: {} | zoom: {:.2} | screen: {:.0} x {:.0} | mouse canvas: {}",
        graph.links.len(),
        summary.zoom,
        summary.screen_size[0],
        summary.screen_size[1],
        mouse_canvas
    ));
    ui.text(format!("Last action: {}", graph.last_action));
}

fn apply_style_preset(editor: &EditorContext, preset: StylePreset) {
    let mut style = editor.style();
    match preset {
        StylePreset::Blueprint => {
            style.node_rounding = 7.0;
            style.group_rounding = 8.0;
            style.pin_rounding = 4.0;
            style.link_strength = 110.0;
            style.flow_speed = 120.0;
            style.flow_duration = 1.2;
            style.set_color(StyleColor::Background, [0.08, 0.09, 0.10, 1.0]);
            style.set_color(StyleColor::Grid, [0.24, 0.28, 0.31, 0.42]);
            style.set_color(StyleColor::GroupBackground, [0.10, 0.12, 0.14, 0.42]);
            style.set_color(StyleColor::GroupBorder, [0.28, 0.34, 0.38, 0.60]);
            style.set_color(StyleColor::SelectedNodeBorder, [0.36, 0.78, 1.0, 1.0]);
        }
        StylePreset::Compact => {
            style.node_rounding = 3.0;
            style.group_rounding = 4.0;
            style.pin_rounding = 2.0;
            style.link_strength = 72.0;
            style.flow_speed = 90.0;
            style.flow_duration = 1.8;
            style.set_color(StyleColor::Background, [0.10, 0.10, 0.09, 1.0]);
            style.set_color(StyleColor::Grid, [0.30, 0.27, 0.20, 0.34]);
            style.set_color(StyleColor::GroupBackground, [0.16, 0.14, 0.10, 0.44]);
            style.set_color(StyleColor::GroupBorder, [0.38, 0.33, 0.22, 0.62]);
            style.set_color(StyleColor::SelectedNodeBorder, [0.95, 0.68, 0.24, 1.0]);
        }
    }
    editor.set_style(&style);
    editor.set_style_color(
        StyleColor::Flow,
        style.color(StyleColor::SelectedNodeBorder),
    );
}

fn normalize_link(a: PinId, b: PinId) -> Option<(PinId, PinId)> {
    let a = pin_spec(a)?;
    let b = pin_spec(b)?;
    if !can_create_link(a, b) {
        return None;
    }
    match (a.kind, b.kind) {
        (PinKind::Output, PinKind::Input) => Some((a.id, b.id)),
        (PinKind::Input, PinKind::Output) => Some((b.id, a.id)),
        _ => None,
    }
}

fn can_create_link(a: &PinSpec, b: &PinSpec) -> bool {
    a.id != b.id && a.node != b.node && a.kind != b.kind && a.ty == b.ty
}

fn pin_spec(pin: PinId) -> Option<&'static PinSpec> {
    NODES
        .iter()
        .flat_map(|node| node.inputs.iter().chain(node.outputs.iter()))
        .find(|spec| spec.id == pin)
}

fn format_node_ids(ids: &[NodeId]) -> String {
    ids.iter()
        .map(|id| id.raw().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_link_ids(ids: &[LinkId]) -> String {
    ids.iter()
        .map(|id| id.raw().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
