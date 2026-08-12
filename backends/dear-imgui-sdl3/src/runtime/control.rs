use super::*;

mod attachments;
mod registration;
#[cfg(test)]
mod tests;

use attachments::{PlatformAttachment, RendererAttachment};
use registration::unregister_runtime;
pub(crate) use registration::{RuntimeRegistration, register_runtime, with_current_runtime};

impl RuntimeControl {
    fn new_with_backend(
        context: &Context,
        lifecycle: NativeLifecycle,
        platform_session: Option<Sdl3PlatformSession>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> Self {
        Self {
            binding: context.binding(),
            state: Cell::new(RuntimeState::Attached),
            platform_initialized: Cell::new(false),
            renderer_initialized: Cell::new(false),
            renderer_release: Cell::new(if lifecycle.renderer_shutdown.is_none() {
                ReleaseState::Released
            } else {
                ReleaseState::Pending
            }),
            platform_release: Cell::new(ReleaseState::Pending),
            callback_teardown_active: Cell::new(false),
            platform_io_key: Cell::new(0),
            platform_graphics,
            #[cfg(feature = "multi-viewport")]
            vulkan_surface_provider: Arc::new(VulkanSurfaceProviderState::default()),
            gl_viewport_swap_interval: Cell::new(Sdl3OpenGlViewportSwapInterval::Immediate),
            native_renderer,
            lifecycle,
            callbacks: RefCell::new(None),
            renderer_callbacks: RefCell::new(None),
            renderer_shutdown_restore: RefCell::new(None),
            renderer_consumer: RefCell::new(None),
            platform_session: RefCell::new(platform_session),
            #[cfg(any(
                feature = "opengl3-renderer",
                feature = "sdlrenderer3-renderer",
                feature = "sdlgpu3-renderer"
            ))]
            renderer_textures: RefCell::new(RendererTextureStore::default()),
            owned_viewports: RefCell::new(HashMap::new()),
            owned_renderer_viewports: RefCell::new(HashMap::new()),
            deferred_platform_viewports: RefCell::new(HashMap::new()),
            deferred_renderer_viewports: RefCell::new(HashMap::new()),
            failed_viewports: RefCell::new(HashMap::new()),
            dispatch_depth: Cell::new(0),
            dispatch_failures: RefCell::new(Vec::new()),
            faults: RefCell::new(VecDeque::new()),
            reported_replacements: RefCell::new(HashSet::new()),
            foreign_platform_user_data_reported: Cell::new(false),
            revoked_capabilities: Cell::new(0),
            foreign_capabilities: Cell::new(0),
            #[cfg(test)]
            phase_log: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(crate) fn install_renderer_consumer(&self, consumer: Rc<SynchronousRendererConsumer>) {
        let previous = self.renderer_consumer.borrow_mut().replace(consumer);
        assert!(
            previous.is_none(),
            "SDL3 runtime already owns a renderer consumer"
        );
    }

    fn take_renderer_consumer(&self) -> Option<Rc<SynchronousRendererConsumer>> {
        self.renderer_consumer.borrow_mut().take()
    }

    pub(crate) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    pub(crate) fn expects_opengl(&self) -> bool {
        self.platform_graphics == PlatformGraphicsKind::OpenGl
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn expects_vulkan(&self) -> bool {
        self.platform_graphics == PlatformGraphicsKind::Vulkan
    }

    #[cfg(feature = "multi-viewport")]
    fn acquire_vulkan_surface_provider(
        &self,
        context: &Context,
    ) -> Result<Sdl3VulkanSurfaceProvider, Sdl3BackendError> {
        let entry = self.enter(context)?;
        if !self.expects_vulkan() {
            return Err(Sdl3BackendError::VulkanSurfaceProviderRequiresVulkan);
        }
        let callback_available = self.binding.try_with_bound_context(|| {
            self.validate_platform_ownership_bound()
                && unsafe {
                    let platform_io = sys::igGetPlatformIO_Nil();
                    !platform_io.is_null() && (*platform_io).Platform_CreateVkSurface.is_some()
                }
        })?;
        if !callback_available {
            entry.finish()?;
            return Err(Sdl3BackendError::VulkanSurfaceCallbackUnavailable);
        }
        self.vulkan_surface_provider
            .leased
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Sdl3BackendError::VulkanSurfaceProviderAlreadyLeased)?;
        let provider = Sdl3VulkanSurfaceProvider {
            state: Arc::clone(&self.vulkan_surface_provider),
        };
        if let Err(error) = entry.finish() {
            drop(provider);
            return Err(error);
        }
        Ok(provider)
    }

    #[cfg(feature = "multi-viewport")]
    fn ensure_vulkan_surface_provider_released(&self) -> Result<(), Sdl3BackendError> {
        if self.vulkan_surface_provider.leased.load(Ordering::Acquire) {
            Err(Sdl3BackendError::VulkanSurfaceProviderActive)
        } else {
            Ok(())
        }
    }

    pub(crate) fn native_renderer(&self) -> NativeRendererKind {
        self.native_renderer
    }

    pub(crate) fn native_gl_swap_interval(&self) -> (u32, i32) {
        self.gl_viewport_swap_interval.get().native_policy()
    }

    pub(crate) fn set_gl_viewport_swap_interval(&self, policy: Sdl3OpenGlViewportSwapInterval) {
        debug_assert!(self.expects_opengl());
        self.gl_viewport_swap_interval.set(policy);
    }

