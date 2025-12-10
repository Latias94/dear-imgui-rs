use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::Context;
use reflect::ImGuiReflect;

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(ImGuiReflect)]
enum Quality {
    Low,
    Medium,
    High,
}

impl Default for Quality {
    fn default() -> Self {
        Quality::Low
    }
}

#[derive(ImGuiReflect, Default)]
struct GameSettings {
    #[imgui(name = "Volume", slider, min = 0, max = 100)]
    volume: i32,
    sensitivity: f32,
    fullscreen: bool,
    #[imgui(multiline, read_only)]
    name: String,
}

#[derive(ImGuiReflect, Default)]
struct AdvancedSettings {
    vec2: [f32; 2],
    vec3: [f32; 3],
    vec4: [f32; 4],
    ivec2: [i32; 2],
    ivec3: [i32; 3],
    ivec4: [i32; 4],
    gain: Option<f32>,
    nested: Option<GameSettings>,
    mode: Quality,
    optional_mode: Option<Quality>,
    ints: Vec<i32>,
    #[imgui(multiline, lines = 3)]
    log: String,
}

/// Simple wrapper to exercise ImGuiReflect for boxed values.
#[derive(ImGuiReflect, Default)]
struct BoxedSettings {
    inner: Box<GameSettings>,
}

#[derive(ImGuiReflect, Default)]
struct SharedSettings {
    rc_inner: Rc<GameSettings>,
    arc_inner: Arc<GameSettings>,
}

/// Simple map-containing struct to exercise map ImGuiValue implementations.
#[derive(ImGuiReflect, Default)]
struct MapDemo {
    hash: HashMap<String, i32>,
    btree: BTreeMap<String, f32>,
}

/// Containers demo to exercise per-member Vec/Array/Map settings.
#[derive(ImGuiReflect, Default)]
struct ContainerSettingsDemo {
    /// Fully editable vector (insert/remove/reorder) using global VecSettings.
    full_vec: Vec<i32>,
    /// Vector that is reorderable only: no insert/remove buttons.
    reorder_only_vec: Vec<i32>,
    /// Fixed-size array with reordering disabled via member-level settings.
    fixed_no_reorder: [i32; 3],
    /// Map treated as a const-map via member-level MapSettings (no insert/remove).
    const_map: HashMap<String, i32>,
}

/// Complex nested containers to exercise deep graphs similar to ImReflect's
/// complex_object_test: nested Options, Vec/Map combinations, and tuples.
#[derive(ImGuiReflect, Default)]
struct ComplexContainerDemo {
    /// Primary game settings embedded directly.
    primary: GameSettings,
    /// Optional secondary settings boxed inside an Option.
    secondary: Option<Box<GameSettings>>,
    /// History of settings snapshots.
    history: Vec<GameSettings>,
    /// Map of string keys to integer vectors.
    map_of_vecs: HashMap<String, Vec<i32>>,
    /// Vector of maps, each representing a named float channel.
    vec_of_maps: Vec<HashMap<String, f32>>,
    /// Tuple combining a map and a vector to exercise tuple+container nesting.
    tuple_mix: (BTreeMap<String, f32>, Vec<i32>),
}

/// Tuple and pair-style fields to exercise tuple ImGuiValue implementations.
#[derive(ImGuiReflect, Default)]
struct TupleDemo {
    pair_int_float: (i32, f32),
    #[imgui(tuple_render = "grid", tuple_columns = 3)]
    triple_mixed: (bool, i32, f32),
    #[imgui(tuple_render = "line", tuple_dropdown)]
    quad_tuple: (i32, i32, i32, i32),
}

/// Larger tuples to exercise higher-arity tuple ImGuiValue implementations.
#[derive(ImGuiReflect, Default)]
struct LargeTupleDemo {
    five: (i32, i32, i32, i32, i32),
    six: (i32, i32, i32, i32, i32, i32),
    eight_mixed: (bool, i32, f32, i32, f32, i32, bool, f32),
}

