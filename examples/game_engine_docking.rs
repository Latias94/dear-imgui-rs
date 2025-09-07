use dear_imgui::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::Window,
};

/// Game engine state with various panels
struct GameEngineState {
    // Scene hierarchy
    selected_entity: Option<String>,
    entities: Vec<String>,
    
    // Inspector properties
    transform_position: [f32; 3],
    transform_rotation: [f32; 3],
    transform_scale: [f32; 3],
    
    // Console logs
    console_logs: Vec<String>,
    console_input: String,
    
    // Asset browser
    current_folder: String,
    assets: Vec<String>,
    
    // Viewport settings
    viewport_size: [f32; 2],
    show_wireframe: bool,
    show_grid: bool,
    
    // Project settings
    project_name: String,
    scene_name: String,
    
    // Performance stats
    fps: f32,
    frame_time: f32,
    draw_calls: u32,
    vertices: u32,
}

impl Default for GameEngineState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            entities: vec![
                "Main Camera".to_string(),
                "Directional Light".to_string(),
                "Player".to_string(),
                "Ground".to_string(),
                "Building_01".to_string(),
                "Building_02".to_string(),
                "Tree_01".to_string(),
                "Tree_02".to_string(),
            ],
            transform_position: [0.0, 0.0, 0.0],
            transform_rotation: [0.0, 0.0, 0.0],
            transform_scale: [1.0, 1.0, 1.0],
            console_logs: vec![
                "[INFO] Game engine initialized".to_string(),
                "[INFO] Renderer started".to_string(),
                "[INFO] Scene loaded: MainScene".to_string(),
                "[WARNING] Texture quality reduced for performance".to_string(),
            ],
            console_input: String::new(),
            current_folder: "Assets/".to_string(),
            assets: vec![
                "Textures/".to_string(),
                "Models/".to_string(),
                "Materials/".to_string(),
                "Scripts/".to_string(),
                "player_texture.png".to_string(),
                "building_model.fbx".to_string(),
                "wood_material.mat".to_string(),
                "player_controller.cs".to_string(),
            ],
            viewport_size: [800.0, 600.0],
            show_wireframe: false,
            show_grid: true,
            project_name: "My Game Project".to_string(),
            scene_name: "MainScene".to_string(),
            fps: 60.0,
            frame_time: 16.67,
            draw_calls: 45,
            vertices: 12543,
        }
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
    game_state: GameEngineState,
    dockspace_id: u32,
    first_frame: bool,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    hidpi_factor: f64,
    imgui: Option<ImguiState>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let version = env!("CARGO_PKG_VERSION");
            let size = LogicalSize::new(1600.0, 900.0); // Larger window for game engine UI

            Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(format!("Game Engine Docking Demo - dear-imgui {version}"))
                            .with_inner_size(size),
                    )
                    .expect("Failed to create window"),
            )
        };

        let hidpi_factor = window.scale_factor();

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
            },
        ))
        .expect("Failed to create device");

        let size = LogicalSize::new(1600.0, 900.0); // Larger window for game engine UI
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

        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            hidpi_factor,
            imgui: None,
        }
    }

    fn setup_imgui(&mut self) {
        let mut context = Context::create();
        context.set_ini_filename(Some("game_engine_docking.ini"));

        // Enable docking
        let io = context.io_mut();
        io.set_config_flags(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        let mut renderer = WgpuRenderer::new(&self.device, &self.queue, self.surface_desc.format);

        // Load font texture - this is crucial for text rendering!
        renderer.reload_font_texture(&mut context, &self.device, &self.queue);

        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        };

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color,
            last_frame: Instant::now(),
            game_state: GameEngineState::default(),
            dockspace_id: 0,
            first_frame: true,
        });
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        let delta_time = now - imgui.last_frame;
        imgui.last_frame = now;

        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        imgui.platform.prepare_frame(&self.window, &mut imgui.context);

        let ui = imgui.context.frame();
        
        // Create main dockspace
        imgui.dockspace_id = ui.dockspace_over_main_viewport();
        
        // Setup initial layout on first frame
        if imgui.first_frame {
            setup_initial_docking_layout(imgui.dockspace_id);
            imgui.first_frame = false;
        }

        // Render all panels
        render_main_menu_bar(&ui, &mut imgui.game_state);
        render_scene_hierarchy(&ui, &mut imgui.game_state);
        render_inspector(&ui, &mut imgui.game_state);
        render_viewport(&ui, &mut imgui.game_state);
        render_console(&ui, &mut imgui.game_state);
        render_asset_browser(&ui, &mut imgui.game_state);
        render_performance_stats(&ui, &mut imgui.game_state);

        let draw_data = imgui.context.render();

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            });

            imgui.renderer.render_with_renderpass(draw_data, &self.queue, &self.device, &mut render_pass).expect("Rendering failed");
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }
}

