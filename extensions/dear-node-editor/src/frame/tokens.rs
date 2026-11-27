use super::core::{FrameToken, NodeEditorFrame, ScopeKind};
use super::validation::{
    assert_finite_f32, assert_finite_rect, assert_finite_vec2, assert_finite_vec4,
    assert_non_negative_finite_f32, assert_non_negative_finite_vec2, assert_style_var_type,
};
use crate::{
    FlowDirection, LinkId, NodeEditorStyle, NodeId, PinId, PinKind, StyleColor, StyleVar,
    StyleVarType, from_vec2, sys, vec2, vec4,
};
use dear_imgui_rs::DrawListMut;

impl<'ui> NodeEditorFrame<'ui> {
    /// Begins a manually balanced node. See [`Self::node`] for scoped drawing.
    ///
    /// # Safety
    /// Keep the host's surrounding ImGui groups and drawing channels active. End all
    /// groups and pins opened inside the node before ending it, including on unwind.
    pub unsafe fn begin_node<'a>(&'a self, node: NodeId) -> NodeToken<'a> {
        self.require_scope(None, "NodeEditorFrame::begin_node()");
        self.with_canvas("NodeEditorFrame::begin_node()", || unsafe {
            sys::dne_begin_node(node.raw())
        });
        NodeToken {
            scope: self.entered(ScopeKind::Node),
        }
    }

    pub fn node<R>(&self, node: NodeId, f: impl FnOnce(&NodeToken<'_>) -> R) -> R {
        self.scoped(|| unsafe { self.begin_node(node) }, |token| f(token))
    }

    /// Draws the group hint when upstream offers one at the current zoom level.
    pub fn group_hint<R>(
        &self,
        node: NodeId,
        f: impl FnOnce(&mut GroupHintToken<'_>) -> R,
    ) -> Option<R> {
        self.scoped(
            || unsafe { self.begin_group_hint(node) },
            |hint| hint.as_mut().map(f),
        )
    }

    /// # Safety
    /// Keep the host's style, clip and drawing scopes properly nested around the returned
    /// hint until it ends. [`Self::group_hint`] owns these boundaries during drawing and unwind.
    pub unsafe fn begin_group_hint<'a>(&'a self, node: NodeId) -> Option<GroupHintToken<'a>> {
        self.require_scope(None, "NodeEditorFrame::begin_group_hint()");
        self.with_canvas("NodeEditorFrame::begin_group_hint()", || unsafe {
            sys::dne_begin_group_hint(node.raw())
        })
        .then(|| GroupHintToken {
            scope: self.entered(ScopeKind::GroupHint),
        })
    }

    /// Draw with a node's background draw list after that node has ended.
    ///
    /// Returns `None` when the node was not submitted in this frame or the host
    /// window skips rendering. Requires the editor canvas outside a build or suspension scope.
    pub fn with_node_background_draw_list<R>(
        &self,
        node: NodeId,
        draw: impl FnOnce(&DrawListMut<'_>) -> R,
    ) -> Option<R> {
        let caller = "NodeEditorFrame::with_node_background_draw_list()";
        self.require_scope(None, caller);
        self.with_channel_draw_list(
            caller,
            || unsafe { sys::dne_get_node_background_draw_list(node.raw()) },
            draw,
        )
    }

    /// Reserve the window draw list before asking native code to change its
    /// channel. Check the returned pointer before using the window-bound wrapper.
    fn with_channel_draw_list<R>(
        &self,
        caller: &'static str,
        select: impl FnOnce() -> *mut sys::ImDrawList,
        draw: impl FnOnce(&DrawListMut<'_>) -> R,
    ) -> Option<R> {
        self.with_canvas(caller, || {
            self.ui.with_window_draw_list(|draw_list| {
                let selected = select();
                if selected.is_null() {
                    return None;
                }
                assert_eq!(
                    selected,
                    unsafe { dear_imgui_rs::sys::igGetWindowDrawList() },
                    "{caller} returned a draw list other than the editor's host"
                );
                Some(draw(draw_list))
            })
        })
    }

    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> NodeEditorStyle {
        self.with_canvas("NodeEditorFrame::style()", NodeEditorStyle::current)
    }

    pub fn group(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::group()", "size", size);
        self.require_scope(Some(ScopeKind::Node), "NodeEditorFrame::group()");
        self.with_canvas("NodeEditorFrame::group()", || unsafe {
            sys::dne_group(vec2(size))
        });
        self.mark_group();
    }

    pub fn set_group_size(&self, node: NodeId, size: [f32; 2]) {
        assert_non_negative_finite_vec2("NodeEditorFrame::set_group_size()", "size", size);
        self.with_editor("NodeEditorFrame::set_group_size()", || unsafe {
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
        self.with_canvas("NodeEditorFrame::link_colored()", || unsafe {
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
        self.with_canvas("NodeEditorFrame::flow()", || unsafe {
            sys::dne_flow(link.raw(), direction.raw())
        });
    }

    pub fn push_style_color<'a>(
        &'a self,
        color: StyleColor,
        value: [f32; 4],
    ) -> StyleColorToken<'a> {
        assert_finite_vec4("NodeEditorFrame::push_style_color()", "value", value);
        self.with_canvas("NodeEditorFrame::push_style_color()", || unsafe {
            sys::dne_push_style_color(color.raw(), vec4(value))
        });
        StyleColorToken {
            _scope: self.entered(ScopeKind::StyleColor),
        }
    }

    pub fn push_style_var_float<'a>(&'a self, var: StyleVar, value: f32) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_float()",
            var,
            StyleVarType::Float,
        );
        assert_finite_f32("NodeEditorFrame::push_style_var_float()", "value", value);
        self.with_canvas("NodeEditorFrame::push_style_var_float()", || unsafe {
            sys::dne_push_style_var_float(var.raw(), value)
        });
        StyleVarToken {
            _scope: self.entered(ScopeKind::StyleVar),
        }
    }

    pub fn push_style_var_vec2<'a>(&'a self, var: StyleVar, value: [f32; 2]) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_vec2()",
            var,
            StyleVarType::Vec2,
        );
        assert_finite_vec2("NodeEditorFrame::push_style_var_vec2()", "value", value);
        self.with_canvas("NodeEditorFrame::push_style_var_vec2()", || unsafe {
            sys::dne_push_style_var_vec2(var.raw(), vec2(value))
        });
        StyleVarToken {
            _scope: self.entered(ScopeKind::StyleVar),
        }
    }

    pub fn push_style_var_vec4<'a>(&'a self, var: StyleVar, value: [f32; 4]) -> StyleVarToken<'a> {
        assert_style_var_type(
            "NodeEditorFrame::push_style_var_vec4()",
            var,
            StyleVarType::Vec4,
        );
        assert_finite_vec4("NodeEditorFrame::push_style_var_vec4()", "value", value);
        self.with_canvas("NodeEditorFrame::push_style_var_vec4()", || unsafe {
            sys::dne_push_style_var_vec4(var.raw(), vec4(value))
        });
        StyleVarToken {
            _scope: self.entered(ScopeKind::StyleVar),
        }
    }

    /// Draws screen-space UI while the canvas is suspended. Suspensions can nest inside a pin.
    pub fn with_suspended<R>(&self, draw: impl FnOnce() -> R) -> R {
        self.scoped(|| unsafe { self.suspend() }, |_| draw())
    }

    /// # Safety
    /// Balance host draw-list scopes around this suspension. Resume in the same window,
    /// table cell and draw channel after nested UI has closed. See [`Self::with_suspended`].
    pub unsafe fn suspend<'a>(&'a self) -> SuspensionToken<'a> {
        self.with_canvas("NodeEditorFrame::suspend()", || unsafe {
            sys::dne_suspend()
        });
        SuspensionToken {
            _scope: self.entered(ScopeKind::Suspension),
        }
    }

    pub fn is_suspended(&self) -> bool {
        self.with_canvas("NodeEditorFrame::is_suspended()", || unsafe {
            sys::dne_is_suspended()
        })
    }
}

