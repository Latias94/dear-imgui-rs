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

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::default(),
        }))
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
        let mut context = Context::create_or_panic();
        context.set_ini_filename_or_panic(Some("game_engine_docking.ini"));

        // Enable docking
        let io = context.io_mut();
        io.set_config_flags(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(
            &self.window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        );

        let mut renderer = WgpuRenderer::new();

        // Initialize renderer with device and queue
        let init_info = dear_imgui_wgpu::WgpuInitInfo::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_desc.format,
        );
        renderer
            .init(init_info)
            .expect("Failed to initialize WGPU renderer");

        // Load font texture - this is crucial for text rendering!
        renderer
            .prepare_font_atlas(&mut context)
            .expect("Failed to prepare font atlas");

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
        println!("Starting render...");
        let imgui = self.imgui.as_mut().unwrap();

        let now = Instant::now();
        let delta_time = now - imgui.last_frame;
        println!("Setting delta time: {:.6}", delta_time.as_secs_f32());
        imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        imgui.last_frame = now;

        println!("Getting surface texture...");
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        println!("Preparing frame...");
        imgui
            .platform
            .prepare_frame(&self.window, &mut imgui.context);

        println!("Creating UI frame...");
        let ui = imgui.context.frame();

        // Test 0: Check ImGui context validity
        unsafe {
            let ctx = dear_imgui::sys::ImGui_GetCurrentContext();
            if ctx.is_null() {
                println!("❌ ERROR: ImGui context is NULL!");
                return Ok(());
            } else {
                println!("✓ ImGui context is valid: {:p}", ctx);
            }

            // Test a simple function that returns a scalar
            let time = dear_imgui::sys::ImGui_GetTime();
            println!("✓ ImGui time: {} (should be > 0)", time);
            if time <= 0.0 {
                println!("❌ ERROR: ImGui_GetTime returned invalid value!");
            }
        }

        // Test 1: Simple window without docking
        println!("Creating simple test window...");
        ui.window("Test Window")
            .size([300.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Hello, World!");
                ui.text("This is a test window");
                if ui.button("Test Button") {
                    println!("Button clicked!");
                }

                // Test inside the window context where ImGui state should be valid
                println!("Testing inside window context...");

                // Test window size (should work)
                unsafe {
                    #[cfg(target_env = "msvc")]
                    {
                        let win_size = dear_imgui::sys::ImGui_GetWindowSize();
                        println!(
                            "Window size inside context (MSVC): x={}, y={}",
                            win_size.x, win_size.y
                        );
                    }
                    #[cfg(not(target_env = "msvc"))]
                    {
                        let win_size = dear_imgui::sys::ImGui_GetWindowSize();
                        println!(
                            "Window size inside context (non-MSVC): x={}, y={}",
                            win_size.x, win_size.y
                        );
                    }
                }

                // Test content region (problematic function)
                let content = ui.content_region_avail();
                println!("Content region inside context: {:?}", content);

                // Test if we're inside a window
                unsafe {
                    let in_window = dear_imgui::sys::ImGui_IsWindowAppearing();
                    println!("Is window appearing: {}", in_window);

                    // Test window position (another ImVec2 function)
                    #[cfg(target_env = "msvc")]
                    {
                        let win_pos = dear_imgui::sys::ImGui_GetWindowPos();
                        println!("Window pos (MSVC): x={}, y={}", win_pos.x, win_pos.y);
                    }
                }
            });

        // Test 2: Test a simple function first that doesn't return ImVec2
        let window_width = unsafe { dear_imgui::sys::ImGui_GetWindowWidth() };
        println!("Window width: {}", window_width);

        // Test 3: Try content_region_avail with detailed debugging
        println!("Testing content_region_avail...");

        // First test the raw POD function
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let raw_pod = dear_imgui::sys::ImGui_GetContentRegionAvail();
                println!("Raw POD result: x={}, y={}", raw_pod.x, raw_pod.y);

                // Test conversion
                let converted: dear_imgui::sys::ImVec2 = raw_pod.into();
                println!("After conversion: x={}, y={}", converted.x, converted.y);
            }
        }

        // Now test the high-level API
        let content_region = ui.content_region_avail();
        println!("High-level API result: {:?}", content_region);

        // Test if values are reasonable (should be positive and within window bounds)
        if content_region[0] < 0.0
            || content_region[1] < 0.0
            || content_region[0] > 10000.0
            || content_region[1] > 10000.0
        {
            println!("⚠️  WARNING: content_region_avail returned suspicious values!");
        }

        // Test 3: Try cursor_screen_pos (another potentially problematic call)
        println!("Testing cursor_screen_pos...");
        let cursor_pos = ui.cursor_screen_pos();
        println!("✓ cursor_screen_pos succeeded: {:?}", cursor_pos);

        // Test if values are reasonable
        if cursor_pos[0] < -1000.0
            || cursor_pos[1] < -1000.0
            || cursor_pos[0] > 10000.0
            || cursor_pos[1] > 10000.0
        {
            println!("⚠️  WARNING: cursor_screen_pos returned suspicious values!");
        }

        // Test 4: Try direct sys calls to compare
        println!("Testing direct sys calls...");
        unsafe {
            #[cfg(target_env = "msvc")]
            {
                let direct_content = dear_imgui::sys::ImGui_GetContentRegionAvail();
                println!(
                    "Direct MSVC content_region_avail: x={}, y={}",
                    direct_content.x, direct_content.y
                );

                let direct_cursor = dear_imgui::sys::ImGui_GetCursorScreenPos();
                println!(
                    "Direct MSVC cursor_screen_pos: x={}, y={}",
                    direct_cursor.x, direct_cursor.y
                );
            }
            #[cfg(not(target_env = "msvc"))]
            {
                let direct_content = dear_imgui::sys::ImGui_GetContentRegionAvail();
                println!(
                    "Direct non-MSVC content_region_avail: x={}, y={}",
                    direct_content.x, direct_content.y
                );

                let direct_cursor = dear_imgui::sys::ImGui_GetCursorScreenPos();
                println!(
                    "Direct non-MSVC cursor_screen_pos: x={}, y={}",
                    direct_cursor.x, direct_cursor.y
                );
            }
        }

        println!("Rendering draw data...");
        let draw_data = imgui.context.render();

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
                        load: wgpu::LoadOp::Clear(imgui.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            imgui
                .renderer
                .render_draw_data(&draw_data, &mut render_pass)
                .expect("Rendering failed");
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        println!("Render completed successfully");
        Ok(())
    }
}

