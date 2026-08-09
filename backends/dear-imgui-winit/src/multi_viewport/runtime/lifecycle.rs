use super::*;

struct RuntimeConstruction<'a> {
    context: &'a mut Context,
    platform: Rc<WinitPlatformControl>,
    control: Rc<RuntimeControl>,
    #[cfg(test)]
    owned_platform: Option<crate::WinitPlatform>,
    prepared_monitors: Option<PreparedMonitors>,
    runtime_installed: bool,
    runtime_registered: bool,
    main_viewport_initialized: bool,
    callbacks_claimed: bool,
    committed: bool,
}

impl<'a> RuntimeConstruction<'a> {
    fn new(
        context: &'a mut Context,
        platform: Rc<WinitPlatformControl>,
        control: Rc<RuntimeControl>,
        #[cfg(test)] owned_platform: Option<crate::WinitPlatform>,
        prepared_monitors: PreparedMonitors,
    ) -> Self {
        Self {
            context,
            platform,
            control,
            #[cfg(test)]
            owned_platform,
            prepared_monitors: Some(prepared_monitors),
            runtime_installed: false,
            runtime_registered: false,
            main_viewport_initialized: false,
            callbacks_claimed: false,
            committed: false,
        }
    }

    fn commit(mut self) -> Result<WinitPlatformRuntime, WinitPlatformError> {
        self.control.state.set(RuntimeState::Attached);
        self.committed = true;
        Ok(WinitPlatformRuntime {
            control: Rc::clone(&self.control),
            platform: Rc::clone(&self.platform),
            #[cfg(test)]
            owned_platform: self.owned_platform.take(),
        })
    }
}

impl Drop for RuntimeConstruction<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        self.control.binding.with_bound_context(|| {
            let _ = self.control.restore_monitors_in_current_context();
            if self.callbacks_claimed {
                let _ = release_platform_callbacks(&self.control);
            }
            if self.main_viewport_initialized {
                self.control.drop_all_viewports();
            }
        });
        if self.runtime_registered {
            unregister_runtime(self.control.binding.id());
        }
        if self.runtime_installed {
            self.platform.clear_runtime(&self.control);
        }
        self.control.main_window.borrow_mut().take();
        self.control.state.set(RuntimeState::Detached);
    }
}

/// Owning Winit platform runtime for Dear ImGui multi-viewport support.
///
/// The runtime shares the `WinitPlatform`'s Context-bound main-window owner, and owns secondary
/// viewport windows plus the platform callback claim. It does not install a second platform
/// attachment; Context teardown reaches it through the platform owner's single attachment.
/// Calling [`Context::destroy_platform_windows`] directly also shuts this runtime down. The base
/// Winit platform remains attached for single-window use; create a new runtime before resuming
/// multi-viewport work. Prefer [`Self::shutdown`] when the caller needs backend-specific errors.
pub(crate) struct WinitPlatformRuntime {
    control: Rc<RuntimeControl>,
    platform: Rc<WinitPlatformControl>,
    #[cfg(test)]
    // Test construction has no native Window, so the runtime keeps its synthetic base platform
    // owner alive and tears it down after the multi-viewport contract.
    owned_platform: Option<crate::WinitPlatform>,
}

impl WinitPlatformRuntime {
    /// Attaches Winit multi-viewport support to the already attached platform main window.
    ///
    /// The platform must use [`crate::HiDpiMode::Default`]. Locked and rounded modes remap the
    /// single-window coordinate space, while Winit's native platform-window callbacks operate in
    /// platform-native desktop coordinates and therefore cannot be mixed without incorrect input
    /// and window geometry.
    pub(crate) fn new(
        context: &mut Context,
        platform: &crate::WinitPlatform,
    ) -> Result<Self, WinitPlatformError> {
        if !dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
            return Err(WinitPlatformError::AggregateCallbackHooksUnavailable);
        }
        validate_multi_viewport_hidpi_mode(platform.hidpi_mode())?;
        let platform_control = platform.control();
        platform_control.ensure_context(context)?;
        let main_window = platform_control.attached_window()?;
        platform_control.validate_operational_contract()?;

