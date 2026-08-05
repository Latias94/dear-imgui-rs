//! Deferred Context teardown after renderer or viewport release acknowledgements.

use std::{
    cell::RefCell,
    collections::VecDeque,
    mem::ManuallyDrop,
    rc::{Rc, Weak},
};

use bevy_app::App;
use bevy_ecs::prelude::World;

use super::ImguiContexts;
use super::backend_contract::ImguiContextRemovalPendingReason;
use super::owner::ContextOwner;

struct ImguiContextRetirementQueue {
    pending: RefCell<VecDeque<ContextRetirement>>,
    #[cfg(feature = "render")]
    snapshot_mailbox: RefCell<Option<super::ImguiFrameMailbox>>,
}

impl Default for ImguiContextRetirementQueue {
    fn default() -> Self {
        Self {
            pending: RefCell::new(VecDeque::new()),
            #[cfg(feature = "render")]
            snapshot_mailbox: RefCell::new(None),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ImguiContextRetirementSink {
    queue: Weak<ImguiContextRetirementQueue>,
}

impl Default for ImguiContextRetirementSink {
    fn default() -> Self {
        Self { queue: Weak::new() }
    }
}

impl ImguiContextRetirementSink {
    fn enqueue_or_leak(&self, owner: ManuallyDrop<ContextOwner>) {
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        let Ok(mut pending) = queue.pending.try_borrow_mut() else {
            return;
        };
        pending.push_back(ContextRetirement {
            owner: Some(owner),
            sink: self.clone(),
        });
    }

    fn try_pop_front(&self) -> Option<ContextRetirement> {
        let queue = self.queue.upgrade()?;
        let mut pending = queue.pending.try_borrow_mut().ok()?;
        pending.pop_front()
    }

    pub(super) fn pending_len(&self) -> usize {
        let Some(queue) = self.queue.upgrade() else {
            return 0;
        };
        queue
            .pending
            .try_borrow()
            .map_or(0, |pending| pending.len())
    }

    #[cfg(feature = "render")]
    fn set_snapshot_mailbox(&self, mailbox: super::ImguiFrameMailbox) {
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        if let Ok(mut installed) = queue.snapshot_mailbox.try_borrow_mut() {
            *installed = Some(mailbox);
        }
    }

    #[cfg(feature = "render")]
    pub(super) fn snapshot_mailbox(&self) -> Option<super::ImguiFrameMailbox> {
        let queue = self.queue.upgrade()?;
        queue.snapshot_mailbox.try_borrow().ok()?.clone()
    }
}

pub(crate) struct ImguiContextRetirements {
    queue: Rc<ImguiContextRetirementQueue>,
}

impl Default for ImguiContextRetirements {
    fn default() -> Self {
        Self {
            queue: Rc::new(ImguiContextRetirementQueue::default()),
        }
    }
}

impl ImguiContextRetirements {
    pub(crate) fn sink(&self) -> ImguiContextRetirementSink {
        ImguiContextRetirementSink {
            queue: Rc::downgrade(&self.queue),
        }
    }

    pub(crate) fn pending_len(&self) -> usize {
        self.sink().pending_len()
    }
}

pub(super) struct ContextRetirement {
    owner: Option<ManuallyDrop<ContextOwner>>,
    sink: ImguiContextRetirementSink,
}

pub(crate) fn install_context_retirements(app: &mut App) {
    if app
        .world()
        .get_non_send::<ImguiContextRetirements>()
        .is_none()
    {
        app.insert_non_send(ImguiContextRetirements::default());
    }
    let sink = app
        .world()
        .get_non_send::<ImguiContextRetirements>()
        .expect("Context retirement storage must be installed")
        .sink();
    #[cfg(feature = "render")]
    sink.set_snapshot_mailbox(app.world().resource::<super::ImguiFrameMailbox>().clone());
    if let Some(mut contexts) = app.world_mut().get_non_send_mut::<ImguiContexts>() {
        contexts.set_retirement_sink(sink);
    }
}

fn maintain_context_retirements(world: &mut World) {
    let Some(sink) = world
        .get_non_send::<ImguiContextRetirements>()
        .map(ImguiContextRetirements::sink)
    else {
        return;
    };
    let pending_at_start = sink.pending_len();
    for _ in 0..pending_at_start {
        let Some(mut retirement) = sink.try_pop_front() else {
            break;
        };
        if retirement.advance().is_ok() {
            retirement.finish();
        }
    }
}

pub(crate) fn begin_context_retirements(world: &mut World) {
    maintain_context_retirements(world);
}

pub(crate) fn finish_context_retirements(world: &mut World) {
    maintain_context_retirements(world);
}
impl ContextRetirement {
    pub(super) fn new(owner: ContextOwner, sink: ImguiContextRetirementSink) -> Self {
        Self {
            owner: Some(ManuallyDrop::new(owner)),
            sink,
        }
    }

    fn advance(&mut self) -> Result<(), ImguiContextRemovalPendingReason> {
        self.owner
            .as_deref_mut()
            .expect("a pending Context retirement must retain its owner")
            .try_detach_backend()
    }

    fn finish(mut self) {
        let owner = self
            .owner
            .take()
            .expect("a completed Context retirement must retain its owner");
        let mut owner = ManuallyDrop::into_inner(owner);
        let context = owner
            .context
            .take()
            .expect("a completed Context retirement must retain its Context");
        drop(owner);
        drop(context);
    }
}

impl Drop for ContextRetirement {
    fn drop(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        // A failed enqueue intentionally leaks the complete owner. Releasing only part of it
        // would invalidate renderer or PlatformIO pointers still owned by another Bevy world.
        self.sink.enqueue_or_leak(owner);
    }
}

#[cfg(test)]
mod retirement_tests {
    use std::{cell::Cell, rc::Rc};

    use bevy_app::App;

    use super::ImguiContextRetirements;
    use crate::context::ImguiContextConfig;
    use crate::context::backend_contract::BackendAttachment;
    use crate::context::owner::ContextOwner;
    #[cfg(feature = "render")]
    use crate::render;
    use crate::test_util::imgui_context_guard as context_guard;

    struct RetirementProbeMarker;

    struct RetirementProbe {
        destroyed: Rc<Cell<bool>>,
    }

    impl dear_imgui_rs::ContextAttachment for RetirementProbe {
        fn context_destroyed(&self, _context: dear_imgui_rs::ContextDestroyed) {
            self.destroyed.set(true);
        }
    }

    fn context_with_retirement_probe(destroyed: &Rc<Cell<bool>>) -> dear_imgui_rs::Context {
        let mut context = dear_imgui_rs::Context::create();
        context
            .register_attachment::<RetirementProbeMarker>(
                dear_imgui_rs::ContextAttachmentRole::Extension,
                Rc::new(RetirementProbe {
                    destroyed: Rc::clone(destroyed),
                }),
            )
            .unwrap()
            .defer_to_context();
        context
    }

    fn headless_backend_attachment() -> BackendAttachment {
        BackendAttachment {
            render_integration_installed: false,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge_registration: None,
            #[cfg(feature = "render")]
            renderer_releases: None,
        }
    }

    fn primary_config() -> ImguiContextConfig {
        let mut app = App::new();
        let pass = crate::context::pass::primary_pass(&mut app);
        ImguiContextConfig::primary(&pass)
    }

    #[test]
    fn drop_releases_a_synchronously_detachable_context_after_retirement_sink_vanishes() {
        let _guard = context_guard();
        let destroyed = Rc::new(Cell::new(false));
        let context = context_with_retirement_probe(&destroyed);

        let retirements = ImguiContextRetirements::default();
        let mut owner = ContextOwner::new(context.suspend());
        owner
            .attach_backend(&headless_backend_attachment(), &primary_config())
            .unwrap();
        assert!(!owner.is_unattached());
        owner.set_retirement_sink(retirements.sink());
        drop(retirements);
        drop(owner);

        assert!(
            destroyed.get(),
            "a synchronously detachable Context must not depend on its retirement queue"
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn vanished_retirement_sink_leaks_renderer_ownership_awaiting_acknowledgement() {
        let _guard = context_guard();
        let destroyed = Rc::new(Cell::new(false));
        let context = context_with_retirement_probe(&destroyed);

        let retirements = ImguiContextRetirements::default();
        let mut owner = ContextOwner::new(context.suspend());
        owner
            .attach_backend(
                &BackendAttachment {
                    render_integration_installed: true,
                    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                    viewport_bridge_registration: None,
                    renderer_releases: Some(render::ImguiRendererReleases::default()),
                },
                &primary_config(),
            )
            .unwrap();
        owner.set_retirement_sink(retirements.sink());
        drop(retirements);
        drop(owner);

        assert!(
            !destroyed.get(),
            "a vanished sink must leak ownership awaiting render-world acknowledgement"
        );
    }
}
