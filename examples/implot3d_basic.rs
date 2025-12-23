//! ImPlot3D Demo - Complete Rust port of official implot3d_demo.cpp
//!
//! This example is a faithful reproduction of the official C++ ImPlot3D demo,
//! showcasing all major features of the library.
//!
//! The demo shows two windows side-by-side using dockspace:
//! - Left: Rust implementation (ImPlot3D Demo (Rust))
//! - Right: Official C++ implementation (ImPlot3D Demo)

use dear_app::{AddOnsConfig, DockingConfig, RunnerConfig, run};
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
        window_title: "ImPlot3D Demo - Rust vs C++ Comparison".to_string(),
        window_size: (1600.0, 900.0), // Wider window for side-by-side comparison
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.06, 0.08, 0.1, 1.0],
        docking: DockingConfig {
            enable: true,
            auto_dockspace: true, // Enable automatic dockspace
            dockspace_flags: DockFlags::PASSTHRU_CENTRAL_NODE,
            host_window_flags: WindowFlags::empty(),
            host_window_name: "DockSpaceHost",
        },
        ini_filename: Some("implot3d_demo.ini".into()), // Save layout
        restore_previous_geometry: true,
        redraw: dear_app::RedrawMode::Poll,
        io_config_flags: None,
        ..Default::default()
    };

    let addons = AddOnsConfig::auto();

    // State to track if layout has been initialized
    let layout_initialized = std::cell::RefCell::new(false);

    run(runner, addons, move |ui, addons| {
        let Some(plot_ctx) = addons.implot3d else {
            ui.text("ImPlot3D add-on not enabled");
            return;
        };
        let plot_ui = plot_ctx.get_plot_ui(ui);

        // Initialize dockspace layout on first frame
        if !*layout_initialized.borrow() {
            setup_dockspace_layout(ui);
            *layout_initialized.borrow_mut() = true;
        }

        // Rust implementation window (will be docked to the left)
        ui.window("ImPlot3D Demo (Rust)")
            .size([750.0, 850.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("ImPlot3D says olá! (0.3 WIP)"));
                ui.spacing();
                // Tools toggles similar to upstream demo
                demo_tools(ui);

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
                        demo_header(ui, "Image Plots", || demo_image_plots(ui, &plot_ui));
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

        // Official C++ implementation window (will be docked to the right)
        // Note: show_demo_window() creates a window titled "ImPlot3D Demo"
        implot3d::show_demo_window();
    })
    .unwrap();
}

/// Setup the dockspace layout: split into left (Rust) and right (C++) panels
fn setup_dockspace_layout(ui: &Ui) {
    // Get the dockspace ID (created by auto_dockspace)
    let dockspace_id = ui.get_id("DockSpaceHost");

    // Only setup layout if the dockspace node doesn't exist yet
    if !DockBuilder::node_exists(&ui, dockspace_id) {
        let viewport_size = ui.main_viewport().size();

        // Clear any existing layout
        DockBuilder::remove_node(dockspace_id);

        // Create the root dockspace node
        DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
        DockBuilder::set_node_size(dockspace_id, viewport_size);

        // Split the dockspace: left 50% for Rust demo, right 50% for C++ demo
        let (left_id, right_id) = DockBuilder::split_node(
            dockspace_id,
            SplitDirection::Left,
            0.5, // 50% split
        );

        // Dock the windows to their respective panels
        DockBuilder::dock_window("ImPlot3D Demo (Rust)", left_id);
        DockBuilder::dock_window("ImPlot3D Demo", right_id); // C++ demo window title

        // Finalize the layout
        DockBuilder::finish(dockspace_id);
    }
}

