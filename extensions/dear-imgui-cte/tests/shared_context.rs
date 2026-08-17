use dear_imgui_cte::{CteUiExt, NotificationType, Notifications, TextDiff, TextEditor};
use dear_imgui_rs::{Condition, Context, FramePrepareOptions, sys};
use dear_imnodes::{ImNodesExt, NodeId};
use dear_implot::PlotContext;
use std::time::Duration;

#[test]
fn cte_core_implot_and_imnodes_share_one_imgui_context() {
    let mut context = Context::create();
    context.prepare_frame(FramePrepareOptions::new([960.0, 720.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless integration test requires the legacy font-atlas capability")
        .build();

    let plot_context = PlotContext::create(&context);
    let nodes_context = dear_imnodes::Context::create(&context);
    let node_editor = nodes_context.create_editor_context();
    let mut editor = TextEditor::create(&context);
    let mut diff = TextDiff::create(&context);
    let mut notifications = Notifications::create(&context);
    editor.set_text("int answer = 42;").unwrap();
    diff.set_text("answer = 41", "answer = 42").unwrap();
    notifications
        .add(
            NotificationType::Info,
            "shared context",
            Duration::from_secs(1),
        )
        .unwrap();

    let raw_context = context.as_raw();
    let ui = context.frame();
    ui.window("Shared extension host")
        .size([900.0, 660.0], Condition::Always)
        .build(|| {
            ui.text("Core Dear ImGui widget");
            ui.text_editor(&mut editor, "CTE editor")
                .size([420.0, 120.0])
                .build()
                .unwrap();
            ui.text_diff(&mut diff, "CTE diff")
                .size([420.0, 120.0])
                .build()
                .unwrap();

            let plot_ui = plot_context.get_plot_ui(ui);
            if let Some(plot) = plot_ui.begin_plot("Shared ImPlot") {
                plot_ui.plot_line("values", &[0.0, 1.0, 2.0], &[1.0, 3.0, 2.0]);
                plot.end();
            }

            let nodes = ui
                .imnodes(&nodes_context)
                .editor(Some(&node_editor))
                .begin_nodes();
            nodes.node(NodeId::new(1)).end();
            nodes.end();
        });
    ui.notifications(&mut notifications)
        .position([940.0, 700.0])
        .build()
        .unwrap();

    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_context);
    let draw_data = context.render_legacy();
    assert!(draw_data.total_vtx_count() > 0);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_context);
}
