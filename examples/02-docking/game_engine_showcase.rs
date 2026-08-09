//! A compact game-editor showcase built from the public Winit and WGPU contracts.
//!
//! The example intentionally keeps the lifecycle visible: input is routed through the platform
//! backend before application shortcuts, two application-owned render targets are registered with
//! the renderer, and shutdown unregisters those textures before releasing renderer and platform
//! attachments.

#[path = "game_engine_showcase_scene.rs"]
mod scene;

use dear_imgui_rs::*;
use dear_imgui_wgpu::{FramebufferExtent, WgpuRenderer};
use dear_imgui_winit::WinitPlatform;
use glam::{Mat4, Quat, Vec3, Vec4};
use pollster::block_on;
use scene::{RenderTarget, SceneRenderer};
use std::{error::Error, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

#[cfg(feature = "imguizmo")]
use dear_imguizmo::{GuizmoExt, Mode, Operation};

type AppResult<T> = Result<T, Box<dyn Error>>;
const MAX_ENTITIES: usize = 96;

#[derive(Clone)]
struct SceneEntity {
    name: String,
    position: [f32; 3],
    rotation_deg: [f32; 3],
    scale: [f32; 3],
}

impl SceneEntity {
    fn cube(name: impl Into<String>, position: [f32; 3]) -> Self {
        Self {
            name: name.into(),
            position,
            rotation_deg: [0.0; 3],
            scale: [1.0; 3],
        }
    }

    fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_euler(
                glam::EulerRot::XYZ,
                self.rotation_deg[0].to_radians(),
                self.rotation_deg[1].to_radians(),
                self.rotation_deg[2].to_radians(),
            ),
            Vec3::from_array(self.position),
        )
    }

    #[cfg(feature = "imguizmo")]
    fn set_from_matrix(&mut self, model: Mat4) {
        let (scale, rotation, translation) = model.to_scale_rotation_translation();
        let (x, y, z) = rotation.to_euler(glam::EulerRot::XYZ);
        self.position = translation.to_array();
        self.rotation_deg = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
        self.scale = scale.to_array().map(|axis| axis.abs().max(0.05));
    }
}

struct EditorState {
    entities: Vec<SceneEntity>,
    selected: Option<usize>,
    assets: Vec<&'static str>,
    asset_filter: ImString,
    console_lines: Vec<String>,
    console_input: ImString,
    show_grid: bool,
    camera_distance: f32,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_view: Mat4,
    camera_proj: Mat4,
    #[cfg(feature = "imguizmo")]
    gizmo_operation: Operation,
    #[cfg(feature = "imguizmo")]
    gizmo_mode: Mode,
}

impl Default for EditorState {
    fn default() -> Self {
        let mut state = Self {
            entities: vec![
                SceneEntity::cube("Cube", [-1.0, 0.5, 0.0]),
                SceneEntity::cube("Cube Two", [1.0, 0.5, 0.0]),
            ],
            selected: Some(0),
            assets: vec![
                "Textures/",
                "Models/",
                "Materials/",
                "Scripts/",
                "checker.png",
                "crate.glb",
                "default.mat",
                "player.rs",
            ],
            asset_filter: ImString::new(""),
            console_lines: vec![
                "[info] editor initialized".to_owned(),
                "[info] scene and game render targets registered".to_owned(),
            ],
            console_input: ImString::new(""),
            show_grid: true,
            camera_distance: 6.0,
            camera_yaw: 45.0_f32.to_radians(),
            camera_pitch: 28.0_f32.to_radians(),
            camera_view: Mat4::IDENTITY,
            camera_proj: Mat4::IDENTITY,
            #[cfg(feature = "imguizmo")]
            gizmo_operation: Operation::TRANSLATE,
            #[cfg(feature = "imguizmo")]
            gizmo_mode: Mode::World,
        };
        state.update_camera(1.0);
        state
    }
}

impl EditorState {
    fn log(&mut self, message: impl Into<String>) {
        self.console_lines.push(message.into());
    }

