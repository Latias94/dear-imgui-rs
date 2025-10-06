//! ImPlot3D Demo - Complete Rust port of official implot3d_demo.cpp
//!
//! This example is a faithful reproduction of the official C++ ImPlot3D demo,
//! showcasing all major features of the library.

use dear_app::{AddOnsConfig, RunnerConfig, run};
use dear_imgui_rs::*;
use dear_implot3d as implot3d;
use implot3d::plots::*;
use implot3d::*;
use std::f32::consts::PI;

// ImPlot3DStyleVar constants (from cimplot3d.h)
const IMPLOT3D_STYLEVAR_LINE_WEIGHT: i32 = 0;
const IMPLOT3D_STYLEVAR_MARKER_SIZE: i32 = 2;
const IMPLOT3D_STYLEVAR_MARKER_WEIGHT: i32 = 3;
const IMPLOT3D_STYLEVAR_FILL_ALPHA: i32 = 4;

fn main() {
    dear_imgui_rs::logging::init_tracing_with_filter(
        "dear_imgui=info,implot3d_basic=info,wgpu=warn",
    );

    let runner = RunnerConfig {
        window_title: "ImPlot3D Demo (Rust)".to_string(),
        window_size: (1280.0, 800.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.06, 0.08, 0.1, 1.0],
        docking: Default::default(),
        ini_filename: None,
        restore_previous_geometry: true,
        redraw: dear_app::RedrawMode::Poll,
        io_config_flags: None,
        ..Default::default()
    };

    let addons = AddOnsConfig::auto();

    run(runner, addons, |ui, addons| {
        let Some(plot_ctx) = addons.implot3d else {
            ui.text("ImPlot3D add-on not enabled");
            return;
        };
        let plot_ui = plot_ctx.get_plot_ui(ui);

        ui.window("ImPlot3D Demo")
            .size([600.0, 750.0], Condition::FirstUseEver)
            .position([100.0, 100.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("ImPlot3D says olá! (0.3 WIP)"));
                ui.spacing();

                if let Some(tab_bar) = ui.tab_bar("ImPlot3DDemoTabs") {
                    // Plots Tab
                    if let Some(_tab) = ui.tab_item("Plots") {
                        demo_header(ui, "Line Plots", || demo_line_plots(ui, &plot_ui));
                        demo_header(ui, "Scatter Plots", || demo_scatter_plots(ui, &plot_ui));
                        demo_header(ui, "Triangle Plots", || demo_triangle_plots(ui, &plot_ui));
                        demo_header(ui, "Quad Plots", || demo_quad_plots(ui, &plot_ui));
                        demo_header(ui, "Surface Plots", || demo_surface_plots(ui, &plot_ui));
                        demo_header(ui, "Mesh Plots", || demo_mesh_plots(ui, &plot_ui));
                        demo_header(ui, "Realtime Plots", || demo_realtime_plots(ui, &plot_ui));
                        demo_header(ui, "Markers and Text", || {
                            demo_markers_and_text(ui, &plot_ui)
                        });
                        demo_header(ui, "NaN Values", || demo_nan_values(ui, &plot_ui));
                    }

                    // Axes Tab
                    if let Some(_tab) = ui.tab_item("Axes") {
                        demo_header(ui, "Box Scale", || demo_box_scale(ui, &plot_ui));
                        demo_header(ui, "Box Rotation", || demo_box_rotation(ui, &plot_ui));
                        demo_header(ui, "Tick Labels", || demo_tick_labels(ui, &plot_ui));
                        demo_header(ui, "Axis Constraints", || {
                            demo_axis_constraints(ui, &plot_ui)
                        });
                    }

                    // Custom Tab
                    if let Some(_tab) = ui.tab_item("Custom") {
                        demo_header(ui, "Custom Styles", || demo_custom_styles(ui, &plot_ui));
                        demo_header(ui, "Custom Rendering", || {
                            demo_custom_rendering(ui, &plot_ui)
                        });
                    }

                    // Help Tab
                    if let Some(_tab) = ui.tab_item("Help") {
                        demo_help(ui);
                    }

                    drop(tab_bar);
                }
            });
    })
    .unwrap();
}

// Helper function to create collapsible demo sections
fn demo_header<F: FnOnce()>(ui: &Ui, label: &str, demo: F) {
    if ui.collapsing_header(label, TreeNodeFlags::NONE) {
        demo();
    }
}

// ============================================================================
// Demo Functions (Complete port from C++ implot3d_demo.cpp)
// ============================================================================

