use super::core::NodeEditorFrame;
use super::queries::{collect_link_ids, collect_node_ids};
use super::validation::{assert_finite_vec4, assert_non_negative_finite_f32};
use crate::{LinkId, NodeId, PinId, sys, vec4};
use std::{marker::PhantomData, rc::Rc};

impl<'ui> NodeEditorFrame<'ui> {
    pub fn begin_create<'a>(
        &'a self,
        color: [f32; 4],
        thickness: f32,
    ) -> Option<CreateSession<'a>> {
        assert_finite_vec4("NodeEditorFrame::begin_create()", "color", color);
        assert_non_negative_finite_f32("NodeEditorFrame::begin_create()", "thickness", thickness);
        unsafe { sys::dne_begin_create(vec4(color), thickness) }.then_some(CreateSession {
            ended: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub fn begin_delete<'a>(&'a self) -> Option<DeleteSession<'a>> {
        unsafe { sys::dne_begin_delete() }.then_some(DeleteSession {
            ended: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub fn begin_shortcut<'a>(&'a self) -> Option<ShortcutSession<'a>> {
        unsafe { sys::dne_begin_shortcut() }.then_some(ShortcutSession {
            ended: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        })
    }
}

pub struct CreateSession<'a> {
    ended: bool,
    _scope: PhantomData<&'a ()>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl CreateSession<'_> {
    pub fn query_new_link(&self) -> Option<(PinId, PinId)> {
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
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_query_new_link_styled(&mut start, &mut end, vec4(color), thickness) }
            .then_some((PinId(start), PinId(end)))
    }

    pub fn query_new_node(&self) -> Option<PinId> {
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
        let mut pin = 0usize;
        unsafe { sys::dne_query_new_node_styled(&mut pin, vec4(color), thickness) }
            .then_some(PinId(pin))
    }

    pub fn accept_new_item(&self) -> bool {
        unsafe { sys::dne_accept_new_item() }
    }

    pub fn accept_new_item_styled(&self, color: [f32; 4], thickness: f32) -> bool {
        assert_finite_vec4("CreateSession::accept_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::accept_new_item_styled()",
            "thickness",
            thickness,
        );
        unsafe { sys::dne_accept_new_item_styled(vec4(color), thickness) }
    }

    pub fn reject_new_item(&self) {
        unsafe { sys::dne_reject_new_item() };
    }

    pub fn reject_new_item_styled(&self, color: [f32; 4], thickness: f32) {
        assert_finite_vec4("CreateSession::reject_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::reject_new_item_styled()",
            "thickness",
            thickness,
        );
        unsafe { sys::dne_reject_new_item_styled(vec4(color), thickness) };
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
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
    ended: bool,
    _scope: PhantomData<&'a ()>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl DeleteSession<'_> {
    pub fn query_deleted_link(&self) -> Option<(LinkId, PinId, PinId)> {
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
        let mut node = 0usize;
        unsafe { sys::dne_query_deleted_node(&mut node) }.then_some(NodeId(node))
    }

    pub fn accept_deleted_item(&self, delete_dependencies: bool) -> bool {
        unsafe { sys::dne_accept_deleted_item(delete_dependencies) }
    }

    pub fn reject_deleted_item(&self) {
        unsafe { sys::dne_reject_deleted_item() };
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
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
    ended: bool,
    _scope: PhantomData<&'a ()>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl ShortcutSession<'_> {
    pub fn accept_cut(&self) -> bool {
        unsafe { sys::dne_accept_cut() }
    }

    pub fn accept_copy(&self) -> bool {
        unsafe { sys::dne_accept_copy() }
    }

    pub fn accept_paste(&self) -> bool {
        unsafe { sys::dne_accept_paste() }
    }

    pub fn accept_duplicate(&self) -> bool {
        unsafe { sys::dne_accept_duplicate() }
    }

    pub fn accept_create_node(&self) -> bool {
        unsafe { sys::dne_accept_create_node() }
    }

    pub fn action_context_size(&self) -> usize {
        unsafe { sys::dne_get_action_context_size() }.max(0) as usize
    }

    pub fn action_context_nodes(&self) -> Vec<NodeId> {
        let count = self.action_context_size();
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_action_context_nodes(ptr, len)
        })
    }

    pub fn action_context_links(&self) -> Vec<LinkId> {
        let count = self.action_context_size();
        collect_link_ids(count, |ptr, len| unsafe {
            sys::dne_get_action_context_links(ptr, len)
        })
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
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
