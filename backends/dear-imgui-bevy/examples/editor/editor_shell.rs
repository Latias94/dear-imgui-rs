//! Persistent editor-oriented Dear ImGui shell with a seeded split dock layout and a Bevy scene
//! render target shown as an ImGui image.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example editor_shell`
//!
//! Native multi-viewport run:
//! `cargo run -p dear-imgui-bevy --features render,multi-viewport --example editor_shell`

use bevy::{
    app::{AppExit, PreUpdate},
    camera::{ClearColorConfig, RenderTarget},
    input::{
        ButtonState,
        mouse::{MouseButton, MouseButtonInput},
    },
    prelude::*,
    window::{
        CursorMoved, PresentMode, PrimaryWindow, WindowCloseRequested, WindowPlugin,
        WindowPosition, WindowTheme,
    },
};
use bevy_ecs::{hierarchy::Children, name::Name};
use dear_imgui_bevy::{
    ImguiBackendConfig, ImguiBackendStatus, ImguiBevyTextures, ImguiContext, ImguiContexts,
    ImguiFrameOutput, ImguiPlugin, ImguiPrimaryContextPass, ImguiViewportCamera,
    ImguiViewportWindow, configure_example_context, input::ImguiInputSystems,
    render::ImguiOverlayDisabled,
};
use dear_imgui_rs::{
    Condition, DockBuilder, DockNodeFlags, SplitDirection, TextureId, ViewportFlags, WindowClass,
    WindowFlags,
};
use std::env;

const SCENE_WIDTH: u32 = 960;
const SCENE_HEIGHT: u32 = 540;
const VIEWPORT_PROBE_ENV: &str = "DEAR_IMGUI_BEVY_VIEWPORT_PROBE";
const VIEWPORT_PROBE_FRAMES_ENV: &str = "DEAR_IMGUI_BEVY_VIEWPORT_PROBE_FRAMES";
const VIEWPORT_PROBE_MOVE_PRIMARY_ENV: &str = "DEAR_IMGUI_BEVY_VIEWPORT_PROBE_MOVE_PRIMARY";
const VIEWPORT_PROBE_CLOSE_PRIMARY_ENV: &str = "DEAR_IMGUI_BEVY_VIEWPORT_PROBE_CLOSE_PRIMARY";
const VIEWPORT_PROBE_DOCK_BACK_ENV: &str = "DEAR_IMGUI_BEVY_VIEWPORT_PROBE_DOCK_BACK";
const VIEWPORT_PROBE_DEFAULT_FRAMES: u64 = 180;
const VIEWPORT_PROBE_EXIT_GRACE_FRAMES: u64 = 24;
const VIEWPORT_PROBE_DOCK_BACK_FRAME: u64 = 75;
const VIEWPORT_PROBE_DOCK_BACK_VERIFY_FRAME: u64 = 120;

type SceneEntityQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Name>,
        Option<&'static Transform>,
        Option<&'static Children>,
        Option<&'static EditorSceneObject>,
    ),
>;

#[derive(Component)]
struct EditorSceneObject {
    base_position: Vec3,
    orbit_radius: f32,
    orbit_speed: f32,
}

#[derive(Resource, Clone)]
struct SceneViewport {
    image: Handle<Image>,
    texture_id: TextureId,
    size: [u32; 2],
}

#[derive(Resource, Clone, Copy, Debug)]
struct EditorSceneRoot {
    entity: Entity,
}

#[derive(Resource, Debug)]
struct EditorState {
    show_inspector: bool,
    show_hierarchy: bool,
    show_input_policy: bool,
    show_diagnostics: bool,
    dock_layout_seeded: bool,
    route_shortcuts_to_imgui: bool,
    route_scene_camera_when_hovered: bool,
    scene_hovered: bool,
    viewport_zoom: f32,
    playback_running: bool,
    selected_entity: Option<Entity>,
    inspector_synced_entity: Option<Entity>,
    inspector_name_buffer: String,
    inspector_translation: [f32; 3],
    inspector_rotation_deg: [f32; 3],
    inspector_scale: [f32; 3],
    last_frame_index: u64,
    editor_events: u32,
}

#[derive(Resource, Debug)]
struct ViewportProbe {
    max_frames: u64,
    move_primary: bool,
    close_primary: bool,
    dock_back: bool,
    moved_primary: bool,
    primary_close_sent: bool,
    secondary_seen: bool,
    dock_back_requested: bool,
    dock_back_clean: bool,
    hierarchy_click_target: Option<HierarchyClickTarget>,
    hierarchy_click_pressed: bool,
    hierarchy_click_released: bool,
    hierarchy_click_press_frame: Option<u64>,
    hierarchy_click_observed: bool,
    hierarchy_click_verified: bool,
    failure_reported: bool,
    exit_sent: bool,
}

#[derive(Clone, Copy, Debug)]
struct HierarchyClickTarget {
    entity: Entity,
    global_pos: [f32; 2],
}

