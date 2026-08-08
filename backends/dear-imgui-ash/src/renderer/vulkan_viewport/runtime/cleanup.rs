use dear_imgui_rs::render::SynchronousRendererConsumer;
use dear_imgui_rs::{Context, ContextAttachmentLease, ContextLifecycle};

use super::super::callbacks::{destroy_renderer_viewport_resources, release_callbacks};
use super::super::registry::{take_viewport_data, unregister_runtime};
use super::{
    AshViewportError, RendererStorage, RuntimeControl, RuntimeState, ShutdownAction, first_error,
    map_renderer_shutdown_result,
};
use crate::renderer::lifecycle::{DeviceIdleOutcome, classify_device_idle};
use crate::{AshRenderer, RendererError};

impl RuntimeControl {
    pub(in super::super) fn wait_device_idle(
        &self,
        renderer: &AshRenderer,
        operation: &'static str,
    ) -> Result<(), AshViewportError> {
        self.wait_device_idle_outcome(renderer, operation)
            .map(|_| ())
    }

    pub(in super::super) fn wait_device_idle_outcome(
        &self,
        renderer: &AshRenderer,
        operation: &'static str,
    ) -> Result<DeviceIdleOutcome, AshViewportError> {
        match classify_device_idle(unsafe { renderer.device.device_wait_idle() }) {
            Ok(DeviceIdleOutcome::Complete) => Ok(DeviceIdleOutcome::Complete),
            Ok(DeviceIdleOutcome::DeviceLost) => {
                self.record_entry_fault(AshViewportError::DeviceLost { operation });
                Ok(DeviceIdleOutcome::DeviceLost)
            }
            Err(source) => Err(AshViewportError::DeviceCompletionFailed { operation, source }),
        }
    }

    fn shutdown_explicit(&self, context: &mut Context) -> Result<(), AshViewportError> {
        // Validate snapshot completion before mutating any viewport, callback, or runtime state.
        // A failed permit preparation must leave the entire multi-viewport runtime retryable.
        let consumer = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            let storage = storage.as_mut().ok_or(AshViewportError::RuntimeDetached)?;
            match storage {
                RendererStorage::Real(renderer) => renderer.take_shutdown_consumer()?,
                #[cfg(test)]
                RendererStorage::Fake { consumer, .. } => {
                    consumer.take().ok_or(RendererError::RendererNotAttached)?
                }
            }
        };

        let permit = match context.prepare_renderer_texture_reset(&consumer) {
            Ok(permit) => permit,
            Err(error) => {
                self.restore_explicit_shutdown_consumer(consumer)?;
                return Err(RendererError::from(error).into());
            }
        };

        self.begin_shutdown();
        if let Err(error) = destroy_renderer_viewport_resources(self) {
            drop(permit);
            self.restore_explicit_shutdown_consumer(consumer)?;
            return Err(error);
        }
        let callback_result = release_callbacks(self);
        self.mark_detached();

