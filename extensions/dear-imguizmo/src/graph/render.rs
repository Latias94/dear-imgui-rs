use dear_imgui_rs::{DrawListMut, Ui};

use super::geometry::{node_size, world_to_screen};
use super::model::{GraphView, Node, NodeId, PinKind};
use super::style::GraphStyle;

pub(super) fn draw_grid(
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
    let major_every = style.grid_major_every.raw_i32();
    for i in 0..x_count {
        let x = origin[0] + start_x + i as f32 * grid;
        let idx = base_i + i;
        let color = if (idx % major_every) == 0 {
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
        let color = if (idy % major_every) == 0 {
            style.grid_color2
        } else {
            style.grid_color
        };
        dl.add_line([origin[0], y], [origin[0] + size[0], y], color)
            .build();
    }
}

pub(super) fn draw_node<'ui>(
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

pub(super) fn draw_link_variant(
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