    fn spawn_cube(&mut self) {
        if self.entities.len() >= MAX_ENTITIES {
            self.log(format!(
                "[error] this showcase is limited to {MAX_ENTITIES} scene entities"
            ));
            return;
        }
        let number = self.entities.len() + 1;
        let column = (number - 1) % 3;
        let row = (number - 1) / 3;
        let position = [column as f32 * 1.5 - 1.5, 0.5, -(row as f32) * 1.5];
        self.entities
            .push(SceneEntity::cube(format!("Cube {number}"), position));
        self.selected = Some(self.entities.len() - 1);
        self.log(format!("[info] spawned Cube {number}"));
    }

    fn duplicate_selected(&mut self) {
        let Some(index) = self.selected else {
            return;
        };
        let Some(mut entity) = self.entities.get(index).cloned() else {
            self.selected = None;
            return;
        };
        entity.name.push_str(" Copy");
        entity.position[0] += 0.75;
        self.entities.push(entity);
        self.selected = Some(self.entities.len() - 1);
        self.log("[info] duplicated selected entity");
    }

    fn delete_selected(&mut self) {
        let Some(index) = self.selected.take() else {
            return;
        };
        if index < self.entities.len() {
            let entity = self.entities.remove(index);
            self.log(format!("[info] deleted {}", entity.name));
        }
        self.selected = (!self.entities.is_empty()).then_some(index.min(self.entities.len() - 1));
    }

    fn update_camera(&mut self, aspect: f32) {
        let eye = Vec3::new(
            self.camera_distance * self.camera_yaw.cos() * self.camera_pitch.cos(),
            self.camera_distance * self.camera_pitch.sin(),
            self.camera_distance * self.camera_yaw.sin() * self.camera_pitch.cos(),
        );
        self.camera_view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        self.camera_proj =
            Mat4::perspective_rh(45.0_f32.to_radians(), aspect.max(0.01), 0.1, 100.0);
    }

    fn process_console_command(&mut self) {
        let command = self.console_input.to_str().trim().to_owned();
        self.console_input.clear();
        if command.is_empty() {
            return;
        }

        self.log(format!("> {command}"));
        match command.as_str() {
            "clear" => self.console_lines.clear(),
            "help" => self.log("[info] commands: clear, help, spawn"),
            "spawn" => self.spawn_cube(),
            _ => self.log(format!("[error] unknown command: {command}")),
        }
    }

    fn route_application_input(
        &mut self,
        event: &WindowEvent,
        capture_mouse: bool,
        capture_keyboard: bool,
    ) {
        match event {
            WindowEvent::MouseWheel { delta, .. } if !capture_mouse => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 40.0,
                };
                self.camera_distance = (self.camera_distance - lines * 0.35).clamp(2.0, 14.0);
            }
            WindowEvent::KeyboardInput { event, .. }
                if !capture_keyboard
                    && event.state == ElementState::Pressed
                    && !event.repeat
                    && matches!(&event.logical_key, Key::Character(key) if key.eq_ignore_ascii_case("n")) =>
            {
                self.spawn_cube();
            }
            _ => {}
        }
    }
}

struct ImguiState {
    renderer: WgpuRenderer,
    platform: WinitPlatform,
    scene_target: RenderTarget,
    game_target: RenderTarget,
    scene_renderer: SceneRenderer,
    editor: EditorState,
    clear_color: wgpu::Color,
    last_frame: Instant,
    layout_dirty: bool,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Context must outlive every attachment, including fallback field drops after a failed shutdown.
    context: Context,
}

impl ImguiState {
    fn register_render_targets(&mut self) -> AppResult<()> {
        let (renderer, scene_target) = (&mut self.renderer, &mut self.scene_target);
        scene_target.register_with(|view| renderer.register_external_texture(view))?;

        let (renderer, game_target) = (&mut self.renderer, &mut self.game_target);
        game_target.register_with(|view| renderer.register_external_texture(view))?;
        Ok(())
    }

