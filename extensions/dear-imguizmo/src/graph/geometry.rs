use super::model::{Graph, GraphView, LinkEnd, LinkId, Node, NodeId, PinId, PinKind};
use super::style::GraphStyle;

// helpers
pub(super) fn world_to_screen(p: [f32; 2], origin: [f32; 2], view: &GraphView) -> [f32; 2] {
    [
        p[0] * view.zoom + view.pan[0] + origin[0],
        p[1] * view.zoom + view.pan[1] + origin[1],
    ]
}

pub(super) fn screen_to_world(p: [f32; 2], origin: [f32; 2], view: &GraphView) -> [f32; 2] {
    [
        (p[0] - origin[0] - view.pan[0]) / view.zoom,
        (p[1] - origin[1] - view.pan[1]) / view.zoom,
    ]
}

pub(super) fn node_size(node: &Node) -> [f32; 2] {
    let w = 160.0;
    let header = 24.0;
    let line = 20.0;
    let pads = 8.0;
    let rows = node.inputs.len().max(node.outputs.len()) as f32;
    [w, header + rows * line + pads]
}

pub(super) fn find_pin<'a>(graph: &'a Graph, pid: PinId) -> Option<(&'a Node, PinKind, usize)> {
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

pub(super) fn pin_screen_pos(
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

pub(super) fn can_connect(graph: &Graph, a: PinId, b: PinId) -> bool {
    if a == b {
        return false;
    }
    match (find_pin(graph, a), find_pin(graph, b)) {
        (Some((na, ka, _)), Some((nb, kb, _))) => ka != kb && na.id != nb.id,
        _ => false,
    }
}

pub(super) fn normalize_link_direction(graph: &Graph, a: PinId, b: PinId) -> (PinId, PinId) {
    let ka = find_pin(graph, a).map(|(_, k, _)| k);
    match ka {
        Some(PinKind::Output) => (a, b),
        Some(PinKind::Input) => (b, a),
        _ => (a, b),
    }
}

// hit_node(): replaced by hit_node_rect() for simpler usage; keep code minimal

pub(super) fn hit_node_rect(
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

pub(super) fn hit_pin(
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

pub(super) fn hit_link(
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

pub(super) fn near_link_end(
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

pub(super) fn link_endpoints<'a>(
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

pub(super) fn bezier_intersects_rect(a: [f32; 2], b: [f32; 2], rect: [f32; 4]) -> bool {
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

pub(super) fn node_rect_screen(origin: [f32; 2], view: &GraphView, n: &Node) -> [f32; 4] {
    let pos = world_to_screen(n.pos, origin, view);
    let sz = node_size(n);
    [
        pos[0],
        pos[1],
        pos[0] + sz[0] * view.zoom,
        pos[1] + sz[1] * view.zoom,
    ]
}

pub(super) fn rect_from_points(a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

pub(super) fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

pub(super) fn node_world_rect(n: &Node) -> [f32; 4] {
    let sz = node_size(n);
    [n.pos[0], n.pos[1], n.pos[0] + sz[0], n.pos[1] + sz[1]]
}

pub(super) fn union_rect(mut acc: [f32; 4], r: [f32; 4]) -> [f32; 4] {
    acc[0] = acc[0].min(r[0]);
    acc[1] = acc[1].min(r[1]);
    acc[2] = acc[2].max(r[2]);
    acc[3] = acc[3].max(r[3]);
    acc
}

pub(super) fn fit_rect(view: &mut GraphView, _origin: [f32; 2], size: [f32; 2], rect: [f32; 4]) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::{Graph, Link, LinkId, Node, NodeId, Pin, PinId, PinKind};

    fn sample_graph() -> Graph {
        let mut graph = Graph::new();
        let mut left = Node::new(NodeId(1), [0.0, 0.0], "left");
        left.outputs
            .push(Pin::new(PinId(10), "out", PinKind::Output));
        let mut right = Node::new(NodeId(2), [240.0, 0.0], "right");
        right.inputs.push(Pin::new(PinId(20), "in", PinKind::Input));
        right
            .outputs
            .push(Pin::new(PinId(21), "out", PinKind::Output));
        graph.nodes.push(left);
        graph.nodes.push(right);
        graph.links.push(Link {
            id: LinkId(7),
            from: PinId(10),
            to: PinId(20),
        });
        graph
    }

    #[test]
    fn graph_geometry_allows_only_cross_node_input_output_links() {
        let graph = sample_graph();

        assert!(can_connect(&graph, PinId(10), PinId(20)));
        assert!(can_connect(&graph, PinId(20), PinId(10)));
        assert!(!can_connect(&graph, PinId(10), PinId(10)));
        assert!(!can_connect(&graph, PinId(20), PinId(21)));
        assert!(!can_connect(&graph, PinId(10), PinId(999)));
    }

    #[test]
    fn graph_geometry_normalizes_links_from_output_to_input() {
        let graph = sample_graph();

        assert_eq!(
            normalize_link_direction(&graph, PinId(10), PinId(20)),
            (PinId(10), PinId(20))
        );
        assert_eq!(
            normalize_link_direction(&graph, PinId(20), PinId(10)),
            (PinId(10), PinId(20))
        );
    }

    #[test]
    fn graph_geometry_rect_intersection_includes_touching_edges() {
        assert!(rects_intersect(
            [0.0, 0.0, 10.0, 10.0],
            [10.0, 10.0, 20.0, 20.0]
        ));
        assert!(!rects_intersect(
            [0.0, 0.0, 10.0, 10.0],
            [11.0, 11.0, 20.0, 20.0]
        ));
    }
}
