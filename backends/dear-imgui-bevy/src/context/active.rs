use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use bevy_ecs::schedule::InternedScheduleLabel;
use bevy_ecs::system::{NonSend, NonSendMarker, SystemParam};
use dear_imgui_rs::{ContextId, Ui};

use super::{ImguiContextError, ImguiContexts};

#[derive(Clone, Copy)]
struct ActiveFrame {
    context_id: ContextId,
    schedule: InternedScheduleLabel,
    frame_index: u64,
    ui: NonNull<Ui>,
}

#[derive(Default)]
struct ActiveUiControl {
    frame: Cell<Option<ActiveFrame>>,
}

/// Non-send state backing the schedule-scoped [`ImguiUi`] capability.
#[derive(Clone, Default)]
pub(crate) struct ImguiActiveUi {
    control: Rc<ActiveUiControl>,
}

impl ImguiActiveUi {
    pub(crate) fn capability(&self) -> ActiveUiCapability {
        ActiveUiCapability {
            control: Rc::clone(&self.control),
        }
    }
}

/// Driver-owned capability that installs and revokes one live `Ui`.
#[derive(Clone)]
pub(crate) struct ActiveUiCapability {
    control: Rc<ActiveUiControl>,
}

impl ActiveUiCapability {
    pub(crate) fn install(
        &self,
        context_id: ContextId,
        schedule: InternedScheduleLabel,
        frame_index: u64,
        ui: &Ui,
    ) {
        assert!(
            self.control.frame.get().is_none(),
            "dear-imgui-bevy attempted to expose two active Context frames"
        );
        self.control.frame.set(Some(ActiveFrame {
            context_id,
            schedule,
            frame_index,
            ui: NonNull::from(ui),
        }));
    }

    pub(crate) fn revoke(&self) {
        self.control.frame.set(None);
    }
}

impl Drop for ActiveUiCapability {
    fn drop(&mut self) {
        self.revoke();
    }
}

/// Schedule-scoped access to the currently driven Dear ImGui `Ui`.
///
/// This capability is available only while the registry driver is running one Context's UI
/// schedule. It never exposes the underlying mutable [`dear_imgui_rs::Context`].
#[derive(SystemParam)]
pub struct ImguiUi<'w> {
    active: NonSend<'w, ImguiActiveUi>,
    contexts: NonSend<'w, ImguiContexts>,
    _main_thread: NonSendMarker,
}

impl ImguiUi<'_> {
    /// Return the Context identity bound to the current UI schedule.
    pub fn context_id(&self) -> Result<ContextId, ImguiContextError> {
        self.active
            .control
            .frame
            .get()
            .map(|frame| frame.context_id)
            .ok_or(ImguiContextError::NoOpenFrame)
    }

    /// Return the frame index local to the active Context.
    pub fn frame_index(&self) -> Result<u64, ImguiContextError> {
        self.active
            .control
            .frame
            .get()
            .map(|frame| frame.frame_index)
            .ok_or(ImguiContextError::NoOpenFrame)
    }

    /// Borrow the `Ui` for the active Context schedule.
    pub fn ui(&self) -> Result<&Ui, ImguiContextError> {
        let frame = self
            .active
            .control
            .frame
            .get()
            .ok_or(ImguiContextError::NoOpenFrame)?;
        // SAFETY: only the serial driver installs this pointer, and its unwind guard revokes the
        // capability before ending or suspending the frame. `ImguiUi` is a non-send SystemParam,
        // and the returned borrow cannot outlive this SystemParam borrow.
        Ok(unsafe { frame.ui.as_ref() })
    }

    /// Borrow the `Ui` only if this schedule is driving `context_id`.
    pub fn ui_for(&self, context_id: ContextId) -> Result<&Ui, ImguiContextError> {
        if !self.contexts.contains(context_id) {
            return Err(ImguiContextError::UnknownContext { context_id });
        }
        if self.contexts.is_tearing_down(context_id) {
            return Err(ImguiContextError::TeardownInProgress { context_id });
        }
        let frame = self
            .active
            .control
            .frame
            .get()
            .ok_or(ImguiContextError::NoOpenFrame)?;
        if frame.context_id != context_id {
            return Err(ImguiContextError::WrongSchedule {
                requested: context_id,
                active: frame.context_id,
                active_schedule: frame.schedule,
            });
        }
        self.ui()
    }
}
