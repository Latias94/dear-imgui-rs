//! Basic imgui-node-editor example for `dear-node-editor`.

use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_node_editor::{
    EditorContext, LinkId, NodeEditorFrame, NodeEditorUiExt, NodeId, PinId, PinKind,
};
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
    node_editor: EditorContext,
    renderer: WgpuRenderer,
    platform: WinitPlatform,
    context: Context,
    clear_color: wgpu::Color,
    last_frame: Instant,
}

#[derive(Clone, Copy)]
struct Link {
    id: LinkId,
    input: PinId,
    output: PinId,
}

struct GraphState {
    links: Vec<Link>,
    next_link_id: usize,
    first_frame: bool,
}

const NODE_A: NodeId = NodeId::new(1);
const NODE_A_INPUT: PinId = PinId::new(2);
const NODE_A_OUTPUT: PinId = PinId::new(3);
const NODE_B: NodeId = NodeId::new(4);
const NODE_B_INPUT_1: PinId = PinId::new(5);
const NODE_B_INPUT_2: PinId = PinId::new(6);
const NODE_B_OUTPUT: PinId = PinId::new(7);

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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(format!("Dear ImGui + Node Editor - {version}"))
                        .with_inner_size(LogicalSize::new(1280.0, 720.0)),
                )?,
            )
        };

        let surface = instance.create_surface(window.clone())?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .expect("failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let physical_size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .copied()
            .find(|f| caps.formats.contains(f))
            .unwrap_or(caps.formats[0]);

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_desc);

        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer =
            WgpuRenderer::new(init_info, &mut context).expect("failed to initialize WGPU renderer");
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        let node_editor = EditorContext::create(&context);
        let imgui = ImguiState {
            node_editor,
            renderer,
            platform,
            context,
            clear_color: wgpu::Color {
                r: 0.08,
                g: 0.09,
                b: 0.10,
                a: 1.0,
            },
            last_frame: Instant::now(),
        };
        let graph = GraphState {
            links: Vec::new(),
            next_link_id: 100,
            first_frame: true,
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            graph,
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

        ui.window("Node Editor")
            .size([920.0, 620.0], Condition::FirstUseEver)
            .position([40.0, 40.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Basic Interaction");
                ui.same_line();
                if ui.button("Clear Links") {
                    self.graph.links.clear();
                }
                ui.separator();

                let editor =
                    ui.node_editor(&self.imgui.node_editor, "node_editor_basic", [0.0, 470.0]);
                if self.graph.first_frame {
                    editor.set_node_position(NODE_A, [40.0, 80.0]);
                    editor.set_node_position(NODE_B, [320.0, 120.0]);
                }

                draw_node_a(&editor, &ui);
                draw_node_b(&editor, &ui);

                for link in &self.graph.links {
                    editor.link_colored(
                        link.id,
                        link.output,
                        link.input,
                        [0.37, 0.72, 0.95, 1.0],
                        2.5,
                    );
                }

                if let Some(create) = editor.begin_create([0.30, 0.85, 0.45, 1.0], 2.0) {
                    if let Some((a, b)) = create.query_new_link() {
                        if let Some((output, input)) = normalize_link(a, b) {
                            let already_exists = self
                                .graph
                                .links
                                .iter()
                                .any(|link| link.output == output && link.input == input);
                            if already_exists {
                                create.reject_new_item();
                            } else if create.accept_new_item() {
                                self.graph.links.push(Link {
                                    id: LinkId::new(self.graph.next_link_id),
                                    input,
                                    output,
                                });
                                self.graph.next_link_id += 1;
                            }
                        } else {
                            create.reject_new_item();
                        }
                    }
                }

                if let Some(delete) = editor.begin_delete() {
                    while let Some((link_id, _, _)) = delete.query_deleted_link() {
                        if delete.accept_deleted_item(true) {
                            self.graph.links.retain(|link| link.id != link_id);
                        }
                    }
                    while delete.query_deleted_node().is_some() {
                        delete.reject_deleted_item();
                    }
                }

                let selected_nodes = editor.selected_nodes().len();
                let selected_links = editor.selected_links().len();
                if self.graph.first_frame {
                    editor.navigate_to_content(0.0);
                    self.graph.first_frame = false;
                }
                editor.end();

                ui.separator();
                ui.text(format!(
                    "Links: {}, selected nodes: {}, selected links: {}",
                    self.graph.links.len(),
                    selected_nodes,
                    selected_links
                ));
            });

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
                label: Some("Node Editor Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Node Editor Render Pass"),
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
        self.queue.present(output);
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

fn draw_node_a(editor: &NodeEditorFrame<'_>, ui: &Ui) {
    editor.node(NODE_A, |node| {
        ui.text("Node A");
        node.pin(NODE_A_INPUT, PinKind::Input, |_pin| {
            ui.text("-> In");
        });
        ui.same_line();
        node.pin(NODE_A_OUTPUT, PinKind::Output, |_pin| {
            ui.text("Out ->");
        });
    });
}

fn draw_node_b(editor: &NodeEditorFrame<'_>, ui: &Ui) {
    editor.node(NODE_B, |node| {
        ui.text("Node B");

        let inputs = ui.begin_group();
        node.pin(NODE_B_INPUT_1, PinKind::Input, |_pin| {
            ui.text("-> In1");
        });
        node.pin(NODE_B_INPUT_2, PinKind::Input, |_pin| {
            ui.text("-> In2");
        });
        inputs.end();

        ui.same_line();

        let outputs = ui.begin_group();
        node.pin(NODE_B_OUTPUT, PinKind::Output, |_pin| {
            ui.text("Out ->");
        });
        outputs.end();
    });
}

fn normalize_link(a: PinId, b: PinId) -> Option<(PinId, PinId)> {
    match (
        is_output_pin(a),
        is_input_pin(a),
        is_output_pin(b),
        is_input_pin(b),
    ) {
        (true, false, false, true) => Some((a, b)),
        (false, true, true, false) => Some((b, a)),
        _ => None,
    }
}

fn is_input_pin(pin: PinId) -> bool {
    matches!(pin, NODE_A_INPUT | NODE_B_INPUT_1 | NODE_B_INPUT_2)
}

fn is_output_pin(pin: PinId) -> bool {
    matches!(pin, NODE_A_OUTPUT | NODE_B_OUTPUT)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