/// Setup the initial docking layout
fn setup_initial_docking_layout(_dockspace_id: u32) {
    // Note: DockBuilder functionality would be used here when fully implemented
    // For now, we'll let users manually arrange the docking layout
}

/// Render the main menu bar
fn render_main_menu_bar(ui: &Ui, game_state: &mut GameEngineState) {
    if let Some(_main_menu_bar) = ui.begin_main_menu_bar() {
        ui.menu("File", || {
            if ui.menu_item("New Scene") {
                game_state.console_logs.push("[INFO] New scene created".to_string());
            }
            if ui.menu_item("Open Scene") {
                game_state.console_logs.push("[INFO] Scene opened".to_string());
            }
            if ui.menu_item("Save Scene") {
                game_state.console_logs.push("[INFO] Scene saved".to_string());
            }
            ui.separator();
            if ui.menu_item("Exit") {
                // Handle exit
            }
        });

        ui.menu("Edit", || {
            ui.menu_item("Undo");
            ui.menu_item("Redo");
            ui.separator();
            ui.menu_item("Cut");
            ui.menu_item("Copy");
            ui.menu_item("Paste");
        });

        ui.menu("GameObject", || {
            if ui.menu_item("Create Empty") {
                game_state.entities.push("GameObject".to_string());
                game_state.console_logs.push("[INFO] Empty GameObject created".to_string());
            }
            ui.menu("3D Object", || {
                if ui.menu_item("Cube") {
                    game_state.entities.push("Cube".to_string());
                }
                if ui.menu_item("Sphere") {
                    game_state.entities.push("Sphere".to_string());
                }
                if ui.menu_item("Plane") {
                    game_state.entities.push("Plane".to_string());
                }
            });
        });

        ui.menu("Window", || {
            ui.menu_item("Scene Hierarchy");
            ui.menu_item("Inspector");
            ui.menu_item("Console");
            ui.menu_item("Asset Browser");
            ui.menu_item("Performance Stats");
        });

        ui.menu("Help", || {
            ui.menu_item("About");
            ui.menu_item("Documentation");
        });
    }
}

/// Render the scene hierarchy panel
fn render_scene_hierarchy(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Scene Hierarchy")
        .size([300.0, 400.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Scene: {}", game_state.scene_name));
            ui.separator();

            // Search filter
            ui.input_text("Search", &mut String::new()).build();

            ui.separator();

            // Entity list
            let mut selected_entity = None;
            let mut entity_to_duplicate = None;
            let mut entity_to_delete = None;

            for (_i, entity) in game_state.entities.iter().enumerate() {
                let is_selected = game_state.selected_entity.as_ref() == Some(entity);

                if ui.selectable_config(entity)
                    .selected(is_selected)
                    .build()
                {
                    selected_entity = Some(entity.clone());
                }

                // Right-click context menu
                if let Some(_popup) = ui.begin_popup_context_item() {
                    if ui.menu_item("Duplicate") {
                        entity_to_duplicate = Some(entity.clone());
                    }
                    if ui.menu_item("Delete") {
                        entity_to_delete = Some(entity.clone());
                    }
                    ui.separator();
                    ui.menu_item("Rename");
                }
            }

            // Handle actions outside the loop
            if let Some(entity) = selected_entity {
                game_state.selected_entity = Some(entity.clone());
                // Update inspector with selected entity data
                match entity.as_str() {
                    "Player" => {
                        game_state.transform_position = [5.0, 0.0, 3.0];
                        game_state.transform_rotation = [0.0, 45.0, 0.0];
                    }
                    "Main Camera" => {
                        game_state.transform_position = [0.0, 10.0, -10.0];
                        game_state.transform_rotation = [15.0, 0.0, 0.0];
                    }
                    _ => {
                        game_state.transform_position = [0.0, 0.0, 0.0];
                        game_state.transform_rotation = [0.0, 0.0, 0.0];
                    }
                }
            }

            if let Some(entity) = entity_to_duplicate {
                game_state.entities.push(format!("{} (Copy)", entity));
            }

            if let Some(entity) = entity_to_delete {
                game_state.console_logs.push(format!("[INFO] {} deleted", entity));
            }

            // Add new entity button
            if ui.button("+ Add Entity") {
                game_state.entities.push("New GameObject".to_string());
            }
        });
}

