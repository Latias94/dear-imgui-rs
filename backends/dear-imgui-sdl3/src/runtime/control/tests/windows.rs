use super::*;

fn with_native_window(callback: impl FnOnce(&sdl3::video::Window)) {
    with_video_driver("dummy", callback);
}

fn with_video_driver(driver: &str, callback: impl FnOnce(&sdl3::video::Window)) {
    struct DriverHintRestore(Option<String>);
    impl Drop for DriverHintRestore {
        fn drop(&mut self) {
            if let Some(previous) = &self.0 {
                sdl3::hint::set_with_priority(
                    "SDL_VIDEO_DRIVER",
                    previous,
                    &sdl3::hint::Hint::Override,
                );
            } else {
                unsafe { sdl3_sys::hints::SDL_ResetHint(c"SDL_VIDEO_DRIVER".as_ptr()) };
            }
        }
    }

    let _restore = DriverHintRestore(sdl3::hint::get("SDL_VIDEO_DRIVER"));
    assert!(sdl3::hint::set_with_priority(
        "SDL_VIDEO_DRIVER",
        driver,
        &sdl3::hint::Hint::Override
    ));
    let sdl = sdl3::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("SDL3 native registration test", 80, 60)
        .hidden()
        .build()
        .unwrap();
    callback(&window);
}

#[test]
fn lifecycle_observations_remain_owned_after_shutdown_and_id_remap_is_not_creation() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let created = backend.window_snapshot(window.id()).unwrap().unwrap();
        assert_eq!(
            backend.drain_window_lifecycle_events(),
            vec![Sdl3WindowLifecycleEvent::Created(created)]
        );
        assert_eq!(created.state.visible, Some(false));
        assert_eq!(created.state.native_owner, None);
        assert_eq!(
            created.capabilities.theme,
            Sdl3WindowPolicySupport::Unsupported
        );
        let pointer = context
            .binding()
            .with_bound_context(|| unsafe { sys::igGetMainViewport() });
        context.binding().with_bound_context(|| unsafe {
            (*pointer).ID = created.target.viewport_id.raw().wrapping_add(1)
        });
        let remapped = backend.window_snapshot(window.id()).unwrap().unwrap();
        assert_eq!(remapped.target.instance_id, created.target.instance_id);
        assert!(backend.drain_window_lifecycle_events().is_empty());
        context
            .binding()
            .with_bound_context(|| unsafe { (*pointer).ID = created.target.viewport_id.raw() });
        backend.window_snapshot(window.id()).unwrap();
        backend.shutdown(&mut context).unwrap();
        let events = adapter.drain_window_lifecycle_events();
        assert!(
            matches!(events.as_slice(), [Sdl3WindowLifecycleEvent::Retired { window, reason: Sdl3WindowRetirementReason::BackendShutdown }] if window.target.instance_id == created.target.instance_id)
        );
        backend.shutdown(&mut context).unwrap();
        assert!(backend.drain_window_lifecycle_events().is_empty());
        assert_eq!(created.window_id, window.id());
    });
}

#[test]
fn invalid_configuration_and_configurator_panics_never_allocate_and_never_reuse_ids() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let attempts = Rc::new(RefCell::new(Vec::new()));
    runtime
        .control
        .secondary_window_configurator
        .borrow_mut()
        .replace(Rc::new({
            let attempts = Rc::clone(&attempts);
            move |request, config| {
                attempts.borrow_mut().push(request.target.instance_id);
                if attempts.borrow().len() == 1 {
                    panic!("intentional configurator panic");
                }
                config.min_client_size = Some([100, 20]);
                config.max_client_size = Some([40, 20]);
            }
        }));
    for expected_count in 1..=2 {
        let mut viewport = sys::ImGuiViewport::default();
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
        assert!(viewport.PlatformUserData.is_null());
        assert!(viewport.PlatformHandle.is_null());
        assert!(viewport.PlatformRequestClose);
        assert_eq!(attempts.borrow().len(), expected_count);
    }
    assert_ne!(attempts.borrow()[0], attempts.borrow()[1]);
    let errors = runtime.drain_faults();
    assert!(matches!(
        errors.as_slice(),
        [
            Sdl3BackendError::WindowConfigurationPanicked,
            Sdl3BackendError::InvalidWindowConfiguration { .. }
        ]
    ));
    assert!(runtime.control.drain_window_lifecycle_events().is_empty());
    runtime.shutdown_platform(&mut context).unwrap();
}

