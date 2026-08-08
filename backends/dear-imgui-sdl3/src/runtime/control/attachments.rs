use super::*;

pub(super) struct PlatformAttachment {
    pub(super) control: Rc<RuntimeControl>,
}

impl ContextAttachment for PlatformAttachment {
    fn quiesce(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        Ok(())
    }

    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        context
            .with_bound_context(|| self.control.shutdown_bound_for_attachment())
            .map_err(|error| {
                ContextAttachmentTeardownError::new(format!(
                    "SDL3 Context teardown could not safely release native resources: {error}"
                ))
            })
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.control.context_destroyed();
    }
}

pub(super) struct RendererAttachment {
    pub(super) control: Rc<RuntimeControl>,
}

impl ContextAttachment for RendererAttachment {
    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        let Some(consumer) = self.control.take_renderer_consumer() else {
            if self.control.lifecycle.renderer_texture_update.is_some() {
                return Err(ContextAttachmentTeardownError::new(
                    "SDL3 renderer-resource teardown lost its renderer consumer",
                ));
            }
            return Ok(());
        };

        // OpenGL3 and SDLGPU3 call DestroyPlatformWindows() from full shutdown. Destroy only
        // their device objects here, then keep the callback tables alive until the platform phase.
        let reset = context.with_bound_context(|| {
            context.with_renderer_texture_reset(consumer.as_ref(), || {
                self.control
                    .release_renderer_device_objects_bound()
                    .map_err(|error| {
                        ContextAttachmentTeardownError::new(format!(
                            "SDL3 Context teardown could not release renderer device objects: {error}"
                        ))
                    })
            })
        });
        if let Err(error) = reset {
            self.control.install_renderer_consumer(consumer);
            return Err(error);
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.control.clear_destroyed_textures();
        Ok(())
    }
}
