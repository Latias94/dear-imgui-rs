use dear_imgui_rs::{
    ContextAttachment, ContextAttachmentTeardownError, ContextDestroyed, ContextTeardown,
};

use super::{RuntimeControl, RuntimeState, ShutdownAction};

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| match self.shutdown_once(ShutdownAction::Quiesce) {
            Ok(()) => Ok(()),
            Err(error) => {
                let teardown_error = ContextAttachmentTeardownError::new(error.to_string());
                self.record_fault(error);
                Err(teardown_error)
            }
        })
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            let consumer = self
                .take_context_teardown_consumer()
                .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))?;
            let mut terminal_error = None;
            let reset = context.with_renderer_texture_reset(&consumer, || {
                match self.shutdown_once(ShutdownAction::ContextTeardown) {
                    Ok(()) => Ok(()),
                    Err(error) if self.state.get() == RuntimeState::ResourceDropped => {
                        terminal_error = Some(error);
                        Ok(())
                    }
                    Err(error) => Err(ContextAttachmentTeardownError::new(error.to_string())),
                }
            });
            if let Err(error) = reset {
                if let Err(restore_error) = self.restore_explicit_shutdown_consumer(consumer) {
                    self.record_fault(restore_error);
                }
                return Err(error);
            }
            drop(consumer);

            if let Some(error) = terminal_error {
                let teardown_error = ContextAttachmentTeardownError::new(error.to_string());
                self.record_fault(error);
                Err(teardown_error)
            } else {
                Ok(())
            }
        })
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.mark_context_destroyed();
    }
}
