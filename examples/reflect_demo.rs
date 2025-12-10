//! Reflect Demo
//!
//! Showcase for `dear-imgui-reflect`: struct/enum reflection, numeric sliders,
//! bool styles, text attributes, containers, and glam vectors.

use dear_app::{AddOnsConfig, RunnerConfig, run};
use dear_imgui_reflect as reflect;
use reflect::imgui::*;
use reflect::{ImGuiReflect, ImGuiReflectExt};

use glam::{Vec2, Vec3, Vec4};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Demo types
// ---------------------------------------------------------------------------

/// Quality setting with custom display names
#[derive(ImGuiReflect)]
enum Quality {
    #[imgui(name = "Low (Fast)")]
    Low,
    Medium,
    High,
}

impl Default for Quality {
    fn default() -> Self {
        Quality::Medium
    }
}

/// Render mode shown as a radio-button style enum
#[derive(ImGuiReflect)]
#[imgui(enum_style = "radio")]
enum RenderMode {
    Forward,
    Deferred,
    PathTracing,
}

impl Default for RenderMode {
    fn default() -> Self {
        RenderMode::Forward
    }
}

/// Primitive numeric + bool showcase
#[derive(ImGuiReflect, Default)]
struct PrimitivesDemo {
    #[imgui(name = "Volume", slider, min = 0, max = 100)]
    volume: i32,

    #[imgui(slider, min = 0.0, max = 1.0, format = "%.3f")]
    gain: f32,

    /// Integer edited via InputScalar, with step / fast-step.
    #[imgui(name = "Counter (Input)", as_input, step = 1, step_fast = 10)]
    counter: i32,

    /// Float edited via a drag widget with speed/range/format and log+clamp flags.
    #[imgui(
        name = "Drag (Speed/Log)",
        as_drag,
        speed = 0.1,
        min = 0.0,
        max = 10.0,
        format = "%.2f",
        log,
        clamp,
        always_clamp
    )]
    drag_value: f32,

    enabled: bool,

    #[imgui(bool_style = "button", true_text = "On", false_text = "Off")]
    power: bool,

    /// Bool rendered as a dropdown combo.
    #[imgui(
        bool_style = "dropdown",
        true_text = "Enabled",
        false_text = "Disabled"
    )]
    dropdown_toggle: bool,

    #[imgui(bool_style = "radio", true_text = "Yes", false_text = "No")]
    debug_mode: bool,

    /// Integer displayed in hexadecimal form to demonstrate numeric formatting.
    #[imgui(name = "Hex Counter", as_input, hex)]
    hex_counter: i32,

    /// Floating-point value displayed as a percentage.
    #[imgui(name = "Percent", as_drag, percentage, speed = 0.5)]
    percent_value: f32,
}

/// Text / containers / nesting showcase
#[derive(ImGuiReflect, Default)]
struct TextAndContainersDemo {
    #[imgui(hint = "Window title", min_width = 200.0)]
    title: String,

    #[imgui(multiline, read_only, auto_resize)]
    description: String,

    /// Display-only status text rendered without an input box.
    #[imgui(display_only)]
    status: String,

    extra_gain: Option<f32>,

    samples: Vec<i32>,

    offset: [f32; 3],
}

/// glam vector types (requires `dear-imgui-reflect` `glam` feature)
#[derive(ImGuiReflect, Default)]
struct GlamDemo {
    vec2: Vec2,
    vec3: Vec3,
    vec4: Vec4,
}

/// Maps and tuples demo (string-key maps and small tuples).
#[derive(ImGuiReflect, Default)]
struct MapAndTupleDemo {
    hash: HashMap<String, i32>,
    btree: BTreeMap<String, f32>,
    pair_int_float: (i32, f32),
    triple_mixed: (bool, i32, f32),
    quad_tuple: (i32, i32, i32, i32),
    /// Tuple using per-element numeric settings to demonstrate tuple member overrides.
    #[imgui(name = "Color (tuple)", tuple_render = "grid", tuple_columns = 4)]
    color: (f32, f32, f32, f32),
}

/// Smart-pointer wrappers demo.
#[derive(ImGuiReflect, Default)]
struct PointerDemo {
    boxed_primitives: Box<PrimitivesDemo>,
    rc_primitives: Rc<PrimitivesDemo>,
    arc_primitives: Arc<PrimitivesDemo>,
}