impl ViewportProbe {
    fn from_env() -> Option<Self> {
        if !env_flag(VIEWPORT_PROBE_ENV) {
            return None;
        }

        Some(Self {
            max_frames: env::var(VIEWPORT_PROBE_FRAMES_ENV)
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|frames| *frames > 0)
                .unwrap_or(VIEWPORT_PROBE_DEFAULT_FRAMES),
            move_primary: env_flag(VIEWPORT_PROBE_MOVE_PRIMARY_ENV),
            close_primary: env_flag(VIEWPORT_PROBE_CLOSE_PRIMARY_ENV),
            dock_back: env_flag(VIEWPORT_PROBE_DOCK_BACK_ENV),
            moved_primary: false,
            primary_close_sent: false,
            secondary_seen: false,
            dock_back_requested: false,
            dock_back_clean: false,
            hierarchy_click_target: None,
            hierarchy_click_pressed: false,
            hierarchy_click_released: false,
            hierarchy_click_press_frame: None,
            hierarchy_click_observed: false,
            hierarchy_click_verified: false,
            failure_reported: false,
            exit_sent: false,
        })
    }

    fn force_detached_hierarchy(&self, frame: u64) -> bool {
        !self.dock_back || frame < VIEWPORT_PROBE_DOCK_BACK_FRAME
    }

    fn should_force_dock_back(&self, frame: u64) -> bool {
        self.dock_back && frame >= VIEWPORT_PROBE_DOCK_BACK_FRAME
    }

    fn capture_hierarchy_click_target(
        &mut self,
        entity: Entity,
        selected_entity: Option<Entity>,
        rect: ([f32; 2], [f32; 2]),
    ) {
        if !self.dock_back
            || !self.dock_back_clean
            || self.hierarchy_click_target.is_some()
            || self.hierarchy_click_pressed
            || selected_entity == Some(entity)
        {
            return;
        }
        let center = [(rect.0[0] + rect.1[0]) * 0.5, (rect.0[1] + rect.1[1]) * 0.5];
        if center[0].is_finite() && center[1].is_finite() {
            self.hierarchy_click_target = Some(HierarchyClickTarget {
                entity,
                global_pos: center,
            });
            println!(
                "[viewport-probe] hierarchy-click-target entity={entity:?} global_pos={center:?}"
            );
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            show_inspector: true,
            show_hierarchy: true,
            show_input_policy: true,
            show_diagnostics: true,
            dock_layout_seeded: false,
            route_shortcuts_to_imgui: true,
            route_scene_camera_when_hovered: true,
            scene_hovered: false,
            viewport_zoom: 1.0,
            playback_running: true,
            selected_entity: None,
            inspector_synced_entity: None,
            inspector_name_buffer: String::new(),
            inspector_translation: [0.0, 0.0, 0.0],
            inspector_rotation_deg: [0.0, 0.0, 0.0],
            inspector_scale: [1.0, 1.0, 1.0],
            last_frame_index: 0,
            editor_events: 0,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy editor shell".to_owned(),
            resolution: (1440, 900).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        multi_viewport: cfg!(feature = "multi-viewport"),
        ..Default::default()
    }))
    .init_resource::<ImguiBevyTextures>()
    .init_resource::<EditorState>()
    .add_systems(Startup, setup)
    .add_systems(Update, (close_on_escape, animate_scene))
    .add_systems(ImguiPrimaryContextPass, editor_ui);

    if let Some(probe) = ViewportProbe::from_env() {
        println!(
            "[viewport-probe] enabled max_frames={} move_primary={} close_primary={} dock_back={}",
            probe.max_frames, probe.move_primary, probe.close_primary, probe.dock_back
        );
        app.insert_resource(probe)
            .add_systems(
                PreUpdate,
                viewport_probe_drive_windows.before(ImguiInputSystems),
            )
            .add_systems(ImguiPrimaryContextPass, viewport_probe_report);
    }

    app.run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut textures: ResMut<ImguiBevyTextures>,
    mut imgui: NonSendMut<ImguiContext>,
    mut state: ResMut<EditorState>,
) {
    // Render the Dear ImGui overlay into the primary window, while the offscreen scene keeps its
    // own image target for the editor viewport.
    commands.spawn(Camera2d);

    let mut image = Image::new_target_texture(
        SCENE_WIDTH,
        SCENE_HEIGHT,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.label = Some("dear_imgui_bevy_editor_scene");
    image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
        | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let texture_id = textures.register(&image);

    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.045, 0.052, 0.062)),
            ..Default::default()
        },
        RenderTarget::Image(image.clone().into()),
        ImguiOverlayDisabled,
    ));
    commands.insert_resource(SceneViewport {
        image,
        texture_id,
        size: [SCENE_WIDTH, SCENE_HEIGHT],
    });

    let camera_preview_transform = Transform::from_xyz(-120.0, 120.0, 0.2);
    let blue_panel_transform = Transform::from_xyz(-220.0, 40.0, 0.0);
    let amber_tool_transform = Transform::from_xyz(170.0, -20.0, 0.5);
    let green_probe_transform = Transform::from_xyz(60.0, 120.0, 1.0);

    let mut camera_preview = Entity::PLACEHOLDER;
    let scene_root = commands
        .spawn((
            Name::new("Scene Root"),
            Transform::default(),
            GlobalTransform::default(),
        ))
        .with_children(|parent| {
            camera_preview = parent
                .spawn((
                    Name::new("Camera Preview"),
                    Sprite::from_color(Color::srgb(0.77, 0.80, 0.95), Vec2::new(180.0, 120.0)),
                    camera_preview_transform,
                ))
                .id();
            parent.spawn((
                Name::new("Blue Panel"),
                Sprite::from_color(Color::srgb(0.18, 0.48, 0.82), Vec2::new(220.0, 140.0)),
                blue_panel_transform,
                EditorSceneObject {
                    base_position: blue_panel_transform.translation,
                    orbit_radius: 18.0,
                    orbit_speed: 1.4,
                },
            ));
            parent.spawn((
                Name::new("Amber Tool"),
                Sprite::from_color(Color::srgb(0.90, 0.62, 0.22), Vec2::new(150.0, 220.0)),
                amber_tool_transform,
                EditorSceneObject {
                    base_position: amber_tool_transform.translation,
                    orbit_radius: 28.0,
                    orbit_speed: -0.9,
                },
            ));
            parent.spawn((
                Name::new("Green Probe"),
                Mesh2d(meshes.add(Circle::new(76.0))),
                MeshMaterial2d(materials.add(Color::srgb(0.24, 0.78, 0.54))),
                green_probe_transform,
                EditorSceneObject {
                    base_position: green_probe_transform.translation,
                    orbit_radius: 22.0,
                    orbit_speed: 1.0,
                },
            ));
        })
        .id();
    commands.insert_resource(EditorSceneRoot { entity: scene_root });

    sync_inspector_buffers_from_values(
        &mut state,
        camera_preview,
        "Camera Preview",
        camera_preview_transform,
    );

    configure_example_context(&mut imgui, true);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn animate_scene(
    time: Res<Time>,
    state: Res<EditorState>,
    mut objects: Query<(&mut Transform, &EditorSceneObject)>,
) {
    if !state.playback_running {
        return;
    }

    let elapsed = time.elapsed_secs();
    for (mut transform, object) in &mut objects {
        let phase = elapsed * object.orbit_speed;
        transform.translation.x = object.base_position.x + phase.cos() * object.orbit_radius;
        transform.translation.y = object.base_position.y + phase.sin() * object.orbit_radius;
        transform.rotation = Quat::from_rotation_z(phase * 0.35);
    }
}

