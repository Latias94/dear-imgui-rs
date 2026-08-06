use super::*;

pub(crate) struct RuntimeRegistration {
    pub(super) control: Rc<RuntimeControl>,
    pub(super) renderer_consumer: Option<Rc<SynchronousRendererConsumer>>,
    pub(super) baseline: Option<PlatformClaimBaseline>,
    pub(super) platform_attachment: Option<ContextAttachmentLease>,
    pub(super) renderer_attachment: Option<ContextAttachmentLease>,
}

impl fmt::Debug for RuntimeRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeRegistration")
            .field("control", &self.control)
            .field("has_renderer_consumer", &self.renderer_consumer.is_some())
            .field("native_initialization_pending", &self.baseline.is_some())
            .field("platform_attached", &self.platform_attachment.is_some())
            .field("renderer_attached", &self.renderer_attachment.is_some())
            .finish()
    }
}

impl RuntimeRegistration {
    pub(crate) fn set_gl_viewport_swap_interval(&mut self, policy: Sdl3OpenGlViewportSwapInterval) {
        self.control.set_gl_viewport_swap_interval(policy);
    }

    pub(crate) fn prepare_with_backend(
        context: &mut Context,
        renderer_shutdown: Option<fn()>,
        renderer_device_objects_destroy: Option<fn()>,
        renderer_texture_update: Option<fn(&mut TextureData)>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> Result<Self, Sdl3BackendError> {
        let platform_session = Sdl3PlatformSession::acquire()?;
        let baseline = preflight_platform_claim(context, native_renderer)?;
        let renderer_shutdown = renderer_shutdown.map(|shutdown| Rc::new(shutdown) as Rc<dyn Fn()>);
        let renderer_device_objects_destroy =
            renderer_device_objects_destroy.map(|destroy| Rc::new(destroy) as Rc<dyn Fn()>);
        let renderer_texture_update =
            renderer_texture_update.map(|update| Rc::new(update) as Rc<dyn Fn(&mut TextureData)>);
        let control = Rc::new(RuntimeControl::new_with_backend(
            context,
            NativeLifecycle::new(
                renderer_shutdown,
                renderer_device_objects_destroy,
                renderer_texture_update,
                Rc::new(shutdown_platform_impl),
            ),
            Some(platform_session),
            platform_graphics,
            native_renderer,
        ));
        let mut platform_attachment = context.register_attachment::<Sdl3PlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(PlatformAttachment {
                control: Rc::clone(&control),
            }),
        )?;
        let renderer_attachment = if control.lifecycle.renderer_shutdown.is_some() {
            match context.register_attachment::<Sdl3RendererAttachmentMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(RendererAttachment {
                    control: Rc::clone(&control),
                }),
            ) {
                Ok(attachment) => Some(attachment),
                Err(error) => {
                    let _ = platform_attachment.detach().expect(
                        "SDL3 renderer registration failed before a renderer dependency existed",
                    );
                    return Err(error.into());
                }
            }
        } else {
            None
        };

