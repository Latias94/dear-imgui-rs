use super::*;

#[test]
fn callback_only_renderer_drift_revokes_reserved_capabilities_without_erasing_foreign_callback() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut registration = synthetic_renderer_registration(&mut context);

    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetPlatformIO_Nil()).Renderer_RenderWindow = Some(foreign_renderer_render_window);
    });

    assert!(matches!(
        registration.poll_fault(),
        Err(Sdl3BackendError::RendererCallbackReplaced {
            callback: "Renderer_RenderWindow"
        })
    ));
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
            0,
            "a partial callback replacement must revoke SDL renderer capabilities"
        );
    });

    let shutdown = registration.shutdown_platform(&mut context);
    assert!(matches!(
        shutdown,
        Ok(()) | Err(Sdl3BackendError::RendererCallbackReplaced { .. })
    ));
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        assert_eq!((*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS, 0);
        assert!((*io).BackendRendererUserData.is_null());
        assert!((*io).BackendRendererName.is_null());
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_RenderWindow.unwrap(),
            foreign_renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
        ));

        sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
    });
}

#[test]
fn core_identity_only_renderer_drift_revokes_reserved_capabilities_without_erasing_foreign_identity()
 {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut registration = synthetic_renderer_registration(&mut context);

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
    });

    assert!(matches!(
        registration.poll_fault(),
        Err(Sdl3BackendError::RendererStateReplaced {
            field: "BackendRendererUserData"
        })
    ));
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
            0,
            "foreign core identity alone is not a complete renderer takeover"
        );
    });

    let shutdown = registration.shutdown_platform(&mut context);
    assert!(matches!(
        shutdown,
        Ok(()) | Err(Sdl3BackendError::RendererStateReplaced { .. })
    ));
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        assert_eq!((*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS, 0);
        assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
        assert_eq!(
            (*io).BackendRendererName,
            FOREIGN_BACKEND_NAME.as_ptr().cast()
        );

        (*io).BackendRendererUserData = std::ptr::null_mut();
        (*io).BackendRendererName = std::ptr::null();
    });
}

#[test]
fn complete_renderer_takeover_preserves_its_capabilities_and_callbacks() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut registration = synthetic_renderer_registration(&mut context);
    let foreign_flags = context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        (*platform_io).Renderer_RenderWindow = Some(foreign_renderer_render_window);
        (*platform_io).Renderer_DestroyWindow = Some(foreign_renderer_destroy_window);
        (*platform_io).Renderer_SetWindowSize = Some(foreign_renderer_set_window_size);
        (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        (*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS
    });

    assert!(matches!(
        registration.poll_fault(),
        Err(Sdl3BackendError::RendererCallbackReplaced { .. })
    ));
    context.binding().with_bound_context(|| unsafe {
        assert_eq!(
            (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
            foreign_flags,
            "a complete foreign renderer takeover retains its own capability publication"
        );
    });

    let shutdown = registration.shutdown_platform(&mut context);
    assert!(matches!(
        shutdown,
        Ok(()) | Err(Sdl3BackendError::RendererCallbackReplaced { .. })
    ));
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        assert_eq!(
            (*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
            foreign_flags
        );
        assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
        assert_eq!(
            (*io).BackendRendererName,
            FOREIGN_BACKEND_NAME.as_ptr().cast()
        );
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_RenderWindow.unwrap(),
            foreign_renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
        ));
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_DestroyWindow.unwrap(),
            foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
        ));
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_SetWindowSize.unwrap(),
            foreign_renderer_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2_c),
        ));

        sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
        (*io).BackendRendererUserData = std::ptr::null_mut();
        (*io).BackendRendererName = std::ptr::null();
        (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
    });
}

#[test]
fn pointer_callback_replacement_survives_renderer_shutdown() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut registration = synthetic_renderer_registration(&mut context);

    context.binding().with_bound_context(|| unsafe {
        sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
            sys::igGetPlatformIO_Nil(),
            Some(foreign_renderer_set_window_size_pointer),
        );
    });

    assert!(matches!(
        registration.poll_fault(),
        Err(Sdl3BackendError::RendererCallbackReplaced {
            callback: "Renderer_SetWindowSize"
        })
    ));

    let shutdown = registration.shutdown_platform(&mut context);
    assert!(matches!(
        shutdown,
        Ok(())
            | Err(Sdl3BackendError::RendererCallbackReplaced {
                callback: "Renderer_SetWindowSize"
            })
    ));

    context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        let restored = sys::ImGuiPlatformIO_RendererSetWindowSizePointerParam(platform_io)
            .expect("foreign pointer callback must survive SDL3 teardown");
        assert!(std::ptr::fn_addr_eq(
            restored,
            foreign_renderer_set_window_size_pointer
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
        ));
        sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(platform_io, None);
    });
}

