use dear_imgui_rs::{Condition, Context, FramePrepareOptions};
use dear_imnodes::{ImNodesContext, ImNodesExt, NodeId};
use dear_imnodes_sys as sys;
use std::sync::Mutex;

static IMGUI: Mutex<()> = Mutex::new(());

const LAYOUT: &str =
    "[editor]\npanning=12,-7\n\n[node.-4]\norigin=40,60\n\n[node.8]\norigin=180,90\n";

#[test]
fn saved_layout_round_trips_and_sections_are_local_to_each_load() {
    let _lock = IMGUI.lock().unwrap_or_else(|e| e.into_inner());
    let imgui = Context::create();
    let nodes = ImNodesContext::create(&imgui);
    let first = nodes.bind_editor(&nodes.create_editor_context());
    let second = nodes.bind_editor(&nodes.create_editor_context());

    first.load_state_from_ini_string(LAYOUT);
    let saved = first.save_state_to_ini_string();
    assert_eq!(saved, LAYOUT);
    second.load_state_from_ini_string(&saved.replace('\n', "\r\n"));
    assert_eq!(second.save_state_to_ini_string(), saved);

    first.load_state_from_ini_string(concat!(
        "[editor]\npanning=24,-3\n",
        "[view]\npanning=16,16\n",
        "[node.8]\norigin=220,100\n",
        "[view]\norigin=32,32\n",
        "[node.8.preview]\norigin=16,16\n",
    ));
    // An application's preview section is independent of both editors' last node sections.
    second.load_state_from_ini_string("[node.preview]\norigin=16,16\n");
    assert_eq!(
        first.save_state_to_ini_string(),
        saved.replace("12,-7", "24,-3").replace("180,90", "220,100")
    );
    assert_eq!(second.save_state_to_ini_string(), saved);
}

#[test]
fn explicit_native_load_restores_the_current_editor_and_draws_the_target_graph() {
    let _lock = IMGUI.lock().unwrap_or_else(|e| e.into_inner());
    let mut imgui = Context::create();
    imgui.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    imgui.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    let nodes = ImNodesContext::create(&imgui);

    // The public native loader also accepts a target other than the current editor.
    // Restore the default and release our temporary editor before checking the results.
    let (restored, panning) = unsafe {
        sys::imnodes_SetCurrentContext(nodes.as_raw());
        let target = sys::imnodes_EditorContextGetCurrent();
        let current = sys::imnodes_EditorContextCreate();
        sys::imnodes_EditorContextSet(current);
        sys::imnodes_EditorContextResetPanning(sys::ImVec2_c { x: 3.0, y: 5.0 });
        sys::imnodes_LoadEditorStateFromIniString(target, LAYOUT.as_ptr().cast(), LAYOUT.len());
        let restored = sys::imnodes_EditorContextGetCurrent() == current;
        let panning = sys::imnodes_EditorContextGetPanning();
        sys::imnodes_EditorContextSet(target);
        sys::imnodes_EditorContextFree(current);
        (restored, [panning.x, panning.y])
    };
    assert!(restored);
    assert_eq!(panning, [3.0, 5.0]);

    // Rendering the restored graph uses the target editor's native node depth order.
    let ui = imgui.frame();
    let saved = ui
        .window("Restored graph")
        .size([500.0, 350.0], Condition::Always)
        .build(|| {
            let post = ui.imnodes_editor(&nodes, None).with_nodes(|editor| {
                editor.node(NodeId::new(-4), |_| ui.text("Source"));
                editor.node(NodeId::new(8), |_| ui.text("Target"));
            });
            post.save_state_to_ini_string()
        })
        .unwrap();
    assert!(imgui.end_frame());
    assert_eq!(saved, LAYOUT);
}
