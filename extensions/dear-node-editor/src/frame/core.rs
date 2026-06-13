use super::validation::assert_finite_vec2;
use crate::{EditorContext, context::CurrentEditorGuard, sys, vec2};
use dear_imgui_rs::Ui;
use std::{cell::Cell, ffi::CString};

/// RAII token for an active node-editor frame.
pub struct NodeEditorFrame<'ui> {
    pub(super) _ui: &'ui Ui,
    pub(super) _editor: &'ui EditorContext,
    pub(super) suspended: Cell<bool>,
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
        let id = CString::new(id.as_ref()).expect("node editor id cannot contain NUL bytes");
        {
            let _current_editor = editor.bind_current("Ui::node_editor");
            unsafe { sys::dne_begin(id.as_ptr(), vec2(size)) };
        }
        Self {
            _ui: ui,
            _editor: editor,
            suspended: Cell::new(false),
            ended: false,
        }
    }

    pub(super) fn bind(&self, caller: &str) -> CurrentEditorGuard<'_> {
        self._editor.bind_current(caller)
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            let _current_editor = self._editor.bind_current("NodeEditorFrame::end()");
            unsafe { sys::dne_end() };
            self.ended = true;
        }
    }
}

impl Drop for NodeEditorFrame<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}