/// Showcase of more advanced numeric and bool/text configuration.
#[derive(ImGuiReflect, Default)]
struct AdvancedNumericAndBool {
    /// Integer edited via InputScalar with step / fast-step.
    #[imgui(as_input, step = 1, step_fast = 10)]
    counter: i32,

    /// Float edited via a drag widget with speed/range/format and log+clamp flags.
    #[imgui(
        as_drag,
        speed = 0.1,
        min = 0.0,
        max = 10.0,
        format = "%.2f",
        log,
        always_clamp
    )]
    drag_value: f32,

    /// Integer slider with range and both manual clamp and always-clamp flags.
    #[imgui(slider, min = 0, max = 100, clamp, always_clamp, no_input)]
    slider_value: i32,

    /// Float slider with wrap-around behavior.
    #[imgui(slider, min = -1.0, max = 1.0, wrap_around, no_round_to_format)]
    wrap_slider: f32,

    /// Drag with clamp-on-input and no-speed-tweaks flags.
    #[imgui(
        as_drag,
        speed = 0.05,
        clamp_on_input,
        clamp_zero_range,
        no_speed_tweaks
    )]
    drag_with_flags: f32,

    /// Bool rendered as a dropdown combo.
    #[imgui(
        bool_style = "dropdown",
        true_text = "Enabled",
        false_text = "Disabled"
    )]
    toggle: bool,

    /// Single-line text with a minimum item width.
    #[imgui(min_width = 200.0)]
    wide_text: String,

    /// Multiline text with automatic height based on content.
    #[imgui(multiline, auto_resize)]
    auto_text: String,

    /// Single-line text with a hint placeholder.
    #[imgui(hint = "Type something...")]
    hint_text: String,

    /// Integer displayed in hexadecimal form.
    #[imgui(as_input, hex)]
    hex_counter: i32,

    /// Floating-point value displayed as a percentage.
    #[imgui(as_drag, percentage, speed = 0.5)]
    percent_value: f32,

    /// Floating-point value with prefix/suffix formatting.
    #[imgui(as_input, prefix = "v=", suffix = " ms")]
    timed_value: f32,

    /// Read-only display-only text, rendered without an input box.
    #[imgui(display_only)]
    status: String,
}

#[test]
fn derive_compiles_and_runs_basic_ui() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([1280.0, 720.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    // Build font atlas so widgets that rely on fonts do not assert.
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut settings = GameSettings {
        volume: 50,
        sensitivity: 1.0,
        fullscreen: true,
        name: "Player".to_owned(),
    };

    // This should compile and not panic. We do not assert on ImGui state here.
    let _changed = reflect::input(ui, "Settings", &mut settings);

    let mut advanced = AdvancedSettings::default();
    advanced.optional_mode = Some(Quality::Medium);
    let _changed2 = reflect::input(ui, "Advanced", &mut advanced);
}

#[test]
fn advanced_numeric_and_bool_styles_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([1024.0, 768.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut advanced = AdvancedNumericAndBool::default();
    let _changed = reflect::input(ui, "AdvancedNumericAndBool", &mut advanced);
}

#[test]
fn boxed_settings_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut boxed = BoxedSettings::default();
    let _changed = reflect::input(ui, "BoxedSettings", &mut boxed);
}

#[test]
fn shared_settings_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut shared = SharedSettings::default();
    let _changed = reflect::input(ui, "SharedSettings", &mut shared);
}

#[test]
fn map_demo_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut maps = MapDemo::default();
    let _changed = reflect::input(ui, "MapDemo", &mut maps);
}

#[test]
fn tuple_demo_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut tuples = TupleDemo::default();
    let _changed = reflect::input(ui, "TupleDemo", &mut tuples);
}