        preflight_window_system(&main_window)?;
        let prepared_monitors = prepare_monitors(context, &main_window)?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;

        let control = Rc::new(RuntimeControl::new(
            context,
            &platform_control,
            Arc::clone(&main_window),
        ));
        let runtime = Self::construct(
            context,
            platform_control,
            control,
            #[cfg(test)]
            None,
            Some(Arc::clone(&main_window)),
            prepared_monitors,
            |_, _| Ok(()),
        )?;
        let io = context.io_mut();
        io.set_display_size(super::super::desktop_size_for_window(&main_window));
        io.set_display_framebuffer_scale(super::super::framebuffer_scale_for_window(&main_window));
        invalidate_mouse_coordinate_cache(io);
        Ok(runtime)
    }

    fn from_platform(platform: &crate::WinitPlatform) -> Result<Self, WinitPlatformError> {
        let platform_control = platform.control();
        let control = platform_control.runtime_control()?;
        Ok(Self {
            control,
            platform: platform_control,
            #[cfg(test)]
            owned_platform: None,
        })
    }

    #[cfg(test)]
    pub(in super::super) fn new_for_test(
        context: &mut Context,
    ) -> Result<Self, WinitPlatformError> {
        Self::new_for_test_with(context, vec![test_monitor()], |_, _| Ok(()))
    }

    #[cfg(test)]
    pub(in super::super) fn new_for_test_with_platform(
        context: &mut Context,
        platform: &crate::WinitPlatform,
    ) -> Result<Self, WinitPlatformError> {
        let platform_control = platform.control();
        platform_control.ensure_context(context)?;
        platform_control.validate_operational_contract()?;
        let prepared_monitors =
            super::super::callbacks::prepare_monitors_for_test(context, vec![test_monitor()])?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;
        let control = Rc::new(RuntimeControl::new_for_test(context, &platform_control));
        Self::construct(
            context,
            platform_control,
            control,
            None,
            None,
            prepared_monitors,
            |_, _| Ok(()),
        )
    }

    #[cfg(test)]
    pub(in super::super) fn new_for_test_with(
        context: &mut Context,
        monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
        checkpoint: impl FnMut(ConstructionStage, &mut Context) -> Result<(), WinitPlatformError>,
    ) -> Result<Self, WinitPlatformError> {
        let platform = crate::WinitPlatform::new(context)?;
        let platform_control = platform.control();
        let prepared_monitors =
            super::super::callbacks::prepare_monitors_for_test(context, monitors)?;
        preflight_platform_callbacks(context)?;
        preflight_main_viewport(context)?;
        let control = Rc::new(RuntimeControl::new_for_test(context, &platform_control));
        Self::construct(
            context,
            platform_control,
            control,
            Some(platform),
            None,
            prepared_monitors,
            checkpoint,
        )
    }

    fn construct(
        context: &mut Context,
        platform: Rc<WinitPlatformControl>,
        control: Rc<RuntimeControl>,
        #[cfg(test)] owned_platform: Option<crate::WinitPlatform>,
        main_window: Option<Arc<Window>>,
        prepared_monitors: PreparedMonitors,
        mut checkpoint: impl FnMut(ConstructionStage, &mut Context) -> Result<(), WinitPlatformError>,
    ) -> Result<Self, WinitPlatformError> {
        let mut transaction = RuntimeConstruction::new(
            context,
            platform,
            control,
            #[cfg(test)]
            owned_platform,
            prepared_monitors,
        );

        transaction
            .platform
            .install_runtime(Rc::clone(&transaction.control))?;
        transaction.runtime_installed = true;
        checkpoint(ConstructionStage::Attachment, transaction.context)?;

        register_runtime(&transaction.control);
        transaction.runtime_registered = true;
        checkpoint(ConstructionStage::Registry, transaction.context)?;

        if let Some(main_window) = main_window {
            init_main_viewport(&transaction.control, main_window)?;
            transaction.main_viewport_initialized = true;
        }
        checkpoint(ConstructionStage::MainViewport, transaction.context)?;

        let callback_contract = claim_platform_callbacks(transaction.context);
        transaction
            .control
            .install_platform_callback_contract(callback_contract);
        transaction.callbacks_claimed = true;
        checkpoint(ConstructionStage::Callbacks, transaction.context)?;

        let prepared_monitors = transaction
            .prepared_monitors
            .take()
            .expect("monitor storage is present until publication");
        let ownership = publish_monitors(transaction.context, prepared_monitors);
        transaction.control.install_monitor_ownership(ownership);
        checkpoint(ConstructionStage::Monitors, transaction.context)?;

        claim_backend_flags(&transaction.control, transaction.context);
        checkpoint(ConstructionStage::BackendFlags, transaction.context)?;

        transaction.commit()
    }

    /// Validates that this runtime still owns the active Winit viewport platform contract.
    ///
    /// Renderer backends use this before interpreting `PlatformHandle` values as Winit windows.
    /// A runtime that has shut down cannot validate even if another platform later attaches to
    /// the same Context.
    pub fn validate_renderer_owner(&self, context: &Context) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()
    }

    #[cfg(test)]
    pub(in super::super) fn owned_platform_for_test_mut(&mut self) -> &mut crate::WinitPlatform {
        self.owned_platform
            .as_mut()
            .expect("test runtimes always retain their synthetic platform owner")
    }

    /// Runs `callback` while viewport callbacks may access `event_loop`.
    ///
    /// Nested scopes restore the outer event loop. The returned attempt keeps the callback output
    /// separate from every platform fault observed after Rust regains control, so no unwind
    /// crosses the native callback boundary and a callback `Result` is never hidden by a later
    /// platform failure.
    pub fn with_event_loop<R>(
        &self,
        event_loop: &ActiveEventLoop,
        callback: impl for<'scope> FnOnce(EventLoopScope<'scope>) -> R,
    ) -> WinitViewportAttempt<R> {
        let faults = self.control.drain_faults();
        if !faults.is_empty() {
            return WinitViewportAttempt::skipped(faults);
        }
        if let Err(error) = self.ensure_attached() {
            return WinitViewportAttempt::skipped(vec![error]);
        }

        let output = self.control.enter_event_loop(event_loop, callback);
        WinitViewportAttempt::completed(output, self.control.drain_faults())
    }

    /// Returns and clears the oldest retryable callback fault.
    ///
    /// Contract drift and callback panics are terminal and remain observable until shutdown.
    pub fn poll_fault(&self) -> Result<(), WinitPlatformError> {
        self.control.poll_fault()
    }

    #[cfg(test)]
    pub(in super::super) fn route_secondary_event<T>(
        &self,
        context: &mut Context,
        event: &Event<T>,
    ) -> Result<bool, WinitPlatformError> {
        self.ensure_context(context)?;
        self.poll_fault()?;
        self.ensure_attached()?;
        let consumed = super::super::events::route_secondary_event(&self.control, context, event)?;
        self.poll_fault()?;
        Ok(consumed)
    }

    /// Explicitly releases platform callbacks and windows.
    ///
    /// The operation is idempotent. The explicit Context lets the core close an open frame before
    /// any platform callback or native window state is released. Dropping the runtime without a
    /// Context defers native cleanup to the Context attachment instead. An active renderer
    /// attachment rejects shutdown before the frame or native state changes.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        if matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ContextDestroyed
        ) {
            return Ok(());
        }
        let attachment = self.platform.attachment_handle()?;
        let (result, released) = {
            let mut release = context.prepare_platform_attachment_release(&attachment)?;
            let context = release.context_mut();
            let result = self.control.shutdown_from_platform(context);
            let released = matches!(
                self.control.state(),
                RuntimeState::Detached | RuntimeState::ContextDestroyed
            );
            (result, released)
        };
        if released {
            self.platform.clear_runtime(&self.control);
        }
        #[cfg(test)]
        let result = if released {
            if let Some(platform) = self.owned_platform.as_mut() {
                let platform_result = platform.shutdown(context);
                match (result, platform_result) {
                    (Err(primary), Err(secondary)) => {
                        self.control.record_fault(secondary);
                        Err(primary)
                    }
                    (Ok(()), platform_result) => platform_result,
                    (result, Ok(())) => result,
                }
            } else {
                result
            }
        } else {
            result
        };
        result
    }

    fn ensure_context(&self, context: &Context) -> Result<(), WinitPlatformError> {
        if context.id() == self.control.binding.id() {
            Ok(())
        } else {
            Err(WinitPlatformError::ContextMismatch)
        }
    }

    fn ensure_attached(&self) -> Result<(), WinitPlatformError> {
        if self.control.state() != RuntimeState::Attached {
            return self
                .control
                .poll_fault()
                .and(Err(WinitPlatformError::RuntimeDetached));
        }
        self.control
            .platform_control()?
            .validate_operational_contract()
    }

    #[cfg(test)]
    pub(in super::super) fn control(&self) -> &Rc<RuntimeControl> {
        &self.control
    }
}