    fn shutdown(&mut self) -> AppResult<()> {
        if !self.renderer_shutdown_complete {
            let (renderer, scene_target) = (&mut self.renderer, &mut self.scene_target);
            scene_target
                .unregister_with(|texture| renderer.unregister_external_texture(texture))?;

            let (renderer, game_target) = (&mut self.renderer, &mut self.game_target);
            game_target.unregister_with(|texture| renderer.unregister_external_texture(texture))?;

            self.renderer.shutdown(&mut self.context)?;
            self.renderer_shutdown_complete = true;
        }

        if !self.platform_shutdown_complete {
            self.platform.shutdown(&mut self.context)?;
            self.platform_shutdown_complete = true;
        }
        Ok(())
    }
}

impl Drop for ImguiState {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("game engine showcase fallback shutdown failed: {error}");
        }
    }
}

struct AppWindow {
    // ImGui owns renderer registrations that must be released before the WGPU and window handles.
    imgui: Option<ImguiState>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    queue: wgpu::Queue,
    device: wgpu::Device,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    error: Option<String>,
}

impl AppWindow {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> AppResult<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let version = env!("CARGO_PKG_VERSION");
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(format!("dear-imgui game engine showcase {version}"))
                    .with_inner_size(LogicalSize::new(1440.0, 900.0)),
            )?,
        );
        let surface = instance.create_surface(Arc::clone(&window))?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("game-engine-showcase-device"),
            ..Default::default()
        }))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .find(|format| capabilities.formats.contains(format))
        .or_else(|| capabilities.formats.first().copied())
        .ok_or("surface reported no supported texture format")?;
        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        Ok(Self {
            imgui: None,
            surface,
            surface_config,
            window,
            queue,
            device,
        })
    }

    fn setup_imgui(&mut self) -> AppResult<()> {
        let mut context = Context::create();
        context.set_ini_filename::<std::path::PathBuf>(None)?;
        let mut config_flags = context.io().config_flags();
        config_flags.insert(ConfigFlags::DOCKING_ENABLE);
        context.io_mut().set_config_flags(config_flags);
        context
            .io_mut()
            .set_config_windows_move_from_title_bar_only(true);

        let mut platform = WinitPlatform::new(&mut context)?;
        platform.attach_window(
            Arc::clone(&self.window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        )?;
        let init_info = dear_imgui_wgpu::WgpuInitInfo::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_config.format,
        );
        let mut renderer = WgpuRenderer::new(init_info, &mut context)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        let scene_renderer = SceneRenderer::new(&self.device, self.surface_config.format);
        let scene_target = RenderTarget::create(
            &self.device,
            self.surface_config.format,
            "game-engine-scene-target",
        );
        let game_target = RenderTarget::create(
            &self.device,
            self.surface_config.format,
            "game-engine-game-target",
        );

        let clear_color = wgpu::Color {
            r: 0.055,
            g: 0.065,
            b: 0.085,
            a: 1.0,
        };
        let mut imgui = ImguiState {
            renderer,
            platform,
            scene_target,
            game_target,
            scene_renderer,
            editor: EditorState::default(),
            clear_color,
            last_frame: Instant::now(),
            layout_dirty: true,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            context,
        };
        imgui.register_render_targets()?;
        self.imgui = Some(imgui);
        Ok(())
    }

    fn render(&mut self) -> AppResult<()> {
        let imgui = self.imgui.as_mut().ok_or("Dear ImGui is not initialized")?;
        let now = Instant::now();
        imgui
            .context
            .io_mut()
            .set_delta_time((now - imgui.last_frame).as_secs_f32());
        imgui.last_frame = now;
        imgui
            .platform
            .prepare_frame(&mut imgui.context, &self.window)?;

        let frame_token = imgui.context.begin_frame();
        let ui = frame_token.ui();
        let dockspace_id = ui.get_id("GameEngineShowcaseDockspace");
        let apply = if imgui.layout_dirty {
            DockLayoutApply::Replace
        } else {
            DockLayoutApply::IfMissing
        };
        ui.dockspace()
            .root_id(dockspace_id)
            .flags(DockNodeFlags::PASSTHRU_CENTRAL_NODE)
            .layout(&initial_layout(), apply)
            .build()?;
        imgui.layout_dirty = false;

        let reset_layout = render_menu(ui, &mut imgui.editor);
        render_hierarchy(ui, &mut imgui.editor);
        render_inspector(ui, &mut imgui.editor);
        render_scene_view(ui, &mut imgui.editor, imgui.scene_target.texture_id());
        render_game_view(ui, &imgui.editor, imgui.game_target.texture_id());
        render_assets(ui, &mut imgui.editor);
        render_console(ui, &mut imgui.editor);

        imgui.platform.prepare_render(ui, &self.window)?;
        let pending = frame_token.try_render(imgui.renderer.renderer_consumer()?)?;
        let prepared = imgui.renderer.reconcile_frame(pending)?;

        let (surface_frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };
        let framebuffer_extent = FramebufferExtent::from_texture(&surface_frame.texture);
        let surface_view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("game-engine-showcase-encoder"),
            });

        let models = imgui
            .editor
            .entities
            .iter()
            .map(SceneEntity::model_matrix)
            .collect::<Vec<_>>();
        imgui.scene_target.render_into(
            &mut encoder,
            &imgui.scene_renderer,
            &self.queue,
            imgui.editor.camera_view,
            imgui.editor.camera_proj,
            &models,
            imgui.editor.show_grid,
        );
        imgui.game_target.render_into(
            &mut encoder,
            &imgui.scene_renderer,
            &self.queue,
            Mat4::look_at_rh(Vec3::new(4.0, 3.2, 4.0), Vec3::ZERO, Vec3::Y),
            Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0),
            &models,
            true,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game-engine-showcase-imgui-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            imgui
                .renderer
                .render_reconciled(prepared, &mut render_pass, framebuffer_extent)?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(surface_frame);
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.surface_config);
        }
        if reset_layout {
            imgui.layout_dirty = true;
        }
        Ok(())
    }
}

