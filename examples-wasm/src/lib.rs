#![cfg(target_arch = "wasm32")]

#[cfg(feature = "cte")]
use dear_imgui_cte::{
    CteResult, CteUiExt, Language, NotificationType, Notifications, TextDiff, TextEditor,
    dejavu_font_source,
};
use dear_imgui_rs::*;
use dear_imgui_wgpu::{FramebufferExtent, WgpuRenderer};
use dear_imgui_winit::WinitPlatform;
#[cfg(feature = "imguizmo")]
use dear_imguizmo::{GuizmoExt, Mode as GuizmoMode, Operation as GuizmoOperation};
#[cfg(feature = "imguizmo-quat")]
use dear_imguizmo_quat::{GizmoQuatExt, Mode as GizmoQuatMode};
#[cfg(feature = "imnodes")]
use dear_imnodes::{Context as ImnodesContext, EditorContext as ImnodesEditorContext, ImNodesExt};
#[cfg(feature = "implot")]
use dear_implot::PlotContext;
#[cfg(feature = "implot3d")]
use dear_implot3d::{Axis3DFlags, Plot3DContext, Plot3DFlags, Scatter3DFlags};
use log::info;
#[cfg(feature = "cte")]
use std::{cell::Cell, rc::Rc, time::Duration};
use std::{cell::RefCell, ffi::CStr, sync::Arc};
use wasm_bindgen::prelude::*;
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(feature = "cte")]
struct CteWebState {
    editor: TextEditor,
    diff: TextDiff,
    notifications: Notifications,
    transaction_count: Rc<Cell<usize>>,
}

#[cfg(feature = "cte")]
impl CteWebState {
    fn create(context: &Context) -> CteResult<Self> {
        let mut editor = TextEditor::try_create(context)?;
        editor.set_text(
            "#include <string>\n\nstd::string greeting = \"Hello from WebAssembly\";\n",
        )?;
        editor.set_language(Some(Language::Cpp));
        let transaction_count: Rc<Cell<usize>> = Rc::new(Cell::new(0));
        let callback_count = Rc::clone(&transaction_count);
        editor.set_transaction_callback(move |_| {
            callback_count.set(callback_count.get().saturating_add(1));
        })?;
        editor.enable_trie_autocomplete()?;

        let mut diff = TextDiff::try_create(context)?;
        diff.set_text("const int answer = 41;\n", "const int answer = 42;\n")?;
        diff.set_language(Some(Language::Cpp));
        diff.set_side_by_side(true);

        let mut notifications = Notifications::try_create(context)?;
        notifications.add(
            NotificationType::Info,
            "CTE is using the shared WASM provider",
            Duration::from_secs(4),
        )?;

        Ok(Self {
            editor,
            diff,
            notifications,
            transaction_count,
        })
    }
}

struct ImguiState {
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    demo_open: bool,
    last_frame: Instant,
    frames: u32,
    layout_size: [f32; 2],
    demo_needs_layout: bool,
    #[cfg(feature = "implot")]
    implot_ctx: PlotContext,
    #[cfg(feature = "implot3d")]
    implot3d_ctx: Plot3DContext,
    #[cfg(feature = "imnodes")]
    imnodes_ctx: ImnodesContext,
    #[cfg(feature = "imnodes")]
    imnodes_editor: ImnodesEditorContext,
    #[cfg(feature = "imguizmo")]
    guizmo_model: [f32; 16],
    #[cfg(feature = "imguizmo-quat")]
    quat_rot: [f32; 4],
    #[cfg(feature = "cte")]
    cte: Option<CteWebState>,
    context: Context,
}

impl Drop for ImguiState {
    fn drop(&mut self) {
        #[cfg(feature = "cte")]
        drop(self.cte.take());
    }
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
    // Complete async GPU setup given an already-created window + surface.
    async fn new_with_window(
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        instance: wgpu::Instance,
    ) -> Result<Self, JsValue> {
        // Request adapter and device
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_adapter: {e}")))?;

        // Dear ImGui needs no compute or storage buffers. Keep device limits compatible
        // with WebGL2 while preserving this adapter's texture sizes and buffer alignment.
        let required_limits = wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())
            .using_alignment(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_limits,
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_device: {e}")))?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        // Use physical size of the canvas for surface configuration (matches scissor/render target)
        let physical = window.inner_size();
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: physical.width.max(1),
            height: physical.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_desc);

