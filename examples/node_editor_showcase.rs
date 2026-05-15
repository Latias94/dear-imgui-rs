//! Extended imgui-node-editor example for the `dear-node-editor` safe API.

#[path = "support/wgpu_init.rs"]
mod wgpu_init;

use dear_imgui_rs::*;
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use dear_node_editor::{
    CanvasSizeMode, EditorConfig, EditorContext, FlowDirection, LinkId, NodeEditorFrame,
    NodeEditorUiExt, NodeId, PinId, PinKind, StyleColor, StyleVar,
};
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const VALUE_NODE: NodeId = NodeId::new(1);
const CURVE_NODE: NodeId = NodeId::new(2);
const MIX_NODE: NodeId = NodeId::new(3);
const PREVIEW_NODE: NodeId = NodeId::new(4);
const GROUP_NODE: NodeId = NodeId::new(100);

const VALUE_OUT: PinId = PinId::new(11);
const CURVE_IN: PinId = PinId::new(21);
const CURVE_OUT: PinId = PinId::new(22);
const MIX_A: PinId = PinId::new(31);
const MIX_B: PinId = PinId::new(32);
const MIX_OUT: PinId = PinId::new(33);
const PREVIEW_IN: PinId = PinId::new(41);

const VALUE_PINS: [PinSpec; 1] = [PinSpec {
    id: VALUE_OUT,
    label: "value",
    kind: PinKind::Output,
}];
const CURVE_PINS: [PinSpec; 2] = [
    PinSpec {
        id: CURVE_IN,
        label: "source",
        kind: PinKind::Input,
    },
    PinSpec {
        id: CURVE_OUT,
        label: "curve",
        kind: PinKind::Output,
    },
];
const MIX_PINS: [PinSpec; 3] = [
    PinSpec {
        id: MIX_A,
        label: "a",
        kind: PinKind::Input,
    },
    PinSpec {
        id: MIX_B,
        label: "b",
        kind: PinKind::Input,
    },
    PinSpec {
        id: MIX_OUT,
        label: "mixed",
        kind: PinKind::Output,
    },
];
const PREVIEW_PINS: [PinSpec; 1] = [PinSpec {
    id: PREVIEW_IN,
    label: "image",
    kind: PinKind::Input,
}];

const NODES: [NodeSpec; 4] = [
    NodeSpec {
        id: VALUE_NODE,
        title: "Value",
        subtitle: "Constant color",
        pins: &VALUE_PINS,
        tint: [0.32, 0.68, 0.96, 1.0],
        initial_position: [80.0, 130.0],
        initial_z: 0.0,
    },
    NodeSpec {
        id: CURVE_NODE,
        title: "Curve",
        subtitle: "Tone remap",
        pins: &CURVE_PINS,
        tint: [0.80, 0.55, 0.98, 1.0],
        initial_position: [330.0, 100.0],
        initial_z: 0.1,
    },
    NodeSpec {
        id: MIX_NODE,
        title: "Mixer",
        subtitle: "Blend sources",
        pins: &MIX_PINS,
        tint: [0.94, 0.64, 0.28, 1.0],
        initial_position: [590.0, 170.0],
        initial_z: 0.2,
    },
    NodeSpec {
        id: PREVIEW_NODE,
        title: "Preview",
        subtitle: "Final target",
        pins: &PREVIEW_PINS,
        tint: [0.36, 0.84, 0.55, 1.0],
        initial_position: [900.0, 140.0],
        initial_z: 0.3,
    },
];

#[derive(Clone, Copy)]
struct PinSpec {
    id: PinId,
    label: &'static str,
    kind: PinKind,
}

