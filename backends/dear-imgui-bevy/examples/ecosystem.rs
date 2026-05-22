//! Extension ecosystem composition inside one Bevy-managed Dear ImGui frame.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example ecosystem`

use bevy_app::{App, ScheduleRunnerPlugin, Startup};
use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass};
use dear_imgui_rs::{Condition, ConfigFlags, DockNodeFlags};
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
    sample_time: [f64; 8],
    cpu_ms: [f64; 8],
    gpu_ms: [f64; 8],
    frame_index: u64,
}

impl Default for EcosystemState {
    fn default() -> Self {
        Self {
            sample_time: [0.0, 0.16, 0.33, 0.50, 0.66, 0.83, 1.0, 1.16],
            cpu_ms: [3.2, 3.7, 4.4, 4.1, 3.9, 4.8, 4.2, 3.8],
            gpu_ms: [5.1, 5.4, 6.2, 5.8, 5.6, 6.5, 6.1, 5.7],
            frame_index: 0,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(ScheduleRunnerPlugin::run_once())
        .add_plugins(ImguiPlugin::default())
        .init_resource::<EcosystemState>()
        .add_systems(Startup, setup_window)
        .add_systems(ImguiPrimaryContextPass, ecosystem_ui);
    install_ecosystem_contexts(&mut app);
    app.run();
}

fn setup_window(mut commands: Commands) {
    commands.spawn((
        Window {
            title: "dear-imgui-bevy ecosystem".to_owned(),
            resolution: WindowResolution::new(1280, 720),
            ..Default::default()
        },
        PrimaryWindow,
    ));
}

fn install_ecosystem_contexts(app: &mut App) {
    let contexts = {
        let mut imgui = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiPlugin should install ImguiContext before examples create extensions");
        let context = imgui.context_mut();
        context.io_mut().set_config_input_trickle_event_queue(false);
        let flags = context.io().config_flags() | ConfigFlags::DOCKING_ENABLE;
        context.io_mut().set_config_flags(flags);
        let _ = context.font_atlas_mut().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);

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
        DockNodeFlags::PASSTHRU_CENTRAL_NODE,
    );

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Profiler Plot")
        .size([460.0, 260.0], Condition::FirstUseEver)
        .build(|| {
            render_profiler_plot(ui, &extensions.plot, &state);
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Frame Graph")
        .size([460.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            render_frame_graph(ui, &extensions.nodes, &extensions.node_editor);
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Gizmo")
        .size([420.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            render_gizmo(ui, state.frame_index);
        });
}

fn render_profiler_plot(
    ui: &dear_imgui_rs::Ui,
    plot_context: &dear_implot::PlotContext,
    state: &EcosystemState,
) {
    ui.text("ImPlot uses the same Ui exposed by ImguiPrimaryContextPass.");
    let plot_ui = ui.implot(plot_context);
    if let Some(plot) = plot_ui.begin_plot_with_size("Frame timing", [-1.0, 180.0]) {
        plot_ui.plot_line("cpu ms", &state.sample_time, &state.cpu_ms);
        plot_ui.plot_line("gpu ms", &state.sample_time, &state.gpu_ms);
        plot.end();
    }
}

fn render_frame_graph(
    ui: &dear_imgui_rs::Ui,
    nodes_context: &dear_imnodes::Context,
    node_editor_context: &dear_imnodes::EditorContext,
) {
    ui.text("ImNodes context is stored as Bevy non-send data.");
    let nodes_ui = ui.imnodes(nodes_context);
    let editor = nodes_ui.editor(Some(node_editor_context));
    let source_output = dear_imnodes::PinId::new(11);
    let pass_input = dear_imnodes::PinId::new(21);
    let pass_output = dear_imnodes::PinId::new(22);
    let sink_input = dear_imnodes::PinId::new(31);

    {
        let node = editor.node(dear_imnodes::NodeId::new(1));
        node.title_bar(|| ui.text("Extract"));
        drop(editor.output_attr(source_output, dear_imnodes::PinShape::CircleFilled));
    }
    {
        let node = editor.node(dear_imnodes::NodeId::new(2));
        node.title_bar(|| ui.text("Render Pass"));
        drop(editor.input_attr(pass_input, dear_imnodes::PinShape::TriangleFilled));
        ui.same_line();
        drop(editor.output_attr(pass_output, dear_imnodes::PinShape::TriangleFilled));
    }
    {
        let node = editor.node(dear_imnodes::NodeId::new(3));
        node.title_bar(|| ui.text("Present"));
        drop(editor.input_attr(sink_input, dear_imnodes::PinShape::QuadFilled));
    }

    editor.link(dear_imnodes::LinkId::new(100), source_output, pass_input);
    editor.link(dear_imnodes::LinkId::new(101), pass_output, sink_input);
    let _post = editor.end();
}

fn render_gizmo(ui: &dear_imgui_rs::Ui, frame_index: u64) {
    ui.text("ImGuizmo also draws into the same Bevy-managed frame.");
    let available = ui.content_region_avail();
    let size = [available[0].max(240.0), available[1].max(180.0)];
    let pos = ui.cursor_screen_pos();
    ui.dummy(size);

    let gizmo = ui.guizmo();
    gizmo.set_drawlist(DrawListTarget::Window);
    gizmo.set_rect(pos[0], pos[1], size[0], size[1]);
    gizmo.set_orthographic(false);

    let view = <[f32; 16] as Mat4Like>::identity();
    let projection = <[f32; 16] as Mat4Like>::identity();
    let angle = (frame_index as f32) * 0.05;
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