#[allow(clippy::too_many_arguments)]
fn viewport_probe_drive_windows(
    mut probe: ResMut<ViewportProbe>,
    state: Res<EditorState>,
    mut primary_windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    viewport_windows: Query<Entity, With<ImguiViewportWindow>>,
    viewport_cameras: Query<Entity, With<ImguiViewportCamera>>,
    mut cursor_moved: MessageWriter<CursorMoved>,
    mut mouse_buttons: MessageWriter<MouseButtonInput>,
    mut close_requests: MessageWriter<WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    let frame = state.last_frame_index;
    if !probe.moved_primary
        && probe.move_primary
        && frame >= 30
        && let Ok((_entity, mut window)) = primary_windows.single_mut()
    {
        window.position = WindowPosition::At(IVec2::new(24, 96));
        probe.moved_primary = true;
        println!("[viewport-probe] moved-primary frame={frame} pos=[24,96]");
    }

    if !probe.secondary_seen && viewport_windows.iter().next().is_some() {
        probe.secondary_seen = true;
        println!("[viewport-probe] secondary-window-seen frame={frame}");
    }

    if probe.dock_back && frame >= VIEWPORT_PROBE_DOCK_BACK_VERIFY_FRAME {
        let secondary_count = viewport_windows.iter().count();
        let camera_count = viewport_cameras.iter().count();
        if secondary_count == 0 && camera_count == 0 {
            if !probe.dock_back_clean {
                probe.dock_back_clean = true;
                println!("[viewport-probe] dock-back-clean frame={frame}");
            }
        } else if !probe.failure_reported {
            probe.failure_reported = true;
            println!(
                "[viewport-probe] ERROR dock-back-stale-state frame={frame} secondary_count={secondary_count} camera_count={camera_count}"
            );
            exit.write(AppExit::error());
            return;
        }
    }

    if probe.dock_back
        && probe.dock_back_clean
        && !probe.hierarchy_click_pressed
        && let Some(target) = probe.hierarchy_click_target
    {
        let Ok((primary_entity, primary_window)) = primary_windows.single_mut() else {
            return;
        };
        let primary_pos = match primary_window.position {
            WindowPosition::At(pos) => pos,
            WindowPosition::Automatic | WindowPosition::Centered(_) => IVec2::ZERO,
        };
        let scale_factor =
            if primary_window.scale_factor().is_finite() && primary_window.scale_factor() > 0.0 {
                primary_window.scale_factor()
            } else {
                1.0
            };
        let local_pos = Vec2::new(
            target.global_pos[0] - primary_pos.x as f32 / scale_factor,
            target.global_pos[1] - primary_pos.y as f32 / scale_factor,
        );
        cursor_moved.write(CursorMoved {
            window: primary_entity,
            position: local_pos,
            delta: None,
        });
        mouse_buttons.write(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: primary_entity,
        });
        probe.hierarchy_click_pressed = true;
        probe.hierarchy_click_press_frame = Some(frame);
        println!(
            "[viewport-probe] hierarchy-click-pressed frame={frame} entity={:?} global_pos={:?} local_pos={local_pos:?}",
            target.entity, target.global_pos
        );
        return;
    }

    if probe.dock_back
        && probe.hierarchy_click_pressed
        && !probe.hierarchy_click_released
        && probe
            .hierarchy_click_press_frame
            .is_some_and(|press_frame| frame > press_frame)
        && let Some(target) = probe.hierarchy_click_target
    {
        let Ok((primary_entity, _primary_window)) = primary_windows.single_mut() else {
            return;
        };
        mouse_buttons.write(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window: primary_entity,
        });
        probe.hierarchy_click_released = true;
        println!(
            "[viewport-probe] hierarchy-click-released frame={frame} entity={:?}",
            target.entity
        );
    }

    if frame < probe.max_frames {
        return;
    }

    if probe.dock_back && !probe.dock_back_clean && !probe.failure_reported {
        probe.failure_reported = true;
        println!("[viewport-probe] ERROR dock-back-never-clean frame={frame}");
        exit.write(AppExit::error());
        return;
    }
    if probe.dock_back && !probe.hierarchy_click_verified && !probe.failure_reported {
        probe.failure_reported = true;
        println!("[viewport-probe] ERROR hierarchy-click-not-verified frame={frame}");
        exit.write(AppExit::error());
        return;
    }

    if probe.close_primary && !probe.primary_close_sent {
        if let Ok((entity, _window)) = primary_windows.single_mut() {
            close_requests.write(WindowCloseRequested { window: entity });
            probe.primary_close_sent = true;
            println!("[viewport-probe] requested-primary-close frame={frame}");
        }
        return;
    }

    let should_exit = !probe.close_primary
        || probe.primary_close_sent && frame >= probe.max_frames + VIEWPORT_PROBE_EXIT_GRACE_FRAMES;
    if should_exit && !probe.exit_sent {
        exit.write(AppExit::Success);
        probe.exit_sent = true;
        println!("[viewport-probe] requested-exit frame={frame}");
    }
}

