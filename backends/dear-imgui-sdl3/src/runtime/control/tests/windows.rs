use super::*;

fn with_native_window<R>(callback: impl FnOnce(&sdl3::video::Window) -> R) -> R {
    with_video_driver("dummy", callback)
}

fn with_video_driver<R>(driver: &str, callback: impl FnOnce(&sdl3::video::Window) -> R) -> R {
    with_video(driver, |video| {
        let window = video
            .window("SDL3 native registration test", 80, 60)
            .hidden()
            .build()
            .unwrap();
        callback(&window)
    })
}

fn with_video<R>(driver: &str, callback: impl FnOnce(&sdl3::VideoSubsystem) -> R) -> R {
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
    callback(&video)
}

#[test]
fn platform_sessions_route_window_events_cursor_and_text_input() {
    use sdl3_sys::{
        keyboard::{SDL_GetKeyboardFocus, SDL_TextInputActive},
        mouse::{SDL_CursorVisible, SDL_GetMouseFocus},
    };

    fn request_text_input(context: &Context) {
        context.binding().with_bound_context(|| unsafe {
            let platform = sys::igGetPlatformIO_Nil();
            let mut request = sys::ImGuiPlatformImeData {
                WantTextInput: true,
                ..Default::default()
            };
            (*platform).Platform_SetImeDataFn.unwrap()(
                sys::igGetCurrentContext(),
                sys::igGetMainViewport(),
                &mut request,
            );
        });
    }

    let _guard = crate::tests::test_guard();
    with_video("dummy", |video| {
        let mut window_a = video.window("Context A", 80, 60).hidden().build().unwrap();
        let mut window_b = video.window("Context B", 80, 60).hidden().build().unwrap();
        let mut context_a = Context::create();
        let mut context_b = Context::create();
        let mut platform_a =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_a, &window_a) }
                .unwrap();
        assert!(matches!(
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_b, &window_a) },
            Err(Sdl3BackendError::PlatformWindowOccupied { window_id }) if window_id == window_a.id()
        ));
        let mut platform_b =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_b, &window_b) }
                .unwrap();

        let event = sdl3::event::Event::TextInput {
            timestamp: 0,
            window_id: window_b.id(),
            text: "B".into(),
        };
        assert!(!platform_a.process_event(&mut context_a, &event).unwrap());
        assert!(platform_b.process_event(&mut context_b, &event).unwrap());
        assert!(platform_a.window_snapshot(window_b.id()).unwrap().is_none());
        assert!(platform_b.window_snapshot(window_a.id()).unwrap().is_none());

        assert!(window_a.show());
        assert_eq!(unsafe { SDL_GetKeyboardFocus() }, window_a.raw());
        assert_eq!(unsafe { SDL_GetMouseFocus() }, window_a.raw());
        context_a.io_mut().set_mouse_draw_cursor(true);
        platform_a.new_frame(&mut context_a).unwrap();
        assert!(!unsafe { SDL_CursorVisible() });
        platform_b.new_frame(&mut context_b).unwrap();
        assert!(!unsafe { SDL_CursorVisible() });
        request_text_input(&context_b);
        assert!(!unsafe { SDL_TextInputActive(window_a.raw()) });
        request_text_input(&context_a);
        assert!(unsafe { SDL_TextInputActive(window_a.raw()) });

        assert!(window_b.show());
        assert_eq!(unsafe { SDL_GetKeyboardFocus() }, window_b.raw());
        assert_eq!(unsafe { SDL_GetMouseFocus() }, window_b.raw());
        platform_b.new_frame(&mut context_b).unwrap();
        assert!(unsafe { SDL_CursorVisible() });
        assert!(unsafe { SDL_TextInputActive(window_b.raw()) });
        assert!(!unsafe { SDL_TextInputActive(window_a.raw()) });
        platform_a.shutdown(&mut context_a).unwrap();
        assert!(unsafe { SDL_CursorVisible() });
        assert!(unsafe { SDL_TextInputActive(window_b.raw()) });
        assert!(platform_b.process_event(&mut context_b, &event).unwrap());
        platform_b.shutdown(&mut context_b).unwrap();
        assert!(!unsafe { SDL_TextInputActive(window_b.raw()) });
    });
}