// Helper function to create collapsible demo sections
fn demo_header<F: FnOnce()>(ui: &Ui, label: &str, demo: F) {
    if let Some(_node) = ui
        .tree_node_config(label)
        .framed(true)
        .span_avail_width(true)
        .frame_padding(true)
        .push()
    {
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
    ui.checkbox_flags("NoLines##Triangles", &mut flags, Triangle3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill##Triangles", &mut flags, Triangle3DFlags::NO_FILL);
    ui.checkbox_flags(
        "NoMarkers##Triangles",
        &mut flags,
        Triangle3DFlags::NO_MARKERS,
    );
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
    ui.checkbox_flags("NoLines##Quads", &mut flags, Quad3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill##Quads", &mut flags, Quad3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers##Quads", &mut flags, Quad3DFlags::NO_MARKERS);
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
    ui.checkbox_flags("NoLines##Surface", &mut flags, Surface3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill##Surface", &mut flags, Surface3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers##Surface", &mut flags, Surface3DFlags::NO_MARKERS);
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

    // Mesh flags & colors
    thread_local! {
        static FLAGS: Cell<Mesh3DFlags> = Cell::new(Mesh3DFlags::NONE);
        static LINE_COLOR: Cell<[f32;4]> = Cell::new([0.5, 0.5, 0.2, 0.6]);
        static FILL_COLOR: Cell<[f32;4]> = Cell::new([0.8, 0.8, 0.2, 0.6]);
        static MARKER_COLOR: Cell<[f32;4]> = Cell::new([0.5, 0.5, 0.2, 0.6]);
        static MESH_ID: Cell<i32> = Cell::new(0); // 0=Tetrahedron, 1=Cube
    }

    // Mesh selector
    let mut mesh_id = MESH_ID.with(|m| m.get());
    let items = ["Tetrahedron", "Cube"];
    let preview = items[mesh_id as usize];
    if let Some(_combo) = ui.begin_combo("Mesh##Mesh", preview) {
        for (i, name) in items.iter().enumerate() {
            if ui.selectable(name) {
                mesh_id = i as i32;
            }
        }
    }
    MESH_ID.with(|m| m.set(mesh_id));

    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox_flags("NoLines##Mesh", &mut flags, Mesh3DFlags::NO_LINES);
    ui.checkbox_flags("NoFill##Mesh", &mut flags, Mesh3DFlags::NO_FILL);
    ui.checkbox_flags("NoMarkers##Mesh", &mut flags, Mesh3DFlags::NO_MARKERS);
    FLAGS.with(|f| f.set(flags));

    let mut line_color = LINE_COLOR.with(|c| c.get());
    let mut fill_color = FILL_COLOR.with(|c| c.get());
    let mut marker_color = MARKER_COLOR.with(|c| c.get());
    ui.color_edit4_config("Line Color##Mesh", &mut line_color)
        .build();
    ui.color_edit4_config("Fill Color##Mesh", &mut fill_color)
        .build();
    ui.color_edit4_config("Marker Color##Mesh", &mut marker_color)
        .build();
    LINE_COLOR.with(|c| c.set(line_color));
    FILL_COLOR.with(|c| c.set(fill_color));
    MARKER_COLOR.with(|c| c.set(marker_color));

    if let Some(_tok) = plot_ui.begin_plot("Mesh Plots").build() {
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Once);
        set_next_fill_style(fill_color, 1.0);
        set_next_line_style(line_color, 1.0);
        set_next_marker_style(Marker3D::Square, 3.0, marker_color, -1.0, marker_color);
        if mesh_id == 0 {
            Mesh3D::new("Tetrahedron", &vertices, &indices)
                .flags(flags)
                .plot(plot_ui);
        } else {
            use implot3d::meshes::{CUBE_INDICES, CUBE_VERTICES};
            Mesh3D::new("Cube", CUBE_VERTICES, CUBE_INDICES)
                .flags(flags)
                .plot(plot_ui);
        }
    }
}

// Scrolling buffers (1D) for realtime demo
struct ScrollingBuffer1D {
    max_size: usize,
    offset: usize,
    data: Vec<f32>,
}

impl ScrollingBuffer1D {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            offset: 0,
            data: Vec::new(),
        }
    }
    fn add(&mut self, v: f32) {
        if self.data.len() < self.max_size {
            self.data.push(v);
        } else {
            self.data[self.offset] = v;
            self.offset = (self.offset + 1) % self.max_size;
        }
    }
    fn clear(&mut self) {
        self.data.clear();
        self.offset = 0;
    }
}

fn demo_realtime_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    ui.bullet_text("Move your mouse to change the data!");

    thread_local! {
        static T: Cell<f32> = Cell::new(0.0);
        static LAST_T: Cell<f32> = Cell::new(-1.0);
        static XS: RefCell<ScrollingBuffer1D> = RefCell::new(ScrollingBuffer1D::new(2000));
        static YS: RefCell<ScrollingBuffer1D> = RefCell::new(ScrollingBuffer1D::new(2000));
        static ZS: RefCell<ScrollingBuffer1D> = RefCell::new(ScrollingBuffer1D::new(2000));
    }

    if let Some(_tok) = plot_ui
        .begin_plot("Scrolling Plot")
        .size([-1.0, 400.0])
        .build()
    {
        let t = T.with(|tc| {
            let v = tc.get() + ui.io().delta_time();
            tc.set(v);
            v
        });

        // Set up axes BEFORE any plot-utils or plot items
        let flags = Axis3DFlags::NO_TICK_LABELS;
        plot_ui.setup_axes("Time", "Mouse X", "Mouse Y", flags, flags, flags);
        plot_ui.setup_axis_limits(Axis3D::X, (t - 10.0) as f64, t as f64, Plot3DCond::Always);
        plot_ui.setup_axis_limits(Axis3D::Y, -400.0, 400.0, Plot3DCond::Once);
        plot_ui.setup_axis_limits(Axis3D::Z, -400.0, 400.0, Plot3DCond::Once);

        // Now sample/fill buffers and use plot utils
        let last_t = LAST_T.with(|lt| lt.get());
        if t - last_t > 0.01 {
            LAST_T.with(|lt| lt.set(t));
            let mouse = ui.io().mouse_pos();
            if mouse[0].abs() < 1.0e4 && mouse[1].abs() < 1.0e4 {
                let pos = plot_ui.get_frame_pos();
                let sz = plot_ui.get_frame_size();
                let center = [pos[0] + sz[0] / 2.0, pos[1] + sz[1] / 2.0];
                XS.with(|b| b.borrow_mut().add(t));
                YS.with(|b| b.borrow_mut().add(mouse[0] - center[0]));
                ZS.with(|b| b.borrow_mut().add(mouse[1] - center[1]));
            }
        }

        // Plot line using raw ring-buffer data
        XS.with(|xs| {
            let xs = xs.borrow();
            YS.with(|ys| {
                let ys = ys.borrow();
                ZS.with(|zs| {
                    let zs = zs.borrow();
                    if !xs.data.is_empty()
                        && xs.data.len() == ys.data.len()
                        && ys.data.len() == zs.data.len()
                    {
                        plot_ui.plot_line_f32_raw(
                            "Mouse",
                            &xs.data,
                            &ys.data,
                            &zs.data,
                            Line3DFlags::NONE,
                            xs.offset as i32,
                            0,
                        );
                    }
                });
            });
        });
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

fn demo_tick_labels(ui: &Ui, plot_ui: &Plot3DUi) {
    thread_local! {
        static CUSTOM_TICKS: Cell<bool> = Cell::new(true);
        static CUSTOM_LABELS: Cell<bool> = Cell::new(true);
    }
    let mut custom_ticks = CUSTOM_TICKS.with(|c| c.get());
    let mut custom_labels = CUSTOM_LABELS.with(|c| c.get());
    ui.checkbox("Show Custom Ticks", &mut custom_ticks);
    if custom_ticks {
        ui.same_line();
        ui.checkbox("Show Custom Labels", &mut custom_labels);
    }
    CUSTOM_TICKS.with(|c| c.set(custom_ticks));
    CUSTOM_LABELS.with(|c| c.set(custom_labels));

    if let Some(_tok) = plot_ui.begin_plot("##Ticks").build() {
        plot_ui.setup_axes_limits(2.0, 5.0, 0.0, 1.0, 0.0, 1.0, Plot3DCond::Once);
        if custom_ticks {
            let pi = std::f64::consts::PI;
            let letters_ticks = [0.0f64, 0.2, 0.4, 0.6, 0.8, 1.0];
            let pi_lbl = ["PI"];
            let letters_lbl = ["A", "B", "C", "D", "E", "F"];
            plot_ui.setup_axis_ticks_values(
                Axis3D::X,
                &[pi],
                if custom_labels { Some(&pi_lbl) } else { None },
                true,
            );
            plot_ui.setup_axis_ticks_values(
                Axis3D::Y,
                &letters_ticks,
                if custom_labels {
                    Some(&letters_lbl)
                } else {
                    None
                },
                false,
            );
            plot_ui.setup_axis_ticks_range(
                Axis3D::Z,
                0.0,
                1.0,
                6,
                if custom_labels {
                    Some(&letters_lbl)
                } else {
                    None
                },
                false,
            );
        }
    }
}

fn demo_axis_constraints(ui: &Ui, plot_ui: &Plot3DUi) {
    thread_local! {
        static LIMITS: Cell<[f32;2]> = Cell::new([-10.0, 10.0]);
        static ZOOMS: Cell<[f32;2]> = Cell::new([1.0, 20.0]);
        static FLAGS: Cell<Axis3DFlags> = Cell::new(Axis3DFlags::empty());
    }

    let mut limits = LIMITS.with(|c| c.get());
    let mut zooms = ZOOMS.with(|c| c.get());
    let mut axis_flags = FLAGS.with(|f| f.get());
    ui.drag_float2("Limits Constraints", &mut limits);
    ui.drag_float2("Zoom Constraints", &mut zooms);
    ui.checkbox_flags("PanStretch", &mut axis_flags, Axis3DFlags::PAN_STRETCH);
    LIMITS.with(|c| c.set(limits));
    ZOOMS.with(|c| c.set(zooms));
    FLAGS.with(|f| f.set(axis_flags));

    if let Some(_tok) = plot_ui.begin_plot("##AxisConstraints").build() {
        plot_ui.setup_axes("X", "Y", "Z", axis_flags, axis_flags, axis_flags);
        plot_ui.setup_axes_limits(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, Plot3DCond::Once);
        plot_ui.setup_axis_limits_constraints(Axis3D::X, limits[0] as f64, limits[1] as f64);
        plot_ui.setup_axis_limits_constraints(Axis3D::Y, limits[0] as f64, limits[1] as f64);
        plot_ui.setup_axis_limits_constraints(Axis3D::Z, limits[0] as f64, limits[1] as f64);
        plot_ui.setup_axis_zoom_constraints(Axis3D::X, zooms[0] as f64, zooms[1] as f64);
        plot_ui.setup_axis_zoom_constraints(Axis3D::Y, zooms[0] as f64, zooms[1] as f64);
        plot_ui.setup_axis_zoom_constraints(Axis3D::Z, zooms[0] as f64, zooms[1] as f64);
    }
}

fn demo_image_plots(ui: &Ui, plot_ui: &Plot3DUi) {
    use dear_imgui_rs::texture::TextureRef as ImgTextureRef;

    // Use the font atlas texture (always available) like the official demo
    let tex_ref_opt: Option<ImgTextureRef> = unsafe {
        let io = dear_imgui_rs::sys::igGetIO_Nil();
        if io.is_null() {
            None
        } else {
            let atlas = (*io).Fonts;
            if atlas.is_null() {
                None
            } else {
                let raw = (*atlas).TexRef; // ImTextureRef from font atlas
                Some(ImgTextureRef::from_raw(raw))
            }
        }
    };
    if tex_ref_opt.is_none() {
        ui.text("Font atlas texture not ready yet.");
        return;
    }
    let tex = tex_ref_opt.unwrap();

    // Controls/info
    ui.bullet_text("Using font atlas texture for demo");
    ui.bullet_text("Use Image3D with center+axes or corner points");

    // Parameters
    thread_local! {
        static CENTER1: Cell<[f32;3]> = Cell::new([0.0, 0.0, 1.0]);
        static AXIS_U1: Cell<[f32;3]> = Cell::new([1.0, 0.0, 0.0]);
        static AXIS_V1: Cell<[f32;3]> = Cell::new([0.0, 1.0, 0.0]);
        static UV0_1: Cell<[f32;2]> = Cell::new([0.0, 0.0]);
        static UV1_1: Cell<[f32;2]> = Cell::new([1.0, 1.0]);
        static TINT1: Cell<[f32;4]> = Cell::new([1.0, 1.0, 1.0, 1.0]);

        static P0: Cell<[f32;3]> = Cell::new([-1.0, -1.0, 0.0]);
        static P1: Cell<[f32;3]> = Cell::new([ 1.0, -1.0, 0.0]);
        static P2: Cell<[f32;3]> = Cell::new([ 1.0,  1.0, 0.0]);
        static P3: Cell<[f32;3]> = Cell::new([-1.0,  1.0, 0.0]);
        static UV0: Cell<[f32;2]> = Cell::new([0.0, 0.0]);
        static UV1: Cell<[f32;2]> = Cell::new([1.0, 0.0]);
        static UV2: Cell<[f32;2]> = Cell::new([1.0, 1.0]);
        static UV3: Cell<[f32;2]> = Cell::new([0.0, 1.0]);
        static TINT2: Cell<[f32;4]> = Cell::new([1.0, 1.0, 1.0, 1.0]);
    }

    // Plot
    if let Some(_tok) = plot_ui
        .begin_plot("Image Plot")
        .flags(Plot3DFlags::NO_CLIP)
        .build()
    {
        let center1 = CENTER1.with(|c| c.get());
        let axis_u1 = AXIS_U1.with(|c| c.get());
        let axis_v1 = AXIS_V1.with(|c| c.get());
        let uv0_1 = UV0_1.with(|c| c.get());
        let uv1_1 = UV1_1.with(|c| c.get());
        let tint1 = TINT1.with(|c| c.get());
        plot_ui
            .image_by_axes("Image 1", tex, center1, axis_u1, axis_v1)
            .uv(uv0_1, uv1_1)
            .tint(tint1)
            .plot();

        let p0 = P0.with(|c| c.get());
        let p1 = P1.with(|c| c.get());
        let p2 = P2.with(|c| c.get());
        let p3 = P3.with(|c| c.get());
        let uv0 = UV0.with(|c| c.get());
        let uv1 = UV1.with(|c| c.get());
        let uv2 = UV2.with(|c| c.get());
        let uv3 = UV3.with(|c| c.get());
        let tint2 = TINT2.with(|c| c.get());
        plot_ui
            .image_by_corners("Image 2", tex, p0, p1, p2, p3)
            .uvs(uv0, uv1, uv2, uv3)
            .tint(tint2)
            .plot();
    }
}

fn demo_markers_and_text(ui: &Ui, plot_ui: &Plot3DUi) {
    // Marker size/weight controls
    thread_local! {
        static MK_SIZE: Cell<f32> = Cell::new(6.0);
        static MK_WEIGHT: Cell<f32> = Cell::new(1.5);
    }
    let mut mk_size = MK_SIZE.with(|c| c.get());
    let mut mk_weight = MK_WEIGHT.with(|c| c.get());
    ui.drag_float("Marker Size", &mut mk_size);
    ui.drag_float("Marker Weight", &mut mk_weight);
    MK_SIZE.with(|c| c.set(mk_size));
    MK_WEIGHT.with(|c| c.set(mk_weight));

    if let Some(_tok) = plot_ui
        .begin_plot("##MarkerStyles")
        .flags(Plot3DFlags::CANVAS_ONLY)
        .build()
    {
        plot_ui.setup_axes(
            "",
            "",
            "",
            Axis3DFlags::NO_DECORATIONS,
            Axis3DFlags::NO_DECORATIONS,
            Axis3DFlags::NO_DECORATIONS,
        );
        plot_ui.setup_axes_limits(-0.5, 1.5, -0.5, 1.5, 0.0, 12.0, Plot3DCond::Once);

        // Prepare two points; marker is drawn at both
        let mut xs = [0.0f32, 0.0];
        let mut ys = [0.0f32, 0.0];
        let mut zs = [0.0f32, 0.0];

        let markers = [
            Marker3D::Circle,
            Marker3D::Square,
            Marker3D::Diamond,
            Marker3D::Up,
            Marker3D::Down,
            Marker3D::Left,
            Marker3D::Right,
            Marker3D::Cross,
            Marker3D::Plus,
            Marker3D::Asterisk,
        ];

        // Filled markers column at x=0
        zs[0] = markers.len() as f32;
        zs[1] = zs[0] + 1.0;
        for (i, &m) in markers.iter().enumerate() {
            xs[0] = 0.0;
            ys[0] = 0.0;
            xs[1] = xs[0] + (zs[0] / markers.len() as f32 * 2.0 * std::f32::consts::PI).cos() * 0.5;
            ys[1] = ys[0] + (zs[0] / markers.len() as f32 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            set_next_marker_style(
                m,
                mk_size,
                get_colormap_color(0),
                mk_weight,
                get_colormap_color(0),
            );
            Line3D::f32(&format!("##Filled_{}", i), &xs, &ys, &zs).plot(plot_ui);
            zs[0] -= 1.0;
            zs[1] -= 1.0;
        }

        // Open markers column at x=1
        zs[0] = markers.len() as f32;
        zs[1] = zs[0] + 1.0;
        xs[0] = 1.0;
        ys[0] = 1.0;
        for (i, &m) in markers.iter().enumerate() {
            xs[1] = xs[0] + (zs[0] / markers.len() as f32 * 2.0 * std::f32::consts::PI).cos() * 0.5;
            ys[1] = ys[0] - (zs[0] / markers.len() as f32 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            set_next_marker_style(
                m,
                mk_size,
                [0.0, 0.0, 0.0, 0.0],
                mk_weight,
                get_colormap_color(0),
            );
            Line3D::f32(&format!("##Open_{}", i), &xs, &ys, &zs).plot(plot_ui);
            zs[0] -= 1.0;
            zs[1] -= 1.0;
        }

        // Labels
        plot_ui.plot_text(
            "Filled Markers",
            0.0,
            0.0,
            markers.len() as f32 + 2.0,
            0.0,
            [0.0, 0.0],
        );
        plot_ui.plot_text(
            "Open Markers",
            1.0,
            1.0,
            markers.len() as f32 + 2.0,
            0.0,
            [0.0, 0.0],
        );
        plot_ui.plot_text(
            "Rotated Text",
            0.5,
            0.5,
            6.0,
            std::f32::consts::PI / 4.0,
            [0.0, 0.0],
        );
    }
}

fn demo_nan_values(ui: &Ui, plot_ui: &Plot3DUi) {
    thread_local! {
        static INCLUDE_NAN: Cell<bool> = Cell::new(true);
        static FLAGS: Cell<Line3DFlags> = Cell::new(Line3DFlags::empty());
    }
    let mut include_nan = INCLUDE_NAN.with(|c| c.get());
    let mut flags = FLAGS.with(|f| f.get());
    ui.checkbox("Include NaN", &mut include_nan);
    ui.same_line();
    ui.checkbox_flags("Skip NaN", &mut flags, Line3DFlags::SKIP_NAN);
    INCLUDE_NAN.with(|c| c.set(include_nan));
    FLAGS.with(|f| f.set(flags));

    let mut xs = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let mut ys = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let mut zs = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    if include_nan {
        xs[2] = f32::NAN;
    }

    if let Some(_tok) = plot_ui.begin_plot("##NaNValues").build() {
        set_next_marker_style(
            Marker3D::Square,
            6.0,
            get_colormap_color(0),
            -1.0,
            get_colormap_color(0),
        );
        Line3D::f32("Line", &xs, &ys, &zs)
            .flags(flags)
            .plot(plot_ui);
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
// Tools toggles and auxiliary windows (metrics, style editors, demos)
fn demo_tools(ui: &Ui) {
    thread_local! {
        static SHOW_IP3D_METRICS: Cell<bool> = Cell::new(false);
        static SHOW_IP3D_STYLE: Cell<bool> = Cell::new(false);
        static SHOW_IMGUI_METRICS: Cell<bool> = Cell::new(false);
        static SHOW_IMGUI_STYLE: Cell<bool> = Cell::new(false);
        static SHOW_IMGUI_DEMO: Cell<bool> = Cell::new(false);
    }

    let mut ip3d_metrics = SHOW_IP3D_METRICS.with(|c| c.get());
    let mut ip3d_style = SHOW_IP3D_STYLE.with(|c| c.get());
    let mut imgui_metrics = SHOW_IMGUI_METRICS.with(|c| c.get());
    let mut imgui_style = SHOW_IMGUI_STYLE.with(|c| c.get());
    let mut imgui_demo = SHOW_IMGUI_DEMO.with(|c| c.get());

    ui.checkbox("ImPlot3D Metrics", &mut ip3d_metrics);
    ui.same_line();
    ui.checkbox("ImPlot3D Style Editor", &mut ip3d_style);
    ui.same_line();
    ui.checkbox("ImGui Demo", &mut imgui_demo);
    ui.same_line();
    ui.checkbox("ImGui Metrics", &mut imgui_metrics);
    ui.same_line();
    ui.checkbox("ImGui Style Editor", &mut imgui_style);

    SHOW_IP3D_METRICS.with(|c| c.set(ip3d_metrics));
    SHOW_IP3D_STYLE.with(|c| c.set(ip3d_style));
    SHOW_IMGUI_METRICS.with(|c| c.set(imgui_metrics));
    SHOW_IMGUI_STYLE.with(|c| c.set(imgui_style));
    SHOW_IMGUI_DEMO.with(|c| c.set(imgui_demo));

    if ip3d_metrics {
        implot3d::show_metrics_window();
    }
    if ip3d_style {
        ui.window("Style Editor (ImPlot3D)").build(|| {
            implot3d::show_style_editor();
        });
    }
    if imgui_metrics {
        let mut opened = true;
        ui.show_metrics_window(&mut opened);
        if !opened {
            SHOW_IMGUI_METRICS.with(|c| c.set(false));
        }
    }
    if imgui_style {
        ui.window("Style Editor (ImGui)")
            .build(|| ui.show_default_style_editor());
    }
    if imgui_demo {
        let mut opened = true;
        ui.show_demo_window(&mut opened);
        if !opened {
            SHOW_IMGUI_DEMO.with(|c| c.set(false));
        }
    }
}