/// Setup the initial docking layout - Unity-style game engine layout
fn setup_initial_docking_layout(dockspace_id: u32) {
    use dear_imgui::{DockBuilder, SplitDirection};

    println!("Setting up initial docking layout...");

    // Clear any existing layout and create fresh dockspace
    DockBuilder::remove_node(dockspace_id);
    DockBuilder::add_node(dockspace_id, dear_imgui::DockNodeFlags::NONE);
    DockBuilder::set_node_size(dockspace_id, [1600.0, 881.0]);

    // Unity-style Professional Layout:
    // +-------------------+---------------------------+-------------------+
    // |                   |        Scene View         |                   |
    // |   Hierarchy       |---------------------------|    Inspector      |
    // |                   |        Game View          |                   |
    // +-------------------+---------------------------+-------------------+
    // |      Project      |         Console           |   Performance     |
    // +-------------------+---------------------------+-------------------+

    // Create main vertical split: Top area (75%) + Bottom area (25%)
    let mut bottom_area_id = 0u32;
    let top_area_id = DockBuilder::split_node(
        dockspace_id,
        SplitDirection::Down,
        0.25,
        Some(&mut bottom_area_id),
    );

    // Split top area horizontally: Left (24%) + Center (52%) + Right (24%)
    let mut left_panel_id = 0u32;
    let remaining_after_left = DockBuilder::split_node(
        top_area_id,
        SplitDirection::Left,
        0.24,
        Some(&mut left_panel_id),
    );

    let mut right_panel_id = 0u32;
    let center_area_id = DockBuilder::split_node(
        remaining_after_left,
        SplitDirection::Right,
        0.32, // 24% of remaining 76%
        Some(&mut right_panel_id),
    );

    // Split left panel vertically: Hierarchy (70%) + Project (30%)
    let mut project_id = 0u32;
    let hierarchy_id = DockBuilder::split_node(
        left_panel_id,
        SplitDirection::Down,
        0.3,
        Some(&mut project_id),
    );

    // Split right panel vertically: Inspector (80%) + Performance (20%)
    let mut performance_id = 0u32;
    let inspector_id = DockBuilder::split_node(
        right_panel_id,
        SplitDirection::Down,
        0.2,
        Some(&mut performance_id),
    );

    // Dock all windows to their designated areas
    DockBuilder::dock_window("Hierarchy", hierarchy_id);
    DockBuilder::dock_window("Project", project_id);
    DockBuilder::dock_window("Scene View", center_area_id);
    DockBuilder::dock_window("Game View", center_area_id); // Same area as Scene View (tabbed)
    DockBuilder::dock_window("Console", bottom_area_id);
    DockBuilder::dock_window("Inspector", inspector_id);
    DockBuilder::dock_window("Performance", performance_id);

    // Finalize the layout
    DockBuilder::finish(dockspace_id);

    println!("Docking layout setup complete");
}

