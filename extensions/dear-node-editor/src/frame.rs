use crate::{
    EditorContext, FlowDirection, LinkId, NodeEditorStyle, NodeId, PinId, PinKind, StyleColor,
    StyleVar, StyleVarType, context::CurrentEditorGuard, from_vec2, sys, vec2, vec4,
};
use dear_imgui_rs::{DrawListMut, MouseButton, Ui};
use std::{cell::Cell, ffi::CString, marker::PhantomData, rc::Rc};

/// RAII token for an active node-editor frame.
pub struct NodeEditorFrame<'ui> {
    _ui: &'ui Ui,
    _editor: &'ui EditorContext,
    _current_editor: CurrentEditorGuard<'ui>,
    suspended: Cell<bool>,
    ended: bool,
}

impl<'ui> NodeEditorFrame<'ui> {
    pub(crate) fn new(
        ui: &'ui Ui,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
    ) -> Self {
        assert_finite_vec2("Ui::node_editor()", "size", size);
        let current_editor = editor.bind_current("Ui::node_editor");
        let id = CString::new(id.as_ref()).expect("node editor id cannot contain NUL bytes");
        unsafe { sys::dne_begin(id.as_ptr(), vec2(size)) };
        Self {
            _ui: ui,
            _editor: editor,
            _current_editor: current_editor,
            suspended: Cell::new(false),
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
            _not_send_sync: PhantomData,
        }
    }

