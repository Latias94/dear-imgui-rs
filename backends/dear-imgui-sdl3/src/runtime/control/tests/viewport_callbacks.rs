use super::*;

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
#[cfg(debug_assertions)]
#[test]
fn real_imgui_destroy_platform_windows_restores_foreign_viewport_state_after_core_cleanup() {
    let _guard = crate::tests::test_guard();
    DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
    RENDERER_DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
    let mut context = Context::create();
    let viewport_pointer = Rc::new(Cell::new(0_usize));

    #[cfg(feature = "opengl3-renderer")]
    let native_renderer = NativeRendererKind::OpenGl3;
    #[cfg(all(not(feature = "opengl3-renderer"), feature = "sdlrenderer3-renderer"))]
    let native_renderer = NativeRendererKind::SdlRenderer3;
    #[cfg(all(
        not(feature = "opengl3-renderer"),
        not(feature = "sdlrenderer3-renderer"),
        feature = "sdlgpu3-renderer"
    ))]
    let native_renderer = NativeRendererKind::SdlGpu3;

    let mut runtime = registration_with_backend_lifecycle(
        &mut context,
        Some(Rc::new(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
            (*io).BackendRendererUserData = std::ptr::null_mut();
            (*io).BackendRendererName = std::ptr::null();
        })),
        {
            let viewport_pointer = Rc::clone(&viewport_pointer);
            Rc::new(move || unsafe {
                crate::core::ffi::dear_imgui_sdl3_destroy_platform_windows_for_test(
                    viewport_pointer.get() as *mut sys::ImGuiViewportP,
                );
                sys::ImGuiPlatformIO_ClearPlatformHandlers(sys::igGetPlatformIO_Nil());
            })
        },
        PlatformGraphicsKind::Other,
        native_renderer,
    );
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
        (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
        (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        (*main_viewport).PlatformHandleRaw = OWNED_VIEWPORT_HANDLE as *mut _;
        (*main_viewport).PlatformWindowCreated = true;
    });
    let baseline = runtime.baseline.take().unwrap();
    let renderer_baseline = baseline.snapshot();
    let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
        (
            PlatformCallbackOwnership::claim(&runtime.control, baseline).unwrap(),
            RendererCallbackOwnership::claim(&runtime.control, &renderer_baseline)
                .unwrap()
                .unwrap(),
        )
    });
    runtime.control.callbacks.borrow_mut().replace(platform);
    runtime
        .control
        .renderer_callbacks
        .borrow_mut()
        .replace(renderer);
    runtime.control.platform_initialized.set(true);
    runtime.control.renderer_initialized.set(true);
    context.binding().with_bound_context(|| unsafe {
        let main_viewport = sys::igGetMainViewport();
        (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        (*main_viewport).PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE as *mut _;
        (*main_viewport).RendererUserData = FOREIGN_BACKEND_DATA as *mut _;
    });

    let mut viewport = Box::new(sys::ImGuiViewportP::default());
    let raw = &mut viewport._ImGuiViewport as *mut sys::ImGuiViewport;
    viewport_pointer.set((&mut *viewport) as *mut sys::ImGuiViewportP as usize);
    viewport._ImGuiViewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
    viewport._ImGuiViewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    viewport._ImGuiViewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;
    viewport._ImGuiViewport.PlatformWindowCreated = true;
    runtime
        .control
        .remember_owned_viewport(raw, unsafe { ViewportPlatformState::capture(raw) });
    runtime
        .control
        .remember_owned_renderer_viewport(raw, viewport._ImGuiViewport.RendererUserData);
    viewport._ImGuiViewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
    viewport._ImGuiViewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
    viewport._ImGuiViewport.PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE_RAW as *mut _;
    viewport._ImGuiViewport.RendererUserData = FOREIGN_BACKEND_DATA as *mut _;

    assert!(matches!(
        runtime.shutdown_platform(&mut context),
        Err(Sdl3BackendError::ForeignPlatformUserData)
    ));

    assert!(!viewport._ImGuiViewport.PlatformWindowCreated);
    assert_eq!(
        RENDERER_DESTROY_OBSERVED_USER_DATA.with(Cell::get),
        OWNED_BACKEND_DATA,
        "the upstream renderer destroy callback must receive SDL's owned sidecar"
    );
    assert_eq!(
        DESTROY_OBSERVED_USER_DATA.with(Cell::get),
        OWNED_VIEWPORT_DATA,
        "the upstream platform destroy callback must receive SDL's owned sidecar after renderer teardown"
    );
    context.binding().with_bound_context(|| unsafe {
        let main_viewport = sys::igGetMainViewport();
        assert_eq!(
            (*main_viewport).PlatformUserData as usize,
            FOREIGN_VIEWPORT_DATA
        );
        assert_eq!(
            (*main_viewport).PlatformHandle as usize,
            FOREIGN_VIEWPORT_HANDLE
        );
        assert_eq!(
            (*main_viewport).PlatformHandleRaw as usize,
            FOREIGN_VIEWPORT_HANDLE
        );
        assert_eq!(
            (*main_viewport).RendererUserData as usize,
            FOREIGN_BACKEND_DATA
        );
    });
    assert_eq!(
        viewport._ImGuiViewport.PlatformUserData as usize,
        FOREIGN_VIEWPORT_DATA
    );
    assert_eq!(
        viewport._ImGuiViewport.PlatformHandle as usize,
        FOREIGN_VIEWPORT_HANDLE
    );
    assert_eq!(
        viewport._ImGuiViewport.PlatformHandleRaw as usize,
        FOREIGN_VIEWPORT_HANDLE_RAW
    );
    assert_eq!(
        viewport._ImGuiViewport.RendererUserData as usize,
        FOREIGN_BACKEND_DATA
    );
    context.binding().with_bound_context(|| unsafe {
        let main_viewport = sys::igGetMainViewport();
        (*main_viewport).PlatformUserData = std::ptr::null_mut();
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
        (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
        (*main_viewport).RendererUserData = std::ptr::null_mut();
        (*main_viewport).PlatformWindowCreated = false;
    });
    let faults = runtime.drain_faults();
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::PlatformStateReplaced {
                field: "MainViewport.PlatformHandle"
            }
        )
    }));
    assert!(
        faults
            .iter()
            .all(|fault| !matches!(fault, Sdl3BackendError::ForeignPlatformUserData)),
        "the terminal foreign-user-data root cause must be reported only once"
    );
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::PlatformStateReplaced {
                field: "Viewport.PlatformHandle"
            }
        )
    }));
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererStateReplaced {
                field: "Viewport.RendererUserData"
            }
        )
    }));
}