impl WinitViewportRendererAdapter {
    /// Returns the Context identity of this exact platform generation.
    #[must_use]
    pub fn context_id(&self) -> dear_imgui_rs::ContextId {
        self.control.binding().id()
    }

    /// Runs one renderer route attempt with the active event-loop capability.
    pub fn with_event_loop<R>(
        &self,
        event_loop: &ActiveEventLoop,
        callback: impl for<'scope> FnOnce(EventLoopScope<'scope>) -> R,
    ) -> WinitViewportAttempt<R> {
        let faults = self.control.drain_faults();
        if !faults.is_empty() {
            return WinitViewportAttempt::skipped(faults);
        }

        let runtime = WinitPlatformRuntime {
            control: Rc::clone(&self.control),
            platform: Rc::clone(&self.platform),
            #[cfg(test)]
            owned_platform: None,
        };
        if let Err(error) = runtime.ensure_attached() {
            return WinitViewportAttempt::skipped(vec![error]);
        }

        let output = self.control.enter_event_loop(event_loop, callback);
        WinitViewportAttempt::completed(output, self.control.drain_faults())
    }
}

impl crate::WinitPlatform {
    /// Enable native multi-viewport ownership on this attached platform.
    ///
    /// The main window must already be attached with [`crate::HiDpiMode::Default`]. The platform
    /// remains the sole public owner for main-window input, secondary windows, callback faults,
    /// and event-loop scopes.
    pub fn enable_viewports(&mut self, context: &mut Context) -> Result<(), WinitPlatformError> {
        WinitPlatformRuntime::new(context, self).map(drop)
    }

