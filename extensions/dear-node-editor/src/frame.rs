use crate::{
    EditorContext, FlowDirection, LinkId, NodeId, PinId, PinKind, StyleColor, StyleVar,
    context::CurrentEditorGuard, from_vec2, sys, vec2, vec4,
};
use dear_imgui_rs::Ui;
use std::{ffi::CString, marker::PhantomData};

/// RAII token for an active node-editor frame.
pub struct NodeEditorFrame<'ui> {
    _ui: &'ui Ui,
    _editor: &'ui EditorContext,
    _current_editor: CurrentEditorGuard<'ui>,
    ended: bool,
}

impl<'ui> NodeEditorFrame<'ui> {
    pub(crate) fn new(
        ui: &'ui Ui,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> Self {
        let current_editor = editor.bind_current("Ui::node_editor");
        let id = CString::new(id.as_ref()).expect("node editor id cannot contain NUL bytes");
        unsafe { sys::dne_begin(id.as_ptr(), vec2(size)) };
        Self {
            _ui: ui,
            _editor: editor,
            _current_editor: current_editor,
            ended: false,
        }
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            unsafe { sys::dne_end() };
            self.ended = true;
        }
    }

    pub fn begin_node<'a>(&'a self, node: NodeId) -> NodeToken<'a> {
        unsafe { sys::dne_begin_node(node.raw()) };
        NodeToken {
            ended: false,
            _scope: PhantomData,
        }
    }

    pub fn begin_pin<'a>(&'a self, pin: PinId, kind: PinKind) -> PinToken<'a> {
        unsafe { sys::dne_begin_pin(pin.raw(), kind.raw()) };
        PinToken {
            ended: false,
            _scope: PhantomData,
        }
    }

    pub fn group(&self, size: [f32; 2]) {
        unsafe { sys::dne_group(vec2(size)) };
    }

    pub fn link(&self, link: LinkId, start_pin: PinId, end_pin: PinId) -> bool {
        self.link_colored(link, start_pin, end_pin, [1.0, 1.0, 1.0, 1.0], 1.0)
    }

    pub fn link_colored(
        &self,
        link: LinkId,
        start_pin: PinId,
        end_pin: PinId,
        color: [f32; 4],
        thickness: f32,
    ) -> bool {
        assert!(
            thickness.is_finite() && thickness >= 0.0,
            "link thickness must be finite"
        );
        unsafe {
            sys::dne_link(
                link.raw(),
                start_pin.raw(),
                end_pin.raw(),
                vec4(color),
                thickness,
            )
        }
    }

    pub fn flow(&self, link: LinkId, direction: FlowDirection) {
        unsafe { sys::dne_flow(link.raw(), direction.raw()) };
    }

    pub fn begin_create<'a>(
        &'a self,
        color: [f32; 4],
        thickness: f32,
    ) -> Option<CreateSession<'a>> {
        assert!(
            thickness.is_finite() && thickness >= 0.0,
            "create thickness must be finite"
        );
        unsafe { sys::dne_begin_create(vec4(color), thickness) }.then_some(CreateSession {
            ended: false,
            _scope: PhantomData,
        })
    }

    pub fn begin_delete<'a>(&'a self) -> Option<DeleteSession<'a>> {
        unsafe { sys::dne_begin_delete() }.then_some(DeleteSession {
            ended: false,
            _scope: PhantomData,
        })
    }

    pub fn begin_shortcut<'a>(&'a self) -> Option<ShortcutSession<'a>> {
        unsafe { sys::dne_begin_shortcut() }.then_some(ShortcutSession {
            ended: false,
            _scope: PhantomData,
        })
    }

    pub fn push_style_color<'a>(
        &'a self,
        color: StyleColor,
        value: [f32; 4],
    ) -> StyleColorToken<'a> {
        unsafe { sys::dne_push_style_color(color.raw(), vec4(value)) };
        StyleColorToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
        }
    }

    pub fn push_style_var_float<'a>(&'a self, var: StyleVar, value: f32) -> StyleVarToken<'a> {
        assert!(value.is_finite(), "style value must be finite");
        unsafe { sys::dne_push_style_var_float(var.raw(), value) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
        }
    }

    pub fn push_style_var_vec2<'a>(&'a self, var: StyleVar, value: [f32; 2]) -> StyleVarToken<'a> {
        unsafe { sys::dne_push_style_var_vec2(var.raw(), vec2(value)) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
        }
    }

    pub fn push_style_var_vec4<'a>(&'a self, var: StyleVar, value: [f32; 4]) -> StyleVarToken<'a> {
        unsafe { sys::dne_push_style_var_vec4(var.raw(), vec4(value)) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
        }
    }

    pub fn set_node_position(&self, node: NodeId, position: [f32; 2]) {
        unsafe { sys::dne_set_node_position(node.raw(), vec2(position)) };
    }

    pub fn node_position(&self, node: NodeId) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_node_position(node.raw()) })
    }

    pub fn node_size(&self, node: NodeId) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_node_size(node.raw()) })
    }

    pub fn center_node_on_screen(&self, node: NodeId) {
        unsafe { sys::dne_center_node_on_screen(node.raw()) };
    }

    pub fn navigate_to_content(&self, duration: f32) {
        unsafe { sys::dne_navigate_to_content(duration) };
    }

    pub fn navigate_to_selection(&self, zoom_in: bool, duration: f32) {
        unsafe { sys::dne_navigate_to_selection(zoom_in, duration) };
    }

    pub fn selected_nodes(&self) -> Vec<NodeId> {
        let count = unsafe { sys::dne_get_selected_object_count() }.max(0) as usize;
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_selected_nodes(ptr, len)
        })
    }

    pub fn selected_links(&self) -> Vec<LinkId> {
        let count = unsafe { sys::dne_get_selected_object_count() }.max(0) as usize;
        collect_link_ids(count, |ptr, len| unsafe {
            sys::dne_get_selected_links(ptr, len)
        })
    }

    pub fn hovered_node(&self) -> Option<NodeId> {
        optional_id(NodeId, |ptr| unsafe { sys::dne_get_hovered_node(ptr) })
    }

    pub fn hovered_pin(&self) -> Option<PinId> {
        optional_id(PinId, |ptr| unsafe { sys::dne_get_hovered_pin(ptr) })
    }

    pub fn hovered_link(&self) -> Option<LinkId> {
        optional_id(LinkId, |ptr| unsafe { sys::dne_get_hovered_link(ptr) })
    }

    pub fn double_clicked_node(&self) -> Option<NodeId> {
        optional_id(NodeId, |ptr| unsafe {
            sys::dne_get_double_clicked_node(ptr)
        })
    }

    pub fn double_clicked_pin(&self) -> Option<PinId> {
        optional_id(PinId, |ptr| unsafe { sys::dne_get_double_clicked_pin(ptr) })
    }

    pub fn double_clicked_link(&self) -> Option<LinkId> {
        optional_id(LinkId, |ptr| unsafe {
            sys::dne_get_double_clicked_link(ptr)
        })
    }

    pub fn show_node_context_menu(&self) -> Option<NodeId> {
        optional_id(NodeId, |ptr| unsafe {
            sys::dne_show_node_context_menu(ptr)
        })
    }

    pub fn show_pin_context_menu(&self) -> Option<PinId> {
        optional_id(PinId, |ptr| unsafe { sys::dne_show_pin_context_menu(ptr) })
    }

    pub fn show_link_context_menu(&self) -> Option<LinkId> {
        optional_id(LinkId, |ptr| unsafe {
            sys::dne_show_link_context_menu(ptr)
        })
    }

    pub fn show_background_context_menu(&self) -> bool {
        unsafe { sys::dne_show_background_context_menu() }
    }

    pub fn current_zoom(&self) -> f32 {
        unsafe { sys::dne_get_current_zoom() }
    }

    pub fn screen_to_canvas(&self, pos: [f32; 2]) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_screen_to_canvas(vec2(pos)) })
    }

    pub fn canvas_to_screen(&self, pos: [f32; 2]) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_canvas_to_screen(vec2(pos)) })
    }
}

