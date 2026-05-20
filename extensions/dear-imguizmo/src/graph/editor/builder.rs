use std::collections::HashSet;

use dear_imgui_rs::{DrawListMut, Ui};

use super::super::model::{Graph, GraphView, LinkId, NodeId, PinId};
use super::super::style::{GraphGridMajorInterval, GraphStyle};
use super::{GraphEditor, GraphEditorResponse, Hooks, RightClickEvent, draw_core};

impl<'ui> GraphEditor<'ui> {
    pub(super) fn new(ui: &'ui Ui) -> Self {
        Self {
            ui,
            graph: None,
            view: None,
            style: GraphStyle::default(),
            allowed_link_cb: None,
            on_select_cb: None,
            on_move_cb: None,
            on_add_link_cb: None,
            on_del_link_cb: None,
            custom_draw_cb: None,
            on_right_click_cb: None,
        }
    }
    pub fn graph(mut self, graph: &'ui mut Graph) -> Self {
        self.graph = Some(graph);
        self
    }
    pub fn view(mut self, view: &'ui mut GraphView) -> Self {
        self.view = Some(view);
        self
    }

    // Style setters
    pub fn style(mut self, style: GraphStyle) -> Self {
        self.style = style;
        self
    }
    pub fn grid_visible(mut self, v: bool) -> Self {
        self.style.grid_visible = v;
        self
    }
    pub fn grid_spacing(mut self, v: f32) -> Self {
        self.style.grid_spacing = v;
        self
    }
    pub fn grid_color(mut self, c: [f32; 4]) -> Self {
        self.style.grid_color = c;
        self
    }
    pub fn grid_color_major(mut self, c: [f32; 4]) -> Self {
        self.style.grid_color2 = c;
        self
    }
    pub fn grid_major_every(mut self, interval: GraphGridMajorInterval) -> Self {
        self.style.grid_major_every = interval;
        self
    }
    pub fn node_bg_color(mut self, c: [f32; 4]) -> Self {
        self.style.node_bg_color = c;
        self
    }
    pub fn node_bg_color_hover(mut self, c: [f32; 4]) -> Self {
        self.style.node_bg_color_hover = c;
        self
    }
    pub fn node_header_color(mut self, c: [f32; 4]) -> Self {
        self.style.node_header_color = c;
        self
    }
    pub fn text_color(mut self, c: [f32; 4]) -> Self {
        self.style.text_color = c;
        self
    }
    pub fn input_pin_color(mut self, c: [f32; 4]) -> Self {
        self.style.input_pin_color = c;
        self
    }
    pub fn output_pin_color(mut self, c: [f32; 4]) -> Self {
        self.style.output_pin_color = c;
        self
    }
    pub fn link_color(mut self, c: [f32; 4]) -> Self {
        self.style.link_color = c;
        self
    }
    pub fn link_thickness(mut self, t: f32) -> Self {
        self.style.link_thickness = t;
        self
    }
    pub fn scale_link_thickness_with_zoom(mut self, v: bool) -> Self {
        self.style.scale_link_thickness_with_zoom = v;
        self
    }
    pub fn pin_radius(mut self, r: f32) -> Self {
        self.style.pin_radius = r;
        self
    }
    pub fn pin_hover_factor(mut self, f: f32) -> Self {
        self.style.pin_hover_factor = f;
        self
    }
    pub fn node_rounding(mut self, r: f32) -> Self {
        self.style.node_rounding = r;
        self
    }
    pub fn border_thickness(mut self, t: f32) -> Self {
        self.style.border_thickness = t;
        self
    }
    pub fn display_links_as_curves(mut self, v: bool) -> Self {
        self.style.display_links_as_curves = v;
        self
    }
    pub fn draw_io_name_on_hover(mut self, v: bool) -> Self {
        self.style.draw_io_name_on_hover = v;
        self
    }
    pub fn allow_quad_selection(mut self, v: bool) -> Self {
        self.style.allow_quad_selection = v;
        self
    }
    pub fn min_zoom(mut self, v: f32) -> Self {
        self.style.min_zoom = v;
        self
    }
    pub fn max_zoom(mut self, v: f32) -> Self {
        self.style.max_zoom = v;
        self
    }
    pub fn zoom_ratio(mut self, v: f32) -> Self {
        self.style.zoom_ratio = v;
        self
    }
    pub fn zoom_lerp_factor(mut self, v: f32) -> Self {
        self.style.zoom_lerp_factor = v;
        self
    }
    pub fn snap(mut self, v: f32) -> Self {
        self.style.snap = v;
        self
    }

    // Minimap
    pub fn minimap_enabled(mut self, v: bool) -> Self {
        self.style.minimap_enabled = v;
        self
    }
    pub fn minimap_rect(mut self, rect01: [f32; 4]) -> Self {
        self.style.minimap_rect = rect01;
        self
    }
    pub fn minimap_bg_color(mut self, c: [f32; 4]) -> Self {
        self.style.minimap_bg_color = c;
        self
    }
    pub fn minimap_view_fill(mut self, c: [f32; 4]) -> Self {
        self.style.minimap_view_fill = c;
        self
    }
    pub fn minimap_view_outline(mut self, c: [f32; 4]) -> Self {
        self.style.minimap_view_outline = c;
        self
    }

    // Hooks (delegate-style)
    pub fn allowed_link_fn(mut self, f: impl FnMut(&Graph, PinId, PinId) -> bool + 'ui) -> Self {
        self.allowed_link_cb = Some(Box::new(f));
        self
    }
    pub fn on_select_node(mut self, f: impl FnMut(NodeId, bool) + 'ui) -> Self {
        self.on_select_cb = Some(Box::new(f));
        self
    }
    pub fn on_move_selected_nodes(
        mut self,
        f: impl FnMut([f32; 2], &HashSet<NodeId>) + 'ui,
    ) -> Self {
        self.on_move_cb = Some(Box::new(f));
        self
    }
    pub fn on_add_link(mut self, f: impl FnMut(LinkId, PinId, PinId) + 'ui) -> Self {
        self.on_add_link_cb = Some(Box::new(f));
        self
    }
    pub fn on_del_link(mut self, f: impl FnMut(LinkId) + 'ui) -> Self {
        self.on_del_link_cb = Some(Box::new(f));
        self
    }
    pub fn custom_draw(
        mut self,
        f: impl for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui,
    ) -> Self {
        self.custom_draw_cb = Some(Box::new(f));
        self
    }
    pub fn on_right_click(mut self, f: impl FnMut(RightClickEvent) + 'ui) -> Self {
        self.on_right_click_cb = Some(Box::new(f));
        self
    }

    pub fn build(mut self) -> GraphEditorResponse {
        let mut graph_ref = match self.graph {
            Some(g) => g,
            None => return GraphEditorResponse::default(),
        };
        let mut view_ref = match self.view {
            Some(v) => v,
            None => return GraphEditorResponse::default(),
        };
        let mut hooks = Hooks {
            allowed_link_cb: self.allowed_link_cb.take(),
            on_select_cb: self.on_select_cb.take(),
            on_move_cb: self.on_move_cb.take(),
            on_add_link_cb: self.on_add_link_cb.take(),
            on_del_link_cb: self.on_del_link_cb.take(),
            custom_draw_cb: self.custom_draw_cb.take(),
            on_right_click_cb: self.on_right_click_cb.take(),
        };
        draw_core(
            self.ui,
            &mut graph_ref,
            &mut view_ref,
            &self.style,
            Some(&mut hooks),
        )
    }
}