        // Setup ImGui
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut context)
            .map_err(|e| JsValue::from_str(&format!("init platform: {e}")))?;
        platform
            .attach_window(
                Arc::clone(&window),
                dear_imgui_winit::HiDpiMode::Default,
                &mut context,
            )
            .map_err(|e| JsValue::from_str(&format!("attach platform window: {e}")))?;

        // Optional: experiment with custom font settings when the experimental
        // feature is enabled. This uses the modern FontAtlas API and verifies
        // that the shared-memory provider and bindings are wired correctly.
        #[cfg(feature = "experimental-fonts")]
        {
            use dear_imgui_rs::{FontConfig, FontSource};
            let fonts = context.font_atlas();
            let cfg = FontConfig::default()
                .size_pixels(18.0)
                .name("Web Default 18px");
            fonts.add_font(&[FontSource::default_font().with_config(cfg)]);
        }

        #[cfg(feature = "cte")]
        {
            let source = dejavu_font_source(16.0)
                .map_err(|error| JsValue::from_str(&format!("init CTE font: {error}")))?;
            context.font_atlas().add_font(&[source]);
        }

        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let renderer = WgpuRenderer::new(init_info, &mut context)
            .map_err(|e| JsValue::from_str(&format!("init renderer: {e}")))?;

        #[cfg(feature = "implot")]
        let implot_ctx = PlotContext::create(&context);
        #[cfg(feature = "implot3d")]
        let implot3d_ctx = Plot3DContext::create(&context);
        #[cfg(feature = "imnodes")]
        let imnodes_ctx = ImnodesContext::create(&context);
        #[cfg(feature = "imnodes")]
        let imnodes_editor = imnodes_ctx.create_editor_context();
        #[cfg(feature = "imguizmo")]
        let guizmo_model = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        #[cfg(feature = "imguizmo-quat")]
        let quat_rot = [0.0_f32, 0.0, 0.0, 1.0];
        #[cfg(feature = "cte")]
        let cte = Some(
            CteWebState::create(&context)
                .map_err(|error| JsValue::from_str(&format!("init CTE state: {error}")))?,
        );

        let imgui = ImguiState {
            platform,
            renderer,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            },
            demo_open: true,
            last_frame: Instant::now(),
            frames: 0,
            layout_size: [0.0, 0.0],
            demo_needs_layout: true,
            #[cfg(feature = "implot")]
            implot_ctx,
            #[cfg(feature = "implot3d")]
            implot3d_ctx,
            #[cfg(feature = "imnodes")]
            imnodes_ctx,
            #[cfg(feature = "imnodes")]
            imnodes_editor,
            #[cfg(feature = "imguizmo")]
            guizmo_model,
            #[cfg(feature = "imguizmo-quat")]
            quat_rot,
            #[cfg(feature = "cte")]
            cte,
            context,
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
        }
    }

    fn render(&mut self) -> Result<(), JsValue> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;

        // Query window size + HiDPI factor and update ImGui IO before NewFrame
        {
            let scale_factor = self.window.scale_factor() as f32;
            let physical = self.window.inner_size();
            // ImGui expects logical size in points, with framebuffer scale carrying the DPI factor
            let logical_w = (physical.width as f32 / scale_factor).max(0.0);
            let logical_h = (physical.height as f32 / scale_factor).max(0.0);

            let io = self.imgui.context.io_mut();
            io.set_display_size([logical_w, logical_h]);
            io.set_display_framebuffer_scale([scale_factor, scale_factor]);
            io.set_delta_time(delta_time.as_secs_f32());

            // Log a few frames for debugging size plumbing
            if self.imgui.frames < 3 {
                info!(
                    "IO set: logical={}x{}, framebuffer_scale={} (winit scale_factor={}), physical={}x{}, surface={}x{}",
                    logical_w,
                    logical_h,
                    scale_factor,
                    self.window.scale_factor(),
                    physical.width,
                    physical.height,
                    self.surface_desc.width,
                    self.surface_desc.height
                );
            }
        }
        self.imgui.last_frame = now;
        self.imgui.frames = self.imgui.frames.saturating_add(1);

        // Allow winit platform backend to update IO (mouse/keyboard/text)
        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)
            .map_err(|e| JsValue::from_str(&format!("prepare platform frame: {e}")))?;

        // Skip a zero-sized canvas (e.g. a hidden element) before acquiring or opening a frame.
        let physical = self.window.inner_size();
        if physical.width == 0 || physical.height == 0 {
            return Ok(());
        }

        // Ensure surface matches current canvas/backing buffer size (DPR changes, CSS resize, etc.).
        if physical.width != self.surface_desc.width || physical.height != self.surface_desc.height
        {
            self.resize(physical);
        }

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
                return Err(JsValue::from_str(
                    "get_current_texture failed with a WGPU validation error",
                ));
            }
        };

        // Dropping this token closes the frame if a fallible widget or platform call returns early.
        let frame_token = self
            .imgui
            .context
            .try_begin_frame()
            .map_err(|error| JsValue::from_str(&format!("begin frame: {error}")))?;
        let ui = frame_token.ui();
        let display_size = ui.io().display_size();
        let orientation_changed = (self.imgui.layout_size[0] >= self.imgui.layout_size[1])
            != (display_size[0] >= display_size[1]);
        self.imgui.layout_size = display_size;
        if orientation_changed {
            self.imgui.demo_needs_layout = true;
        }
        let condition = if orientation_changed {
            Condition::Always
        } else {
            Condition::FirstUseEver
        };
        let margin = [
            10.0_f32.min((display_size[0] - 1.0).max(0.0) * 0.5),
            10.0_f32.min((display_size[1] - 1.0).max(0.0) * 0.5),
        ];
        let available = [
            (display_size[0] - margin[0] * 2.0).max(1.0),
            (display_size[1] - margin[1] * 2.0).max(1.0),
        ];
        let panel_width = available[0].min(400.0);
        let gap = 10.0_f32.min(available[1] * 0.1);
        let side_by_side = available[0] >= panel_width + gap + 240.0;
        // A stacked layout shares the height so both windows retain scrollable content.
        let panel_height = if side_by_side {
            available[1].min(300.0)
        } else {
            ((available[1] - gap) * 0.4).clamp(1.0, 300.0)
        };
        let panel_size = [panel_width, panel_height];
        let demo_pos = if side_by_side {
            [margin[0] + panel_width + gap, margin[1]]
        } else {
            [margin[0], margin[1] + panel_height + gap]
        };
        let demo_size = [
            (display_size[0] - demo_pos[0] - margin[0]).clamp(1.0, 550.0),
            (display_size[1] - demo_pos[1] - margin[1]).clamp(1.0, 680.0),
        ];

        // Preserve user positioning while it fits. A smaller canvas only clamps windows
        // that leave the client area, including optional extension windows.
        for title in [
            c"Hello, Dear ImGui (Web)",
            c"Dear ImGui Demo",
            #[cfg(feature = "cte")]
            c"ImGuiColorTextEdit (Web)",
            #[cfg(feature = "implot")]
            c"ImPlot (Web)",
            #[cfg(feature = "implot3d")]
            c"ImPlot3D (Web)",
            #[cfg(feature = "imnodes")]
            c"ImNodes (Web)",
            #[cfg(feature = "imguizmo")]
            c"ImGuizmo (Web)",
            #[cfg(feature = "imguizmo-quat")]
            c"ImGuIZMO.quat (Web)",
        ] {
            clamp_window_to_canvas(ui, title);
        }

        ui.window("Hello, Dear ImGui (Web)")
            .size(panel_size, condition)
            .position(margin, condition)
            .build(|| {
                ui.text("This is running under wasm32-unknown-unknown");
                ui.separator();
                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }
            });

        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
            // The demo chooses its own first-use geometry. Set its visible layout after creation.
            if self.imgui.demo_needs_layout {
                ui.set_window_pos_by_name("Dear ImGui Demo", demo_pos);
                ui.set_window_size_by_name("Dear ImGui Demo", demo_size);
                self.imgui.demo_needs_layout = false;
            }
        }

        #[cfg(feature = "cte")]
        {
            let cte = self
                .imgui
                .cte
                .as_mut()
                .expect("CTE state is initialized with the feature");
            let submitted = ui
                .window("ImGuiColorTextEdit (Web)")
                .size([700.0, 620.0], Condition::FirstUseEver)
                .build(|| -> CteResult<()> {
                    let connected = cte
                        .editor
                        .trie_autocomplete()
                        .map(|trie| trie.is_connected())
                        .transpose()?
                        .unwrap_or(false);
                    ui.text(format!(
                        "Trie connected: {connected} | transaction events: {}",
                        cte.transaction_count.get()
                    ));
                    if ui.button("Notify") {
                        cte.notifications.add(
                            NotificationType::Success,
                            "CTE notification from WebAssembly",
                            Duration::from_secs(3),
                        )?;
                    }
                    ui.separator();
                    ui.text_editor(&mut cte.editor, "Source##cte_web")
                        .size([0.0, 280.0])
                        .build()?;
                    ui.separator();
                    ui.text_diff(&mut cte.diff, "Diff##cte_web")
                        .size([0.0, 210.0])
                        .build()?;
                    Ok(())
                });
            if let Some(result) = submitted {
                result
                    .map_err(|error| JsValue::from_str(&format!("render CTE widgets: {error}")))?;
            }
            ui.notifications(&mut cte.notifications)
                .build()
                .map_err(|error| {
                    JsValue::from_str(&format!("render CTE notifications: {error}"))
                })?;
        }

        #[cfg(feature = "implot")]
        {
            ui.window("ImPlot (Web)")
                .size([480.0, 320.0], Condition::FirstUseEver)
                .build(|| {
                    let plot_ui = self.imgui.implot_ctx.get_plot_ui(ui);
                    let xs = [0.0_f64, 1.0, 2.0, 3.0];
                    let ys = [0.0_f64, 1.0, 0.0, 1.0];
                    if let Some(token) = plot_ui.begin_plot("ImPlot demo") {
                        plot_ui.plot_line("line", &xs, &ys);
                        token.end();
                    }
                });
        }

        #[cfg(feature = "implot3d")]
        {
            ui.window("ImPlot3D (Web)")
                .size([480.0, 320.0], Condition::FirstUseEver)
                .build(|| {
                    let plot_ui = self.imgui.implot3d_ctx.get_plot_ui(ui);
                    if let Some(_token) = plot_ui
                        .begin_plot("ImPlot3D demo")
                        .size([0.0, 0.0])
                        .flags(Plot3DFlags::NONE)
                        .build()
                    {
                        plot_ui.setup_axes(
                            "X",
                            "Y",
                            "Z",
                            Axis3DFlags::NONE,
                            Axis3DFlags::NONE,
                            Axis3DFlags::NONE,
                        );
                        let xs = [0.0_f32, 1.0, 2.0, 3.0];
                        let ys = [0.0_f32, 1.0, 0.0, -1.0];
                        let zs = [0.0_f32, 0.5, 1.0, 0.0];
                        plot_ui.plot_scatter_f32("points", &xs, &ys, &zs, Scatter3DFlags::NONE);
                    }
                });
        }

        #[cfg(feature = "imnodes")]
        {
            use dear_imnodes::{LinkId, NodeId, PinId, PinShape};

            ui.window("ImNodes (Web)")
                .size([480.0, 320.0], Condition::FirstUseEver)
                .build(|| {
                    let nodes_ui = ui.imnodes(&self.imgui.imnodes_ctx);
                    let editor = nodes_ui
                        .editor(Some(&self.imgui.imnodes_editor))
                        .begin_nodes();

                    {
                        let _node = editor.node(NodeId::from(1));
                        ui.text("Input Node");
                        {
                            let _attr = _node.input_attr(PinId::from(2), PinShape::CircleFilled);
                            ui.text("In");
                        }
                    }

                    {
                        let _node = editor.node(NodeId::from(3));
                        ui.text("Output Node");
                        {
                            let _attr = _node.output_attr(PinId::from(4), PinShape::CircleFilled);
                            ui.text("Out");
                        }
                    }

                    editor.link(LinkId::from(5), PinId::from(2), PinId::from(4));
                    let _ = editor.end();
                });
        }

        #[cfg(feature = "imguizmo")]
        {
            ui.window("ImGuizmo (Web)")
                .size([480.0, 320.0], Condition::FirstUseEver)
                .build(|| {
                    let giz = ui.guizmo();

                    // Use the current window as the drawing target.
                    let [wx, wy] = ui.window_pos();
                    let [ww, wh] = ui.window_size();
                    giz.set_rect(wx, wy, ww, wh);
                    giz.set_drawlist_window();
                    giz.set_orthographic(false);

                    // Simple camera looking at the origin.
                    let view = [
                        1.0, 0.0, 0.0, 0.0, //
                        0.0, 1.0, 0.0, 0.0, //
                        0.0, 0.0, 1.0, 0.0, //
                        0.0, 0.0, -5.0, 1.0,
                    ];
                    // Very simple perspective-style projection; good enough for a demo.
                    let proj = [
                        1.0, 0.0, 0.0, 0.0, //
                        0.0, 1.0, 0.0, 0.0, //
                        0.0, 0.0, -1.0, -1.0, //
                        0.0, 0.0, -0.2, 0.0,
                    ];

                    // Draw a grid at the origin and manipulate a single cube model matrix.
                    giz.draw_grid(&view, &proj, &self.imgui.guizmo_model, 10.0);
                    let _id = giz.push_id(0);
                    let _ = giz
                        .manipulate_config(&view, &proj, &mut self.imgui.guizmo_model)
                        .operation(GuizmoOperation::TRANSLATE)
                        .mode(GuizmoMode::Local)
                        .build();
                });
        }

        #[cfg(feature = "imguizmo-quat")]
        {
            ui.window("ImGuIZMO.quat (Web)")
                .size([260.0, 260.0], Condition::FirstUseEver)
                .build(|| {
                    let giz = ui.gizmo_quat();
                    let _used = giz
                        .builder()
                        .size(220.0)
                        .mode(GizmoQuatMode::MODE_DUAL | GizmoQuatMode::CUBE_AT_ORIGIN)
                        .quat("##quat", &mut self.imgui.quat_rot);
                });
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let framebuffer_extent = FramebufferExtent::from_texture(&frame.texture);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.imgui
            .platform
            .prepare_render(ui, &self.window)
            .map_err(|e| JsValue::from_str(&format!("prepare platform render: {e}")))?;
        let pending_frame = frame_token
            .try_render(
                self.imgui
                    .renderer
                    .renderer_consumer()
                    .map_err(|e| JsValue::from_str(&format!("renderer consumer: {e}")))?,
            )
            .map_err(|e| JsValue::from_str(&format!("capture frame: {e}")))?;

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
                multiview_mask: None,
            });

            self.imgui
                .renderer
                .render(pending_frame, &mut rpass, framebuffer_extent)
                .map_err(|e| JsValue::from_str(&format!("render: {e}")))?;
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.surface_desc);
        }
        Ok(())
    }
}

