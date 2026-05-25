//! Extension ecosystem composition inside one Bevy-managed Dear ImGui frame.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example ecosystem`

use bevy::{
    app::AppExit,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
    render::ImguiOverlayCamera,
};
use dear_imgui_rs::{Condition, DockBuilder, DockNodeFlags, SplitDirection};
use dear_imguizmo::{DrawListTarget, GuizmoExt, Mat4Like};
use dear_imnodes::ImNodesExt;
use dear_implot::ImPlotExt;

struct EcosystemContexts {
    plot: dear_implot::PlotContext,
    nodes: dear_imnodes::Context,
    node_editor: dear_imnodes::EditorContext,
}

#[derive(Resource, Debug)]
struct EcosystemState {
    sample_time: [f64; 16],
    cpu_ms: [f64; 16],
    gpu_ms: [f64; 16],
    frame_index: u64,
    dockspace_seeded: bool,
    graph_positions_initialized: bool,
}

impl Default for EcosystemState {
    fn default() -> Self {
        Self {
            sample_time: [
                0.0, 0.08, 0.16, 0.24, 0.32, 0.40, 0.48, 0.56, 0.64, 0.72, 0.80, 0.88, 0.96, 1.04,
                1.12, 1.20,
            ],
            cpu_ms: [
                3.2, 3.7, 4.4, 4.1, 3.9, 4.8, 4.2, 3.8, 4.6, 5.1, 4.7, 4.0, 3.6, 4.3, 4.9, 4.1,
            ],
            gpu_ms: [
                5.1, 5.4, 6.2, 5.8, 5.6, 6.5, 6.1, 5.7, 6.8, 7.3, 6.9, 6.0, 5.5, 6.3, 7.0, 6.4,
            ],
            frame_index: 0,
            dockspace_seeded: false,
            graph_positions_initialized: false,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy ecosystem".to_owned(),
            resolution: (1280, 720).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(ImguiPlugin::default())
    .init_resource::<EcosystemState>()
    .add_systems(Startup, setup_scene)
    .add_systems(Update, close_on_escape)
    .add_systems(ImguiPrimaryContextPass, ecosystem_ui);
    install_ecosystem_contexts(&mut app);
    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((Camera2d, ImguiOverlayCamera));
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn install_ecosystem_contexts(app: &mut App) {
    let contexts = {
        let mut imgui = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiPlugin should install ImguiContext before examples create extensions");
        configure_example_context(&mut imgui, false);
        let context = imgui.context_mut();

        let plot = dear_implot::PlotContext::create(context);
        let nodes = dear_imnodes::Context::create(context);
        let node_editor = nodes.create_editor_context();
        EcosystemContexts {
            plot,
            nodes,
            node_editor,
        }
    };
    app.world_mut().insert_non_send(contexts);
}

fn ecosystem_ui(
    mut contexts: ImguiContexts,
    extensions: NonSend<EcosystemContexts>,
    mut state: ResMut<EcosystemState>,
) {
    let frame_index = contexts.frame_index().unwrap_or_default();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };
    state.frame_index = frame_index;

    let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
        ui.get_id("DearImguiBevyEcosystemDockspace"),
        DockNodeFlags::NONE,
    );
    seed_ecosystem_dockspace(ui, dockspace_id, &mut state.dockspace_seeded);

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Profiler Plot")
        .size([520.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            render_profiler_plot(ui, &extensions.plot, &state);
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Frame Graph")
        .size([520.0, 560.0], Condition::FirstUseEver)
        .build(|| {
            render_frame_graph(
                ui,
                &extensions.nodes,
                &extensions.node_editor,
                &mut state.graph_positions_initialized,
            );
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Gizmo")
        .size([520.0, 260.0], Condition::FirstUseEver)
        .build(|| {
            render_gizmo(ui, state.frame_index);
        });
}

fn seed_ecosystem_dockspace(
    ui: &dear_imgui_rs::Ui,
    dockspace_id: dear_imgui_rs::Id,
    seeded: &mut bool,
) {
    if *seeded {
        return;
    }

    let viewport = ui.main_viewport();
    DockBuilder::remove_node(dockspace_id);
    let root = DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
    DockBuilder::set_node_pos(root, viewport.pos());
    DockBuilder::set_node_size(root, viewport.size());
    let (left_id, graph_id) = DockBuilder::split_node(root, SplitDirection::Left, 0.42);
    let (plot_id, gizmo_id) = DockBuilder::split_node(left_id, SplitDirection::Up, 0.48);
    DockBuilder::dock_window("Profiler Plot", plot_id);
    DockBuilder::dock_window("Gizmo", gizmo_id);
    DockBuilder::dock_window("Frame Graph", graph_id);
    DockBuilder::finish(root);

    *seeded = true;
}

fn render_profiler_plot(
    ui: &dear_imgui_rs::Ui,
    plot_context: &dear_implot::PlotContext,
    state: &EcosystemState,
) {
    ui.text("ImPlot uses the same Ui exposed by ImguiPrimaryContextPass.");
    let plot_ui = ui.implot(plot_context);
    if let Some(plot) = plot_ui.begin_plot_with_size("Frame timing", [-1.0, 230.0]) {
        plot_ui.plot_line("cpu ms", &state.sample_time, &state.cpu_ms);
        plot_ui.plot_line("gpu ms", &state.sample_time, &state.gpu_ms);
        plot.end();
    }
}

fn render_frame_graph(
    ui: &dear_imgui_rs::Ui,
    nodes_context: &dear_imnodes::Context,
    node_editor_context: &dear_imnodes::EditorContext,
    positions_initialized: &mut bool,
) {
    ui.text("ImNodes shares the Bevy-managed frame.");
    let nodes_ui = ui.imnodes(nodes_context);
    let editor = nodes_ui.editor(Some(node_editor_context));
    let source_output = dear_imnodes::PinId::new(11);
    let pass_input = dear_imnodes::PinId::new(21);
    let pass_output = dear_imnodes::PinId::new(22);
    let sink_input = dear_imnodes::PinId::new(31);
    if !*positions_initialized {
        editor.set_node_pos_grid(dear_imnodes::NodeId::new(1), [48.0, 80.0]);
        editor.set_node_pos_grid(dear_imnodes::NodeId::new(2), [180.0, 230.0]);
        editor.set_node_pos_grid(dear_imnodes::NodeId::new(3), [64.0, 360.0]);
        *positions_initialized = true;
    }

    render_graph_source(ui, &editor, source_output);
    render_graph_pass(ui, &editor, pass_input, pass_output);
    render_graph_sink(ui, &editor, sink_input);

    editor.link(dear_imnodes::LinkId::new(100), source_output, pass_input);
    editor.link(dear_imnodes::LinkId::new(101), pass_output, sink_input);
    let _post = editor.end();
}

fn render_graph_source(
    ui: &dear_imgui_rs::Ui,
    editor: &dear_imnodes::NodeEditor<'_>,
    output_id: dear_imnodes::PinId,
) {
    let node = editor.node(dear_imnodes::NodeId::new(1));
    node.title_bar(|| ui.text("Extract"));
    let output = editor.output_attr(output_id, dear_imnodes::PinShape::CircleFilled);
    ui.text("Frame Data");
    output.end();
    node.end();
}

fn render_graph_pass(
    ui: &dear_imgui_rs::Ui,
    editor: &dear_imnodes::NodeEditor<'_>,
    input_id: dear_imnodes::PinId,
    output_id: dear_imnodes::PinId,
) {
    let node = editor.node(dear_imnodes::NodeId::new(2));
    node.title_bar(|| ui.text("Render Pass"));
    let input = editor.input_attr(input_id, dear_imnodes::PinShape::TriangleFilled);
    ui.text("Input");
    input.end();
    let output = editor.output_attr(output_id, dear_imnodes::PinShape::TriangleFilled);
    ui.text("Color Target");
    output.end();
    node.end();
}

fn render_graph_sink(
    ui: &dear_imgui_rs::Ui,
    editor: &dear_imnodes::NodeEditor<'_>,
    input_id: dear_imnodes::PinId,
) {
    let node = editor.node(dear_imnodes::NodeId::new(3));
    node.title_bar(|| ui.text("Present"));
    let input = editor.input_attr(input_id, dear_imnodes::PinShape::QuadFilled);
    ui.text("Swapchain");
    input.end();
    node.end();
}

fn render_gizmo(ui: &dear_imgui_rs::Ui, frame_index: u64) {
    ui.text("ImGuizmo also draws into the same Bevy-managed frame.");
    let available = ui.content_region_avail();
    let size = [available[0].max(420.0), available[1].max(240.0)];
    let pos = ui.cursor_screen_pos();
    let draw_list = ui.get_window_draw_list();
    draw_list
        .add_rect(
            pos,
            [pos[0] + size[0], pos[1] + size[1]],
            [0.07, 0.09, 0.12, 1.0],
        )
        .filled(true)
        .build();
    draw_list
        .add_rect(
            [pos[0] + 16.0, pos[1] + 16.0],
            [pos[0] + size[0] - 16.0, pos[1] + size[1] - 16.0],
            [0.22, 0.36, 0.52, 1.0],
        )
        .thickness(2.0)
        .build();
    let center = [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.54];
    let angle = (frame_index as f32) * 0.05;
    let half = [96.0, 58.0];
    let corners = [
        [-half[0], -half[1]],
        [half[0], -half[1]],
        [half[0], half[1]],
        [-half[0], half[1]],
    ];
    let rotated = corners.map(|p| {
        [
            center[0] + p[0] * angle.cos() - p[1] * angle.sin(),
            center[1] + p[0] * angle.sin() + p[1] * angle.cos(),
        ]
    });
    for pair in rotated.windows(2) {
        draw_list
            .add_line(pair[0], pair[1], [0.95, 0.78, 0.28, 1.0])
            .thickness(4.0)
            .build();
    }
    draw_list
        .add_line(rotated[3], rotated[0], [0.95, 0.78, 0.28, 1.0])
        .thickness(4.0)
        .build();
    draw_list
        .add_circle(center, 8.0, [0.28, 0.86, 0.62, 1.0])
        .filled(true)
        .build();
    ui.dummy(size);

    let gizmo = ui.guizmo();
    gizmo.set_drawlist(DrawListTarget::Window);
    gizmo.set_rect(pos[0], pos[1], size[0], size[1]);
    gizmo.set_orthographic(false);

    let view = <[f32; 16] as Mat4Like>::identity();
    let projection = <[f32; 16] as Mat4Like>::identity();
    let model = [
        angle.cos(),
        angle.sin(),
        0.0,
        0.0,
        -angle.sin(),
        angle.cos(),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    gizmo.draw_grid(&view, &projection, &model, 8.0);
}
