use super::*;
use dear_imgui_rs::{
    BackendFlags, Condition, FramePrepareOptions, OwnedTextureData, TextureFormat, TextureId,
    TextureStatus, Ui,
};

#[test]
fn sdlgpu_prepare_retries_failed_texture_on_the_next_native_viewport_frame() {
    const PIXEL: &[u8] = &[12, 34, 56, 255];
    const UPLOADED_ID: TextureId = TextureId::new(77);

    let _guard = crate::tests::test_guard();
    PLATFORM_RENDER_COUNT.set(0);
    PLATFORM_SWAP_COUNT.set(0);
    let mut context = Context::create();
    context.set_ini_filename(None::<&str>).unwrap();
    let attempts = Rc::new(Cell::new(0));
    let mut runtime = registration_with_backend_lifecycle_and_texture_update(
        &mut context,
        Some(Rc::new(|| {})),
        None,
        Some({
            let attempts = Rc::clone(&attempts);
            Rc::new(move |proxy: &mut TextureData| match proxy.status() {
                TextureStatus::WantCreate | TextureStatus::WantUpdates => {
                    let id = if proxy.pixels() == Some(PIXEL) {
                        attempts.set(attempts.get() + 1);
                        if attempts.get() == 1 {
                            return;
                        }
                        UPLOADED_ID
                    } else {
                        TextureId::new(1) // Font atlas
                    };
                    // Only the synthetic renderer observes these proxy handles.
                    unsafe {
                        proxy.set_tex_id(id);
                        proxy.set_status(TextureStatus::OK);
                    }
                }
                TextureStatus::WantDestroy => unsafe { proxy.set_status(TextureStatus::Destroyed) },
                TextureStatus::OK | TextureStatus::Destroyed => {}
            })
        }),
        Rc::new(|| unsafe { sys::igDestroyPlatformWindows() }),
        PlatformGraphicsKind::Other,
        NativeRendererKind::SdlGpu3,
    );
    context.prepare_frame(
        FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0)
            .renderer_has_textures()
            .backend_flags(
                BackendFlags::PLATFORM_HAS_VIEWPORTS | BackendFlags::RENDERER_HAS_VIEWPORTS,
            ),
    );
    context.enable_multi_viewport();
    context.io_mut().set_config_viewports_no_auto_merge(true);
    let screen = sys::ImVec2::new(640.0, 480.0);
    unsafe {
        context
            .platform_io_mut()
            .set_monitors(&[sys::ImGuiPlatformMonitor {
                MainSize: screen,
                WorkSize: screen,
                DpiScale: 1.0,
                ..Default::default()
            }]);
        synthetic_create_window(sys::igGetMainViewport());
        install_synthetic_platform_callback_inventory(
            sys::igGetPlatformIO_Nil(),
            synthetic_create_window,
        );
    }
    claim_synthetic_renderer_callbacks(&context, &mut runtime);
    runtime.install_renderer_consumer(context.create_synchronous_renderer_consumer().unwrap());
    let mut backend = crate::SdlGpu3RendererBackend::from_initialized_context(runtime);
    let target = context.register_texture(
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, PIXEL).unwrap(),
    );
    let draw_target = |ui: &Ui| {
        ui.window("Texture retry")
            .position([200.0, 20.0], Condition::Always)
            .size([80.0, 80.0], Condition::Always)
            .build(|| {
                ui.image(target, [8.0, 8.0]);
                ui.window_viewport().id()
            })
            .expect("test window must be visible")
    };

    let frame = context.begin_frame();
    let viewport_id = draw_target(frame.ui());
    let error = backend
        .prepare(frame)
        .err()
        .expect("first upload must fail");
    assert!(
        matches!(
            error.faults(),
            [Sdl3BackendError::TextureOperationFailed {
                texture: SnapshotTextureId::User(id),
                operation: "upload",
            }] if *id == target
        ),
        "{error:?}"
    );
    assert_eq!(attempts.get(), 1);
    assert_eq!(
        context.with_texture(target, |t| t.texture_id()).unwrap(),
        TextureId::null()
    );
    let secondary = context
        .platform_io()
        .viewports_iter()
        .find(|viewport| viewport.id() == viewport_id && !viewport.is_main())
        .expect("the texture must be drawn in a secondary viewport");
    assert!(
        secondary.platform_window_created(),
        "the failed frame must create the secondary platform window"
    );
    assert_eq!(PLATFORM_RENDER_COUNT.get(), 0);
    assert_eq!(PLATFORM_SWAP_COUNT.get(), 0);

    let frame = context.begin_frame();
    assert_eq!(draw_target(frame.ui()), viewport_id);
    backend
        .prepare(frame)
        .expect("next frame must retry the upload")
        .skip_main();
    assert_eq!(attempts.get(), 2);
    assert_eq!(
        context.with_texture(target, |t| t.texture_id()).unwrap(),
        UPLOADED_ID
    );
    assert_eq!(PLATFORM_RENDER_COUNT.get(), 1);
    assert_eq!(PLATFORM_SWAP_COUNT.get(), 1);
    backend.shutdown(&mut context).unwrap();
}
