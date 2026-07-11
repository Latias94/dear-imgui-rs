//! Explicit state owners for reflection rendering.
//!
//! [`ReflectSession`] owns settings and persistent map-popup drafts. An
//! [`Inspector`] borrows a session for one UI pass and owns that pass's
//! response and logical field path.

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::{ReflectEvent, ReflectResponse, ReflectSettings, imgui};

/// Extension methods for starting a reflection pass from a Dear ImGui [`imgui::Ui`].
///
/// Import this trait and call [`ImGuiReflectExt::inspector`] once per rendered
/// inspector. The [`ReflectSession`] remains the explicit owner of persistent
/// settings and map-popup drafts.
pub trait ImGuiReflectExt {
    /// Starts a reflection pass backed by `session`.
    ///
    /// The UI and session are borrowed independently for the lifetime of the
    /// returned [`Inspector`].
    fn inspector<'ui, 'session>(
        &'ui self,
        session: &'session ReflectSession,
    ) -> Inspector<'ui, 'session>;
}

impl ImGuiReflectExt for imgui::Ui {
    fn inspector<'ui, 'session>(
        &'ui self,
        session: &'session ReflectSession,
    ) -> Inspector<'ui, 'session> {
        session.inspector(self)
    }
}

/// Persistent state shared by reflection passes for one UI owner.
///
/// A session is intentionally UI-thread oriented. It owns settings and popup
/// drafts, but does not require the reflected values themselves to be `Send`.
#[derive(Default)]
pub struct ReflectSession {
    settings: ReflectSettings,
    map_drafts: RefCell<HashMap<MapDraftIdentity, StoredMapDraft>>,
    bound_context: Cell<Option<usize>>,
}

impl ReflectSession {
    /// Creates an empty session with default reflection settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the settings used by inspectors created from this session.
    pub fn settings(&self) -> &ReflectSettings {
        &self.settings
    }

    /// Returns mutable access to this session's reflection settings.
    ///
    /// Configure settings before creating an [`Inspector`]. The mutable borrow
    /// makes changing settings during a render pass impossible by construction.
    pub fn settings_mut(&mut self) -> &mut ReflectSettings {
        &mut self.settings
    }

    /// Starts a reflection pass for `ui`.
    ///
    /// The returned inspector owns the response and path state for this pass.
    /// Persistent settings and map drafts remain owned by this session. A
    /// session is normally owned beside one Dear ImGui context. Reusing it
    /// with a different live context clears old popup drafts automatically;
    /// when a context is destroyed and rebuilt, rebuild its session too.
    ///
    /// [`ImGuiReflectExt::inspector`] is the canonical per-frame entry point;
    /// this method is its equivalent session-oriented form.
    pub fn inspector<'ui, 'session>(
        &'session self,
        ui: &'ui imgui::Ui,
    ) -> Inspector<'ui, 'session> {
        let context = ui.with_bound_context(|| unsafe {
            // SAFETY: `with_bound_context` makes this Ui's owning context
            // current for the duration of the raw Dear ImGui query.
            imgui::sys::igGetCurrentContext() as usize
        });
        if self.bound_context.get() != Some(context) {
            self.map_drafts.borrow_mut().clear();
            self.bound_context.set(Some(context));
        }

        Inspector {
            ui,
            session: self,
            context,
            response: ReflectResponse::default(),
            path: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Removes every persistent map-popup draft retained by this session.
    pub fn clear_map_drafts(&self) {
        self.map_drafts.borrow_mut().clear();
    }
}

/// One reflection render pass.
///
/// An inspector makes response collection and path tracking explicit. It is
/// passed through [`crate::ImGuiValue`] and [`crate::ImGuiReflect`] so nested
/// renderers cannot rely on global or thread-local state.
pub struct Inspector<'ui, 'session> {
    ui: &'ui imgui::Ui,
    session: &'session ReflectSession,
    context: usize,
    response: ReflectResponse,
    path: Rc<RefCell<Vec<Cow<'static, str>>>>,
}

impl<'ui, 'session> Inspector<'ui, 'session> {
    /// Returns the Dear ImGui UI for this render pass.
    pub fn ui(&self) -> &'ui imgui::Ui {
        self.ui
    }

    /// Returns the immutable settings owned by this inspector's session.
    pub fn settings(&self) -> &'session ReflectSettings {
        self.session.settings()
    }

