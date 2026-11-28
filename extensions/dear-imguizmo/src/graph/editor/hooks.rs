use std::collections::HashSet;

use super::super::model::{Graph, LinkId, NodeId, PinId};
use super::super::render::CustomNodeDraw;
use super::events::RightClickEvent;

type LinkPredicate<'ui> = dyn FnMut(&Graph, PinId, PinId) -> bool + 'ui;
type MoveNodes<'ui> = dyn FnMut([f32; 2], &HashSet<NodeId>) + 'ui;
type AddLink<'ui> = dyn FnMut(LinkId, PinId, PinId) + 'ui;

#[derive(Default)]
pub(super) struct Hooks<'ui> {
    pub(super) allowed_link_cb: Option<Box<LinkPredicate<'ui>>>,
    pub(super) on_select_cb: Option<Box<dyn FnMut(NodeId, bool) + 'ui>>,
    pub(super) on_move_cb: Option<Box<MoveNodes<'ui>>>,
    pub(super) on_add_link_cb: Option<Box<AddLink<'ui>>>,
    pub(super) on_del_link_cb: Option<Box<dyn FnMut(LinkId) + 'ui>>,
    pub(super) custom_draw_cb: Option<Box<CustomNodeDraw<'ui>>>,
    pub(super) on_right_click_cb: Option<Box<dyn FnMut(RightClickEvent) + 'ui>>,
}