struct NodeSpec {
    id: NodeId,
    title: &'static str,
    subtitle: &'static str,
    pins: &'static [PinSpec],
    tint: [f32; 4],
    initial_position: [f32; 2],
    initial_z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StylePreset {
    Graphite,
    Studio,
}

#[derive(Clone, Copy)]
struct Link {
    id: LinkId,
    start: PinId,
    end: PinId,
}

struct GraphState {
    links: Vec<Link>,
    next_link_id: usize,
    positions_initialized: bool,
    shortcuts_enabled: bool,
    diagnostics_visible: bool,
    animate_flow: bool,
    preview_raised: bool,
    style_preset: StylePreset,
    last_action: String,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            links: vec![
                Link {
                    id: LinkId::new(1000),
                    start: VALUE_OUT,
                    end: CURVE_IN,
                },
                Link {
                    id: LinkId::new(1001),
                    start: CURVE_OUT,
                    end: MIX_A,
                },
                Link {
                    id: LinkId::new(1002),
                    start: MIX_OUT,
                    end: PREVIEW_IN,
                },
            ],
            next_link_id: 1003,
            positions_initialized: false,
            shortcuts_enabled: true,
            diagnostics_visible: true,
            animate_flow: true,
            preview_raised: true,
            style_preset: StylePreset::Graphite,
            last_action: "Ready".to_owned(),
        }
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    node_editor: EditorContext,
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
                    .with_title("Dear ImGui Node Editor Showcase")
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
            .drag_button(MouseButton::Middle)
            .select_button(MouseButton::Left)
            .navigate_button(MouseButton::Right)
            .context_menu_button(MouseButton::Right)
            .smooth_zoom(true, 1.18);
        let node_editor = EditorContext::create_with_config(&context, config);
        apply_style_preset(&node_editor, StylePreset::Graphite);

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            node_editor,
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

