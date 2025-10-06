//! ImPlot3D Demo - Rust port of official implot3d_demo.cpp
//!
//! This example demonstrates the main features of ImPlot3D by replicating
//! key demos from the official C++ demo.

use dear_app::{AddOnsConfig, RunnerConfig, run};
use dear_imgui_rs::*;
use dear_implot3d as implot3d;
use implot3d::plots::*;
use implot3d::*;
use std::f32::consts::PI;

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
            .size([1000.0, 700.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("ImPlot3D says olá! (Rust bindings)"));
                ui.separator();

                if let Some(tab_bar) = ui.tab_bar("ImPlot3D Demo Tabs") {
                    // Tab 1: Line Plots
                    if let Some(_tab) = ui.tab_item("Line Plots") {
                        demo_line_plots(ui, &plot_ui);
                    }

                    // Tab 2: Scatter Plots
                    if let Some(_tab) = ui.tab_item("Scatter Plots") {
                        demo_scatter_plots(ui, &plot_ui);
                    }

                    // Tab 3: Triangle Plots (Pyramid)
                    if let Some(_tab) = ui.tab_item("Triangle Plots") {
                        demo_triangle_plots(ui, &plot_ui);
                    }

                    // Tab 4: Quad Plots (Cube)
                    if let Some(_tab) = ui.tab_item("Quad Plots") {
                        demo_quad_plots(ui, &plot_ui);
                    }

                    // Tab 5: Surface Plots
                    if let Some(_tab) = ui.tab_item("Surface Plots") {
                        demo_surface_plots(ui, &plot_ui);
                    }

                    // Tab 6: Mesh Plots
                    if let Some(_tab) = ui.tab_item("Mesh Plots") {
                        demo_mesh_plots(ui, &plot_ui);
                    }

                    // Tab 7: Box Scale
                    if let Some(_tab) = ui.tab_item("Box Scale") {
                        demo_box_scale(ui, &plot_ui);
                    }

                    // Tab 8: Box Rotation
                    if let Some(_tab) = ui.tab_item("Box Rotation") {
                        demo_box_rotation(ui, &plot_ui);
                    }

                    // Tab 9: Tick Labels
                    if let Some(_tab) = ui.tab_item("Tick Labels") {
                        demo_tick_labels(ui, &plot_ui);
                    }

                    // Tab 10: Axis Constraints
                    if let Some(_tab) = ui.tab_item("Axis Constraints") {
                        demo_axis_constraints(ui, &plot_ui);
                    }

                    // Tab 11: Markers and Text
                    if let Some(_tab) = ui.tab_item("Markers & Text") {
                        demo_markers_and_text(ui, &plot_ui);
                    }

                    // Tab 12: NaN Values
                    if let Some(_tab) = ui.tab_item("NaN Values") {
                        demo_nan_values(ui, &plot_ui);
                    }
                    drop(tab_bar);
                }
            });
    })
    .unwrap();
}

// ============================================================================
// Demo Functions (ported from C++ implot3d_demo.cpp)
// ============================================================================

fn demo_line_plots(_ui: &Ui, plot_ui: &Plot3DUi) {
    // Animated line plot
    let time = unsafe { dear_imgui_sys::igGetTime() } as f32;
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
        push_style_var_f32(dear_implot3d_sys::ImPlot3DStyleVar_FillAlpha as i32, 0.25);
        let col1 = get_colormap_color(1);
        set_next_marker_style(Marker3D::Square, 6.0, col1, -1.0, col1);
        Scatter3D::f32("Data 2", &xs2, &ys2, &zs2).plot(plot_ui);
        pop_style_var(1);
    }
}

fn demo_triangle_plots(_ui: &Ui, plot_ui: &Plot3DUi) {
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

    if let Some(_tok) = plot_ui.begin_plot("Triangle Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -0.5, 1.5, Plot3DCond::Once);
        set_next_fill_style(get_colormap_color(0), 1.0);
        set_next_line_style(get_colormap_color(1), 2.0);
        let col2 = get_colormap_color(2);
        set_next_marker_style(Marker3D::Square, 3.0, col2, -1.0, col2);
        Triangles3D::f32("Pyramid", &xs, &ys, &zs).plot(plot_ui);
    }
}

