use super::core::NodeEditorFrame;
use super::validation::{
    assert_finite_f32, assert_finite_rect, assert_finite_vec2, assert_finite_vec4,
    assert_non_negative_finite_f32, assert_non_negative_finite_vec2, assert_style_var_type,
};
use crate::{
    FlowDirection, LinkId, NodeEditorStyle, NodeId, PinId, PinKind, StyleColor, StyleVar,
    StyleVarType, from_vec2, sys, vec2, vec4,
};
use dear_imgui_rs::{DrawListMut, Ui};
use std::{cell::Cell, marker::PhantomData, rc::Rc};

impl<'ui> NodeEditorFrame<'ui> {
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
