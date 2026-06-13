use super::core::NodeEditorFrame;
use super::queries::{collect_link_ids, collect_node_ids};
use super::validation::{assert_finite_vec4, assert_non_negative_finite_f32};
use crate::{EditorContext, LinkId, NodeId, PinId, sys, vec4};
use std::{marker::PhantomData, rc::Rc};

impl<'ui> NodeEditorFrame<'ui> {
    pub fn begin_create<'a>(
        &'a self,
        color: [f32; 4],
        thickness: f32,
    ) -> Option<CreateSession<'a>> {
        assert_finite_vec4("NodeEditorFrame::begin_create()", "color", color);
        assert_non_negative_finite_f32("NodeEditorFrame::begin_create()", "thickness", thickness);
        let _current_editor = self.bind("NodeEditorFrame::begin_create()");
        unsafe { sys::dne_begin_create(vec4(color), thickness) }.then_some(CreateSession {
            editor: self._editor,
            ended: false,
            _not_send_sync: PhantomData,
        })
    }

    pub fn begin_delete<'a>(&'a self) -> Option<DeleteSession<'a>> {
        let _current_editor = self.bind("NodeEditorFrame::begin_delete()");
        unsafe { sys::dne_begin_delete() }.then_some(DeleteSession {
            editor: self._editor,
            ended: false,
            _not_send_sync: PhantomData,
        })
    }

    pub fn begin_shortcut<'a>(&'a self) -> Option<ShortcutSession<'a>> {
        let _current_editor = self.bind("NodeEditorFrame::begin_shortcut()");
        unsafe { sys::dne_begin_shortcut() }.then_some(ShortcutSession {
            editor: self._editor,
            ended: false,
            _not_send_sync: PhantomData,
        })
    }
}

pub struct CreateSession<'a> {
    editor: &'a EditorContext,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl CreateSession<'_> {
    pub fn query_new_link(&self) -> Option<(PinId, PinId)> {
        let _current_editor = self.editor.bind_current("CreateSession::query_new_link()");
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_query_new_link(&mut start, &mut end) }
            .then_some((PinId(start), PinId(end)))
    }

    pub fn query_new_link_styled(&self, color: [f32; 4], thickness: f32) -> Option<(PinId, PinId)> {
        assert_finite_vec4("CreateSession::query_new_link_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::query_new_link_styled()",
            "thickness",
            thickness,
        );
        let _current_editor = self
            .editor
            .bind_current("CreateSession::query_new_link_styled()");
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_query_new_link_styled(&mut start, &mut end, vec4(color), thickness) }
            .then_some((PinId(start), PinId(end)))
    }

    pub fn query_new_node(&self) -> Option<PinId> {
        let _current_editor = self.editor.bind_current("CreateSession::query_new_node()");
        let mut pin = 0usize;
        unsafe { sys::dne_query_new_node(&mut pin) }.then_some(PinId(pin))
    }

    pub fn query_new_node_styled(&self, color: [f32; 4], thickness: f32) -> Option<PinId> {
        assert_finite_vec4("CreateSession::query_new_node_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::query_new_node_styled()",
            "thickness",
            thickness,
        );
        let _current_editor = self
            .editor
            .bind_current("CreateSession::query_new_node_styled()");
        let mut pin = 0usize;
        unsafe { sys::dne_query_new_node_styled(&mut pin, vec4(color), thickness) }
            .then_some(PinId(pin))
    }

    pub fn accept_new_item(&self) -> bool {
        let _current_editor = self.editor.bind_current("CreateSession::accept_new_item()");
        unsafe { sys::dne_accept_new_item() }
    }

    pub fn accept_new_item_styled(&self, color: [f32; 4], thickness: f32) -> bool {
        assert_finite_vec4("CreateSession::accept_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::accept_new_item_styled()",
            "thickness",
            thickness,
        );
        let _current_editor = self
            .editor
            .bind_current("CreateSession::accept_new_item_styled()");
        unsafe { sys::dne_accept_new_item_styled(vec4(color), thickness) }
    }

    pub fn reject_new_item(&self) {
        let _current_editor = self.editor.bind_current("CreateSession::reject_new_item()");
        unsafe { sys::dne_reject_new_item() };
    }

    pub fn reject_new_item_styled(&self, color: [f32; 4], thickness: f32) {
        assert_finite_vec4("CreateSession::reject_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::reject_new_item_styled()",
            "thickness",
            thickness,
        );
        let _current_editor = self
            .editor
            .bind_current("CreateSession::reject_new_item_styled()");
        unsafe { sys::dne_reject_new_item_styled(vec4(color), thickness) };
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            let _current_editor = self.editor.bind_current("CreateSession::end()");
            unsafe { sys::dne_end_create() };
            self.ended = true;
        }
    }
}