#[test]
fn pointer_original_callback_is_invoked() {
    let _guard = crate::tests::test_guard();
    RENDERER_POINTER_SET_SIZE.with(|recorded| recorded.set((0, 0)));
    let mut context = Context::create();
    let mut registration = synthetic_renderer_registration_with_pointer_callback(
        &mut context,
        Some(recording_renderer_set_window_size_pointer),
    );

    let mut viewport = sys::ImGuiViewport {
        PlatformUserData: OWNED_VIEWPORT_DATA as *mut _,
        PlatformHandle: OWNED_VIEWPORT_HANDLE as *mut _,
        RendererUserData: OWNED_BACKEND_DATA as *mut _,
        ..Default::default()
    };
    registration
        .control
        .remember_owned_viewport(&mut viewport, unsafe {
            ViewportPlatformState::capture(&viewport)
        });
    registration
        .control
        .remember_owned_renderer_viewport(&mut viewport, viewport.RendererUserData);
    let size = sys::ImVec2_c { x: 320.0, y: 240.0 };
    context.binding().with_bound_context(|| unsafe {
        renderer_set_window_size_callback_for_test(&mut viewport, &size)
    });
    assert_eq!(
        RENDERER_POINTER_SET_SIZE.with(Cell::get),
        (size.x.to_bits(), size.y.to_bits())
    );

    assert!(registration.shutdown_platform(&mut context).is_ok());
    context.binding().with_bound_context(|| unsafe {
        let platform_io = sys::igGetPlatformIO_Nil();
        assert!(
            sys::ImGuiPlatformIO_RendererSetWindowSizePointerParam(platform_io).is_none(),
            "the original native renderer callback must not outlive its renderer teardown"
        );
    });
}

