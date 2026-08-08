use super::*;

#[test]
fn platform_only_shutdown_rejects_an_active_renderer_before_closing_the_frame() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::clone(&platform_count),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let mut renderer = context
        .register_attachment::<ActiveExternalRendererMarker>(
            ContextAttachmentRole::Renderer,
            Rc::new(ActiveExternalRendererAttachment),
        )
        .unwrap();
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("the attachment lifecycle test uses a headless legacy renderer")
        .build();
    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [128.0, 128.0],
        1.0 / 60.0,
    ));
    context
        .frame()
        .text("platform release must not close this frame");

    assert!(matches!(
        runtime.shutdown_platform(&mut context),
        Err(Sdl3BackendError::PlatformAttachmentRelease(
            dear_imgui_rs::ContextPlatformAttachmentReleaseError::RendererActive
        ))
    ));
    assert_eq!(
        context.frame_lifecycle_state(),
        dear_imgui_rs::FrameLifecycleState::InFrame
    );
    assert_eq!(runtime.control.state(), RuntimeState::Attached);
    assert_eq!(platform_count.get(), 0);

    assert_eq!(renderer.detach(), Ok(true));
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(platform_count.get(), 1);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn vulkan_provider_requires_matching_initialization_mode_and_callback() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut other = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    assert!(matches!(
        other.acquire_vulkan_surface_provider(&context),
        Err(Sdl3BackendError::VulkanSurfaceProviderRequiresVulkan)
    ));
    other.shutdown_platform(&mut context).unwrap();
    drop(context);

    let mut context = Context::create();
    let mut vulkan =
        synthetic_vulkan_registration_with_callback(&mut context, Rc::new(Cell::new(0)), None);
    assert!(matches!(
        vulkan.acquire_vulkan_surface_provider(&context),
        Err(Sdl3BackendError::VulkanSurfaceCallbackUnavailable)
    ));
    vulkan.shutdown_platform(&mut context).unwrap();
}

#[cfg(feature = "multi-viewport")]
#[test]
fn vulkan_provider_is_exclusive_and_blocks_retryable_platform_shutdown() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let mut runtime = synthetic_vulkan_registration(&mut context, Rc::clone(&platform_count));

    let provider = runtime
        .acquire_vulkan_surface_provider(&context)
        .expect("Vulkan SDL runtime must issue one provider");
    assert!(matches!(
        runtime.acquire_vulkan_surface_provider(&context),
        Err(Sdl3BackendError::VulkanSurfaceProviderAlreadyLeased)
    ));
    assert!(matches!(
        runtime.shutdown_platform(&mut context),
        Err(Sdl3BackendError::VulkanSurfaceProviderActive)
    ));
    assert_eq!(runtime.control.state(), RuntimeState::Attached);
    assert_eq!(platform_count.get(), 0);

    drop(provider);
    runtime.shutdown_platform(&mut context).unwrap();
    assert_eq!(platform_count.get(), 1);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn vulkan_provider_validates_callback_and_viewport_immediately_before_invocation() {
    let _guard = crate::tests::test_guard();
    VULKAN_SURFACE_CREATE_COUNT.with(|count| count.set(0));
    FOREIGN_VULKAN_SURFACE_CREATE_COUNT.with(|count| count.set(0));
    let mut context = Context::create();
    let mut runtime = synthetic_vulkan_registration(&mut context, Rc::new(Cell::new(0)));
    let provider = runtime
        .acquire_vulkan_surface_provider(&context)
        .expect("Vulkan SDL runtime must issue a provider");
    let mut viewport = sys::ImGuiViewport::default();
    viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
    viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    runtime
        .control
        .remember_owned_viewport(&mut viewport, unsafe {
            ViewportPlatformState::capture(&viewport)
        });

    let surface = context.binding().with_bound_context(|| unsafe {
        provider.create_surface(Viewport::from_raw_mut(&mut viewport), 0x4141)
    });
    assert_eq!(surface.unwrap(), 0x5151);
    assert_eq!(VULKAN_SURFACE_CREATE_COUNT.with(Cell::get), 1);

    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetPlatformIO_Nil()).Platform_CreateVkSurface = Some(foreign_create_vk_surface);
    });
    let error = context.binding().with_bound_context(|| unsafe {
        provider
            .create_surface(Viewport::from_raw_mut(&mut viewport), 0x4141)
            .unwrap_err()
    });
    assert!(matches!(
        error,
        Sdl3VulkanSurfaceError::Backend(Sdl3BackendError::PlatformCallbackReplaced {
            callback: "Platform_CreateVkSurface"
        })
    ));
    assert_eq!(FOREIGN_VULKAN_SURFACE_CREATE_COUNT.with(Cell::get), 0);

    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetPlatformIO_Nil()).Platform_CreateVkSurface = Some(synthetic_create_vk_surface);
    });
    drop(provider);
    runtime.shutdown_platform(&mut context).unwrap();
}

