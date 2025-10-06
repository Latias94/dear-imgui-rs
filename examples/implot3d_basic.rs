//! ImPlot3D Demo - Complete Rust port of official implot3d_demo.cpp
//!
//! This example is a faithful reproduction of the official C++ ImPlot3D demo,
//! showcasing all major features of the library.

use dear_app::{AddOnsConfig, RunnerConfig, run};
use dear_imgui_rs::*;
use dear_implot3d as implot3d;
use implot3d::plots::*;
use implot3d::*;
use std::cell::{Cell, RefCell};
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
    thread_local! {
        static FLAGS: Cell<Triangle3DFlags> = Cell::new(Triangle3DFlags::NONE);
    }

    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox_flags("NoLines", &mut flags, Triangle3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill", &mut flags, Triangle3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers", &mut flags, Triangle3DFlags::NO_MARKERS);
    FLAGS.with(|f| f.set(flags));

    if let Some(_tok) = plot_ui.begin_plot("Triangle Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -0.5, 1.5, Plot3DCond::Once);
        set_next_fill_style(get_colormap_color(0), 1.0);
        set_next_line_style(get_colormap_color(1), 2.0);
        let col2 = get_colormap_color(2);
        set_next_marker_style(Marker3D::Square, 3.0, col2, -1.0, col2);
        Triangles3D::f32("Pyramid", &xs, &ys, &zs)
            .flags(flags)
            .plot(plot_ui);
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
    thread_local! {
        static FLAGS: Cell<Quad3DFlags> = Cell::new(Quad3DFlags::NONE);
    }

    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox_flags("NoLines", &mut flags, Quad3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill", &mut flags, Quad3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers", &mut flags, Quad3DFlags::NO_MARKERS);
    FLAGS.with(|f| f.set(flags));

    if let Some(_tok) = plot_ui.begin_plot("Quad Plots").build() {
        plot_ui.setup_axes_limits(-1.5, 1.5, -1.5, 1.5, -1.5, 1.5, Plot3DCond::Once);

        let color_x = [0.8, 0.2, 0.2, 0.8];
        let color_y = [0.2, 0.8, 0.2, 0.8];
        let color_z = [0.2, 0.2, 0.8, 0.8];

        set_next_fill_style(color_x, 1.0);
        set_next_line_style(color_x, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_x, -1.0, color_x);
        Quads3D::f32("X", &xs[0..8], &ys[0..8], &zs[0..8])
            .flags(flags)
            .plot(plot_ui);

        set_next_fill_style(color_y, 1.0);
        set_next_line_style(color_y, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_y, -1.0, color_y);
        Quads3D::f32("Y", &xs[8..16], &ys[8..16], &zs[8..16])
            .flags(flags)
            .plot(plot_ui);

        set_next_fill_style(color_z, 1.0);
        set_next_line_style(color_z, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_z, -1.0, color_z);
        Quads3D::f32("Z", &xs[16..24], &ys[16..24], &zs[16..24])
            .flags(flags)
            .plot(plot_ui);
    }
}