        draw_node_editor_window(&ui, &self.imgui.node_editor, &mut self.graph);

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
                label: Some("Node Editor Showcase Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Node Editor Showcase Render Pass"),
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

fn draw_node_editor_window(ui: &Ui, node_editor: &EditorContext, graph: &mut GraphState) {
    ui.window("Node Editor Showcase")
        .size([1160.0, 720.0], Condition::FirstUseEver)
        .position([32.0, 32.0], Condition::FirstUseEver)
        .build(|| {
            let select_chain = ui.button("Select Pipeline");
            ui.same_line();
            let clear_selection = ui.button("Clear Selection");
            ui.same_line();
            let center_preview = ui.button("Center Preview");
            ui.same_line();
            let break_value_links = ui.button("Break Value Links");
            ui.same_line();
            let navigate_content = ui.button("Fit Content");

            ui.checkbox("Shortcuts", &mut graph.shortcuts_enabled);
            ui.same_line();
            ui.checkbox("Diagnostics", &mut graph.diagnostics_visible);
            ui.same_line();
            ui.checkbox("Flow", &mut graph.animate_flow);
            ui.same_line();
            ui.checkbox("Raise Preview", &mut graph.preview_raised);

            let mut preset_changed = false;
            if ui.radio_button("Graphite", graph.style_preset == StylePreset::Graphite) {
                graph.style_preset = StylePreset::Graphite;
                preset_changed = true;
            }
            ui.same_line();
            if ui.radio_button("Studio", graph.style_preset == StylePreset::Studio) {
                graph.style_preset = StylePreset::Studio;
                preset_changed = true;
            }
            if preset_changed {
                apply_style_preset(node_editor, graph.style_preset);
            }

            let editor = ui.node_editor(node_editor, "node_editor_showcase", [0.0, 500.0]);
            editor.set_shortcuts_enabled(graph.shortcuts_enabled);

            if !graph.positions_initialized {
                initialize_node_layout(&editor);
                graph.positions_initialized = true;
            }

            editor.set_group_size(GROUP_NODE, [1040.0, 330.0]);
            editor.set_node_z_position(PREVIEW_NODE, if graph.preview_raised { 2.0 } else { 0.3 });

            if select_chain {
                editor.clear_selection();
                editor.select_node(VALUE_NODE);
                editor.add_node_to_selection(CURVE_NODE);
                editor.add_node_to_selection(MIX_NODE);
                editor.add_node_to_selection(PREVIEW_NODE);
                graph.last_action = "Selected pipeline nodes".to_owned();
            }
            if clear_selection {
                editor.clear_selection();
                graph.last_action = "Cleared selection".to_owned();
            }
            if center_preview {
                editor.center_node_on_screen(PREVIEW_NODE);
                graph.last_action = "Centered preview node".to_owned();
            }
            if navigate_content {
                editor.navigate_to_content(0.35);
                graph.last_action = "Navigated to content".to_owned();
            }
            if break_value_links {
                let native_count = editor.break_pin_links(VALUE_OUT);
                let before = graph.links.len();
                graph.links.retain(|link| link.start != VALUE_OUT);
                let model_count = before - graph.links.len();
                graph.last_action =
                    format!("Broke {model_count} model links, native reported {native_count}");
            }

            draw_group_node(&editor, ui);
            for node in &NODES {
                draw_demo_node(&editor, ui, node);
            }

            for node in &NODES {
                draw_node_background(&editor, node);
            }

            for link in &graph.links {
                editor.link_colored(link.id, link.start, link.end, [0.38, 0.74, 0.94, 1.0], 2.4);
                if graph.animate_flow {
                    editor.flow(link.id, FlowDirection::Forward);
                }
            }

            draw_group_hint(&editor);
            handle_create_session(&editor, graph);
            handle_delete_session(&editor, graph);
            handle_shortcuts(&editor, graph);
            record_context_and_clicks(&editor, graph);

            let summary = collect_summary(&editor, ui, graph);
            {
                let suspension = editor.suspend();
                if graph.diagnostics_visible {
                    ui.separator();
                    draw_diagnostics(ui, &summary, &graph.last_action);
                }
                debug_assert!(editor.is_suspended());
                suspension.resume();
            }

            editor.end();
        });
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

fn initialize_node_layout(editor: &NodeEditorFrame<'_>) {
    editor.set_node_position(GROUP_NODE, [40.0, 60.0]);
    editor.set_node_z_position(GROUP_NODE, -1.0);
    for node in &NODES {
        editor.set_node_position(node.id, node.initial_position);
        editor.set_node_z_position(node.id, node.initial_z);
    }
    editor.navigate_to_content(0.0);
}

fn draw_group_node(editor: &NodeEditorFrame<'_>, ui: &Ui) {
    let node = editor.begin_node(GROUP_NODE);
    ui.text("Pipeline Group");
    editor.group([1040.0, 330.0]);
    node.end();
}

fn draw_demo_node(editor: &NodeEditorFrame<'_>, ui: &Ui, node: &NodeSpec) {
    let node_bg = [
        node.tint[0] * 0.18,
        node.tint[1] * 0.18,
        node.tint[2] * 0.18,
        0.94,
    ];
    let _node_color = editor.push_style_color(StyleColor::NodeBackground, node_bg);
    let _border_color = editor.push_style_color(StyleColor::NodeBorder, node.tint);
    let _padding = editor.push_style_var_vec4(StyleVar::NodePadding, [12.0, 9.0, 12.0, 10.0]);
    let _rounding = editor.push_style_var_float(StyleVar::NodeRounding, 6.0);

    let token = editor.begin_node(node.id);
    ui.text_colored(node.tint, node.title);
    ui.text(node.subtitle);
    ui.separator();
    for pin in node.pins {
        draw_pin(ui, &token, *pin);
    }
    token.end();
}

fn draw_pin(ui: &Ui, node: &dear_node_editor::NodeToken<'_>, pin: PinSpec) {
    let token = node.begin_pin(pin.id, pin.kind);
    let cursor = ui.cursor_screen_pos();
    token.rect(
        [cursor[0] - 6.0, cursor[1] - 2.0],
        [cursor[0] + 130.0, cursor[1] + 20.0],
    );
    token.pivot_rect(
        [cursor[0] - 7.0, cursor[1] + 1.0],
        [cursor[0] + 7.0, cursor[1] + 17.0],
    );
    token.pivot_size([12.0, 12.0]);
    token.pivot_scale([1.0, 1.0]);
    token.pivot_alignment(match pin.kind {
        PinKind::Input => [0.0, 0.5],
        PinKind::Output => [1.0, 0.5],
    });
    ui.text(match pin.kind {
        PinKind::Input => format!("< {}", pin.label),
        PinKind::Output => format!("{} >", pin.label),
    });
    token.end();
}

fn draw_node_background(editor: &NodeEditorFrame<'_>, node: &NodeSpec) {
    let size = editor.node_size(node.id);
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return;
    }

    let min = editor.canvas_to_screen(editor.node_position(node.id));
    let max = [min[0] + 6.0, min[1] + size[1]];
    let draw_list = editor.node_background_draw_list(node.id);
    draw_list
        .add_rect(min, max, [node.tint[0], node.tint[1], node.tint[2], 0.45])
        .filled(true)
        .rounding(3.0)
        .build();
}

fn draw_group_hint(editor: &NodeEditorFrame<'_>) {
    if let Some(hint) = editor.begin_group_hint(GROUP_NODE) {
        let min = hint.min();
        let max = hint.max();
        hint.background_draw_list()
            .add_rect(
                [min[0] + 4.0, min[1] + 4.0],
                [max[0] - 4.0, max[1] - 4.0],
                [0.16, 0.18, 0.20, 0.30],
            )
            .filled(true)
            .rounding(8.0)
            .build();
        hint.foreground_draw_list().add_text(
            [min[0] + 16.0, min[1] + 10.0],
            [0.70, 0.82, 0.92, 1.0],
            "safe API group hint",
        );
        hint.end();
    }
}

fn handle_create_session(editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    if let Some(create) = editor.begin_create([0.30, 0.84, 0.46, 1.0], 2.0) {
        if let Some((a, b)) = create.query_new_link_styled([0.40, 0.80, 1.0, 1.0], 2.5) {
            match normalize_link(a, b) {
                Some((start, end))
                    if !graph
                        .links
                        .iter()
                        .any(|link| link.start == start && link.end == end) =>
                {
                    if create.accept_new_item_styled([0.35, 0.92, 0.55, 1.0], 3.0) {
                        graph.links.push(Link {
                            id: LinkId::new(graph.next_link_id),
                            start,
                            end,
                        });
                        graph.next_link_id += 1;
                        graph.last_action =
                            format!("Created link {} -> {}", start.raw(), end.raw());
                    }
                }
                _ => {
                    create.reject_new_item_styled([0.95, 0.24, 0.25, 1.0], 2.0);
                }
            }
        } else if let Some(pin) = create.query_new_node_styled([0.95, 0.65, 0.24, 1.0], 2.0) {
            create.reject_new_item_styled([0.95, 0.65, 0.24, 1.0], 2.0);
            graph.last_action = format!("Rejected new-node request from pin {}", pin.raw());
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

fn record_context_and_clicks(editor: &NodeEditorFrame<'_>, graph: &mut GraphState) {
    if let Some(node) = editor.show_node_context_menu() {
        graph.last_action = format!("Node context menu requested for {}", node.raw());
    } else if let Some(pin) = editor.show_pin_context_menu() {
        graph.last_action = format!("Pin context menu requested for {}", pin.raw());
    } else if let Some(link) = editor.show_link_context_menu() {
        graph.last_action = format!("Link context menu requested for {}", link.raw());
    } else if editor.show_background_context_menu() {
        graph.last_action = "Background context menu requested".to_owned();
    }

    if editor.is_background_clicked() {
        graph.last_action = format!(
            "Background clicked with {:?}",
            editor.background_click_button()
        );
    }
    if editor.is_background_double_clicked() {
        graph.last_action = format!(
            "Background double-clicked with {:?}",
            editor.background_double_click_button()
        );
    }
}

struct FrameSummary {
    selected_nodes: Vec<NodeId>,
    selected_links: Vec<LinkId>,
    ordered_nodes: Vec<NodeId>,
    hovered_node: Option<NodeId>,
    hovered_pin: Option<PinId>,
    hovered_link: Option<LinkId>,
    double_clicked_node: Option<NodeId>,
    first_link_pins: Option<(PinId, PinId)>,
    value_has_links: bool,
    value_had_links: bool,
    shortcuts_enabled: bool,
    active: bool,
    selection_changed: bool,
    zoom: f32,
    screen_size: [f32; 2],
    mouse_canvas: [f32; 2],
    node_count: usize,
    preview_z: f32,
}

fn collect_summary(editor: &NodeEditorFrame<'_>, ui: &Ui, graph: &GraphState) -> FrameSummary {
    FrameSummary {
        selected_nodes: editor.selected_nodes(),
        selected_links: editor.selected_links(),
        ordered_nodes: editor.ordered_node_ids(),
        hovered_node: editor.hovered_node(),
        hovered_pin: editor.hovered_pin(),
        hovered_link: editor.hovered_link(),
        double_clicked_node: editor.double_clicked_node(),
        first_link_pins: graph
            .links
            .first()
            .and_then(|link| editor.link_pins(link.id)),
        value_has_links: editor.pin_has_any_links(VALUE_OUT),
        value_had_links: editor.pin_had_any_links(VALUE_OUT),
        shortcuts_enabled: editor.shortcuts_enabled(),
        active: editor.is_active(),
        selection_changed: editor.has_selection_changed(),
        zoom: editor.current_zoom(),
        screen_size: editor.screen_size(),
        mouse_canvas: editor.screen_to_canvas(ui.mouse_pos()),
        node_count: editor.node_count(),
        preview_z: editor.node_z_position(PREVIEW_NODE),
    }
}

fn draw_diagnostics(ui: &Ui, summary: &FrameSummary, last_action: &str) {
    ui.text(format!("Last action: {last_action}"));
    ui.text(format!(
        "Selected nodes: [{}] | selected links: [{}]",
        format_node_ids(&summary.selected_nodes),
        format_link_ids(&summary.selected_links)
    ));
    ui.text(format!(
        "Ordered nodes: [{}] | node count: {} | preview z: {:.1}",
        format_node_ids(&summary.ordered_nodes),
        summary.node_count,
        summary.preview_z
    ));
    ui.text(format!(
        "Hovered node/pin/link: {:?} / {:?} / {:?} | double-clicked node: {:?}",
        summary.hovered_node.map(NodeId::raw),
        summary.hovered_pin.map(PinId::raw),
        summary.hovered_link.map(LinkId::raw),
        summary.double_clicked_node.map(NodeId::raw)
    ));
    ui.text(format!(
        "First link pins: {:?} | value has/had links: {}/{}",
        summary
            .first_link_pins
            .map(|(start, end)| (start.raw(), end.raw())),
        summary.value_has_links,
        summary.value_had_links
    ));
    ui.text(format!(
        "Active: {} | selection changed: {} | shortcuts: {} | zoom: {:.2}",
        summary.active, summary.selection_changed, summary.shortcuts_enabled, summary.zoom
    ));
    ui.text(format!(
        "Screen: {:.0} x {:.0} | mouse canvas: {:.1}, {:.1}",
        summary.screen_size[0],
        summary.screen_size[1],
        summary.mouse_canvas[0],
        summary.mouse_canvas[1]
    ));
}

fn apply_style_preset(editor: &EditorContext, preset: StylePreset) {
    let mut style = editor.style();
    match preset {
        StylePreset::Graphite => {
            style.node_rounding = 7.0;
            style.group_rounding = 8.0;
            style.pin_rounding = 4.0;
            style.link_strength = 110.0;
            style.flow_speed = 120.0;
            style.flow_duration = 1.2;
            style.set_color(StyleColor::Background, [0.08, 0.09, 0.10, 1.0]);
            style.set_color(StyleColor::Grid, [0.24, 0.28, 0.31, 0.42]);
            style.set_color(StyleColor::GroupBackground, [0.10, 0.12, 0.14, 0.42]);
            style.set_color(StyleColor::SelectedNodeBorder, [0.36, 0.78, 1.0, 1.0]);
        }
        StylePreset::Studio => {
            style.node_rounding = 4.0;
            style.group_rounding = 3.0;
            style.pin_rounding = 2.0;
            style.link_strength = 80.0;
            style.flow_speed = 90.0;
            style.flow_duration = 1.8;
            style.set_color(StyleColor::Background, [0.11, 0.10, 0.08, 1.0]);
            style.set_color(StyleColor::Grid, [0.32, 0.28, 0.20, 0.38]);
            style.set_color(StyleColor::GroupBackground, [0.17, 0.14, 0.10, 0.44]);
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
    match (pin_kind(a), pin_kind(b)) {
        (Some(PinKind::Output), Some(PinKind::Input)) => Some((a, b)),
        (Some(PinKind::Input), Some(PinKind::Output)) => Some((b, a)),
        _ => None,
    }
}

fn pin_kind(pin: PinId) -> Option<PinKind> {
    NODES
        .iter()
        .flat_map(|node| node.pins.iter())
        .find(|spec| spec.id == pin)
        .map(|spec| spec.kind)
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