#[cfg(feature = "multi-viewport")]
#[test]
fn context_renderer_phase_releases_provider_before_sdl_platform_phase() {
    struct ProviderAttachment(RefCell<Option<Sdl3VulkanSurfaceProvider>>);
    impl ContextAttachment for ProviderAttachment {
        fn release_renderer_resources(
            &self,
            _context: &ContextTeardown<'_>,
        ) -> Result<(), ContextAttachmentTeardownError> {
            self.0.borrow_mut().take();
            Ok(())
        }
    }
    struct ProviderAttachmentMarker;

    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let platform_count = Rc::new(Cell::new(0));
    let runtime = synthetic_vulkan_registration(&mut context, Rc::clone(&platform_count));
    let provider = runtime
        .acquire_vulkan_surface_provider(&context)
        .expect("Vulkan SDL runtime must issue a provider");
    let renderer_attachment = context
        .register_attachment::<ProviderAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::new(ProviderAttachment(RefCell::new(Some(provider)))),
        )
        .unwrap();
    renderer_attachment.defer_to_context();
    drop(runtime);

    drop(context);

    assert_eq!(platform_count.get(), 1);
}

#[test]
fn platform_session_is_exclusive_and_reusable_across_contexts() {
    let _guard = crate::tests::test_guard();
    let mut context_a = Context::create();
    let runtime_a = synthetic_claimed_registration(
        &mut context_a,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let id_a = context_a.id();
    let key_a = runtime_a.control.platform_io_key.get();
    assert_eq!(
        with_current_runtime(|control| control.binding.id()),
        Some(id_a)
    );

    let suspended_a = context_a.suspend_or_panic();
    let mut blocked_context = Context::create();
    let error = RuntimeRegistration::prepare_with_backend(
        &mut blocked_context,
        None,
        None,
        None,
        PlatformGraphicsKind::Other,
        NativeRendererKind::None,
    )
    .unwrap_err();
    assert!(matches!(error, Sdl3BackendError::PlatformSessionOccupied));
    drop(blocked_context);

    let mut context_a = suspended_a.activate().expect("Context A should reactivate");
    assert_eq!(
        with_current_runtime(|control| control.binding.id()),
        Some(id_a)
    );
    let mut runtime_a = runtime_a;
    runtime_a.shutdown_platform(&mut context_a).unwrap();
    assert!(!registry_contains(key_a));
    drop(context_a);

    let mut context_b = Context::create();
    let mut runtime_b = synthetic_claimed_registration(
        &mut context_b,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );
    let id_b = context_b.id();
    let key_b = runtime_b.control.platform_io_key.get();
    assert!(registry_contains(key_b));
    assert_eq!(
        with_current_runtime(|control| control.binding.id()),
        Some(id_b)
    );
    runtime_b.shutdown_platform(&mut context_b).unwrap();
    assert!(!registry_contains(key_b));
}

#[test]
fn runtime_entry_detects_callback_drift_while_unwinding() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let mut runtime = synthetic_claimed_registration(
        &mut context,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
    );

    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _entry = runtime.control.enter(&context).unwrap();
        context.binding().with_bound_context(|| unsafe {
            (*sys::igGetPlatformIO_Nil()).Platform_CreateWindow = Some(foreign_create_window);
        });
        panic!("synthetic operation failure");
    }));
    assert!(unwind.is_err());
    assert!(runtime.control.faults.borrow().iter().any(|fault| {
        matches!(
            fault,
            RuntimeFault::CallbackReplaced("Platform_CreateWindow")
        )
    }));
    assert!(matches!(
        runtime.poll_fault(),
        Err(Sdl3BackendError::PlatformCallbackReplaced {
            callback: "Platform_CreateWindow"
        })
    ));

    let shutdown = runtime.shutdown_platform(&mut context);
    assert!(matches!(
        shutdown,
        Ok(())
            | Err(Sdl3BackendError::PlatformCallbackReplaced {
                callback: "Platform_CreateWindow"
            })
    ));
    context.binding().with_bound_context(|| unsafe {
        (*sys::igGetPlatformIO_Nil()).Platform_CreateWindow = None;
    });
}

