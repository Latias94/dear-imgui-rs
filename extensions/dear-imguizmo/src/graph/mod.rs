use dear_imgui::{
    input::{Key, MouseButton},
    DrawListMut, Ui,
};
use std::collections::HashSet;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PinId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinkId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PinKind {
    Input,
    Output,
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub id: PinId,
    pub label: String,
    pub kind: PinKind,
    pub color: Option<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub pos: [f32; 2],
    pub title: String,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
}

impl Node {
    pub fn new<P: Vec2Like, S: Into<String>>(id: NodeId, pos: P, title: S) -> Self {
        Self {
            id,
            pos: pos.to_array(),
            title: title.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

impl Pin {
    pub fn new<S: Into<String>>(id: PinId, label: S, kind: PinKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            color: None,
        }
    }
    pub fn colored<S: Into<String>>(id: PinId, label: S, kind: PinKind, color: [f32; 4]) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            color: Some(color),
        }
    }
    pub fn set_color(&mut self, color: Option<[f32; 4]>) {
        self.color = color;
    }
}

#[derive(Clone, Debug)]
pub struct Link {
    pub id: LinkId,
    pub from: PinId,
    pub to: PinId,
}

#[derive(Default, Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    next_node_id: u32,
    next_pin_id: u32,
    next_link_id: u32,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn alloc_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        NodeId(id)
    }
    pub fn alloc_pin_id(&mut self) -> PinId {
        let id = self.next_pin_id;
        self.next_pin_id += 1;
        PinId(id)
    }
    pub fn alloc_link_id(&mut self) -> LinkId {
        let id = self.next_link_id;
        self.next_link_id += 1;
        LinkId(id)
    }
}

#[derive(Debug)]
pub struct GraphView {
    pub pan: [f32; 2],
    pub zoom: f32,
    zoom_target: f32,
    // transient state
    last_mouse: [f32; 2],
    dragging_node: Option<(NodeId, [f32; 2])>, // (node, offset in world)
    active_pin: Option<PinId>,
    // selection
    pub selected_nodes: HashSet<NodeId>,
    pub selected_links: HashSet<LinkId>,
    box_select_start: Option<[f32; 2]>,
    // hover states
    hovered_node: Option<NodeId>,
    hovered_link: Option<LinkId>,
    hovered_pin: Option<(PinId, NodeId, PinKind)>,
    // reconnect state
    reconnecting: Option<(LinkId, LinkEnd)>,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            pan: [0.0, 0.0],
            zoom: 1.0,
            zoom_target: 1.0,
            last_mouse: [0.0, 0.0],
            dragging_node: None,
            active_pin: None,
            selected_nodes: HashSet::new(),
            selected_links: HashSet::new(),
            box_select_start: None,
            hovered_node: None,
            hovered_link: None,
            hovered_pin: None,
            reconnecting: None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum LinkEnd {
    From,
    To,
}

#[derive(Default, Debug)]
pub struct GraphEditorResponse {
    pub created_links: Vec<LinkId>,
    pub right_click: Option<RightClickEvent>,
}

pub struct GraphEditorUi<'ui> {
    ui: &'ui Ui,
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
    ui: &'ui Ui,
    graph: Option<&'ui mut Graph>,
    view: Option<&'ui mut GraphView>,
    style: GraphStyle,
    // Hooks (delegate-style)
    allowed_link_cb: Option<Box<dyn FnMut(&Graph, PinId, PinId) -> bool + 'ui>>,
    on_select_cb: Option<Box<dyn FnMut(NodeId, bool) + 'ui>>,
    on_move_cb: Option<Box<dyn FnMut([f32; 2], &HashSet<NodeId>) + 'ui>>,
    on_add_link_cb: Option<Box<dyn FnMut(LinkId, PinId, PinId) + 'ui>>,
    on_del_link_cb: Option<Box<dyn FnMut(LinkId) + 'ui>>,
    custom_draw_cb: Option<Box<dyn for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui>>,
    on_right_click_cb: Option<Box<dyn FnMut(RightClickEvent) + 'ui>>,
}