    pub fn node<R>(&self, node: NodeId, f: impl FnOnce(&NodeToken<'_>) -> R) -> R {
        let token = self.begin_node(node);
        let result = f(&token);
        token.end();
        result
    }

    pub fn begin_group_hint<'a>(&'a self, node: NodeId) -> Option<GroupHintToken<'a>> {
        unsafe { sys::dne_begin_group_hint(node.raw()) }.then_some(GroupHintToken {
            ui: self._ui,
            ended: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub fn node_background_draw_list(&self, node: NodeId) -> DrawListMut<'_> {
        let draw_list = unsafe { sys::dne_get_node_background_draw_list(node.raw()) };
        unsafe { DrawListMut::from_raw_mut(self._ui, draw_list.cast()) }
    }

    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> NodeEditorStyle {
        self._editor.style()
    }

    pub fn group(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::group()", "size", size);
        unsafe { sys::dne_group(vec2(size)) };
    }

    pub fn set_group_size(&self, node: NodeId, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::set_group_size()", "size", size);
        unsafe { sys::dne_set_group_size(node.raw(), vec2(size)) };
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
        assert_finite_vec4("NodeEditorFrame::link_colored()", "color", color);
        assert_non_negative_finite_f32("NodeEditorFrame::link_colored()", "thickness", thickness);
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

    pub fn push_style_color<'a>(
        &'a self,
        color: StyleColor,
        value: [f32; 4],
    ) -> StyleColorToken<'a> {
        assert_finite_vec4("NodeEditorFrame::push_style_color()", "value", value);
        unsafe { sys::dne_push_style_color(color.raw(), vec4(value)) };
        StyleColorToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    pub fn push_style_var_float<'a>(&'a self, var: StyleVar, value: f32) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_float()",
            var,
            StyleVarType::Float,
        );
        assert_finite_f32("NodeEditorFrame::push_style_var_float()", "value", value);
        unsafe { sys::dne_push_style_var_float(var.raw(), value) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    pub fn push_style_var_vec2<'a>(&'a self, var: StyleVar, value: [f32; 2]) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_vec2()",
            var,
            StyleVarType::Vec2,
        );
        assert_finite_vec2("NodeEditorFrame::push_style_var_vec2()", "value", value);
        unsafe { sys::dne_push_style_var_vec2(var.raw(), vec2(value)) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    pub fn push_style_var_vec4<'a>(&'a self, var: StyleVar, value: [f32; 4]) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_vec4()",
            var,
            StyleVarType::Vec4,
        );
        assert_finite_vec4("NodeEditorFrame::push_style_var_vec4()", "value", value);
        unsafe { sys::dne_push_style_var_vec4(var.raw(), vec4(value)) };
        StyleVarToken {
            count: 1,
            popped: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    pub fn set_node_position(&self, node: NodeId, position: [f32; 2]) {
        assert_finite_vec2("NodeEditorFrame::set_node_position()", "position", position);
        unsafe { sys::dne_set_node_position(node.raw(), vec2(position)) };
    }

    pub fn node_position(&self, node: NodeId) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_node_position(node.raw()) })
    }

    pub fn node_size(&self, node: NodeId) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_node_size(node.raw()) })
    }

    pub fn set_node_z_position(&self, node: NodeId, z: f32) {
        assert_finite_f32("NodeEditorFrame::set_node_z_position()", "z", z);
        unsafe { sys::dne_set_node_z_position(node.raw(), z) };
    }

    pub fn node_z_position(&self, node: NodeId) -> f32 {
        unsafe { sys::dne_get_node_z_position(node.raw()) }
    }

    pub fn restore_node_state(&self, node: NodeId) {
        unsafe { sys::dne_restore_node_state(node.raw()) };
    }

    pub fn center_node_on_screen(&self, node: NodeId) {
        unsafe { sys::dne_center_node_on_screen(node.raw()) };
    }

    pub fn navigate_to_content(&self, duration: f32) {
        assert_finite_f32(
            "NodeEditorFrame::navigate_to_content()",
            "duration",
            duration,
        );
        unsafe { sys::dne_navigate_to_content(duration) };
    }

    pub fn navigate_to_selection(&self, zoom_in: bool, duration: f32) {
        assert_finite_f32(
            "NodeEditorFrame::navigate_to_selection()",
            "duration",
            duration,
        );
        unsafe { sys::dne_navigate_to_selection(zoom_in, duration) };
    }

    pub fn suspend<'a>(&'a self) -> SuspensionToken<'a> {
        assert!(
            !self.suspended.replace(true),
            "NodeEditorFrame::suspend() cannot be called while the editor is already suspended"
        );
        unsafe { sys::dne_suspend() };
        SuspensionToken {
            suspended: &self.suspended,
            resumed: false,
        }
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.get() || unsafe { sys::dne_is_suspended() }
    }

    pub fn is_active(&self) -> bool {
        unsafe { sys::dne_is_active() }
    }

    pub fn has_selection_changed(&self) -> bool {
        unsafe { sys::dne_has_selection_changed() }
    }

    pub fn selected_object_count(&self) -> usize {
        unsafe { sys::dne_get_selected_object_count() }.max(0) as usize
    }

    pub fn selected_nodes(&self) -> Vec<NodeId> {
        let count = self.selected_object_count();
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_selected_nodes(ptr, len)
        })
    }

    pub fn selected_links(&self) -> Vec<LinkId> {
        let count = self.selected_object_count();
        collect_link_ids(count, |ptr, len| unsafe {
            sys::dne_get_selected_links(ptr, len)
        })
    }

    pub fn is_node_selected(&self, node: NodeId) -> bool {
        unsafe { sys::dne_is_node_selected(node.raw()) }
    }

    pub fn is_link_selected(&self, link: LinkId) -> bool {
        unsafe { sys::dne_is_link_selected(link.raw()) }
    }

    pub fn clear_selection(&self) {
        unsafe { sys::dne_clear_selection() };
    }

    pub fn select_node(&self, node: NodeId) {
        unsafe { sys::dne_select_node(node.raw(), false) };
    }

    pub fn add_node_to_selection(&self, node: NodeId) {
        unsafe { sys::dne_select_node(node.raw(), true) };
    }

    pub fn select_link(&self, link: LinkId) {
        unsafe { sys::dne_select_link(link.raw(), false) };
    }

    pub fn add_link_to_selection(&self, link: LinkId) {
        unsafe { sys::dne_select_link(link.raw(), true) };
    }

    pub fn deselect_node(&self, node: NodeId) {
        unsafe { sys::dne_deselect_node(node.raw()) };
    }

    pub fn deselect_link(&self, link: LinkId) {
        unsafe { sys::dne_deselect_link(link.raw()) };
    }

    pub fn delete_node(&self, node: NodeId) -> bool {
        unsafe { sys::dne_delete_node(node.raw()) }
    }

    pub fn delete_link(&self, link: LinkId) -> bool {
        unsafe { sys::dne_delete_link(link.raw()) }
    }

    pub fn node_has_any_links(&self, node: NodeId) -> bool {
        unsafe { sys::dne_has_any_links_node(node.raw()) }
    }

    pub fn pin_has_any_links(&self, pin: PinId) -> bool {
        unsafe { sys::dne_has_any_links_pin(pin.raw()) }
    }

    pub fn break_node_links(&self, node: NodeId) -> usize {
        unsafe { sys::dne_break_links_node(node.raw()) }.max(0) as usize
    }

    pub fn break_pin_links(&self, pin: PinId) -> usize {
        unsafe { sys::dne_break_links_pin(pin.raw()) }.max(0) as usize
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

    pub fn set_shortcuts_enabled(&self, enabled: bool) {
        unsafe { sys::dne_enable_shortcuts(enabled) };
    }

    pub fn shortcuts_enabled(&self) -> bool {
        unsafe { sys::dne_are_shortcuts_enabled() }
    }

    pub fn current_zoom(&self) -> f32 {
        unsafe { sys::dne_get_current_zoom() }
    }

    pub fn is_background_clicked(&self) -> bool {
        unsafe { sys::dne_is_background_clicked() }
    }

    pub fn is_background_double_clicked(&self) -> bool {
        unsafe { sys::dne_is_background_double_clicked() }
    }

    pub fn background_click_button(&self) -> Option<MouseButton> {
        mouse_button_from_index(unsafe { sys::dne_get_background_click_button_index() })
    }

    pub fn background_double_click_button(&self) -> Option<MouseButton> {
        mouse_button_from_index(unsafe { sys::dne_get_background_double_click_button_index() })
    }

    pub fn link_pins(&self, link: LinkId) -> Option<(PinId, PinId)> {
        let mut start = 0usize;
        let mut end = 0usize;
        unsafe { sys::dne_get_link_pins(link.raw(), &mut start, &mut end) }
            .then_some((PinId(start), PinId(end)))
    }

    pub fn pin_had_any_links(&self, pin: PinId) -> bool {
        unsafe { sys::dne_pin_had_any_links(pin.raw()) }
    }

    pub fn screen_size(&self) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_screen_size() })
    }

    pub fn screen_to_canvas(&self, pos: [f32; 2]) -> [f32; 2] {
        assert_finite_vec2("NodeEditorFrame::screen_to_canvas()", "pos", pos);
        from_vec2(unsafe { sys::dne_screen_to_canvas(vec2(pos)) })
    }

    pub fn canvas_to_screen(&self, pos: [f32; 2]) -> [f32; 2] {
        assert_finite_vec2("NodeEditorFrame::canvas_to_screen()", "pos", pos);
        from_vec2(unsafe { sys::dne_canvas_to_screen(vec2(pos)) })
    }

    pub fn node_count(&self) -> usize {
        unsafe { sys::dne_get_node_count() }.max(0) as usize
    }

    pub fn ordered_node_ids(&self) -> Vec<NodeId> {
        let count = self.node_count();
        collect_node_ids(count, |ptr, len| unsafe {
            sys::dne_get_ordered_node_ids(ptr, len)
        })
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
    _not_send_sync: PhantomData<Rc<()>>,
}

