use super::validation::assert_finite_vec2;
use crate::{EditorContext, PinId, sys, vec2};
use dear_imgui_rs::Ui;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    ffi::CString,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BuildPhase {
    Idle,
    Node,
    Pin,
    Group,
    GroupHint,
    Create,
    Delete,
    Shortcut,
}

/// RAII token for an active node-editor frame.
pub struct NodeEditorFrame<'ui> {
    pub(super) _ui: &'ui Ui,
    pub(super) _editor: &'ui EditorContext,
    pub(super) suspended: Cell<bool>,
    pub(super) phase: Cell<BuildPhase>,
    pub(super) submitted_pins: RefCell<HashSet<PinId>>,
    pub(super) ended: bool,
}

impl<'ui> NodeEditorFrame<'ui> {
    pub(crate) fn new(
        ui: &'ui Ui,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> Self {
        assert_finite_vec2("Ui::node_editor()", "size", size);
        assert_eq!(
            ui.context_id(),
            editor.imgui_context_id(),
            "Ui::node_editor() requires a Ui from the editor's owning Dear ImGui context"
        );
        let id = CString::new(id.as_ref()).expect("node editor id cannot contain NUL bytes");
        assert!(
            !editor.frame_active.get(),
            "Ui::node_editor() requires the previous frame for this editor to be ended"
        );
        editor.with_current("Ui::node_editor", || unsafe {
            sys::dne_begin(id.as_ptr(), vec2(size))
        });
        editor.frame_active.set(true);
        Self {
            _ui: ui,
            _editor: editor,
            suspended: Cell::new(false),
            phase: Cell::new(BuildPhase::Idle),
            submitted_pins: RefCell::new(HashSet::new()),
            ended: false,
        }
    }

    #[inline]
    pub(super) fn with_current<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self._editor.with_current(caller, f)
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            self._editor
                .with_current("NodeEditorFrame::end()", || unsafe { sys::dne_end() });
            self._editor.frame_active.set(false);
            self.ended = true;
        }
    }
}

impl Drop for NodeEditorFrame<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}