    /// Renders a reflected value and records structural events for this pass.
    pub fn input<T: crate::ImGuiReflect>(&mut self, label: &str, value: &mut T) -> bool {
        value.imgui_reflect(self, label)
    }

    /// Returns the response collected so far during this render pass.
    pub fn response(&self) -> &ReflectResponse {
        &self.response
    }

    /// Returns mutable access to the response collected during this render pass.
    pub fn response_mut(&mut self) -> &mut ReflectResponse {
        &mut self.response
    }

    /// Consumes the inspector and returns its collected response.
    pub fn into_response(self) -> ReflectResponse {
        self.response
    }

    /// Returns the current logical field path, if nested rendering has entered one.
    pub fn current_path(&self) -> Option<String> {
        let path = self.path.borrow();
        if path.is_empty() {
            return None;
        }

        let mut result = String::new();
        for (index, segment) in path.iter().enumerate() {
            if index > 0 && !segment.starts_with('[') {
                result.push('.');
            }
            result.push_str(segment);
        }
        Some(result)
    }

    pub(crate) fn record_event(&mut self, event: ReflectEvent) {
        self.response.push(event);
    }

    pub(crate) fn is_path_active(&self) -> bool {
        !self.path.borrow().is_empty()
    }

    /// Enters a dynamic path segment until the returned guard is dropped.
    ///
    /// This is public for hand-written implementations that need the same
    /// structural event paths as derive-generated implementations.
    #[doc(hidden)]
    pub fn push_path(&self, segment: impl Into<String>) -> InspectorPathGuard {
        self.push_path_inner(Cow::Owned(segment.into()))
    }

    /// Enters a static path segment until the returned guard is dropped.
    ///
    /// This is used by derive-generated implementations to avoid an
    /// intermediate formatting closure. The guard owns its path state, so a
    /// caller can continue mutably using this inspector while it is alive.
    #[doc(hidden)]
    pub fn push_path_static(&self, segment: &'static str) -> InspectorPathGuard {
        self.push_path_inner(Cow::Borrowed(segment))
    }

    fn push_path_inner(&self, segment: Cow<'static, str>) -> InspectorPathGuard {
        let depth = {
            let mut path = self.path.borrow_mut();
            let depth = path.len();
            path.push(segment);
            depth
        };
        InspectorPathGuard {
            path: Rc::clone(&self.path),
            depth,
        }
    }

    pub(crate) fn with_map_draft<V, R>(
        &mut self,
        identity: MapDraftIdentity,
        initial_key: impl FnOnce() -> String,
        render: impl FnOnce(&mut Self, &mut String, &mut V) -> R,
    ) -> R
    where
        V: Default + 'static,
    {
        debug_assert_eq!(identity.value_type, TypeId::of::<V>());
        let stored = self.session.map_drafts.borrow_mut().remove(&identity);
        let draft = match stored {
            Some(stored) => MapDraft {
                key: stored.key,
                value: stored
                    .value
                    .downcast::<V>()
                    .map(|value| *value)
                    .unwrap_or_default(),
            },
            None => MapDraft {
                key: initial_key(),
                value: V::default(),
            },
        };

        let mut restore = MapDraftRestore {
            drafts: &self.session.map_drafts,
            identity,
            draft: Some(draft),
        };
        let draft = restore
            .draft
            .as_mut()
            .expect("MapDraftRestore always contains a draft while rendering");
        render(self, &mut draft.key, &mut draft.value)
    }

    pub(crate) fn clear_map_draft(&self, identity: MapDraftIdentity) {
        self.session.map_drafts.borrow_mut().remove(&identity);
    }

    pub(crate) fn map_draft_identity<V: 'static>(&self, popup_label: &str) -> MapDraftIdentity {
        MapDraftIdentity {
            // Pair the actual ImGuiContext identity with an ID calculated in
            // the active stack, preventing cross-context and cross-scope reuse.
            imgui_context: self.context,
            popup_id: self.ui.get_id(popup_label).raw(),
            value_type: TypeId::of::<V>(),
        }
    }
}