/// Render the inspector panel
fn render_inspector(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Inspector")
        .size([350.0, 500.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(ref selected) = game_state.selected_entity {
                ui.text(format!("Selected: {}", selected));
                ui.separator();

                // Transform component
                if ui.collapsing_header("Transform", TreeNodeFlags::DEFAULT_OPEN) {
                    ui.text("Position");
                    ui.drag_float("X##pos", &mut game_state.transform_position[0]);
                    ui.same_line();
                    ui.drag_float("Y##pos", &mut game_state.transform_position[1]);
                    ui.same_line();
                    ui.drag_float("Z##pos", &mut game_state.transform_position[2]);

                    ui.text("Rotation");
                    ui.drag_float("X##rot", &mut game_state.transform_rotation[0]);
                    ui.same_line();
                    ui.drag_float("Y##rot", &mut game_state.transform_rotation[1]);
                    ui.same_line();
                    ui.drag_float("Z##rot", &mut game_state.transform_rotation[2]);

                    ui.text("Scale");
                    ui.drag_float("X##scale", &mut game_state.transform_scale[0]);
                    ui.same_line();
                    ui.drag_float("Y##scale", &mut game_state.transform_scale[1]);
                    ui.same_line();
                    ui.drag_float("Z##scale", &mut game_state.transform_scale[2]);
                }

                // Renderer component (example)
                if ui.collapsing_header("Mesh Renderer", TreeNodeFlags::empty()) {
                    ui.text("Material: Default");
                    if ui.button("Select Material") {
                        game_state.console_logs.push("[INFO] Material selector opened".to_string());
                    }

                    ui.checkbox("Cast Shadows", &mut true);
                    ui.checkbox("Receive Shadows", &mut true);
                }

                // Collider component (example)
                if selected == "Player" {
                    if ui.collapsing_header("Box Collider", TreeNodeFlags::empty()) {
                        let mut is_trigger = false;
                        ui.checkbox("Is Trigger", &mut is_trigger);
                        ui.text("Size");
                        let mut size = [1.0, 1.0, 1.0];
                        ui.drag_float("X##size", &mut size[0]);
                        ui.same_line();
                        ui.drag_float("Y##size", &mut size[1]);
                        ui.same_line();
                        ui.drag_float("Z##size", &mut size[2]);
                    }
                }

                ui.separator();
                if ui.button("Add Component") {
                    ui.open_popup("add_component");
                }

                ui.popup("add_component", || {
                    if ui.menu_item("Rigidbody") {
                        game_state.console_logs.push("[INFO] Rigidbody component added".to_string());
                    }
                    if ui.menu_item("Audio Source") {
                        game_state.console_logs.push("[INFO] Audio Source component added".to_string());
                    }
                    if ui.menu_item("Script") {
                        game_state.console_logs.push("[INFO] Script component added".to_string());
                    }
                });

            } else {
                ui.text("No object selected");
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "Select an object in the Scene Hierarchy to view its properties");
            }
        });
}

/// Render the main viewport
fn render_viewport(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Viewport")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Viewport toolbar
            if ui.button("🎮 Play") {
                game_state.console_logs.push("[INFO] Play mode started".to_string());
            }
            ui.same_line();
            if ui.button("⏸ Pause") {
                game_state.console_logs.push("[INFO] Play mode paused".to_string());
            }
            ui.same_line();
            if ui.button("⏹ Stop") {
                game_state.console_logs.push("[INFO] Play mode stopped".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.checkbox("Wireframe", &mut game_state.show_wireframe);
            ui.same_line();
            ui.checkbox("Grid", &mut game_state.show_grid);

            ui.separator();

            // Main viewport area
            let content_region = ui.content_region_avail();
            game_state.viewport_size = [content_region[0], content_region[1]];

            // Only draw if we have a valid canvas size
            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                // Placeholder for actual 3D rendering
                let draw_list = ui.get_window_draw_list();
                let canvas_pos = ui.cursor_screen_pos();
                let canvas_size = content_region;

                // Draw a simple "3D scene" placeholder
                draw_list
                    .add_rect(
                        canvas_pos,
                        [canvas_pos[0] + canvas_size[0], canvas_pos[1] + canvas_size[1]],
                        [0.2, 0.3, 0.4, 1.0],
                    )
                    .filled(true)
                    .build();

                // Draw grid if enabled
                if game_state.show_grid {
                    let grid_step = 50.0;
                    let grid_color = [0.4, 0.4, 0.4, 0.5];

                    // Vertical lines
                    let mut x = canvas_pos[0];
                    while x < canvas_pos[0] + canvas_size[0] {
                        draw_list
                            .add_line(
                                [x, canvas_pos[1]],
                                [x, canvas_pos[1] + canvas_size[1]],
                                grid_color,
                            )
                            .thickness(1.0)
                            .build();
                        x += grid_step;
                    }

                    // Horizontal lines
                    let mut y = canvas_pos[1];
                    while y < canvas_pos[1] + canvas_size[1] {
                        draw_list
                            .add_line(
                                [canvas_pos[0], y],
                                [canvas_pos[0] + canvas_size[0], y],
                                grid_color,
                            )
                            .thickness(1.0)
                            .build();
                        y += grid_step;
                    }
                }

                // Draw some placeholder objects
                draw_list
                    .add_rect(
                        [canvas_pos[0] + 100.0, canvas_pos[1] + 100.0],
                        [canvas_pos[0] + 150.0, canvas_pos[1] + 150.0],
                        [1.0, 0.5, 0.2, 1.0],
                    )
                    .filled(true)
                    .build();

                draw_list
                    .add_circle(
                        [canvas_pos[0] + 200.0, canvas_pos[1] + 200.0],
                        30.0,
                        [0.2, 1.0, 0.5, 1.0],
                    )
                    .filled(true)
                    .num_segments(32)
                    .build();

                ui.text(format!("Viewport Size: {:.0}x{:.0}", canvas_size[0], canvas_size[1]));
            } else {
                ui.text("Viewport too small to render");
            }
        });
}

