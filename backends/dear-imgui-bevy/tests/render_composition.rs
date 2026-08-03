#![cfg(feature = "render")]

#[cfg(feature = "bevy-ui")]
use bevy::ecs::schedule::ScheduleLabel;
#[cfg(feature = "bevy-ui")]
use bevy::ecs::schedule::{NodeId, ScheduleGraph};
use bevy::{
    app::App,
    asset::{Assets, Handle},
    camera::{Hdr, RenderTarget},
    color::LinearRgba,
    core_pipeline::{Core2d, Core3d, tonemapping::Tonemapping},
    ecs::prelude::*,
    image::Image,
    prelude::{Camera2d, Camera3d, DefaultPlugins, PluginGroup, Window},
    render::{
        RenderApp,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{
            LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
            TextureFormat, TextureUsages,
        },
        renderer::{RenderContext, ViewQuery},
        view::{Msaa, ViewTarget},
    },
    window::{ExitCondition, PrimaryWindow, WindowPlugin, WindowResolution},
    winit::WinitPlugin,
};
#[cfg(feature = "bevy-ui")]
use bevy::{
    color::Color as BevyColor,
    prelude::{BackgroundColor, Node, UiTargetCamera, percent},
};
use dear_imgui_bevy::{
    ImguiAppExt, ImguiContextConfig, ImguiContexts, ImguiFrame, ImguiPlugin, ImguiPluginConfig,
    ImguiRenderSystems,
    route::{ImguiInputPolicy, ImguiInputRoute, ImguiInputSource, ImguiRenderRoute},
};
#[cfg(feature = "bevy-ui")]
use dear_imgui_bevy::{ImguiPrimaryPass, ImguiUiRenderOrder};
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const REQUIRED_CASES: usize = 8;
const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(20);
const GPU_READBACK_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy)]
enum CameraKind {
    Core2d,
    Core3d,
}

