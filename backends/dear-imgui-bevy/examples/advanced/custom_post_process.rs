//! Compose Dear ImGui with a fullscreen Bevy post-process before or after the overlay.
//!
//! The same shader can be moved between `ImguiRenderSystems::BeforeOverlay` and
//! `ImguiRenderSystems::AfterOverlay`. In the latter mode it grades the UI as well as the scene.
//! MSAA and HDR can also be toggled at runtime.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example custom_post_process`

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    camera::Hdr,
    core_pipeline::{
        Core2d,
        fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
        tonemapping::Tonemapping,
    },
    ecs::{
        schedule::{IntoScheduleConfigs, ScheduleConfigs, ScheduleLabel},
        system::BoxedSystem,
    },
    prelude::*,
    render::{extract_component::ExtractComponent, render_resource::ShaderType, view::Msaa},
    shader::ShaderRef,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::prelude::*;
use dear_imgui_rs::Condition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectOrder {
    BeforeOverlay,
    AfterOverlay,
}

#[derive(Resource)]
struct CompositionSettings {
    effect_order: EffectOrder,
    intensity: f32,
    sample4: bool,
    hdr: bool,
}

impl Default for CompositionSettings {
    fn default() -> Self {
        Self {
            effect_order: EffectOrder::BeforeOverlay,
            intensity: 0.65,
            sample4: true,
            hdr: false,
        }
    }
}

#[derive(Resource)]
struct CompositionCamera(Entity);

#[derive(Component)]
struct AnimatedShape {
    speed: f32,
}

#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
struct BeforeOverlayEffect {
    intensity: f32,
    _padding: Vec3,
}

#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
struct AfterOverlayEffect {
    intensity: f32,
    _padding: Vec3,
}

impl BeforeOverlayEffect {
    fn new(intensity: f32) -> Self {
        Self {
            intensity,
            _padding: Vec3::ZERO,
        }
    }
}

impl AfterOverlayEffect {
    fn new(intensity: f32) -> Self {
        Self {
            intensity,
            _padding: Vec3::ZERO,
        }
    }
}

impl FullscreenMaterial for BeforeOverlayEffect {
    fn fragment_shader() -> ShaderRef {
        "custom_post_process.wgsl".into()
    }

    fn schedule() -> impl ScheduleLabel + Clone {
        Core2d
    }

    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system.in_set(ImguiRenderSystems::BeforeOverlay)
    }
}

impl FullscreenMaterial for AfterOverlayEffect {
    fn fragment_shader() -> ShaderRef {
        "custom_post_process.wgsl".into()
    }

    fn schedule() -> impl ScheduleLabel + Clone {
        Core2d
    }

    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system.in_set(ImguiRenderSystems::AfterOverlay)
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: format!("{}/examples/advanced", env!("CARGO_MANIFEST_DIR")),
                ..Default::default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "dear-imgui-bevy custom post-process".to_owned(),
                    resolution: (1180, 720).into(),
                    present_mode: PresentMode::AutoVsync,
                    window_theme: Some(WindowTheme::Dark),
                    ..Default::default()
                }),
                ..Default::default()
            }),
    )
    .add_plugins((
        ImguiPlugin::default(),
        FullscreenMaterialPlugin::<BeforeOverlayEffect>::default(),
        FullscreenMaterialPlugin::<AfterOverlayEffect>::default(),
    ))
    .init_resource::<CompositionSettings>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (close_on_escape, animate_scene, apply_composition_settings),
    );
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(composition_ui))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Sprite::from_color(Color::srgb(0.08, 0.12, 0.20), Vec2::new(980.0, 540.0)),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
    for (position, size, color, speed) in [
        (
            Vec3::new(-270.0, 100.0, 0.0),
            Vec2::new(220.0, 150.0),
            Color::srgb(0.92, 0.30, 0.24),
            0.45,
        ),
        (
            Vec3::new(20.0, -40.0, 0.1),
            Vec2::new(250.0, 180.0),
            Color::srgb(0.20, 0.70, 0.92),
            -0.32,
        ),
        (
            Vec3::new(300.0, 110.0, 0.2),
            Vec2::new(170.0, 210.0),
            Color::srgb(0.28, 0.84, 0.50),
            0.27,
        ),
    ] {
        commands.spawn((
            Sprite::from_color(color, size),
            Transform::from_translation(position),
            AnimatedShape { speed },
        ));
    }

    let camera = commands
        .spawn((
            Camera2d,
            Msaa::Sample4,
            Tonemapping::None,
            BeforeOverlayEffect::new(CompositionSettings::default().intensity),
        ))
        .id();
    commands.insert_resource(CompositionCamera(camera));
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn animate_scene(time: Res<Time>, mut shapes: Query<(&mut Transform, &AnimatedShape)>) {
    for (mut transform, shape) in &mut shapes {
        transform.rotate_z(shape.speed * time.delta_secs());
    }
}

fn apply_composition_settings(
    mut commands: Commands,
    settings: Res<CompositionSettings>,
    camera: Res<CompositionCamera>,
) {
    if !settings.is_changed() {
        return;
    }

    let mut entity = commands.entity(camera.0);
    entity.insert(if settings.sample4 {
        Msaa::Sample4
    } else {
        Msaa::Off
    });
    if settings.hdr {
        entity.insert((Hdr, Tonemapping::Reinhard));
    } else {
        entity.remove::<Hdr>().insert(Tonemapping::None);
    }

    match settings.effect_order {
        EffectOrder::BeforeOverlay => {
            entity
                .remove::<AfterOverlayEffect>()
                .insert(BeforeOverlayEffect::new(settings.intensity));
        }
        EffectOrder::AfterOverlay => {
            entity
                .remove::<BeforeOverlayEffect>()
                .insert(AfterOverlayEffect::new(settings.intensity));
        }
    }
}

fn composition_ui(frame: ImguiFrame<'_>, mut settings: ResMut<CompositionSettings>) -> Result {
    let ui = frame.ui();

    ui.window("Composition Contract")
        .position([32.0, 32.0], Condition::FirstUseEver)
        .size([440.0, 340.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("The fullscreen effect is ordered through public render sets.");
            ui.separator();

            if ui.radio_button(
                "Effect before ImGui",
                settings.effect_order == EffectOrder::BeforeOverlay,
            ) {
                settings.effect_order = EffectOrder::BeforeOverlay;
            }
            if ui.radio_button(
                "Effect after ImGui",
                settings.effect_order == EffectOrder::AfterOverlay,
            ) {
                settings.effect_order = EffectOrder::AfterOverlay;
            }
            ui.slider_f32("Grade intensity", &mut settings.intensity, 0.0, 1.0);
            ui.checkbox("MSAA Sample4", &mut settings.sample4);
            ui.checkbox("HDR + Reinhard", &mut settings.hdr);

            ui.separator();
            match settings.effect_order {
                EffectOrder::BeforeOverlay => {
                    ui.text("The scene is graded; Dear ImGui keeps its original colors.");
                }
                EffectOrder::AfterOverlay => {
                    ui.text("The same pass grades both the scene and Dear ImGui.");
                }
            }
        });

    Ok(())
}
