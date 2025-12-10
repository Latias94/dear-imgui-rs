use dear_imgui_rs::Context;
use dear_imnodes as imnodes;
use dear_imnodes::ImNodesExt;

/// Basic smoke test: create a context + editor, run one frame, and perform a few
/// high-level operations (begin/end editor, create/save/load state, selection helpers).
#[test]
fn basic_editor_smoke_test() {
    let mut imgui = Context::create();
    let imnodes_ctx = imnodes::Context::create(&imgui);

    // One editor context for this test
    let editor = imnodes::EditorContext::create();

    {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }

    let _ = imgui.font_atlas_mut().build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);

    let ui = imgui.frame();

    // Begin an editor scope and end it explicitly
    let editor_ui = ui.imnodes(&imnodes_ctx).editor(Some(&editor));

    // Create a trivial node (no actual widgets, just exercise Begin/End)
    let node_id: i32 = 1;
    let node = editor_ui.node(node_id);
    // Normally you'd draw title and attributes here using NodeToken helpers.
    // Explicitly end the node so the borrow ends before we end the editor.
    node.end();

    let post = editor_ui.end();

    // Exercise some post-editor helpers; we don't assert contents, just ensure
    // the calls don't panic and respect basic contracts.
    let _ini = post.save_state_to_ini_string();
    post.save_state_to_ini_file("imnodes_test.ini");
    post.load_state_from_ini_file("imnodes_test.ini");

    // Selection helpers: these are no-ops but should not crash.
    post.select_node(node_id);
    let _selected_nodes = post.selected_nodes();
    let _selected_links = post.selected_links();

    // Link queries: simply ensure they can be called without panicking.
    let _ = post.is_link_created();
    let _ = post.is_link_created_with_nodes();
    let _ = post.is_link_destroyed();
}