    /// Returns whether this platform currently owns native multi-viewport state.
    #[must_use]
    pub fn viewports_enabled(&self) -> bool {
        self.control().has_live_runtime()
    }

    /// Captures the exact platform generation used by a first-party renderer route.
    #[doc(hidden)]
    pub fn viewport_renderer_adapter(
        &self,
        context: &Context,
    ) -> Result<WinitViewportRendererAdapter, WinitPlatformError> {
        let runtime = WinitPlatformRuntime::from_platform(self)?;
        runtime.validate_renderer_owner(context)?;
        Ok(WinitViewportRendererAdapter {
            control: Rc::clone(&runtime.control),
            platform: Rc::clone(&runtime.platform),
        })
    }

    /// Runs `callback` while native viewport callbacks may access `event_loop`.
    ///
    /// The [`super::EventLoopScope`] cannot escape this closure. Nested scopes restore the outer
    /// event loop, and the returned [`WinitViewportAttempt`] retains both the callback output and
    /// every deferred fault after Rust regains control.
    ///
    /// ```compile_fail
    /// use dear_imgui_winit::WinitPlatform;
    /// use winit::event_loop::ActiveEventLoop;
    ///
    /// fn leak_event_loop<'a>(
    ///     platform: &WinitPlatform,
    ///     event_loop: &'a ActiveEventLoop,
    /// ) -> &'a ActiveEventLoop {
    ///     platform
    ///         .with_event_loop(event_loop, |scope| scope.active_event_loop())
    ///         .into_parts()
    ///         .0
    ///         .unwrap()
    /// }
    /// ```
    pub fn with_event_loop<R>(
        &self,
        event_loop: &ActiveEventLoop,
        callback: impl for<'scope> FnOnce(EventLoopScope<'scope>) -> R,
    ) -> WinitViewportAttempt<R> {
        match WinitPlatformRuntime::from_platform(self) {
            Ok(runtime) => runtime.with_event_loop(event_loop, callback),
            Err(error) => WinitViewportAttempt::skipped(vec![error]),
        }
    }

    /// Returns all deferred multi-viewport faults in observation order.
    ///
    /// First-party renderer routes already aggregate these failures into their frame result. This
    /// method is the advanced escape hatch for custom renderer routes, which must drain faults
    /// after every native platform-window pass. Retryable faults are removed; a terminal contract
    /// fault remains observable until shutdown.
    pub fn drain_viewport_faults(&self) -> Result<Vec<WinitPlatformError>, WinitPlatformError> {
        Ok(WinitPlatformRuntime::from_platform(self)?
            .control
            .drain_faults())
    }

    /// Disable native multi-viewport ownership while keeping the main platform attached.
    ///
    /// Any attached renderer route must be shut down first. The operation is retryable when a
    /// deferred callback or ownership fault is returned.
    pub fn disable_viewports(&mut self, context: &mut Context) -> Result<(), WinitPlatformError> {
        match WinitPlatformRuntime::from_platform(self) {
            Ok(mut runtime) => runtime.shutdown(context),
            Err(WinitPlatformError::RuntimeDetached) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn validate_window_system(
    is_supported_desktop: bool,
    is_wayland: bool,
) -> Result<(), WinitPlatformError> {
    if !is_supported_desktop {
        return Err(WinitPlatformError::UnsupportedWindowSystem {
            target: std::env::consts::OS,
        });
    }
    if is_wayland {
        Err(WinitPlatformError::WaylandUnsupported)
    } else {
        Ok(())
    }
}

pub(in super::super) fn validate_multi_viewport_hidpi_mode(
    mode: crate::HiDpiMode,
) -> Result<(), WinitPlatformError> {
    if mode == crate::HiDpiMode::Default {
        Ok(())
    } else {
        Err(WinitPlatformError::CustomHiDpiModeUnsupported)
    }
}

#[cfg(target_os = "linux")]
fn preflight_window_system(window: &Window) -> Result<(), WinitPlatformError> {
    let handle = window
        .window_handle()
        .map_err(|error| WinitPlatformError::WindowOperation {
            operation: "query raw window handle",
            message: error.to_string(),
        })?
        .as_raw();
    validate_window_system(true, matches!(handle, RawWindowHandle::Wayland(_)))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn preflight_window_system(_window: &Window) -> Result<(), WinitPlatformError> {
    validate_window_system(true, false)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn preflight_window_system(_window: &Window) -> Result<(), WinitPlatformError> {
    validate_window_system(false, false)
}

#[cfg(test)]
pub(in super::super) fn validate_window_system_for_test(
    is_supported_desktop: bool,
    is_wayland: bool,
) -> Result<(), WinitPlatformError> {
    validate_window_system(is_supported_desktop, is_wayland)
}

fn claim_backend_flags(control: &RuntimeControl, context: &mut Context) {
    control.binding().with_bound_context(|| {
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | crate::platform::WINIT_VIEWPORT_FLAGS);
    });
}

#[cfg(test)]
fn test_monitor() -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
    dear_imgui_rs::sys::ImGuiPlatformMonitor {
        MainPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
        MainSize: dear_imgui_rs::sys::ImVec2 {
            x: 1920.0,
            y: 1080.0,
        },
        WorkPos: dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 },
        WorkSize: dear_imgui_rs::sys::ImVec2 {
            x: 1920.0,
            y: 1040.0,
        },
        DpiScale: 1.0,
        PlatformHandle: std::ptr::null_mut(),
    }
}