fn viewport_probe_report(
    state: Res<EditorState>,
    output: Res<ImguiFrameOutput>,
    primary_windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
    viewport_cameras: Query<(Entity, &ImguiViewportCamera)>,
) {
    let frame = state.last_frame_index;
    if frame == 0 || !frame.is_multiple_of(15) {
        return;
    }

    let primary = primary_windows
        .single()
        .ok()
        .map(|(entity, window)| {
            format!(
                "primary={entity:?}:{}:{:?}:{}x{}@{}",
                window.title,
                window.position,
                window.width(),
                window.height(),
                window.scale_factor()
            )
        })
        .unwrap_or_else(|| "primary=<missing>".to_owned());
    let mut viewport_window_positions = Vec::new();
    let secondary = viewport_windows
        .iter()
        .map(|(entity, window, viewport)| {
            let logical_pos = probe_window_logical_pos(window);
            if let Some(pos) = logical_pos {
                viewport_window_positions.push((viewport.viewport_id.raw(), pos));
            }
            format!(
                "{entity:?}:vp={}:{}:{:?}:logical_pos={:?}:{}x{}@{}:visible={}",
                viewport.viewport_id.raw(),
                window.title,
                window.position,
                logical_pos,
                window.width(),
                window.height(),
                window.scale_factor(),
                window.visible
            )
        })
        .collect::<Vec<_>>();
    let cameras = viewport_cameras
        .iter()
        .map(|(entity, camera)| format!("{entity:?}:vp={}", camera.viewport_id.raw()))
        .collect::<Vec<_>>();
    let snapshot = output
        .snapshot()
        .map(|snapshot| {
            let viewports = snapshot
                .viewports
                .iter()
                .map(|viewport| {
                    let window_delta = viewport_window_positions
                        .iter()
                        .find(|(id, _)| *id == viewport.viewport_id.raw())
                        .map(|(_, window_pos)| {
                            let delta = [
                                viewport.draw.display_pos[0] - window_pos[0],
                                viewport.draw.display_pos[1] - window_pos[1],
                            ];
                            format!(":window_pos={window_pos:?}:delta={delta:?}")
                        })
                        .unwrap_or_default();
                    format!(
                        "{}:{:?}:{:?}:lists={}{}",
                        viewport.viewport_id.raw(),
                        viewport.draw.display_pos,
                        viewport.draw.display_size,
                        viewport.draw.draw_lists.len(),
                        window_delta
                    )
                })
                .collect::<Vec<_>>();
            format!(
                "main={:?}:{:?}:lists={} viewports=[{}]",
                snapshot.draw.display_pos,
                snapshot.draw.display_size,
                snapshot.draw.draw_lists.len(),
                viewports.join("|")
            )
        })
        .unwrap_or_else(|| {
            output
                .snapshot_error()
                .map(|error| format!("error={error}"))
                .unwrap_or_else(|| "snapshot=<none>".to_owned())
        });

    println!(
        "[viewport-probe] frame={frame} {primary} secondary=[{}] cameras=[{}] {snapshot}",
        secondary.join("|"),
        cameras.join("|"),
    );
}