impl NodeToken<'_> {
    pub fn begin_pin<'a>(&'a self, pin: PinId, kind: PinKind) -> PinToken<'a> {
        unsafe { sys::dne_begin_pin(pin.raw(), kind.raw()) };
        PinToken {
            ended: false,
            _scope: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    pub fn pin<R>(&self, pin: PinId, kind: PinKind, f: impl FnOnce(&PinToken<'_>) -> R) -> R {
        let token = self.begin_pin(pin, kind);
        let result = f(&token);
        token.end();
        result
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
    _not_send_sync: PhantomData<Rc<()>>,
}

impl PinToken<'_> {
    pub fn end(mut self) {
        self.end_inner();
    }

    pub fn rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::rect()", min, max);
        unsafe { sys::dne_pin_rect(vec2(min), vec2(max)) };
    }

    pub fn pivot_rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::pivot_rect()", min, max);
        unsafe { sys::dne_pin_pivot_rect(vec2(min), vec2(max)) };
    }

    pub fn pivot_size(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("PinToken::pivot_size()", "size", size);
        unsafe { sys::dne_pin_pivot_size(vec2(size)) };
    }

    pub fn pivot_scale(&self, scale: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_scale()", "scale", scale);
        unsafe { sys::dne_pin_pivot_scale(vec2(scale)) };
    }

    pub fn pivot_alignment(&self, alignment: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_alignment()", "alignment", alignment);
        unsafe { sys::dne_pin_pivot_alignment(vec2(alignment)) };
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

pub struct GroupHintToken<'a> {
    ui: &'a Ui,
    ended: bool,
    _scope: PhantomData<&'a ()>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'a> GroupHintToken<'a> {
    pub fn min(&self) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_group_min() })
    }

    pub fn max(&self) -> [f32; 2] {
        from_vec2(unsafe { sys::dne_get_group_max() })
    }

    pub fn foreground_draw_list(&self) -> DrawListMut<'_> {
        let draw_list = unsafe { sys::dne_get_hint_foreground_draw_list() };
        unsafe { DrawListMut::from_raw_mut(self.ui, draw_list.cast()) }
    }

    pub fn background_draw_list(&self) -> DrawListMut<'_> {
        let draw_list = unsafe { sys::dne_get_hint_background_draw_list() };
        unsafe { DrawListMut::from_raw_mut(self.ui, draw_list.cast()) }
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            unsafe { sys::dne_end_group_hint() };
            self.ended = true;
        }
    }
}

