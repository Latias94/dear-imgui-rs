use dear_imgui_rs::Context;
use dear_imnodes as imnodes;
use dear_imnodes::ImNodesExt;

/// Basic smoke test: create a context + editor, run one frame, and perform a few
/// high-level operations (begin/end editor, create/save/load state, selection helpers).
#[test]
fn basic_editor_smoke_test() {
    let mut imgui = Context::create();
    let imnodes_ctx = imnodes::ImNodesContext::create(&imgui);

    // One editor context for this test
    let editor = imnodes_ctx.create_editor_context();

    {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }

    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);

    let frame = imgui.begin_frame();

    let node_id = imnodes::NodeId::new(1);
    let post = frame
        .ui()
        .imnodes(&imnodes_ctx)
        .editor(Some(&editor))
        .with_nodes(|editor_ui| {
            editor_ui.node(node_id, |_| {});
        });

    // Exercise some post-editor helpers; we don't assert contents, just ensure
    // the calls don't panic and respect basic contracts.
    let _ini = post.save_state_to_ini_string();
    #[cfg(not(target_arch = "wasm32"))]
    {
        post.save_state_to_ini_file("imnodes_test.ini");
        post.load_state_from_ini_file("imnodes_test.ini");
    }

    // Selection helpers: these are no-ops but should not crash.
    post.select_node(node_id);
    let _selected_nodes = post.selected_nodes();
    let _selected_links = post.selected_links();

    // Link queries: simply ensure they can be called without panicking.
    let _ = post.is_link_created();
    let _ = post.is_link_created_with_nodes();
    let _ = post.is_link_destroyed();
}