#[derive(Component)]
struct CompositionCase(&'static str);

#[derive(Component)]
struct CompositionReadbackTarget {
    image: Handle<Image>,
    expectation: CompositionExpectation,
}

#[derive(Clone, Copy)]
enum CompositionExpectation {
    PostProcessAndImgui,
    #[cfg(feature = "bevy-ui")]
    DominantRed {
        at: [u32; 2],
    },
    #[cfg(feature = "bevy-ui")]
    DominantBlue {
        at: [u32; 2],
    },
    OrderedContexts,
}

impl CompositionExpectation {
    fn is_satisfied(self, data: &[u8]) -> bool {
        if data.len() < WIDTH as usize * HEIGHT as usize * 4 {
            return false;
        }
        match self {
            Self::PostProcessAndImgui => {
                is_post_process_blue(rgba8_pixel(data, 48, 48))
                    && is_dominant_red(rgba8_pixel(data, 12, 12))
            }
            #[cfg(feature = "bevy-ui")]
            Self::DominantRed { at } => is_dominant_red(rgba8_pixel(data, at[0], at[1])),
            #[cfg(feature = "bevy-ui")]
            Self::DominantBlue { at } => is_dominant_blue(rgba8_pixel(data, at[0], at[1])),
            Self::OrderedContexts => {
                is_dominant_red(rgba8_pixel(data, 12, 12))
                    && is_dominant_blue(rgba8_pixel(data, 40, 40))
                    && is_dominant_blue(rgba8_pixel(data, 24, 24))
            }
        }
    }
}

struct CompositionPass;

struct SameCameraSecondaryPass;

#[derive(Resource, Default)]
struct CompositionReadbacks {
    samples: HashMap<&'static str, Vec<u8>>,
    completed: HashSet<&'static str>,
}

#[test]
fn post_process_and_imgui_pixels_coexist_across_camera_msaa_and_hdr_modes() {
    if std::env::var("DEAR_IMGUI_BEVY_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "skipping Bevy GPU composition test; set DEAR_IMGUI_BEVY_GPU_TESTS=1 to require it"
        );
        return;
    }
    let _guard = gpu_test_guard();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_docking(false),
    ))
    .init_resource::<CompositionReadbacks>()
    .add_observer(collect_readback);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let primary = contexts.primary_id().expect("primary Context must exist");
        contexts
            .configure(primary, |context| {
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
            })
            .expect("primary Context configuration must succeed");
    }

    for kind in [CameraKind::Core2d, CameraKind::Core3d] {
        for msaa in [Msaa::Off, Msaa::Sample4] {
            for hdr in [false, true] {
                spawn_case(&mut app, kind, msaa, hdr);
            }
        }
    }

    {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_systems(
                Core2d,
                write_custom_post_process.in_set(ImguiRenderSystems::BeforeOverlay),
            )
            .add_systems(
                Core3d,
                write_custom_post_process.in_set(ImguiRenderSystems::BeforeOverlay),
            )
            .add_systems(
                Core2d,
                preserve_after_overlay.in_set(ImguiRenderSystems::AfterOverlay),
            )
            .add_systems(
                Core3d,
                preserve_after_overlay.in_set(ImguiRenderSystems::AfterOverlay),
            );
    }

    app.finish();
    app.cleanup();
    install_composition_readbacks(&mut app);
    wait_for_composition_readbacks(&mut app, REQUIRED_CASES);

    let readbacks = app.world().resource::<CompositionReadbacks>();
    let mut failures = Vec::new();
    if readbacks.completed.len() != REQUIRED_CASES {
        failures.push(format!(
            "only {}/{} cases satisfied their GPU readback contract: {:?}",
            readbacks.completed.len(),
            REQUIRED_CASES,
            readbacks.completed,
        ));
    }
    let mut cases = readbacks.samples.iter().collect::<Vec<_>>();
    cases.sort_unstable_by_key(|(name, _)| **name);
    for (name, pixels) in cases {
        let post_process_pixel = rgba8_pixel(pixels, 48, 48);
        if !(post_process_pixel[2] > post_process_pixel[1].saturating_add(20)
            && post_process_pixel[1] > post_process_pixel[0].saturating_add(20))
        {
            failures.push(format!(
                "{name}: the custom post-process region was overwritten: {post_process_pixel:?}"
            ));
        }

        let imgui_pixel = rgba8_pixel(pixels, 12, 12);
        if !(imgui_pixel[0] > imgui_pixel[1].saturating_add(80)
            && imgui_pixel[0] > imgui_pixel[2].saturating_add(80))
        {
            failures.push(format!(
                "{name}: the Dear ImGui region was not preserved: {imgui_pixel:?}; red bounds: {:?}",
                dominant_red_bounds(pixels)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "composition failures:\n{}",
        failures.join("\n")
    );
}

#[cfg(feature = "bevy-ui")]
#[test]
fn bevy_ui_order_modes_control_overlap_pixels() {
    if std::env::var("DEAR_IMGUI_BEVY_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "skipping Bevy UI GPU ordering test; set DEAR_IMGUI_BEVY_GPU_TESTS=1 to require it"
        );
        return;
    }
    let _guard = gpu_test_guard();

    let imgui_above = render_ui_order(
        ImguiUiRenderOrder::ImguiAboveBevyUi,
        CompositionExpectation::DominantRed { at: [12, 12] },
    );
    assert!(
        imgui_above[0] > imgui_above[1].saturating_add(80)
            && imgui_above[0] > imgui_above[2].saturating_add(80),
        "Dear ImGui should cover Bevy UI in the default mode: {imgui_above:?}"
    );

    let bevy_ui_above = render_ui_order(
        ImguiUiRenderOrder::BevyUiAboveImgui,
        CompositionExpectation::DominantBlue { at: [12, 12] },
    );
    assert!(
        bevy_ui_above[2] > bevy_ui_above[0].saturating_add(80)
            && bevy_ui_above[2] > bevy_ui_above[1].saturating_add(80),
        "Bevy UI should cover Dear ImGui in the alternate mode: {bevy_ui_above:?}"
    );
}

#[cfg(feature = "bevy-ui")]
#[test]
fn bevy_ui_order_modes_define_complete_overlay_topology() {
    for (order, expected) in [
        (
            ImguiUiRenderOrder::ImguiAboveBevyUi,
            [
                RenderTopologyNode::BeforeOverlay,
                RenderTopologyNode::BevyUi,
                RenderTopologyNode::Overlay,
                RenderTopologyNode::AfterOverlay,
            ],
        ),
        (
            ImguiUiRenderOrder::BevyUiAboveImgui,
            [
                RenderTopologyNode::BeforeOverlay,
                RenderTopologyNode::Overlay,
                RenderTopologyNode::BevyUi,
                RenderTopologyNode::AfterOverlay,
            ],
        ),
    ] {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..Default::default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(
            ImguiPlugin::new(ImguiPluginConfig::default().with_docking(false))
                .with_ui_render_order(order),
        );

        assert_render_dependency_chain(&app, Core2d, expected);
        assert_render_dependency_chain(&app, Core3d, expected);
    }
}

#[test]
fn multiple_contexts_compose_in_route_order_on_one_camera() {
    if std::env::var("DEAR_IMGUI_BEVY_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "skipping Bevy multi-Context GPU ordering test; set \
             DEAR_IMGUI_BEVY_GPU_TESTS=1 to require it"
        );
        return;
    }
    let _guard = gpu_test_guard();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_docking(false),
    ))
    .init_resource::<CompositionReadbacks>()
    .add_observer(collect_readback);

    let primary_pass = app.imgui_primary_pass();
    let secondary_pass = app.declare_imgui_pass::<SameCameraSecondaryPass>();
    app.add_imgui_system(&primary_pass, draw_primary_context_fixture)
        .add_imgui_system(&secondary_pass, draw_secondary_context_fixture);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    let (primary, secondary) = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let primary = contexts.primary_id().expect("primary Context must exist");
        contexts
            .configure(primary, |context| {
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
            })
            .expect("primary Context configuration must succeed");

        let secondary = contexts
            .create(ImguiContextConfig::new(secondary_pass).with_docking(false))
            .expect("secondary Context creation must succeed");
        contexts
            .configure(secondary, |context| {
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
            })
            .expect("secondary Context configuration must succeed");
        (primary, secondary)
    };

    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(image.clone().into()),
            Msaa::Off,
        ))
        .id();

    app.world_mut().spawn((
        ImguiRenderRoute::new(primary, camera).with_order(0),
        ImguiInputRoute::new(primary, ImguiInputSource::camera(camera))
            .with_policy(ImguiInputPolicy::Disabled),
    ));
    app.world_mut().spawn((
        ImguiRenderRoute::new(secondary, camera).with_order(10),
        ImguiInputRoute::new(secondary, ImguiInputSource::camera(camera))
            .with_policy(ImguiInputPolicy::Disabled),
    ));
    app.world_mut().spawn((
        CompositionReadbackTarget {
            image,
            expectation: CompositionExpectation::OrderedContexts,
        },
        CompositionCase("same-camera-context-order"),
    ));

    app.finish();
    app.cleanup();
    install_composition_readbacks(&mut app);
    wait_for_composition_readbacks(&mut app, 1);

    let readbacks = app.world().resource::<CompositionReadbacks>();
    let pixels = readbacks
        .samples
        .get("same-camera-context-order")
        .expect("the same-camera multi-Context fixture must produce a readback");
    assert_dominant_red(
        rgba8_pixel(pixels, 12, 12),
        "the primary Context must render its exclusive region",
    );
    assert_dominant_blue(
        rgba8_pixel(pixels, 40, 40),
        "the secondary Context must render its exclusive region",
    );
    assert_dominant_blue(
        rgba8_pixel(pixels, 24, 24),
        "the higher-order secondary Context must win the overlap",
    );
}

