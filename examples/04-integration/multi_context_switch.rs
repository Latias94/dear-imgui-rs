use dear_imgui_rs as imgui;
use std::path::PathBuf;

fn prepare_headless_context(
    ctx: &mut imgui::Context,
    display_size: [f32; 2],
) -> imgui::ImGuiResult<()> {
    ctx.set_ini_filename::<PathBuf>(None)?;
    // This example only proves Context activation and suspension. It intentionally uses the
    // legacy headless font-atlas path instead of advertising a renderer capability it does not
    // provide.
    ctx.prepare_frame(imgui::FramePrepareOptions::new(display_size, 1.0 / 60.0));
    let _ = ctx.font_atlas().build();
    Ok(())
}

fn main() -> imgui::ImGuiResult<()> {
    let mut context_a = imgui::Context::create();
    prepare_headless_context(&mut context_a, [640.0, 360.0])?;

    let suspended_a = context_a.suspend();

    let mut context_b = imgui::Context::create();
    prepare_headless_context(&mut context_b, [320.0, 180.0])?;

    let frame_b = context_b.begin_frame();
    frame_b.ui().text("Context B can render its own frame.");
    drop(frame_b.render_legacy());

    let suspended_b = context_b.suspend();
    let mut context_a = suspended_a
        .activate()
        .expect("context B was suspended, so context A can be activated");

    let frame = context_a.begin_frame();
    let ui = frame.ui();

    ui.text("Context A owns this frame after context B was suspended.");

    {
        let _alpha = ui.push_style_var(imgui::StyleVar::Alpha(0.5));
        ui.text("This style stack entry belongs to context A.");
    }

    drop(frame.render_legacy());

    drop(context_a);
    drop(suspended_b);

    println!("multi-context switch example completed");
    Ok(())
}