impl<'ui> GraphEditor<'ui> {
    fn new(ui: &'ui Ui) -> Self {
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
    pub fn grid_major_every(mut self, v: i32) -> Self {
        self.style.grid_major_every = v;
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

impl<'ui> GraphEditorUi<'ui> {
    pub fn draw(&self, graph: &mut Graph, view: &mut GraphView) -> GraphEditorResponse {
        draw_core(self.ui, graph, view, &GraphStyle::default(), None)
    }
}

struct Hooks<'ui> {
    allowed_link_cb: Option<Box<dyn FnMut(&Graph, PinId, PinId) -> bool + 'ui>>,
    on_select_cb: Option<Box<dyn FnMut(NodeId, bool) + 'ui>>,
    on_move_cb: Option<Box<dyn FnMut([f32; 2], &HashSet<NodeId>) + 'ui>>,
    on_add_link_cb: Option<Box<dyn FnMut(LinkId, PinId, PinId) + 'ui>>,
    on_del_link_cb: Option<Box<dyn FnMut(LinkId) + 'ui>>,
    custom_draw_cb: Option<Box<dyn for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui>>,
    on_right_click_cb: Option<Box<dyn FnMut(RightClickEvent) + 'ui>>,
}

fn draw_core<'ui, 'h>(
    ui: &'ui Ui,
    graph: &mut Graph,
    view: &mut GraphView,
    style: &GraphStyle,
    mut hooks: Option<&'h mut Hooks<'ui>>,
) -> GraphEditorResponse {
    let mut resp = GraphEditorResponse::default();
    let dl = ui.get_window_draw_list();
    let origin = ui.window_pos();
    let size = ui.window_size();

    // background fill
    dl.add_rect(
        [origin[0], origin[1]],
        [origin[0] + size[0], origin[1] + size[1]],
        style.background_color,
    )
    .filled(true)
    .build();

    // grid
    if style.grid_visible {
        draw_grid(&dl, origin, size, view, style);
    }

    // links first (under nodes)
    for ln in &graph.links {
        if let (Some((np1, pk1, idx1)), Some((np2, pk2, idx2))) =
            (find_pin(graph, ln.from), find_pin(graph, ln.to))
        {
            let p0 = pin_screen_pos(origin, view, np1, pk1, idx1, style);
            let p1s = pin_screen_pos(origin, view, np2, pk2, idx2, style);
            let sel = view.selected_links.contains(&ln.id);
            let hov = view.hovered_link == Some(ln.id);
            let link_touches_hovered_node = match view.hovered_node {
                Some(nid) => np1.id == nid || np2.id == nid,
                None => false,
            };
            let (color, thick) = if sel {
                (style.selected_outline_color, style.link_thickness + 1.0)
            } else if hov || link_touches_hovered_node {
                (style.hover_outline_color, style.link_thickness)
            } else {
                (style.link_color, style.link_thickness)
            };
            draw_link_variant(&dl, p0, p1s, color, thick, view.zoom, style);
        }
    }

    // nodes
    let io = ui.io();
    let mouse = io.mouse_pos();
    let mouse_delta = [mouse[0] - view.last_mouse[0], mouse[1] - view.last_mouse[1]];
    let left_down = io.mouse_down(0);
    let middle_down = io.mouse_down(2);
    let right_down = io.mouse_down(1);
    let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
    let shift = ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift);
    let left_clicked = ui.is_mouse_clicked(MouseButton::Left);
    let left_released = ui.is_mouse_released(MouseButton::Left);
    let right_clicked = ui.is_mouse_clicked(MouseButton::Right);

    // Zoom via mouse wheel with smoothing, keep mouse world pos stable
    let region_min = origin;
    let region_max = [origin[0] + size[0], origin[1] + size[1]];
    let mouse_in_region = mouse[0] >= region_min[0]
        && mouse[0] <= region_max[0]
        && mouse[1] >= region_min[1]
        && mouse[1] <= region_max[1];
    if mouse_in_region && io.mouse_wheel().abs() > f32::EPSILON {
        if io.mouse_wheel() < 0.0 {
            view.zoom_target *= 1.0 - style.zoom_ratio;
        }
        if io.mouse_wheel() > 0.0 {
            view.zoom_target *= 1.0 + style.zoom_ratio;
        }
        view.zoom_target = view.zoom_target.clamp(style.min_zoom, style.max_zoom);
        let pre = screen_to_world(mouse, origin, view);
        view.zoom = view.zoom + (view.zoom_target - view.zoom) * style.zoom_lerp_factor;
        let post = screen_to_world(mouse, origin, view);
        view.pan[0] += (post[0] - pre[0]) * view.zoom;
        view.pan[1] += (post[1] - pre[1]) * view.zoom;
    } else {
        // Smooth towards target even when wheel not moving
        if (view.zoom - view.zoom_target).abs() > 0.0001 {
            view.zoom = view.zoom + (view.zoom_target - view.zoom) * style.zoom_lerp_factor;
        }
    }

    // Hover detection priority: pin > node > link
    view.hovered_pin = hit_pin(graph, origin, view, mouse, style);
    if view.hovered_pin.is_none() {
        view.hovered_node = hit_node_rect(graph, origin, view, mouse);
    } else {
        view.hovered_node = None;
    }
    if view.hovered_pin.is_none() && view.hovered_node.is_none() {
        view.hovered_link = hit_link(graph, origin, view, mouse, style).map(|(id, _)| id);
    } else {
        view.hovered_link = None;
    }

    // panning with middle or right mouse
    if middle_down || right_down {
        view.pan[0] += mouse_delta[0];
        view.pan[1] += mouse_delta[1];
    }

    // node dragging
    if left_down {
        if view.dragging_node.is_none() {
            if let Some(nid) = view.hovered_node {
                // selection behavior on click node
                if !ctrl && !view.selected_nodes.contains(&nid) {
                    // deselect others
                    let prev: Vec<NodeId> = view.selected_nodes.iter().copied().collect();
                    view.selected_nodes.clear();
                    if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut()) {
                        for id in prev {
                            (h)(id, false);
                        }
                    }
                }
                if ctrl {
                    if view.selected_nodes.contains(&nid) {
                        view.selected_nodes.remove(&nid);
                        if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut())
                        {
                            (h)(nid, false);
                        }
                    } else {
                        view.selected_nodes.insert(nid);
                        if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut())
                        {
                            (h)(nid, true);
                        }
                    }
                } else {
                    view.selected_nodes.insert(nid);
                    if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut()) {
                        (h)(nid, true);
                    }
                }
                view.dragging_node = Some((nid, [0.0, 0.0]));
            } else if let Some(lid) = view.hovered_link {
                // link click selection
                if !ctrl && !view.selected_links.contains(&lid) {
                    view.selected_nodes.clear();
                    view.selected_links.clear();
                }
                if ctrl {
                    if view.selected_links.contains(&lid) {
                        view.selected_links.remove(&lid);
                    } else {
                        view.selected_links.insert(lid);
                    }
                } else {
                    view.selected_links.insert(lid);
                }
            } else if view.hovered_pin.is_none() && style.allow_quad_selection {
                // start box select on empty click
                if !ctrl && !shift {
                    view.selected_nodes.clear();
                    view.selected_links.clear();
                }
                if view.box_select_start.is_none() {
                    view.box_select_start = Some(mouse);
                }
            }
        }
    } else {
        view.dragging_node = None;
        // complete box selection if any
        if let Some(start) = view.box_select_start.take() {
            let rect = rect_from_points(start, mouse);
            // apply selection
            for n in &graph.nodes {
                let nrect = node_rect_screen(origin, view, n);
                if rects_intersect(rect, nrect) {
                    if ctrl {
                        if view.selected_nodes.contains(&n.id) {
                            view.selected_nodes.remove(&n.id);
                            if let Some(h) =
                                hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut())
                            {
                                (h)(n.id, false);
                            }
                        } else {
                            view.selected_nodes.insert(n.id);
                            if let Some(h) =
                                hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut())
                            {
                                (h)(n.id, true);
                            }
                        }
                    } else {
                        if !view.selected_nodes.contains(&n.id) {
                            view.selected_nodes.insert(n.id);
                            if let Some(h) =
                                hooks.as_deref_mut().and_then(|h| h.on_select_cb.as_mut())
                            {
                                (h)(n.id, true);
                            }
                        }
                    }
                }
            }
            // select links whose both endpoints are inside rect
            for ln in &graph.links {
                if let (Some((np1, pk1, idx1)), Some((np2, pk2, idx2))) =
                    (find_pin(graph, ln.from), find_pin(graph, ln.to))
                {
                    let p0 = pin_screen_pos(origin, view, np1, pk1, idx1, style);
                    let p1s = pin_screen_pos(origin, view, np2, pk2, idx2, style);
                    if bezier_intersects_rect(p0, p1s, rect) {
                        view.selected_links.insert(ln.id);
                    }
                }
            }
            // If SHIFT not held, unselect nodes/links outside rect
            if !shift {
                for n in &graph.nodes {
                    let inside = rects_intersect(rect, node_rect_screen(origin, view, n));
                    if !inside {
                        view.selected_nodes.remove(&n.id);
                    }
                }
                // Links: keep selection only for those intersecting rect
                let selected_links: Vec<LinkId> = view.selected_links.iter().copied().collect();
                for lid in selected_links {
                    if let Some(ln) = graph.links.iter().find(|l| l.id == lid) {
                        if let (Some((np1, pk1, idx1)), Some((np2, pk2, idx2))) =
                            (find_pin(graph, ln.from), find_pin(graph, ln.to))
                        {
                            let p0 = pin_screen_pos(origin, view, np1, pk1, idx1, style);
                            let p1s = pin_screen_pos(origin, view, np2, pk2, idx2, style);
                            if !bezier_intersects_rect(p0, p1s, rect) {
                                view.selected_links.remove(&lid);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some((nid, _)) = view.dragging_node {
        let delta_world = [mouse_delta[0] / view.zoom, mouse_delta[1] / view.zoom];
        let move_set = if view.selected_nodes.is_empty() {
            let mut s = HashSet::new();
            s.insert(nid);
            s
        } else {
            view.selected_nodes.clone()
        };
        for node in graph.nodes.iter_mut() {
            if move_set.contains(&node.id) {
                node.pos[0] += delta_world[0];
                node.pos[1] += delta_world[1];
                if style.snap > 0.0 {
                    node.pos[0] = (node.pos[0] / style.snap).round() * style.snap;
                    node.pos[1] = (node.pos[1] / style.snap).round() * style.snap;
                }
            }
        }
        if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_move_cb.as_mut()) {
            (h)(delta_world, &move_set);
        }
    }

    // pin interaction (create link)
    if left_down {
        if view.active_pin.is_none() {
            if let Some((pin_id, _node_id, _kind)) = hit_pin(graph, origin, view, mouse, style) {
                view.active_pin = Some(pin_id);
            }
        }
    } else {
        if let Some(start_pin) = view.active_pin.take() {
            if let Some((end_pin, _, _)) = hit_pin(graph, origin, view, mouse, style) {
                if can_connect(graph, start_pin, end_pin)
                    && hooks
                        .as_deref_mut()
                        .and_then(|h| h.allowed_link_cb.as_mut())
                        .map(|f| f(graph, start_pin, end_pin))
                        .unwrap_or(true)
                {
                    let id = graph.alloc_link_id();
                    let (from, to) = normalize_link_direction(graph, start_pin, end_pin);
                    graph.links.push(Link { id, from, to });
                    resp.created_links.push(id);
                    if let Some(h) = hooks.as_deref_mut().and_then(|h| h.on_add_link_cb.as_mut()) {
                        (h)(id, from, to);
                    }
                }
            }
        }
    }

    // render link being created
    if let Some(pin_id) = view.active_pin {
        if let Some((node, kind, idx)) = find_pin(graph, pin_id) {
            let start = pin_screen_pos(origin, view, node, kind, idx, style);
            draw_link_variant(
                &dl,
                start,
                mouse,
                [0.78, 0.78, 0.78, 1.0],
                style.link_thickness,
                view.zoom,
                style,
            );
        }
    }

    // Reconnect interactions: click near link endpoint -> drag -> release on pin to reattach
    if left_clicked && view.reconnecting.is_none() {
        if let Some(lid) = view.hovered_link {
            if let Some(end) = near_link_end(graph, origin, view, style, lid, mouse) {
                view.reconnecting = Some((lid, end));
            }
        }
    }
    if left_down {
        if let Some((lid, end)) = view.reconnecting {
            if let Some((np1, pk1, idx1, np2, pk2, idx2)) = link_endpoints(graph, lid) {
                let (fixed, _) = match end {
                    LinkEnd::From => (
                        pin_screen_pos(origin, view, np2, pk2, idx2, style),
                        pin_screen_pos(origin, view, np1, pk1, idx1, style),
                    ),
                    LinkEnd::To => (
                        pin_screen_pos(origin, view, np1, pk1, idx1, style),
                        pin_screen_pos(origin, view, np2, pk2, idx2, style),
                    ),
                };
                draw_link_variant(
                    &dl,
                    fixed,
                    mouse,
                    style.selected_outline_color,
                    style.link_thickness + 1.0,
                    view.zoom,
                    style,
                );
            }
        }
    }
    if left_released {
        if let Some((lid, end)) = view.reconnecting.take() {
            if let Some((pin, _nid, _kind)) = view.hovered_pin {
                // Stage candidate update without holding mutable borrow across can_connect
                if let Some(idx) = graph.links.iter().position(|l| l.id == lid) {
                    let (old_from, old_to) = {
                        let ln = &graph.links[idx];
                        (ln.from, ln.to)
                    };
                    let (cand_from, cand_to) = match end {
                        LinkEnd::From => (pin, old_to),
                        LinkEnd::To => (old_from, pin),
                    };
                    if can_connect(graph, cand_from, cand_to)
                        && hooks
                            .as_deref_mut()
                            .and_then(|h| h.allowed_link_cb.as_mut())
                            .map(|f| f(graph, cand_from, cand_to))
                            .unwrap_or(true)
                    {
                        let ln = &mut graph.links[idx];
                        match end {
                            LinkEnd::From => ln.from = pin,
                            LinkEnd::To => ln.to = pin,
                        }
                    }
                }
            }
        }
    }

    // draw nodes & pins on top
    for node in &graph.nodes {
        let selected = view.selected_nodes.contains(&node.id);
        let hovered = view.hovered_node == Some(node.id);
        draw_node(
            ui,
            hooks.as_deref_mut().and_then(|h| h.custom_draw_cb.as_mut()),
            &dl,
            origin,
            view,
            node,
            style,
            selected,
            hovered,
        );
    }

    // draw selection rectangle if active
    if let Some(start) = view.box_select_start {
        let rect = rect_from_points(start, mouse);
        dl.add_rect(
            [rect[0], rect[1]],
            [rect[2], rect[3]],
            style.selection_rect_color,
        )
        .build();
    }

    // delete key handling
    if ui.is_key_pressed(Key::Delete) {
        let mut opt_cb = hooks
            .as_deref_mut()
            .and_then(|h| h.on_del_link_cb.as_mut())
            .map(|f| f as &mut dyn FnMut(LinkId));
        match opt_cb.as_deref_mut() {
            Some(cb) => delete_selected_core(graph, view, Some(cb)),
            None => delete_selected_core(graph, view, None),
        }
    }

    // right click reporting
    if right_clicked && mouse_in_region {
        let mut rc_node = view.hovered_node;
        let mut rc_pin: Option<(PinId, PinKind)> = None;
        if let Some((pid, _nid, kind)) = view.hovered_pin {
            rc_pin = Some((pid, kind));
        }
        let evt = RightClickEvent {
            node: rc_node,
            pin: rc_pin,
            mouse_pos: mouse,
        };
        resp.right_click = Some(evt);
        if let Some(h) = hooks
            .as_deref_mut()
            .and_then(|h| h.on_right_click_cb.as_mut())
        {
            (h)(resp.right_click.unwrap());
        }
    }

    // store last mouse
    view.last_mouse = mouse;
    // minimap
    if style.minimap_enabled {
        draw_minimap(ui, &dl, graph, view, style, origin, size);
    }

    resp
}

#[derive(Copy, Clone, Debug)]
pub struct GraphStyle {
    pub background_color: [f32; 4],
    pub grid_visible: bool,
    pub grid_spacing: f32,
    pub grid_color: [f32; 4],
    pub grid_color2: [f32; 4],
    pub grid_major_every: i32,
    pub node_bg_color: [f32; 4],
    pub node_bg_color_hover: [f32; 4],
    pub node_header_color: [f32; 4],
    pub text_color: [f32; 4],
    pub input_pin_color: [f32; 4],
    pub output_pin_color: [f32; 4],
    pub link_color: [f32; 4],
    pub link_thickness: f32,
    /// If true, scale link thickness by zoom for consistency
    pub scale_link_thickness_with_zoom: bool,
    pub pin_radius: f32,
    pub pin_hover_factor: f32,
    pub node_rounding: f32,
    pub border_thickness: f32,
    pub selected_outline_color: [f32; 4],
    pub selected_outline_thickness: f32,
    pub selection_rect_color: [f32; 4],
    pub pin_hover_color: [f32; 4],
    pub hover_outline_color: [f32; 4],
    pub draw_io_name_on_hover: bool,
    pub display_links_as_curves: bool,
    pub allow_quad_selection: bool,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub zoom_ratio: f32,
    pub zoom_lerp_factor: f32,
    pub snap: f32,
    // Minimap
    pub minimap_enabled: bool,
    /// [xmin, ymin, xmax, ymax] in [0,1] window space
    pub minimap_rect: [f32; 4],
    pub minimap_bg_color: [f32; 4],
    pub minimap_view_fill: [f32; 4],
    pub minimap_view_outline: [f32; 4],
}

impl Default for GraphStyle {
    fn default() -> Self {
        Self {
            background_color: [0.16, 0.16, 0.16, 1.0],
            grid_visible: true,
            grid_spacing: 32.0,
            grid_color: [0.8, 0.8, 0.8, 0.2],
            grid_color2: [0.5, 0.5, 0.5, 0.35],
            grid_major_every: 10,
            node_bg_color: [0.35, 0.35, 0.35, 1.0],
            node_bg_color_hover: [0.40, 0.40, 0.46, 1.0],
            node_header_color: [0.24, 0.24, 0.32, 1.0],
            text_color: [0.90, 0.90, 0.90, 1.0],
            input_pin_color: [0.39, 0.78, 0.39, 1.0],
            output_pin_color: [0.78, 0.78, 0.39, 1.0],
            link_color: [0.71, 0.71, 0.47, 1.0],
            link_thickness: 2.0,
            scale_link_thickness_with_zoom: false,
            pin_radius: 5.0,
            pin_hover_factor: 1.2,
            node_rounding: 4.0,
            border_thickness: 1.0,
            selected_outline_color: [0.95, 0.85, 0.30, 1.0],
            selected_outline_thickness: 2.0,
            selection_rect_color: [0.90, 0.90, 0.90, 0.6],
            pin_hover_color: [0.95, 0.75, 0.25, 1.0],
            hover_outline_color: [0.80, 0.80, 0.95, 1.0],
            draw_io_name_on_hover: false,
            display_links_as_curves: true,
            allow_quad_selection: true,
            min_zoom: 0.2,
            max_zoom: 1.2,
            zoom_ratio: 0.1,
            zoom_lerp_factor: 0.25,
            snap: 0.0,
            minimap_enabled: true,
            minimap_rect: [0.75, 0.80, 0.99, 0.99],
            minimap_bg_color: [0.12, 0.12, 0.12, 0.8],
            minimap_view_fill: [1.0, 1.0, 1.0, 0.25],
            minimap_view_outline: [1.0, 1.0, 1.0, 0.6],
        }
    }
}

// helpers
fn world_to_screen(p: [f32; 2], origin: [f32; 2], view: &GraphView) -> [f32; 2] {
    [
        p[0] * view.zoom + view.pan[0] + origin[0],
        p[1] * view.zoom + view.pan[1] + origin[1],
    ]
}

// Simple 2D vector adaptor for glam/mint/[f32;2]
pub trait Vec2Like {
    fn to_array(self) -> [f32; 2];
}
impl Vec2Like for [f32; 2] {
    fn to_array(self) -> [f32; 2] {
        self
    }
}
impl Vec2Like for (f32, f32) {
    fn to_array(self) -> [f32; 2] {
        [self.0, self.1]
    }
}
#[cfg(feature = "glam")]
impl Vec2Like for glam::Vec2 {
    fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}
#[cfg(feature = "mint")]
impl Vec2Like for mint::Vector2<f32> {
    fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}
fn screen_to_world(p: [f32; 2], origin: [f32; 2], view: &GraphView) -> [f32; 2] {
    [
        (p[0] - origin[0] - view.pan[0]) / view.zoom,
        (p[1] - origin[1] - view.pan[1]) / view.zoom,
    ]
}

fn draw_grid(
    dl: &DrawListMut,
    origin: [f32; 2],
    size: [f32; 2],
    view: &GraphView,
    style: &GraphStyle,
) {
    let grid = style.grid_spacing * view.zoom.max(0.0001);
    let start_x = ((-view.pan[0] - origin[0]) % grid + grid) % grid;
    let start_y = ((-view.pan[1] - origin[1]) % grid + grid) % grid;
    let base_i = (((-view.pan[0] - origin[0]) / grid).floor()) as i32;
    let base_j = (((-view.pan[1] - origin[1]) / grid).floor()) as i32;
    let x_count = (size[0] / grid + 2.0) as i32;
    let y_count = (size[1] / grid + 2.0) as i32;
    for i in 0..x_count {
        let x = origin[0] + start_x + i as f32 * grid;
        let idx = base_i + i;
        let color = if style.grid_major_every > 0 && (idx % style.grid_major_every) == 0 {
            style.grid_color2
        } else {
            style.grid_color
        };
        dl.add_line([x, origin[1]], [x, origin[1] + size[1]], color)
            .build();
    }
    for j in 0..y_count {
        let y = origin[1] + start_y + j as f32 * grid;
        let idy = base_j + j;
        let color = if style.grid_major_every > 0 && (idy % style.grid_major_every) == 0 {
            style.grid_color2
        } else {
            style.grid_color
        };
        dl.add_line([origin[0], y], [origin[0] + size[0], y], color)
            .build();
    }
}

fn node_size(node: &Node) -> [f32; 2] {
    let w = 160.0;
    let header = 24.0;
    let line = 20.0;
    let pads = 8.0;
    let rows = node.inputs.len().max(node.outputs.len()) as f32;
    [w, header + rows * line + pads]
}

fn draw_node<'ui>(
    ui: &'ui Ui,
    mut custom: Option<&mut Box<dyn for<'a> FnMut(&DrawListMut<'a>, [f32; 4], NodeId) + 'ui>>,
    dl: &DrawListMut,
    origin: [f32; 2],
    view: &GraphView,
    node: &Node,
    style: &GraphStyle,
    selected: bool,
    hovered_node: bool,
) {
    let pos = world_to_screen(node.pos, origin, view);
    let sz = node_size(node);
    let rect = [
        pos[0],
        pos[1],
        pos[0] + sz[0] * view.zoom,
        pos[1] + sz[1] * view.zoom,
    ];
    let bg = if hovered_node {
        style.node_bg_color_hover
    } else {
        style.node_bg_color
    };
    dl.add_rect([rect[0], rect[1]], [rect[2], rect[3]], bg)
        .filled(true)
        .rounding(style.node_rounding * view.zoom)
        .build();
    // header
    dl.add_rect(
        [rect[0], rect[1]],
        [rect[2], rect[1] + 24.0 * view.zoom],
        style.node_header_color,
    )
    .filled(true)
    .rounding(style.node_rounding * view.zoom)
    .build();
    dl.add_text(
        [rect[0] + 8.0, rect[1] + 4.0],
        style.text_color,
        &node.title,
    );
    // border
    let border_col = if selected {
        style.selected_outline_color
    } else {
        style.hover_outline_color
    };
    let border_thick = if selected {
        style.selected_outline_thickness
    } else {
        style.border_thickness
    };
    if selected || hovered_node {
        dl.add_rect(
            [rect[0] - 1.0, rect[1] - 1.0],
            [rect[2] + 1.0, rect[3] + 1.0],
            border_col,
        )
        .thickness(border_thick)
        .rounding(style.node_rounding * view.zoom)
        .build();
    }
    // pins
    let line = 20.0 * view.zoom;
    let base_y = rect[1] + 24.0 * view.zoom + 10.0;
    for (i, p) in node.inputs.iter().enumerate() {
        let cy = base_y + i as f32 * line;
        let pin_pos = [rect[0] + 6.0, cy];
        let hovered = matches!(view.hovered_pin, Some((pid, nid, PinKind::Input)) if pid==p.id && nid==node.id);
        let base = p.color.unwrap_or(style.input_pin_color);
        let color = if hovered { style.pin_hover_color } else { base };
        let radius =
            style.pin_radius * view.zoom * if hovered { style.pin_hover_factor } else { 1.0 };
        dl.add_circle(pin_pos, radius, color).filled(true).build();
        if !style.draw_io_name_on_hover || hovered {
            dl.add_text([rect[0] + 14.0, cy - 7.0], style.text_color, &p.label);
        }
    }
    for (i, p) in node.outputs.iter().enumerate() {
        let cy = base_y + i as f32 * line;
        let pin_pos = [rect[2] - 6.0, cy];
        let hovered = matches!(view.hovered_pin, Some((pid, nid, PinKind::Output)) if pid==p.id && nid==node.id);
        let base = p.color.unwrap_or(style.output_pin_color);
        let color = if hovered { style.pin_hover_color } else { base };
        let radius =
            style.pin_radius * view.zoom * if hovered { style.pin_hover_factor } else { 1.0 };
        dl.add_circle(pin_pos, radius, color).filled(true).build();
        let tw = 60.0; // rough text offset
        if !style.draw_io_name_on_hover || hovered {
            dl.add_text([rect[2] - 14.0 - tw, cy - 7.0], style.text_color, &p.label);
        }
    }

    // Custom draw area inside node body
    let custom_min = [
        rect[0] + style.node_rounding * view.zoom,
        rect[1] + 20.0 * view.zoom + style.node_rounding * view.zoom,
    ];
    let custom_max = [
        rect[2] - style.node_rounding * view.zoom,
        rect[3] - style.node_rounding * view.zoom,
    ];
    if custom_min[0] < custom_max[0] && custom_min[1] < custom_max[1] {
        if let Some(cb) = custom.as_deref_mut() {
            ui.push_clip_rect(custom_min, custom_max, true);
            (*cb)(
                dl,
                [custom_min[0], custom_min[1], custom_max[0], custom_max[1]],
                node.id,
            );
            ui.pop_clip_rect();
        }
    }
}

fn find_pin<'a>(graph: &'a Graph, pid: PinId) -> Option<(&'a Node, PinKind, usize)> {
    for n in &graph.nodes {
        if let Some(idx) = n.inputs.iter().position(|p| p.id == pid) {
            return Some((n, PinKind::Input, idx));
        }
        if let Some(idx) = n.outputs.iter().position(|p| p.id == pid) {
            return Some((n, PinKind::Output, idx));
        }
    }
    None
}