#[test]
fn unsupported_dynamic_policy_does_not_mutate_the_target() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let original = backend.window_snapshot(window.id()).unwrap().unwrap();
        let error = backend
            .update_window(
                original.target.instance_id,
                Sdl3WindowUpdate {
                    resizable: Some(true),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            Sdl3BackendError::UnsupportedWindowPolicy {
                attribute: "resizable"
            }
        ));
        assert_eq!(
            backend.window_snapshot(window.id()).unwrap(),
            Some(original)
        );
        backend.shutdown(&mut context).unwrap();
    });
}

#[test]
fn context_first_teardown_keeps_lifecycle_history_readable_without_retaining_native_windows() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        let backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let original = backend.window_snapshot(window.id()).unwrap().unwrap();
        backend.drain_window_lifecycle_events();
        drop(context);
        let events = adapter.drain_window_lifecycle_events();
        assert!(
            matches!(events.as_slice(), [Sdl3WindowLifecycleEvent::Retired { window, .. }] if window.target.instance_id == original.target.instance_id)
        );
        drop(backend);
        assert!(adapter.drain_window_lifecycle_events().is_empty());
        assert_eq!(window.id(), original.window_id);
    });
}

#[test]
fn transparent_creation_bridge_preserves_backend_flags_and_clears_policy_between_attempts() {
    let _guard = crate::tests::test_guard();
    with_native_window(|_| unsafe {
        assert!(!crate::core::ffi::dear_imgui_sdl3_native_set_window_transparent(true));
        assert_eq!(
            crate::core::ffi::dear_imgui_sdl3_native_begin(1, 0, 0, 0, std::ptr::null_mut()),
            0
        );
        assert!(crate::core::ffi::dear_imgui_sdl3_native_set_window_transparent(true));
        let window = crate::core::ffi::dear_imgui_sdl3_hook_create_window(
            c"transparent policy test".as_ptr(),
            20,
            20,
            sdl3_sys::video::SDL_WINDOW_HIDDEN | sdl3_sys::video::SDL_WINDOW_RESIZABLE,
        );
        assert!(!window.is_null());
        let flags = sdl3_sys::video::SDL_GetWindowFlags(window);
        assert_ne!(flags & sdl3_sys::video::SDL_WINDOW_HIDDEN, 0);
        assert_ne!(flags & sdl3_sys::video::SDL_WINDOW_RESIZABLE, 0);
        assert_ne!(flags & sdl3_sys::video::SDL_WINDOW_TRANSPARENT, 0);
        sdl3_sys::video::SDL_DestroyWindow(window);
        assert_eq!(
            crate::core::ffi::dear_imgui_sdl3_native_end(std::ptr::null_mut()),
            0
        );
        assert_eq!(
            crate::core::ffi::dear_imgui_sdl3_native_begin(1, 0, 0, 0, std::ptr::null_mut()),
            0
        );
        let window = crate::core::ffi::dear_imgui_sdl3_hook_create_window(
            c"default policy test".as_ptr(),
            20,
            20,
            sdl3_sys::video::SDL_WINDOW_HIDDEN,
        );
        assert!(!window.is_null());
        assert_eq!(
            sdl3_sys::video::SDL_GetWindowFlags(window) & sdl3_sys::video::SDL_WINDOW_TRANSPARENT,
            0
        );
        sdl3_sys::video::SDL_DestroyWindow(window);
        assert_eq!(
            crate::core::ffi::dear_imgui_sdl3_native_end(std::ptr::null_mut()),
            0
        );
    });
}

