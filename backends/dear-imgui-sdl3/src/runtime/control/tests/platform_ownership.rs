use super::*;

#[test]
fn attachment_teardown_reports_renderer_panic_without_hidden_retry() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let runtime = registration_with_lifecycle(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || {
                let attempt = renderer_count.get() + 1;
                renderer_count.set(attempt);
                if attempt == 1 {
                    panic!("synthetic renderer teardown failure");
                }
            })
        }),
        {
            let platform_count = Rc::clone(&platform_count);
            Rc::new(move || platform_count.set(platform_count.get() + 1))
        },
    );
    runtime.control.platform_initialized.set(true);
    runtime.control.renderer_initialized.set(true);
    let control = Rc::clone(&runtime.control);

    control.begin_shutdown();
    let first = context
        .binding()
        .with_bound_context(|| control.shutdown_bound_for_attachment());
    assert!(matches!(
        first,
        Err(Sdl3BackendError::ShutdownPanicked {
            phase: "renderer resources"
        })
    ));
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(control.phase_log(), ["platform", "renderer"]);
    assert_eq!(control.state(), RuntimeState::ShuttingDown);

    context
        .binding()
        .with_bound_context(|| control.shutdown_bound_for_attachment())
        .unwrap();
    assert_eq!(renderer_count.get(), 2);
    assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
    assert_eq!(control.state(), RuntimeState::Detached);
    drop(context);
    drop(runtime);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
}

#[test]
fn platform_phase_does_not_drop_renderer_global_state() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let runtime = test_registration(
        &mut context,
        Rc::clone(&renderer_count),
        Rc::clone(&platform_count),
    );

    runtime.control.begin_shutdown();
    context
        .binding()
        .with_bound_context(|| runtime.control.release_platform_bound().unwrap());

    assert_eq!(runtime.control.phase_log(), ["platform"]);
    assert_eq!(renderer_count.get(), 0);
    assert_eq!(platform_count.get(), 1);

    let _ = context
        .binding()
        .with_bound_context(|| runtime.control.release_renderer_bound());
    assert_eq!(runtime.control.phase_log(), ["platform", "renderer"]);
    assert_eq!(renderer_count.get(), 1);
}

#[test]
fn owned_callback_state_restores_baseline_without_dangling_native_data() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let baseline_clipboard = context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        (
            (*platform_io).Platform_GetClipboardTextFn,
            (*platform_io).Platform_ClipboardUserData,
        )
    });
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::clone(&observed_backend_data),
        Rc::clone(&observed_main_viewport_data),
        synthetic_create_window,
    );
    let registry_key = runtime.control.platform_io_key.get();
    assert!(registry_contains(registry_key));

    runtime.shutdown_platform(&mut context).unwrap();

    assert_eq!(platform_count.get(), 1);
    assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
    assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
    assert_eq!(runtime.control.platform_io_key.get(), 0);
    assert!(!registry_contains(registry_key));
    assert!(with_current_runtime(|_| ()).is_none());
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        assert!((*io).BackendPlatformUserData.is_null());
        assert!((*io).BackendPlatformName.is_null());
        assert!((*platform_io).Platform_CreateWindow.is_none());
        assert_eq!(
            (*platform_io).Platform_ClipboardUserData,
            baseline_clipboard.1
        );
        match (
            (*platform_io).Platform_GetClipboardTextFn,
            baseline_clipboard.0,
        ) {
            (Some(actual), Some(expected)) => assert!(std::ptr::fn_addr_eq(actual, expected)),
            (None, None) => {}
            _ => panic!("clipboard callback baseline was not restored"),
        }
        assert!((*main_viewport).PlatformUserData.is_null());
        assert!((*main_viewport).PlatformHandle.is_null());
    });
}