pub struct NodeToken<'a> {
    scope: FrameToken<'a>,
}

impl NodeToken<'_> {
    /// Begins a pin whose ID is unique across this editor's current Begin/End scope.
    ///
    /// Panics before FFI if another pin is active, this node is a group, or the ID
    /// was already submitted. IDs can be reused after the editor frame ends.
    ///
    /// # Safety
    /// Keep enclosing ImGui groups and drawing scopes active until the pin ends. Balance
    /// scopes opened inside the pin first, including on unwind. See [`Self::pin`].
    pub unsafe fn begin_pin<'a>(&'a self, pin: PinId, kind: PinKind) -> PinToken<'a> {
        let frame = self.scope.frame;
        frame.require_scope(Some(ScopeKind::Node), "NodeToken::begin_pin()");
        frame.with_canvas("NodeToken::begin_pin()", || unsafe {
            assert!(
                frame.submitted_pins.borrow_mut().insert(pin),
                "NodeToken::begin_pin() requires each pin ID to be submitted only once per editor frame"
            );
            sys::dne_begin_pin(pin.raw(), kind.raw())
        });
        PinToken {
            scope: frame.entered(ScopeKind::Pin),
        }
    }

    pub fn pin<R>(&self, pin: PinId, kind: PinKind, f: impl FnOnce(&PinToken<'_>) -> R) -> R {
        self.scope
            .frame
            .scoped(|| unsafe { self.begin_pin(pin, kind) }, |token| f(token))
    }

    pub fn end(self) {}
}