#[cfg(all(target_os = "windows", debug_assertions))]
#[test]
fn native_secondary_configuration_is_applied_hidden_and_failed_creation_rolls_back_once() {
    let _guard = crate::tests::test_guard();
    with_video_driver("windows", |window| {
        struct LiveViewport(*mut sys::ImGuiViewport);
        impl Drop for LiveViewport {
            fn drop(&mut self) {
                unsafe { crate::core::ffi::dear_imgui_sdl3_delete_live_viewport_for_test(self.0) };
            }
        }
        let count_windows = || unsafe {
            let mut count = 0;
            let windows = sdl3_sys::video::SDL_GetWindows(&mut count);
            sdl3_sys::stdinc::SDL_free(windows.cast());
            count
        };
        let mut context = Context::create();
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        backend.drain_window_lifecycle_events();
        let icon = Sdl3WindowIcon::from_rgba(2, 2, vec![255; 16]).unwrap();
        backend
            .set_secondary_window_config(Sdl3WindowConfig {
                min_client_size: Some([32, 32]),
                max_client_size: Some([256, 256]),
                resizable: Some(false),
                focusable: Some(false),
                icon: Some(icon),
                ..Default::default()
            })
            .unwrap();
        let viewport = LiveViewport(context.binding().with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x410001)
        }));
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(viewport.0) });
        let observed = backend
            .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x410001))
            .unwrap()
            .unwrap();
        assert_eq!(observed.state.visible, Some(false));
        assert_eq!(observed.state.resizable, Some(false));
        assert_eq!(observed.state.focusable, Some(false));
        assert_eq!(observed.physical_client_size, Some([80, 60]));
        assert_eq!(
            observed.state.native_owner.unwrap().unwrap().window_id,
            window.id()
        );
        assert_eq!(
            backend.drain_window_lifecycle_events(),
            vec![Sdl3WindowLifecycleEvent::Created(observed)]
        );
        drop(viewport);
        assert!(matches!(
            backend.drain_window_lifecycle_events().as_slice(),
            [Sdl3WindowLifecycleEvent::Retired { .. }]
        ));
        let before_failure = count_windows();
        backend
            .set_secondary_window_config(Sdl3WindowConfig {
                min_client_size: Some([1000, 1000]),
                ..Default::default()
            })
            .unwrap();
        let viewport = LiveViewport(context.binding().with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x410002)
        }));
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(viewport.0) });
        assert!(unsafe { (*viewport.0).PlatformUserData.is_null() });
        assert!(unsafe { (*viewport.0).PlatformHandle.is_null() });
        assert_eq!(count_windows(), before_failure);
        context
            .binding()
            .with_bound_context(|| unsafe { destroy_window_callback_for_test(viewport.0) });
        assert_eq!(count_windows(), before_failure);
        assert!(backend.drain_window_lifecycle_events().is_empty());
        drop(viewport);
        assert!(!backend.drain_faults().is_empty());
        backend.shutdown(&mut context).unwrap();
    });
}

#[test]
fn main_window_registration_is_available_without_multi_viewport_and_changes_on_restart() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        // SAFETY: The fixture retains the native window through both backend shutdowns.
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let history = backend.window_snapshot(window.id()).unwrap().unwrap();
        assert!(history.target.is_main);
        assert_eq!(history.target.instance_id.context_id(), context.id());
        assert_eq!(history.physical_client_size, Some([80, 60]));
        assert_eq!(adapter.clone().window_snapshots().unwrap(), vec![history]);
        assert_eq!(
            backend
                .with_window(history.target.instance_id, |window_ref| {
                    assert_eq!(window_ref.snapshot(), history);
                    window_ref.as_ptr()
                })
                .unwrap(),
            Some(window.raw())
        );

        backend.shutdown(&mut context).unwrap();
        assert!(matches!(
            adapter.window_snapshots(),
            Err(Sdl3BackendError::RuntimeDetached)
        ));
        assert!(matches!(
            adapter.with_window(history.target.instance_id, |_| ()),
            Err(Sdl3BackendError::RuntimeDetached)
        ));
        // SAFETY: The same host window remains live, but its old registration has retired.
        let mut replacement =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let current = replacement.window_snapshot(window.id()).unwrap().unwrap();
        assert_eq!(current.window_id, history.window_id);
        assert_ne!(current.target.instance_id, history.target.instance_id);
        assert!(
            replacement
                .window_snapshot_for_instance(history.target.instance_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(history.physical_client_size, Some([80, 60]));
        replacement.shutdown(&mut context).unwrap();
    });
}

