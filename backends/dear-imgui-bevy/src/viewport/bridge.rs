use super::*;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridge {
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn commands(&self) -> Vec<ImguiViewportCommand> {
        self.inner
            .state
            .borrow()
            .commands
            .iter()
            .map(|queued| queued.command.clone())
            .collect()
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn queue(&mut self, command: ImguiViewportCommand) {
        let context_id = self
            .inner
            .context_id
            .get()
            .expect("the test viewport bridge must have a Context before queueing commands");
        self.inner
            .state
            .borrow_mut()
            .queue_for_test(context_id, command);
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn queue_for_context(
        &self,
        context_id: imgui::ContextId,
        command: ImguiViewportCommand,
    ) -> bool {
        let Some(context) = self.context(context_id) else {
            return false;
        };
        context
            .inner
            .state
            .borrow_mut()
            .queue_for_test(context_id, command);
        true
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn drain_commands(
        &mut self,
    ) -> Result<Vec<ImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self
            .inner
            .state
            .borrow_mut()
            .commands
            .drain(..)
            .map(|queued| queued.command)
            .collect())
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn callback_error(&self) -> Option<ImguiViewportRuntimeError> {
        self.inner.callback_fault.get()
    }

    /// Return the Bevy window currently mapped to one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context. Callers must retain the
    /// `ContextId` that created the viewport rather than assuming numeric IDs are process-wide.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn viewport_window(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<Entity> {
        self.context(context_id)
            .and_then(|context| context.viewport_window(viewport_id))
    }

    pub(crate) fn viewport_for_window(
        &self,
        context_id: imgui::ContextId,
        entity: Entity,
    ) -> Option<ImguiViewportId> {
        self.context(context_id)
            .and_then(|context| context.viewport_for_window(entity))
    }

    pub(crate) fn viewport_desktop_origin_for_window(
        &self,
        context_id: imgui::ContextId,
        entity: Entity,
    ) -> Option<[f32; 2]> {
        let context = self.context(context_id)?;
        let viewport_id = context.viewport_for_window(entity)?;
        context
            .viewport_feedback(viewport_id)
            .map(|feedback| feedback.pos)
    }

    /// Return the Bevy camera currently mapped to one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context.
    #[cfg(all(
        test,
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    #[must_use]
    pub fn viewport_camera(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<Entity> {
        self.context(context_id)
            .and_then(|context| context.viewport_camera(viewport_id))
    }

    /// Return the latest Bevy-observed state for one native viewport.
    ///
    /// Viewport identifiers are scoped to a Dear ImGui Context.
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn viewport_feedback(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportFeedback> {
        self.context(context_id)
            .and_then(|context| context.viewport_feedback(viewport_id))
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn set_viewport_feedback_for_test(
        &self,
        context_id: imgui::ContextId,
        viewport_id: ImguiViewportId,
        feedback: ImguiViewportFeedback,
    ) {
        let context = self
            .context(context_id)
            .expect("the test viewport Context must remain registered");
        let instance_id = context
            .instance_for_id(viewport_id)
            .expect("the test viewport route must remain registered");
        context.set_viewport_feedback(instance_id, feedback);
    }

    /// Returns a deferred callback failure from the native callback boundary.
    ///
    /// Reading the error does not clear it. The failure remains sticky until the viewport bridge is
    /// torn down and rebuilt, so callers cannot accidentally resume from a partially observed
    /// callback sequence.
    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn callback_error_for(
        &self,
        context_id: imgui::ContextId,
    ) -> Option<ImguiViewportRuntimeError> {
        self.context(context_id)
            .and_then(|context| context.callback_error())
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    pub(super) fn clear_viewport_state(&mut self) {
        self.inner.clear_viewport_state();
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    pub(super) fn keepalive(&self) -> ImguiViewportBridgeKeepalive {
        Rc::clone(&self.inner)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    pub(super) fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    pub(super) fn register_context(
        &mut self,
        context_id: imgui::ContextId,
        keepalive: ImguiViewportBridgeKeepalive,
    ) {
        keepalive.set_context_id(context_id);
        let mut contexts = self.contexts.borrow_mut();
        match contexts.entry(context_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(keepalive);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!("a Dear ImGui Context cannot register two viewport bridge allocations");
            }
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[cfg(test)]
    pub(super) fn set_viewport_window(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        let context_id = self
            .inner
            .context_id
            .get()
            .expect("the test viewport bridge must have a Context before mapping windows");
        let mut state = self.inner.state.borrow_mut();
        let instance_id = state.instance_for_id(viewport_id).unwrap_or_else(|| {
            state
                .register_viewport(
                    context_id,
                    ImguiViewportIdentity {
                        context_address: 0,
                        address: viewport_id.raw() as usize + 1,
                    },
                    viewport_id,
                )
                .expect("a synthetic test viewport route should be registerable")
        });
        state
            .record_mut(instance_id)
            .expect("the synthetic viewport record should exist")
            .window = Some(entity);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn registration(&self) -> ImguiViewportBridgeRegistration {
        ImguiViewportBridgeRegistration {
            contexts: Rc::clone(&self.contexts),
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn context(
        &self,
        context_id: imgui::ContextId,
    ) -> Option<ImguiViewportBridgeContext> {
        self.contexts
            .borrow()
            .get(&context_id)
            .cloned()
            .map(|inner| ImguiViewportBridgeContext { context_id, inner })
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn contexts(&self) -> Vec<ImguiViewportBridgeContext> {
        let mut contexts = self
            .contexts
            .borrow()
            .iter()
            .map(|(&context_id, inner)| ImguiViewportBridgeContext {
                context_id,
                inner: Rc::clone(inner),
            })
            .collect::<Vec<_>>();
        contexts.sort_by_key(|context| context.context_id.get().get());
        contexts
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeRegistration {
    pub(crate) fn register_context(
        &self,
        context_id: imgui::ContextId,
        keepalive: ImguiViewportBridgeKeepalive,
    ) {
        keepalive.set_context_id(context_id);
        let mut contexts = self.contexts.borrow_mut();
        match contexts.entry(context_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(keepalive);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!("a Dear ImGui Context cannot register two viewport bridge allocations");
            }
        }
    }

    pub(crate) fn unregister_context(
        &self,
        context_id: imgui::ContextId,
        owner: &ImguiViewportBridgeKeepalive,
    ) {
        let mut contexts = self.contexts.borrow_mut();
        let is_current_owner = contexts
            .get(&context_id)
            .is_some_and(|registered| Rc::ptr_eq(registered, owner));
        if is_current_owner {
            contexts.remove(&context_id);
        }
    }
}

/// A Context-qualified view of the native viewport bridge.
///
/// The ECS bridge is global because Bevy resources are global, but all mutable platform state is
/// owned by this per-Context handle. Keeping the Context id beside the keepalive makes it
/// impossible for a viewport command to accidentally resolve another Context's numeric id.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone)]
pub(crate) struct ImguiViewportBridgeContext {
    pub(crate) context_id: imgui::ContextId,
    pub(crate) inner: ImguiViewportBridgeKeepalive,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeContext {
    pub(super) fn drain_commands(
        &self,
    ) -> Result<Vec<QueuedImguiViewportCommand>, ImguiViewportRuntimeError> {
        if let Some(error) = self.inner.callback_fault.get() {
            return Err(error);
        }
        Ok(self.inner.state.borrow_mut().commands.drain(..).collect())
    }

    pub(super) fn pending_create_instances(&self) -> HashSet<ImguiViewportInstanceId> {
        self.inner
            .state
            .borrow()
            .commands
            .iter()
            .filter_map(|queued| {
                matches!(&queued.command, ImguiViewportCommand::Create(_))
                    .then_some(queued.instance_id)
            })
            .collect()
    }

    pub(crate) fn ecs_release_pending(&self) -> bool {
        self.inner.ecs_release_pending()
    }

    #[cfg(test)]
    pub(super) fn callback_error(&self) -> Option<ImguiViewportRuntimeError> {
        self.inner.callback_fault.get()
    }

    pub(super) fn instance_for_id(
        &self,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportInstanceId> {
        self.inner.state.borrow().instance_for_id(viewport_id)
    }

    pub(crate) fn viewport_id(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportId> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .map(|record| record.current_id)
    }

    pub(super) fn set_viewport_window(&self, instance_id: ImguiViewportInstanceId, entity: Entity) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.window = Some(entity);
        }
    }

    pub(crate) fn viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        let state = self.inner.state.borrow();
        state
            .instance_for_id(viewport_id)
            .and_then(|instance_id| state.record(instance_id))
            .and_then(|record| record.window)
    }

    pub(crate) fn viewport_window_for_instance(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.window)
    }

    pub(crate) fn viewport_for_window(&self, entity: Entity) -> Option<ImguiViewportId> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .find_map(|record| (record.window == Some(entity)).then_some(record.current_id))
    }

    pub(super) fn remove_viewport_window(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .record_mut(instance_id)
            .and_then(|record| record.window.take())
    }

    #[cfg(feature = "render")]
    pub(super) fn set_viewport_camera(&self, instance_id: ImguiViewportInstanceId, entity: Entity) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.camera = Some(entity);
        }
    }

    #[cfg(all(test, feature = "render"))]
    pub(super) fn viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        let state = self.inner.state.borrow();
        state
            .instance_for_id(viewport_id)
            .and_then(|instance_id| state.record(instance_id))
            .and_then(|record| record.camera)
    }

    #[cfg(feature = "render")]
    pub(super) fn viewport_camera_for_instance(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Entity> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.camera)
    }

    #[cfg(feature = "render")]
    pub(super) fn remove_viewport_camera(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Entity> {
        self.inner
            .state
            .borrow_mut()
            .record_mut(instance_id)
            .and_then(|record| record.camera.take())
    }

    pub(crate) fn viewport_feedback(
        &self,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportFeedback> {
        let instance_id = self.instance_for_id(viewport_id)?;
        self.viewport_feedback_for_instance(instance_id)
    }

    pub(crate) fn viewport_feedback_for_instance(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportFeedback> {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.feedback)
    }

    pub(super) fn set_viewport_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.feedback = Some(feedback);
        }
    }

    pub(super) fn observe_viewport_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) -> geometry::ViewportGeometryReconciliation {
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return geometry::ViewportGeometryReconciliation::default();
        };
        let previous = record.feedback.unwrap_or(feedback);
        let geometry = std::mem::take(&mut record.geometry);
        let reconciliation = geometry.reconcile(previous, feedback);
        record.feedback = Some(feedback);
        reconciliation
    }

    pub(super) fn record_position_request(
        &self,
        instance_id: ImguiViewportInstanceId,
        pos: [f32; 2],
        dpi_scale: f32,
    ) {
        let pos = finite_desktop_pos(pos);
        let dpi_scale = positive_finite_or(dpi_scale, 1.0);
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return;
        };
        if let Some(placement) = record.pending_client_placement.as_mut() {
            placement.pos = pos;
            placement.dpi_scale = dpi_scale;
            record.geometry.clear_position();
            if record.geometry.is_empty() {
                record.geometry = geometry::ViewportGeometryReconciler::default();
            }
            return;
        }
        record.geometry.record_position(pos, dpi_scale);
    }

    pub(super) fn record_size_request(
        &self,
        instance_id: ImguiViewportInstanceId,
        size: [f32; 2],
        dpi_scale: f32,
    ) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record
                .geometry
                .record_size(finite_desktop_size(size), dpi_scale);
        }
    }

    pub(super) fn remove_viewport_feedback(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.feedback = None;
            record.geometry = geometry::ViewportGeometryReconciler::default();
        }
    }

    pub(super) fn client_placement_is_pending(&self, instance_id: ImguiViewportInstanceId) -> bool {
        self.inner
            .state
            .borrow()
            .record(instance_id)
            .is_some_and(|record| record.pending_client_placement.is_some())
    }

    pub(super) fn remove_pending_client_placement(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.pending_client_placement = None;
        }
    }

    pub(super) fn refresh_viewport_non_geometry_feedback(
        &self,
        instance_id: ImguiViewportInstanceId,
        feedback: ImguiViewportFeedback,
    ) {
        let mut state = self.inner.state.borrow_mut();
        let Some(record) = state.record_mut(instance_id) else {
            return;
        };
        if let Some(cached) = record.feedback.as_mut() {
            let pos = cached.pos;
            let size = cached.size;
            *cached = ImguiViewportFeedback {
                pos,
                size,
                ..feedback
            };
        } else {
            record.feedback = Some(feedback);
        }
    }

    pub(super) fn remove_viewport_flags(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.flags = None;
        }
    }

    pub(super) fn show_should_focus(&self, instance_id: ImguiViewportInstanceId) -> bool {
        !self
            .inner
            .state
            .borrow()
            .record(instance_id)
            .and_then(|record| record.flags)
            .is_some_and(|flags| flags.contains(imgui::ViewportFlags::NO_FOCUS_ON_APPEARING))
    }

    pub(super) fn request_focus_next_frame(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.focus_next_frame = true;
        }
    }

    pub(super) fn clear_focus_request(&self, instance_id: ImguiViewportInstanceId) {
        if let Some(record) = self.inner.state.borrow_mut().record_mut(instance_id) {
            record.focus_next_frame = false;
            record.focus_ready = false;
        }
    }

    pub(super) fn take_all_ecs_entities_for_release(&self) -> HashSet<Entity> {
        self.inner.take_all_ecs_entities_for_release()
    }

    pub(super) fn pending_ecs_despawns(&self) -> HashSet<Entity> {
        self.inner.pending_ecs_despawns()
    }

    pub(super) fn mapped_ecs_entities(&self) -> HashSet<Entity> {
        let state = self.inner.state.borrow();
        state
            .viewports
            .values()
            .flat_map(|record| record.window.into_iter().chain(record.camera))
            .collect()
    }

    pub(super) fn mapped_window_entities(&self) -> HashSet<Entity> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .filter_map(|record| record.window)
            .collect()
    }

    #[cfg(feature = "render")]
    pub(super) fn mapped_camera_entities(&self) -> HashSet<Entity> {
        self.inner
            .state
            .borrow()
            .viewports
            .values()
            .filter_map(|record| record.camera)
            .collect()
    }

    pub(super) fn track_ecs_despawn(&self, entity: Entity) {
        self.inner.track_ecs_despawn(entity);
    }

    pub(super) fn track_ecs_despawns(&self, entities: impl IntoIterator<Item = Entity>) {
        self.inner.track_ecs_despawns(entities);
    }

    pub(super) fn acknowledge_ecs_despawns(&self, mut entity_is_live: impl FnMut(Entity) -> bool) {
        self.inner.acknowledge_ecs_despawns(&mut entity_is_live);
    }
}