fn demo_line_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    // Animated line plot
    let time = ui.time() as f32;
    let mut xs1 = vec![0.0f32; 1001];
    let mut ys1 = vec![0.0f32; 1001];
    let mut zs1 = vec![0.0f32; 1001];
    for i in 0..1001 {
        xs1[i] = i as f32 * 0.001;
        ys1[i] = 0.5 + 0.5 * (50.0 * (xs1[i] + time / 10.0)).cos();
        zs1[i] = 0.5 + 0.5 * (50.0 * (xs1[i] + time / 10.0)).sin();
    }

    let mut xs2 = vec![0.0f64; 20];
    let mut ys2 = vec![0.0f64; 20];
    let mut zs2 = vec![0.0f64; 20];
    for i in 0..20 {
        xs2[i] = i as f64 / 19.0;
        ys2[i] = xs2[i] * xs2[i];
        zs2[i] = xs2[i] * ys2[i];
    }

    if let Some(_tok) = plot_ui.begin_plot("Line Plots").build() {
        plot_ui.setup_axes(
            "x",
            "y",
            "z",
            Axis3DFlags::NONE,
            Axis3DFlags::NONE,
            Axis3DFlags::NONE,
        );
        Line3D::f32("f(x)", &xs1, &ys1, &zs1).plot(plot_ui);
        set_next_marker_style(
            Marker3D::Circle,
            4.0,
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            [1.0, 1.0, 1.0, 1.0],
        );
        Line3D::f64("g(x)", &xs2, &ys2, &zs2)
            .flags(Line3DFlags::SEGMENTS)
            .plot(plot_ui);
    }
}

fn demo_scatter_plots(_ui: &Ui, plot_ui: &Plot3DUi) {
    // Simple pseudo-random using sine functions
    let mut xs1 = vec![0.0f32; 100];
    let mut ys1 = vec![0.0f32; 100];
    let mut zs1 = vec![0.0f32; 100];
    for i in 0..100 {
        xs1[i] = i as f32 * 0.01;
        ys1[i] = xs1[i] + 0.1 * ((i as f32 * 12.9898).sin() * 43758.5453).fract();
        zs1[i] = xs1[i] + 0.1 * ((i as f32 * 78.233).sin() * 43758.5453).fract();
    }

    let mut xs2 = vec![0.0f32; 50];
    let mut ys2 = vec![0.0f32; 50];
    let mut zs2 = vec![0.0f32; 50];
    for i in 0..50 {
        xs2[i] = 0.25 + 0.2 * ((i as f32 * 12.9898).sin() * 43758.5453).fract();
        ys2[i] = 0.50 + 0.2 * ((i as f32 * 78.233).sin() * 43758.5453).fract();
        zs2[i] = 0.75 + 0.2 * ((i as f32 * 45.164).sin() * 43758.5453).fract();
    }

    if let Some(_tok) = plot_ui.begin_plot("Scatter Plots").build() {
        Scatter3D::f32("Data 1", &xs1, &ys1, &zs1).plot(plot_ui);
        push_style_var_f32(IMPLOT3D_STYLEVAR_FILL_ALPHA, 0.25);
        let col1 = get_colormap_color(1);
        set_next_marker_style(Marker3D::Square, 6.0, col1, -1.0, col1);
        Scatter3D::f32("Data 2", &xs2, &ys2, &zs2).plot(plot_ui);
        pop_style_var(1);
    }
}

fn demo_triangle_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    // Pyramid coordinates
    let ax = 0.0f32;
    let ay = 0.0f32;
    let az = 1.0f32;
    let cx = [-0.5f32, 0.5, 0.5, -0.5];
    let cy = [-0.5f32, -0.5, 0.5, 0.5];
    let cz = [0.0f32, 0.0, 0.0, 0.0];

    let mut xs = vec![0.0f32; 18];
    let mut ys = vec![0.0f32; 18];
    let mut zs = vec![0.0f32; 18];
    let mut i = 0;

    // Helper to add vertex
    let mut add_vertex = |x: f32, y: f32, z: f32| {
        xs[i] = x;
        ys[i] = y;
        zs[i] = z;
        i += 1;
    };

    // 4 side triangles
    for j in 0..4 {
        add_vertex(ax, ay, az);
        add_vertex(cx[j], cy[j], cz[j]);
        add_vertex(cx[(j + 1) % 4], cy[(j + 1) % 4], cz[(j + 1) % 4]);
    }

    // 2 base triangles
    add_vertex(cx[0], cy[0], cz[0]);
    add_vertex(cx[1], cy[1], cz[1]);
    add_vertex(cx[2], cy[2], cz[2]);

    add_vertex(cx[0], cy[0], cz[0]);
    add_vertex(cx[2], cy[2], cz[2]);
    add_vertex(cx[3], cy[3], cz[3]);

    // Triangle flags
    static mut FLAGS: Triangle3DFlags = Triangle3DFlags::NONE;
    unsafe {
        ui.checkbox_flags("NoLines", &mut FLAGS, Triangle3DFlags::NO_LINES);
        ui.checkbox_flags("NoFill", &mut FLAGS, Triangle3DFlags::NO_FILL);
        ui.checkbox_flags("NoMarkers", &mut FLAGS, Triangle3DFlags::NO_MARKERS);
    }

    if let Some(_tok) = plot_ui.begin_plot("Triangle Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -0.5, 1.5, Plot3DCond::Once);
        set_next_fill_style(get_colormap_color(0), 1.0);
        set_next_line_style(get_colormap_color(1), 2.0);
        let col2 = get_colormap_color(2);
        set_next_marker_style(Marker3D::Square, 3.0, col2, -1.0, col2);
        unsafe {
            Triangles3D::f32("Pyramid", &xs, &ys, &zs)
                .flags(FLAGS)
                .plot(plot_ui);
        }
    }
}

