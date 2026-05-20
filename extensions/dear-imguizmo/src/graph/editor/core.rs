use std::collections::HashSet;

use dear_imgui_rs::{DrawListMut, Ui};

use super::super::model::{Graph, GraphView, LinkId, NodeId, PinId};
use super::super::style::GraphStyle;
use super::events::RightClickEvent;

pub struct GraphEditorUi<'ui> {
    pub(super) ui: &'ui Ui,
}

pub trait GraphEditorExt {
    fn graph_editor(&self) -> GraphEditorUi<'_>;
    fn graph_editor_config(&self) -> GraphEditor<'_>;
}
impl GraphEditorExt for Ui {
    fn graph_editor(&self) -> GraphEditorUi<'_> {
        GraphEditorUi { ui: self }
    }
    fn graph_editor_config(&self) -> GraphEditor<'_> {
        GraphEditor::new(self)
    }
}

/// Builder-style API aligned with dear-imgui patterns
pub struct GraphEditor<'ui> {
    pub(super) ui: &'ui Ui,
    pub(super) graph: Option<&'ui mut Graph>,
    pub(super) view: Option<&'ui mut GraphView>,
    pub(super) style: GraphStyle,
    // Hooks (delegate-style)
    pub(super) allowed_link_cb: Option<Box<dyn FnMut(&Graph, PinId, PinId) -> bool + 'ui>>,
    pub(super) on_select_cb: Option<Box<dyn FnMut(NodeId, bool) + 'ui>>,
    pub(super) on_move_cb: Option<Box<dyn FnMut([f32; 2], &HashSet<NodeId>) + 'ui>>,
    pub(super) on_add_link_cb: Option<Box<dyn FnMut(LinkId, PinId, PinId) + 'ui>>,
    pub(super) on_del_link_cb: Option<Box<dyn FnMut(LinkId) + 'ui>>,
    pub(super) custom_draw_cb:
        Option<Box<dyn for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui>>,
    pub(super) on_right_click_cb: Option<Box<dyn FnMut(RightClickEvent) + 'ui>>,
}