impl Drop for GroupHintToken<'_> {
    fn drop(&mut self) {
        self.end_inner();
    }
}

pub struct SuspensionToken<'a> {
    suspended: &'a Cell<bool>,
    resumed: bool,
}

impl SuspensionToken<'_> {
    pub fn resume(mut self) {
        self.resume_inner();
    }

    fn resume_inner(&mut self) {
        if !self.resumed {
            unsafe { sys::dne_resume() };
            self.suspended.set(false);
            self.resumed = true;
        }
    }
}

impl Drop for SuspensionToken<'_> {
    fn drop(&mut self) {
        self.resume_inner();
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

pub struct StyleColorToken<'a> {
    count: i32,
    popped: bool,
    _scope: PhantomData<&'a ()>,
    _not_send_sync: PhantomData<Rc<()>>,
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
    _not_send_sync: PhantomData<Rc<()>>,
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
    let count = count.min(i32::MAX as usize);
    let mut values = vec![0usize; count];
    let written = f(values.as_mut_ptr(), values.len() as i32).max(0) as usize;
    values.truncate(written.min(count));
    values
}

fn mouse_button_from_index(index: sys::ImGuiMouseButton) -> Option<MouseButton> {
    match index {
        value if value == MouseButton::Left as i32 => Some(MouseButton::Left),
        value if value == MouseButton::Right as i32 => Some(MouseButton::Right),
        value if value == MouseButton::Middle as i32 => Some(MouseButton::Middle),
        value if value == MouseButton::Extra1 as i32 => Some(MouseButton::Extra1),
        value if value == MouseButton::Extra2 as i32 => Some(MouseButton::Extra2),
        _ => None,
    }
}

fn assert_style_var_type(caller: &str, var: StyleVar, expected: StyleVarType) {
    let actual = var.value_type();
    assert_eq!(
        actual, expected,
        "{caller} expected {expected:?} style variable, got {actual:?} for {var:?}"
    );
}

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_non_negative_finite_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

fn assert_finite_rect(caller: &str, min: [f32; 2], max: [f32; 2]) {
    assert_finite_vec2(caller, "min", min);
    assert_finite_vec2(caller, "max", max);
    assert!(
        min[0] <= max[0] && min[1] <= max[1],
        "{caller} min must not exceed max"
    );
}

fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value.iter().all(|component| *component >= 0.0),
        "{caller} {name} components must be non-negative"
    );
}

fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_button_indices_map_known_imgui_buttons() {
        assert_eq!(mouse_button_from_index(0), Some(MouseButton::Left));
        assert_eq!(mouse_button_from_index(1), Some(MouseButton::Right));
        assert_eq!(mouse_button_from_index(2), Some(MouseButton::Middle));
        assert_eq!(mouse_button_from_index(3), Some(MouseButton::Extra1));
        assert_eq!(mouse_button_from_index(4), Some(MouseButton::Extra2));
        assert_eq!(mouse_button_from_index(-1), None);
        assert_eq!(mouse_button_from_index(99), None);
    }

    #[test]
    #[should_panic(expected = "expected Float style variable")]
    fn style_var_push_rejects_wrong_value_type() {
        assert_style_var_type("test", StyleVar::NodePadding, StyleVarType::Float);
    }
}
