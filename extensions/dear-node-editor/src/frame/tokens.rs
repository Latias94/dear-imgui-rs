use super::core::{BuildPhase, NodeEditorFrame};
use super::validation::{
    assert_finite_f32, assert_finite_rect, assert_finite_vec2, assert_finite_vec4,
    assert_non_negative_finite_f32, assert_non_negative_finite_vec2, assert_style_var_type,
};
use crate::{
    EditorContext, FlowDirection, LinkId, NodeEditorStyle, NodeId, PinId, PinKind, StyleColor,
    StyleVar, StyleVarType, from_vec2, sys, vec2, vec4,
};
use dear_imgui_rs::{DrawListMut, Ui};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    marker::PhantomData,
    rc::Rc,
};

impl<'ui> NodeEditorFrame<'ui> {
    pub fn begin_node<'a>(&'a self, node: NodeId) -> NodeToken<'a> {
        assert!(
            self.phase.get() == BuildPhase::Idle,
            "NodeEditorFrame::begin_node() requires the current node or group hint to be ended"
        );
        self.with_current("NodeEditorFrame::begin_node()", || unsafe {
            sys::dne_begin_node(node.raw())
        });
        self.phase.set(BuildPhase::Node);
        NodeToken {
            editor: self._editor,
            phase: &self.phase,
            submitted_pins: &self.submitted_pins,
            ended: false,
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
        assert!(
            self.phase.get() == BuildPhase::Idle,
            "NodeEditorFrame::begin_group_hint() requires the current node or group hint to be ended"
        );
        self.with_current("NodeEditorFrame::begin_group_hint()", || unsafe {
            sys::dne_begin_group_hint(node.raw())
        })
        .then(|| {
            self.phase.set(BuildPhase::GroupHint);
            GroupHintToken {
                editor: self._editor,
                ui: self._ui,
                phase: &self.phase,
                ended: false,
                _not_send_sync: PhantomData,
            }
        })
    }

    /// Returns a node's background draw list after that node has been ended.
    ///
    /// Returns `None` when the node was not submitted in this frame or the host
    /// window skips rendering. Panics while a node or group hint is being built.
    pub fn node_background_draw_list(&self, node: NodeId) -> Option<DrawListMut<'_>> {
        assert!(
            self.phase.get() == BuildPhase::Idle,
            "NodeEditorFrame::node_background_draw_list() requires the current node or group hint to be ended"
        );
        let draw_list = self
            .with_current("NodeEditorFrame::node_background_draw_list()", || unsafe {
                sys::dne_get_node_background_draw_list(node.raw())
            });
        (!draw_list.is_null())
            .then(|| unsafe { DrawListMut::from_raw_mut(self._ui, draw_list.cast()) })
    }

    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> NodeEditorStyle {
        self._editor.style()
    }

    pub fn group(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::group()", "size", size);
        assert_eq!(
            self.phase.get(),
            BuildPhase::Node,
            "NodeEditorFrame::group() requires an active node with no pin or previous group"
        );
        self.with_current("NodeEditorFrame::group()", || unsafe {
            sys::dne_group(vec2(size))
        });
        self.phase.set(BuildPhase::Group);
    }

    pub fn set_group_size(&self, node: NodeId, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::set_group_size()", "size", size);
        self.with_current("NodeEditorFrame::set_group_size()", || unsafe {
            sys::dne_set_group_size(node.raw(), vec2(size))
        });
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
        self.with_current("NodeEditorFrame::link_colored()", || unsafe {
            sys::dne_link(
                link.raw(),
                start_pin.raw(),
                end_pin.raw(),
                vec4(color),
                thickness,
            )
        })
    }

    pub fn flow(&self, link: LinkId, direction: FlowDirection) {
        self.with_current("NodeEditorFrame::flow()", || unsafe {
            sys::dne_flow(link.raw(), direction.raw())
        });
    }

    pub fn push_style_color<'a>(
        &'a self,
        color: StyleColor,
        value: [f32; 4],
    ) -> StyleColorToken<'a> {
        assert_finite_vec4("NodeEditorFrame::push_style_color()", "value", value);
        self.with_current("NodeEditorFrame::push_style_color()", || unsafe {
            sys::dne_push_style_color(color.raw(), vec4(value))
        });
        StyleColorToken {
            editor: self._editor,
            count: 1,
            popped: false,
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
        self.with_current("NodeEditorFrame::push_style_var_float()", || unsafe {
            sys::dne_push_style_var_float(var.raw(), value)
        });
        StyleVarToken {
            editor: self._editor,
            count: 1,
            popped: false,
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
        self.with_current("NodeEditorFrame::push_style_var_vec2()", || unsafe {
            sys::dne_push_style_var_vec2(var.raw(), vec2(value))
        });
        StyleVarToken {
            editor: self._editor,
            count: 1,
            popped: false,
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
        self.with_current("NodeEditorFrame::push_style_var_vec4()", || unsafe {
            sys::dne_push_style_var_vec4(var.raw(), vec4(value))
        });
        StyleVarToken {
            editor: self._editor,
            count: 1,
            popped: false,
            _not_send_sync: PhantomData,
        }
    }

    pub fn suspend<'a>(&'a self) -> SuspensionToken<'a> {
        assert!(
            !self.suspended.replace(true),
            "NodeEditorFrame::suspend() cannot be called while the editor is already suspended"
        );
        self.with_current("NodeEditorFrame::suspend()", || unsafe {
            sys::dne_suspend()
        });
        SuspensionToken {
            editor: self._editor,
            suspended: &self.suspended,
            resumed: false,
        }
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.get()
            || self.with_current("NodeEditorFrame::is_suspended()", || unsafe {
                sys::dne_is_suspended()
            })
    }
}