fn pin_screen_pos(
    origin: [f32; 2],
    view: &GraphView,
    node: &Node,
    kind: PinKind,
    index: usize,
    _style: &GraphStyle,
) -> [f32; 2] {
    let pos = world_to_screen(node.pos, origin, view);
    let sz = node_size(node);
    let line = 20.0 * view.zoom;
    let base_y = pos[1] + 24.0 * view.zoom + 10.0;
    match kind {
        PinKind::Input => [pos[0] + 6.0, base_y + index as f32 * line],
        PinKind::Output => [
            pos[0] + sz[0] * view.zoom - 6.0,
            base_y + index as f32 * line,
        ],
    }
}

fn can_connect(graph: &Graph, a: PinId, b: PinId) -> bool {
    if a == b {
        return false;
    }
    match (find_pin(graph, a), find_pin(graph, b)) {
        (Some((na, ka, _)), Some((nb, kb, _))) => ka != kb && na.id != nb.id,
        _ => false,
    }
}

fn normalize_link_direction(graph: &Graph, a: PinId, b: PinId) -> (PinId, PinId) {
    let ka = find_pin(graph, a).map(|(_, k, _)| k);
    match ka {
        Some(PinKind::Output) => (a, b),
        Some(PinKind::Input) => (b, a),
        _ => (a, b),
    }
}

