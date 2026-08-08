#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use dear_imgui_rs::FrameLifecycleState;
    use winit::error::EventLoopError;
    use winit::window::WindowId;

    use crate::runtime::ownership::{OrderedRuntimeOwner, RuntimeOwnershipLifecycle};
    use crate::runtime::runner::{
        GpuFaultDisposition, RunnerOwnership, classify_uncaptured_gpu_fault,
        dispatch_live_window_event, initialize_runtime_once, should_process_runtime_event,
        uncaptured_gpu_fault, validate_config,
    };
    use crate::runtime::shutdown::{
        RuntimeShutdownErrors, ShutdownCoordinator, finish_runtime_shutdown, resolve_run_result,
    };
    use crate::runtime::state::GpuFaultKind;
    use crate::runtime::surface::build_frame;
    use crate::{AppConfig, ApplicationStage, GpuGeneration, RunError};
    use dear_imgui_rs::ConfigFlags;

    struct DropProbe {
        event: &'static str,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    #[test]
    fn uncaptured_gpu_faults_preserve_their_terminal_classification() {
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::OutOfMemory, "oom".to_owned()),
            RunError::GpuOutOfMemory { message } if message == "oom"
        ));
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::Validation, "invalid".to_owned()),
            RunError::GpuValidation { message } if message == "invalid"
        ));
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::Internal, "driver".to_owned()),
            RunError::GpuInternal { message } if message == "driver"
        ));
    }

    #[test]
    fn uncaptured_gpu_fault_dispatch_is_generation_bound() {
        let current = GpuGeneration(8);
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                Some(current),
                GpuGeneration(7),
                GpuFaultKind::Validation,
                "stale".to_owned(),
            ),
            GpuFaultDisposition::IgnoreStale
        ));
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                Some(current),
                current,
                GpuFaultKind::OutOfMemory,
                "live oom".to_owned(),
            ),
            GpuFaultDisposition::Terminate(RunError::GpuOutOfMemory { message })
                if message == "live oom"
        ));
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                None,
                current,
                GpuFaultKind::Internal,
                "early".to_owned(),
            ),
            GpuFaultDisposition::Terminate(RunError::Recovery { message })
                if message.contains("before runtime initialization")
        ));
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.events.borrow_mut().push(self.event);
        }
    }

    #[test]
    fn runner_ownership_drops_runtime_before_application_during_unwind() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                let _ownership = RunnerOwnership {
                    runtime: Some(DropProbe {
                        event: "drop_runtime",
                        events: Rc::clone(&events),
                    }),
                    application: DropProbe {
                        event: "drop_application",
                        events,
                    },
                };
                panic!("injected runner callback panic");
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(*events.borrow(), ["drop_runtime", "drop_application"]);
    }

    struct ProbeRuntimeOwnership {
        events: Rc<RefCell<Vec<&'static str>>>,
        renderer_release: ProbeRelease,
        platform_release: ProbeRelease,
        renderer: DropProbe,
        platform: DropProbe,
        context: DropProbe,
        window: DropProbe,
    }

    #[derive(Clone, Copy)]
    enum ProbeRelease {
        Succeeds,
        Fails,
        Panics,
    }

    impl ProbeRuntimeOwnership {
        fn new(events: Rc<RefCell<Vec<&'static str>>>, renderer_release: ProbeRelease) -> Self {
            Self::with_platform_release(events, renderer_release, ProbeRelease::Succeeds)
        }

        fn with_platform_release(
            events: Rc<RefCell<Vec<&'static str>>>,
            renderer_release: ProbeRelease,
            platform_release: ProbeRelease,
        ) -> Self {
            Self {
                events: Rc::clone(&events),
                renderer_release,
                platform_release,
                renderer: DropProbe {
                    event: "drop_renderer",
                    events: Rc::clone(&events),
                },
                platform: DropProbe {
                    event: "drop_platform",
                    events: Rc::clone(&events),
                },
                context: DropProbe {
                    event: "drop_context",
                    events: Rc::clone(&events),
                },
                window: DropProbe {
                    event: "drop_window",
                    events,
                },
            }
        }
    }

    impl RuntimeOwnershipLifecycle for ProbeRuntimeOwnership {
        fn release_renderer(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("release_renderer");
            match self.renderer_release {
                ProbeRelease::Succeeds => Ok(()),
                ProbeRelease::Fails => Err(RunError::Recovery {
                    message: "injected renderer release failure".to_owned(),
                }),
                ProbeRelease::Panics => panic!("injected renderer release panic"),
            }
        }

        fn release_platform(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("release_platform");
            match self.platform_release {
                ProbeRelease::Succeeds => Ok(()),
                ProbeRelease::Fails => Err(RunError::Recovery {
                    message: "injected platform release failure".to_owned(),
                }),
                ProbeRelease::Panics => panic!("injected platform release panic"),
            }
        }

        fn teardown_after_backend_release(self) {
            let Self {
                events: _,
                renderer_release: _,
                platform_release: _,
                renderer,
                platform,
                context,
                window,
            } = self;
            drop(renderer);
            drop(platform);
            drop(context);
            drop(window);
        }
    }

    #[test]
    fn runtime_owner_drop_releases_renderer_before_context_and_window() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        )));

        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn explicit_runtime_owner_teardown_uses_the_same_order_once() {
        let events = Rc::new(RefCell::new(Vec::new()));
        OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        ))
        .teardown()
        .expect("explicit renderer release should succeed");

        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn runtime_shutdown_reports_application_failure_after_ordered_backend_release() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        ));

        let errors = finish_runtime_shutdown(
            None,
            || {
                events.borrow_mut().push("application_shutdown");
                Some(RunError::application_message(
                    ApplicationStage::Shutdown,
                    "injected application failure",
                ))
            },
            || owner.teardown(),
        );

        assert_eq!(
            *events.borrow(),
            [
                "application_shutdown",
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
        assert!(errors.terminal_error.is_none());
        assert_eq!(
            errors
                .shutdown_error
                .expect("application shutdown failure must remain reportable")
                .to_string(),
            "application callback failed during shutdown: injected application failure"
        );
    }

    #[test]
    fn runtime_shutdown_quarantines_context_after_platform_release_failure() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::with_platform_release(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
            ProbeRelease::Fails,
        ));

        let errors = finish_runtime_shutdown(
            None,
            || {
                events.borrow_mut().push("application_shutdown");
                None
            },
            || owner.teardown(),
        );

        assert_eq!(
            *events.borrow(),
            [
                "application_shutdown",
                "release_renderer",
                "release_platform"
            ]
        );
        assert!(errors.terminal_error.is_none());
        assert_eq!(
            errors
                .shutdown_error
                .expect("platform release failure must be reportable")
                .to_string(),
            "GPU generation recovery failed: injected platform release failure"
        );
    }

    #[test]
    fn application_shutdown_error_precedes_later_backend_release_error() {
        let errors = finish_runtime_shutdown(
            None,
            || {
                Some(RunError::application_message(
                    ApplicationStage::Shutdown,
                    "application shutdown failed first",
                ))
            },
            || {
                Err(RunError::Recovery {
                    message: "backend release failed second".to_owned(),
                })
            },
        );

        assert!(errors.terminal_error.is_none());
        assert!(matches!(
            errors.shutdown_error,
            Some(RunError::Application {
                stage: ApplicationStage::Shutdown,
                ..
            })
        ));
    }

    #[test]
    fn runtime_owner_uses_ordered_teardown_during_application_panic_unwind() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                let _owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
                    events,
                    ProbeRelease::Succeeds,
                ));
                panic!("injected application callback panic");
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_renderer_release_fails() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Fails,
        )));

        assert_eq!(*events.borrow(), ["release_renderer"]);
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_platform_release_fails() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(
            ProbeRuntimeOwnership::with_platform_release(
                Rc::clone(&events),
                ProbeRelease::Succeeds,
                ProbeRelease::Fails,
            ),
        ));

        assert_eq!(*events.borrow(), ["release_renderer", "release_platform"]);
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_renderer_release_panics() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
                    events,
                    ProbeRelease::Panics,
                )));
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(*events.borrow(), ["release_renderer"]);
    }

    #[test]
    fn config_rejects_multi_viewport_before_runtime_initialization() {
        let mut config = AppConfig::default();
        config.io_config_flags = Some(ConfigFlags::VIEWPORTS_ENABLE);

        assert!(matches!(
            validate_config(&config),
            Err(RunError::MultiViewportUnsupported)
        ));
    }

    #[test]
    fn live_context_rejects_multi_viewport_enabled_by_application_callbacks() {
        let _guard = super::super::imgui_test_guard();
        let mut context = dear_imgui_rs::Context::create();
        let mut flags = context.io().config_flags();
        flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_flags(flags);

        assert!(matches!(
            super::super::state::validate_supported_imgui_config(&context),
            Err(RunError::MultiViewportUnsupported)
        ));
    }

    #[test]
    fn application_frame_error_closes_the_active_frame() {
        let _guard = super::super::imgui_test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _consumer = context.create_synchronous_renderer_consumer().unwrap();

        {
            let result = build_frame(&mut context, |_ui| {
                Err(RunError::application_message(
                    ApplicationStage::Frame,
                    "injected frame failure",
                ))
            });
            assert!(result.is_err());
        }
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
    }

    #[test]
    fn delayed_device_loss_is_ignored_after_shutdown_or_event_loop_exit() {
        assert!(should_process_runtime_event(false, false));
        assert!(!should_process_runtime_event(true, false));
        assert!(!should_process_runtime_event(false, true));
        assert!(!should_process_runtime_event(true, true));
    }

    #[test]
    fn resumed_initializes_once_and_never_after_shutdown_starts() {
        let calls = Cell::new(0);
        let mut runtime = None;
        let initialize = || {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(())
        };

        assert_eq!(
            initialize_runtime_once(&mut runtime, false, initialize),
            Some(Ok(()))
        );
        assert_eq!(calls.get(), 1);
        assert!(runtime.is_some());

        assert_eq!(
            initialize_runtime_once(&mut runtime, false, initialize),
            None
        );
        assert_eq!(calls.get(), 1);

        runtime = None;
        assert_eq!(
            initialize_runtime_once(&mut runtime, true, initialize),
            None
        );
        assert_eq!(calls.get(), 1);
        assert!(runtime.is_none());
    }

    #[test]
    fn only_the_live_window_id_dispatches_an_event() {
        let live = WindowId::from(41_u64);
        let foreign = WindowId::from(42_u64);
        let calls = Cell::new(0);

        assert_eq!(
            dispatch_live_window_event(live, foreign, || {
                calls.set(calls.get() + 1);
                "foreign"
            }),
            None
        );
        assert_eq!(calls.get(), 0);

        assert_eq!(
            dispatch_live_window_event(live, live, || {
                calls.set(calls.get() + 1);
                "live"
            }),
            Some("live")
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn shutdown_coordinator_hands_off_the_first_runtime_error_exactly_once() {
        struct ProbeRuntime {
            teardown_calls: Rc<Cell<usize>>,
            terminal_error: Option<RunError>,
        }

        impl Drop for ProbeRuntime {
            fn drop(&mut self) {
                self.teardown_calls.set(self.teardown_calls.get() + 1);
            }
        }

        #[derive(Default)]
        struct ProbeApplication {
            shutdown_calls: usize,
        }

        let mut shutdown = ShutdownCoordinator::default();
        let teardown_calls = Rc::new(Cell::new(0));
        let mut runtime = Some(ProbeRuntime {
            teardown_calls: Rc::clone(&teardown_calls),
            terminal_error: Some(RunError::GpuValidation {
                message: "primary failure".to_owned(),
            }),
        });
        let mut application = ProbeApplication::default();
        for _ in 0..2 {
            shutdown.shutdown_once(
                &mut runtime,
                &mut application,
                |mut runtime, application| {
                    application.shutdown_calls += 1;
                    RuntimeShutdownErrors {
                        terminal_error: runtime.terminal_error.take(),
                        shutdown_error: Some(RunError::application_message(
                            ApplicationStage::Shutdown,
                            "secondary failure",
                        )),
                    }
                },
            );
        }

        assert!(shutdown.started());
        assert!(runtime.is_none());
        assert_eq!(application.shutdown_calls, 1);
        assert_eq!(teardown_calls.get(), 1);
        let error = shutdown
            .take_terminal_error()
            .expect("the runtime error must reach the runner owner");
        assert_eq!(
            error.to_string(),
            "WGPU reported an uncaptured validation error: primary failure"
        );
        assert!(shutdown.take_terminal_error().is_none());
        let shutdown_error = shutdown
            .take_shutdown_error()
            .expect("the shutdown error must remain separately observable");
        assert_eq!(
            shutdown_error.to_string(),
            "application callback failed during shutdown: secondary failure"
        );
        assert!(shutdown.take_shutdown_error().is_none());
    }

    #[test]
    fn shutdown_coordinator_does_not_replace_an_earlier_runner_error() {
        let mut shutdown = ShutdownCoordinator::default();
        shutdown.remember_error(RunError::GpuOutOfMemory {
            message: "primary failure".to_owned(),
        });
        let mut runtime = Some(());
        let mut shutdown_calls = 0;

        shutdown.shutdown_once(&mut runtime, &mut shutdown_calls, |_runtime, calls| {
            *calls += 1;
            RuntimeShutdownErrors {
                terminal_error: Some(RunError::GpuInternal {
                    message: "secondary failure".to_owned(),
                }),
                shutdown_error: Some(RunError::application_message(
                    ApplicationStage::Shutdown,
                    "shutdown failure",
                )),
            }
        });

        assert_eq!(shutdown_calls, 1);
        let error = shutdown
            .take_terminal_error()
            .expect("the first runner error must survive shutdown");
        assert_eq!(
            error.to_string(),
            "WGPU exhausted GPU memory: primary failure"
        );
        assert_eq!(
            shutdown
                .take_shutdown_error()
                .expect("the separate shutdown error must be retained")
                .to_string(),
            "application callback failed during shutdown: shutdown failure"
        );
    }

    #[test]
    fn run_result_resolution_covers_every_error_combination_in_observed_order() {
        for mask in 0_u8..8 {
            let has_terminal_before_shutdown = mask & 0b001 != 0;
            let event_loop_failed = mask & 0b010 != 0;
            let shutdown_failed = mask & 0b100 != 0;

            let terminal_before_shutdown =
                has_terminal_before_shutdown.then(|| RunError::GpuValidation {
                    message: "runtime failure".to_owned(),
                });
            let event_loop_result = if event_loop_failed {
                Err(EventLoopError::ExitFailure(73))
            } else {
                Ok(())
            };
            let shutdown_error = shutdown_failed.then(|| {
                RunError::application_message(ApplicationStage::Shutdown, "shutdown failure")
            });

            let result =
                resolve_run_result(terminal_before_shutdown, event_loop_result, shutdown_error);
            let actual = match result {
                Ok(()) => "ok",
                Err(RunError::GpuValidation { .. }) => "runtime",
                Err(RunError::Application { stage, .. }) => stage.as_str(),
                Err(RunError::EventLoop(EventLoopError::ExitFailure(73))) => "event-loop",
                Err(error) => panic!("unexpected run result for mask {mask:#05b}: {error}"),
            };
            let expected = if has_terminal_before_shutdown {
                "runtime"
            } else if event_loop_failed {
                "event-loop"
            } else if shutdown_failed {
                "shutdown"
            } else {
                "ok"
            };
            assert_eq!(actual, expected, "result mask: {mask:#05b}");
        }
    }
}