fn probe_window_logical_pos(window: &Window) -> Option<[f32; 2]> {
    let WindowPosition::At(pos) = window.position else {
        return None;
    };
    let scale_factor = if window.scale_factor().is_finite() && window.scale_factor() > 0.0 {
        window.scale_factor()
    } else {
        1.0
    };
    Some([pos.x as f32 / scale_factor, pos.y as f32 / scale_factor])
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            let value = value.trim();
            !(value.is_empty()
                || value == "0"
                || value.eq_ignore_ascii_case("false")
                || value.eq_ignore_ascii_case("no")
                || value.eq_ignore_ascii_case("off"))
        })
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
fn editor_ui(
    mut contexts: ImguiContexts,
    viewport: Res<SceneViewport>,
    scene_root: Res<EditorSceneRoot>,
    scene_entities: SceneEntityQuery,
    mut commands: Commands,
    mut state: ResMut<EditorState>,
    output: Res<ImguiFrameOutput>,
    backend_status: Res<ImguiBackendStatus>,
    mut viewport_probe: Option<ResMut<ViewportProbe>>,
) {
    let frame_index = contexts.frame_index().unwrap_or_default();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    state.last_frame_index = frame_index;

    let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
        ui.get_id("DearImguiBevyEditorDockspace"),
        DockNodeFlags::PASSTHRU_CENTRAL_NODE,
    );
    let viewport_probe_active = viewport_probe.is_some();
    seed_editor_dock_layout(ui, dockspace_id, &mut state, viewport_probe_active);

    render_menu_bar(ui, &mut state);

    ui.window("Scene")
        .size([820.0, 560.0], Condition::FirstUseEver)
        .build(|| {
            render_scene_view(ui, &viewport, &mut state);
        });

    if state.show_hierarchy {
        let force_detached_hierarchy = viewport_probe
            .as_ref()
            .map(|probe| probe.force_detached_hierarchy(frame_index))
            .unwrap_or(false);
        let force_dock_back_hierarchy = viewport_probe
            .as_ref()
            .map(|probe| probe.should_force_dock_back(frame_index))
            .unwrap_or(false);
        if force_detached_hierarchy {
            let class = WindowClass::new(ui.get_id("ViewportProbeHierarchyClass"))
                .no_parent_viewport()
                .viewport_flags_override_set(ViewportFlags::NO_AUTO_MERGE);
            ui.set_next_window_class(&class);
        } else if force_dock_back_hierarchy {
            ui.set_next_window_dock_id(dockspace_id);
        }

        let main_viewport = ui.main_viewport();
        let hierarchy_pos = [
            main_viewport.pos()[0] + main_viewport.size()[0] + 48.0,
            main_viewport.pos()[1] + 96.0,
        ];
        let mut hierarchy = ui.window("Hierarchy").size(
            [260.0, 420.0],
            if viewport_probe_active {
                Condition::Appearing
            } else {
                Condition::FirstUseEver
            },
        );
        if force_detached_hierarchy {
            hierarchy = hierarchy
                .position(hierarchy_pos, Condition::Appearing)
                .focused(true);
        }
        if force_dock_back_hierarchy
            && let Some(probe) = viewport_probe.as_deref_mut()
            && !probe.dock_back_requested
        {
            probe.dock_back_requested = true;
            println!("[viewport-probe] dock-back-requested frame={frame_index}");
        }
        hierarchy.build(|| {
            render_hierarchy(
                ui,
                &mut state,
                scene_root.entity,
                &scene_entities,
                viewport_probe.as_deref_mut(),
            );
        });
    }

    if let Some(probe) = viewport_probe.as_deref_mut()
        && probe.dock_back
        && probe.hierarchy_click_released
        && !probe.hierarchy_click_verified
        && let Some(target) = probe.hierarchy_click_target
        && state.selected_entity == Some(target.entity)
        && probe.hierarchy_click_observed
    {
        probe.hierarchy_click_verified = true;
        println!(
            "[viewport-probe] hierarchy-click-verified frame={frame_index} entity={:?}",
            target.entity
        );
    }

    if state.show_inspector {
        ui.window("Inspector")
            .size([340.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                render_inspector(
                    ui,
                    &viewport,
                    &mut state,
                    &output,
                    &scene_entities,
                    &mut commands,
                );
            });
    }

    if state.show_input_policy {
        ui.window("Input Policy")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .flags(WindowFlags::NO_COLLAPSE)
            .build(|| {
                render_input_policy(ui, &mut state);
            });
    }

    if state.show_diagnostics {
        ui.window("Diagnostics")
            .size([340.0, 220.0], Condition::FirstUseEver)
            .build(|| {
                render_diagnostics(ui, &state, &output, &backend_status);
            });
    }
}

