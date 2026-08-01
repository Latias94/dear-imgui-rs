use dear_imgui_rs::{Context, FramePrepareOptions};
use dear_imnodes::{ImNodesExt, NodeId, PinId, PinShape};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    imgui.prepare_frame(FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0));
    let _ = imgui.font_atlas().build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn queued_unsubmitted_node_options_do_not_touch_native_node_bookkeeping() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("queued options")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let setup = ui.imnodes(&nodes).editor(Some(&editor_context));
            setup.set_node_pos_grid(NodeId::new(900), [120.0, 80.0]);
            setup.set_node_draggable(NodeId::new(900), false);
            setup.snap_node_to_grid(NodeId::new(900));

            // No native setter is invoked because node 900 is never submitted.
            let _ = setup.begin_nodes().end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn editor_scopes_own_their_explicit_editor_context_lease() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);

    let editor_context = nodes.create_editor_context();
    let bound = nodes.bind_editor(&editor_context);
    drop(editor_context);
    assert_eq!(bound.get_panning(), [0.0, 0.0]);
    drop(bound);

    let editor_context = nodes.create_editor_context();
    let ui = imgui.frame();
    ui.window("owned editor lease")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let setup = ui.imnodes(&nodes).editor(Some(&editor_context));
            drop(editor_context);

            let editor = setup.begin_nodes();
            let node = editor.node(NodeId::new(1));
            node.end();
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn native_scope_and_duplicate_ids_are_rejected_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("scope contract")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let editor = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            let node = editor.node(NodeId::new(1));

            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _second = editor.node(NodeId::new(2));
                }))
                .is_err()
            );

            let input = node.input_attr(PinId::new(11), PinShape::CircleFilled);
            input.end();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _duplicate = node.input_attr(PinId::new(11), PinShape::CircleFilled);
                }))
                .is_err()
            );
            node.end();
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn forgotten_node_token_is_closed_before_explicit_editor_end() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("forgotten token")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let editor = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            let node = editor.node(NodeId::new(1));
            std::mem::forget(node);

            // `end` repairs the native node scope instead of delegating the assertion to C++.
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn forgotten_attribute_token_is_closed_before_its_node() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("forgotten attribute")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let editor = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            let node = editor.node(NodeId::new(1));
            let input = node.input_attr(PinId::new(11), PinShape::CircleFilled);
            std::mem::forget(input);

            // Ending the node first closes the forgotten attribute scope.
            node.end();
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn only_one_editor_frame_can_be_active_per_imnodes_context() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("active frame lease")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let first = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _second = ui
                        .imnodes(&nodes)
                        .editor(Some(&editor_context))
                        .begin_nodes();
                }))
                .is_err()
            );
            drop(first);

            let second = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            let _ = second.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn minimap_finalization_rejects_later_node_submission() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("minimap finalization")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let editor = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            editor.minimap(0.25, dear_imnodes::MiniMapLocation::TopRight);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _node = editor.node(NodeId::new(1));
                }))
                .is_err()
            );
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}

#[test]
fn invalid_minimap_size_is_rejected_before_finalization_or_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = dear_imnodes::Context::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("invalid minimap")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let editor = ui
                .imnodes(&nodes)
                .editor(Some(&editor_context))
                .begin_nodes();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    editor.minimap(0.0, dear_imnodes::MiniMapLocation::TopRight);
                }))
                .is_err()
            );

            let node = editor.node(NodeId::new(1));
            node.end();
            let _ = editor.end();
        });
    assert!(imgui.end_frame());
}