fn draw_link_cubic(dl: &DrawListMut, a: [f32; 2], b: [f32; 2], color: [f32; 4], thickness: f32) {
    let dx = (b[0] - a[0]).abs();
    let c1 = [a[0] + dx * 0.5, a[1]];
    let c2 = [b[0] - dx * 0.5, b[1]];
    dl.add_bezier_curve(a, c1, c2, b, color)
        .thickness(thickness)
        .build();
}

fn draw_link_straight(dl: &DrawListMut, a: [f32; 2], b: [f32; 2], color: [f32; 4], thickness: f32) {
    // Simplified polyline similar to C++ logic
    let dif = [b[0] - a[0], b[1] - a[1]];
    let mut pts: Vec<[f32; 2]> = Vec::new();
    let limitx = 12.0;
    if dif[0] < limitx {
        let p10 = [a[0] + limitx, a[1]];
        let p20 = [b[0] - limitx, b[1]];
        let dif2 = [p20[0] - p10[0], p20[1] - p10[1]];
        let p1a = [p10[0], p10[1] + dif2[1] * 0.5];
        let p1b = [p1a[0] + dif2[0], p1a[1]];
        pts.extend_from_slice(&[a, p10, p1a, p1b, p20, b]);
    } else if dif[1].abs() < 1.0 {
        pts.extend_from_slice(&[a, [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], b]);
    } else {
        // four segments variant
        let (p1a, p1b) = if dif[0].abs() > dif[1].abs() {
            let d = dif[1].abs() * dif[0].signum() * 0.5;
            let p1a = [a[0] + d, a[1] + dif[1] * 0.5];
            let p1b = [
                p1a[0] + (dif[0].abs() - d.abs() * 2.0).abs() * dif[0].signum(),
                p1a[1],
            ];
            (p1a, p1b)
        } else {
            let d = dif[0].abs() * dif[1].signum() * 0.5;
            let p1a = [a[0] + dif[0] * 0.5, a[1] + d];
            let p1b = [
                p1a[0],
                p1a[1] + (dif[1].abs() - d.abs() * 2.0).abs() * dif[1].signum(),
            ];
            (p1a, p1b)
        };
        pts.extend_from_slice(&[a, p1a, p1b, b]);
    }
    dl.add_polyline(pts, color).thickness(thickness).build();
}