    fn begin_shutdown(&self) {
        if self.state.get() == RuntimeState::Attached {
            self.state.set(RuntimeState::ShuttingDown);
        }
    }

    fn finish_shutdown(&self) {
        if self.renderer_released()
            && self.platform_released()
            && self.state.get() != RuntimeState::ResourceDropped
        {
            self.state.set(RuntimeState::Detached);
        }
    }

    pub(super) fn renderer_released(&self) -> bool {
        self.renderer_release.get().is_released()
    }

    pub(super) fn platform_released(&self) -> bool {
        self.platform_release.get().is_released()
    }

    fn release_platform_session(&self) {
        self.platform_session.borrow_mut().take();
    }

    fn platform_session_generation(&self) -> Option<u64> {
        self.platform_session
            .borrow()
            .as_ref()
            .map(Sdl3PlatformSession::generation)
    }

    fn resolve_live_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut sys::ImGuiViewport> {
        if viewport.is_null() || self.platform_session_generation().is_none() {
            return None;
        }
        let context = unsafe { sys::igGetCurrentContext() };
        if context.is_null() {
            return None;
        }
        let address = viewport as usize;
        let live = unsafe { sys::ImGuiContext_FindLiveViewportByAddress(context, address) };
        (live == viewport && !live.is_null()).then_some(live)
    }

    fn viewport_key(&self, viewport: *mut sys::ImGuiViewport) -> Option<ViewportLeaseKey> {
        let generation = self.platform_session_generation()?;
        let live = self.resolve_live_viewport(viewport)?;
        Some(ViewportLeaseKey {
            context: self.binding.id(),
            generation,
            address: live as usize,
            id: unsafe { (*live).ID },
        })
    }