#[test]
fn platform_service_override_does_not_revoke_viewport_ownership() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let baseline_clipboard = context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        (
            (*platform_io).Platform_GetClipboardTextFn,
            (*platform_io).Platform_SetClipboardTextFn,
            (*platform_io).Platform_ClipboardUserData,
        )
    });
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );

    context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        (*platform_io).Platform_GetClipboardTextFn = Some(foreign_get_clipboard_text);
        (*platform_io).Platform_SetClipboardTextFn = Some(foreign_set_clipboard_text);
        (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
    });

    let mut viewport = sys::ImGuiViewport::default();
    context.binding().with_bound_context(|| unsafe {
        create_window_callback_for_test(&mut viewport);
        destroy_window_callback_for_test(&mut viewport);
    });
    runtime.poll_fault().unwrap();
    runtime.shutdown_platform(&mut context).unwrap();

    context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Platform_GetClipboardTextFn.unwrap(),
            foreign_get_clipboard_text
                as unsafe extern "C" fn(*mut sys::ImGuiContext) -> *const std::ffi::c_char,
        ));
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Platform_SetClipboardTextFn.unwrap(),
            foreign_set_clipboard_text
                as unsafe extern "C" fn(*mut sys::ImGuiContext, *const std::ffi::c_char),
        ));
        assert_eq!(
            (*platform_io).Platform_ClipboardUserData as usize,
            FOREIGN_PLATFORM_DATA
        );

        (*platform_io).Platform_GetClipboardTextFn = baseline_clipboard.0;
        (*platform_io).Platform_SetClipboardTextFn = baseline_clipboard.1;
        (*platform_io).Platform_ClipboardUserData = baseline_clipboard.2;
    });
}

#[test]
fn context_first_claimed_runtime_unregisters_callback_registry() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::clone(&observed_backend_data),
        Rc::clone(&observed_main_viewport_data),
        synthetic_create_window,
    );
    let control = Rc::clone(&runtime.control);
    let registry_key = control.platform_io_key.get();

    drop(context);

    assert_eq!(platform_count.get(), 1);
    assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
    assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
    assert!(!registry_contains(registry_key));
    assert_eq!(control.platform_io_key.get(), 0);
    assert_eq!(control.state(), RuntimeState::Detached);
    drop(runtime);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
}

#[test]
fn failed_viewport_creation_is_reported_on_next_rust_entry() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        observed_backend_data,
        observed_main_viewport_data,
        failing_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();

    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });

    assert!(viewport.PlatformRequestClose);
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportCreationFailed)
    ));
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn failed_viewport_opengl_context_is_reported_on_next_rust_entry() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let runtime = registration_with_lifecycle(&mut context, None, Rc::new(|| {}));

    runtime
        .control
        .record_viewport_opengl_context_failed_for_test();

    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportOpenGlContextFailed)
    ));
}

#[test]
fn foreign_callback_and_user_data_replacements_survive_shutdown() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let baseline_clipboard_user_data = context
        .binding()
        .with_bound_context(|| unsafe { (*sys::igGetPlatformIO_Nil()).Platform_ClipboardUserData });
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::clone(&observed_backend_data),
        Rc::clone(&observed_main_viewport_data),
        synthetic_create_window,
    );
    let registry_key = runtime.control.platform_io_key.get();

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*io).BackendPlatformUserData = FOREIGN_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        (*platform_io).Platform_CreateWindow = Some(foreign_create_window);
        (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
        (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
    });

    assert!(matches!(
        runtime.shutdown_platform(&mut context),
        Err(Sdl3BackendError::PlatformCallbackReplaced {
            callback: "Platform_CreateWindow"
        })
    ));

    assert_eq!(platform_count.get(), 1);
    assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
    assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
    assert!(!registry_contains(registry_key));
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        assert_eq!((*io).BackendPlatformUserData as usize, FOREIGN_BACKEND_DATA);
        assert_eq!(
            (*io).BackendPlatformName,
            FOREIGN_BACKEND_NAME.as_ptr().cast()
        );
        assert_eq!(
            (*platform_io).Platform_ClipboardUserData as usize,
            FOREIGN_PLATFORM_DATA
        );
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Platform_CreateWindow.unwrap(),
            foreign_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        ));
        assert_eq!(
            (*main_viewport).PlatformUserData as usize,
            FOREIGN_VIEWPORT_DATA
        );
        assert_eq!(
            (*main_viewport).PlatformHandle as usize,
            FOREIGN_VIEWPORT_HANDLE
        );
    });
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "BackendPlatformUserData"
        })
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "BackendPlatformName"
        })
    ));
    let fault = runtime.poll_fault();
    assert!(
        matches!(fault, Err(Sdl3BackendError::ForeignPlatformUserData)),
        "unexpected fourth platform ownership fault: {fault:?}"
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "MainViewport.PlatformHandle"
        })
    ));
    runtime.poll_fault().unwrap();

    // The synthetic foreign owner now performs its own shutdown before the
    // Context is dropped.
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*io).BackendPlatformUserData = std::ptr::null_mut();
        (*io).BackendPlatformName = std::ptr::null();
        (*platform_io).Platform_CreateWindow = None;
        (*platform_io).Platform_ClipboardUserData = baseline_clipboard_user_data;
        (*main_viewport).PlatformUserData = std::ptr::null_mut();
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
    });
}
