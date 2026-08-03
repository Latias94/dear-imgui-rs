//! Render one Dear ImGui Context to a Bevy `Image`, then present it in the primary Context.
//!
//! The two Contexts never sample their own render target: the offscreen Context writes the image,
//! while the primary Context holds a strong RAII texture lease and displays it. The displayed item
//! rectangle becomes an explicit logical input route, so resizing or moving the host window does
//! not broadcast input to the offscreen Context.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example render_to_image`

use bevy::{
    app::AppExit,
    camera::{RenderTarget, visibility::RenderLayers},
    ecs::schedule::ScheduleLabel,
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat, TextureUsages},
    window::{PresentMode, PrimaryWindow, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::prelude::*;
use dear_imgui_rs::Condition;

const INITIAL_TARGET_SIZE: UVec2 = UVec2::new(640, 360);

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct OffscreenContextPass;

#[derive(Resource)]
struct RenderToImageState {
    context_id: ContextId,
    host_window: Entity,
    image: Handle<Image>,
    texture: ImguiTexture,
    input_route: Option<Entity>,
    target_size: UVec2,
    requested_size: UVec2,
    clicks: u32,
    value: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "dear-imgui-bevy render to image".to_owned(),
                resolution: (1280, 760).into(),
                present_mode: PresentMode::AutoVsync,
                window_theme: Some(WindowTheme::Dark),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ImguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (close_on_escape, resize_render_target))
        .add_systems(ImguiPrimaryContextPass, primary_ui)
        .add_systems(ImguiContextPass::new(OffscreenContextPass), offscreen_ui)
        .run();
}

fn setup(
    mut commands: Commands,
    mut contexts: NonSendMut<ImguiContexts>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<ImguiBevyTextures>,
    primary_window: Single<Entity, With<PrimaryWindow>>,
) -> Result {
    let offscreen_context = contexts.create(
        ImguiContextConfig::new(ImguiContextPass::new(OffscreenContextPass)).with_docking(false),
    )?;

    let mut image = Image::new_target_texture(
        INITIAL_TARGET_SIZE.x,
        INITIAL_TARGET_SIZE.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.label = Some("dear_imgui_bevy_offscreen_context");
    image.texture_descriptor.usage |=
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let texture = textures
        .register_strong(image.clone())
        .map_err(|error| format!("failed to register the offscreen UI image: {error}"))?;

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.04, 0.05, 0.07)),
            ..Default::default()
        },
        RenderLayers::none(),
    ));
    let offscreen_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.10, 0.06, 0.14)),
                ..Default::default()
            },
            RenderTarget::Image(image.clone().into()),
            RenderLayers::none(),
        ))
        .id();
    commands.spawn(ImguiRenderRoute::new(offscreen_context, offscreen_camera));

    commands.insert_resource(RenderToImageState {
        context_id: offscreen_context,
        host_window: *primary_window,
        image,
        texture,
        input_route: None,
        target_size: INITIAL_TARGET_SIZE,
        requested_size: INITIAL_TARGET_SIZE,
        clicks: 0,
        value: 0.5,
    });
    Ok(())
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn resize_render_target(mut state: ResMut<RenderToImageState>, mut images: ResMut<Assets<Image>>) {
    if state.requested_size == state.target_size {
        return;
    }
    let Some(mut image) = images.get_mut(&state.image) else {
        return;
    };

    let size = state.requested_size.max(UVec2::ONE);
    image.resize(Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    });
    state.target_size = size;
}

fn primary_ui(
    mut commands: Commands,
    imgui: ImguiUi,
    mut state: ResMut<RenderToImageState>,
) -> Result {
    let ui = imgui.ui()?;

    ui.window("Offscreen Context Host")
        .position([32.0, 32.0], Condition::FirstUseEver)
        .size([840.0, 650.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("This image is the output of an independent Dear ImGui Context.");
            ui.text(format!(
                "Target: {} x {}",
                state.target_size.x, state.target_size.y
            ));

            if ui.button("640 x 360") {
                state.requested_size = UVec2::new(640, 360);
            }
            ui.same_line();
            if ui.button("768 x 432") {
                state.requested_size = UVec2::new(768, 432);
            }
            ui.same_line();
            if ui.button("480 x 480") {
                state.requested_size = UVec2::new(480, 480);
            }
            ui.separator();

            let available = ui.content_region_avail();
            let display_size = fit_aspect(
                [available[0].max(1.0), available[1].max(1.0)],
                state.target_size,
            );
            ui.image(&state.texture, display_size);

            let min = ui.item_rect_min();
            let max = ui.item_rect_max();
            let region = Rect {
                min: Vec2::new(min[0], min[1]),
                max: Vec2::new(max[0], max[1]),
            };
            let route = ImguiInputRoute::logical(state.context_id, state.host_window, region);
            if let Some(route_entity) = state.input_route {
                commands.entity(route_entity).insert(route);
            } else {
                state.input_route = Some(commands.spawn(route).id());
            }
        });

    Ok(())
}

fn offscreen_ui(imgui: ImguiUi, mut state: ResMut<RenderToImageState>) -> Result {
    let frame_index = imgui.frame_index()?;
    let ui = imgui.ui()?;

    ui.window("Rendered to a Bevy Image")
        .position([20.0, 20.0], Condition::Always)
        .size(
            [
                (state.target_size.x as f32 - 40.0).max(280.0),
                (state.target_size.y as f32 - 40.0).max(220.0),
            ],
            Condition::Always,
        )
        .build(|| {
            ui.text(format!("Offscreen frame: {frame_index}"));
            ui.text("Pointer and keyboard input come from the image rectangle.");
            ui.separator();
            if ui.button("Count click") {
                state.clicks = state.clicks.saturating_add(1);
            }
            ui.same_line();
            ui.text(format!("{}", state.clicks));
            ui.slider_f32("Value", &mut state.value, 0.0, 1.0);
            ui.progress_bar(state.value).build();
        });

    Ok(())
}

fn fit_aspect(available: [f32; 2], target: UVec2) -> [f32; 2] {
    let target_aspect = target.x as f32 / target.y.max(1) as f32;
    let available_aspect = available[0] / available[1].max(1.0);
    if available_aspect > target_aspect {
        [available[1] * target_aspect, available[1]]
    } else {
        [available[0], available[0] / target_aspect]
    }
}
