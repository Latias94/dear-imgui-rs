#![cfg(feature = "render")]

use bevy::{
    app::App,
    asset::Assets,
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
#[cfg(feature = "bevy-ui")]
use dear_imgui_bevy::render::ImguiUiRenderOrder;
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
    render::{ImguiOverlayCamera, ImguiRenderSystems},
};
use std::collections::HashMap;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const REQUIRED_CASES: usize = 8;

#[derive(Clone, Copy)]
enum CameraKind {
    Core2d,
    Core3d,
}

#[derive(Component)]
struct CompositionCase(&'static str);

#[derive(Resource, Default)]
struct CompositionReadbacks(HashMap<&'static str, Vec<u8>>);

#[test]
fn post_process_and_imgui_pixels_coexist_across_camera_msaa_and_hdr_modes() {
    if std::env::var("DEAR_IMGUI_BEVY_GPU_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "skipping Bevy GPU composition test; set DEAR_IMGUI_BEVY_GPU_TESTS=1 to require it"
        );
        return;
    }

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
    .add_plugins(ImguiPlugin::default())
    .init_resource::<CompositionReadbacks>()
    .add_observer(collect_readback)
    .add_systems(ImguiPrimaryContextPass, draw_composition_fixture);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    {
        let mut context = app.world_mut().non_send_mut::<ImguiContext>();
        configure_example_context(&mut context, false);
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

    for frame in 0..120 {
        app.update();
        if frame >= 30 && app.world().resource::<CompositionReadbacks>().0.len() == REQUIRED_CASES {
            break;
        }
    }

    let readbacks = app.world().resource::<CompositionReadbacks>();
    assert_eq!(
        readbacks.0.len(),
        REQUIRED_CASES,
        "every 2D/3D, MSAA, and HDR case must produce a GPU readback"
    );
    for (name, pixels) in &readbacks.0 {
        let post_process_pixel = rgba8_pixel(pixels, 48, 48);
        assert!(
            post_process_pixel[2] > post_process_pixel[1].saturating_add(20)
                && post_process_pixel[1] > post_process_pixel[0].saturating_add(20),
            "{name}: the custom post-process region was overwritten: {post_process_pixel:?}"
        );

        let imgui_pixel = rgba8_pixel(pixels, 12, 12);
        assert!(
            imgui_pixel[0] > imgui_pixel[1].saturating_add(80)
                && imgui_pixel[0] > imgui_pixel[2].saturating_add(80),
            "{name}: the Dear ImGui region was not preserved: {imgui_pixel:?}"
        );
    }
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

    let imgui_above = render_ui_order(ImguiUiRenderOrder::ImguiAboveBevyUi);
    assert!(
        imgui_above[0] > imgui_above[1].saturating_add(80)
            && imgui_above[0] > imgui_above[2].saturating_add(80),
        "Dear ImGui should cover Bevy UI in the default mode: {imgui_above:?}"
    );

    let bevy_ui_above = render_ui_order(ImguiUiRenderOrder::BevyUiAboveImgui);
    assert!(
        bevy_ui_above[2] > bevy_ui_above[0].saturating_add(80)
            && bevy_ui_above[2] > bevy_ui_above[1].saturating_add(80),
        "Bevy UI should cover Dear ImGui in the alternate mode: {bevy_ui_above:?}"
    );
}

#[cfg(feature = "bevy-ui")]
fn render_ui_order(order: ImguiUiRenderOrder) -> [u8; 4] {
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
    .add_plugins(ImguiPlugin::default().with_ui_render_order(order))
    .init_resource::<CompositionReadbacks>()
    .add_observer(collect_readback)
    .add_systems(ImguiPrimaryContextPass, draw_composition_fixture);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    {
        let mut context = app.world_mut().non_send_mut::<ImguiContext>();
        configure_example_context(&mut context, false);
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
            ImguiOverlayCamera,
        ))
        .id();
    app.world_mut().spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..Default::default()
        },
        BackgroundColor(BevyColor::srgb(0.0, 0.0, 1.0)),
        UiTargetCamera(camera),
    ));
    app.world_mut()
        .spawn((Readback::texture(image), CompositionCase("ui-order")));

    app.finish();
    app.cleanup();
    for _ in 0..40 {
        app.update();
    }

    let readbacks = app.world().resource::<CompositionReadbacks>();
    let pixels = readbacks
        .0
        .get("ui-order")
        .expect("the Bevy UI ordering fixture must produce a readback");
    rgba8_pixel(pixels, 12, 12)
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

    let mut entity = match kind {
        CameraKind::Core2d => app
            .world_mut()
            .spawn((Camera2d, target, msaa, ImguiOverlayCamera)),
        CameraKind::Core3d => {
            app.world_mut()
                .spawn((Camera3d::default(), target, msaa, ImguiOverlayCamera))
        }
    };
    if hdr {
        entity.insert((Hdr, Tonemapping::Reinhard));
    } else {
        entity.insert(Tonemapping::None);
    }

    app.world_mut()
        .spawn((Readback::texture(image), CompositionCase(name)));
}

fn draw_composition_fixture(mut contexts: ImguiContexts) {
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };
    ui.get_background_draw_list()
        .add_rect([8.0, 8.0], [24.0, 24.0], [1.0, 0.0, 0.0, 1.0])
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
    cases: Query<&CompositionCase>,
    mut readbacks: ResMut<CompositionReadbacks>,
) {
    let Ok(case) = cases.get(event.entity) else {
        return;
    };
    readbacks.0.insert(case.0, event.data.clone());
}

fn rgba8_pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let row_stride = WIDTH as usize * 4;
    let offset = y as usize * row_stride + x as usize * 4;
    data[offset..offset + 4].try_into().unwrap()
}
