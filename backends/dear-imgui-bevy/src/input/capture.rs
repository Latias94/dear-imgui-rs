//! Scoped capture bookkeeping shared by aggregate and per-route queries.

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
#[cfg(feature = "render")]
use dear_imgui_rs as imgui;
use dear_imgui_rs::ContextId;

use super::ImguiInputCaptureState;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CaptureScopes {
    contexts: HashMap<ContextId, ImguiInputCaptureState>,
    windows: HashMap<Entity, ImguiInputCaptureState>,
    context_windows: HashMap<ContextId, Vec<Entity>>,
    primary_context: Option<ContextId>,
}

impl CaptureScopes {
    pub(super) fn context(&self, context_id: ContextId) -> ImguiInputCaptureState {
        self.contexts.get(&context_id).copied().unwrap_or_default()
    }

    pub(super) fn for_context(&self, context_id: ContextId) -> Option<ImguiInputCaptureState> {
        self.contexts.get(&context_id).copied()
    }

    pub(super) fn window(&self, window: Entity) -> ImguiInputCaptureState {
        self.windows.get(&window).copied().unwrap_or_default()
    }

    pub(super) fn for_window(&self, window: Entity) -> Option<ImguiInputCaptureState> {
        self.windows.get(&window).copied()
    }

    pub(super) fn primary_context(&self) -> Option<ContextId> {
        self.primary_context
    }

    pub(super) fn update_primary(
        &mut self,
        context_id: ContextId,
        window: Entity,
        state: ImguiInputCaptureState,
    ) {
        self.primary_context = Some(context_id);
        self.contexts.clear();
        self.contexts.insert(context_id, state);
        self.windows.clear();
        self.windows.insert(window, state);
        self.context_windows.clear();
        self.context_windows.insert(context_id, vec![window]);
    }

    #[cfg(feature = "render")]
    pub(super) fn begin_routes(
        &mut self,
        primary_context: Option<ContextId>,
        routes: &[(ContextId, Entity)],
    ) -> ImguiInputCaptureState {
        self.primary_context = primary_context;
        self.context_windows.clear();
        for &(context_id, window) in routes {
            let windows = self.context_windows.entry(context_id).or_default();
            if !windows.contains(&window) {
                windows.push(window);
            }
        }
        self.contexts
            .retain(|context_id, _| self.context_windows.contains_key(context_id));
        self.rebuild()
    }

    #[cfg(feature = "render")]
    pub(super) fn update_context(
        &mut self,
        context_id: ContextId,
        io: &imgui::Io,
    ) -> Option<ImguiInputCaptureState> {
        if !self.context_windows.contains_key(&context_id) {
            return None;
        }
        self.contexts
            .insert(context_id, ImguiInputCaptureState::from_io(io));
        Some(self.rebuild())
    }

    #[cfg(feature = "render")]
    pub(super) fn remove_context(&mut self, context_id: ContextId) -> ImguiInputCaptureState {
        self.contexts.remove(&context_id);
        self.context_windows.remove(&context_id);
        self.rebuild()
    }

    #[cfg(feature = "render")]
    fn rebuild(&mut self) -> ImguiInputCaptureState {
        self.windows.clear();
        let mut aggregate = ImguiInputCaptureState::default();
        for (&context_id, windows) in &self.context_windows {
            let state = self.context(context_id);
            aggregate = aggregate.merge(state);
            for &window in windows {
                self.windows
                    .entry(window)
                    .and_modify(|window_state| *window_state = window_state.merge(state))
                    .or_insert(state);
            }
        }
        aggregate
    }
}
