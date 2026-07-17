use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::{BackendFlags, Context, ManagedTextureId, TextureId};
use dear_implot3d::Plot3DContext;

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
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
}

fn register_texture(imgui: &mut Context) -> ManagedTextureId {
    let mut texture = dear_imgui_rs::texture::OwnedTextureData::new();
    texture.create(dear_imgui_rs::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 255, 255, 255]);
    imgui.register_texture(texture)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
        })
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[test]
fn managed_and_legacy_images_resolve_in_the_owner_context() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let managed = register_texture(&mut imgui);
    let plot = Plot3DContext::create(&imgui);

    {
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(ui);
        let _plot = plot_ui
            .begin_plot("owner images")
            .build()
            .expect("plot should begin");
        plot_ui
            .image_by_axes(
                "managed",
                managed,
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .plot();
        plot_ui
            .image_by_axes(
                "legacy",
                TextureId::new(17),
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .plot();
    }

    let _ = imgui.render();
    drop(plot);
}

#[test]
fn managed_image_rejects_a_foreign_context_before_plot_ffi() {
    let _guard = test_guard();
    let mut owner = Context::create();
    let managed = register_texture(&mut owner);
    let owner = owner.suspend();

    let mut foreign = Context::create();
    prepare_imgui(&mut foreign);
    let plot = Plot3DContext::create(&foreign);
    {
        let ui = foreign.frame();
        let plot_ui = plot.get_plot_ui(ui);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _plot = plot_ui
                .begin_plot("foreign image")
                .build()
                .expect("plot should begin");
            plot_ui
                .image_by_axes(
                    "foreign",
                    managed,
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .plot();
        }));
        let message = panic_message(result.expect_err("foreign texture should be rejected"));
        assert!(
            message.contains("belongs to Context"),
            "unexpected panic: {message}"
        );
    }
    let _ = foreign.render();
    drop(plot);
    drop(foreign);

    let _owner = owner
        .activate()
        .unwrap_or_else(|_| panic!("owner context should reactivate"));
}

#[test]
fn managed_image_rejects_a_stale_generation_before_plot_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let stale = register_texture(&mut imgui);
    imgui
        .remove_texture(stale)
        .expect("unsubmitted texture should retire immediately");
    let replacement = register_texture(&mut imgui);
    assert_ne!(stale, replacement);
    let plot = Plot3DContext::create(&imgui);

    {
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(ui);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _plot = plot_ui
                .begin_plot("stale image")
                .build()
                .expect("plot should begin");
            plot_ui
                .image_by_axes(
                    "stale",
                    stale,
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                )
                .plot();
        }));
        let message = panic_message(result.expect_err("stale texture should be rejected"));
        assert!(
            message.contains("stale generation"),
            "unexpected panic: {message}"
        );
    }
    let _ = imgui.render();
    drop(plot);
}