/// A scoped path entry returned by [`Inspector::push_path`].
///
/// Dropping this guard restores the prior path, including during panic unwind.
#[doc(hidden)]
#[must_use = "dropping the guard immediately restores the inspector path"]
pub struct InspectorPathGuard {
    path: Rc<RefCell<Vec<Cow<'static, str>>>>,
    depth: usize,
}

impl Drop for InspectorPathGuard {
    fn drop(&mut self) {
        let mut path = self.path.borrow_mut();
        debug_assert!(
            path.len() > self.depth,
            "path guards must drop in LIFO order"
        );
        path.truncate(self.depth);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct MapDraftIdentity {
    imgui_context: usize,
    popup_id: imgui::sys::ImGuiID,
    value_type: TypeId,
}

struct StoredMapDraft {
    key: String,
    value: Box<dyn Any>,
}

struct MapDraft<V> {
    key: String,
    value: V,
}

struct MapDraftRestore<'a, V: 'static> {
    drafts: &'a RefCell<HashMap<MapDraftIdentity, StoredMapDraft>>,
    identity: MapDraftIdentity,
    draft: Option<MapDraft<V>>,
}

impl<V: 'static> Drop for MapDraftRestore<'_, V> {
    fn drop(&mut self) {
        let Some(draft) = self.draft.take() else {
            return;
        };
        self.drafts.borrow_mut().insert(
            self.identity,
            StoredMapDraft {
                key: draft.key,
                value: Box::new(draft.value),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imgui::{Context, SuspendedContext};
    use crate::test_guard;

    fn test_ui_context() -> Context {
        let mut context = Context::create();
        {
            let io = context.io_mut();
            io.set_display_size([640.0, 480.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = context.font_atlas_mut().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        context
    }

    #[test]
    fn path_guards_restore_after_nested_panic() {
        let _guard = test_guard();
        let mut context = test_ui_context();
        let session = ReflectSession::new();
        let ui = context.frame();
        let inspector = session.inspector(ui);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _outer = inspector.push_path_static("outer");
            let _inner = inspector.push_path("[0]");
            assert_eq!(inspector.current_path().as_deref(), Some("outer[0]"));
            panic!("path unwind probe");
        }));

        assert!(result.is_err());
        assert!(inspector.current_path().is_none());
    }

    #[test]
    fn sessions_isolate_responses_and_map_drafts() {
        let _guard = test_guard();
        let mut context = test_ui_context();
        let left_session = ReflectSession::new();
        let right_session = ReflectSession::new();
        let ui = context.frame();

        let mut left = left_session.inspector(ui);
        left.record_event(ReflectEvent::VecInserted {
            path: Some("left".to_owned()),
            index: 1,
        });
        let left_identity = left.map_draft_identity::<i32>("shared-popup");
        left.with_map_draft::<i32, _>(
            left_identity,
            || "left-key".to_owned(),
            |_, key, value| {
                *key = "left-key".to_owned();
                *value = 41;
            },
        );

        let mut right = right_session.inspector(ui);
        assert!(right.response().is_empty());
        let right_identity = right.map_draft_identity::<i32>("shared-popup");
        right.with_map_draft::<i32, _>(
            right_identity,
            || "right-key".to_owned(),
            |_, key, value| {
                assert_eq!(key, "right-key");
                assert_eq!(*value, 0);
                *value = 73;
            },
        );

        left.with_map_draft::<i32, _>(
            left_identity,
            || "missing".to_owned(),
            |_, key, value| {
                assert_eq!(key, "left-key");
                assert_eq!(*value, 41);
            },
        );

        assert_eq!(left.response().events().len(), 1);
    }

    #[test]
    fn map_draft_identity_includes_active_imgui_id_scope() {
        let _guard = test_guard();
        let mut context = test_ui_context();
        let session = ReflectSession::new();
        let ui = context.frame();
        let mut inspector = session.inspector(ui);

        {
            let _scope = ui.push_id("left-scope");
            let identity = inspector.map_draft_identity::<i32>("same-popup");
            inspector.with_map_draft::<i32, _>(
                identity,
                || "left".to_owned(),
                |_, key, value| {
                    *key = "left".to_owned();
                    *value = 7;
                },
            );
        }

        {
            let _scope = ui.push_id("right-scope");
            let identity = inspector.map_draft_identity::<i32>("same-popup");
            inspector.with_map_draft::<i32, _>(
                identity,
                || "right".to_owned(),
                |_, key, value| {
                    assert_eq!(key, "right");
                    assert_eq!(*value, 0);
                },
            );
        }
    }

    #[test]
    fn switching_real_imgui_contexts_clears_old_drafts() {
        let _guard = test_guard();
        let session = ReflectSession::new();
        let mut first = test_ui_context();
        {
            let ui = first.frame();
            let mut inspector = session.inspector(ui);
            let identity = inspector.map_draft_identity::<i32>("same-popup");
            inspector.with_map_draft::<i32, _>(
                identity,
                || "first".to_owned(),
                |_, key, value| {
                    *key = "first".to_owned();
                    *value = 11;
                },
            );
        }
        first.render();
        let first: SuspendedContext = first.suspend();

        let mut second = test_ui_context();
        {
            let ui = second.frame();
            let mut inspector = session.inspector(ui);
            let identity = inspector.map_draft_identity::<i32>("same-popup");
            inspector.with_map_draft::<i32, _>(
                identity,
                || "second".to_owned(),
                |_, key, value| {
                    assert_eq!(key, "second");
                    assert_eq!(*value, 0);
                },
            );
        }
        second.render();
        drop(second);
        drop(first);
    }

    #[test]
    fn nested_map_draft_restores_after_panic() {
        let _guard = test_guard();
        let mut context = test_ui_context();
        let session = ReflectSession::new();
        let ui = context.frame();
        let mut inspector = session.inspector(ui);
        let outer_identity = inspector.map_draft_identity::<Vec<i32>>("outer-popup");
        let inner_identity = inspector.map_draft_identity::<String>("inner-popup");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inspector.with_map_draft::<Vec<i32>, _>(
                outer_identity,
                || "outer".to_owned(),
                |inspector, key, value| {
                    *key = "restored".to_owned();
                    value.push(5);
                    inspector.with_map_draft::<String, _>(
                        inner_identity,
                        || "inner".to_owned(),
                        |_, _, nested| nested.push_str("nested"),
                    );
                    panic!("draft unwind probe");
                },
            );
        }));
        assert!(result.is_err());

        inspector.with_map_draft::<Vec<i32>, _>(
            outer_identity,
            || "missing".to_owned(),
            |_, key, value| {
                assert_eq!(key, "restored");
                assert_eq!(value, &[5]);
            },
        );
        inspector.with_map_draft::<String, _>(
            inner_identity,
            || "missing".to_owned(),
            |_, key, value| {
                assert_eq!(key, "inner");
                assert_eq!(value.as_str(), "nested");
            },
        );
    }

    #[test]
    fn map_draft_uses_owner_identity_inside_popup_window() {
        let _guard = test_guard();
        let mut context = test_ui_context();
        let session = ReflectSession::new();
        let ui = context.frame();
        let mut inspector = session.inspector(ui);

        ui.window("draft-owner").build(|| {
            let popup_label = "draft-popup";
            let owner_identity = inspector.map_draft_identity::<i32>(popup_label);
            inspector.with_map_draft::<i32, _>(
                owner_identity,
                || "initial".to_owned(),
                |_, key, value| {
                    *key = "retained".to_owned();
                    *value = 19;
                },
            );

            ui.open_popup(popup_label);
            let _popup = ui
                .begin_popup(popup_label)
                .expect("programmatically opened popup should render");
            let popup_window_identity = inspector.map_draft_identity::<i32>(popup_label);
            assert_ne!(owner_identity.popup_id, popup_window_identity.popup_id);

            inspector.with_map_draft::<i32, _>(
                owner_identity,
                || "missing".to_owned(),
                |_, key, value| {
                    assert_eq!(key, "retained");
                    assert_eq!(*value, 19);
                },
            );
        });
    }
}
