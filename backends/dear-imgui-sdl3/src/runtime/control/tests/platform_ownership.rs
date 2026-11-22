use super::*;

#[test]
fn shutdown_preserves_live_application_viewport_callbacks() {
    unsafe extern "C" fn dpi(_: *mut sys::ImGuiViewport) -> f32 {
        1.25
    }
    unsafe extern "C" fn replacement_dpi(_: *mut sys::ImGuiViewport) -> f32 {
        2.5
    }
    unsafe extern "C" fn changed(_: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn insets(_: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        unsafe { out.write(sys::ImVec4::zero()) };
    }
    unsafe extern "C" fn replacement_insets(_: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        unsafe { out.write(sys::ImVec4::new(1.0, 2.0, 3.0, 4.0)) };
    }

    let _guard = crate::tests::test_guard();
    for replace in [false, true] {
        let mut context = Context::create();
        let pio = context.platform_io_mut().as_raw_mut();
        unsafe {
            (*pio).Platform_GetWindowDpiScale = Some(dpi);
            (*pio).Platform_OnChangedViewport = Some(changed);
            sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(pio, Some(insets));
        }
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        if replace {
            unsafe {
                (*pio).Platform_GetWindowDpiScale = Some(replacement_dpi);
                (*pio).Platform_OnChangedViewport = None;
                sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                    pio,
                    Some(replacement_insets),
                );
            }
        }
        runtime.shutdown_platform(&mut context).unwrap();

        let viewport = context.main_viewport().as_raw_mut();
        assert_eq!(
            unsafe { (*pio).Platform_GetWindowDpiScale.unwrap()(viewport) },
            if replace { 2.5 } else { 1.25 }
        );
        if replace {
            assert!(unsafe { (*pio).Platform_OnChangedViewport }.is_none());
            assert!(unsafe {
                sys::ImGuiPlatformIO_ClearPlatformGetWindowWorkAreaInsetsIfOutParam(
                    pio,
                    replacement_insets,
                )
            });
        } else {
            assert!(std::ptr::fn_addr_eq(
                unsafe { (*pio).Platform_OnChangedViewport }.unwrap(),
                changed as unsafe extern "C" fn(*mut sys::ImGuiViewport)
            ));
            assert!(unsafe {
                sys::ImGuiPlatformIO_ClearPlatformGetWindowWorkAreaInsetsIfOutParam(pio, insets)
            });
        }
    }
}

#[test]
fn failed_initialization_restores_unclaimed_viewport_callbacks() {
    unsafe extern "C" fn dpi(_: *mut sys::ImGuiViewport) -> f32 {
        1.5
    }
    unsafe extern "C" fn changed(_: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn insets(_: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        unsafe { out.write(sys::ImVec4::zero()) };
    }

    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let pio = context.platform_io_mut().as_raw_mut();
    unsafe {
        (*pio).Platform_GetWindowDpiScale = Some(dpi);
        (*pio).Platform_OnChangedViewport = Some(changed);
        sys::ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(pio, Some(insets));
    }
    let mut registration = registration_with_lifecycle(&mut context, None, Rc::new(|| {}));
    unsafe { sys::ImGuiPlatformIO_ClearPlatformHandlers(pio) };
    registration.native_initialization_failed();

    let viewport = context.main_viewport().as_raw_mut();
    assert_eq!(
        unsafe { (*pio).Platform_GetWindowDpiScale.unwrap()(viewport) },
        1.5
    );
    assert!(std::ptr::fn_addr_eq(
        unsafe { (*pio).Platform_OnChangedViewport }.unwrap(),
        changed as unsafe extern "C" fn(*mut sys::ImGuiViewport)
    ));
    assert!(unsafe {
        sys::ImGuiPlatformIO_ClearPlatformGetWindowWorkAreaInsetsIfOutParam(pio, insets)
    });
}

#[test]
fn platform_claim_wraps_every_official_sdl3_window_callback() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );

    context.binding().with_bound_context(|| {
        let callbacks = runtime.control.callbacks.borrow();
        let ownership = callbacks
            .as_ref()
            .expect("synthetic registration must own the callback table");
        for slot in PlatformCallbackSlot::ALL {
            assert!(
                ownership.wraps_slot(slot),
                "{} was not wrapped",
                slot.name()
            );
            assert!(
                unsafe { ownership.owns_live_slot(slot) },
                "{} did not retain exact live ownership",
                slot.name()
            );
        }
    });

    runtime.shutdown_platform(&mut context).unwrap();
}

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