struct TeardownPhaseObserver {
    renderer_count: Rc<Cell<usize>>,
    platform_count: Rc<Cell<usize>>,
    renderer_phase_counts: Rc<Cell<(usize, usize)>>,
    platform_phase_counts: Rc<Cell<(usize, usize)>>,
}

impl ContextAttachment for TeardownPhaseObserver {
    fn release_renderer_resources(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.renderer_phase_counts
            .set((self.renderer_count.get(), self.platform_count.get()));
        Ok(())
    }

    fn release_platform_windows(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.platform_phase_counts
            .set((self.renderer_count.get(), self.platform_count.get()));
        Ok(())
    }
}

struct TeardownPhaseObserverMarker;

#[test]
fn context_first_shutdown_runs_each_phase_once_in_order() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let runtime = test_registration(
        &mut context,
        Rc::clone(&renderer_count),
        Rc::clone(&platform_count),
    );
    let renderer_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
    let platform_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
    let _observer = context
        .register_attachment::<TeardownPhaseObserverMarker>(
            ContextAttachmentRole::Extension,
            Rc::new(TeardownPhaseObserver {
                renderer_count: Rc::clone(&renderer_count),
                platform_count: Rc::clone(&platform_count),
                renderer_phase_counts: Rc::clone(&renderer_phase_counts),
                platform_phase_counts: Rc::clone(&platform_phase_counts),
            }),
        )
        .unwrap();
    let control = Rc::clone(&runtime.control);

    drop(context);

    assert_eq!(renderer_phase_counts.get(), (0, 0));
    assert_eq!(platform_phase_counts.get(), (1, 1));
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(control.phase_log(), ["platform", "renderer"]);
    assert_eq!(control.state(), RuntimeState::Detached);
    drop(runtime);
    assert_eq!(control.state(), RuntimeState::ResourceDropped);
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn wrapper_first_and_repeated_shutdown_are_idempotent_after_move() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let runtime = test_registration(
        &mut context,
        Rc::clone(&renderer_count),
        Rc::clone(&platform_count),
    );
    let control_address = Rc::as_ptr(&runtime.control);
    let mut slot = Some(runtime);
    let mut moved = slot.take().unwrap();
    assert_eq!(Rc::as_ptr(&moved.control), control_address);

    moved.shutdown_platform(&mut context).unwrap();
    moved.shutdown_platform(&mut context).unwrap();

    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(moved.control.phase_log(), ["platform", "renderer"]);
    drop(moved);
    drop(context);
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn platform_shutdown_keeps_platform_destroy_callbacks_live() {
    let _guard = crate::tests::test_guard();
    let mut context = Context::create();
    let viewport = Rc::new(RefCell::new(sys::ImGuiViewport::default()));
    let platform_shutdown_hook: Rc<dyn Fn()> = {
        let viewport = Rc::clone(&viewport);
        Rc::new(move || unsafe {
            destroy_window_callback_for_test(&mut *viewport.borrow_mut());
        })
    };
    let platform_count = Rc::new(Cell::new(0));
    let observed_backend_data = Rc::new(Cell::new(0));
    let observed_main_viewport_data = Rc::new(Cell::new(0));
    let mut runtime = synthetic_claimed_registration_with_renderer(
        &mut context,
        Some(Rc::new(|| {})),
        Some(platform_shutdown_hook),
        Rc::clone(&platform_count),
        observed_backend_data,
        observed_main_viewport_data,
        synthetic_create_window,
    );

    context.binding().with_bound_context(|| unsafe {
        create_window_callback_for_test(&mut *viewport.borrow_mut());
    });
    assert_eq!(
        viewport.borrow().PlatformUserData as usize,
        OWNED_VIEWPORT_DATA
    );

    runtime.shutdown_platform(&mut context).unwrap();

    assert!(viewport.borrow().PlatformUserData.is_null());
    assert!(viewport.borrow().PlatformHandle.is_null());
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn explicit_shutdown_normalizes_an_open_frame_before_native_release() {
    let _guard = crate::tests::test_guard();
    let frame_open_during_platform_shutdown = Rc::new(Cell::new(true));
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let mut runtime = registration_with_lifecycle(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || renderer_count.set(renderer_count.get() + 1))
        }),
        {
            let platform_count = Rc::clone(&platform_count);
            let frame_open = Rc::clone(&frame_open_during_platform_shutdown);
            Rc::new(move || unsafe {
                platform_count.set(platform_count.get() + 1);
                let context = sys::igGetCurrentContext();
                frame_open.set(!context.is_null() && (*context).WithinFrameScope);
            })
        },
    );
    runtime.control.platform_initialized.set(true);
    runtime.control.renderer_initialized.set(true);
    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [320.0, 240.0],
        1.0 / 60.0,
    ));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("the shutdown lifecycle test uses a headless legacy renderer")
        .build();
    context.frame().text("close before SDL teardown");

    runtime.shutdown_platform(&mut context).unwrap();

    assert!(!frame_open_during_platform_shutdown.get());
    assert_eq!(
        context.frame_lifecycle_state(),
        dear_imgui_rs::FrameLifecycleState::Idle
    );
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);

    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [320.0, 240.0],
        1.0 / 60.0,
    ));
    context.frame().text("context remains reusable");
    assert!(context.end_frame());
}

