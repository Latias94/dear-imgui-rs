use std::collections::HashMap;

use super::geometry::{fit_rect, node_world_rect, union_rect};
use super::model::{Graph, GraphView, LinkId, NodeId, PinId};

/// Delete selected nodes and any links connected to them
pub fn delete_selected(graph: &mut Graph, view: &mut GraphView) {
    delete_selected_core(graph, view, None);
}

pub(super) fn delete_selected_core(
    graph: &mut Graph,
    view: &mut GraphView,
    mut on_del_link: Option<&mut dyn FnMut(LinkId)>,
) {
    if view.selected_nodes.is_empty() && view.selected_links.is_empty() {
        return;
    }
    // Build pin->node map immutably first
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