#[test]
fn platform_sessions_retain_sdl_video_until_the_last_host_is_released() {
    use sdl3_sys::init::{SDL_INIT_VIDEO, SDL_InitFlags, SDL_WasInit};

    let _guard = crate::tests::test_guard();
    let (window_a, window_b) = with_video("dummy", |video| {
        (
            video.window("Context A", 80, 60).hidden().build().unwrap(),
            video.window("Context B", 80, 60).hidden().build().unwrap(),
        )
    });
    let mut context_a = Context::create();
    let mut context_b = Context::create();
    let mut platform_a =
        unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_a, &window_a) }.unwrap();
    let mut platform_b =
        unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_b, &window_b) }.unwrap();
    let id_b = sdl3_sys::video::SDL_WindowID(window_b.id());
    let raw_b = window_b.raw();
    drop(window_a);
    drop(window_b);

    platform_a.shutdown(&mut context_a).unwrap();
    assert_eq!(unsafe { SDL_WasInit(SDL_INIT_VIDEO) }, SDL_INIT_VIDEO);
    assert_eq!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(id_b) }, raw_b);
    platform_b.new_frame(&mut context_b).unwrap();
    platform_b.shutdown(&mut context_b).unwrap();
    assert_eq!(
        unsafe { SDL_WasInit(SDL_INIT_VIDEO) },
        SDL_InitFlags::default()
    );
}

#[test]
fn main_native_lease_outlives_context_backend_and_original_sdl_window() {
    let _guard = crate::tests::test_guard();
    let lease = with_native_window(|window| {
        let mut context = Context::create();
        let backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let target = backend
            .window_snapshot(window.id())
            .unwrap()
            .unwrap()
            .target;
        let lease = adapter
            .acquire_native_window_lease(target.instance_id)
            .unwrap()
            .unwrap();
        assert_eq!(lease.target(), target);
        assert!(!lease.is_retired());
        drop(context);
        drop(backend);
        assert!(lease.is_retired());
        lease
    });
    let id = sdl3_sys::video::SDL_WindowID(lease.snapshot().window_id);
    assert_eq!(
        unsafe { sdl3_sys::video::SDL_GetWindowFromID(id) },
        lease.as_ptr()
    );
    let retained = lease.clone();
    drop(lease);
    assert_eq!(
        unsafe { sdl3_sys::video::SDL_GetWindowFromID(id) },
        retained.as_ptr()
    );
    drop(retained);
    assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(id) }.is_null());
}

#[test]
fn callback_drop_keeps_observed_registration_after_retirement_without_aliasing_reattachment() {
    let _guard = crate::tests::test_guard();
    with_native_window(|window| {
        let mut context = Context::create();
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let handoff = backend.callback_event_handoff();
        let original = backend.window_snapshot(window.id()).unwrap().unwrap();
        let path = std::ffi::CString::new("queued-asset.png").unwrap();
        let source = std::ffi::CString::new("asset browser").unwrap();
        let raw = sdl3_sys::events::SDL_Event {
            drop: sdl3_sys::events::SDL_DropEvent {
                r#type: sdl3_sys::events::SDL_EVENT_DROP_FILE,
                timestamp: unsafe { sdl3_sys::timer::SDL_GetTicksNS() },
                windowID: sdl3_sys::video::SDL_WindowID(window.id()),
                x: 13.0,
                y: 24.0,
                source: source.as_ptr(),
                data: path.as_ptr(),
                ..Default::default()
            },
        };
        unsafe { handoff.push_from_callback(&raw) };
        drop((path, source));
        let event = handoff.drain().pop().unwrap();
        let drop = event.drop_event().unwrap();
        assert_eq!(drop.observed_target, Some(original.target));
        assert_eq!(
            backend.resolve_drop_target(drop).unwrap(),
            crate::Sdl3DropTarget::Current(original.target)
        );
        backend.shutdown(&mut context).unwrap();
        assert_eq!(
            backend.resolve_drop_target(drop).unwrap(),
            crate::Sdl3DropTarget::Retired(original.target)
        );
        let mut replacement =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        assert_eq!(
            replacement.resolve_drop_target(drop).unwrap(),
            crate::Sdl3DropTarget::Retired(original.target)
        );
        assert_eq!(
            drop.payload,
            crate::Sdl3DropPayload::File("queued-asset.png".into())
        );
        assert_eq!(drop.source.as_deref(), Some("asset browser"));
        replacement.shutdown(&mut context).unwrap();
    });
}