        Ok(Self {
            control,
            renderer_consumer: None,
            baseline: Some(baseline),
            platform_attachment: Some(platform_attachment),
            renderer_attachment,
        })
    }

    pub(crate) fn finish_native_initialization(
        &mut self,
        context: &Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        let baseline = self
            .baseline
            .as_ref()
            .expect("SDL3 native initialization was already completed")
            .snapshot();
        let renderer_baseline = baseline.snapshot();
        self.control.platform_initialized.set(true);
        self.control
            .renderer_initialized
            .set(self.control.lifecycle.renderer_shutdown.is_some());
        let claim_result = self.control.binding.try_with_bound_context(|| unsafe {
            let platform = PlatformCallbackOwnership::claim(&self.control, baseline)?;
            let renderer = RendererCallbackOwnership::claim(&self.control, &renderer_baseline)?;
            Ok::<_, Sdl3BackendError>((platform, renderer))
        });
        match claim_result {
            Ok(Ok((ownership, renderer_ownership))) => {
                self.baseline.take();
                self.control.callbacks.borrow_mut().replace(ownership);
                if let Some(renderer_ownership) = renderer_ownership {
                    self.control
                        .renderer_callbacks
                        .borrow_mut()
                        .replace(renderer_ownership);
                }
                Ok(())
            }
            Ok(Err(error)) => {
                self.rollback_claim_failure();
                Err(error)
            }
            Err(error) => {
                self.rollback_claim_failure();
                Err(error.into())
            }
        }
    }

    fn rollback_claim_failure(&mut self) {
        let baseline = self.baseline.take();
        self.control.begin_shutdown();
        let _ = self.control.binding.try_with_bound_context(|| {
            let _ = self.control.release_platform_bound();
            if self.control.platform_released() {
                let _ = self.control.release_renderer_bound();
            }
            if let Some(baseline) = baseline {
                unsafe { restore_baseline_after_failed_initialization(baseline) };
            }
        });
        self.detach_attachments();
    }

    pub(crate) fn native_initialization_failed(&mut self) {
        if let Some(baseline) = self.baseline.take() {
            let _ = self.control.binding.try_with_bound_context(|| unsafe {
                restore_baseline_after_failed_initialization(baseline)
            });
        }
        self.control.platform_initialized.set(false);
        self.control.renderer_initialized.set(false);
        self.control.release_platform_session();
        self.control.take_renderer_consumer();
        self.renderer_consumer.take();
        self.control.renderer_release.set(ReleaseState::Released);
        self.control.platform_release.set(ReleaseState::Released);
        self.control.state.set(RuntimeState::Detached);
        self.detach_attachments();
    }

    pub(crate) fn control(&self) -> &RuntimeControl {
        &self.control
    }

    pub(crate) fn install_renderer_consumer(&mut self, consumer: SynchronousRendererConsumer) {
        let consumer = Rc::new(consumer);
        self.control.install_renderer_consumer(Rc::clone(&consumer));
        let previous = self.renderer_consumer.replace(consumer);
        assert!(
            previous.is_none(),
            "SDL3 runtime registration already owns a renderer consumer"
        );
    }

    pub(crate) fn renderer_consumer(&self) -> &SynchronousRendererConsumer {
        self.renderer_consumer
            .as_deref()
            .expect("SDL3 renderer consumer is unavailable after renderer shutdown")
    }

    pub(crate) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.control.poll_fault()
    }

    pub(crate) fn begin_opengl_viewport_frame_trace(
        &self,
    ) -> Result<Sdl3OpenGlViewportFrameTrace<'_>, Sdl3OpenGlViewportFrameTraceError> {
        self.control.begin_opengl_viewport_frame_trace()
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn acquire_vulkan_surface_provider(
        &self,
        context: &Context,
    ) -> Result<Sdl3VulkanSurfaceProvider, Sdl3BackendError> {
        self.control.acquire_vulkan_surface_provider(context)
    }

    pub(crate) fn normalize_open_frame_for_shutdown(
        &self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        if self.control.state() == RuntimeState::Attached {
            context.end_frame();
        }
        Ok(())
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn destroy_renderer_device_objects(
        &self,
        context: &mut Context,
        destroy_device_objects: impl FnOnce(),
    ) -> Result<(), Sdl3BackendError> {
        let entry = self.control.enter(context)?;
        let consumer_guard = self.control.renderer_consumer.borrow();
        let consumer = consumer_guard
            .as_ref()
            .expect("initialized SDL3 renderer lost its renderer consumer");
        let reset = context.prepare_renderer_texture_reset(consumer.as_ref())?;

        self.control.binding.try_with_bound_context(|| {
            self.control.destroy_uninstalled_renderer_textures_bound()?;
            destroy_device_objects();
            self.control.forget_textures_destroyed_by_upstream();
            Ok::<(), Sdl3BackendError>(())
        })??;

        reset.commit();
        drop(consumer_guard);
        self.control.clear_destroyed_textures();
        entry.finish()
    }

    pub(crate) fn shutdown_platform(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        if self.renderer_attachment.is_some() || self.platform_attachment.is_none() {
            let result = self.shutdown_platform_inner(context);
            if matches!(self.control.state(), RuntimeState::Detached) {
                self.detach_attachments();
            }
            return result;
        }

        let attachment = self
            .platform_attachment
            .as_ref()
            .expect("an SDL3 platform-only runtime must retain its attachment")
            .handle();
        let mut release = context.prepare_platform_attachment_release(&attachment)?;
        #[cfg(feature = "multi-viewport")]
        self.control.ensure_vulkan_surface_provider_released()?;
        let result = self.shutdown_platform_inner(release.context_mut());
        if matches!(self.control.state(), RuntimeState::Detached) {
            release.commit();
            self.detach_attachments();
        }
        result
    }

    fn shutdown_platform_inner(&mut self, context: &mut Context) -> Result<(), Sdl3BackendError> {
        self.normalize_open_frame_for_shutdown(context)?;
        let pending = self.control.take_pending_fault();
        let shutdown_result = self.control.shutdown_native_explicit();
        first_error([pending, shutdown_result.err()])
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(crate) fn shutdown_renderer(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.normalize_open_frame_for_shutdown(context)?;
        if matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            let pending = self.control.take_pending_fault();
            self.detach_attachments();
            return first_error([pending, None]);
        }

        let (pending, shutdown_result, renderer_released) = {
            let consumer_guard = (!self.control.renderer_released())
                .then(|| self.control.renderer_consumer.borrow());
            let reset = match consumer_guard.as_ref() {
                Some(consumer) => {
                    let consumer = consumer
                        .as_ref()
                        .expect("initialized SDL3 renderer lost its renderer consumer");
                    match context.prepare_renderer_texture_reset(consumer.as_ref()) {
                        Ok(reset) => Some(reset),
                        Err(error) => return Err(error.into()),
                    }
                }
                None => None,
            };
            let pending = self.control.take_pending_fault();
            let shutdown_result = self.control.shutdown_native_explicit();
            let renderer_released = self.control.renderer_released();
            if renderer_released && let Some(reset) = reset {
                reset.commit();
            }
            (pending, shutdown_result, renderer_released)
        };
        if renderer_released {
            self.control.take_renderer_consumer();
            self.renderer_consumer.take();
            self.control.clear_destroyed_textures();
        }
        if matches!(self.control.state(), RuntimeState::Detached) {
            self.detach_attachments();
        }
        first_error([pending, shutdown_result.err()])
    }

    fn detach_attachments(&mut self) {
        if let Some(mut renderer) = self.renderer_attachment.take() {
            let _ = renderer
                .detach()
                .expect("a renderer attachment cannot have a platform release dependency");
        }
        if let Some(mut platform) = self.platform_attachment.take() {
            let _ = platform
                .detach()
                .expect("SDL3 detaches its renderer attachment before its platform attachment");
        }
    }

    fn defer_attachments_to_context(&mut self) {
        if let Some(renderer) = self.renderer_attachment.take() {
            renderer.defer_to_context();
        }
        if let Some(platform) = self.platform_attachment.take() {
            platform.defer_to_context();
        }
    }
}

impl Drop for RuntimeRegistration {
    fn drop(&mut self) {
        self.defer_attachments_to_context();
        self.control.mark_owner_dropped();
    }
}

thread_local! {
    static RUNTIMES: RefCell<HashMap<usize, Weak<RuntimeControl>>> = RefCell::new(HashMap::new());
}

#[cfg(test)]
pub(super) fn registry_contains(key: usize) -> bool {
    RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(&key))
}

pub(crate) fn register_runtime(control: &Rc<RuntimeControl>) {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    control.platform_io_key.set(key);
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().insert(key, Rc::downgrade(control));
    });
}

pub(crate) fn unregister_runtime(key: usize) {
    if key == 0 {
        return;
    }
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().remove(&key);
    });
}

pub(crate) fn with_current_runtime<R>(callback: impl FnOnce(&RuntimeControl) -> R) -> Option<R> {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    RUNTIMES.with(|runtimes| {
        let control = runtimes.borrow().get(&key).cloned()?.upgrade()?;
        control
            .accepts_current_callback()
            .then(|| callback(&control))
    })
}