#[test]
fn deferred_restore_rejects_a_reused_viewport_address_with_a_new_id() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    viewport.ID = 41;
    viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
    viewport.PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE_RAW as *mut _;
    viewport.RendererUserData = FOREIGN_BACKEND_DATA as *mut _;

    runtime.control.callback_teardown_active.set(true);
    runtime
        .control
        .defer_platform_viewport_restore(&mut viewport, unsafe {
            ViewportPlatformState::capture(&viewport)
        });
    runtime
        .control
        .defer_renderer_viewport_restore(&mut viewport, viewport.RendererUserData);
    runtime.control.callback_teardown_active.set(false);

    viewport.ID = 42;
    viewport.PlatformUserData = std::ptr::null_mut();
    viewport.PlatformHandle = std::ptr::null_mut();
    viewport.PlatformHandleRaw = std::ptr::null_mut();
    viewport.RendererUserData = std::ptr::null_mut();
    runtime.control.restore_deferred_viewport_state();

    assert!(viewport.PlatformUserData.is_null());
    assert!(viewport.PlatformHandle.is_null());
    assert!(viewport.PlatformHandleRaw.is_null());
    assert!(viewport.RendererUserData.is_null());
    runtime.shutdown_platform(&mut context).unwrap();
}

#[test]
fn owned_platform_lease_accepts_an_in_place_docking_id_change() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    viewport.ID = 41;
    viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    runtime
        .control
        .remember_owned_viewport(&mut viewport, unsafe {
            ViewportPlatformState::capture(&viewport)
        });

    viewport.ID = 42;
    let expected = runtime
        .control
        .take_owned_viewport(&mut viewport)
        .expect("an exact platform sidecar proves that docking changed the existing viewport");

    assert_eq!(expected, unsafe {
        ViewportPlatformState::capture(&viewport)
    });
    runtime.shutdown_platform(&mut context).unwrap();
}