fn editor_window(title: &str) -> WindowKey {
    WindowKey::new(title, title).expect("showcase window keys are static and valid")
}

fn initial_layout() -> DockLayout {
    DockLayout::split(
        DockSplit::Right,
        0.23,
        DockLayout::tabs([editor_window("Inspector")]),
        DockLayout::split(
            DockSplit::Left,
            0.21,
            DockLayout::split(
                DockSplit::Down,
                0.45,
                DockLayout::tabs([editor_window("Assets")]),
                DockLayout::tabs([editor_window("Hierarchy")]),
            ),
            DockLayout::split(
                DockSplit::Down,
                0.27,
                DockLayout::tabs([editor_window("Console")]),
                DockLayout::tabs([editor_window("Scene"), editor_window("Game")]),
            ),
        ),
    )
}

fn render_menu(ui: &Ui, editor: &mut EditorState) -> bool {
    let mut reset_layout = false;
    if let Some(_menu_bar) = ui.begin_main_menu_bar() {
        ui.menu("Scene", || {
            if ui.menu_item("Add Cube") {
                editor.spawn_cube();
            }
            if ui.menu_item("Duplicate Selected") {
                editor.duplicate_selected();
            }
            if ui.menu_item("Delete Selected") {
                editor.delete_selected();
            }
        });
        ui.menu("Layout", || {
            if ui.menu_item("Reset") {
                reset_layout = true;
            }
        });
        ui.text_disabled("N: spawn cube when Dear ImGui is not capturing keyboard input");
    }
    reset_layout
}

fn render_hierarchy(ui: &Ui, editor: &mut EditorState) {
    ui.window(&editor_window("Hierarchy")).build(|| {
        ui.text("Scene");
        ui.separator();
        for (index, entity) in editor.entities.iter().enumerate() {
            if ui
                .selectable_config(&entity.name)
                .selected(editor.selected == Some(index))
                .build()
            {
                editor.selected = Some(index);
            }
        }
        ui.separator();
        if ui.button("Add") {
            editor.spawn_cube();
        }
        ui.same_line();
        if ui.button("Duplicate") {
            editor.duplicate_selected();
        }
        ui.same_line();
        if ui.button("Delete") {
            editor.delete_selected();
        }
    });
}