/// Render the console panel
fn render_console(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Console")
        .size([800.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            // Console toolbar
            if ui.button("Clear") {
                game_state.console_logs.clear();
            }
            ui.same_line();

            let mut show_info = true;
            let mut show_warnings = true;
            let mut show_errors = true;

            ui.checkbox("Info", &mut show_info);
            ui.same_line();
            ui.checkbox("Warnings", &mut show_warnings);
            ui.same_line();
            ui.checkbox("Errors", &mut show_errors);

            ui.separator();

            // Console output
            ui.child_window("console_output")
                .size([0.0, -25.0])
                .build(&ui, || {
                    for log in &game_state.console_logs {
                        let color = if log.contains("[ERROR]") {
                            [1.0, 0.4, 0.4, 1.0] // Red
                        } else if log.contains("[WARNING]") {
                            [1.0, 1.0, 0.4, 1.0] // Yellow
                        } else {
                            [1.0, 1.0, 1.0, 1.0] // White
                        };

                        ui.text_colored(color, log);
                    }

                    // Auto-scroll to bottom
                    if ui.scroll_y() >= ui.scroll_max_y() {
                        ui.set_scroll_here_y(1.0);
                    }
                });

            // Console input
            ui.separator();
            let mut enter_pressed = false;
            ui.input_text("##console_input", &mut game_state.console_input).build();
            if ui.is_item_focused() && ui.is_key_pressed(dear_imgui::Key::Enter) {
                enter_pressed = true;
            }
            if enter_pressed {
                if !game_state.console_input.is_empty() {
                    game_state.console_logs.push(format!("> {}", game_state.console_input));

                    // Simple command processing
                    match game_state.console_input.as_str() {
                        "help" => {
                            game_state.console_logs.push("[INFO] Available commands: help, clear, fps, version".to_string());
                        }
                        "clear" => {
                            game_state.console_logs.clear();
                        }
                        "fps" => {
                            game_state.console_logs.push(format!("[INFO] Current FPS: {:.1}", game_state.fps));
                        }
                        "version" => {
                            game_state.console_logs.push("[INFO] Game Engine v1.0.0".to_string());
                        }
                        _ => {
                            game_state.console_logs.push(format!("[ERROR] Unknown command: {}", game_state.console_input));
                        }
                    }

                    game_state.console_input.clear();
                }
            }
        });
}