#[test]
fn owned_platform_lease_rejects_reused_address_with_foreign_sidecar() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    viewport.ID = 41;
    viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    runtime
        .control
        .remember_owned_viewport(&mut viewport, unsafe {
            ViewportPlatformState::capture(&viewport)
        });

    viewport.ID = 42;
    viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

    assert!(runtime.control.take_owned_viewport(&mut viewport).is_none());
    assert_eq!(viewport.PlatformUserData as usize, FOREIGN_VIEWPORT_DATA);
    assert_eq!(viewport.PlatformHandle as usize, FOREIGN_VIEWPORT_HANDLE);
    runtime.shutdown_platform(&mut context).unwrap();
}

#[test]
fn destroy_callback_never_revisits_a_reused_viewport_address() {
    let _guard = crate::tests::test_guard();
    DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        observed_backend_data,
        observed_main_viewport_data,
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
    assert_eq!(viewport.PlatformUserData as usize, OWNED_VIEWPORT_DATA);
    assert_eq!(viewport.PlatformHandle as usize, OWNED_VIEWPORT_HANDLE);
    viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

    context
        .binding()
        .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });

    assert_eq!(
        DESTROY_OBSERVED_USER_DATA.with(Cell::get),
        OWNED_VIEWPORT_DATA
    );
    assert!(viewport.PlatformUserData.is_null());
    assert!(viewport.PlatformHandle.is_null());
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ForeignPlatformUserData)
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "Viewport.PlatformHandle"
        })
    ));
    // Simulate Dear ImGui reusing the same allocation for a new viewport before platform
    // shutdown. Teardown must not retain the old address and write the prior foreign state
    // into this new object.
    viewport.PlatformUserData = FOREIGN_PLATFORM_DATA as *mut _;
    viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE_RAW as *mut _;
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(viewport.PlatformUserData as usize, FOREIGN_PLATFORM_DATA);
    assert_eq!(
        viewport.PlatformHandle as usize,
        FOREIGN_VIEWPORT_HANDLE_RAW
    );
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn platform_render_rejects_foreign_viewport_state_before_native_callback() {
    let _guard = crate::tests::test_guard();
    PLATFORM_RENDER_COUNT.with(|count| count.set(0));
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
    viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;

    context
        .binding()
        .with_bound_context(|| unsafe { render_window_callback_for_test(&mut viewport) });

    assert_eq!(PLATFORM_RENDER_COUNT.with(Cell::get), 0);
    assert!(viewport.PlatformRequestClose);
    assert!(viewport.DrawData.is_null());
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ForeignPlatformUserData)
    ));

    context
        .binding()
        .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn platform_swap_rejects_foreign_viewport_state_before_native_callback() {
    let _guard = crate::tests::test_guard();
    PLATFORM_SWAP_COUNT.with(|count| count.set(0));
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut viewport = sys::ImGuiViewport::default();
    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
    viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

    context
        .binding()
        .with_bound_context(|| unsafe { swap_buffers_callback_for_test(&mut viewport) });

    assert_eq!(PLATFORM_SWAP_COUNT.with(Cell::get), 0);
    assert!(viewport.PlatformRequestClose);
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "Viewport.PlatformHandle"
        })
    ));

    context
        .binding()
        .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn main_viewport_handle_drift_blocks_unrelated_direct_trampoline() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let published_flags = context.binding().with_bound_context(|| unsafe {
        (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
    });
    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetMainViewport()).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
    });
    let mut secondary = sys::ImGuiViewport::default();

    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut secondary) });

    assert!(secondary.PlatformUserData.is_null());
    assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
            0,
            "a partial foreign drift must revoke SDL platform capabilities"
        );
    });
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "MainViewport.PlatformHandle"
        })
    ));

    runtime.shutdown_platform(&mut context).unwrap();
    context.binding().with_bound_context(|| unsafe {
        let main_viewport = sys::igGetMainViewport();
        assert_eq!(
            (*main_viewport).PlatformHandle as usize,
            FOREIGN_VIEWPORT_HANDLE
        );
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
            0,
            "teardown must not restore capabilities after only one platform field changed"
        );
    });
    assert_ne!(published_flags, 0);
}