fn demo_quad_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    let mut xs = vec![0.0f32; 6 * 4];
    let mut ys = vec![0.0f32; 6 * 4];
    let mut zs = vec![0.0f32; 6 * 4];

    // Cube faces (+x, -x, +y, -y, +z, -z)
    let faces = [
        // +x face
        (
            [1.0, 1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0, 1.0],
        ),
        // -x face
        (
            [-1.0, -1.0, -1.0, -1.0],
            [-1.0, 1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0, 1.0],
        ),
        // +y face
        (
            [-1.0, 1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0, 1.0],
        ),
        // -y face
        (
            [-1.0, 1.0, 1.0, -1.0],
            [-1.0, -1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0, 1.0],
        ),
        // +z face
        (
            [-1.0, 1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        ),
        // -z face
        (
            [-1.0, 1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0, 1.0],
            [-1.0, -1.0, -1.0, -1.0],
        ),
    ];

    for (face_idx, (fx, fy, fz)) in faces.iter().enumerate() {
        for v in 0..4 {
            let idx = face_idx * 4 + v;
            xs[idx] = fx[v];
            ys[idx] = fy[v];
            zs[idx] = fz[v];
        }
    }

    // Quad flags
    static mut FLAGS: Quad3DFlags = Quad3DFlags::NONE;
    unsafe {
        ui.checkbox_flags("NoLines", &mut FLAGS, Quad3DFlags::NO_LINES);
        ui.checkbox_flags("NoFill", &mut FLAGS, Quad3DFlags::NO_FILL);
        ui.checkbox_flags("NoMarkers", &mut FLAGS, Quad3DFlags::NO_MARKERS);
    }

    if let Some(_tok) = plot_ui.begin_plot("Quad Plots").build() {
        plot_ui.setup_axes_limits(-1.5, 1.5, -1.5, 1.5, -1.5, 1.5, Plot3DCond::Once);

        let color_x = [0.8, 0.2, 0.2, 0.8];
        let color_y = [0.2, 0.8, 0.2, 0.8];
        let color_z = [0.2, 0.2, 0.8, 0.8];

        set_next_fill_style(color_x, 1.0);
        set_next_line_style(color_x, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_x, -1.0, color_x);
        unsafe {
            Quads3D::f32("X", &xs[0..8], &ys[0..8], &zs[0..8])
                .flags(FLAGS)
                .plot(plot_ui);
        }

        set_next_fill_style(color_y, 1.0);
        set_next_line_style(color_y, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_y, -1.0, color_y);
        unsafe {
            Quads3D::f32("Y", &xs[8..16], &ys[8..16], &zs[8..16])
                .flags(FLAGS)
                .plot(plot_ui);
        }

        set_next_fill_style(color_z, 1.0);
        set_next_line_style(color_z, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_z, -1.0, color_z);
        unsafe {
            Quads3D::f32("Z", &xs[16..24], &ys[16..24], &zs[16..24])
                .flags(FLAGS)
                .plot(plot_ui);
        }
    }
}