pub struct PinToken<'a> {
    scope: FrameToken<'a>,
}

impl PinToken<'_> {
    pub fn end(self) {}

    pub fn rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::rect()", min, max);
        self.scope.frame.with_canvas("PinToken::rect()", || unsafe {
            sys::dne_pin_rect(vec2(min), vec2(max))
        });
    }

    pub fn pivot_rect(&self, min: [f32; 2], max: [f32; 2]) {
        assert_finite_rect("PinToken::pivot_rect()", min, max);
        self.scope
            .frame
            .with_canvas("PinToken::pivot_rect()", || unsafe {
                sys::dne_pin_pivot_rect(vec2(min), vec2(max))
            });
    }

    pub fn pivot_size(&self, size: [f32; 2]) {
        assert_non_negative_finite_vec2("PinToken::pivot_size()", "size", size);
        self.scope
            .frame
            .with_canvas("PinToken::pivot_size()", || unsafe {
                sys::dne_pin_pivot_size(vec2(size))
            });
    }

    pub fn pivot_scale(&self, scale: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_scale()", "scale", scale);
        self.scope
            .frame
            .with_canvas("PinToken::pivot_scale()", || unsafe {
                sys::dne_pin_pivot_scale(vec2(scale))
            });
    }

    pub fn pivot_alignment(&self, alignment: [f32; 2]) {
        assert_finite_vec2("PinToken::pivot_alignment()", "alignment", alignment);
        self.scope
            .frame
            .with_canvas("PinToken::pivot_alignment()", || unsafe {
                sys::dne_pin_pivot_alignment(vec2(alignment))
            });
    }
}

pub struct GroupHintToken<'a> {
    scope: FrameToken<'a>,
}

impl GroupHintToken<'_> {
    pub fn min(&self) -> [f32; 2] {
        self.scope.frame.with_canvas("GroupHintToken::min()", || {
            from_vec2(unsafe { sys::dne_get_group_min() })
        })
    }

    pub fn max(&self) -> [f32; 2] {
        self.scope.frame.with_canvas("GroupHintToken::max()", || {
            from_vec2(unsafe { sys::dne_get_group_max() })
        })
    }

    /// Selects the foreground hint channel until the draw-list borrow ends.
    ///
    /// ```compile_fail
    /// # use dear_node_editor::GroupHintToken;
    /// fn overlap(hint: &mut GroupHintToken<'_>) {
    ///     hint.with_foreground_draw_list(|_| hint.with_background_draw_list(|_| {}));
    /// }
    /// ```
    pub fn with_foreground_draw_list<R>(&mut self, draw: impl FnOnce(&DrawListMut<'_>) -> R) -> R {
        self.scope
            .frame
            .with_channel_draw_list(
                "GroupHintToken::with_foreground_draw_list()",
                || unsafe { sys::dne_get_hint_foreground_draw_list() },
                draw,
            )
            .expect("active group hint returned no foreground draw list")
    }

    /// Draw on the background channel of the active group hint.
    pub fn with_background_draw_list<R>(&mut self, draw: impl FnOnce(&DrawListMut<'_>) -> R) -> R {
        self.scope
            .frame
            .with_channel_draw_list(
                "GroupHintToken::with_background_draw_list()",
                || unsafe { sys::dne_get_hint_background_draw_list() },
                draw,
            )
            .expect("active group hint returned no background draw list")
    }

    pub fn end(self) {}
}

pub struct SuspensionToken<'a> {
    _scope: FrameToken<'a>,
}

impl SuspensionToken<'_> {
    pub fn resume(self) {}
}

/// A scoped style color. An early pop takes effect after the colors above it are popped.
pub struct StyleColorToken<'a> {
    _scope: FrameToken<'a>,
}

impl StyleColorToken<'_> {
    pub fn pop(self) {}
}

/// A scoped style variable. An early pop takes effect after the variables above it are popped.
pub struct StyleVarToken<'a> {
    _scope: FrameToken<'a>,
}

impl StyleVarToken<'_> {
    pub fn pop(self) {}
}