/// Type-level numeric defaults demo (no per-field numeric attributes).
#[derive(ImGuiReflect, Default)]
struct TypeLevelNumericDemo {
    i_primary: i32,
    i_secondary: i32,
    f_primary: f32,
    f_secondary: f32,
}

/// All demo groups in a single struct for convenience
#[derive(ImGuiReflect, Default)]
struct ReflectDemoState {
    primitives: PrimitivesDemo,
    text_and_containers: TextAndContainersDemo,
    quality: Quality,
    mode: RenderMode,
    glam: GlamDemo,
    maps_and_tuples: MapAndTupleDemo,
    pointers: PointerDemo,
    type_level_numerics: TypeLevelNumericDemo,
}

fn configure_global_reflect_settings() {
    use reflect::{NumericRange, NumericTypeSettings, NumericWidgetKind, TupleRenderMode};

    reflect::with_settings(|settings| {
        // Demonstrate type-level defaults for i32: sliders 0..100 with clamping.
        *settings.numerics_i32_mut() = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: 0.0,
                max: 100.0,
            },
            speed: None,
            step: Some(1.0),
            step_fast: Some(10.0),
            format: Some("%.0f".to_owned()),
            log: false,
            clamp: true,
            always_clamp: true,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: false,
        };

        // Demonstrate type-level defaults for f32: drag widgets with a small speed.
        *settings.numerics_f32_mut() = NumericTypeSettings {
            widget: NumericWidgetKind::Drag,
            range: NumericRange::None,
            speed: Some(0.1),
            step: None,
            step_fast: None,
            format: Some("%.3f".to_owned()),
            log: false,
            clamp: false,
            always_clamp: false,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: true,
        };

        // Use grid rendering for tuples to highlight TupleSettings.
        let tuples = settings.tuples_mut();
        tuples.dropdown = false;
        tuples.render_mode = TupleRenderMode::Grid;
        tuples.columns = 4;
        tuples.same_line = true;
        tuples.min_width = Some(80.0);

        // Demonstrate member-level tuple element settings on MapAndTupleDemo::color:
        //
        // - color[0]: slider in [0, 1]
        // - color[1]: slider in [-1, 1]
        // - color[2]: drag with small speed
        // - color[3]: slider in [0, 1] but read-only
        let color0 = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit { min: 0.0, max: 1.0 },
            speed: None,
            step: None,
            step_fast: None,
            format: Some("%.3f".to_owned()),
            log: false,
            clamp: true,
            always_clamp: true,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: false,
        };
        settings
            .for_member::<MapAndTupleDemo>("color[0]")
            .numerics_f32 = Some(color0);

        let color1 = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit {
                min: -1.0,
                max: 1.0,
            },
            speed: None,
            step: None,
            step_fast: None,
            format: Some("%.3f".to_owned()),
            log: false,
            clamp: true,
            always_clamp: true,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: false,
        };
        settings
            .for_member::<MapAndTupleDemo>("color[1]")
            .numerics_f32 = Some(color1);

        let color2 = NumericTypeSettings {
            widget: NumericWidgetKind::Drag,
            range: NumericRange::None,
            speed: Some(0.01),
            step: None,
            step_fast: None,
            format: Some("%.4f".to_owned()),
            log: false,
            clamp: false,
            always_clamp: false,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: true,
        };
        settings
            .for_member::<MapAndTupleDemo>("color[2]")
            .numerics_f32 = Some(color2);

        let color3 = NumericTypeSettings {
            widget: NumericWidgetKind::Slider,
            range: NumericRange::Explicit { min: 0.0, max: 1.0 },
            speed: None,
            step: None,
            step_fast: None,
            format: Some("%.2f".to_owned()),
            log: false,
            clamp: true,
            always_clamp: true,
            wrap_around: false,
            no_round_to_format: false,
            no_input: false,
            clamp_on_input: false,
            clamp_zero_range: false,
            no_speed_tweaks: false,
        };
        let color3_member = settings.for_member::<MapAndTupleDemo>("color[3]");
        color3_member.numerics_f32 = Some(color3);
        color3_member.read_only = true;
    });
}