fn demo_surface_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    const N: usize = 20;
    static mut T: f32 = 0.0;

    unsafe {
        T += ui.io().delta_time();
    }

    let mut xs = vec![0.0f32; N * N];
    let mut ys = vec![0.0f32; N * N];
    let mut zs = vec![0.0f32; N * N];

    const MIN_VAL: f32 = -1.0;
    const MAX_VAL: f32 = 1.0;
    const STEP: f32 = (MAX_VAL - MIN_VAL) / (N as f32 - 1.0);

    for i in 0..N {
        for j in 0..N {
            let idx = i * N + j;
            xs[idx] = MIN_VAL + j as f32 * STEP;
            ys[idx] = MIN_VAL + i as f32 * STEP;
            let r = (xs[idx] * xs[idx] + ys[idx] * ys[idx]).sqrt();
            unsafe {
                zs[idx] = (2.0 * T + r).sin();
            }
        }
    }

    // Choose fill color
    ui.text("Fill color");
    static mut SELECTED_FILL: i32 = 1; // Colormap by default
    static mut SOLID_COLOR: [f32; 4] = [0.8, 0.8, 0.2, 0.6];
    static mut SEL_COLORMAP: i32 = 5; // Jet by default

    unsafe {
        ui.indent();

        // Choose solid color
        if ui.radio_button("Solid", SELECTED_FILL == 0) {
            SELECTED_FILL = 0;
        }
        if SELECTED_FILL == 0 {
            ui.same_line();
            ui.color_edit4_config("##SurfaceSolidColor", &mut SOLID_COLOR)
                .build();
        }

        // Choose colormap
        if ui.radio_button("Colormap", SELECTED_FILL == 1) {
            SELECTED_FILL = 1;
        }
        if SELECTED_FILL == 1 {
            ui.same_line();
            let colormaps = [
                "Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet", "Twilight", "RdBu", "BrBG",
                "PiYG", "Spectral", "Greys",
            ];
            if let Some(_combo) =
                ui.begin_combo("##SurfaceColormap", colormaps[SEL_COLORMAP as usize])
            {
                for (i, name) in colormaps.iter().enumerate() {
                    if ui.selectable(name) {
                        SEL_COLORMAP = i as i32;
                    }
                }
            }
        }
        ui.unindent();
    }

    // Choose range
    static mut CUSTOM_RANGE: bool = false;
    static mut RANGE_MIN: f32 = -1.0;
    static mut RANGE_MAX: f32 = 1.0;

    unsafe {
        ui.checkbox("Custom range", &mut CUSTOM_RANGE);
        ui.indent();

        let _disabled = if !CUSTOM_RANGE {
            Some(ui.begin_disabled())
        } else {
            None
        };
        ui.slider_config("Range min", -1.0, RANGE_MAX - 0.01)
            .build(&mut RANGE_MIN);
        ui.slider_config("Range max", RANGE_MIN + 0.01, 1.0)
            .build(&mut RANGE_MAX);
        drop(_disabled);

        ui.unindent();
    }

    // Select flags
    static mut FLAGS: Surface3DFlags = Surface3DFlags::NO_MARKERS;
    unsafe {
        ui.checkbox_flags("NoLines", &mut FLAGS, Surface3DFlags::NO_LINES);
        ui.checkbox_flags("NoFill", &mut FLAGS, Surface3DFlags::NO_FILL);
        ui.checkbox_flags("NoMarkers", &mut FLAGS, Surface3DFlags::NO_MARKERS);
    }

    // Begin the plot
    unsafe {
        if SELECTED_FILL == 1 {
            let colormaps = [
                "Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet", "Twilight", "RdBu", "BrBG",
                "PiYG", "Spectral", "Greys",
            ];
            push_colormap_name(colormaps[SEL_COLORMAP as usize]);
        }
    }

    if let Some(_tok) = plot_ui
        .begin_plot("Surface Plots")
        .size([-1.0, 400.0])
        .flags(Plot3DFlags::NO_CLIP)
        .build()
    {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.5, 1.5, Plot3DCond::Once);

        // Set fill style
        push_style_var_f32(IMPLOT3D_STYLEVAR_FILL_ALPHA, 0.8);
        unsafe {
            if SELECTED_FILL == 0 {
                set_next_fill_style(SOLID_COLOR, 1.0);
            }
        }

        // Set line style
        set_next_line_style(get_colormap_color(1), 1.0);

        // Set marker style
        set_next_marker_style(
            Marker3D::Square,
            -1.0,
            get_colormap_color(2),
            -1.0,
            get_colormap_color(2),
        );

        let x_grid: Vec<f32> = (0..N).map(|j| MIN_VAL + j as f32 * STEP).collect();
        let y_grid: Vec<f32> = (0..N).map(|i| MIN_VAL + i as f32 * STEP).collect();

        unsafe {
            if CUSTOM_RANGE {
                Surface3D::new("Wave Surface", &x_grid, &y_grid, &zs)
                    .scale(RANGE_MIN as f64, RANGE_MAX as f64)
                    .flags(FLAGS)
                    .plot(plot_ui);
            } else {
                Surface3D::new("Wave Surface", &x_grid, &y_grid, &zs)
                    .scale(0.0, 0.0)
                    .flags(FLAGS)
                    .plot(plot_ui);
            }
        }

        pop_style_var(1);
    }

    unsafe {
        if SELECTED_FILL == 1 {
            pop_colormap(1);
        }
    }
}