fn render_menu_bar(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    if let Some(_bar) = ui.begin_main_menu_bar()
        && let Some(_menu) = ui.begin_menu("Window")
    {
        let _ = ui.menu_item_toggle_no_shortcut("Hierarchy", &mut state.show_hierarchy, true);
        let _ = ui.menu_item_toggle_no_shortcut("Inspector", &mut state.show_inspector, true);
        let _ = ui.menu_item_toggle_no_shortcut("Input Policy", &mut state.show_input_policy, true);
        let _ = ui.menu_item_toggle_no_shortcut("Diagnostics", &mut state.show_diagnostics, true);
    }
}

fn seed_editor_dock_layout(
    ui: &dear_imgui_rs::Ui,
    dockspace_id: dear_imgui_rs::Id,
    state: &mut EditorState,
    skip_hierarchy: bool,
) {
    if state.dock_layout_seeded {
        return;
    }

    let viewport = ui.main_viewport();
    let viewport_pos = viewport.pos();
    let viewport_size = viewport.size();

    DockBuilder::remove_node(dockspace_id);
    let root = DockBuilder::add_node(dockspace_id, DockNodeFlags::PASSTHRU_CENTRAL_NODE);
    DockBuilder::set_node_pos(root, viewport_pos);
    DockBuilder::set_node_size(root, viewport_size);

    let (hierarchy_id, center_stack) = DockBuilder::split_node(root, SplitDirection::Left, 0.20);
    let (inspector_id, scene_stack) =
        DockBuilder::split_node(center_stack, SplitDirection::Right, 0.24);
    let (bottom_id, scene_id) = DockBuilder::split_node(scene_stack, SplitDirection::Down, 0.28);

    if !skip_hierarchy {
        DockBuilder::dock_window("Hierarchy", hierarchy_id);
    }
    DockBuilder::dock_window("Scene", scene_id);
    DockBuilder::dock_window("Inspector", inspector_id);
    DockBuilder::dock_window("Input Policy", bottom_id);
    DockBuilder::dock_window("Diagnostics", bottom_id);
    DockBuilder::finish(root);

    state.dock_layout_seeded = true;
}

fn render_scene_view(ui: &dear_imgui_rs::Ui, viewport: &SceneViewport, state: &mut EditorState) {
    let available = ui.content_region_avail();
    let fit = fit_aspect(
        [available[0].max(96.0), (available[1] - 44.0).max(96.0)],
        viewport.size,
    );
    let image_size = [
        (fit[0] * state.viewport_zoom).max(64.0),
        (fit[1] * state.viewport_zoom).max(64.0),
    ];

    if ui.button(if state.playback_running {
        "Pause"
    } else {
        "Play"
    }) {
        state.playback_running = !state.playback_running;
        state.editor_events = state.editor_events.saturating_add(1);
    }
    ui.same_line();
    if ui.button("Frame") {
        state.editor_events = state.editor_events.saturating_add(1);
    }
    ui.same_line();
    ui.text(format!("zoom {:.2}x", state.viewport_zoom));
    ui.slider_f32("Viewport zoom", &mut state.viewport_zoom, 0.50, 2.00);
    ui.separator();
    ui.image_config(viewport.texture_id, image_size)
        .uv0([0.0, 1.0])
        .uv1([1.0, 0.0])
        .build();
    state.scene_hovered = ui.is_item_hovered();
}

fn render_hierarchy_branch(
    ui: &dear_imgui_rs::Ui,
    state: &mut EditorState,
    scene_entities: &SceneEntityQuery,
    entity: Entity,
    is_root: bool,
    mut viewport_probe: Option<&mut ViewportProbe>,
) {
    let Ok((name, _transform, children, _scene_object)) = scene_entities.get(entity) else {
        return;
    };

    let label = name
        .map(|name| name.as_str().to_owned())
        .unwrap_or_else(|| entity.to_string());
    let selected = state.selected_entity == Some(entity);
    let mut node = ui
        .tree_node_config(&label)
        .selected(selected)
        .framed(true)
        .span_avail_width(true)
        .leaf(children.is_none());
    if is_root {
        node = node.default_open(true);
    }

    let opened = node.push();
    if let Some(probe) = viewport_probe.as_deref_mut() {
        probe.capture_hierarchy_click_target(entity, state.selected_entity, ui.item_rect());
    }
    if ui.is_item_clicked() {
        if let Some(probe) = viewport_probe.as_deref_mut()
            && probe.dock_back
            && probe.hierarchy_click_pressed
            && probe
                .hierarchy_click_target
                .is_some_and(|target| target.entity == entity)
            && !probe.hierarchy_click_observed
        {
            probe.hierarchy_click_observed = true;
            println!("[viewport-probe] hierarchy-click-observed entity={entity:?}");
        }
        select_entity(state, entity);
    }

    if let Some(_node) = opened
        && let Some(children) = children
    {
        for child in children.iter() {
            render_hierarchy_branch(
                ui,
                state,
                scene_entities,
                child,
                false,
                viewport_probe.as_deref_mut(),
            );
        }
    }
}

