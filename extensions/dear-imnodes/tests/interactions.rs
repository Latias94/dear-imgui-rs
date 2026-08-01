use dear_imgui_rs::{Condition, Context, MouseButton};
use dear_imnodes as imnodes;
use dear_imnodes::ImNodesExt;
use std::sync::{Mutex, OnceLock};

const EDITOR_WINDOW: &str = "dear-imnodes interaction test";
const NODE_A: imnodes::NodeId = imnodes::NodeId::new(1);
const NODE_B: imnodes::NodeId = imnodes::NodeId::new(2);

#[derive(Default)]
struct EditorFrame {
    hovered: bool,
    selected_nodes: Vec<imnodes::NodeId>,
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    let io = imgui.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    let _ = imgui.font_atlas().build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);
}

fn queue_mouse(imgui: &mut Context, position: [f32; 2], button: Option<(MouseButton, bool)>) {
    let io = imgui.io_mut();
    io.add_mouse_pos_event(position);
    if let Some((button, down)) = button {
        io.add_mouse_button_event(button, down);
    }
}

fn draw_editor_frame(
    imgui: &mut Context,
    nodes: &imnodes::Context,
    editor: &imnodes::EditorContext,
    node_positions: Option<[[f32; 2]; 2]>,
) -> EditorFrame {
    let mut frame = EditorFrame::default();
    {
        let ui = imgui.frame();
        ui.window(EDITOR_WINDOW)
            .position([0.0, 0.0], Condition::Always)
            .size([800.0, 600.0], Condition::Always)
            .build(|| {
                let editor_setup = ui.imnodes(nodes).editor(Some(editor));

                if let Some([node_a, node_b]) = node_positions {
                    editor_setup.set_node_pos_screen(NODE_A, node_a);
                    editor_setup.set_node_pos_screen(NODE_B, node_b);
                }

                let editor_ui = editor_setup.begin_nodes();

                let node_a = editor_ui.node(NODE_A);
                node_a.title_bar(|| ui.text("Node A"));
                ui.text("First node");
                node_a.end();

                let node_b = editor_ui.node(NODE_B);
                node_b.title_bar(|| ui.text("Node B"));
                ui.text("Second node");
                node_b.end();

                let post = editor_ui.end();
                frame.hovered = post.is_editor_hovered();
                frame.selected_nodes = post.selected_nodes();
            });
    }
    assert!(imgui.end_frame());
    frame
}

fn assert_vec2_eq(actual: [f32; 2], expected: [f32; 2]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "expected {expected}, got {actual}"
        );
    }
}

#[test]
fn mouse_focus_and_middle_drag_update_the_bound_editor() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = imnodes::Context::create(&imgui);
    let editor = nodes.create_editor_context();
    let bound_editor = nodes.bind_editor(&editor);

    let _ = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    bound_editor.reset_panning([0.0, 0.0]);

    queue_mouse(
        &mut imgui,
        [650.0, 450.0],
        Some((MouseButton::Middle, true)),
    );
    let press = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert!(press.hovered);

    queue_mouse(&mut imgui, [690.0, 480.0], None);
    let drag = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert!(drag.hovered);
    assert_vec2_eq(bound_editor.get_panning(), [40.0, 30.0]);

    queue_mouse(
        &mut imgui,
        [690.0, 480.0],
        Some((MouseButton::Middle, false)),
    );
    let release = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert!(release.hovered);
    assert_vec2_eq(bound_editor.get_panning(), [40.0, 30.0]);
}

#[test]
fn box_selection_tracks_nodes_in_the_active_editor_context() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = imnodes::Context::create(&imgui);
    let editor = nodes.create_editor_context();
    let bound_editor = nodes.bind_editor(&editor);

    let _ = draw_editor_frame(
        &mut imgui,
        &nodes,
        &editor,
        Some([[200.0, 200.0], [500.0, 200.0]]),
    );
    bound_editor.reset_panning([0.0, 0.0]);

    queue_mouse(&mut imgui, [140.0, 140.0], Some((MouseButton::Left, true)));
    let press = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert!(press.hovered);
    assert!(press.selected_nodes.is_empty());

    queue_mouse(&mut imgui, [340.0, 340.0], None);
    let drag = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert_eq!(drag.selected_nodes, vec![NODE_A]);

    queue_mouse(&mut imgui, [340.0, 340.0], Some((MouseButton::Left, false)));
    let release = draw_editor_frame(&mut imgui, &nodes, &editor, None);
    assert_eq!(release.selected_nodes, vec![NODE_A]);
}
