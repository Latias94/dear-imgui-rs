/// Lifecycle of the Test Engine's unique Context attachment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AttachmentState {
    /// The native engine exists but has no Context attachment.
    Detached,
    /// The attachment slot is reserved while native start is transactional.
    Reserved,
    /// The engine is attached to a live Context.
    Attached,
    /// Context teardown has quiesced the engine but native destruction has not run yet.
    ContextDropping,
    /// The native Context was destroyed and the upstream hook detached the engine.
    ContextDestroyed,
    /// The native engine itself has been destroyed.
    Destroyed,
}

/// Run lifecycle tracked independently from Context attachment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RunState {
    /// No run can be queued because the engine is not attached.
    Inactive,
    /// The attached engine accepts a new queue request.
    Ready,
    /// Tests were queued but have not started pumping yet.
    Queued,
    /// At least one queued test is running.
    Running,
    /// The queue reached a terminal state and its summary awaits consumption.
    Terminal,
}

impl RunState {
    /// Returns whether a new queue request is valid.
    pub const fn accepts_queue(self) -> bool {
        matches!(self, Self::Ready)
    }
}