pub struct NodeToken<'a> {
    editor: &'a EditorContext,
    phase: &'a Cell<BuildPhase>,
    submitted_pins: &'a RefCell<HashSet<PinId>>,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl NodeToken<'_> {
    /// Begins a pin whose ID is unique across this editor's current Begin/End scope.
    ///
    /// Panics before FFI if another pin is active, this node is a group, or the ID
    /// was already submitted. IDs can be reused after the editor frame ends.
    pub fn begin_pin<'a>(&'a self, pin: PinId, kind: PinKind) -> PinToken<'a> {
        assert_eq!(
            self.phase.get(),
            BuildPhase::Node,
            "NodeToken::begin_pin() requires a node with no active pin or group"
        );
        assert!(
            self.submitted_pins.borrow_mut().insert(pin),
            "NodeToken::begin_pin() requires each pin ID to be submitted only once per editor frame"
        );
        self.editor
            .with_current("NodeToken::begin_pin()", || unsafe {
                sys::dne_begin_pin(pin.raw(), kind.raw())
            });
        self.phase.set(BuildPhase::Pin);
        PinToken {
            editor: self.editor,
            phase: self.phase,
            ended: false,
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
            self.editor
                .with_current("NodeToken::end()", || unsafe { sys::dne_end_node() });
            self.phase.set(BuildPhase::Idle);
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
    editor: &'a EditorContext,
    phase: &'a Cell<BuildPhase>,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl PinToken<'_> {
    pub fn end(mut self) {
        self.end_inner();
    }

    pub fn rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::rect()", min, max);
        self.editor.with_current("PinToken::rect()", || unsafe {
            sys::dne_pin_rect(vec2(min), vec2(max))
        });
    }

    pub fn pivot_rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::pivot_rect()", min, max);
        self.editor
            .with_current("PinToken::pivot_rect()", || unsafe {
                sys::dne_pin_pivot_rect(vec2(min), vec2(max))
            });
    }

    pub fn pivot_size(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("PinToken::pivot_size()", "size", size);
        self.editor
            .with_current("PinToken::pivot_size()", || unsafe {
                sys::dne_pin_pivot_size(vec2(size))
            });
    }

    pub fn pivot_scale(&self, scale: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_scale()", "scale", scale);
        self.editor
            .with_current("PinToken::pivot_scale()", || unsafe {
                sys::dne_pin_pivot_scale(vec2(scale))
            });
    }

    pub fn pivot_alignment(&self, alignment: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_alignment()", "alignment", alignment);
        self.editor
            .with_current("PinToken::pivot_alignment()", || unsafe {
                sys::dne_pin_pivot_alignment(vec2(alignment))
            });
    }

    fn end_inner(&mut self) {
        if !self.ended {
            self.editor
                .with_current("PinToken::end()", || unsafe { sys::dne_end_pin() });
            self.phase.set(BuildPhase::Node);
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
    editor: &'a EditorContext,
    ui: &'a Ui,
    phase: &'a Cell<BuildPhase>,
    ended: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'a> GroupHintToken<'a> {
    pub fn min(&self) -> [f32; 2] {
        self.editor.with_current("GroupHintToken::min()", || {
            from_vec2(unsafe { sys::dne_get_group_min() })
        })
    }

    pub fn max(&self) -> [f32; 2] {
        self.editor.with_current("GroupHintToken::max()", || {
            from_vec2(unsafe { sys::dne_get_group_max() })
        })
    }

    pub fn foreground_draw_list(&self) -> DrawListMut<'_> {
        let draw_list = self
            .editor
            .with_current("GroupHintToken::foreground_draw_list()", || unsafe {
                sys::dne_get_hint_foreground_draw_list()
            });
        unsafe { DrawListMut::from_raw_mut(self.ui, draw_list.cast()) }
    }

    pub fn background_draw_list(&self) -> DrawListMut<'_> {
        let draw_list = self
            .editor
            .with_current("GroupHintToken::background_draw_list()", || unsafe {
                sys::dne_get_hint_background_draw_list()
            });
        unsafe { DrawListMut::from_raw_mut(self.ui, draw_list.cast()) }
    }

    pub fn end(mut self) {
        self.end_inner();
    }

    fn end_inner(&mut self) {
        if !self.ended {
            self.editor
                .with_current("GroupHintToken::end()", || unsafe {
                    sys::dne_end_group_hint()
                });
            self.phase.set(BuildPhase::Idle);
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
    editor: &'a EditorContext,
    suspended: &'a Cell<bool>,
    resumed: bool,
}

impl SuspensionToken<'_> {
    pub fn resume(mut self) {
        self.resume_inner();
    }

    fn resume_inner(&mut self) {
        if !self.resumed {
            self.editor
                .with_current("SuspensionToken::resume()", || unsafe { sys::dne_resume() });
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
    editor: &'a EditorContext,
    count: i32,
    popped: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl StyleColorToken<'_> {
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if !self.popped {
            self.editor
                .with_current("StyleColorToken::pop()", || unsafe {
                    sys::dne_pop_style_color(self.count)
                });
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
    editor: &'a EditorContext,
    count: i32,
    popped: bool,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl StyleVarToken<'_> {
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if !self.popped {
            self.editor.with_current("StyleVarToken::pop()", || unsafe {
                sys::dne_pop_style_var(self.count)
            });
            self.popped = true;
        }
    }
}

impl Drop for StyleVarToken<'_> {
    fn drop(&mut self) {
        self.pop_inner();
    }
}