#[cfg(feature = "bevy-ui")]
fn render_ui_order(order: ImguiUiRenderOrder, expectation: CompositionExpectation) -> [u8; 4] {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(
        ImguiPlugin::new(ImguiPluginConfig::default().with_docking(false))
            .with_ui_render_order(order),
    )
    .init_resource::<CompositionReadbacks>()
    .add_observer(collect_readback);

    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_system(&primary_pass, draw_composition_fixture::<ImguiPrimaryPass>);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let primary = contexts.primary_id().expect("primary Context must exist");
        contexts
            .configure(primary, |context| {
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
            })
            .expect("primary Context configuration must succeed");
    }

    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(image.clone().into()),
            Msaa::Off,
        ))
        .id();
    let primary = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("primary Context must exist");
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.world_mut().spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..Default::default()
        },
        BackgroundColor(BevyColor::srgb(0.0, 0.0, 1.0)),
        UiTargetCamera(camera),
    ));
    app.world_mut().spawn((
        CompositionReadbackTarget { image, expectation },
        CompositionCase("ui-order"),
    ));

    app.finish();
    app.cleanup();
    install_composition_readbacks(&mut app);
    wait_for_composition_readbacks(&mut app, 1);

    let readbacks = app.world().resource::<CompositionReadbacks>();
    let pixels = readbacks
        .samples
        .get("ui-order")
        .expect("the Bevy UI ordering fixture must produce a readback");
    rgba8_pixel(pixels, 12, 12)
}

