use dear_imgui_rs::{DrawListMut, Ui, input::MouseButton};

use super::geometry::node_world_rect;
use super::model::{Graph, GraphView};
use super::style::GraphStyle;

pub(super) fn draw_minimap(
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
