use dear_file_browser::{DialogMode, FileDialogExt, FileDialogState};
use dear_imgui_rs::Context;

/// Basic smoke test: create an ImGui context, open a file browser window for a
/// single frame, and ensure it can render without panicking.
#[test]
fn imgui_file_browser_smoke_test() {
    let mut imgui = Context::create();

    {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }

    let _ = imgui.font_atlas_mut().build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);

    let ui = imgui.frame();

    let mut state = FileDialogState::new(DialogMode::OpenFile);
    state.ui.visible = true;

    let _ = ui.file_browser().show(&mut state);
}