fn draw_link_variant(
    dl: &DrawListMut,
    a: [f32; 2],
    b: [f32; 2],
    color: [f32; 4],
    thickness: f32,
    zoom: f32,
    style: &GraphStyle,
) {
    let mut th = thickness;
    if style.scale_link_thickness_with_zoom {
        // mildly scale with zoom for visual consistency
        // clamp scaling to avoid extremes at very low/high zoom
        let scale = zoom.clamp(0.6, 1.8);
        th = thickness * scale;
    }
    if style.display_links_as_curves {
        draw_link_cubic(dl, a, b, color, th);
    } else {
        draw_link_straight(dl, a, b, color, th);
    }
}

// hit_node(): replaced by hit_node_rect() for simpler usage; keep code minimal

fn hit_node_rect(
    graph: &Graph,
    origin: [f32; 2],
    view: &GraphView,
    mouse: [f32; 2],
) -> Option<NodeId> {
    for n in graph.nodes.iter().rev() {
        let r = node_rect_screen(origin, view, n);
        if point_in_rect(mouse, r) {
            return Some(n.id);
        }
    }
    None
}

fn hit_pin(
    graph: &Graph,
    origin: [f32; 2],
    view: &GraphView,
    mouse: [f32; 2],
    style: &GraphStyle,
) -> Option<(PinId, NodeId, PinKind)> {
    let radius = (style.pin_radius + 2.0) * view.zoom;
    for n in &graph.nodes {
        for (i, p) in n.inputs.iter().enumerate() {
            let ppos = pin_screen_pos(origin, view, n, PinKind::Input, i, style);
            let d2 = (mouse[0] - ppos[0]).powi(2) + (mouse[1] - ppos[1]).powi(2);
            if d2 <= radius * radius {
                return Some((p.id, n.id, PinKind::Input));
            }
        }
        for (i, p) in n.outputs.iter().enumerate() {
            let ppos = pin_screen_pos(origin, view, n, PinKind::Output, i, style);
            let d2 = (mouse[0] - ppos[0]).powi(2) + (mouse[1] - ppos[1]).powi(2);
            if d2 <= radius * radius {
                return Some((p.id, n.id, PinKind::Output));
            }
        }
    }
    None
}