        let (shutdown_result, destroyed) = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
                RendererStorage::Real(renderer) => {
                    let shutdown_result = renderer.destroy_internal();
                    (shutdown_result, renderer.destroyed)
                }
                #[cfg(test)]
                RendererStorage::Fake { .. } => (Ok(()), true),
            }
        };

        if !destroyed {
            drop(permit);
            self.restore_explicit_shutdown_consumer(consumer)?;
            return first_error([
                callback_result.err(),
                map_renderer_shutdown_result(shutdown_result, "renderer shutdown").err(),
            ]);
        }

        // The renderer's complete texture map is gone, so the already-validated reset can now
        // invalidate Context-owned bindings before we publish the renderer teardown.
        permit.commit();
        let renderer = {
            let mut storage = self.renderer.try_borrow_mut().map_err(|_| {
                AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                }
            })?;
            match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
                RendererStorage::Real(renderer) => renderer.finalize_shutdown_after_reset(context),
                #[cfg(test)]
                RendererStorage::Fake { .. } => {}
            }
            storage.take()
        };
        drop(renderer);
        drop(consumer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        self.detach_attachment();
        first_error([
            callback_result.err(),
            map_renderer_shutdown_result(shutdown_result, "renderer shutdown").err(),
        ])
    }

    pub(super) fn restore_explicit_shutdown_consumer(
        &self,
        consumer: SynchronousRendererConsumer,
    ) -> Result<(), AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Ash viewport runtime shutdown",
                })?;
        match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
            RendererStorage::Real(renderer) => renderer.restore_shutdown_consumer(consumer),
            #[cfg(test)]
            RendererStorage::Fake {
                consumer: stored_consumer,
                ..
            } => {
                debug_assert!(stored_consumer.is_none());
                *stored_consumer = Some(consumer);
            }
        }
        Ok(())
    }

    pub(super) fn take_context_teardown_consumer(
        &self,
    ) -> Result<SynchronousRendererConsumer, AshViewportError> {
        let mut storage =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        match storage.as_mut().ok_or(AshViewportError::RuntimeDetached)? {
            RendererStorage::Real(renderer) => {
                renderer.take_shutdown_consumer().map_err(Into::into)
            }
            #[cfg(test)]
            RendererStorage::Fake { consumer, .. } => consumer
                .take()
                .ok_or(RendererError::RendererNotAttached)
                .map_err(Into::into),
        }
    }

    /// Releases the renderer after Context entered its terminal teardown phase.
    ///
    /// This may run only from the release closure of `ContextTeardown::with_renderer_texture_reset`.
    fn release_renderer_during_context_teardown(&self) -> Result<(), AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        let Some(storage) = renderer.as_mut() else {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        };
        let shutdown_result = match storage {
            RendererStorage::Real(renderer) => renderer.shutdown_during_context_teardown(),
            #[cfg(test)]
            RendererStorage::Fake { .. } => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake { .. } => true,
        };
        if !may_release {
            return shutdown_result.map_err(Into::into);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        map_renderer_shutdown_result(shutdown_result, "renderer teardown")
    }

    /// Releases remaining renderer resources after native Context destruction.
    ///
    /// A previous Context teardown can only reach this retry path after a retryable Vulkan wait
    /// failure. Native ImGui state is gone, so the renderer must not attempt a texture reset or
    /// touch current-context global pointers.
    fn release_renderer_after_context_destroyed(&self) -> Result<(), AshViewportError> {
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| AshViewportError::CallbackReentered {
                    callback: "destroyed Context renderer-resource cleanup",
                })?;
        let Some(storage) = renderer.as_mut() else {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        };
        let shutdown_result = match storage {
            RendererStorage::Real(renderer) => renderer.shutdown_after_context_destroyed(),
            #[cfg(test)]
            RendererStorage::Fake { .. } => Ok(()),
        };
        let may_release = match storage {
            RendererStorage::Real(renderer) => renderer.destroyed,
            #[cfg(test)]
            RendererStorage::Fake { .. } => true,
        };
        if !may_release {
            return shutdown_result.map_err(Into::into);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        map_renderer_shutdown_result(
            shutdown_result,
            "renderer cleanup after Context destruction",
        )
    }

    pub(super) fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), AshViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            if !matches!(action, ShutdownAction::ContextTeardown) {
                self.detach_attachment();
            }
            return Ok(());
        }

        match action {
            ShutdownAction::Quiesce => {
                self.begin_shutdown();
                release_callbacks(self)
            }
            ShutdownAction::Explicit(context) => self.shutdown_explicit(context),
            ShutdownAction::ContextTeardown => {
                self.begin_shutdown();
                // A failed ownership preflight leaves every sidecar and callback publication
                // intact. Do not clear `Renderer_DestroyWindow` while it is still the only safe
                // way to reclaim a live foreign-replaced slot.
                destroy_renderer_viewport_resources(self)?;
                let callback_result = release_callbacks(self);
                self.mark_detached();
                let renderer_result = self.release_renderer_during_context_teardown();
                first_error([callback_result.err(), renderer_result.err()])
            }
        }
    }

    pub(super) fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        match self.binding.lifecycle() {
            // `Drop` has no exclusive `&mut Context`, so it cannot prepare and commit the
            // renderer-texture reset transaction. Leave the attachment owned by Context instead
            // of releasing Vulkan resources behind still-live managed texture bindings.
            ContextLifecycle::Alive => self.defer_attachment_to_context(),
            // Context has already begun its ordered teardown and still owns this attachment.
            // Its RendererResources phase performs the terminal cleanup with the Context bound.
            ContextLifecycle::Dropping => {}
            ContextLifecycle::NativeDestroyed => {
                if let Err(error) = self.retry_detached_cleanup() {
                    self.record_fault(error);
                }
            }
            _ => {}
        }
    }

    pub(super) fn store_attachment(&self, attachment: ContextAttachmentLease) {
        self.attachment.borrow_mut().replace(attachment);
    }

    fn detach_attachment(&self) {
        if let Some(mut attachment) = self.attachment.borrow_mut().take() {
            let _ = attachment
                .detach()
                .expect("a renderer attachment cannot have a platform release dependency");
        }
    }

    fn defer_attachment_to_context(&self) {
        if let Some(attachment) = self.attachment.borrow_mut().take() {
            attachment.defer_to_context();
        }
    }

    pub(super) fn recover_renderer(&self) -> AshRenderer {
        self.globals.borrow_mut().take();
        match self
            .renderer
            .borrow_mut()
            .take()
            .expect("failed Ash runtime construction lost its renderer")
        {
            RendererStorage::Real(renderer) => *renderer,
            #[cfg(test)]
            RendererStorage::Fake { .. } => {
                unreachable!("test runtime does not recover AshRenderer")
            }
        }
    }

    pub(super) fn mark_context_destroyed(&self) {
        unregister_runtime(self.binding.id());
        self.retained_viewports
            .borrow_mut()
            .extend(take_viewport_data(self.binding.id()));
        self.attachment.borrow_mut().take();
        if self.renderer.borrow().is_none() {
            self.set_state(RuntimeState::ResourceDropped);
        } else {
            self.mark_detached();
            if let Err(error) = self.retry_detached_cleanup() {
                self.record_fault(error);
            }
        }
    }

    pub(super) fn retry_detached_cleanup(&self) -> Result<(), AshViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            return Ok(());
        }
        if !self.retained_viewports.borrow().is_empty() {
            self.with_renderer_teardown(|renderer, globals| {
                self.wait_device_idle(renderer, "retained viewport cleanup")?;
                let surface_loader =
                    super::super::khr_surface::Instance::new(&globals.entry, &globals.instance);
                let retained = std::mem::take(&mut *self.retained_viewports.borrow_mut());
                for data in retained {
                    data.destroy_after_device_idle(renderer, &surface_loader)?;
                }
                Ok(())
            })?;
        }
        self.release_renderer_after_context_destroyed()
    }
}