#[cfg(all(target_os = "windows", sdl3_native_self_test))]
#[test]
fn native_secondary_window_rejects_parent_from_another_session() {
    let _guard = crate::tests::test_guard();
    with_video("windows", |video| {
        let window_a = video.window("Context A", 80, 60).hidden().build().unwrap();
        let window_b = video.window("Context B", 80, 60).hidden().build().unwrap();
        let mut context_a = Context::create();
        let mut context_b = Context::create();
        let mut platform_a =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_a, &window_a) }
                .unwrap();
        let mut platform_b =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context_b, &window_b) }
                .unwrap();
        let foreign_parent = context_b
            .binding()
            .with_bound_context(|| unsafe { sys::igGetMainViewport() });
        let binding_a = context_a.binding();
        let child = binding_a.with_bound_context(|| unsafe {
            let child = crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x610003);
            assert!(!child.is_null());
            create_window_callback_for_test(child);
            // Match UpdatePlatformWindows so normal shutdown dispatches the destroy callback.
            (*child).PlatformWindowCreated = true;
            child
        });
        let before = platform_a
            .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x610003))
            .unwrap()
            .unwrap();
        assert_eq!(
            before.state.native_owner.unwrap().unwrap().window_id,
            window_a.id()
        );
        binding_a.with_bound_context(|| unsafe {
            (*child).ParentViewport = foreign_parent;
            (*sys::igGetPlatformIO_Nil()).Platform_UpdateWindow.unwrap()(child);
            (*child).ParentViewport = sys::igGetMainViewport();
            let window = sdl3_sys::video::SDL_GetWindowFromID(sdl3_sys::video::SDL_WindowID(
                before.window_id,
            ));
            assert_eq!(sdl3_sys::video::SDL_GetWindowParent(window), window_a.raw());
        });
        let faults = platform_a.drain_faults();
        assert!(
            matches!(
                faults.as_slice(),
                [Sdl3BackendError::PlatformStateReplaced {
                    field: "Viewport.ParentViewport"
                }]
            ),
            "unexpected parent validation faults: {faults:?}"
        );
        platform_a.shutdown(&mut context_a).unwrap();
        let child_id = sdl3_sys::video::SDL_WindowID(before.window_id);
        assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) }.is_null());
        binding_a.with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_delete_live_viewport_for_test(child);
        });
        assert!(platform_b.window_snapshot(window_b.id()).unwrap().is_some());
        platform_b.shutdown(&mut context_b).unwrap();
    });
}