/// Render the main menu bar
fn render_main_menu_bar(ui: &Ui, game_state: &mut GameEngineState) {
    if let Some(_main_menu_bar) = ui.begin_main_menu_bar() {
        ui.menu("File", || {
            if ui.menu_item("New Scene") {
                game_state
                    .console_logs
                    .push("[INFO] New scene created".to_string());
            }
            if ui.menu_item("Open Scene") {
                game_state
                    .console_logs
                    .push("[INFO] Scene opened".to_string());
            }
            if ui.menu_item("Save Scene") {
                game_state
                    .console_logs
                    .push("[INFO] Scene saved".to_string());
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
                game_state
                    .console_logs
                    .push("[INFO] Empty GameObject created".to_string());
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

/// Render the Hierarchy panel (Unity-style)
fn render_hierarchy(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Hierarchy")
        .size([300.0, 400.0], Condition::FirstUseEver)
        .build(|| {
            // Scene name header
            ui.text_colored(
                [0.8, 0.8, 0.8, 1.0],
                format!("Scene: {}", game_state.scene_name),
            );
            ui.separator();

            // Search filter with icon
            ui.text("🔍");
            ui.same_line();
            ui.input_text("##search", &mut String::new()).build();

            ui.separator();

            // Entity list with hierarchy
            let mut selected_entity = None;
            let entity_to_duplicate: Option<String> = None;
            let entity_to_delete: Option<String> = None;

            for (_i, entity) in game_state.entities.iter().enumerate() {
                let is_selected = game_state.selected_entity.as_ref() == Some(entity);

                // Add hierarchy indentation and icons
                let icon = if entity.contains("Camera") {
                    "📷"
                } else if entity.contains("Light") {
                    "💡"
                } else if entity.contains("Mesh") {
                    "🔷"
                } else {
                    "🎯"
                };

                ui.text(icon);
                ui.same_line();

                if ui.selectable_config(entity).selected(is_selected).build() {
                    selected_entity = Some(entity.clone());
                }

                // Right-click context menu - temporarily disabled for debugging
                // if let Some(_popup) = ui.begin_popup_context_item() {
                //     if ui.menu_item("Create Empty Child") {
                //         entity_to_duplicate = Some(format!("{} - Child", entity));
                //     }
                //     ui.separator();
                //     if ui.menu_item("Duplicate") {
                //         entity_to_duplicate = Some(entity.clone());
                //     }
                //     if ui.menu_item("Delete") {
                //         entity_to_delete = Some(entity.clone());
                //     }
                //     ui.separator();
                //     if ui.menu_item("Rename") {
                //         // TODO: Implement rename functionality
                //     }
                //     // popup.end() is called automatically by Drop
                // }
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
                game_state
                    .console_logs
                    .push(format!("[INFO] {} deleted", entity));
                game_state.entities.retain(|e| e != &entity);
                if game_state.selected_entity.as_ref() == Some(&entity) {
                    game_state.selected_entity = None;
                }
            }

            ui.separator();

            // Create buttons
            if ui.button("Create Empty") {
                let new_entity = format!("GameObject ({})", game_state.entities.len() + 1);
                game_state.entities.push(new_entity);
            }
            ui.same_line();
            if ui.button("Create Cube") {
                let new_entity = format!("Cube ({})", game_state.entities.len() + 1);
                game_state.entities.push(new_entity);
            }
        });
}

/// Render the Project panel (Unity-style)
fn render_project(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Project")
        .size([300.0, 200.0], Condition::FirstUseEver)
        .build(|| {
            // Project folder navigation
            ui.text_colored(
                [0.8, 0.8, 0.8, 1.0],
                format!("Project: {}", game_state.project_name),
            );
            ui.separator();

            // Folder path
            ui.text("📁");
            ui.same_line();
            ui.text(&game_state.current_folder);

            ui.separator();

            // Asset grid view
            let mut columns = 4;
            if ui.button("List View") {
                columns = 1;
            }
            ui.same_line();
            if ui.button("Grid View") {
                columns = 4;
            }

            ui.separator();

            // Assets display
            for (i, asset) in game_state.assets.iter().enumerate() {
                if i % columns != 0 {
                    ui.same_line();
                }

                let icon = if asset.ends_with(".cs") {
                    "📄"
                } else if asset.ends_with(".png") || asset.ends_with(".jpg") {
                    "🖼️"
                } else if asset.ends_with(".fbx") || asset.ends_with(".obj") {
                    "🎲"
                } else if asset.ends_with(".wav") || asset.ends_with(".mp3") {
                    "🔊"
                } else {
                    "📄"
                };

                ui.button(format!("{}\n{}", icon, asset));

                // Right-click context menu - temporarily disabled for testing
                // if let Some(popup) = ui.begin_popup_context_item() {
                //     if ui.menu_item("Import") {
                //         game_state.console_logs.push(format!("[INFO] Importing {}", asset));
                //     }
                //     if ui.menu_item("Delete") {
                //         game_state.console_logs.push(format!("[INFO] Deleted {}", asset));
                //     }
                //     ui.separator();
                //     if ui.menu_item("Show in Explorer") {
                //         game_state.console_logs.push(format!("[INFO] Opening {}", asset));
                //     }
                //     popup.end();
                // }
            }

            ui.separator();

            // Import button
            if ui.button("Import New Asset") {
                game_state
                    .assets
                    .push(format!("NewAsset_{}.png", game_state.assets.len()));
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
                        game_state
                            .console_logs
                            .push("[INFO] Material selector opened".to_string());
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
                // Temporarily disabled popup to test
                // if ui.button("Add Component") {
                //     ui.open_popup("add_component");
                // }

                // ui.popup("add_component", || {
                //     if ui.menu_item("Rigidbody") {
                //         game_state.console_logs.push("[INFO] Rigidbody component added".to_string());
                //     }
                //     if ui.menu_item("Audio Source") {
                //         game_state.console_logs.push("[INFO] Audio Source component added".to_string());
                //     }
                //     if ui.menu_item("Script") {
                //         game_state.console_logs.push("[INFO] Script component added".to_string());
                //     }
                // });
            } else {
                ui.text("No object selected");
                ui.text_colored(
                    [0.7, 0.7, 0.7, 1.0],
                    "Select an object in the Scene Hierarchy to view its properties",
                );
            }
        });
}

/// Render the Scene View (Unity-style editor view)
fn render_scene_view(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Scene View")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Scene view toolbar
            ui.text("🔧");
            ui.same_line();
            if ui.button("Move") {
                game_state
                    .console_logs
                    .push("[INFO] Move tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Rotate") {
                game_state
                    .console_logs
                    .push("[INFO] Rotate tool selected".to_string());
            }
            ui.same_line();
            if ui.button("Scale") {
                game_state
                    .console_logs
                    .push("[INFO] Scale tool selected".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.checkbox("Wireframe", &mut game_state.show_wireframe);
            ui.same_line();
            ui.checkbox("Grid", &mut game_state.show_grid);

            ui.separator();

            // Scene view area
            let content_region = ui.content_region_avail();
            game_state.viewport_size = [content_region[0], content_region[1]];

            // Only draw if we have a valid canvas size
            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                let draw_list = ui.get_window_draw_list();
                let canvas_pos = ui.cursor_screen_pos();
                let canvas_size = content_region;

                // Draw scene background
                draw_list
                    .add_rect(
                        canvas_pos,
                        [
                            canvas_pos[0] + canvas_size[0],
                            canvas_pos[1] + canvas_size[1],
                        ],
                        [0.15, 0.15, 0.15, 1.0],
                    )
                    .filled(true)
                    .build();

                // Draw grid if enabled
                if game_state.show_grid {
                    let grid_step = 50.0;
                    let grid_color = [0.3, 0.3, 0.3, 0.8];

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

                // Draw scene objects
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

                // Scene info
                ui.text(format!(
                    "Scene Size: {:.0}x{:.0}",
                    canvas_size[0], canvas_size[1]
                ));
            } else {
                ui.text("Scene view too small to render");
            }
        });
}

/// Render the Game View (Unity-style play view)
fn render_game_view(ui: &Ui, game_state: &mut GameEngineState) {
    ui.window("Game View")
        .size([800.0, 600.0], Condition::FirstUseEver)
        .build(|| {
            // Game view toolbar
            if ui.button("🎮 Play") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode started".to_string());
            }
            ui.same_line();
            if ui.button("⏸ Pause") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode paused".to_string());
            }
            ui.same_line();
            if ui.button("⏹ Stop") {
                game_state
                    .console_logs
                    .push("[INFO] Play mode stopped".to_string());
            }

            ui.same_line();
            ui.separator_vertical();
            ui.same_line();

            ui.text("Aspect:");
            ui.same_line();
            if ui.button("16:9") {
                game_state
                    .console_logs
                    .push("[INFO] Aspect ratio set to 16:9".to_string());
            }
            ui.same_line();
            if ui.button("4:3") {
                game_state
                    .console_logs
                    .push("[INFO] Aspect ratio set to 4:3".to_string());
            }

            ui.separator();

            // Game view area
            let content_region = ui.content_region_avail();

            if content_region[0] > 0.0 && content_region[1] > 0.0 {
                let draw_list = ui.get_window_draw_list();
                let canvas_pos = ui.cursor_screen_pos();
                let canvas_size = content_region;

                // Draw game background
                draw_list
                    .add_rect(
                        canvas_pos,
                        [
                            canvas_pos[0] + canvas_size[0],
                            canvas_pos[1] + canvas_size[1],
                        ],
                        [0.1, 0.2, 0.4, 1.0],
                    )
                    .filled(true)
                    .build();

                // Draw game objects (different from scene view)
                draw_list
                    .add_rect(
                        [canvas_pos[0] + 120.0, canvas_pos[1] + 120.0],
                        [canvas_pos[0] + 170.0, canvas_pos[1] + 170.0],
                        [0.8, 0.2, 0.8, 1.0],
                    )
                    .filled(true)
                    .build();

                // Game info
                ui.text(format!(
                    "Game Size: {:.0}x{:.0}",
                    canvas_size[0], canvas_size[1]
                ));
                ui.text("FPS: 60 | Frame: 1234");
            } else {
                ui.text("Game view too small to render");
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
                game_state
                    .console_logs
                    .push("[INFO] Console cleared".to_string());
            }
            ui.same_line();

            let mut show_info = true;
            let mut show_warning = true;
            let mut show_error = true;

            ui.checkbox("Info", &mut show_info);
            ui.same_line();
            ui.checkbox("Warning", &mut show_warning);
            ui.same_line();
            ui.checkbox("Error", &mut show_error);

            ui.separator();

            // Console output area
            let text_height = 16.0; // Hardcoded text height since text_line_height() is not available
            let footer_height = text_height + 10.0; // Approximate spacing

            ui.child_window("ConsoleOutput")
                .size([0.0, -footer_height])
                .build(ui, || {
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

            ui.separator();

            // Command input
            ui.text(">");
            ui.same_line();

            let mut input_changed = false;
            let _token = ui.push_item_width(-1.0);
            if ui
                .input_text("##console_input", &mut game_state.console_input)
                .enter_returns_true(true)
                .build()
            {
                input_changed = true;
            }

            if input_changed && !game_state.console_input.trim().is_empty() {
                let command = game_state.console_input.trim().to_string();
                game_state.console_logs.push(format!("> {}", command));

                // Process simple commands
                match command.as_str() {
                    "clear" => {
                        game_state.console_logs.clear();
                        game_state
                            .console_logs
                            .push("[INFO] Console cleared".to_string());
                    }
                    "help" => {
                        game_state.console_logs.push(
                            "[INFO] Available commands: clear, help, fps, version".to_string(),
                        );
                    }
                    "fps" => {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Current FPS: {:.1}", game_state.fps));
                    }
                    "version" => {
                        game_state
                            .console_logs
                            .push("[INFO] Game Engine v1.0.0".to_string());
                    }
                    _ => {
                        game_state
                            .console_logs
                            .push(format!("[ERROR] Unknown command: {}", command));
                    }
                }

                game_state.console_input.clear();
            }
        });
}

/// Render the asset browser panel (already handled by Project panel)
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
                game_state
                    .console_logs
                    .push("[INFO] Asset browser refreshed".to_string());
            }

            ui.separator();

            // Asset grid
            let button_size = [80.0, 80.0];
            let mut items_per_row = (ui.content_region_avail()[0] / (button_size[0] + 8.0)) as i32;
            if items_per_row < 1 {
                items_per_row = 1;
            }

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
                        game_state
                            .console_logs
                            .push(format!("[INFO] Opened folder: {}", asset));
                    } else {
                        game_state
                            .console_logs
                            .push(format!("[INFO] Selected asset: {}", asset));
                    }
                }

                // Right-click context menu - temporarily disabled for testing
                // if let Some(_popup) = ui.begin_popup_context_item() {
                //     if ui.menu_item("Import") {
                //         game_state.console_logs.push(format!("[INFO] Importing {}", asset));
                //     }
                //     if ui.menu_item("Delete") {
                //         game_state.console_logs.push(format!("[WARNING] Deleted {}", asset));
                //     }
                //     ui.separator();
                //     ui.menu_item("Properties");
                // }
            }
        });
}

