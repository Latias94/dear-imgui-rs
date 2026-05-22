use dear_imgui_rs::{Context, FramePrepareOptions};
use dear_imguizmo::{DrawListTarget, GuizmoExt, Mat4Like};
use dear_imnodes::ImNodesExt;
use dear_implot::ImPlotExt;
use dear_node_editor::NodeEditorUiExt;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut Context) {
    ctx.prepare_frame(FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0).renderer_has_textures());
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn extensions_compose_inside_one_engine_managed_imgui_frame() {
    let _guard = test_guard();

    let mut imgui = Context::create();
    prepare_context(&mut imgui);

    let plot_ctx = dear_implot::PlotContext::create(&imgui);
    let imnodes_ctx = dear_imnodes::Context::create(&imgui);
    let imnodes_editor = imnodes_ctx.create_editor_context();
    let node_editor_ctx = dear_node_editor::EditorContext::create(&imgui);

    let frame = imgui.begin_frame();
    let ui = frame.ui();

    let plot_ui = ui.implot(&plot_ctx);
    if let Some(plot) = plot_ui.begin_plot("bevy-shared-frame-plot") {
        plot_ui.plot_line("line", &[0.0, 1.0, 2.0], &[0.0, 1.0, 0.0]);
        plot.end();
    }

    let imnodes_ui = ui.imnodes(&imnodes_ctx);
    let imnodes = imnodes_ui.editor(Some(&imnodes_editor));
    let output = dear_imnodes::PinId::new(11);
    let input = dear_imnodes::PinId::new(21);
    {
        let node = imnodes.node(dear_imnodes::NodeId::new(1));
        node.title_bar(|| ui.text("Source"));
        drop(imnodes.output_attr(output, dear_imnodes::PinShape::CircleFilled));
    }
    {
        let node = imnodes.node(dear_imnodes::NodeId::new(2));
        node.title_bar(|| ui.text("Target"));
        drop(imnodes.input_attr(input, dear_imnodes::PinShape::CircleFilled));
    }
    imnodes.link(dear_imnodes::LinkId::new(100), output, input);
    let _ = imnodes.end();

    let node_editor = ui.node_editor(
        &node_editor_ctx,
        "bevy-shared-frame-node-editor",
        [240.0, 160.0],
    );
    let node_a = dear_node_editor::NodeId::new(1);
    let node_b = dear_node_editor::NodeId::new(2);
    let pin_a = dear_node_editor::PinId::new(11);
    let pin_b = dear_node_editor::PinId::new(21);
    node_editor.node(node_a, |node| {
        node.pin(pin_a, dear_node_editor::PinKind::Output, |_| ui.text("out"));
    });
    node_editor.node(node_b, |node| {
        node.pin(pin_b, dear_node_editor::PinKind::Input, |_| ui.text("in"));
    });
    let _ = node_editor.link(dear_node_editor::LinkId::new(100), pin_a, pin_b);
    node_editor.end();

    let gizmo = ui.guizmo();
    gizmo.set_drawlist(DrawListTarget::Foreground);
    gizmo.set_rect(0.0, 0.0, 800.0, 600.0);
    gizmo.set_orthographic(false);
    let view = <[f32; 16] as Mat4Like>::identity();
    let projection = <[f32; 16] as Mat4Like>::identity();
    let model = <[f32; 16] as Mat4Like>::identity();
    gizmo.draw_grid(&view, &projection, &model, 10.0);

    let snapshot = frame
        .render_snapshot(dear_imgui_rs::render::snapshot::SnapshotOptions::default())
        .expect("shared extension frame should snapshot cleanly");

    assert!(
        snapshot
            .draw
            .draw_lists
            .iter()
            .any(|list| !list.vtx.is_empty() && !list.idx.is_empty()),
        "extension composition should produce draw geometry in the shared frame"
    );
}