fn render_hierarchy(
    ui: &dear_imgui_rs::Ui,
    state: &mut EditorState,
    root: Entity,
    scene_entities: &SceneEntityQuery,
    viewport_probe: Option<&mut ViewportProbe>,
) {
    ui.text("Scene Graph");
    ui.separator();
    render_hierarchy_branch(ui, state, scene_entities, root, true, viewport_probe);
}

fn select_entity(state: &mut EditorState, entity: Entity) {
    if state.selected_entity != Some(entity) {
        state.selected_entity = Some(entity);
        state.inspector_synced_entity = None;
        state.editor_events = state.editor_events.saturating_add(1);
    }
}

fn sync_inspector_buffers_from_values(
    state: &mut EditorState,
    entity: Entity,
    name: impl Into<String>,
    transform: Transform,
) {
    state.selected_entity = Some(entity);
    state.inspector_synced_entity = Some(entity);
    state.inspector_name_buffer = name.into();
    state.inspector_translation = transform.translation.to_array();
    let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
    state.inspector_rotation_deg = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
    state.inspector_scale = transform.scale.to_array();
}

fn sync_inspector_buffers_from_query(
    state: &mut EditorState,
    entity: Entity,
    name: Option<&Name>,
    transform: Option<&Transform>,
) {
    let display_name = name
        .map(|name| name.as_str().to_owned())
        .unwrap_or_else(|| entity.to_string());
    sync_inspector_buffers_from_values(
        state,
        entity,
        display_name,
        transform.copied().unwrap_or(Transform::IDENTITY),
    );
}

fn render_inspector(
    ui: &dear_imgui_rs::Ui,
    viewport: &SceneViewport,
    state: &mut EditorState,
    output: &ImguiFrameOutput,
    scene_entities: &SceneEntityQuery,
    commands: &mut Commands,
) {
    ui.text("Entity Inspector");
    ui.separator();
    ui.text(format!("Image handle: {:?}", viewport.image.id()));
    ui.text(format!("TextureId: {:?}", viewport.texture_id));
    ui.text(format!(
        "Target size: {} x {}",
        viewport.size[0], viewport.size[1]
    ));
    ui.text(format!("Backend frame: {}", output.frame_index()));
    ui.text(format!("UI frame: {}", state.last_frame_index));
    ui.text(format!("Scene hovered: {}", state.scene_hovered));
    ui.checkbox("Playback running", &mut state.playback_running);
    ui.checkbox(
        "Route scene camera when hovered",
        &mut state.route_scene_camera_when_hovered,
    );
    ui.separator();

    let Some(entity) = state.selected_entity else {
        ui.text_wrapped("Select an entity in the hierarchy to inspect or edit it.");
        return;
    };

    if state.inspector_synced_entity != Some(entity) {
        match scene_entities.get(entity) {
            Ok((name, transform, _, _)) => {
                sync_inspector_buffers_from_query(&mut *state, entity, name, transform);
            }
            Err(_) => {
                sync_inspector_buffers_from_values(
                    &mut *state,
                    entity,
                    entity.to_string(),
                    Transform::IDENTITY,
                );
            }
        }
    }

    ui.text(format!("Entity: {entity}"));
    ui.separator();

    if ui
        .input_text("Name", &mut state.inspector_name_buffer)
        .build()
    {
        let trimmed = state.inspector_name_buffer.trim();
        let name = if trimmed.is_empty() {
            entity.to_string()
        } else {
            trimmed.to_owned()
        };
        state.inspector_name_buffer = name.clone();
        commands.entity(entity).insert(Name::new(name));
        state.editor_events = state.editor_events.saturating_add(1);
    }

    let mut transform_changed = false;
    if ui
        .input_float3("Translation", &mut state.inspector_translation)
        .build()
    {
        transform_changed = true;
    }
    if ui
        .input_float3("Rotation (deg)", &mut state.inspector_rotation_deg)
        .build()
    {
        transform_changed = true;
    }
    if ui.input_float3("Scale", &mut state.inspector_scale).build() {
        transform_changed = true;
    }
    if transform_changed {
        let translation = Vec3::from_array(state.inspector_translation);
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            state.inspector_rotation_deg[0].to_radians(),
            state.inspector_rotation_deg[1].to_radians(),
            state.inspector_rotation_deg[2].to_radians(),
        );
        let scale = Vec3::from_array(state.inspector_scale);
        commands.entity(entity).insert(Transform {
            translation,
            rotation,
            scale,
        });
        state.editor_events = state.editor_events.saturating_add(1);
    }

    if let Ok((_, _, _, Some(scene_object))) = scene_entities.get(entity) {
        ui.separator();
        ui.text("Motion");
        ui.text(format!("Base position: {:?}", scene_object.base_position));
        ui.text(format!("Orbit radius: {:.2}", scene_object.orbit_radius));
        ui.text(format!("Orbit speed: {:.2}", scene_object.orbit_speed));
    }
}