#[test]
fn large_tuple_and_member_read_only_no_panic() {
    let _guard = test_guard();

    // Configure member-level read-only both at field level and per-tuple-element.
    reflect::with_settings(|s| {
        // Entire field read-only (quad_tuple).
        s.for_member::<TupleDemo>("quad_tuple").read_only = true;
        // Per-element read-only on a tuple element: second element of triple_mixed.
        s.for_member::<TupleDemo>("triple_mixed[1]").read_only = true;
        // Per-element read-only on a larger tuple type.
        s.for_member::<LargeTupleDemo>("five[0]").read_only = true;
        s.for_member::<LargeTupleDemo>("eight_mixed[7]").read_only = true;
    });

    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut tuples = TupleDemo::default();
    let _ = reflect::input(ui, "TupleDemoWithReadOnly", &mut tuples);

    let mut large = LargeTupleDemo::default();
    let _ = reflect::input(ui, "LargeTupleDemo", &mut large);
}

#[test]
fn enum_reflect_no_panic() {
    let _guard = test_guard();
    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut q = Quality::Low;
    let _ = reflect::input(ui, "QualityEnum", &mut q);
}

#[test]
fn complex_container_graph_no_panic() {
    let _guard = test_guard();

    // Configure some member-level container settings to ensure nested settings
    // are exercised inside the complex graph.
    reflect::with_settings(|s| {
        // Make history reorderable-only: no insert/remove via vector buttons.
        s.for_member::<ComplexContainerDemo>("history")
            .vec_reorder_only();

        // Treat map_of_vecs as const at the map level (no insert/remove),
        // while still allowing edits within the inner vectors.
        s.for_member::<ComplexContainerDemo>("map_of_vecs")
            .maps_const();
    });

    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([1024.0, 768.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut demo = ComplexContainerDemo {
        secondary: Some(Box::new(GameSettings {
            volume: 75,
            sensitivity: 0.5,
            fullscreen: false,
            name: "Secondary".to_owned(),
        })),
        history: vec![
            GameSettings {
                volume: 30,
                sensitivity: 1.0,
                fullscreen: true,
                name: "Snapshot A".to_owned(),
            },
            GameSettings {
                volume: 60,
                sensitivity: 0.8,
                fullscreen: false,
                name: "Snapshot B".to_owned(),
            },
        ],
        map_of_vecs: HashMap::from([
            ("Group1".to_string(), vec![1, 2, 3]),
            ("Group2".to_string(), vec![4, 5]),
        ]),
        vec_of_maps: vec![
            HashMap::from([("x".to_string(), 0.1), ("y".to_string(), 0.2)]),
            HashMap::from([("z".to_string(), 0.3)]),
        ],
        tuple_mix: (
            BTreeMap::from([("alpha".to_string(), 1.0), ("beta".to_string(), 2.0)]),
            vec![10, 20, 30],
        ),
        ..Default::default()
    };

    let _ = reflect::input(ui, "ComplexContainerDemo", &mut demo);
}

#[test]
fn container_member_settings_no_panic() {
    let _guard = test_guard();

    // Configure member-level container settings to mimic ImSettings-style
    // insertable/removable/reorderable toggles.
    reflect::with_settings(|s| {
        // reorder_only_vec: disable insertion/removal, keep reordering enabled.
        s.for_member::<ContainerSettingsDemo>("reorder_only_vec")
            .vec_reorder_only();

        // fixed_no_reorder: disable array reordering while keeping dropdown.
        s.for_member::<ContainerSettingsDemo>("fixed_no_reorder")
            .arrays_fixed_order();

        // const_map: disable insertion/removal, still render dropdown and use
        // table layout to emphasize MapSettings.
        s.for_member::<ContainerSettingsDemo>("const_map")
            .maps_const();
    });

    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let ui = ctx.frame();

    let mut demo = ContainerSettingsDemo {
        reorder_only_vec: vec![1, 2, 3],
        fixed_no_reorder: [10, 20, 30],
        const_map: HashMap::from([("A".to_string(), 1), ("B".to_string(), 2)]),
        ..Default::default()
    };

    let _ = reflect::input(ui, "ContainerSettingsDemo", &mut demo);
}