#[cfg(feature = "bevy-ui")]
#[derive(Clone, Copy, Debug)]
enum RenderTopologyNode {
    BeforeOverlay,
    BevyUi,
    Overlay,
    AfterOverlay,
}

#[cfg(feature = "bevy-ui")]
fn assert_render_dependency_chain(
    app: &App,
    schedule_label: impl ScheduleLabel,
    expected: [RenderTopologyNode; 4],
) {
    let render_world = app.sub_app(RenderApp).world();
    let schedules = render_world.resource::<Schedules>();
    let schedule = schedules
        .get(schedule_label)
        .expect("the core render schedule must be installed");
    let graph = schedule.graph();
    let dependencies = graph.dependency().graph();

    for pair in expected.windows(2) {
        let before = render_topology_node(graph, pair[0]);
        let after = render_topology_node(graph, pair[1]);
        assert!(
            dependencies.contains_edge(before, after),
            "missing direct render dependency {:?} -> {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[cfg(feature = "bevy-ui")]
fn render_topology_node(graph: &ScheduleGraph, node: RenderTopologyNode) -> NodeId {
    let set = match node {
        RenderTopologyNode::BeforeOverlay => ImguiRenderSystems::BeforeOverlay.intern(),
        RenderTopologyNode::BevyUi => {
            IntoSystemSet::into_system_set(bevy_ui_render::ui_pass).intern()
        }
        RenderTopologyNode::Overlay => ImguiRenderSystems::Overlay.intern(),
        RenderTopologyNode::AfterOverlay => ImguiRenderSystems::AfterOverlay.intern(),
    };
    let key = graph
        .system_sets
        .get_key(set)
        .expect("every topology node must be registered as a system set");
    NodeId::Set(key)
}

fn spawn_case(app: &mut App, kind: CameraKind, msaa: Msaa, hdr: bool) {
    let name = match (kind, msaa, hdr) {
        (CameraKind::Core2d, Msaa::Off, false) => "core2d-1x-ldr",
        (CameraKind::Core2d, Msaa::Off, true) => "core2d-1x-hdr",
        (CameraKind::Core2d, Msaa::Sample4, false) => "core2d-4x-ldr",
        (CameraKind::Core2d, Msaa::Sample4, true) => "core2d-4x-hdr",
        (CameraKind::Core3d, Msaa::Off, false) => "core3d-1x-ldr",
        (CameraKind::Core3d, Msaa::Off, true) => "core3d-1x-hdr",
        (CameraKind::Core3d, Msaa::Sample4, false) => "core3d-4x-ldr",
        (CameraKind::Core3d, Msaa::Sample4, true) => "core3d-4x-hdr",
        _ => unreachable!("the composition matrix uses only Off and Sample4"),
    };

    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let target = RenderTarget::Image(image.clone().into());

    let camera = {
        let mut entity = match kind {
            CameraKind::Core2d => app.world_mut().spawn((Camera2d, target, msaa)),
            CameraKind::Core3d => app.world_mut().spawn((Camera3d::default(), target, msaa)),
        };
        if hdr {
            entity.insert((Hdr, Tonemapping::Reinhard));
        } else {
            entity.insert(Tonemapping::None);
        }
        entity.id()
    };
    let pass = app.declare_imgui_pass::<CompositionPass>();
    app.add_imgui_system(&pass, draw_composition_fixture::<CompositionPass>);
    let context_id = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let context_id = contexts
            .create(ImguiContextConfig::new(pass).with_docking(false))
            .expect("each composition camera needs an independently routed Context");
        contexts
            .configure(context_id, |context| {
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
            })
            .expect("composition Context configuration must succeed");
        context_id
    };
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, camera));
    app.world_mut().spawn((
        CompositionReadbackTarget {
            image,
            expectation: CompositionExpectation::PostProcessAndImgui,
        },
        CompositionCase(name),
    ));
}

fn draw_composition_fixture<P: 'static>(frame: ImguiFrame<'_, P>) {
    let ui = frame.ui();
    ui.get_background_draw_list()
        .add_rect([8.0, 8.0], [24.0, 24.0], [1.0, 0.0, 0.0, 1.0])
        .filled(true)
        .build();
}