#[cfg(all(target_os = "windows", sdl3_native_self_test))]
#[test]
fn native_lease_freezes_owner_changes_and_parent_retirement_does_not_destroy_child() {
    let _guard = crate::tests::test_guard();
    with_video_driver("windows", |window| {
        struct LiveViewport(*mut sys::ImGuiViewport);
        impl Drop for LiveViewport {
            fn drop(&mut self) {
                unsafe { crate::core::ffi::dear_imgui_sdl3_delete_live_viewport_for_test(self.0) };
            }
        }
        let mut context = Context::create();
        let mut backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let parent = LiveViewport(context.binding().with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x610001)
        }));
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(parent.0) });
        let child = LiveViewport(context.binding().with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x610002)
        }));
        context.binding().with_bound_context(|| unsafe {
            (*child.0).ParentViewport = parent.0;
            create_window_callback_for_test(child.0)
        });
        let parent_snapshot = backend
            .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x610001))
            .unwrap()
            .unwrap();
        let child_snapshot = backend
            .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x610002))
            .unwrap()
            .unwrap();
        let surface_lease = adapter
            .acquire_surface_window_lease(child_snapshot.target.instance_id)
            .unwrap()
            .unwrap();
        let lease = adapter
            .acquire_native_window_lease(child_snapshot.target.instance_id)
            .unwrap()
            .unwrap();
        let parent_id = sdl3_sys::video::SDL_WindowID(parent_snapshot.window_id);
        let child_id = sdl3_sys::video::SDL_WindowID(child_snapshot.window_id);
        let parent_pointer = unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) };
        drop(parent);
        assert!(
            lease.is_retired(),
            "retiring any captured owner retires the pre-submit capability"
        );
        assert!(
            backend
                .window_snapshot_for_instance(parent_snapshot.target.instance_id)
                .unwrap()
                .is_none()
        );
        assert!(!unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) }.is_null());
        context.binding().with_bound_context(|| unsafe {
            (*child.0).ParentViewport = sys::igGetMainViewport();
            (*sys::igGetPlatformIO_Nil()).Platform_UpdateWindow.unwrap()(child.0);
        });
        assert_eq!(
            unsafe { sdl3_sys::video::SDL_GetWindowParent(lease.as_ptr()) },
            parent_pointer
        );
        assert!(matches!(
            backend.drain_faults().as_slice(),
            [Sdl3BackendError::NativeWindowOwnerLeased]
        ));
        drop(lease);
        // Persistent native ownership still keeps the retired parent alive until the live child
        // has actually moved to its new owner, even with no outstanding renderer work.
        assert!(!unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) }.is_null());
        context.binding().with_bound_context(|| unsafe {
            (*sys::igGetPlatformIO_Nil()).Platform_UpdateWindow.unwrap()(child.0)
        });
        assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) }.is_null());
        assert!(!unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) }.is_null());
        assert_eq!(
            unsafe { sdl3_sys::video::SDL_GetWindowParent(surface_lease.as_ptr()) },
            window.raw(),
        );
        assert!(backend.drain_faults().is_empty());
        drop(child);
        assert_eq!(
            unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) },
            surface_lease.as_ptr(),
        );
        drop(surface_lease);
        assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) }.is_null());
        backend.shutdown(&mut context).unwrap();
    });
}

#[cfg(all(target_os = "windows", sdl3_native_self_test))]
#[test]
fn retired_secondary_native_lease_releases_child_before_owner_after_context_teardown() {
    let _guard = crate::tests::test_guard();
    let (lease, parent_id) = with_video_driver("windows", |window| {
        let mut context = Context::create();
        let backend =
            unsafe { crate::Sdl3PlatformBackend::init_for_other(&mut context, window) }.unwrap();
        let adapter = backend.viewport_renderer_adapter(&context).unwrap();
        let (parent, child) = context.binding().with_bound_context(|| unsafe {
            let parent = crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x620001);
            create_window_callback_for_test(parent);
            let child = crate::core::ffi::dear_imgui_sdl3_create_live_viewport_for_test(0x620002);
            (*child).ParentViewport = parent;
            create_window_callback_for_test(child);
            (parent, child)
        });
        let child_snapshot = backend
            .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x620002))
            .unwrap()
            .unwrap();
        let parent_id = sdl3_sys::video::SDL_WindowID(
            backend
                .window_snapshot_for_viewport(dear_imgui_rs::Id::from(0x620001))
                .unwrap()
                .unwrap()
                .window_id,
        );
        let lease = adapter
            .acquire_native_window_lease(child_snapshot.target.instance_id)
            .unwrap()
            .unwrap();
        // Retire the native sidecars while their allocator still belongs to this live Context.
        context.binding().with_bound_context(|| unsafe {
            crate::core::ffi::dear_imgui_sdl3_delete_live_viewport_for_test(child);
            crate::core::ffi::dear_imgui_sdl3_delete_live_viewport_for_test(parent);
        });
        drop(context);
        drop(backend);
        (lease, parent_id)
    });
    let child_id = sdl3_sys::video::SDL_WindowID(lease.snapshot().window_id);
    assert!(lease.is_retired());
    assert_eq!(
        unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) },
        lease.as_ptr()
    );
    assert!(!unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) }.is_null());
    let pending = lease.clone();
    drop(lease);
    assert!(!unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) }.is_null());
    drop(pending);
    assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(child_id) }.is_null());
    assert!(unsafe { sdl3_sys::video::SDL_GetWindowFromID(parent_id) }.is_null());
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

#[cfg(all(target_os = "windows", sdl3_native_self_test))]
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