impl Drop for NodeEditorFrame<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct NodeToken<'a> {
    ended: bool,
    _scope: PhantomData<&'a ()>,
}

impl NodeToken<'_> {
    pub fn begin_pin<'a>(&'a self, pin: PinId, kind: PinKind) -> PinToken<'a> {
        unsafe { sys::dne_begin_pin(pin.raw(), kind.raw()) };
        PinToken {
            ended: false,
            _scope: PhantomData,
        }
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            unsafe { sys::dne_end_node() };
            self.ended = true;
        }
    }
}

impl Drop for NodeToken<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct PinToken<'a> {
    ended: bool,
    _scope: PhantomData<&'a ()>,
}

impl PinToken<'_> {
    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            unsafe { sys::dne_end_pin() };
            self.ended = true;
        }
    }
}

impl Drop for PinToken<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct CreateSession<'a> {
    ended: bool,
    _scope: PhantomData<&'a ()>,
}

impl CreateSession<'_> {
    pub fn query_new_link(&self) -> Option<(PinId, PinId)> {
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_query_new_link(&mut start, &mut end) }
            .then_some((PinId(start), PinId(end)))
    }

    pub fn query_new_node(&self) -> Option<PinId> {
        let mut pin = 0usize;
        unsafe { sys::dne_query_new_node(&mut pin) }.then_some(PinId(pin))
    }

    pub fn accept_new_item(&self) -> bool {
        unsafe { sys::dne_accept_new_item() }
    }

    pub fn accept_new_item_styled(&self, color: [f32; 4], thickness: f32) -> bool {
        unsafe { sys::dne_accept_new_item_styled(vec4(color), thickness) }
    }

    pub fn reject_new_item(&self) {
        unsafe { sys::dne_reject_new_item() };
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

    pub fn action_context_nodes(&self) -> Vec<NodeId> {
        let count = unsafe { sys::dne_get_action_context_size() }.max(0) as usize;
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_action_context_nodes(ptr, len)
        })
    }

    pub fn action_context_links(&self) -> Vec<LinkId> {
        let count = unsafe { sys::dne_get_action_context_size() }.max(0) as usize;
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

pub struct StyleColorToken<'a> {
    count: i32,
    popped: bool,
    _scope: PhantomData<&'a ()>,
}