impl Drop for CreateSession<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct DeleteSession<'a> {
    editor: &'a EditorContext,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl DeleteSession<'_> {
    pub fn query_deleted_link(&self) -> Option<(LinkId, PinId, PinId)> {
        let _current_editor = self
            .editor
            .bind_current("DeleteSession::query_deleted_link()");
        let mut link = 0usize;
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_query_deleted_link(&mut link, &mut start, &mut end) }.then_some((
            LinkId(link),
            PinId(start),
            PinId(end),
        ))
    }

    pub fn query_deleted_node(&self) -> Option<NodeId> {
        let _current_editor = self
            .editor
            .bind_current("DeleteSession::query_deleted_node()");
        let mut node = 0usize;
        unsafe { sys::dne_query_deleted_node(&mut node) }.then_some(NodeId(node))
    }

    pub fn accept_deleted_item(&self, delete_dependencies: bool) -> bool {
        let _current_editor = self
            .editor
            .bind_current("DeleteSession::accept_deleted_item()");
        unsafe { sys::dne_accept_deleted_item(delete_dependencies) }
    }

    pub fn reject_deleted_item(&self) {
        let _current_editor = self
            .editor
            .bind_current("DeleteSession::reject_deleted_item()");
        unsafe { sys::dne_reject_deleted_item() };
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            let _current_editor = self.editor.bind_current("DeleteSession::end()");
            unsafe { sys::dne_end_delete() };
            self.ended = true;
        }
    }
}

impl Drop for DeleteSession<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct ShortcutSession<'a> {
    editor: &'a EditorContext,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl ShortcutSession<'_> {
    pub fn accept_cut(&self) -> bool {
        let _current_editor = self.editor.bind_current("ShortcutSession::accept_cut()");
        unsafe { sys::dne_accept_cut() }
    }

    pub fn accept_copy(&self) -> bool {
        let _current_editor = self.editor.bind_current("ShortcutSession::accept_copy()");
        unsafe { sys::dne_accept_copy() }
    }

    pub fn accept_paste(&self) -> bool {
        let _current_editor = self.editor.bind_current("ShortcutSession::accept_paste()");
        unsafe { sys::dne_accept_paste() }
    }

    pub fn accept_duplicate(&self) -> bool {
        let _current_editor = self
            .editor
            .bind_current("ShortcutSession::accept_duplicate()");
        unsafe { sys::dne_accept_duplicate() }
    }

    pub fn accept_create_node(&self) -> bool {
        let _current_editor = self
            .editor
            .bind_current("ShortcutSession::accept_create_node()");
        unsafe { sys::dne_accept_create_node() }
    }

    pub fn action_context_size(&self) -> usize {
        let _current_editor = self
            .editor
            .bind_current("ShortcutSession::action_context_size()");
        unsafe { sys::dne_get_action_context_size() }.max(0) as usize
    }

    pub fn action_context_nodes(&self) -> Vec<NodeId> {
        let count = self.action_context_size();
        let _current_editor = self
            .editor
            .bind_current("ShortcutSession::action_context_nodes()");
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_action_context_nodes(ptr, len)
        })
    }

    pub fn action_context_links(&self) -> Vec<LinkId> {
        let count = self.action_context_size();
        let _current_editor = self
            .editor
            .bind_current("ShortcutSession::action_context_links()");
        collect_link_ids(count, |ptr, len| unsafe {
            sys::dne_get_action_context_links(ptr, len)
        })
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            let _current_editor = self.editor.bind_current("ShortcutSession::end()");
            unsafe { sys::dne_end_shortcut() };
            self.ended = true;
        }
    }
}

impl Drop for ShortcutSession<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}
