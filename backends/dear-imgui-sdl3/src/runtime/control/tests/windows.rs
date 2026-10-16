use super::*;

fn with_native_window(callback: impl FnOnce(&sdl3::video::Window)) {
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
        "dummy",
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
