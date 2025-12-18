//! IME / text input integration demo (winit 0.30 + WGPU).
//!
//! This example focuses on:
//! - Verifying that `WindowEvent::Ime` is wired into Dear ImGui.
//! - Showing how `io.want_text_input()` drives IME auto-management.
//! - Visualizing the current IME/IO state while typing into `InputText`.
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin ime_debug

use dear_imgui_rs::*;
use dear_imgui_rs::{FontConfig, FontSource};
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[path = "../support/wgpu_init.rs"]
mod wgpu_init;

/// Configure fonts for this example.
///
/// - Always ensure a default font is present.
/// - If a CJK font is available under `examples/assets`, merge it so IME
///   input (e.g. Chinese/Japanese) renders instead of showing `?`.
fn init_fonts(context: &mut Context) {
    let mut fonts = context.fonts();

    // Make sure we have a default Latin font.
    fonts.add_font(&[FontSource::DefaultFontData {
        size_pixels: Some(16.0),
        config: None,
    }]);

    // Optional: merge a CJK font if present.
    // The repository ships `examples/assets/NotoSansSC-Regular.ttf` (Noto Sans SC).
    // If the file is missing or invalid, we skip it and keep ASCII-only rendering.
    let path = "examples/assets/NotoSansSC-Regular.ttf";
    match std::fs::read(path) {
        Ok(data) => {
            // Minimal sanity check: classic TrueType header (0x00010000) or 'true'.
            if data.len() >= 4 {
                let tag = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                const TAG_TRUE: u32 = 0x7472_7565; // 'true'
                if tag == 0x0001_0000 || tag == TAG_TRUE {
                    let cfg = FontConfig::new().size_pixels(18.0).merge_mode(true);
                    let _id = fonts.add_font(&[FontSource::TtfData {
                        data: &data,
                        size_pixels: Some(18.0),
                        config: Some(cfg),
                    }]);
                } else {
                    eprintln!(
                        "[ime_debug] {} does not look like a TrueType font (unexpected header); skipping CJK font.",
                        path
                    );
                }
            } else {
                eprintln!(
                    "[ime_debug] {} is too small to be a valid TTF; skipping CJK font.",
                    path
                );
            }
        }
        Err(err) => {
            eprintln!(
                "[ime_debug] Failed to read {} ({}); IME text will fall back to ASCII-only fonts.",
                path, err
            );
        }
    }

    // Note:
    // Do not call `fonts.build()` here. With the ImGui 1.92+ "new backend"
    // model, font atlas building should happen after the renderer has set
    // ImGuiBackendFlags_RendererHasTextures, typically via the renderer
    // or NewFrame path. Here we only register font sources into the atlas.
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    input_text: String,
    ime_forced: bool,
    ime_force_state: bool,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Create window and WGPU surface using the shared helper
        let window: Arc<Window> = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui IME Debug (winit + WGPU)")
                        .with_inner_size(LogicalSize::new(960.0, 540.0)),
                )?
                .into(),
        );

        let (device, queue, surface, surface_desc) = wgpu_init::init_wgpu_for_window(&window)?;

        // ImGui + winit platform
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();

        // Fonts: default + optional CJK merge (Noto Sans SC) for IME text.
        init_fonts(&mut context);

        // Basic style
        unsafe {
            dear_imgui_rs::sys::igStyleColorsDark(std::ptr::null_mut());
        }

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Renderer
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = WgpuRenderer::new(init_info, &mut context)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

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
            },
            input_text: String::new(),
            ime_forced: false,
            ime_force_state: false,
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
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        let io = ui.io();

        ui.window("IME Debug")
            .size([520.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                // Push a stable root ID so that even widgets with empty labels
                // (if any) do not collide with the window ID. This also makes
                // it easier to extend this example without hitting Dear ImGui's
                // "empty ID at root of a window" debug assertion.
                let _id = ui.push_id("ime_debug_root");

                ui.text("Type into the input below using an IME (e.g. Chinese/Japanese).");
                ui.text("This window shows how winit IME and Dear ImGui integrate.");
                ui.separator();

                ui.input_text("Input", &mut self.input_text)
                    .hint("Type here...")
                    .build();

                ui.separator();
                ui.text("IO / backend state:");
                ui.bullet_text(&format!("io.want_text_input = {}", io.want_text_input()));
                ui.bullet_text(&format!(
                    "io.want_capture_keyboard = {}",
                    io.want_capture_keyboard()
                ));
                ui.bullet_text(&format!(
                    "ime_enabled() = {}",
                    self.imgui.platform.ime_enabled()
                ));

                ui.separator();
                ui.text("IME control:");
                ui.text(
                    "By default, the backend auto-manages IME via io.want_text_input().\
                     You can override it temporarily:",
                );

                if ui.checkbox("Force IME state (disable auto)", &mut self.ime_forced) {
                    self.imgui
                        .platform
                        .set_ime_auto_management(!self.ime_forced);
                }
                if self.ime_forced {
                    ui.same_line();
                    if ui.button(if self.ime_force_state {
                        "Disable IME"
                    } else {
                        "Enable IME"
                    }) {
                        self.ime_force_state = !self.ime_force_state;
                        self.imgui
                            .platform
                            .set_ime_allowed(&self.window, self.ime_force_state);
                    }
                }

                ui.separator();
                ui.text("Notes:");
                ui.bullet_text(
                    "On desktop, IME candidates should follow the text caret in the input box.",
                );
                ui.bullet_text(
                    "On mobile platforms, enabling IME typically shows the soft keyboard.",
                );
            });

        // Let the platform backend update IME/cursor state based on the UI we just built.
        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);

        // Clear + render
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ime_debug_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ime_debug_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
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
            let draw_data = self.imgui.context.render();
            self.imgui.renderer.render_draw_data_with_fb_size(
                draw_data,
                &mut rpass,
                self.surface_desc.width,
                self.surface_desc.height,
            )?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(win) => {
                    self.window = Some(win);
                    self.window.as_ref().unwrap().window.request_redraw();
                }
                Err(e) => {
                    eprintln!("Failed to create window: {e}");
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
                let new_size = window.window.inner_size();
                window.resize(new_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    eprintln!("Render error: {e}");
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