fn demo_mesh_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    // Simple tetrahedron
    let vertices: [[f32; 3]; 4] = [
        [0.0, 0.0, 0.8],
        [0.8, 0.0, -0.2],
        [0.0, 0.8, -0.2],
        [-0.8, -0.8, -0.2],
    ];
    let indices: [u32; 12] = [0, 1, 2, 0, 1, 3, 0, 2, 3, 1, 2, 3];

    // Mesh flags
    static mut FLAGS: Mesh3DFlags = Mesh3DFlags::NONE;
    unsafe {
        ui.checkbox_flags("NoLines", &mut FLAGS, Mesh3DFlags::NO_LINES);
        ui.checkbox_flags("NoFill", &mut FLAGS, Mesh3DFlags::NO_FILL);
        ui.checkbox_flags("NoMarkers", &mut FLAGS, Mesh3DFlags::NO_MARKERS);
    }

    if let Some(_tok) = plot_ui.begin_plot("Mesh Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Once);
        set_next_fill_style([0.8, 0.8, 0.2, 0.6], 1.0);
        set_next_line_style([0.5, 0.5, 0.2, 0.6], 1.0);
        let marker_col = [0.5, 0.5, 0.2, 0.6];
        set_next_marker_style(Marker3D::Square, 3.0, marker_col, -1.0, marker_col);
        unsafe {
            Mesh3D::new("Tetrahedron", &vertices, &indices)
                .flags(FLAGS)
                .plot(plot_ui);
        }
    }
}

// ScrollingBuffer helper for realtime plots
struct ScrollingBuffer {
    max_size: usize,
    offset: usize,
    data: Vec<[f32; 2]>,
}

impl ScrollingBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            offset: 0,
            data: Vec::new(),
        }
    }

    fn add_point(&mut self, x: f32, y: f32) {
        if self.data.len() < self.max_size {
            self.data.push([x, y]);
        } else {
            self.data[self.offset] = [x, y];
            self.offset = (self.offset + 1) % self.max_size;
        }
    }

    fn erase(&mut self) {
        if !self.data.is_empty() {
            self.data.clear();
            self.offset = 0;
        }
    }

    fn get_data(&self) -> (Vec<f32>, Vec<f32>) {
        let mut xs = Vec::with_capacity(self.data.len());
        let mut ys = Vec::with_capacity(self.data.len());

        for i in 0..self.data.len() {
            let idx = (self.offset + i) % self.data.len();
            xs.push(self.data[idx][0]);
            ys.push(self.data[idx][1]);
        }

        (xs, ys)
    }
}

fn demo_realtime_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    static mut T: f32 = 0.0;
    static mut HISTORY: f32 = 10.0;
    static mut SDATA1: Option<ScrollingBuffer> = None;
    static mut SDATA2: Option<ScrollingBuffer> = None;
    static mut SDATA3: Option<ScrollingBuffer> = None;

    unsafe {
        if SDATA1.is_none() {
            SDATA1 = Some(ScrollingBuffer::new(1000));
            SDATA2 = Some(ScrollingBuffer::new(1000));
            SDATA3 = Some(ScrollingBuffer::new(1000));
        }

        T += ui.io().delta_time();

        let sdata1 = SDATA1.as_mut().unwrap();
        let sdata2 = SDATA2.as_mut().unwrap();
        let sdata3 = SDATA3.as_mut().unwrap();

        sdata1.add_point(T, (2.0 * T).sin());
        sdata2.add_point(T, (2.0 * T).cos());
        sdata3.add_point(T, (2.0 * T + PI / 2.0).sin() * (2.0 * T + PI / 2.0).cos());

        ui.slider_config("History", 1.0, 30.0).build(&mut HISTORY);
        ui.same_line();
        if ui.button("Reset") {
            sdata1.erase();
            sdata2.erase();
            sdata3.erase();
            T = 0.0;
        }

        if let Some(_tok) = plot_ui
            .begin_plot("Realtime Plots")
            .size([-1.0, 400.0])
            .build()
        {
            plot_ui.setup_axes_limits(
                (T - HISTORY) as f64,
                T as f64,
                -1.0,
                1.0,
                -1.0,
                1.0,
                Plot3DCond::Always,
            );

            let (xs1, ys1) = sdata1.get_data();
            let (xs2, ys2) = sdata2.get_data();
            let (xs3, zs3) = sdata3.get_data();

            if !xs1.is_empty() {
                Line3D::f32("sin(2t)", &xs1, &ys1, &vec![0.0; xs1.len()]).plot(plot_ui);
            }
            if !xs2.is_empty() {
                Line3D::f32("cos(2t)", &xs2, &vec![0.0; xs2.len()], &ys2).plot(plot_ui);
            }
            if !xs3.is_empty() {
                Line3D::f32("sin*cos", &xs3, &vec![0.0; xs3.len()], &zs3).plot(plot_ui);
            }
        }
    }
}