fn render_input_policy(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    let io = ui.io();
    ui.text(format!("want_capture_mouse: {}", io.want_capture_mouse()));
    ui.text(format!(
        "want_capture_keyboard: {}",
        io.want_capture_keyboard()
    ));
    ui.text(format!("scene_hovered: {}", state.scene_hovered));
    ui.separator();
    ui.checkbox(
        "Route global shortcuts while ImGui wants keyboard",
        &mut state.route_shortcuts_to_imgui,
    );
    ui.checkbox(
        "Route scene camera only when viewport is hovered",
        &mut state.route_scene_camera_when_hovered,
    );
    ui.separator();
    ui.text_wrapped(
        "Bevy messages stay readable by game and editor systems; Dear ImGui capture flags are policy inputs.",
    );
}

fn render_diagnostics(
    ui: &dear_imgui_rs::Ui,
    state: &EditorState,
    output: &ImguiFrameOutput,
    backend_status: &ImguiBackendStatus,
) {
    ui.text(format!("Editor events: {}", state.editor_events));
    ui.text(format!("Frame output: {}", output.frame_index()));
    ui.text(format!(
        "Multi-viewport requested: {}",
        backend_status.multi_viewport_requested
    ));
    ui.text(format!(
        "Render integration installed: {}",
        backend_status.render_integration_installed
    ));
    ui.text(format!(
        "Multi-viewport supported: {}",
        backend_status.multi_viewport_supported
    ));
    ui.text(format!(
        "Viewport render routing: {}",
        backend_status.viewport_render_routing_enabled
    ));
    if let Some(snapshot) = output.snapshot() {
        ui.text(format!("Draw lists: {}", snapshot.draw.draw_lists.len()));
        ui.text(format!(
            "Texture requests: {}",
            snapshot.texture_requests.len()
        ));
        ui.text(format!(
            "Display size: {:.0} x {:.0}",
            snapshot.draw.display_size[0], snapshot.draw.display_size[1]
        ));
    } else if let Some(error) = output.snapshot_error() {
        ui.text(format!("Snapshot error: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_entity_marks_inspector_for_resync() {
        let first = Entity::from_raw_u32(1).unwrap();
        let second = Entity::from_raw_u32(2).unwrap();
        let mut state = EditorState::default();

        sync_inspector_buffers_from_values(
            &mut state,
            first,
            "First",
            Transform::from_xyz(1.0, 2.0, 3.0),
        );
        assert_eq!(state.selected_entity, Some(first));
        assert_eq!(state.inspector_synced_entity, Some(first));
        assert_eq!(state.editor_events, 0);

        select_entity(&mut state, second);
        assert_eq!(state.selected_entity, Some(second));
        assert_eq!(state.inspector_synced_entity, None);
        assert_eq!(state.editor_events, 1);
    }

    #[test]
    fn syncing_inspector_buffers_captures_transform_fields() {
        let entity = Entity::from_raw_u32(3).unwrap();
        let transform = Transform {
            translation: Vec3::new(4.0, 5.0, 6.0),
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                10.0_f32.to_radians(),
                20.0_f32.to_radians(),
                30.0_f32.to_radians(),
            ),
            scale: Vec3::new(1.5, 2.0, 2.5),
        };
        let mut state = EditorState::default();

        sync_inspector_buffers_from_values(&mut state, entity, "Camera Preview", transform);

        assert_eq!(state.selected_entity, Some(entity));
        assert_eq!(state.inspector_name_buffer, "Camera Preview");
        assert_eq!(state.inspector_translation, [4.0, 5.0, 6.0]);
        assert_eq!(state.inspector_scale, [1.5, 2.0, 2.5]);
        assert!((state.inspector_rotation_deg[0] - 10.0).abs() < 0.001);
        assert!((state.inspector_rotation_deg[1] - 20.0).abs() < 0.001);
        assert!((state.inspector_rotation_deg[2] - 30.0).abs() < 0.001);
    }
}

fn fit_aspect(available: [f32; 2], target: [u32; 2]) -> [f32; 2] {
    let target_aspect = target[0] as f32 / target[1] as f32;
    let available_aspect = available[0] / available[1].max(1.0);
    if available_aspect > target_aspect {
        [available[1] * target_aspect, available[1]]
    } else {
        [available[0], available[0] / target_aspect]
    }
}