fn draw_primary_context_fixture(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.get_background_draw_list()
        .add_rect([8.0, 8.0], [32.0, 32.0], [1.0, 0.0, 0.0, 1.0])
        .filled(true)
        .build();
}

fn draw_secondary_context_fixture(frame: ImguiFrame<'_, SameCameraSecondaryPass>) {
    let ui = frame.ui();
    ui.get_background_draw_list()
        .add_rect([20.0, 20.0], [48.0, 48.0], [0.0, 0.0, 1.0, 1.0])
        .filled(true)
        .build();
}

fn write_custom_post_process(view: ViewQuery<&ViewTarget>, mut render_context: RenderContext) {
    let post_process = view.into_inner().post_process_write();
    let _pass = render_context
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("dear_imgui_bevy_composition_fixture"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(LinearRgba::new(0.05, 0.25, 0.65, 1.0).into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
}

fn preserve_after_overlay(view: ViewQuery<&ViewTarget>, mut render_context: RenderContext) {
    let attachment = view.into_inner().get_unsampled_color_attachment();
    let _pass = render_context
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("dear_imgui_bevy_after_overlay_fixture"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: attachment.view,
                depth_slice: attachment.depth_slice,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
}

fn collect_readback(
    event: On<ReadbackComplete>,
    cases: Query<(&CompositionCase, &CompositionReadbackTarget)>,
    mut readbacks: ResMut<CompositionReadbacks>,
    mut commands: Commands,
) {
    let Ok((case, target)) = cases.get(event.entity) else {
        return;
    };
    if readbacks.completed.contains(case.0) {
        return;
    }
    // The first asynchronous readback may predate the first completed render. Keep sampling until
    // the case-specific pixel contract succeeds, then freeze that successful frame.
    readbacks.samples.insert(case.0, event.data.clone());
    if target.expectation.is_satisfied(&event.data) {
        readbacks.completed.insert(case.0);
        commands.entity(event.entity).remove::<Readback>();
    }
}

fn install_composition_readbacks(app: &mut App) {
    let targets = app
        .world_mut()
        .query::<(Entity, &CompositionReadbackTarget)>()
        .iter(app.world())
        .map(|(entity, target)| (entity, target.image.clone()))
        .collect::<Vec<_>>();
    for (entity, image) in targets {
        app.world_mut()
            .entity_mut(entity)
            .insert(Readback::texture(image));
    }
}

fn wait_for_composition_readbacks(app: &mut App, expected: usize) {
    let deadline = Instant::now() + GPU_READBACK_TIMEOUT;
    loop {
        app.update();
        if app
            .world()
            .resource::<CompositionReadbacks>()
            .completed
            .len()
            >= expected
            || Instant::now() >= deadline
        {
            return;
        }
        std::thread::sleep(GPU_READBACK_POLL_INTERVAL);
    }
}

fn rgba8_pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let row_stride = WIDTH as usize * 4;
    let offset = y as usize * row_stride + x as usize * 4;
    data[offset..offset + 4].try_into().unwrap()
}

fn is_post_process_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > pixel[1].saturating_add(20) && pixel[1] > pixel[0].saturating_add(20)
}

fn is_dominant_red(pixel: [u8; 4]) -> bool {
    pixel[0] > pixel[1].saturating_add(80) && pixel[0] > pixel[2].saturating_add(80)
}

fn is_dominant_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > pixel[0].saturating_add(80) && pixel[2] > pixel[1].saturating_add(80)
}

fn assert_dominant_red(pixel: [u8; 4], message: &str) {
    assert!(is_dominant_red(pixel), "{message}: {pixel:?}");
}

fn assert_dominant_blue(pixel: [u8; 4], message: &str) {
    assert!(is_dominant_blue(pixel), "{message}: {pixel:?}");
}

fn gpu_test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn dominant_red_bounds(data: &[u8]) -> Option<([u32; 2], [u32; 2])> {
    let mut min = [WIDTH, HEIGHT];
    let mut max = [0, 0];
    let mut found = false;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel = rgba8_pixel(data, x, y);
            if pixel[0] > pixel[1].saturating_add(80) && pixel[0] > pixel[2].saturating_add(80) {
                min[0] = min[0].min(x);
                min[1] = min[1].min(y);
                max[0] = max[0].max(x);
                max[1] = max[1].max(y);
                found = true;
            }
        }
    }
    found.then_some((min, max))
}