fn render_inspector(ui: &Ui, editor: &mut EditorState) {
    ui.window(&editor_window("Inspector")).build(|| {
        let Some(index) = editor.selected else {
            ui.text_disabled("Select an entity in Hierarchy or Scene.");
            return;
        };
        let Some(entity) = editor.entities.get_mut(index) else {
            editor.selected = None;
            ui.text_disabled("The selected entity no longer exists.");
            return;
        };

        ui.text(&entity.name);
        ui.separator();
        if ui.collapsing_header("Transform", TreeNodeFlags::DEFAULT_OPEN) {
            ui.drag_float3("Position", &mut entity.position);
            ui.drag_float3("Rotation", &mut entity.rotation_deg);
            if ui.drag_float3("Scale", &mut entity.scale) {
                for axis in &mut entity.scale {
                    *axis = axis.abs().max(0.05);
                }
            }
        }
        ui.text_wrapped("Inspector changes feed both render targets in the same frame.");
    });
}

fn render_assets(ui: &Ui, editor: &mut EditorState) {
    ui.window(&editor_window("Assets")).build(|| {
        ui.set_next_item_width(-1.0);
        ui.input_text_imstr("##asset-filter", &mut editor.asset_filter)
            .hint("Filter assets")
            .build();
        ui.separator();

        let query = editor.asset_filter.to_str().to_ascii_lowercase();
        let button_size = [96.0, 58.0];
        let columns = (ui.content_region_avail()[0] / (button_size[0] + 8.0))
            .floor()
            .max(1.0) as usize;
        let visible_assets = editor
            .assets
            .iter()
            .copied()
            .filter(|asset| query.is_empty() || asset.to_ascii_lowercase().contains(&query))
            .collect::<Vec<_>>();
        for (visible_index, asset) in visible_assets.into_iter().enumerate() {
            if visible_index % columns != 0 {
                ui.same_line();
            }
            let kind = if asset.ends_with('/') { "DIR" } else { "FILE" };
            if ui.button_with_size(
                format!("[{kind}]\n{}", asset.trim_end_matches('/')),
                button_size,
            ) {
                editor.log(format!("[info] selected asset {asset}"));
            }
        }
    });
}

fn render_console(ui: &Ui, editor: &mut EditorState) {
    ui.window(&editor_window("Console")).build(|| {
        if ui.button("Clear") {
            editor.console_lines.clear();
        }
        ui.same_line();
        ui.text_disabled("Commands: help, spawn, clear");
        ui.separator();

        ui.child_window("console-output")
            .size([0.0, -32.0])
            .build(ui, || {
                for line in &editor.console_lines {
                    let color = if line.starts_with("[error]") {
                        [1.0, 0.45, 0.45, 1.0]
                    } else {
                        [0.82, 0.86, 0.92, 1.0]
                    };
                    ui.text_colored(color, line);
                }
                if ui.scroll_y() >= ui.scroll_max_y() {
                    ui.set_scroll_here_y(1.0);
                }
            });

        ui.set_next_item_width(-1.0);
        if ui
            .input_text_imstr("##console-input", &mut editor.console_input)
            .hint("Enter a command")
            .enter_returns_true(true)
            .build()
        {
            editor.process_console_command();
        }
    });
}