#[test]
fn wrapper_drop_defers_each_phase_to_context_teardown() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let runtime = test_registration(
        &mut context,
        Rc::clone(&renderer_count),
        Rc::clone(&platform_count),
    );
    let control = Rc::clone(&runtime.control);

    drop(runtime);

    assert_eq!(renderer_count.get(), 0);
    assert_eq!(platform_count.get(), 0);
    assert!(control.phase_log().is_empty());
    assert_eq!(control.state(), RuntimeState::Attached);
    drop(context);
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(control.phase_log(), ["platform", "renderer"]);
    assert_eq!(control.state(), RuntimeState::Detached);
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
#[test]
fn wrapper_drop_keeps_uninstalled_texture_proxies_alive_until_context_teardown() {
    let _guard = crate::tests::test_guard();
    let destroy_count = Rc::new(Cell::new(0));
    let device_destroy_count = Rc::new(Cell::new(0));
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let texture = dear_imgui_rs::render::SnapshotTextureId::FontAtlas {
        context: context.id(),
        stamp: 1,
        generation: 1,
    };
    let mut runtime = registration_with_backend_lifecycle_and_texture_update(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || renderer_count.set(renderer_count.get() + 1))
        }),
        Some({
            let device_destroy_count = Rc::clone(&device_destroy_count);
            Rc::new(move || device_destroy_count.set(device_destroy_count.get() + 1))
        }),
        Some({
            let destroy_count = Rc::clone(&destroy_count);
            Rc::new(move |texture: &mut TextureData| {
                if texture.status() == dear_imgui_rs::TextureStatus::WantDestroy {
                    destroy_count.set(destroy_count.get() + 1);
                    unsafe {
                        texture.set_status(dear_imgui_rs::TextureStatus::Destroyed);
                    }
                }
            })
        }),
        {
            let platform_count = Rc::clone(&platform_count);
            Rc::new(move || platform_count.set(platform_count.get() + 1))
        },
        PlatformGraphicsKind::Other,
        NativeRendererKind::None,
    );
    runtime
        .control
        .renderer_textures
        .borrow_mut()
        .insert_uninstalled_for_test(texture);
    runtime.control.platform_initialized.set(true);
    runtime.control.renderer_initialized.set(true);
    runtime.install_renderer_consumer(context.create_synchronous_renderer_consumer().unwrap());

    drop(runtime);
    assert_eq!(destroy_count.get(), 0);
    assert_eq!(device_destroy_count.get(), 0);
    drop(context);

    assert_eq!(destroy_count.get(), 1);
    assert_eq!(device_destroy_count.get(), 1);
    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
}