#[test]
fn window_queries_follow_hidden_live_viewports_and_id_remaps_without_consuming_faults() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        // SAFETY: The fixture retains the native window until explicit backend shutdown.
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let original = backend.window_snapshot(window.id()).unwrap().unwrap();
        let binding = context.binding();
        let (viewport, platform_io, original_size) = binding.with_bound_context(|| unsafe {
            let viewport = sys::igGetMainViewport();
            let platform_io = sys::igGetPlatformIO_Nil();
            let size = (*platform_io).Viewports.Size;
            (*platform_io).Viewports.Size = 0;
            (*viewport).ID = original.target.viewport_id.raw().wrapping_add(1);
            (viewport, platform_io, size)
        });
        let observed = backend
            .window_snapshot_for_instance(original.target.instance_id)
            .unwrap()
            .unwrap();
        binding.with_bound_context(|| unsafe {
            (*platform_io).Viewports.Size = original_size;
            (*viewport).ID = original.target.viewport_id.raw();
        });
        assert_eq!(observed.target.instance_id, original.target.instance_id);
        assert_ne!(observed.target.viewport_id, original.target.viewport_id);
        assert_eq!(
            backend
                .window_snapshot_for_viewport(original.target.viewport_id)
                .unwrap(),
            Some(original)
        );

        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        adapter.control.record_viewport_creation_failed();
        assert_eq!(backend.window_snapshots().unwrap(), vec![original]);
        assert!(matches!(
            backend.drain_faults().as_slice(),
            [Sdl3BackendError::ViewportCreationFailed]
        ));

        let suspended = context.suspend_or_panic();
        let other = Context::create();
        let foreign = NativeWindowInstanceId::allocate(other.id()).unwrap();
        assert!(
            backend
                .window_snapshot_for_instance(foreign)
                .unwrap()
                .is_none()
        );
        assert!(
            backend
                .with_window(foreign, |_| panic!(
                    "foreign instance must not enter native scope"
                ))
                .unwrap()
                .is_none()
        );
        drop(other);
        let mut context = suspended.activate().unwrap();
        backend.shutdown(&mut context).unwrap();
    });
}

#[test]
fn secondary_window_callback_recreation_retires_the_previous_registration() {
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
    let address = (&mut viewport) as *mut sys::ImGuiViewport as usize;
    let create = |viewport: &mut sys::ImGuiViewport| {
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(viewport) });
        runtime
            .control
            .owned_viewports
            .borrow()
            .get(&address)
            .unwrap()
            .instance_id
    };
    let first = create(&mut viewport);
    context
        .binding()
        .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
    assert!(
        !runtime
            .control
            .owned_viewports
            .borrow()
            .contains_key(&address)
    );
    let second = create(&mut viewport);
    assert_ne!(first, second);
    assert_eq!(viewport.ID, 41);
    // A synthetic sidecar is not a member of the full native Context registry and cannot grant
    // public access even though callback tests intentionally accept it as a test fixture.
    assert!(
        runtime
            .control
            .window_snapshot_for_instance(second)
            .unwrap()
            .is_none()
    );
    context
        .binding()
        .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
    runtime.shutdown_platform(&mut context).unwrap();
}
