use std::collections::HashSet;

use dear_imgui_rs::{
    Ui,
    input::{Key, MouseButton},
};

use super::super::geometry::{
    bezier_intersects_rect, can_connect, find_pin, hit_link, hit_node_rect, hit_pin,
    link_endpoints, near_link_end, node_rect_screen, normalize_link_direction, pin_screen_pos,
    rect_from_points, rects_intersect, screen_to_world,
};
use super::super::minimap::draw_minimap;
use super::super::model::{Graph, GraphView, Link, LinkEnd, LinkId, NodeId, PinId, PinKind};
use super::super::operations::delete_selected_core;
use super::super::render::{draw_grid, draw_link_variant, draw_node};
use super::super::style::GraphStyle;
use super::{GraphEditorResponse, Hooks, RightClickEvent};

pub(super) fn draw_core<'ui, 'h>(
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
    let left_down = io.mouse_down(MouseButton::Left);
    let middle_down = io.mouse_down(MouseButton::Middle);
    let right_down = io.mouse_down(MouseButton::Right);
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
        let rc_node = view.hovered_node;
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