fn demo_quad_plots(_ui: &Ui, plot_ui: &Plot3DUi) {
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

    if let Some(_tok) = plot_ui.begin_plot("Quad Plots").build() {
        plot_ui.setup_axes_limits(-1.5, 1.5, -1.5, 1.5, -1.5, 1.5, Plot3DCond::Once);

        let color_x = [0.8, 0.2, 0.2, 0.8];
        let color_y = [0.2, 0.8, 0.2, 0.8];
        let color_z = [0.2, 0.2, 0.8, 0.8];

        set_next_fill_style(color_x, 1.0);
        set_next_line_style(color_x, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_x, -1.0, color_x);
        Quads3D::f32("X", &xs[0..8], &ys[0..8], &zs[0..8]).plot(plot_ui);

        set_next_fill_style(color_y, 1.0);
        set_next_line_style(color_y, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_y, -1.0, color_y);
        Quads3D::f32("Y", &xs[8..16], &ys[8..16], &zs[8..16]).plot(plot_ui);

        set_next_fill_style(color_z, 1.0);
        set_next_line_style(color_z, 2.0);
        set_next_marker_style(Marker3D::Square, 3.0, color_z, -1.0, color_z);
        Quads3D::f32("Z", &xs[16..24], &ys[16..24], &zs[16..24]).plot(plot_ui);
    }
}

fn demo_surface_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    const N: usize = 20;
    let time = unsafe { dear_imgui_sys::igGetTime() } as f32;

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
            zs[idx] = (2.0 * time + r).sin();
        }
    }

    ui.text("Fill color");
    static mut SELECTED_FILL: bool = true;
    static mut SEL_COLORMAP: i32 = 5;

    unsafe {
        if ui.radio_button_bool("Solid", !SELECTED_FILL) {
            SELECTED_FILL = false;
        }
        if ui.radio_button_bool("Colormap", SELECTED_FILL) {
            SELECTED_FILL = true;
        }
        if SELECTED_FILL {
            ui.same_line();
            let colormaps = ["Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet"];
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

        if SELECTED_FILL {
            let colormaps = ["Viridis", "Plasma", "Hot", "Cool", "Pink", "Jet"];
            set_style_colormap_by_name(colormaps[SEL_COLORMAP as usize]);
        }
    }

    if let Some(_tok) = plot_ui
        .begin_plot("Surface Plots")
        .size([0.0, 400.0])
        .build()
    {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.5, 1.5, Plot3DCond::Once);
        push_style_var_f32(dear_implot3d_sys::ImPlot3DStyleVar_FillAlpha as i32, 0.8);

        let x_grid: Vec<f32> = (0..N).map(|j| MIN_VAL + j as f32 * STEP).collect();
        let y_grid: Vec<f32> = (0..N).map(|i| MIN_VAL + i as f32 * STEP).collect();

        Surface3D::new("Wave Surface", &x_grid, &y_grid, &zs).plot(plot_ui);
        pop_style_var(1);
    }
}

fn demo_mesh_plots(_ui: &Ui, plot_ui: &Plot3DUi) {
    // Simple tetrahedron
    let vertices: [[f32; 3]; 4] = [
        [0.0, 0.0, 0.8],
        [0.8, 0.0, -0.2],
        [0.0, 0.8, -0.2],
        [-0.8, -0.8, -0.2],
    ];
    let indices: [u32; 12] = [0, 1, 2, 0, 1, 3, 0, 2, 3, 1, 2, 3];

    if let Some(_tok) = plot_ui.begin_plot("Mesh Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Once);
        set_next_fill_style([0.8, 0.8, 0.2, 0.6], 1.0);
        set_next_line_style([0.5, 0.5, 0.2, 0.6], 1.0);
        let marker_col = [0.5, 0.5, 0.2, 0.6];
        set_next_marker_style(Marker3D::Square, 3.0, marker_col, -1.0, marker_col);
        Mesh3D::new("Tetrahedron", &vertices, &indices).plot(plot_ui);
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
    let xs = [0.0f32, 1.0, 2.0];
    let ys = [0.0f32, 1.0, 2.0];
    let zs = [0.0f32, 1.0, 2.0];

    if let Some(_tok) = plot_ui.begin_plot("Tick Labels").build() {
        plot_ui.setup_axes_limits(-0.5, 2.5, -0.5, 2.5, -0.5, 2.5, Plot3DCond::Once);

        // Custom tick labels
        let x_labels = ["Low", "Mid", "High"];
        let y_labels = ["A", "B", "C"];
        let z_labels = ["Min", "Med", "Max"];

        plot_ui.setup_axis_ticks_values(Axis3D::X, &xs, Some(&x_labels), false);
        plot_ui.setup_axis_ticks_values(Axis3D::Y, &ys, Some(&y_labels), false);
        plot_ui.setup_axis_ticks_values(Axis3D::Z, &zs, Some(&z_labels), false);

        Scatter3D::f32("Points", &xs, &ys, &zs).plot(plot_ui);
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