// This small query also covers the upstream demo, whose Begin call lives outside this example.
fn clamp_window_to_canvas(ui: &Ui, title: &CStr) {
    let geometry = ui.with_bound_context(|| {
        // SAFETY: the bound, live UI owns this context. Dear ImGui keeps window objects alive
        // for the frame. Copy geometry before calling any API that mutates the window.
        unsafe {
            let window = sys::igFindWindowByName(title.as_ptr());
            window.as_ref().map(|window| {
                (
                    [window.Pos.x, window.Pos.y],
                    [window.SizeFull.x, window.SizeFull.y],
                )
            })
        }
    });
    let Some((position, size)) = geometry else {
        return;
    };
    let display = ui.io().display_size();
    let bounded_size = [
        size[0].clamp(1.0, display[0].max(1.0)),
        size[1].clamp(1.0, display[1].max(1.0)),
    ];
    let bounded_position = [
        position[0].clamp(0.0, (display[0] - bounded_size[0]).max(0.0)),
        position[1].clamp(0.0, (display[1] - bounded_size[1]).max(0.0)),
    ];
    let title = title.to_str().expect("example window titles are UTF-8");
    if bounded_size != size {
        ui.set_window_size_by_name(title, bounded_size);
    }
    if bounded_position != position {
        ui.set_window_pos_by_name(title, bounded_position);
    }
}