/// Render the performance stats panel
fn render_performance(ui: &Ui, game_state: &mut GameEngineState) {
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
            let _fps_history: Vec<f32> = (0..60)
                .map(|i| 60.0 + ((ui.time() - i as f64 * 0.1) * 2.0).sin() as f32 * 5.0)
                .collect();
            // TODO: Implement actual graph rendering with plot_lines when available

            // Note: plot_lines might not be available in current API
            ui.text("(Graph visualization would go here)");
        });
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window = AppWindow::setup_gpu(event_loop);
            window.setup_imgui();
            // Request initial redraw to start the render loop
            window.window.request_redraw();
            self.window = Some(window);
            println!("Window created successfully");
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Handle exit events first to avoid borrowing issues
        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested");
                // Clean up resources before exiting
                if let Some(mut window) = self.window.take() {
                    // Explicitly drop ImGui context first
                    window.imgui = None;
                    // Then drop the window
                    drop(window);
                }
                event_loop.exit();
                return;
            }
            WindowEvent::KeyboardInput { ref event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    println!("Escape pressed, exiting");
                    // Clean up resources before exiting
                    if let Some(mut window) = self.window.take() {
                        // Explicitly drop ImGui context first
                        window.imgui = None;
                        // Then drop the window
                        drop(window);
                    }
                    event_loop.exit();
                    return;
                }
            }
            _ => {}
        }

        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        // Handle platform events first
        {
            let imgui = window.imgui.as_mut().unwrap();
            // Create a fake Event::WindowEvent for the platform handler
            let fake_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
                window_id: _window_id,
                event: event.clone(),
            };
            imgui
                .platform
                .handle_event(&mut imgui.context, &window.window, &fake_event);
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                if physical_size.width > 0 && physical_size.height > 0 {
                    window.surface_desc.width = physical_size.width;
                    window.surface_desc.height = physical_size.height;
                    window
                        .surface
                        .configure(&window.device, &window.surface_desc);
                    window.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let render_result = window.render();
                match render_result {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        eprintln!("Surface lost/outdated, reconfiguring");
                        window
                            .surface
                            .configure(&window.device, &window.surface_desc);
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
                // Request next frame
                window.window.request_redraw();
            }
            _ => {}
        }

        // Check input capture after handling events
        {
            let imgui = window.imgui.as_mut().unwrap();
            if !imgui.context.io().want_capture_mouse()
                && !imgui.context.io().want_capture_keyboard()
            {
                // Handle game input here when not captured by ImGui
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    println!("🎮 Game Engine Docking Demo - MSVC ABI Debug Version");
    println!("Testing MSVC ABI compatibility issues...");
    println!();

    // Test 1: Basic event loop creation
    println!("Test 1: Creating event loop...");
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    println!("✓ Event loop created successfully");

    let mut app = App::default();
    println!("✓ App created successfully");

    println!();
    println!("Starting event loop...");
    println!("Press ESC to exit");
    println!();

    event_loop.run_app(&mut app).unwrap();
}