fn demo_box_scale(ui: &Ui, plot_ui: &Plot3DUi) {
    const N: usize = 100;
    let mut xs = vec![0.0f32; N];
    let mut ys = vec![0.0f32; N];
    let mut zs = vec![0.0f32; N];

    for i in 0..N {
        let t = i as f32 / (N as f32 - 1.0);
        xs[i] = (t * 2.0 * PI).sin();
        ys[i] = (t * 4.0 * PI).cos();
        zs[i] = t * 2.0 - 1.0;
    }

    static mut SCALE: [f32; 3] = [1.0, 1.0, 1.0];
    unsafe {
        ui.slider_config("Box Scale", 0.1, 2.0)
            .build_array(&mut SCALE);
    }

    if let Some(_tok) = plot_ui.begin_plot("##BoxScale").build() {
        unsafe {
            plot_ui.setup_box_scale(SCALE[0], SCALE[1], SCALE[2]);
        }
        Line3D::f32("3D Curve", &xs, &ys, &zs).plot(plot_ui);
    }
}

fn demo_box_rotation(ui: &Ui, plot_ui: &Plot3DUi) {
    static mut ELEVATION: f32 = 45.0;
    static mut AZIMUTH: f32 = -135.0;
    static mut ANIMATE: bool = false;
    static mut INIT_ELEVATION: f32 = 45.0;
    static mut INIT_AZIMUTH: f32 = -135.0;

    unsafe {
        ui.text("Rotation");
        let mut changed = false;
        changed |= ui
            .slider_config("Elevation", -90.0, 90.0)
            .build(&mut ELEVATION);
        changed |= ui
            .slider_config("Azimuth", -180.0, 180.0)
            .build(&mut AZIMUTH);
        ui.checkbox("Animate", &mut ANIMATE);

        ui.text("Initial Rotation");
        ui.slider_config("Initial Elevation", -90.0, 90.0)
            .build(&mut INIT_ELEVATION);
        ui.slider_config("Initial Azimuth", -180.0, 180.0)
            .build(&mut INIT_AZIMUTH);

        if let Some(_tok) = plot_ui.begin_plot("##BoxRotation").build() {
            plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Always);
            plot_ui.setup_box_initial_rotation(INIT_ELEVATION, INIT_AZIMUTH);
            if changed {
                plot_ui.setup_box_rotation(ELEVATION, AZIMUTH, ANIMATE, Plot3DCond::Always);
            }

            // Plot axis lines
            let origin = [0.0f32, 0.0];
            let axis = [0.0f32, 1.0];
            set_next_line_style([0.8, 0.2, 0.2, 1.0], 1.0);
            Line3D::f32("X-Axis", &axis, &origin, &origin).plot(plot_ui);
            set_next_line_style([0.2, 0.8, 0.2, 1.0], 1.0);
            Line3D::f32("Y-Axis", &origin, &axis, &origin).plot(plot_ui);
            set_next_line_style([0.2, 0.2, 0.8, 1.0], 1.0);
            Line3D::f32("Z-Axis", &origin, &origin, &axis).plot(plot_ui);
        }
    }
}

fn demo_tick_labels(_ui: &Ui, plot_ui: &Plot3DUi) {
    let xs = [0.0f64, 1.0, 2.0];
    let ys = [0.0f64, 1.0, 2.0];
    let zs = [0.0f64, 1.0, 2.0];

    if let Some(_tok) = plot_ui.begin_plot("Tick Labels").build() {
        plot_ui.setup_axes_limits(-0.5, 2.5, -0.5, 2.5, -0.5, 2.5, Plot3DCond::Once);

        // Custom tick labels
        let x_labels = ["Low", "Mid", "High"];
        let y_labels = ["A", "B", "C"];
        let z_labels = ["Min", "Med", "Max"];

        plot_ui.setup_axis_ticks_values(Axis3D::X, &xs, Some(&x_labels), false);
        plot_ui.setup_axis_ticks_values(Axis3D::Y, &ys, Some(&y_labels), false);
        plot_ui.setup_axis_ticks_values(Axis3D::Z, &zs, Some(&z_labels), false);

        let xs_f32 = [0.0f32, 1.0, 2.0];
        let ys_f32 = [0.0f32, 1.0, 2.0];
        let zs_f32 = [0.0f32, 1.0, 2.0];
        Scatter3D::f32("Points", &xs_f32, &ys_f32, &zs_f32).plot(plot_ui);
    }
}