#[test]
fn platform_name_only_drift_revokes_reserved_capabilities() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetIO_Nil()).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
    });

    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformStateReplaced {
            field: "BackendPlatformName"
        })
    ));
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
            0,
            "a foreign name alone is not a complete platform takeover"
        );
    });

    runtime.shutdown_platform(&mut context).unwrap();
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        assert_eq!((*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS, 0);
        assert_eq!(
            (*io).BackendPlatformName,
            FOREIGN_BACKEND_NAME.as_ptr().cast()
        );
        (*io).BackendPlatformName = std::ptr::null();
    });
}

#[test]
fn complete_foreign_platform_takeover_preserves_capability_flags() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let original_service_callbacks = context.binding().with_bound_context(|| unsafe {
        let platform_io = &*sys::igGetPlatformIO_Nil();
        (
            platform_io.Platform_GetClipboardTextFn,
            platform_io.Platform_SetClipboardTextFn,
            platform_io.Platform_OpenInShellFn,
            platform_io.Platform_SetImeDataFn,
        )
    });
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let foreign_flags = context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*platform_io).Platform_GetClipboardTextFn = original_service_callbacks.0;
        (*platform_io).Platform_SetClipboardTextFn = original_service_callbacks.1;
        (*platform_io).Platform_OpenInShellFn = original_service_callbacks.2;
        (*platform_io).Platform_SetImeDataFn = original_service_callbacks.3;
        (*platform_io).Platform_CreateWindow = Some(foreign_create_window);
        (*platform_io).Platform_DestroyWindow = Some(foreign_destroy_window);
        (*platform_io).Platform_ShowWindow = Some(foreign_show_window);
        (*platform_io).Platform_UpdateWindow = Some(foreign_update_window);
        sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
            platform_io,
            Some(foreign_set_window_pos),
        );
        sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
            platform_io,
            Some(foreign_get_window_pos),
        );
        sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
            platform_io,
            Some(foreign_set_window_size),
        );
        sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
            platform_io,
            Some(foreign_get_window_size),
        );
        sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
            platform_io,
            Some(foreign_get_window_framebuffer_scale),
        );
        (*platform_io).Platform_SetWindowFocus = Some(foreign_set_window_focus);
        (*platform_io).Platform_GetWindowFocus = Some(foreign_get_window_focus);
        (*platform_io).Platform_GetWindowMinimized = Some(foreign_get_window_minimized);
        (*platform_io).Platform_SetWindowTitle = Some(foreign_set_window_title);
        (*platform_io).Platform_RenderWindow = Some(foreign_platform_render_window);
        (*platform_io).Platform_SwapBuffers = Some(foreign_platform_swap_buffers);
        (*platform_io).Platform_SetWindowAlpha = Some(foreign_set_window_alpha);
        (*platform_io).Platform_CreateVkSurface = Some(foreign_create_vk_surface);
        (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
        (*io).BackendPlatformUserData = FOREIGN_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        (*main_viewport).PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE_RAW as *mut _;
        (*io).BackendFlags =
            ((*io).BackendFlags & !SDL_PLATFORM_RESERVED_FLAGS) | SDL_PLATFORM_RESERVED_FLAGS;
        (*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
    });
    let mut secondary = sys::ImGuiViewport::default();

    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut secondary) });

    assert!(secondary.PlatformUserData.is_null());
    let observed_flags = context.binding().with_bound_context(|| unsafe {
        (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
    });

    let shutdown = runtime.shutdown_platform(&mut context);
    let restored_flags = context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        let restored_flags = (*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS;
        sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
        (*platform_io).Platform_ClipboardUserData = std::ptr::null_mut();
        (*io).BackendPlatformUserData = std::ptr::null_mut();
        (*io).BackendPlatformName = std::ptr::null();
        (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
        (*main_viewport).PlatformUserData = std::ptr::null_mut();
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
        (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
        restored_flags
    });
    assert_eq!(
        observed_flags, foreign_flags,
        "detecting a foreign backend must not clear its published capabilities"
    );
    assert!(matches!(
        shutdown,
        Err(Sdl3BackendError::PlatformCallbackReplaced { .. })
    ));
    assert_eq!(
        restored_flags, foreign_flags,
        "shutdown must restore the foreign platform capability snapshot"
    );
}

#[test]
fn foreign_write_to_reserved_platform_slot_blocks_direct_trampoline() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        (*platform_io).Platform_SetWindowAlpha = Some(foreign_set_window_alpha);
    });
    let mut viewport = sys::ImGuiViewport::default();

    context
        .binding()
        .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });

    assert!(viewport.PlatformUserData.is_null());
    assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformCallbackReplaced {
            callback: "Platform_SetWindowAlpha"
        })
    ));
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
            0,
            "filling one reserved callback slot must revoke SDL platform capabilities"
        );
    });
    runtime.shutdown_platform(&mut context).unwrap();
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
            0
        );
        (*sys::igGetPlatformIO_Nil()).Platform_SetWindowAlpha = None;
    });
}

