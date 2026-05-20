use std::collections::HashSet;

use dear_imgui_rs::DrawListMut;

use super::super::model::{Graph, LinkId, NodeId, PinId};
use super::events::RightClickEvent;

pub(super) struct Hooks<'ui> {
    pub(super) allowed_link_cb: Option<Box<dyn FnMut(&Graph, PinId, PinId) -> bool + 'ui>>,
    pub(super) on_select_cb: Option<Box<dyn FnMut(NodeId, bool) + 'ui>>,
    pub(super) on_move_cb: Option<Box<dyn FnMut([f32; 2], &HashSet<NodeId>) + 'ui>>,
    pub(super) on_add_link_cb: Option<Box<dyn FnMut(LinkId, PinId, PinId) + 'ui>>,
    pub(super) on_del_link_cb: Option<Box<dyn FnMut(LinkId) + 'ui>>,
    pub(super) custom_draw_cb:
        Option<Box<dyn for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui>>,
    pub(super) on_right_click_cb: Option<Box<dyn FnMut(RightClickEvent) + 'ui>>,
}