fn hit_link(
    graph: &Graph,
    origin: [f32; 2],
    view: &GraphView,
    mouse: [f32; 2],
    style: &GraphStyle,
) -> Option<(LinkId, [f32; 2])> {
    let mut best: Option<(LinkId, f32, [f32; 2])> = None;
    for ln in &graph.links {
        if let (Some((np1, pk1, idx1)), Some((np2, pk2, idx2))) =
            (find_pin(graph, ln.from), find_pin(graph, ln.to))
        {
            let a = pin_screen_pos(origin, view, np1, pk1, idx1, style);
            let b = pin_screen_pos(origin, view, np2, pk2, idx2, style);
            let dx = (b[0] - a[0]).abs();
            let c1 = [a[0] + dx * 0.5, a[1]];
            let c2 = [b[0] - dx * 0.5, b[1]];
            let (d, p) = bezier_hit_distance(a, c1, c2, b, mouse);
            let thresh = 6.0 + style.link_thickness;
            if d <= thresh {
                match best {
                    Some((_, bd, _)) if bd <= d => {}
                    _ => {
                        best = Some((ln.id, d, p));
                    }
                }
            }
        }
    }
    best.map(|(id, _, p)| (id, p))
}

fn near_link_end(
    graph: &Graph,
    origin: [f32; 2],
    view: &GraphView,
    style: &GraphStyle,
    lid: LinkId,
    mouse: [f32; 2],
) -> Option<LinkEnd> {
    if let Some((np1, pk1, idx1, np2, pk2, idx2)) = link_endpoints(graph, lid) {
        let a = pin_screen_pos(origin, view, np1, pk1, idx1, style);
        let b = pin_screen_pos(origin, view, np2, pk2, idx2, style);
        let thr = (style.pin_radius + 4.0) * view.zoom;
        let da2 = (mouse[0] - a[0]).powi(2) + (mouse[1] - a[1]).powi(2);
        let db2 = (mouse[0] - b[0]).powi(2) + (mouse[1] - b[1]).powi(2);
        if da2.sqrt() <= thr {
            return Some(LinkEnd::From);
        }
        if db2.sqrt() <= thr {
            return Some(LinkEnd::To);
        }
    }
    None
}

fn link_endpoints<'a>(
    graph: &'a Graph,
    lid: LinkId,
) -> Option<(&'a Node, PinKind, usize, &'a Node, PinKind, usize)> {
    let ln = graph.links.iter().find(|l| l.id == lid)?;
    let (np1, pk1, idx1) = find_pin(graph, ln.from)?;
    let (np2, pk2, idx2) = find_pin(graph, ln.to)?;
    Some((np1, pk1, idx1, np2, pk2, idx2))
}

fn bezier_hit_distance(
    a: [f32; 2],
    c1: [f32; 2],
    c2: [f32; 2],
    b: [f32; 2],
    pt: [f32; 2],
) -> (f32, [f32; 2]) {
    // sample along the curve and compute min distance to segments
    let mut prev = a;
    let mut best_d2 = f32::MAX;
    let mut best_p = a;
    let steps = 24;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = cubic_bezier_point(a, c1, c2, b, t);
        let (d2, cp) = point_segment_distance2(prev, p, pt);
        if d2 < best_d2 {
            best_d2 = d2;
            best_p = cp;
        }
        prev = p;
    }
    (best_d2.sqrt(), best_p)
}

fn cubic_bezier_point(a: [f32; 2], c1: [f32; 2], c2: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    let mut p = [0.0, 0.0];
    p[0] = uuu * a[0] + 3.0 * uu * t * c1[0] + 3.0 * u * tt * c2[0] + ttt * b[0];
    p[1] = uuu * a[1] + 3.0 * uu * t * c1[1] + 3.0 * u * tt * c2[1] + ttt * b[1];
    p
}

