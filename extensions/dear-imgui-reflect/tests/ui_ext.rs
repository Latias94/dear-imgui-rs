use dear_imgui_reflect::{ImGuiReflectExt, Inspector, ReflectSession, imgui::Context};

fn inspector_with_independent_borrows<'ui, 'session>(
    ui: &'ui dear_imgui_reflect::imgui::Ui,
    session: &'session ReflectSession,
) -> Inspector<'ui, 'session> {
    ui.inspector(session)
}

#[test]
fn ui_extension_starts_an_empty_reflection_pass() {
    let mut context = Context::create();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);

    let session = ReflectSession::new();
    let ui = context.frame();
    let inspector = inspector_with_independent_borrows(ui, &session);

    assert!(inspector.response().is_empty());
}