fn demo_surface_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    const N: usize = 20;
    thread_local! {
        static T: Cell<f32> = Cell::new(0.0);
        static SELECTED_FILL: Cell<i32> = Cell::new(1); // Colormap by default
        static SOLID_COLOR: Cell<[f32; 4]> = Cell::new([0.8, 0.8, 0.2, 0.6]);
        static SEL_COLORMAP: Cell<i32> = Cell::new(5); // Jet by default
        static CUSTOM_RANGE: Cell<bool> = Cell::new(false);
        static RANGE_MIN: Cell<f32> = Cell::new(-1.0);
        static RANGE_MAX: Cell<f32> = Cell::new(1.0);
        static FLAGS: Cell<Surface3DFlags> = Cell::new(Surface3DFlags::NO_MARKERS);
    }

    let t = T.with(|t| {
        let val = t.get() + ui.io().delta_time();
        t.set(val);
        val
    });

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
            zs[idx] = (2.0 * t + r).sin();
        }
    }

    // Choose fill color
    ui.text("Fill color");
    let mut selected_fill = SELECTED_FILL.with(|f| f.get());
    let mut solid_color = SOLID_COLOR.with(|c| c.get());
    let mut sel_colormap = SEL_COLORMAP.with(|c| c.get());

    ui.indent();

    // Choose solid color
    if ui.radio_button("Solid", selected_fill == 0) {
        selected_fill = 0;
    }
    if selected_fill == 0 {
        ui.same_line();
        ui.color_edit4_config("##SurfaceSolidColor", &mut solid_color)
            .build();
    }

    // Choose colormap
    if ui.radio_button("Colormap", selected_fill == 1) {
        selected_fill = 1;
    }
    if selected_fill == 1 {
        ui.same_line();
        let colormaps = [
            "Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet", "Twilight", "RdBu", "BrBG", "PiYG",
            "Spectral", "Greys",
        ];
        if let Some(_combo) = ui.begin_combo("##SurfaceColormap", colormaps[sel_colormap as usize])
        {
            for (i, name) in colormaps.iter().enumerate() {
                if ui.selectable(name) {
                    sel_colormap = i as i32;
                }
            }
        }
    }
    ui.unindent();

    SELECTED_FILL.with(|f| f.set(selected_fill));
    SOLID_COLOR.with(|c| c.set(solid_color));
    SEL_COLORMAP.with(|c| c.set(sel_colormap));

    // Choose range
    let mut custom_range = CUSTOM_RANGE.with(|r| r.get());
    let mut range_min = RANGE_MIN.with(|r| r.get());
    let mut range_max = RANGE_MAX.with(|r| r.get());

    ui.checkbox("Custom range", &mut custom_range);
    ui.indent();

    let _disabled = if !custom_range {
        Some(ui.begin_disabled())
    } else {
        None
    };
    ui.slider_config("Range min", -1.0, range_max - 0.01)
        .build(&mut range_min);
    ui.slider_config("Range max", range_min + 0.01, 1.0)
        .build(&mut range_max);
    drop(_disabled);

    ui.unindent();

    CUSTOM_RANGE.with(|r| r.set(custom_range));
    RANGE_MIN.with(|r| r.set(range_min));
    RANGE_MAX.with(|r| r.set(range_max));

    // Select flags
    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox_flags("NoLines", &mut flags, Surface3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill", &mut flags, Surface3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers", &mut flags, Surface3DFlags::NO_MARKERS);
    FLAGS.with(|f| f.set(flags));

    // Begin the plot
    if selected_fill == 1 {
        let colormaps = [
            "Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet", "Twilight", "RdBu", "BrBG", "PiYG",
            "Spectral", "Greys",
        ];
        push_colormap_name(colormaps[sel_colormap as usize]);
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
        if selected_fill == 0 {
            set_next_fill_style(solid_color, 1.0);
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

        if custom_range {
            Surface3D::new("Wave Surface", &x_grid, &y_grid, &zs)
                .scale(range_min as f64, range_max as f64)
                .flags(flags)
                .plot(plot_ui);
        } else {
            Surface3D::new("Wave Surface", &x_grid, &y_grid, &zs)
                .scale(0.0, 0.0)
                .flags(flags)
                .plot(plot_ui);
        }

        pop_style_var(1);
    }

    if selected_fill == 1 {
        pop_colormap(1);
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
    thread_local! {
        static FLAGS: Cell<Mesh3DFlags> = Cell::new(Mesh3DFlags::NONE);
    }

    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox_flags("NoLines", &mut flags, Mesh3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill", &mut flags, Mesh3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers", &mut flags, Mesh3DFlags::NO_MARKERS);
    FLAGS.with(|f| f.set(flags));

    if let Some(_tok) = plot_ui.begin_plot("Mesh Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Once);
        set_next_fill_style([0.8, 0.8, 0.2, 0.6], 1.0);
        set_next_line_style([0.5, 0.5, 0.2, 0.6], 1.0);
        let marker_col = [0.5, 0.5, 0.2, 0.6];
        set_next_marker_style(Marker3D::Square, 3.0, marker_col, -1.0, marker_col);
        Mesh3D::new("Tetrahedron", &vertices, &indices)
            .flags(flags)
            .plot(plot_ui);
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
    thread_local! {
        static T: Cell<f32> = Cell::new(0.0);
        static HISTORY: Cell<f32> = Cell::new(10.0);
        static SDATA1: RefCell<ScrollingBuffer> = RefCell::new(ScrollingBuffer::new(1000));
        static SDATA2: RefCell<ScrollingBuffer> = RefCell::new(ScrollingBuffer::new(1000));
        static SDATA3: RefCell<ScrollingBuffer> = RefCell::new(ScrollingBuffer::new(1000));
    }

    let t = T.with(|t| {
        let val = t.get() + ui.io().delta_time();
        t.set(val);
        val
    });

    SDATA1.with(|s| s.borrow_mut().add_point(t, (2.0 * t).sin()));
    SDATA2.with(|s| s.borrow_mut().add_point(t, (2.0 * t).cos()));
    SDATA3.with(|s| {
        s.borrow_mut()
            .add_point(t, (2.0 * t + PI / 2.0).sin() * (2.0 * t + PI / 2.0).cos())
    });

    let mut history = HISTORY.with(|h| h.get());
    ui.slider_config("History", 1.0, 30.0).build(&mut history);
    HISTORY.with(|h| h.set(history));

    ui.same_line();
    if ui.button("Reset") {
        SDATA1.with(|s| s.borrow_mut().erase());
        SDATA2.with(|s| s.borrow_mut().erase());
        SDATA3.with(|s| s.borrow_mut().erase());
        T.with(|t| t.set(0.0));
    }

    if let Some(_tok) = plot_ui
        .begin_plot("Realtime Plots")
        .size([-1.0, 400.0])
        .build()
    {
        plot_ui.setup_axes_limits(
            (t - history) as f64,
            t as f64,
            -1.0,
            1.0,
            -1.0,
            1.0,
            Plot3DCond::Always,
        );

        let (xs1, ys1) = SDATA1.with(|s| s.borrow().get_data());
        let (xs2, ys2) = SDATA2.with(|s| s.borrow().get_data());
        let (xs3, zs3) = SDATA3.with(|s| s.borrow().get_data());

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

    thread_local! {
        static SCALE: Cell<[f32; 3]> = Cell::new([1.0, 1.0, 1.0]);
    }

    let mut scale = SCALE.with(|s| s.get());
    ui.slider_config("Box Scale", 0.1, 2.0)
        .build_array(&mut scale);
    SCALE.with(|s| s.set(scale));

    if let Some(_tok) = plot_ui.begin_plot("##BoxScale").build() {
        plot_ui.setup_box_scale(scale[0], scale[1], scale[2]);
        Line3D::f32("3D Curve", &xs, &ys, &zs).plot(plot_ui);
    }
}

fn demo_box_rotation(ui: &Ui, plot_ui: &Plot3DUi) {
    thread_local! {
        static ELEVATION: Cell<f32> = Cell::new(45.0);
        static AZIMUTH: Cell<f32> = Cell::new(-135.0);
        static ANIMATE: Cell<bool> = Cell::new(false);
        static INIT_ELEVATION: Cell<f32> = Cell::new(45.0);
        static INIT_AZIMUTH: Cell<f32> = Cell::new(-135.0);
    }

    ui.text("Rotation");
    let mut elevation = ELEVATION.with(|e| e.get());
    let mut azimuth = AZIMUTH.with(|a| a.get());
    let mut animate = ANIMATE.with(|a| a.get());
    let mut init_elevation = INIT_ELEVATION.with(|e| e.get());
    let mut init_azimuth = INIT_AZIMUTH.with(|a| a.get());

    let mut changed = false;
    changed |= ui
        .slider_config("Elevation", -90.0, 90.0)
        .build(&mut elevation);
    changed |= ui
        .slider_config("Azimuth", -180.0, 180.0)
        .build(&mut azimuth);
    ui.checkbox("Animate", &mut animate);

    ui.text("Initial Rotation");
    ui.slider_config("Initial Elevation", -90.0, 90.0)
        .build(&mut init_elevation);
    ui.slider_config("Initial Azimuth", -180.0, 180.0)
        .build(&mut init_azimuth);

    ELEVATION.with(|e| e.set(elevation));
    AZIMUTH.with(|a| a.set(azimuth));
    ANIMATE.with(|a| a.set(animate));
    INIT_ELEVATION.with(|e| e.set(init_elevation));
    INIT_AZIMUTH.with(|a| a.set(init_azimuth));

    if let Some(_tok) = plot_ui.begin_plot("##BoxRotation").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Always);
        plot_ui.setup_box_initial_rotation(init_elevation, init_azimuth);
        if changed {
            plot_ui.setup_box_rotation(elevation, azimuth, animate, Plot3DCond::Always);
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
    thread_local! {
        static ENABLE_LIMITS: Cell<bool> = Cell::new(false);
        static ENABLE_ZOOM: Cell<bool> = Cell::new(false);
    }

    let mut enable_limits = ENABLE_LIMITS.with(|e| e.get());
    let mut enable_zoom = ENABLE_ZOOM.with(|e| e.get());

    ui.checkbox("Enable Limits Constraints", &mut enable_limits);
    ui.checkbox("Enable Zoom Constraints", &mut enable_zoom);

    ENABLE_LIMITS.with(|e| e.set(enable_limits));
    ENABLE_ZOOM.with(|e| e.set(enable_zoom));

    let xs = [0.0f32, 1.0, 2.0];
    let ys = [0.0f32, 1.0, 2.0];
    let zs = [0.0f32, 1.0, 2.0];

    if let Some(_tok) = plot_ui.begin_plot("Axis Constraints").build() {
        plot_ui.setup_axes_limits(-1.0, 3.0, -1.0, 3.0, -1.0, 3.0, Plot3DCond::Once);

        if enable_limits {
            plot_ui.setup_axis_limits_constraints(Axis3D::X, -0.5, 2.5);
            plot_ui.setup_axis_limits_constraints(Axis3D::Y, -0.5, 2.5);
            plot_ui.setup_axis_limits_constraints(Axis3D::Z, -0.5, 2.5);
        }

        if enable_zoom {
            plot_ui.setup_axis_zoom_constraints(Axis3D::X, 0.5, 5.0);
            plot_ui.setup_axis_zoom_constraints(Axis3D::Y, 0.5, 5.0);
            plot_ui.setup_axis_zoom_constraints(Axis3D::Z, 0.5, 5.0);
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
    thread_local! {
        static LINE_WEIGHT: Cell<f32> = Cell::new(2.0);
        static MARKER_SIZE: Cell<f32> = Cell::new(5.0);
        static MARKER_WEIGHT: Cell<f32> = Cell::new(1.0);
        static FILL_ALPHA: Cell<f32> = Cell::new(0.5);
    }

    let mut line_weight = LINE_WEIGHT.with(|w| w.get());
    let mut marker_size = MARKER_SIZE.with(|s| s.get());
    let mut marker_weight = MARKER_WEIGHT.with(|w| w.get());
    let mut fill_alpha = FILL_ALPHA.with(|a| a.get());

    ui.slider_config("LineWeight", 0.5, 5.0)
        .build(&mut line_weight);
    ui.slider_config("MarkerSize", 2.0, 10.0)
        .build(&mut marker_size);
    ui.slider_config("MarkerWeight", 0.5, 3.0)
        .build(&mut marker_weight);
    ui.slider_config("FillAlpha", 0.0, 1.0)
        .build(&mut fill_alpha);

    LINE_WEIGHT.with(|w| w.set(line_weight));
    MARKER_SIZE.with(|s| s.set(marker_size));
    MARKER_WEIGHT.with(|w| w.set(marker_weight));
    FILL_ALPHA.with(|a| a.set(fill_alpha));

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

        push_style_var_f32(IMPLOT3D_STYLEVAR_LINE_WEIGHT, line_weight);
        push_style_var_f32(IMPLOT3D_STYLEVAR_MARKER_SIZE, marker_size);
        push_style_var_f32(IMPLOT3D_STYLEVAR_MARKER_WEIGHT, marker_weight);
        push_style_var_f32(IMPLOT3D_STYLEVAR_FILL_ALPHA, fill_alpha);

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
