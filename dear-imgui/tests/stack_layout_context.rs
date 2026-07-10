#![cfg(feature = "stack-layout")]

use dear_imgui_rs as imgui;

#[test]
fn context_drop_releases_native_stack_layout_state() {
    let baseline = unsafe { imgui::sys::ImGuiStack_StateCount() };
    let mut context = imgui::Context::create();
    context.io_mut().set_display_size([320.0, 240.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let _ = context.font_atlas_mut().build();

    let ui = context.frame();
    let _ = ui.window("stack-layout context state").build(|| {
        let layout = ui.begin_horizontal("row", [0.0, 0.0], -1.0);
        ui.text("item");
        layout.end();
    });
    let _ = context.render();

    assert_eq!(unsafe { imgui::sys::ImGuiStack_StateCount() }, baseline + 1);

    drop(context);

    assert_eq!(unsafe { imgui::sys::ImGuiStack_StateCount() }, baseline);
}