fn main() {
    // Basic logging
    dear_imgui_rs::logging::init_tracing_with_filter("dear_imgui=info,reflect_demo=info,wgpu=warn");

    let runner = RunnerConfig {
        window_title: "Dear ImGui Reflect Demo".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.12, 0.16, 1.0],
        docking: Default::default(),
        ini_filename: None,
        restore_previous_geometry: true,
        redraw: dear_app::RedrawMode::Poll,
        io_config_flags: None,
        ..Default::default()
    };

    let addons = AddOnsConfig::auto();

    let mut state = ReflectDemoState {
        text_and_containers: TextAndContainersDemo {
            title: "Reflect Demo".to_owned(),
            description: "This panel is generated by dear-imgui-reflect.\n\
                          It demonstrates primitives, containers, enums,\n\
                          text attributes, and glam vectors."
                .to_owned(),
            status: "Status: all systems nominal".to_owned(),
            samples: vec![1, 2, 3, 5, 8, 13],
            offset: [0.0, 1.0, 2.0],
            ..Default::default()
        },
        ..Default::default()
    };

    run(runner, addons, move |ui, _addons| {
        // Configure global ReflectSettings each frame so the demo is
        // self-contained and does not rely on external setup.
        configure_global_reflect_settings();

        // Collect container-structure events (insert/remove/reorder/rename)
        // while the reflected UI is rendered, using the new ReflectResponse
        // API as a lightweight analogue to ImReflect's ImResponse.
        let mut response = reflect::ReflectResponse::default();

        ui.window("Reflect Demo")
            .size([520.0, 640.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("dear-imgui-reflect: automatic UI from Rust types");
                ui.separator();

                if let Some(_node) = ui.tree_node("Primitives & Bool Styles") {
                    reflect::input_with_response(
                        ui,
                        "Primitives",
                        &mut state.primitives,
                        &mut response,
                    );
                }

                if let Some(_node) = ui.tree_node("Text & Containers") {
                    reflect::input_with_response(
                        ui,
                        "Text/Containers",
                        &mut state.text_and_containers,
                        &mut response,
                    );
                }

                if let Some(_node) = ui.tree_node("Maps & Tuples") {
                    reflect::input_with_response(
                        ui,
                        "Maps/Tuples",
                        &mut state.maps_and_tuples,
                        &mut response,
                    );
                }

                if let Some(_node) = ui.tree_node("Pointers") {
                    reflect::input_with_response(
                        ui,
                        "Pointers",
                        &mut state.pointers,
                        &mut response,
                    );
                }

                if let Some(_node) = ui.tree_node("Type-level Numerics") {
                    reflect::input_with_response(
                        ui,
                        "Type Defaults",
                        &mut state.type_level_numerics,
                        &mut response,
                    );
                }

                if let Some(_node) = ui.tree_node("Enums") {
                    ui.input_reflect("Quality", &mut state.quality);
                    ui.input_reflect("Render Mode", &mut state.mode);
                }

                if let Some(_node) = ui.tree_node("glam Vectors") {
                    ui.input_reflect("Glam", &mut state.glam);
                }

                if let Some(_node) = ui.tree_node("ReflectResponse Events") {
                    if response.is_empty() {
                        ui.text("No container-structure events this frame.");
                    } else {
                        ui.text("Container events this frame:");
                        for event in response.events() {
                            match event {
                                reflect::ReflectEvent::VecInserted { path, index } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "VecInserted at index {} (field: {})",
                                        index, path
                                    ));
                                }
                                reflect::ReflectEvent::VecRemoved { path, index } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "VecRemoved at index {} (field: {})",
                                        index, path
                                    ));
                                }
                                reflect::ReflectEvent::VecReordered { path, from, to } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "VecReordered {} -> {} (field: {})",
                                        from, to, path
                                    ));
                                }
                                reflect::ReflectEvent::ArrayReordered { path, from, to } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "ArrayReordered {} <-> {} (field: {})",
                                        from, to, path
                                    ));
                                }
                                reflect::ReflectEvent::MapInserted { path, key } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "MapInserted key \"{}\" (field: {})",
                                        key, path
                                    ));
                                }
                                reflect::ReflectEvent::MapRemoved { path, key } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "MapRemoved key \"{}\" (field: {})",
                                        key, path
                                    ));
                                }
                                reflect::ReflectEvent::MapRenamed { path, from, to } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "MapRenamed \"{}\" -> \"{}\" (field: {})",
                                        from, to, path
                                    ));
                                }
                                reflect::ReflectEvent::MapCleared { path, previous_len } => {
                                    let path = path.as_deref().unwrap_or("<unknown field>");
                                    ui.bullet_text(&format!(
                                        "MapCleared ({} entries removed) (field: {})",
                                        previous_len, path
                                    ));
                                }
                            }
                        }
                    }
                }

                ui.separator();
                ui.text(format!(
                    "Frame {:.3} ms ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });
    })
    .unwrap();
}
