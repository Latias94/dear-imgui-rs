use crate::{Sdl3BackendError, Sdl3DropEvent, Sdl3DropTarget};

use super::*;

impl RuntimeControl {
    pub(crate) fn callback_event_handoff(&self) -> crate::Sdl3CallbackEventHandoff {
        crate::Sdl3CallbackEventHandoff::for_window_targets(std::sync::Arc::clone(
            &self.callback_window_targets,
        ))
    }

    pub(crate) fn resolve_drop_target(
        &self,
        event: &Sdl3DropEvent,
    ) -> Result<Sdl3DropTarget, Sdl3BackendError> {
        let Some(observed) = event.observed_target else {
            return Ok(Sdl3DropTarget::Unowned);
        };
        if observed.instance_id.context_id() != self.binding.id() {
            return Ok(Sdl3DropTarget::Unowned);
        }
        if self.state.get() != RuntimeState::Attached {
            return Ok(Sdl3DropTarget::Retired(observed));
        }
        let current = self.window_snapshot_for_instance(observed.instance_id)?;
        Ok(match current {
            Some(current) if current.window_id == event.window_id => {
                Sdl3DropTarget::Current(current.target)
            }
            _ => Sdl3DropTarget::Retired(observed),
        })
    }
}
