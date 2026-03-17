//! Regression example for GlowRenderer::with_external_context + render_with_context.
//!
//! This exercises Dear ImGui managed texture create/update/destroy requests through
//! DrawData::textures() while the OpenGL context is owned by the application.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::{GlowRenderer, SimpleTextureMap};
use dear_imgui_rs::{
    Condition, Context as ImguiContext, RegisteredUserTexture, TextureId,
    texture::{OwnedTextureData, TextureStatus},
};
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const MAX_REGRESSION_FRAMES: u32 = 120;
const TEXTURE_WIDTH: u32 = 128;
const TEXTURE_HEIGHT: u32 = 128;

fn boxed_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message.into()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegressionStage {
    AwaitCreate,
    SubmitUpdate,
    AwaitUpdate,
    SubmitDestroy,
    AwaitDestroy,
    Verified,
}

impl RegressionStage {
    fn label(self) -> &'static str {
        match self {
            Self::AwaitCreate => "await_create",
            Self::SubmitUpdate => "submit_update",
            Self::AwaitUpdate => "await_update",
            Self::SubmitDestroy => "submit_destroy",
            Self::AwaitDestroy => "await_destroy",
            Self::Verified => "verified",
        }
    }
}

struct ImguiState {
    // Drop order matters: unregister before dropping the texture and context.
    _registered_user_textures: Vec<RegisteredUserTexture>,
    managed_texture: OwnedTextureData,
    renderer: GlowRenderer,
    platform: WinitPlatform,
    context: ImguiContext,
    live_texture_id: Option<TextureId>,
    stage: RegressionStage,
    last_frame: Instant,
    clear_color: [f32; 4],
    frame_count: u32,
}