fn render_scene_view(ui: &Ui, editor: &mut EditorState, texture_id: Option<TextureId>) {
    ui.window(&editor_window("Scene")).build(|| {
        #[cfg(feature = "imguizmo")]
        {
            if ui.button("Move") {
                editor.gizmo_operation = Operation::TRANSLATE;
            }
            ui.same_line();
            if ui.button("Rotate") {
                editor.gizmo_operation = Operation::ROTATE;
            }
            ui.same_line();
            if ui.button("Scale") {
                editor.gizmo_operation = Operation::SCALE;
            }
            ui.same_line();
            if ui.button("Local") {
                editor.gizmo_mode = Mode::Local;
            }
            ui.same_line();
            if ui.button("World") {
                editor.gizmo_mode = Mode::World;
            }
            ui.same_line();
        }
        ui.checkbox("Grid", &mut editor.show_grid);
        ui.same_line();
        ui.text_disabled("Click a cube to select it");
        ui.separator();

        let available = ui.content_region_avail();
        if available[0] < 32.0 || available[1] < 32.0 {
            ui.text_disabled("Scene view is too small.");
            return;
        }
        let canvas_size = [available[0], available[1].max(32.0)];
        editor.update_camera(canvas_size[0] / canvas_size[1]);
        let Some(texture_id) = texture_id else {
            ui.text_disabled("Scene render target is not registered.");
            return;
        };

        #[cfg(feature = "imguizmo")]
        let canvas_position = ui.cursor_screen_pos();
        ui.image(texture_id, canvas_size);
        let image_min = ui.item_rect_min();
        let image_max = ui.item_rect_max();
        let image_size = [image_max[0] - image_min[0], image_max[1] - image_min[1]];
        let pick_requested = ui.is_item_hovered() && ui.is_item_clicked();

        #[cfg(feature = "imguizmo")]
        let gizmo_blocks_pick = {
            let gizmo = ui.guizmo();
            gizmo.set_drawlist_window();
            gizmo.set_rect(
                canvas_position[0],
                canvas_position[1],
                canvas_size[0],
                canvas_size[1],
            );
            if let Some(entity) = editor
                .selected
                .and_then(|index| editor.entities.get_mut(index))
            {
                let mut model = entity.model_matrix();
                if gizmo
                    .manipulate_config(&editor.camera_view, &editor.camera_proj, &mut model)
                    .operation(editor.gizmo_operation)
                    .mode(editor.gizmo_mode)
                    .build()
                {
                    entity.set_from_matrix(model);
                }
            }
            gizmo.is_over() || gizmo.is_using()
        };
        #[cfg(not(feature = "imguizmo"))]
        let gizmo_blocks_pick = false;

        if pick_requested
            && !gizmo_blocks_pick
            && let Some((origin, direction)) = ray_from_screen_rect(
                ui.mouse_pos(),
                image_min,
                image_size,
                editor.camera_view,
                editor.camera_proj,
            )
        {
            editor.selected = pick_cube(&editor.entities, origin, direction);
        }
    });
}

fn render_game_view(ui: &Ui, editor: &EditorState, texture_id: Option<TextureId>) {
    ui.window(&editor_window("Game")).build(|| {
        ui.text("Runtime camera");
        ui.same_line();
        ui.text_disabled(format!("{} entities", editor.entities.len()));
        ui.separator();
        let available = ui.content_region_avail();
        let width = available[0].min(available[1] * 16.0 / 9.0).max(32.0);
        let size = [width, width * 9.0 / 16.0];
        if let Some(texture_id) = texture_id {
            ui.image(texture_id, size);
        } else {
            ui.text_disabled("Game render target is not registered.");
        }
    });
}

fn ray_from_screen_rect(
    mouse: [f32; 2],
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    view: Mat4,
    projection: Mat4,
) -> Option<(Vec3, Vec3)> {
    if rect_size[0] <= 0.0 || rect_size[1] <= 0.0 {
        return None;
    }
    let u = (mouse[0] - rect_min[0]) / rect_size[0];
    let v = (mouse[1] - rect_min[1]) / rect_size[1];
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }

    let inverse_view_projection = (projection * view).inverse();
    let ndc = [u * 2.0 - 1.0, 1.0 - v * 2.0];
    let near = inverse_view_projection * Vec4::new(ndc[0], ndc[1], 0.0, 1.0);
    let far = inverse_view_projection * Vec4::new(ndc[0], ndc[1], 1.0, 1.0);
    let near = near.truncate() / near.w;
    let far = far.truncate() / far.w;
    Some((near, (far - near).normalize()))
}

fn ray_aabb(origin: Vec3, direction: Vec3, minimum: Vec3, maximum: Vec3) -> Option<f32> {
    let mut entry = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    for axis in 0..3 {
        let origin = origin[axis];
        let direction = direction[axis];
        let minimum = minimum[axis];
        let maximum = maximum[axis];
        if direction.abs() <= f32::EPSILON {
            if origin < minimum || origin > maximum {
                return None;
            }
            continue;
        }

        let first = (minimum - origin) / direction;
        let second = (maximum - origin) / direction;
        entry = entry.max(first.min(second));
        exit = exit.min(first.max(second));
        if exit < entry {
            return None;
        }
    }
    (exit >= entry.max(0.0)).then_some(entry.max(0.0))
}