fn point_segment_distance2(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> (f32, [f32; 2]) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let ab_len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if ab_len2 <= 1e-6 {
        let d2 = ap[0] * ap[0] + ap[1] * ap[1];
        return (d2, a);
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / ab_len2).clamp(0.0, 1.0);
    let proj = [a[0] + t * ab[0], a[1] + t * ab[1]];
    let dx = p[0] - proj[0];
    let dy = p[1] - proj[1];
    (dx * dx + dy * dy, proj)
}

fn point_in_rect(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
}

fn bezier_intersects_rect(a: [f32; 2], b: [f32; 2], rect: [f32; 4]) -> bool {
    // Approximate: sample cubic via straight segments and test segment-rect intersection
    let dx = (b[0] - a[0]).abs();
    let c1 = [a[0] + dx * 0.5, a[1]];
    let c2 = [b[0] - dx * 0.5, b[1]];
    let mut prev = a;
    let steps = 24;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = cubic_bezier_point(a, c1, c2, b, t);
        if segment_intersects_rect(prev, p, rect) {
            return true;
        }
        prev = p;
    }
    false
}

fn segment_intersects_rect(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> bool {
    if point_in_rect(a, r) || point_in_rect(b, r) {
        return true;
    }
    // Check intersection with rect edges
    let edges = [
        ([r[0], r[1]], [r[2], r[1]]),
        ([r[2], r[1]], [r[2], r[3]]),
        ([r[2], r[3]], [r[0], r[3]]),
        ([r[0], r[3]], [r[0], r[1]]),
    ];
    for (e1, e2) in edges {
        if segments_intersect(a, b, e1, e2) {
            return true;
        }
    }
    false
}

fn orient(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn on_segment(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> bool {
    p[0].min(a[0]) - 1e-3 <= b[0]
        && b[0] <= p[0].max(a[0]) + 1e-3
        && p[1].min(a[1]) - 1e-3 <= b[1]
        && b[1] <= p[1].max(a[1]) + 1e-3
}
fn segments_intersect(p1: [f32; 2], p2: [f32; 2], q1: [f32; 2], q2: [f32; 2]) -> bool {
    let o1 = orient(p1, p2, q1);
    let o2 = orient(p1, p2, q2);
    let o3 = orient(q1, q2, p1);
    let o4 = orient(q1, q2, p2);
    if (o1 > 0.0 && o2 < 0.0 || o1 < 0.0 && o2 > 0.0)
        && (o3 > 0.0 && o4 < 0.0 || o3 < 0.0 && o4 > 0.0)
    {
        return true;
    }
    if o1.abs() < 1e-3 && on_segment(p1, q1, p2) {
        return true;
    }
    if o2.abs() < 1e-3 && on_segment(p1, q2, p2) {
        return true;
    }
    if o3.abs() < 1e-3 && on_segment(q1, p1, q2) {
        return true;
    }
    if o4.abs() < 1e-3 && on_segment(q1, p2, q2) {
        return true;
    }
    false
}

fn node_rect_screen(origin: [f32; 2], view: &GraphView, n: &Node) -> [f32; 4] {
    let pos = world_to_screen(n.pos, origin, view);
    let sz = node_size(n);
    [
        pos[0],
        pos[1],
        pos[0] + sz[0] * view.zoom,
        pos[1] + sz[1] * view.zoom,
    ]
}

fn rect_from_points(a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Delete selected nodes and any links connected to them
pub fn delete_selected(graph: &mut Graph, view: &mut GraphView) {
    delete_selected_core(graph, view, None);
}

fn delete_selected_core(
    graph: &mut Graph,
    view: &mut GraphView,
    mut on_del_link: Option<&mut dyn FnMut(LinkId)>,
) {
    if view.selected_nodes.is_empty() && view.selected_links.is_empty() {
        return;
    }
    // Build pin->node map immutably first
    use std::collections::HashMap;
    let mut pin_to_node: HashMap<PinId, NodeId> = HashMap::new();
    for n in &graph.nodes {
        for p in &n.inputs {
            pin_to_node.insert(p.id, n.id);
        }
        for p in &n.outputs {
            pin_to_node.insert(p.id, n.id);
        }
    }
    // Collect links to remove (to allow callbacks)
    let mut to_remove: Vec<LinkId> = Vec::new();
    for ln in &graph.links {
        let remove_by_sel_link = view.selected_links.contains(&ln.id);
        let a = pin_to_node.get(&ln.from).copied();
        let b = pin_to_node.get(&ln.to).copied();
        let remove_by_node = a
            .map(|id| view.selected_nodes.contains(&id))
            .unwrap_or(false)
            || b.map(|id| view.selected_nodes.contains(&id))
                .unwrap_or(false);
        if remove_by_sel_link || remove_by_node {
            to_remove.push(ln.id);
        }
    }
    if let Some(cb) = on_del_link.as_deref_mut() {
        for lid in &to_remove {
            (cb)(*lid);
        }
    }
    graph.links.retain(|ln| !to_remove.contains(&ln.id));
    // Remove selected nodes
    graph.nodes.retain(|n| !view.selected_nodes.contains(&n.id));
    view.selected_links.clear();
    view.selected_nodes.clear();
}

fn node_world_rect(n: &Node) -> [f32; 4] {
    let sz = node_size(n);
    [n.pos[0], n.pos[1], n.pos[0] + sz[0], n.pos[1] + sz[1]]
}

fn union_rect(mut acc: [f32; 4], r: [f32; 4]) -> [f32; 4] {
    acc[0] = acc[0].min(r[0]);
    acc[1] = acc[1].min(r[1]);
    acc[2] = acc[2].max(r[2]);
    acc[3] = acc[3].max(r[3]);
    acc
}

fn fit_rect(view: &mut GraphView, _origin: [f32; 2], size: [f32; 2], rect: [f32; 4]) {
    let rw = (rect[2] - rect[0]).max(1.0);
    let rh = (rect[3] - rect[1]).max(1.0);
    let margin = 0.10_f32; // 10% margin
    let avail_w = size[0] * (1.0 - margin * 2.0);
    let avail_h = size[1] * (1.0 - margin * 2.0);
    let zoom = (avail_w / rw).min(avail_h / rh).min(1.0);
    view.zoom = zoom;
    view.zoom_target = zoom;
    // center
    let wcx = (rect[0] + rect[2]) * 0.5;
    let wcy = (rect[1] + rect[3]) * 0.5;
    let scx = size[0] * 0.5;
    let scy = size[1] * 0.5;
    view.pan[0] = scx - wcx * view.zoom;
    view.pan[1] = scy - wcy * view.zoom;
}

/// Fit all nodes in view to the current window rectangle
pub fn fit_all_nodes(graph: &Graph, view: &mut GraphView, origin: [f32; 2], size: [f32; 2]) {
    if graph.nodes.is_empty() {
        return;
    }
    let mut r = node_world_rect(&graph.nodes[0]);
    for n in &graph.nodes[1..] {
        r = union_rect(r, node_world_rect(n));
    }
    fit_rect(view, origin, size, r);
}

/// Fit selected nodes in view to the current window rectangle
pub fn fit_selected_nodes(graph: &Graph, view: &mut GraphView, origin: [f32; 2], size: [f32; 2]) {
    let mut iter = graph
        .nodes
        .iter()
        .filter(|n| view.selected_nodes.contains(&n.id));
    let Some(first) = iter.next() else { return };
    let mut r = node_world_rect(first);
    for n in iter {
        r = union_rect(r, node_world_rect(n));
    }
    fit_rect(view, origin, size, r);
}

fn draw_minimap(
    ui: &Ui,
    dl: &DrawListMut,
    graph: &Graph,
    view: &mut GraphView,
    style: &GraphStyle,
    origin: [f32; 2],
    size: [f32; 2],
) -> bool {
    // compute screen rect from normalized
    let min_screen = [
        origin[0] + style.minimap_rect[0] * size[0],
        origin[1] + style.minimap_rect[1] * size[1],
    ];
    let max_screen = [
        origin[0] + style.minimap_rect[2] * size[0],
        origin[1] + style.minimap_rect[3] * size[1],
    ];
    let view_size = [max_screen[0] - min_screen[0], max_screen[1] - min_screen[1]];
    if view_size[0] <= 0.0 || view_size[1] <= 0.0 {
        return false;
    }

    // compute world bounds of nodes
    if graph.nodes.is_empty() {
        return false;
    }
    let mut min = [f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN];
    let margin = [50.0, 50.0];
    for n in &graph.nodes {
        let r = node_world_rect(n);
        min[0] = min[0].min(r[0] - margin[0]);
        min[1] = min[1].min(r[1] - margin[1]);
        max[0] = max[0].max(r[2] + margin[0]);
        max[1] = max[1].max(r[3] + margin[1]);
    }
    // include current view
    let world_view_min = [-view.pan[0] / view.zoom, -view.pan[1] / view.zoom];
    let world_view_max = [
        world_view_min[0] + size[0] / view.zoom,
        world_view_min[1] + size[1] / view.zoom,
    ];
    min[0] = min[0].min(world_view_min[0]);
    min[1] = min[1].min(world_view_min[1]);
    max[0] = max[0].max(world_view_max[0]);
    max[1] = max[1].max(world_view_max[1]);
    let nodes_size = [max[0] - min[0], max[1] - min[1]];
    if nodes_size[0] <= 0.0 || nodes_size[1] <= 0.0 {
        return false;
    }
    let middle_world = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let middle_screen = [
        (min_screen[0] + max_screen[0]) * 0.5,
        (min_screen[1] + max_screen[1]) * 0.5,
    ];
    let ratio_y = view_size[1] / nodes_size[1];
    let ratio_x = view_size[0] / nodes_size[0];
    let factor = ratio_y.min(ratio_x).min(1.0);

    // draw minimap bg
    dl.add_rect(min_screen, max_screen, style.minimap_bg_color)
        .filled(true)
        .build();

    // draw nodes rectangles
    for n in &graph.nodes {
        let mut r = node_world_rect(n);
        // world -> screen mapping
        r[0] = (r[0] - middle_world[0]) * factor + middle_screen[0];
        r[1] = (r[1] - middle_world[1]) * factor + middle_screen[1];
        r[2] = (r[2] - middle_world[0]) * factor + middle_screen[0];
        r[3] = (r[3] - middle_world[1]) * factor + middle_screen[1];
        dl.add_rect([r[0], r[1]], [r[2], r[3]], style.node_bg_color)
            .filled(true)
            .build();
        if view.selected_nodes.contains(&n.id) {
            dl.add_rect([r[0], r[1]], [r[2], r[3]], style.selected_outline_color)
                .build();
        }
    }

    // draw view rect in minimap
    let mut vr = [
        world_view_min[0],
        world_view_min[1],
        world_view_max[0],
        world_view_max[1],
    ];
    vr[0] = (vr[0] - middle_world[0]) * factor + middle_screen[0];
    vr[1] = (vr[1] - middle_world[1]) * factor + middle_screen[1];
    vr[2] = (vr[2] - middle_world[0]) * factor + middle_screen[0];
    vr[3] = (vr[3] - middle_world[1]) * factor + middle_screen[1];
    dl.add_rect([vr[0], vr[1]], [vr[2], vr[3]], style.minimap_view_fill)
        .filled(true)
        .build();
    dl.add_rect([vr[0], vr[1]], [vr[2], vr[3]], style.minimap_view_outline)
        .build();

    // click to reposition view
    let io = ui.io();
    let mouse = io.mouse_pos();
    let mouse_in_minimap = mouse[0] >= min_screen[0]
        && mouse[0] <= max_screen[0]
        && mouse[1] >= min_screen[1]
        && mouse[1] <= max_screen[1];
    if mouse_in_minimap && ui.is_mouse_clicked(MouseButton::Left) {
        let clicked_ratio = [
            (mouse[0] - min_screen[0]) / view_size[0],
            (mouse[1] - min_screen[1]) / view_size[1],
        ];
        let world_pos_center = [
            min[0] + (max[0] - min[0]) * clicked_ratio[0],
            min[1] + (max[1] - min[1]) * clicked_ratio[1],
        ];
        let world_size_view = [size[0] / view.zoom, size[1] / view.zoom];
        let world_view_min_new = [
            world_pos_center[0] - world_size_view[0] * 0.5,
            world_pos_center[1] - world_size_view[1] * 0.5,
        ];
        // clamp
        let mut wmin = world_view_min_new;
        let mut wmax = [wmin[0] + world_size_view[0], wmin[1] + world_size_view[1]];
        if wmin[0] < min[0] {
            wmin[0] = min[0];
            wmax[0] = wmin[0] + world_size_view[0];
        }
        if wmin[1] < min[1] {
            wmin[1] = min[1];
            wmax[1] = wmin[1] + world_size_view[1];
        }
        if wmax[0] > max[0] {
            wmax[0] = max[0];
            wmin[0] = wmax[0] - world_size_view[0];
        }
        if wmax[1] > max[1] {
            wmax[1] = max[1];
            wmin[1] = wmax[1] - world_size_view[1];
        }
        view.pan[0] = size[0] * 0.5 - (wmin[0] + world_size_view[0] * 0.5) * view.zoom;
        view.pan[1] = size[1] * 0.5 - (wmin[1] + world_size_view[1] * 0.5) * view.zoom;
    }
    true
}

#[derive(Copy, Clone, Debug, Default)]
pub struct RightClickEvent {
    pub node: Option<NodeId>,
    pub pin: Option<(PinId, PinKind)>,
    pub mouse_pos: [f32; 2],
}