impl StyleColorToken<'_> {
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if !self.popped {
            unsafe { sys::dne_pop_style_color(self.count) };
            self.popped = true;
        }
    }
}

impl Drop for StyleColorToken<'_> {
    fn drop(&mut self) {
        self.pop_inner();
    }
}

pub struct StyleVarToken<'a> {
    count: i32,
    popped: bool,
    _scope: PhantomData<&'a ()>,
}

impl StyleVarToken<'_> {
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if !self.popped {
            unsafe { sys::dne_pop_style_var(self.count) };
            self.popped = true;
        }
    }
}

impl Drop for StyleVarToken<'_> {
    fn drop(&mut self) {
        self.pop_inner();
    }
}

fn optional_id<T>(make: fn(usize) -> T, f: impl FnOnce(*mut usize) -> bool) -> Option<T> {
    let mut raw = 0usize;
    f(&mut raw).then_some(make(raw))
}

fn collect_node_ids(count: usize, f: impl FnOnce(*mut usize, i32) -> i32) -> Vec<NodeId> {
    collect_ids(count, f).into_iter().map(NodeId).collect()
}

fn collect_link_ids(count: usize, f: impl FnOnce(*mut usize, i32) -> i32) -> Vec<LinkId> {
    collect_ids(count, f).into_iter().map(LinkId).collect()
}

fn collect_ids(count: usize, f: impl FnOnce(*mut usize, i32) -> i32) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let mut values = vec![0usize; count];
    let written = f(values.as_mut_ptr(), values.len() as i32).max(0) as usize;
    values.truncate(written.min(count));
    values
}