#[test]
fn native_renderer_shutdown_preserves_foreign_callback_and_backend_replacements() {
    let _guard = crate::tests::test_guard();
    RENDERER_RENDER_COUNT.with(|count| count.set(0));
    RENDERER_SET_SIZE_COUNT.with(|count| count.set(0));
    OWNED_RENDERER_DESTROY_COUNT.with(|count| count.set(0));
    FOREIGN_RENDERER_DESTROY_COUNT.with(|count| count.set(0));
    RENDERER_DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
    let mut context = Context::create();
    let renderer_shutdown_count = Rc::new(Cell::new(0));
    let renderer_observed_owned_state = Rc::new(Cell::new(false));

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

    let mut registration = registration_with_backend_lifecycle(
        &mut context,
        Some({
            let count = Rc::clone(&renderer_shutdown_count);
            let observed = Rc::clone(&renderer_observed_owned_state);
            Rc::new(move || unsafe {
                count.set(count.get() + 1);
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                let callback_is_owned =
                    (*platform_io)
                        .Renderer_RenderWindow
                        .is_some_and(|callback| {
                            std::ptr::fn_addr_eq(
                                callback,
                                synthetic_renderer_render_window
                                    as unsafe extern "C" fn(
                                        *mut sys::ImGuiViewport,
                                        *mut std::ffi::c_void,
                                    ),
                            )
                        });
                observed.set(
                    callback_is_owned
                        && (*io).BackendRendererUserData as usize == OWNED_BACKEND_DATA
                        && (*io).BackendFlags & sys::ImGuiBackendFlags_RendererHasViewports as i32
                            != 0,
                );
                sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
                (*io).BackendRendererUserData = std::ptr::null_mut();
                (*io).BackendRendererName = std::ptr::null();
            })
        }),
        Rc::new(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let mut viewport = sys::ImGuiViewport::default();
            viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
            viewport.RendererUserData = FOREIGN_BACKEND_DATA as *mut _;
            let _ = with_current_runtime(|control| {
                control.remember_owned_viewport(
                    &mut viewport,
                    ViewportPlatformState::capture(&viewport),
                );
                control
                    .remember_owned_renderer_viewport(&mut viewport, OWNED_BACKEND_DATA as *mut _);
            });
            (*platform_io)
                .Renderer_DestroyWindow
                .expect("renderer destroy wrapper must remain installed")(&mut viewport);
            sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
            (*io).BackendFlags &= !(sys::ImGuiBackendFlags_HasMouseCursors as i32);
        }),
        PlatformGraphicsKind::Other,
        native_renderer,
    );

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        (*platform_io).Platform_CreateWindow = Some(synthetic_create_window);
        (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
        (*platform_io).Renderer_RenderWindow = Some(synthetic_renderer_render_window);
        (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
        (*platform_io).Renderer_SetWindowSize = Some(synthetic_renderer_set_window_size);
        (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendFlags |= sys::ImGuiBackendFlags_RendererHasViewports as i32
            | sys::ImGuiBackendFlags_HasMouseCursors as i32;
    });

    let baseline = registration.baseline.take().unwrap();
    let renderer_baseline = baseline.snapshot();
    let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
        (
            PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap(),
            RendererCallbackOwnership::claim(&registration.control, &renderer_baseline)
                .unwrap()
                .unwrap(),
        )
    });
    registration
        .control
        .callbacks
        .borrow_mut()
        .replace(platform);
    registration
        .control
        .renderer_callbacks
        .borrow_mut()
        .replace(renderer);
    registration.control.platform_initialized.set(true);
    registration.control.renderer_initialized.set(true);

    let mut owned_viewport = sys::ImGuiViewport::default();
    owned_viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
    owned_viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    owned_viewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;
    registration
        .control
        .remember_owned_viewport(&mut owned_viewport, unsafe {
            ViewportPlatformState::capture(&owned_viewport)
        });
    registration
        .control
        .remember_owned_renderer_viewport(&mut owned_viewport, owned_viewport.RendererUserData);
    let size = sys::ImVec2_c { x: 320.0, y: 240.0 };
    context.binding().with_bound_context(|| unsafe {
        renderer_set_window_size_callback_for_test(&mut owned_viewport, &size)
    });
    assert_eq!(RENDERER_SET_SIZE_COUNT.with(Cell::get), 1);

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        (*platform_io).Renderer_RenderWindow = Some(foreign_renderer_render_window);
        (*platform_io).Renderer_DestroyWindow = Some(foreign_renderer_destroy_window);
        (*platform_io).Renderer_SetWindowSize = Some(foreign_renderer_set_window_size);
        (*platform_io).Renderer_SwapBuffers = Some(foreign_renderer_render_window);
        (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
    });

    let mut viewport = sys::ImGuiViewport::default();
    context
        .binding()
        .with_bound_context(|| unsafe { renderer_render_window_callback_for_test(&mut viewport) });
    assert_eq!(RENDERER_RENDER_COUNT.with(Cell::get), 0);
    context.binding().with_bound_context(|| unsafe {
        let flags = (*sys::igGetIO_Nil()).BackendFlags;
        assert_ne!(
            flags & sys::ImGuiBackendFlags_RendererHasViewports as i32,
            0,
            "foreign renderer takeover must retain its published capability"
        );
        assert_ne!(flags & sys::ImGuiBackendFlags_HasMouseCursors as i32, 0);
    });

    assert!(matches!(
        registration.shutdown_platform(&mut context),
        Err(Sdl3BackendError::RendererCallbackReplaced {
            callback: "Renderer_DestroyWindow"
        })
    ));

    assert_eq!(renderer_shutdown_count.get(), 1);
    assert!(renderer_observed_owned_state.get());
    assert_eq!(OWNED_RENDERER_DESTROY_COUNT.with(Cell::get), 1);
    assert_eq!(FOREIGN_RENDERER_DESTROY_COUNT.with(Cell::get), 0);
    assert_eq!(
        RENDERER_DESTROY_OBSERVED_USER_DATA.with(Cell::get),
        OWNED_BACKEND_DATA
    );
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_RenderWindow.unwrap(),
            foreign_renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
        ));
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_DestroyWindow.unwrap(),
            foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
        ));
        assert!(std::ptr::fn_addr_eq(
            (*platform_io).Renderer_SwapBuffers.unwrap(),
            foreign_renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
        ));
        assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
        assert_eq!(
            (*io).BackendRendererName,
            FOREIGN_BACKEND_NAME.as_ptr().cast()
        );
        assert_eq!(
            (*io).BackendFlags & sys::ImGuiBackendFlags_HasMouseCursors as i32,
            0,
            "renderer shutdown must not resurrect a platform bit cleared by native shutdown"
        );
        assert_ne!(
            (*io).BackendFlags & sys::ImGuiBackendFlags_RendererHasViewports as i32,
            0,
            "renderer shutdown must restore a foreign renderer capability snapshot"
        );
    });
    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
        (*io).BackendRendererUserData = std::ptr::null_mut();
        (*io).BackendRendererName = std::ptr::null();
        (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
    });
    let mut faults = Vec::new();
    while let Err(fault) = registration.poll_fault() {
        faults.push(fault);
    }
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererCallbackReplaced {
                callback: "Renderer_RenderWindow"
            }
        )
    }));
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererCallbackReplaced {
                callback: "Renderer_SwapBuffers"
            }
        )
    }));
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererStateReplaced {
                field: "BackendRendererUserData"
            }
        )
    }));
    assert!(faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererStateReplaced {
                field: "BackendRendererName"
            }
        )
    }));
    assert!(!faults.iter().any(|fault| {
        matches!(
            fault,
            Sdl3BackendError::RendererStateReplaced {
                field: "BackendFlags(renderer-owned bits)"
            }
        )
    }));
}