    pub(crate) fn begin_platform_dispatch(&self) -> PlatformDispatchGuard<'_> {
        // The depth is diagnostic state only; native callback entry must never panic because a
        // re-entrant application exceeded the counter's representable range. The scope stack is
        // the authoritative failure state and remains intact even when the counter saturates.
        let depth = self.dispatch_depth.get().saturating_add(1);
        self.dispatch_depth.set(depth);
        self.dispatch_failures.borrow_mut().push(HashMap::new());
        PlatformDispatchGuard {
            control: self,
            active: true,
        }
    }

    pub(crate) fn live_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut sys::ImGuiViewport> {
        self.resolve_live_viewport(viewport).or_else(|| {
            #[cfg(test)]
            {
                self.synthetic_viewport_key(viewport).map(|_| viewport)
            }
            #[cfg(not(test))]
            {
                None
            }
        })
    }

    pub(crate) unsafe fn capture_viewport_platform_state(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportPlatformState> {
        self.live_viewport(viewport)
            .map(|live| unsafe { ViewportPlatformState::capture(live) })
    }

    pub(crate) unsafe fn viewport_renderer_user_data(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut std::ffi::c_void> {
        self.live_viewport(viewport)
            .map(|live| unsafe { (*live).RendererUserData })
    }

    pub(crate) unsafe fn set_viewport_renderer_user_data(
        &self,
        viewport: *mut sys::ImGuiViewport,
        user_data: *mut std::ffi::c_void,
    ) -> bool {
        let Some(live) = self.live_viewport(viewport) else {
            return false;
        };
        unsafe { (*live).RendererUserData = user_data };
        true
    }

    pub(crate) unsafe fn clear_viewport_platform_state(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> bool {
        let Some(live) = self.live_viewport(viewport) else {
            return false;
        };
        unsafe { ViewportPlatformState::clear(live) };
        true
    }

    pub(crate) unsafe fn restore_viewport_platform_state(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) -> bool {
        let Some(live) = self.live_viewport(viewport) else {
            return false;
        };
        unsafe { state.restore(live) };
        true
    }

    fn request_viewport_close(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        let Some(live) = self.live_viewport(viewport) else {
            return false;
        };
        unsafe {
            (*live).PlatformRequestClose = true;
            (*live).DrawData = std::ptr::null_mut();
        }
        true
    }

    fn release_renderer_device_objects_bound(&self) -> Result<(), Sdl3BackendError> {
        if !self.renderer_initialized.get() {
            return Ok(());
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.destroy_uninstalled_renderer_textures_bound()?;
        if let Some(destroy) = &self.lifecycle.renderer_device_objects_destroy {
            destroy();
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.forget_textures_destroyed_by_upstream();
        Ok(())
    }

    fn release_renderer_bound(&self) -> Result<bool, Sdl3BackendError> {
        if self.renderer_released() {
            return Ok(true);
        }
        let Some(release) = ReleaseGuard::begin(&self.renderer_release) else {
            return Ok(false);
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("renderer");
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        if self.renderer_initialized.get() {
            self.destroy_uninstalled_renderer_textures_bound()?;
        }
        let restore = if let Some(restore) = self.renderer_shutdown_restore.borrow_mut().take() {
            let prepare_result = self
                .renderer_callbacks
                .borrow()
                .as_ref()
                .map(|callbacks| unsafe { callbacks.switch_from_platform_to_native_shutdown() })
                .transpose();
            if let Err(error) = prepare_result {
                self.renderer_shutdown_restore.borrow_mut().replace(restore);
                return Err(error);
            }
            Some(restore)
        } else {
            self.renderer_callbacks
                .borrow()
                .as_ref()
                .map(|callbacks| unsafe { callbacks.prepare_native_shutdown(self) })
                .transpose()?
        };
        let shutdown_result = if self.renderer_initialized.get()
            && let Some(shutdown) = &self.lifecycle.renderer_shutdown
        {
            catch_unwind(AssertUnwindSafe(|| shutdown()))
        } else {
            Ok(())
        };
        if let Some(restore) = restore {
            let restore_result = unsafe {
                self.renderer_callbacks
                    .borrow()
                    .as_ref()
                    .expect("initialized SDL3 renderer lost its callback claim")
                    .restore_after_shutdown(restore)
            };
            if restore_result.is_err() {
                self.record_platform_state_replaced("renderer callback shutdown state");
            }
        }
        if let Err(payload) = shutdown_result {
            resume_unwind(payload);
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.renderer_textures
            .borrow_mut()
            .forget_destroyed_by_upstream();
        release.commit();
        self.renderer_initialized.set(false);
        self.renderer_callbacks.borrow_mut().take();
        self.owned_renderer_viewports.borrow_mut().clear();
        self.finish_shutdown();
        Ok(true)
    }

    fn release_platform_bound(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        #[cfg(feature = "multi-viewport")]
        self.ensure_vulkan_surface_provider_released()?;
        let Some(release) = ReleaseGuard::begin(&self.platform_release) else {
            return Ok(());
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("platform");

        if !self.platform_initialized.get() {
            release.commit();
            self.release_platform_session();
            self.finish_shutdown();
            return Ok(());
        }

        let renderer_restore = self
            .renderer_callbacks
            .borrow()
            .as_ref()
            .map(|callbacks| unsafe { callbacks.prepare_platform_shutdown(self) })
            .transpose()?;
        let restore = {
            let callbacks = self.callbacks.borrow();
            match callbacks
                .as_ref()
                .map(|callbacks| unsafe { callbacks.prepare_shutdown(self) })
                .transpose()
            {
                Ok(restore) => restore,
                Err(error) => {
                    if let Some(renderer_restore) = renderer_restore {
                        let _ = self
                            .renderer_callbacks
                            .borrow()
                            .as_ref()
                            .map(|callbacks| unsafe {
                                callbacks.restore_after_shutdown(renderer_restore)
                            });
                    }
                    return Err(error);
                }
            }
        };
        self.callback_teardown_active.set(true);
        struct CallbackTeardownGuard<'a>(&'a Cell<bool>);
        impl Drop for CallbackTeardownGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let callback_guard = CallbackTeardownGuard(&self.callback_teardown_active);
        let shutdown_result = catch_unwind(AssertUnwindSafe(|| {
            (self.lifecycle.platform_shutdown)();
        }));
        drop(callback_guard);
        self.restore_deferred_viewport_state();

        if let Err(payload) = shutdown_result {
            if let Some(restore) = restore {
                let callbacks = self.callbacks.borrow();
                let _ = unsafe {
                    callbacks
                        .as_ref()
                        .expect("initialized SDL3 runtime lost its callback claim")
                        .restore_after_shutdown(restore)
                };
            }
            if let Some(renderer_restore) = renderer_restore {
                let _ = self
                    .renderer_callbacks
                    .borrow()
                    .as_ref()
                    .map(|callbacks| unsafe { callbacks.restore_after_shutdown(renderer_restore) });
            }
            resume_unwind(payload);
        }

        // Native shutdown is the irreversible boundary: never call it twice,
        // even if restoring foreign callback state reports an error.
        release.commit();
        self.platform_initialized.set(false);
        let restore_result = if let Some(restore) = restore {
            let callbacks = self.callbacks.borrow();
            unsafe {
                callbacks
                    .as_ref()
                    .expect("initialized SDL3 runtime lost its callback claim")
                    .restore_after_shutdown(restore)
            }
        } else {
            Ok(())
        };
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.owned_viewports.borrow_mut().clear();
        self.failed_viewports.borrow_mut().clear();
        if let Some(renderer_restore) = renderer_restore {
            self.renderer_shutdown_restore
                .borrow_mut()
                .replace(renderer_restore);
        }
        self.release_platform_session();
        self.finish_shutdown();
        restore_result
    }

    fn release_renderer_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.renderer_released() {
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound()))
        })?;
        match result {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => Err(Sdl3BackendError::ShutdownInProgress {
                phase: "renderer resources",
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources",
            }),
        }
    }

    fn release_platform_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_platform_bound()))
        })?;
        result.unwrap_or(Err(Sdl3BackendError::ShutdownPanicked {
            phase: "platform windows",
        }))
    }

    fn shutdown_native_explicit(&self) -> Result<(), Sdl3BackendError> {
        #[cfg(feature = "multi-viewport")]
        self.ensure_vulkan_surface_provider_released()?;
        self.begin_shutdown();
        let platform_result = self.release_platform_explicit();
        let platform_retry_result = if self.platform_released() {
            Ok(())
        } else {
            self.release_platform_explicit()
        };
        let renderer_result = if self.platform_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        let renderer_retry_result = if self.platform_released() && !self.renderer_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        first_error([
            platform_result.err(),
            platform_retry_result.err(),
            renderer_result.err(),
            renderer_retry_result.err(),
        ])
    }

    fn shutdown_bound_for_attachment(&self) -> Result<(), Sdl3BackendError> {
        let platform_result = match catch_unwind(AssertUnwindSafe(|| self.release_platform_bound()))
        {
            Ok(result) => result,
            Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                phase: "platform windows",
            }),
        };
        let renderer_result = if self.platform_released() {
            match catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound())) {
                Ok(Ok(true)) => Ok(()),
                Ok(Ok(false)) => Err(Sdl3BackendError::ShutdownInProgress {
                    phase: "renderer resources",
                }),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                    phase: "renderer resources",
                }),
            }
        } else {
            Ok(())
        };
        first_error([platform_result.err(), renderer_result.err()])
    }

    fn detect_callback_replacements(&self) {
        if self.state.get() != RuntimeState::Attached || !self.platform_initialized.get() {
            return;
        }
        let _ = self.binding.try_with_bound_context(|| {
            if let Some(callbacks) = self.callbacks.borrow().as_ref() {
                unsafe { callbacks.detect_replacements(self) };
            }
            if let Some(callbacks) = self.renderer_callbacks.borrow().as_ref() {
                unsafe { callbacks.detect_replacements(self) };
            }
        });
    }

    pub(crate) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.detect_callback_replacements();
        match self.faults.borrow_mut().pop_front() {
            Some(fault) => Err(fault.into_error()),
            None => Ok(()),
        }
    }

    pub(crate) fn drain_faults(&self) -> Vec<Sdl3BackendError> {
        self.detect_callback_replacements();
        self.faults
            .borrow_mut()
            .drain(..)
            .map(RuntimeFault::into_error)
            .collect()
    }

    fn take_pending_fault(&self) -> Option<Sdl3BackendError> {
        self.detect_callback_replacements();
        self.faults
            .borrow_mut()
            .pop_front()
            .map(RuntimeFault::into_error)
    }

    pub(crate) fn ensure_entry(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        self.ensure_context(context)?;
        self.ensure_bound_entry()
    }

    pub(crate) fn enter(&self, context: &Context) -> Result<RuntimeEntry<'_>, Sdl3BackendError> {
        self.ensure_entry(context)?;
        Ok(RuntimeEntry {
            control: self,
            finished: false,
        })
    }

    pub(crate) fn ensure_bound_entry(&self) -> Result<(), Sdl3BackendError> {
        self.request_failed_viewport_closes();
        self.poll_fault()?;
        if self.state.get() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        Ok(())
    }

    #[cfg(any(
        feature = "multi-viewport",
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn enter_bound(&self) -> Result<RuntimeEntry<'_>, Sdl3BackendError> {
        self.ensure_bound_entry()?;
        Ok(RuntimeEntry {
            control: self,
            finished: false,
        })
    }

    pub(crate) fn finish_entry(&self) -> Result<(), Sdl3BackendError> {
        self.poll_fault()
    }

    pub(super) fn inspect_abandoned_entry(&self) {
        self.detect_callback_replacements();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn process_texture_requests(
        &self,
        requests: &[TextureRequest],
        request_epoch: u64,
    ) -> Result<ProcessedTextureRequests, Sdl3BackendError> {
        let update_texture = self
            .lifecycle
            .renderer_texture_update
            .as_ref()
            .expect("initialized SDL3 renderer has no texture updater");
        self.renderer_textures
            .borrow_mut()
            .process_requests(requests, request_epoch, |texture| update_texture(texture))
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn mark_textures_reconciled(&self, installed: &[SnapshotTextureId]) {
        self.renderer_textures
            .borrow_mut()
            .mark_reconciled(installed);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn prune_destroyed_textures(&self, completion_watermark: u64) {
        self.renderer_textures
            .borrow_mut()
            .prune_destroyed(completion_watermark);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn clear_destroyed_textures(&self) {
        self.renderer_textures.borrow_mut().clear_destroyed();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn forget_textures_destroyed_by_upstream(&self) {
        self.renderer_textures
            .borrow_mut()
            .forget_destroyed_by_upstream();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn destroy_uninstalled_renderer_textures_bound(
        &self,
    ) -> Result<(), Sdl3BackendError> {
        let Some(update_texture) = self.lifecycle.renderer_texture_update.as_ref() else {
            return Ok(());
        };
        self.renderer_textures
            .borrow_mut()
            .destroy_uninstalled(|texture| update_texture(texture))
    }

    pub(crate) fn ensure_context(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        let expected = self.binding.id();
        let actual = context.id();
        if expected != actual {
            return Err(Sdl3BackendError::ContextMismatch { expected, actual });
        }
        Ok(())
    }

    pub(crate) fn original_platform_callbacks(&self) -> Option<PlatformCallbacks> {
        self.callbacks
            .borrow()
            .as_ref()
            .map(PlatformCallbackOwnership::original_callbacks)
    }

    pub(crate) fn validate_platform_callback_slot(&self, slot: PlatformCallbackSlot) -> bool {
        let owns_slot = {
            let callbacks = self.callbacks.borrow();
            let Some(callbacks) = callbacks.as_ref() else {
                self.record_platform_state_replaced("platform callback ownership");
                return false;
            };
            unsafe { callbacks.owns_live_slot(slot) }
        };
        if owns_slot {
            return true;
        }

        // Re-run the complete publication check before revoking capabilities.
        // A foreign backend may have replaced every SDL-owned slot in one
        // transaction; in that case its capability bits remain valid even
        // though this trampoline must no longer call through the slot.
        let _ = self.validate_platform_ownership_bound();
        false
    }

    pub(crate) fn original_renderer_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_create_window)
    }

    pub(crate) fn original_renderer_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_destroy_window)
    }

    pub(crate) fn original_renderer_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_render_window)
    }

    pub(crate) fn invoke_original_renderer_set_window_size(
        &self,
        viewport: *mut sys::ImGuiViewport,
        size: *const sys::ImVec2,
    ) {
        let invocation = self
            .renderer_callbacks
            .borrow()
            .as_ref()
            .map(RendererCallbackOwnership::original_set_window_size_invocation);
        if let Some(invocation) = invocation {
            invocation.invoke(viewport, size);
        }
    }

    pub(crate) fn original_renderer_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_swap_buffers)
    }

    pub(crate) fn validate_renderer_ownership_bound(&self) -> bool {
        if self.native_renderer == NativeRendererKind::None {
            return true;
        }
        let callbacks = self.renderer_callbacks.borrow();
        let Some(callbacks) = callbacks.as_ref() else {
            self.record_renderer_state_replaced("renderer callback ownership");
            return false;
        };
        unsafe { callbacks.detect_replacements(self) }
    }

    pub(crate) fn validate_platform_ownership_bound(&self) -> bool {
        let callbacks = self.callbacks.borrow();
        let Some(callbacks) = callbacks.as_ref() else {
            self.record_platform_state_replaced("platform callback ownership");
            return false;
        };
        unsafe { callbacks.detect_replacements(self) }
    }

    pub(crate) fn callback_teardown_active(&self) -> bool {
        self.callback_teardown_active.get()
    }

    pub(crate) fn defer_platform_viewport_restore(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) {
        if !self.callback_teardown_active() || viewport.is_null() || state.is_empty() {
            return;
        }
        let Some(key) = self.viewport_key(viewport).or_else(|| {
            #[cfg(test)]
            {
                return self.synthetic_viewport_key(viewport);
            }
            #[cfg(not(test))]
            {
                None
            }
        }) else {
            return;
        };
        self.deferred_platform_viewports.borrow_mut().insert(
            viewport as usize,
            DeferredPlatformViewportState { key, state },
        );
    }

    pub(crate) fn defer_renderer_viewport_restore(
        &self,
        viewport: *mut sys::ImGuiViewport,
        user_data: *mut std::ffi::c_void,
    ) {
        if !self.callback_teardown_active() || viewport.is_null() || user_data.is_null() {
            return;
        }
        if self.owned_renderer_viewport(viewport) == Some(user_data) {
            return;
        }
        let Some(key) = self.current_or_test_viewport_key(viewport) else {
            return;
        };
        self.deferred_renderer_viewports.borrow_mut().insert(
            viewport as usize,
            DeferredRendererViewportState { key, user_data },
        );
    }

    fn restore_deferred_viewport_state(&self) {
        let platform = std::mem::take(&mut *self.deferred_platform_viewports.borrow_mut());
        let renderer = std::mem::take(&mut *self.deferred_renderer_viewports.borrow_mut());
        let platform_keys = platform.keys().copied().collect::<HashSet<_>>();
        let mut restored_platform = HashSet::new();

        for (address, deferred) in platform {
            let viewport = address as *mut sys::ImGuiViewport;
            let can_restore = self
                .current_or_test_viewport_key(viewport)
                .is_some_and(|key| {
                    key.same_identity(deferred.key)
                        && key.id == deferred.key.id
                        && unsafe {
                            self.capture_viewport_platform_state(viewport)
                                .is_some_and(|state| state.is_empty())
                        }
                });
            if can_restore {
                if unsafe { self.restore_viewport_platform_state(viewport, deferred.state) } {
                    restored_platform.insert(address);
                }
            }
        }

        for (address, deferred) in renderer {
            let viewport = address as *mut sys::ImGuiViewport;
            let platform_is_compatible = if platform_keys.contains(&address) {
                restored_platform.contains(&address)
            } else {
                self.current_or_test_viewport_key(viewport).is_some()
                    && unsafe {
                        self.capture_viewport_platform_state(viewport)
                            .is_some_and(|state| state.is_empty())
                    }
            };
            let can_restore = self
                .current_or_test_viewport_key(viewport)
                .is_some_and(|key| {
                    key.same_identity(deferred.key)
                        && key.id == deferred.key.id
                        && unsafe {
                            self.viewport_renderer_user_data(viewport)
                                .is_some_and(|user_data| user_data.is_null())
                        }
                        && platform_is_compatible
                });
            if can_restore {
                let _ =
                    unsafe { self.set_viewport_renderer_user_data(viewport, deferred.user_data) };
            }
        }
    }

    pub(crate) fn refresh_platform_monitors_bound(&self) {
        if let Some(callbacks) = self.callbacks.borrow().as_ref() {
            unsafe { callbacks.refresh_owned_monitors() };
        }
    }

    pub(crate) fn remember_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) {
        let Some(key) = self.viewport_key(viewport).or_else(|| {
            #[cfg(test)]
            {
                return self.synthetic_viewport_key(viewport);
            }
            #[cfg(not(test))]
            {
                None
            }
        }) else {
            return;
        };
        self.owned_viewports
            .borrow_mut()
            .insert(viewport as usize, OwnedViewportLease { key, state });
    }

    pub(crate) fn take_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportPlatformState> {
        let key = self.current_or_test_viewport_key(viewport)?;
        let actual = unsafe { self.capture_viewport_platform_state(viewport) }?;
        let address = viewport as usize;
        let mut owned = self.owned_viewports.borrow_mut();
        let lease = owned.get(&address).copied()?;
        if !lease.key.same_identity(key) {
            owned.remove(&address);
            return None;
        }
        if lease.key.id != key.id && lease.state != actual {
            // A matching address is not sufficient authority after Dear ImGui has changed the
            // numeric ID. Docking may mutate an ID in place, but only an exact sidecar match proves
            // that the lease still belongs to that live viewport rather than a reused allocation.
            owned.remove(&address);
            return None;
        }
        owned.remove(&address).map(|lease| lease.state)
    }

    pub(crate) fn inspect_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<(ViewportPlatformState, ViewportPlatformState)> {
        let key = self.current_or_test_viewport_key(viewport)?;
        let address = viewport as usize;
        let mut owned = self.owned_viewports.borrow_mut();
        let lease = owned.get_mut(&address)?;
        if !lease.key.same_identity(key) {
            owned.remove(&address);
            return None;
        }
        let expected = lease.state;
        let actual = unsafe { ViewportPlatformState::capture(viewport) };
        if expected == actual {
            // Docking may change the numeric ID in place. Refresh it only after the complete
            // platform sidecar still proves that this is the same owned viewport.
            lease.key.id = key.id;
        }
        Some((expected, actual))
    }

    fn current_or_test_viewport_key(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportLeaseKey> {
        self.viewport_key(viewport).or_else(|| {
            #[cfg(test)]
            {
                return self.synthetic_viewport_key(viewport);
            }
            #[cfg(not(test))]
            {
                None
            }
        })
    }

    #[cfg(test)]
    fn synthetic_viewport_key(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportLeaseKey> {
        if viewport.is_null() {
            return None;
        }
        Some(ViewportLeaseKey {
            context: self.binding.id(),
            generation: self.platform_session_generation().unwrap_or(0),
            address: viewport as usize,
            id: unsafe { (*viewport).ID },
        })
    }

    pub(crate) fn remember_owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
        user_data: *mut std::ffi::c_void,
    ) {
        if !viewport.is_null() && !user_data.is_null() {
            let Some(key) = self.current_or_test_viewport_key(viewport) else {
                return;
            };
            self.owned_renderer_viewports.borrow_mut().insert(
                viewport as usize,
                OwnedRendererViewportLease { key, user_data },
            );
        }
    }

    pub(crate) fn owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut std::ffi::c_void> {
        let key = self.current_or_test_viewport_key(viewport)?;
        let actual = unsafe { self.viewport_renderer_user_data(viewport) }?;
        let mut owned = self.owned_renderer_viewports.borrow_mut();
        let lease = owned.get_mut(&(viewport as usize))?;
        if !lease.key.same_identity(key) {
            owned.remove(&(viewport as usize));
            return None;
        }
        if lease.key.id != key.id && lease.user_data != actual {
            owned.remove(&(viewport as usize));
            return None;
        }
        if lease.user_data == actual {
            // As with platform sidecars, an ID refresh is valid only after the renderer sidecar
            // itself still matches this runtime's lease.
            lease.key.id = key.id;
        }
        Some(lease.user_data)
    }

    pub(crate) fn forget_owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut std::ffi::c_void> {
        let key = self.current_or_test_viewport_key(viewport)?;
        let mut owned = self.owned_renderer_viewports.borrow_mut();
        let lease = owned.get(&(viewport as usize)).copied()?;
        if !lease.key.same_identity(key) {
            owned.remove(&(viewport as usize));
            return None;
        }
        let actual = unsafe { self.viewport_renderer_user_data(viewport) }?;
        if lease.key.id != key.id && lease.user_data != actual {
            owned.remove(&(viewport as usize));
            return None;
        }
        owned
            .remove(&(viewport as usize))
            .map(|lease| lease.user_data)
    }

    pub(crate) fn mark_viewport_failed(&self, viewport: *mut sys::ImGuiViewport) {
        let Some(key) = self.current_or_test_viewport_key(viewport) else {
            return;
        };
        self.failed_viewports
            .borrow_mut()
            .insert(viewport as usize, key);
        if let Some(current) = self.dispatch_failures.borrow_mut().last_mut() {
            current.insert(viewport as usize, key);
        }
        let _ = self.request_viewport_close(viewport);
    }

    pub(crate) fn viewport_failed(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        let Some(key) = self.current_or_test_viewport_key(viewport) else {
            return false;
        };
        let address = viewport as usize;
        let persistent = self.failed_viewports.borrow().get(&address).copied();
        if let Some(persistent) = persistent {
            if persistent == key {
                return true;
            }
            self.failed_viewports.borrow_mut().remove(&address);
        }
        self.dispatch_failures
            .borrow()
            .iter()
            .rev()
            .any(|failures| failures.get(&address).is_some_and(|failed| *failed == key))
    }

    pub(crate) fn forget_failed_viewport(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        let Some(key) = self.current_or_test_viewport_key(viewport) else {
            return false;
        };
        let address = viewport as usize;
        let removed = self
            .failed_viewports
            .borrow_mut()
            .remove(&address)
            .is_some_and(|failed| failed == key);
        for failures in self.dispatch_failures.borrow_mut().iter_mut() {
            if failures.get(&address).is_some_and(|failed| *failed == key) {
                failures.remove(&address);
            }
        }
        removed
    }

    fn request_failed_viewport_closes(&self) {
        let failed = self
            .failed_viewports
            .borrow()
            .iter()
            .map(|(address, key)| (*address, *key))
            .collect::<Vec<_>>();
        let _ = self.binding.try_with_bound_context(|| {
            let context = unsafe { sys::igGetCurrentContext() };
            let mut stale = Vec::new();
            for (address, key) in failed {
                let viewport =
                    unsafe { sys::ImGuiContext_FindLiveViewportByAddress(context, address) };
                let current = if viewport.is_null() {
                    None
                } else {
                    self.viewport_key(viewport)
                };
                if current.is_none_or(|current| {
                    current.context != key.context
                        || current.generation != key.generation
                        || current.address != key.address
                        || current.id != key.id
                }) {
                    stale.push(address);
                    continue;
                }
                let _ = self.request_viewport_close(viewport);
            }
            if !stale.is_empty() {
                let mut failures = self.failed_viewports.borrow_mut();
                for address in stale {
                    failures.remove(&address);
                }
            }
        });
    }

    fn record_fault(&self, fault: RuntimeFault) {
        self.faults.borrow_mut().push_back(fault);
    }

    pub(crate) fn record_callback_replaced(&self, callback: &'static str) {
        if self.reported_replacements.borrow_mut().insert(callback) {
            self.record_fault(RuntimeFault::CallbackReplaced(callback));
        }
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(crate) fn record_renderer_callback_replaced(&self, callback: &'static str) {
        if self.reported_replacements.borrow_mut().insert(callback) {
            self.record_fault(RuntimeFault::RendererCallbackReplaced(callback));
        }
        self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(crate) fn record_platform_state_replaced(&self, field: &'static str) {
        if self.reported_replacements.borrow_mut().insert(field) {
            self.record_fault(RuntimeFault::PlatformStateReplaced(field));
        }
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(crate) fn record_renderer_state_replaced(&self, field: &'static str) {
        if self.reported_replacements.borrow_mut().insert(field) {
            self.record_fault(RuntimeFault::RendererStateReplaced(field));
        }
        self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    /// Retain renderer capability bits only after a complete foreign renderer publication was
    /// observed. A single callback or core-field replacement is an incomplete takeover, so its
    /// untagged capability bits must stay revoked after SDL releases its own renderer.
    pub(crate) fn preserve_complete_foreign_renderer_capabilities(&self, flags: i32) {
        self.mark_capabilities_foreign(SDL_RENDERER_RESERVED_FLAGS);
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags = ((*io).BackendFlags & !SDL_RENDERER_RESERVED_FLAGS)
                    | (flags & SDL_RENDERER_RESERVED_FLAGS);
            }
        }
    }

    /// Retain platform capability bits only after the entire platform publication has moved to a
    /// foreign owner. Individual callback, userdata, or viewport-field replacements are partial
    /// takeovers and must leave SDL's capability bits revoked.
    pub(crate) fn preserve_complete_foreign_platform_capabilities(&self, flags: i32) {
        self.mark_capabilities_foreign(SDL_PLATFORM_RESERVED_FLAGS);
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags = ((*io).BackendFlags & !SDL_PLATFORM_RESERVED_FLAGS)
                    | (flags & SDL_PLATFORM_RESERVED_FLAGS);
            }
        }
    }

    pub(crate) fn record_callback_panicked(&self, callback: &'static str) {
        self.record_fault(RuntimeFault::CallbackPanicked(callback));
        if callback.starts_with("Renderer_") {
            self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        } else {
            self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        }
        self.begin_shutdown();
    }

    pub(crate) fn record_foreign_platform_user_data(&self) {
        if !self.foreign_platform_user_data_reported.replace(true) {
            self.record_fault(RuntimeFault::ForeignPlatformUserData);
        }
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    fn mark_capabilities_foreign(&self, mask: i32) {
        self.foreign_capabilities
            .set(self.foreign_capabilities.get() | mask);
    }

    fn revoke_capabilities(&self, mask: i32) {
        self.revoked_capabilities
            .set(self.revoked_capabilities.get() | mask);
        if self.capabilities_are_foreign(mask) {
            return;
        }
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags &= !mask;
            }
        }
    }

    pub(crate) fn capabilities_were_revoked(&self, mask: i32) -> bool {
        self.revoked_capabilities.get() & mask == mask
    }

    pub(crate) fn capabilities_are_foreign(&self, mask: i32) -> bool {
        self.foreign_capabilities.get() & mask != 0
    }

    pub(crate) fn record_viewport_creation_failed(&self) {
        self.record_fault(RuntimeFault::ViewportCreationFailed);
    }

    pub(crate) fn record_native_faults(&self, faults: u64, first_fault: u64) {
        const GL_SHARE_CAPTURE: u64 = 1 << 0;
        const GL_SHARE_SET: u64 = 1 << 1;
        const GL_MAIN_CONTEXT: u64 = 1 << 2;
        const GL_CREATE_CONTEXT: u64 = 1 << 4;
        const GL_SET_SWAP_INTERVAL: u64 = 1 << 5;
        const GL_RESTORE_CONTEXT: u64 = 1 << 6;
        const GL_RESTORE_SHARE: u64 = 1 << 7;
        const GL_RENDER_CONTEXT: u64 = 1 << 8;
        const GL_SWAP_CONTEXT: u64 = 1 << 9;
        const GL_SWAP_WINDOW: u64 = 1 << 10;
        const SDLGPU_CLAIM: u64 = 1 << 11;
        const SDLGPU_CONFIGURE: u64 = 1 << 12;
        const NATIVE_PROTOCOL: u64 = 1 << 13;
        const SDLGPU_COMMAND_BUFFER: u64 = 1 << 14;
        const SDLGPU_SWAPCHAIN: u64 = 1 << 15;
        const SDLGPU_RENDER_PASS: u64 = 1 << 16;
        const SDLGPU_SUBMIT: u64 = 1 << 17;
        const SDLGPU_CANCEL: u64 = 1 << 18;

        const GROUPS: &[(u64, RuntimeFault)] = &[
            (
                GL_SHARE_CAPTURE,
                RuntimeFault::ViewportOpenGlStateCaptureFailed,
            ),
            (
                GL_SHARE_SET,
                RuntimeFault::ViewportOpenGlShareConfigurationFailed,
            ),
            (
                GL_MAIN_CONTEXT | GL_CREATE_CONTEXT,
                RuntimeFault::ViewportOpenGlContextFailed,
            ),
            (
                GL_SET_SWAP_INTERVAL,
                RuntimeFault::ViewportOpenGlSwapIntervalFailed,
            ),
            (
                GL_RESTORE_SHARE | GL_RESTORE_CONTEXT,
                RuntimeFault::ViewportOpenGlStateRestoreFailed,
            ),
            (
                GL_RENDER_CONTEXT,
                RuntimeFault::ViewportOpenGlRenderContextFailed,
            ),
            (
                GL_SWAP_CONTEXT | GL_SWAP_WINDOW,
                RuntimeFault::ViewportOpenGlSwapFailed,
            ),
            (SDLGPU_CLAIM, RuntimeFault::ViewportSdlGpuClaimFailed),
            (
                SDLGPU_CONFIGURE,
                RuntimeFault::ViewportSdlGpuConfigureFailed,
            ),
            (NATIVE_PROTOCOL, RuntimeFault::NativeBridgeProtocolFailed),
            (
                SDLGPU_COMMAND_BUFFER,
                RuntimeFault::ViewportSdlGpuCommandBufferFailed,
            ),
            (
                SDLGPU_SWAPCHAIN,
                RuntimeFault::ViewportSdlGpuSwapchainFailed,
            ),
            (
                SDLGPU_CANCEL,
                RuntimeFault::ViewportSdlGpuCommandBufferCancelFailed,
            ),
            (
                SDLGPU_RENDER_PASS,
                RuntimeFault::ViewportSdlGpuRenderPassFailed,
            ),
            (SDLGPU_SUBMIT, RuntimeFault::ViewportSdlGpuSubmitFailed),
        ];

        let mut recorded_groups = 0;
        let first_fault = first_fault & faults;
        if first_fault != 0 {
            debug_assert!(first_fault.is_power_of_two());
            if let Some((mask, fault)) = GROUPS.iter().find(|(mask, _)| first_fault & *mask != 0) {
                self.record_fault(*fault);
                recorded_groups |= *mask;
            }
        }

        for &(mask, fault) in GROUPS {
            if faults & mask != 0 && recorded_groups & mask == 0 {
                self.record_fault(fault);
                recorded_groups |= mask;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn record_viewport_opengl_context_failed_for_test(&self) {
        self.record_fault(RuntimeFault::ViewportOpenGlContextFailed);
    }

    fn context_destroyed(&self) {
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.renderer_callbacks.borrow_mut().take();
        self.renderer_shutdown_restore.borrow_mut().take();
        self.renderer_consumer.borrow_mut().take();
        self.release_platform_session();
        self.owned_viewports.borrow_mut().clear();
        self.owned_renderer_viewports.borrow_mut().clear();
        self.deferred_platform_viewports.borrow_mut().clear();
        self.deferred_renderer_viewports.borrow_mut().clear();
        self.failed_viewports.borrow_mut().clear();
        self.platform_initialized.set(false);
        self.renderer_initialized.set(false);
        self.renderer_release.set(ReleaseState::Released);
        self.platform_release.set(ReleaseState::Released);
        self.state.set(RuntimeState::Detached);
    }

    fn mark_owner_dropped(&self) {
        if self.state.get() == RuntimeState::Detached {
            self.state.set(RuntimeState::ResourceDropped);
        }
    }

    #[cfg(test)]
    fn phase_log(&self) -> Vec<&'static str> {
        self.phase_log.borrow().clone()
    }

    fn accepts_current_callback(&self) -> bool {
        if !self.platform_initialized.get() {
            return false;
        }
        match (self.state.get(), self.binding.lifecycle()) {
            (RuntimeState::Attached, ContextLifecycle::Alive) => true,
            (RuntimeState::ShuttingDown, ContextLifecycle::Alive | ContextLifecycle::Dropping) => {
                self.callback_teardown_active.get()
            }
            _ => false,
        }
    }
}

impl Sdl3ViewportRendererAdapter {
    /// Returns the Context identity of this exact platform generation.
    #[must_use]
    pub fn context_id(&self) -> ContextId {
        self.control.binding().id()
    }

    /// Runs one renderer route attempt and retains every deferred platform fault.
    pub fn run<R>(&self, callback: impl FnOnce() -> R) -> Sdl3ViewportAttempt<R> {
        let faults = self.control.drain_faults();
        if !faults.is_empty() {
            return Sdl3ViewportAttempt::skipped(faults);
        }
        if let Err(error) = self.control.ensure_bound_entry() {
            let mut faults = vec![error];
            faults.extend(self.control.drain_faults());
            return Sdl3ViewportAttempt::skipped(faults);
        }

        let output = {
            let _dispatch = self.control.begin_platform_dispatch();
            callback()
        };
        Sdl3ViewportAttempt::completed(output, self.control.drain_faults())
    }
}

fn first_error<const N: usize>(
    errors: [Option<Sdl3BackendError>; N],
) -> Result<(), Sdl3BackendError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}