thread_local! {
    static APP_READY: RefCell<Option<AppWindow>> = const { RefCell::new(None) };
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window + surface synchronously, then finish GPU init asynchronously.
        use winit::platform::web::WindowAttributesExtWebSys;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        // Acquire target canvas from the page
        let win = web_sys::window().expect("no window");
        let document = win.document().expect("no document");
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("wasm-canvas")
            .expect("canvas not found")
            .dyn_into()
            .expect("element is not canvas");

        // The page owns the canvas CSS size; winit observes its size and pixel density.
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui Web Demo")
                        .with_canvas(Some(canvas)),
                )
                .expect("create_window"),
        );

        let surface = instance
            .create_surface(window.clone())
            .expect("create_surface");

        // Finish GPU init on the JS task queue
        let window_for_redraw = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match AppWindow::new_with_window(window, surface, instance).await {
                Ok(appwin) => {
                    APP_READY.with(|cell| *cell.borrow_mut() = Some(appwin));
                    window_for_redraw.request_redraw();
                }
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "Failed to init GPU: {:?}",
                        e
                    )));
                }
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // If async init completed, pick up the window now
        if self.window.is_none()
            && let Some(w) = APP_READY.with(|cell| cell.borrow_mut().take())
        {
            self.window = Some(w);
        }
        let Some(window) = self.window.as_mut() else {
            return;
        };

        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id: _window_id,
            event: event.clone(),
        };
        if let Err(error) = window.imgui.platform.handle_event(
            &mut window.imgui.context,
            &full_event,
        ) {
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "Winit platform event error: {error}"
            )));
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "Render error: {:?}",
                        e
                    )));
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Pick up async init completion if available
        if self.window.is_none()
            && let Some(w) = APP_READY.with(|cell| cell.borrow_mut().take())
        {
            self.window = Some(w);
        }
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    // Logging and panic hooks for the browser console
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    // Create an event loop and run the application
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("failed to run app");
}
