use dear_imgui_rs as imgui;

fn prepare_imgui(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    io.set_backend_flags(io.backend_flags() | imgui::BackendFlags::RENDERER_HAS_TEXTURES);
}

#[test]
fn focus_scope_accepts_typed_imgui_id_and_restores_previous_scope() {
    let mut ctx = imgui::Context::create();
    prepare_imgui(&mut ctx);

    let ui = ctx.frame();
    let scope_id = ui.get_id("typed focus scope");

    ui.window("focus scope host").build(|| {
        let previous_scope = unsafe { imgui::sys::igGetCurrentFocusScope() };

        {
            let _scope = ui.push_focus_scope(scope_id);
            assert_eq!(
                unsafe { imgui::sys::igGetCurrentFocusScope() },
                scope_id.raw()
            );
        }

        assert_eq!(
            unsafe { imgui::sys::igGetCurrentFocusScope() },
            previous_scope
        );
    });
}