#[test]
fn callback_panic_latches_shutdown_after_the_fault_is_consumed() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );

    runtime
        .control
        .record_callback_panicked("Platform_CreateWindow");

    assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformCallbackPanicked {
            callback: "Platform_CreateWindow"
        })
    ));
    assert!(matches!(
        runtime.control.ensure_bound_entry(),
        Err(Sdl3BackendError::RuntimeDetached)
    ));
    runtime.shutdown_platform(&mut context).unwrap();
}

#[test]
fn native_faults_report_the_original_share_configuration_failure_first() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let share_configuration = 1_u64 << 1;
    let context_creation = 1_u64 << 4;

    runtime
        .control
        .record_native_faults(share_configuration | context_creation, share_configuration);

    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportOpenGlShareConfigurationFailed)
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportOpenGlContextFailed)
    ));
    runtime.shutdown_platform(&mut context).unwrap();
}

#[cfg(feature = "sdlgpu3-renderer")]
#[test]
fn sdlgpu_create_failure_clears_upstream_sentinel_before_destroy_can_release_it() {
    let _guard = crate::tests::test_guard();
    for (fault, is_claim_failure) in [(1_u64 << 11, true), (1_u64 << 12, false)] {
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();
        viewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;

        runtime.control.record_native_faults(fault, fault);
        unsafe { finish_sdlgpu_renderer_create(&runtime.control, &mut viewport, fault) };

        assert!(viewport.RendererUserData.is_null());
        assert!(
            runtime
                .control
                .owned_renderer_viewport(&mut viewport)
                .is_none()
        );
        assert!(runtime.control.viewport_failed(&mut viewport));
        let error = runtime.poll_fault();
        if is_claim_failure {
            assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuClaimFailed)
            ));
        } else {
            assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuConfigureFailed)
            ));
        }
        runtime.shutdown_platform(&mut context).unwrap();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
#[test]
fn sdlgpu_secondary_render_faults_are_typed_and_close_the_viewport() {
    let _guard = crate::tests::test_guard();
    for fault in [
        1_u64 << 14,
        1_u64 << 15,
        1_u64 << 16,
        1_u64 << 17,
        1_u64 << 18,
    ] {
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();

        runtime.control.record_native_faults(fault, fault);
        runtime.control.mark_viewport_failed(&mut viewport);

        assert!(viewport.PlatformRequestClose);
        assert!(viewport.DrawData.is_null());
        let error = runtime.poll_fault();
        match fault {
            value if value == 1_u64 << 14 => assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuCommandBufferFailed)
            )),
            value if value == 1_u64 << 15 => assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuSwapchainFailed)
            )),
            value if value == 1_u64 << 16 => assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuRenderPassFailed)
            )),
            value if value == 1_u64 << 17 => assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuSubmitFailed)
            )),
            _ => assert!(matches!(
                error,
                Err(Sdl3BackendError::ViewportSdlGpuCommandBufferCancelFailed)
            )),
        }
        runtime.shutdown_platform(&mut context).unwrap();
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
#[test]
fn sdlgpu_swapchain_failure_is_reported_before_command_buffer_cancel_failure() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let swapchain_fault = 1_u64 << 15;
    let cancel_fault = 1_u64 << 18;

    runtime
        .control
        .record_native_faults(swapchain_fault | cancel_fault, swapchain_fault);

    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportSdlGpuSwapchainFailed)
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::ViewportSdlGpuCommandBufferCancelFailed)
    ));
    runtime.shutdown_platform(&mut context).unwrap();
}