fn pick_cube(entities: &[SceneEntity], ray_origin: Vec3, ray_direction: Vec3) -> Option<usize> {
    entities
        .iter()
        .enumerate()
        .filter_map(|(index, entity)| {
            let model = entity.model_matrix();
            let determinant = model.determinant();
            if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
                return None;
            }
            let inverse_model = model.inverse();
            let origin = (inverse_model * ray_origin.extend(1.0)).truncate();
            let direction = (inverse_model * ray_direction.extend(0.0)).truncate();
            ray_aabb(origin, direction, Vec3::splat(-0.5), Vec3::splat(0.5))
                .map(|distance| (index, distance))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(index, _)| index)
}

impl App {
    fn shutdown(&mut self) -> AppResult<()> {
        self.window
            .as_mut()
            .and_then(|window| window.imgui.as_mut())
            .map_or(Ok(()), ImguiState::shutdown)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let result = AppWindow::setup_gpu(event_loop).and_then(|mut window| {
            window.setup_imgui()?;
            Ok(window)
        });
        match result {
            Ok(window) => {
                window.window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                eprintln!("failed to initialize game engine showcase: {error}");
                self.error = Some(error.to_string());
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(main_window_id) = self.window.as_ref().map(|window| window.window.id()) else {
            return;
        };
        let is_main_window = window_id == main_window_id;
        if is_main_window
            && (matches!(&event, WindowEvent::CloseRequested)
                || matches!(
                    &event,
                    WindowEvent::KeyboardInput { event, .. }
                        if event.logical_key == Key::Named(NamedKey::Escape)
                ))
        {
            event_loop.exit();
            return;
        }

        let Some(window) = self.window.as_mut() else {
            return;
        };
        let platform_result = {
            let Some(imgui) = window.imgui.as_mut() else {
                return;
            };
            imgui
                .platform
                .handle_window_event(&mut imgui.context, &window.window, &event)
        };
        if let Err(error) = platform_result {
            eprintln!("Winit platform event failed: {error}");
            self.error = Some(error.to_string());
            event_loop.exit();
            return;
        }

        if is_main_window {
            let Some(imgui) = window.imgui.as_mut() else {
                return;
            };
            let io = imgui.context.io();
            let capture_mouse = io.want_capture_mouse();
            let capture_keyboard = io.want_capture_keyboard();
            imgui
                .editor
                .route_application_input(&event, capture_mouse, capture_keyboard);
        }

        match event {
            WindowEvent::Resized(size) if is_main_window && size.width > 0 && size.height > 0 => {
                window.surface_config.width = size.width;
                window.surface_config.height = size.height;
                window
                    .surface
                    .configure(&window.device, &window.surface_config);
                window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } if is_main_window => {
                let size = window.window.inner_size();
                if size.width > 0 && size.height > 0 {
                    window.surface_config.width = size.width;
                    window.surface_config.height = size.height;
                    window
                        .surface
                        .configure(&window.device, &window.surface_config);
                    window.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested if is_main_window => {
                let result = window.render();
                if let Err(error) = result {
                    eprintln!("render failed: {error}");
                    self.error = Some(error.to_string());
                    event_loop.exit();
                    return;
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

fn main() -> AppResult<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    println!("dear-imgui game engine showcase");
    println!("- drag tabs to change the docking layout");
    println!("- click Scene cubes, edit transforms, and use ImGuizmo when enabled");
    println!("- press N outside captured UI input to spawn a cube");
    println!("- press Escape to exit");

    let mut app = App::default();
    let event_loop_result = event_loop.run_app(&mut app);
    let application_error = app.error.take();
    let shutdown_result = app.shutdown();
    drop(app);

    let mut errors = Vec::new();
    if let Err(error) = event_loop_result {
        errors.push(format!("event loop failed: {error}"));
    }
    if let Some(error) = application_error {
        errors.push(error);
    }
    if let Err(error) = shutdown_result {
        errors.push(format!("shutdown failed: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}
