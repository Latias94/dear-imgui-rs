use super::core::NodeEditorFrame;
use super::validation::{assert_finite_f32, assert_finite_vec2};
use crate::{LinkId, NodeId, PinId, from_vec2, sys, vec2};
use dear_imgui_rs::MouseButton;

impl<'ui> NodeEditorFrame<'ui> {
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

fn optional_id<T>(make: fn(usize) -> T, f: impl FnOnce(*mut usize) -> bool) -> Option<T> {
    let mut raw = 0usize;
    f(&mut raw).then_some(make(raw))
}

pub(super) fn collect_node_ids(
    count: usize,
    f: impl FnOnce(*mut usize, i32) -> i32,
) -> Vec<NodeId> {
    collect_ids(count, f).into_iter().map(NodeId).collect()
}

pub(super) fn collect_link_ids(
    count: usize,
    f: impl FnOnce(*mut usize, i32) -> i32,
) -> Vec<LinkId> {
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
}