/// Render the asset browser panel
fn render_asset_browser(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Asset Browser")
        .size([300.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            // Current folder path
            ui.text(format!("📁 {}", game_state.current_folder));
            ui.separator();

            // Navigation buttons
            if ui.button("⬆ Up") && game_state.current_folder != "Assets/" {
                game_state.current_folder = "Assets/".to_string();
            }
            ui.same_line();
            if ui.button("🔄 Refresh") {
                game_state.console_logs.push("[INFO] Asset browser refreshed".to_string());
            }

            ui.separator();

            // Asset grid
            let button_size = [80.0, 80.0];
            let mut items_per_row = (ui.content_region_avail()[0] / (button_size[0] + 8.0)) as i32;
            if items_per_row < 1 { items_per_row = 1; }

            for (i, asset) in game_state.assets.iter().enumerate() {
                if i > 0 && (i as i32) % items_per_row != 0 {
                    ui.same_line();
                }

                let is_folder = asset.ends_with('/');
                let icon = if is_folder { "📁" } else { "📄" };
                let display_name = if is_folder {
                    asset.trim_end_matches('/')
                } else {
                    asset
                };

                if ui.button_with_size(format!("{}\n{}", icon, display_name), button_size) {
                    if is_folder {
                        game_state.current_folder = format!("Assets/{}", asset);
                        game_state.console_logs.push(format!("[INFO] Opened folder: {}", asset));
                    } else {
                        game_state.console_logs.push(format!("[INFO] Selected asset: {}", asset));
                    }
                }

                // Right-click context menu
                if let Some(_popup) = ui.begin_popup_context_item() {
                    if ui.menu_item("Import") {
                        game_state.console_logs.push(format!("[INFO] Importing {}", asset));
                    }
                    if ui.menu_item("Delete") {
                        game_state.console_logs.push(format!("[WARNING] Deleted {}", asset));
                    }
                    ui.separator();
                    ui.menu_item("Properties");
                }
            }
        });
}

/// Render the performance stats panel
fn render_performance_stats(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Performance")
        .size([250.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Performance Statistics");
            ui.separator();

            // Update fake performance data
            game_state.fps = 60.0 + (ui.time() * 2.0).sin() as f32 * 5.0;
            game_state.frame_time = 1000.0 / game_state.fps;
            game_state.draw_calls = 45 + ((ui.time() * 0.5).sin() as f32 * 10.0) as u32;
            game_state.vertices = 12543 + ((ui.time() * 0.3).cos() as f32 * 1000.0) as u32;

            ui.text(format!("FPS: {:.1}", game_state.fps));
            ui.text(format!("Frame Time: {:.2}ms", game_state.frame_time));
            ui.text(format!("Draw Calls: {}", game_state.draw_calls));
            ui.text(format!("Vertices: {}", game_state.vertices));

            ui.separator();

            // Memory usage (fake data)
            let memory_used = 256.0 + (ui.time() * 0.1).sin() as f32 * 50.0;
            ui.text(format!("Memory: {:.1}MB", memory_used));

            // Simple performance graph
            ui.text("FPS Graph:");
            let fps_history: Vec<f32> = (0..60).map(|i| {
                60.0 + ((ui.time() - i as f64 * 0.1) * 2.0).sin() as f32 * 5.0
            }).collect();

            // Note: plot_lines might not be available in current API
            ui.text("(Graph visualization would go here)");
        });
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window = AppWindow::setup_gpu(event_loop);
            window.setup_imgui();
            self.window = Some(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        let window = self.window.as_mut().unwrap();

        // Handle platform events first
        {
            let imgui = window.imgui.as_mut().unwrap();
            // Create a fake Event::WindowEvent for the platform handler
            let fake_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                window_id: _window_id,
                event: event.clone(),
            };
            imgui.platform.handle_event(&fake_event, &window.window, &mut imgui.context);
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                window.surface_desc.width = physical_size.width;
                window.surface_desc.height = physical_size.height;
                window.surface.configure(&window.device, &window.surface_desc);
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
                let render_result = window.render();
                match render_result {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        window.surface.configure(&window.device, &window.surface_desc);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("OutOfMemory");
                        event_loop.exit();
                    }
                    Err(wgpu::SurfaceError::Timeout) => {
                        eprintln!("Surface timeout");
                    }
                    Err(wgpu::SurfaceError::Other) => {
                        eprintln!("Other surface error occurred");
                    }
                }
            }
            _ => {}
        }

        // Check input capture after handling events
        {
            let imgui = window.imgui.as_mut().unwrap();
            if !imgui.context.io().want_capture_mouse() && !imgui.context.io().want_capture_keyboard() {
                // Handle game input here when not captured by ImGui
            }
        }

        window.window.request_redraw();
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();

    println!("🎮 Game Engine Docking Demo");
    println!("Features:");
    println!("  • Scene Hierarchy - Manage game objects");
    println!("  • Inspector - Edit object properties");
    println!("  • Viewport - 3D scene view with controls");
    println!("  • Console - Command input and logging");
    println!("  • Asset Browser - File management");
    println!("  • Performance Stats - Real-time metrics");
    println!();
    println!("Controls:");
    println!("  • Drag panel tabs to rearrange layout");
    println!("  • Right-click on objects for context menus");
    println!("  • Use console commands: help, clear, fps, version");
    println!("  • Press ESC to exit");
    println!();

    event_loop.run_app(&mut app).unwrap();
}