fn demo_axis_constraints(ui: &Ui, plot_ui: &Plot3DUi) {
    static mut ENABLE_LIMITS: bool = false;
    static mut ENABLE_ZOOM: bool = false;

    unsafe {
        ui.checkbox("Enable Limits Constraints", &mut ENABLE_LIMITS);
        ui.checkbox("Enable Zoom Constraints", &mut ENABLE_ZOOM);
    }

    let xs = [0.0f32, 1.0, 2.0];
    let ys = [0.0f32, 1.0, 2.0];
    let zs = [0.0f32, 1.0, 2.0];

    if let Some(_tok) = plot_ui.begin_plot("Axis Constraints").build() {
        plot_ui.setup_axes_limits(-1.0, 3.0, -1.0, 3.0, -1.0, 3.0, Plot3DCond::Once);

        unsafe {
            if ENABLE_LIMITS {
                plot_ui.setup_axis_limits_constraints(Axis3D::X, -0.5, 2.5);
                plot_ui.setup_axis_limits_constraints(Axis3D::Y, -0.5, 2.5);
                plot_ui.setup_axis_limits_constraints(Axis3D::Z, -0.5, 2.5);
            }

            if ENABLE_ZOOM {
                plot_ui.setup_axis_zoom_constraints(Axis3D::X, 0.5, 5.0);
                plot_ui.setup_axis_zoom_constraints(Axis3D::Y, 0.5, 5.0);
                plot_ui.setup_axis_zoom_constraints(Axis3D::Z, 0.5, 5.0);
            }
        }

        Scatter3D::f32("Points", &xs, &ys, &zs).plot(plot_ui);
    }
}

fn demo_markers_and_text(_ui: &Ui, plot_ui: &Plot3DUi) {
    let xs = [0.0f32, 1.0, 2.0, 3.0];
    let ys = [0.0f32, 1.0, 2.0, 3.0];
    let zs = [0.0f32, 1.0, 2.0, 3.0];

    if let Some(_tok) = plot_ui.begin_plot("Markers & Text").build() {
        plot_ui.setup_axes_limits(-0.5, 3.5, -0.5, 3.5, -0.5, 3.5, Plot3DCond::Once);

        // Different marker styles
        let markers = [
            Marker3D::Circle,
            Marker3D::Square,
            Marker3D::Diamond,
            Marker3D::Up,
        ];

        for (i, &marker) in markers.iter().enumerate() {
            let col = get_colormap_color(i as i32);
            set_next_marker_style(marker, 8.0, col, 2.0, col);
            Scatter3D::f32(&format!("Marker {}", i), &[xs[i]], &[ys[i]], &[zs[i]]).plot(plot_ui);

            // Add text label
            plot_ui.plot_text(&format!("P{}", i), xs[i], ys[i], zs[i], 0.0, [10.0, 10.0]);
        }
    }
}

fn demo_nan_values(_ui: &Ui, plot_ui: &Plot3DUi) {
    let xs = [0.0f32, 1.0, 2.0, f32::NAN, 4.0, 5.0];
    let ys = [0.0f32, 1.0, f32::NAN, 3.0, 4.0, 5.0];
    let zs = [0.0f32, f32::NAN, 2.0, 3.0, 4.0, 5.0];

    if let Some(_tok) = plot_ui.begin_plot("NaN Values").build() {
        plot_ui.setup_axes_limits(-0.5, 5.5, -0.5, 5.5, -0.5, 5.5, Plot3DCond::Once);

        // NaN values are automatically skipped
        Line3D::f32("Line with NaN", &xs, &ys, &zs).plot(plot_ui);
        set_next_marker_style(
            Marker3D::Circle,
            6.0,
            [1.0, 0.0, 0.0, 1.0],
            -1.0,
            [1.0, 0.0, 0.0, 1.0],
        );
        Scatter3D::f32("Scatter with NaN", &xs, &ys, &zs).plot(plot_ui);
    }
}