struct AppWindow {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    gl: glow::Context,
    imgui: ImguiState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    success_message: Option<String>,
    failure_message: Option<String>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = Window::default_attributes()
            .with_title("Glow External Context Regression")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());
        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(true))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1280).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        let mut imgui_context = ImguiContext::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut imgui_context);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };

        let mut renderer = GlowRenderer::with_external_context(
            &gl,
            &mut imgui_context,
            Box::new(SimpleTextureMap::default()),
        )?;
        renderer.set_framebuffer_srgb_enabled(true);
        renderer.create_device_objects(&gl)?;

        let mut managed_texture = OwnedTextureData::new();
        managed_texture.create(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            TEXTURE_WIDTH as i32,
            TEXTURE_HEIGHT as i32,
        );
        fill_texture_pixels(&mut managed_texture, 0);
        managed_texture.set_status(TextureStatus::WantCreate);

        let registered_user_textures =
            vec![imgui_context.register_user_texture_token(&mut *managed_texture)];

        let imgui = ImguiState {
            _registered_user_textures: registered_user_textures,
            managed_texture,
            renderer,
            platform,
            context: imgui_context,
            live_texture_id: None,
            stage: RegressionStage::AwaitCreate,
            last_frame: Instant::now(),
            clear_color: [0.08, 0.12, 0.16, 1.0],
            frame_count: 0,
        };

        Ok(Self {
            window,
            surface,
            context,
            gl,
            imgui,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(new_size.width).unwrap(),
                NonZeroU32::new(new_size.height).unwrap(),
            );
        }
    }

    fn prepare_regression_step(&mut self) {
        match self.imgui.stage {
            RegressionStage::SubmitUpdate => {
                fill_texture_pixels(&mut self.imgui.managed_texture, self.imgui.frame_count);
                self.imgui.stage = RegressionStage::AwaitUpdate;
                println!("Submitted managed texture update request");
            }
            RegressionStage::SubmitDestroy => {
                self.imgui
                    .managed_texture
                    .set_status(TextureStatus::WantDestroy);
                self.imgui.stage = RegressionStage::AwaitDestroy;
                println!("Submitted managed texture destroy request");
            }
            _ => {}
        }
    }

    fn verify_regression_step(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        match self.imgui.stage {
            RegressionStage::AwaitCreate => {
                if self.imgui.managed_texture.status() == TextureStatus::OK {
                    let tex_id = self.imgui.managed_texture.tex_id();
                    if tex_id.is_null() {
                        return Err(boxed_error(
                            "managed texture create completed without assigning a TextureId",
                        ));
                    }
                    if self.imgui.renderer.texture_map().get(tex_id).is_none() {
                        return Err(boxed_error(
                            "managed texture create completed but renderer texture map is missing the GPU texture",
                        ));
                    }
                    self.imgui.live_texture_id = Some(tex_id);
                    self.imgui.stage = RegressionStage::SubmitUpdate;
                    println!("Verified managed texture create request");
                }
            }
            RegressionStage::AwaitUpdate => {
                if self.imgui.managed_texture.status() == TextureStatus::OK {
                    let tex_id = self.imgui.live_texture_id.ok_or_else(|| {
                        boxed_error("missing TextureId after create verification")
                    })?;
                    if self.imgui.managed_texture.tex_id() != tex_id {
                        return Err(boxed_error(
                            "managed texture update unexpectedly changed the TextureId",
                        ));
                    }
                    if self.imgui.renderer.texture_map().get(tex_id).is_none() {
                        return Err(boxed_error(
                            "managed texture update completed but renderer texture map no longer contains the GPU texture",
                        ));
                    }
                    self.imgui.stage = RegressionStage::SubmitDestroy;
                    println!("Verified managed texture update request");
                }
            }
            RegressionStage::AwaitDestroy => {
                if self.imgui.managed_texture.status() == TextureStatus::Destroyed {
                    let tex_id = self.imgui.live_texture_id.ok_or_else(|| {
                        boxed_error("missing TextureId before destroy verification")
                    })?;
                    if !self.imgui.managed_texture.tex_id().is_null() {
                        return Err(boxed_error(
                            "managed texture destroy completed without clearing TextureId",
                        ));
                    }
                    if self.imgui.renderer.texture_map().get(tex_id).is_some() {
                        return Err(boxed_error(
                            "managed texture destroy completed but renderer texture map still contains the GPU texture",
                        ));
                    }
                    self.imgui.stage = RegressionStage::Verified;
                    let message = format!(
                        "Regression passed: external-context Glow renderer handled managed texture create/update/destroy in {} frames.",
                        self.imgui.frame_count
                    );
                    println!("{message}");
                    return Ok(Some(message));
                }
            }
            RegressionStage::Verified => {
                return Ok(Some(
                    "Regression already verified; exiting cleanly.".to_string(),
                ));
            }
            RegressionStage::SubmitUpdate | RegressionStage::SubmitDestroy => {}
        }

        Ok(None)
    }

    fn render(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        self.prepare_regression_step();

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        let stage = self.imgui.stage;
        let texture_status = self.imgui.managed_texture.status();
        let texture_id = self.imgui.managed_texture.tex_id();
        let show_texture = !matches!(
            stage,
            RegressionStage::SubmitDestroy
                | RegressionStage::AwaitDestroy
                | RegressionStage::Verified
        ) && texture_status != TextureStatus::Destroyed;

        ui.window("Glow External Context Regression")
            .size([520.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Reproduces issue #22 with with_external_context + render_with_context");
                ui.separator();
                ui.text(format!("Stage: {}", stage.label()));
                ui.text(format!("Texture status: {:?}", texture_status));
                ui.text(format!("TextureId: {}", texture_id.id()));
                ui.text(format!(
                    "Frame {}/{}",
                    self.imgui.frame_count + 1,
                    MAX_REGRESSION_FRAMES
                ));
                ui.separator();
                ui.text("Expected sequence:");
                ui.bullet_text("WantCreate -> OK");
                ui.bullet_text("WantUpdates -> OK");
                ui.bullet_text("WantDestroy -> Destroyed");

                if show_texture {
                    ui.separator();
                    ui.text("Managed texture preview:");
                    ui.image(&mut *self.imgui.managed_texture, [256.0, 256.0]);
                }
            });

        unsafe {
            self.gl.enable(glow::FRAMEBUFFER_SRGB);
            self.gl.clear_color(
                self.imgui.clear_color[0],
                self.imgui.clear_color[1],
                self.imgui.clear_color[2],
                self.imgui.clear_color[3],
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.disable(glow::FRAMEBUFFER_SRGB);
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        // Mirror the original issue reproduction, where the application keeps the GL
        // context externally and explicitly ensures device objects exist.
        self.imgui.renderer.create_device_objects(&self.gl)?;
        self.imgui
            .renderer
            .render_with_context(&self.gl, draw_data)?;

        self.surface.swap_buffers(&self.context)?;

        self.imgui.frame_count = self.imgui.frame_count.saturating_add(1);
        if self.imgui.frame_count > MAX_REGRESSION_FRAMES {
            return Err(boxed_error(format!(
                "regression scenario did not finish within {MAX_REGRESSION_FRAMES} frames"
            )));
        }

        self.verify_regression_step()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    window.window.request_redraw();
                    self.window = Some(window);
                }
                Err(err) => {
                    self.failure_message = Some(err.to_string());
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
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
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
            WindowEvent::RedrawRequested => match window.render() {
                Ok(Some(message)) => {
                    self.success_message = Some(message);
                    event_loop.exit();
                }
                Ok(None) => {
                    window.window.request_redraw();
                }
                Err(err) => {
                    self.failure_message = Some(err.to_string());
                    event_loop.exit();
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.success_message.is_none() && self.failure_message.is_none() {
            if let Some(window) = &self.window {
                window.window.request_redraw();
            }
        }
    }
}

fn fill_texture_pixels(texture: &mut dear_imgui_rs::texture::TextureData, phase: u32) {
    let mut pixels = vec![0u8; (TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize];
    let t = phase as f32 * 0.15;

    for y in 0..TEXTURE_HEIGHT {
        for x in 0..TEXTURE_WIDTH {
            let i = ((y * TEXTURE_WIDTH + x) * 4) as usize;
            let fx = x as f32 / TEXTURE_WIDTH as f32;
            let fy = y as f32 / TEXTURE_HEIGHT as f32;
            pixels[i + 0] = ((fx * 255.0 + t.sin() * 96.0).clamp(0.0, 255.0)) as u8;
            pixels[i + 1] = ((fy * 255.0 + (t * 1.4).cos() * 96.0).clamp(0.0, 255.0)) as u8;
            pixels[i + 2] = (((fx + fy + t * 0.25).sin().abs()) * 255.0) as u8;
            pixels[i + 3] = 255;
        }
    }

    texture.set_data(&pixels);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    if let Some(message) = app.failure_message.take() {
        return Err(boxed_error(message));
    }

    if let Some(message) = app.success_message.take() {
        println!("{message}");
    }

    Ok(())
}