#[test]
fn deferred_owner_can_finish_fallible_teardown_before_context_drop() {
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
                    panic!("synthetic renderer shutdown failure");
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

    drop(runtime);

    assert_eq!(renderer_count.get(), 0);
    assert_eq!(platform_count.get(), 0);
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
    context
        .binding()
        .with_bound_context(|| control.shutdown_bound_for_attachment())
        .unwrap();
    drop(context);

    assert_eq!(renderer_count.get(), 2);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
    assert_eq!(control.state(), RuntimeState::Detached);
}

#[test]
fn explicit_shutdown_reports_renderer_panic_after_completing_cleanup() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let mut runtime = registration_with_lifecycle(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || {
                let attempt = renderer_count.get() + 1;
                renderer_count.set(attempt);
                if attempt == 1 {
                    panic!("synthetic explicit renderer failure");
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

    assert!(matches!(
        runtime.shutdown_platform(&mut context),
        Err(Sdl3BackendError::ShutdownPanicked {
            phase: "renderer resources"
        })
    ));
    assert_eq!(renderer_count.get(), 2);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(runtime.control.state(), RuntimeState::Detached);
    runtime.poll_fault().unwrap();
    runtime.shutdown_platform(&mut context).unwrap();
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
#[test]
fn renderer_shutdown_retries_before_texture_cleanup_after_platform_release() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    let mut runtime = registration_with_lifecycle(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || {
                let attempt = renderer_count.get() + 1;
                renderer_count.set(attempt);
                if attempt == 1 {
                    panic!("synthetic composite renderer failure");
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
    runtime.install_renderer_consumer(consumer);

    let result = runtime.shutdown_renderer(&mut context);

    assert!(matches!(
        result,
        Err(Sdl3BackendError::ShutdownPanicked {
            phase: "renderer resources"
        })
    ));
    assert_eq!(renderer_count.get(), 2);
    assert_eq!(platform_count.get(), 1);
    assert_eq!(runtime.control.state(), RuntimeState::Detached);
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
#[test]
fn device_object_destroy_commits_synchronous_texture_reset_after_native_destruction() {
    let _guard = crate::tests::test_guard();
    let native_destroy_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    context
        .io_mut()
        .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    let mut runtime = registration_with_lifecycle(&mut context, None, Rc::new(|| {}));
    runtime.install_renderer_consumer(consumer);

    runtime
        .reset_renderer_device_objects(&mut context, {
            let native_destroy_count = Rc::clone(&native_destroy_count);
            move || native_destroy_count.set(native_destroy_count.get() + 1)
        })
        .unwrap();

    assert_eq!(native_destroy_count.get(), 1);
    assert!(runtime.renderer_consumer.is_some());
    runtime.shutdown_platform(&mut context).unwrap();
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
#[test]
fn renderer_shutdown_releases_the_synchronous_consumer_after_native_teardown() {
    let _guard = crate::tests::test_guard();
    let renderer_count = Rc::new(Cell::new(0));
    let platform_count = Rc::new(Cell::new(0));
    let mut context = Context::create();
    context
        .io_mut()
        .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
    let consumer = context.create_synchronous_renderer_consumer().unwrap();
    let mut runtime = registration_with_lifecycle(
        &mut context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || renderer_count.set(renderer_count.get() + 1))
        }),
        {
            let platform_count = Rc::clone(&platform_count);
            Rc::new(move || platform_count.set(platform_count.get() + 1))
        },
    );
    runtime.control.platform_initialized.set(true);
    runtime.control.renderer_initialized.set(true);
    runtime.install_renderer_consumer(consumer);

    runtime.shutdown_renderer(&mut context).unwrap();

    assert_eq!(renderer_count.get(), 1);
    assert_eq!(platform_count.get(), 1);
    assert!(runtime.renderer_consumer.is_none());
    assert!(runtime.control.renderer_consumer.borrow().is_none());
    assert_eq!(runtime.control.state(), RuntimeState::Detached);
}