fn demo_custom_styles(ui: &Ui, plot_ui: &Plot3DUi) {
    ui.text("Modify the style of plots using ImPlot3D style variables.");
    ui.spacing();

    // Style variables
    static mut LINE_WEIGHT: f32 = 2.0;
    static mut MARKER_SIZE: f32 = 5.0;
    static mut MARKER_WEIGHT: f32 = 1.0;
    static mut FILL_ALPHA: f32 = 0.5;

    unsafe {
        ui.slider_config("LineWeight", 0.5, 5.0)
            .build(&mut LINE_WEIGHT);
        ui.slider_config("MarkerSize", 2.0, 10.0)
            .build(&mut MARKER_SIZE);
        ui.slider_config("MarkerWeight", 0.5, 3.0)
            .build(&mut MARKER_WEIGHT);
        ui.slider_config("FillAlpha", 0.0, 1.0)
            .build(&mut FILL_ALPHA);
    }

    // Generate data
    let mut xs = vec![0.0f32; 100];
    let mut ys = vec![0.0f32; 100];
    let mut zs = vec![0.0f32; 100];
    for i in 0..100 {
        let t = i as f32 / 99.0;
        xs[i] = (t * 2.0 * PI).sin();
        ys[i] = (t * 2.0 * PI).cos();
        zs[i] = t * 2.0 - 1.0;
    }

    if let Some(_tok) = plot_ui.begin_plot("Custom Styles").build() {
        plot_ui.setup_axes_limits(-1.5, 1.5, -1.5, 1.5, -1.5, 1.5, Plot3DCond::Once);

        unsafe {
            push_style_var_f32(IMPLOT3D_STYLEVAR_LINE_WEIGHT, LINE_WEIGHT);
            push_style_var_f32(IMPLOT3D_STYLEVAR_MARKER_SIZE, MARKER_SIZE);
            push_style_var_f32(IMPLOT3D_STYLEVAR_MARKER_WEIGHT, MARKER_WEIGHT);
            push_style_var_f32(IMPLOT3D_STYLEVAR_FILL_ALPHA, FILL_ALPHA);
        }

        set_next_marker_style(
            Marker3D::Circle,
            -1.0,
            get_colormap_color(0),
            -1.0,
            get_colormap_color(0),
        );
        Line3D::f32("Styled Line", &xs, &ys, &zs).plot(plot_ui);

        pop_style_var(4);
    }
}

fn demo_custom_rendering(ui: &Ui, plot_ui: &Plot3DUi) {
    ui.text("Use custom rendering to draw additional elements in the plot.");
    ui.text("This demo shows how to use PlotToPixels for custom drawing.");
    ui.spacing();

    // Generate a simple helix
    let mut xs = vec![0.0f32; 100];
    let mut ys = vec![0.0f32; 100];
    let mut zs = vec![0.0f32; 100];
    for i in 0..100 {
        let t = i as f32 / 99.0 * 4.0 * PI;
        xs[i] = t.cos();
        ys[i] = t.sin();
        zs[i] = t / (4.0 * PI) * 2.0 - 1.0;
    }

    if let Some(_tok) = plot_ui.begin_plot("Custom Rendering").build() {
        plot_ui.setup_axes_limits(-1.5, 1.5, -1.5, 1.5, -1.5, 1.5, Plot3DCond::Once);

        // Draw the helix
        Line3D::f32("Helix", &xs, &ys, &zs).plot(plot_ui);

        // Custom rendering: highlight start and end points
        let draw_list = ui.get_window_draw_list();

        // Convert 3D points to 2D screen coordinates
        let start_pixel = plot_ui.plot_to_pixels([xs[0], ys[0], zs[0]]);
        let end_pixel = plot_ui.plot_to_pixels([xs[99], ys[99], zs[99]]);

        // Draw circles at start and end
        draw_list
            .add_circle([start_pixel[0], start_pixel[1]], 10.0, [0.0, 1.0, 0.0, 1.0])
            .filled(true)
            .build();

        draw_list
            .add_circle([end_pixel[0], end_pixel[1]], 10.0, [1.0, 0.0, 0.0, 1.0])
            .filled(true)
            .build();

        // Add text labels
        draw_list.add_text(
            [start_pixel[0] + 15.0, start_pixel[1]],
            [0.0, 1.0, 0.0, 1.0],
            "Start",
        );
        draw_list.add_text(
            [end_pixel[0] + 15.0, end_pixel[1]],
            [1.0, 0.0, 0.0, 1.0],
            "End",
        );
    }
}

fn demo_help(ui: &Ui) {
    ui.text("ABOUT THIS DEMO:");
    ui.bullet_text("This is a complete port of the official ImPlot3D C++ demo.");
    ui.bullet_text("It showcases all major features of the ImPlot3D library.");
    ui.spacing();

    ui.text("USER GUIDE:");
    ui.bullet_text("Left click and drag to rotate the plot.");
    ui.bullet_text("Right click and drag to pan the plot.");
    ui.bullet_text("Scroll to zoom in and out.");
    ui.bullet_text("Double-click to reset the view.");
    ui.spacing();

    ui.text("FEATURES:");
    ui.bullet_text("Line plots with animated data");
    ui.bullet_text("Scatter plots with custom markers");
    ui.bullet_text("Triangle and quad plots for 3D shapes");
    ui.bullet_text("Surface plots with colormap support");
    ui.bullet_text("Mesh plots for complex geometries");
    ui.bullet_text("Realtime plots with scrolling buffers");
    ui.bullet_text("Custom styles and rendering");
    ui.bullet_text("Axis constraints and transformations");
    ui.spacing();

    ui.text("RUST BINDINGS:");
    ui.bullet_text("Safe Rust API with builder patterns");
    ui.bullet_text("Type-safe f32/f64 support");
    ui.bullet_text("RAII-based resource management");
    ui.bullet_text("Integration with dear-imgui-rs ecosystem");
}
