use crate::{StyleColor, from_vec2, sys, vec2, vec4};

#[derive(Clone, Debug, PartialEq)]
pub struct NodeEditorStyle {
    pub node_padding: [f32; 4],
    pub node_rounding: f32,
    pub node_border_width: f32,
    pub hovered_node_border_width: f32,
    pub hovered_node_border_offset: f32,
    pub selected_node_border_width: f32,
    pub selected_node_border_offset: f32,
    pub pin_rounding: f32,
    pub pin_border_width: f32,
    pub link_strength: f32,
    pub source_direction: [f32; 2],
    pub target_direction: [f32; 2],
    pub scroll_duration: f32,
    pub flow_marker_distance: f32,
    pub flow_speed: f32,
    pub flow_duration: f32,
    pub pivot_alignment: [f32; 2],
    pub pivot_size: [f32; 2],
    pub pivot_scale: [f32; 2],
    pub pin_corners: f32,
    pub pin_radius: f32,
    pub pin_arrow_size: f32,
    pub pin_arrow_width: f32,
    pub group_rounding: f32,
    pub group_border_width: f32,
    pub highlight_connected_links: f32,
    pub snap_link_to_pin_dir: f32,
    pub colors: [[f32; 4]; StyleColor::COUNT],
}

impl NodeEditorStyle {
    #[doc(alias = "GetStyle")]
    pub(crate) fn current() -> Self {
        let mut colors = [[0.0; 4]; StyleColor::COUNT];
        for color in StyleColor::ALL {
            colors[color.index()] = from_vec4(unsafe { sys::dne_get_style_color(color.raw()) });
        }

        Self {
            node_padding: from_vec4(unsafe { sys::dne_get_style_node_padding() }),
            node_rounding: unsafe { sys::dne_get_style_node_rounding() },
            node_border_width: unsafe { sys::dne_get_style_node_border_width() },
            hovered_node_border_width: unsafe { sys::dne_get_style_hovered_node_border_width() },
            hovered_node_border_offset: unsafe { sys::dne_get_style_hovered_node_border_offset() },
            selected_node_border_width: unsafe { sys::dne_get_style_selected_node_border_width() },
            selected_node_border_offset: unsafe {
                sys::dne_get_style_selected_node_border_offset()
            },
            pin_rounding: unsafe { sys::dne_get_style_pin_rounding() },
            pin_border_width: unsafe { sys::dne_get_style_pin_border_width() },
            link_strength: unsafe { sys::dne_get_style_link_strength() },
            source_direction: from_vec2(unsafe { sys::dne_get_style_source_direction() }),
            target_direction: from_vec2(unsafe { sys::dne_get_style_target_direction() }),
            scroll_duration: unsafe { sys::dne_get_style_scroll_duration() },
            flow_marker_distance: unsafe { sys::dne_get_style_flow_marker_distance() },
            flow_speed: unsafe { sys::dne_get_style_flow_speed() },
            flow_duration: unsafe { sys::dne_get_style_flow_duration() },
            pivot_alignment: from_vec2(unsafe { sys::dne_get_style_pivot_alignment() }),
            pivot_size: from_vec2(unsafe { sys::dne_get_style_pivot_size() }),
            pivot_scale: from_vec2(unsafe { sys::dne_get_style_pivot_scale() }),
            pin_corners: unsafe { sys::dne_get_style_pin_corners() },
            pin_radius: unsafe { sys::dne_get_style_pin_radius() },
            pin_arrow_size: unsafe { sys::dne_get_style_pin_arrow_size() },
            pin_arrow_width: unsafe { sys::dne_get_style_pin_arrow_width() },
            group_rounding: unsafe { sys::dne_get_style_group_rounding() },
            group_border_width: unsafe { sys::dne_get_style_group_border_width() },
            highlight_connected_links: unsafe { sys::dne_get_style_highlight_connected_links() },
            snap_link_to_pin_dir: unsafe { sys::dne_get_style_snap_link_to_pin_dir() },
            colors,
        }
    }

    pub(crate) fn apply(&self) {
        validate_style(self);
        unsafe {
            sys::dne_set_style_node_padding(vec4(self.node_padding));
            sys::dne_set_style_node_rounding(self.node_rounding);
            sys::dne_set_style_node_border_width(self.node_border_width);
            sys::dne_set_style_hovered_node_border_width(self.hovered_node_border_width);
            sys::dne_set_style_hovered_node_border_offset(self.hovered_node_border_offset);
            sys::dne_set_style_selected_node_border_width(self.selected_node_border_width);
            sys::dne_set_style_selected_node_border_offset(self.selected_node_border_offset);
            sys::dne_set_style_pin_rounding(self.pin_rounding);
            sys::dne_set_style_pin_border_width(self.pin_border_width);
            sys::dne_set_style_link_strength(self.link_strength);
            sys::dne_set_style_source_direction(vec2(self.source_direction));
            sys::dne_set_style_target_direction(vec2(self.target_direction));
            sys::dne_set_style_scroll_duration(self.scroll_duration);
            sys::dne_set_style_flow_marker_distance(self.flow_marker_distance);
            sys::dne_set_style_flow_speed(self.flow_speed);
            sys::dne_set_style_flow_duration(self.flow_duration);
            sys::dne_set_style_pivot_alignment(vec2(self.pivot_alignment));
            sys::dne_set_style_pivot_size(vec2(self.pivot_size));
            sys::dne_set_style_pivot_scale(vec2(self.pivot_scale));
            sys::dne_set_style_pin_corners(self.pin_corners);
            sys::dne_set_style_pin_radius(self.pin_radius);
            sys::dne_set_style_pin_arrow_size(self.pin_arrow_size);
            sys::dne_set_style_pin_arrow_width(self.pin_arrow_width);
            sys::dne_set_style_group_rounding(self.group_rounding);
            sys::dne_set_style_group_border_width(self.group_border_width);
            sys::dne_set_style_highlight_connected_links(self.highlight_connected_links);
            sys::dne_set_style_snap_link_to_pin_dir(self.snap_link_to_pin_dir);
            for color in StyleColor::ALL {
                sys::dne_set_style_color(color.raw(), vec4(self.colors[color.index()]));
            }
        }
    }

    pub fn color(&self, color: StyleColor) -> [f32; 4] {
        self.colors[color.index()]
    }

    pub fn set_color(&mut self, color: StyleColor, value: [f32; 4]) {
        assert_finite_vec4("NodeEditorStyle::set_color()", "value", value);
        self.colors[color.index()] = value;
    }
}

pub(crate) fn current_style_color(color: StyleColor) -> [f32; 4] {
    from_vec4(unsafe { sys::dne_get_style_color(color.raw()) })
}

pub(crate) fn apply_style_color(color: StyleColor, value: [f32; 4]) {
    assert_finite_vec4("EditorContext::set_style_color()", "value", value);
    unsafe { sys::dne_set_style_color(color.raw(), vec4(value)) };
}

fn validate_style(style: &NodeEditorStyle) {
    assert_finite_vec4(
        "NodeEditorStyle::apply()",
        "node_padding",
        style.node_padding,
    );
    for (name, value) in [
        ("node_rounding", style.node_rounding),
        ("node_border_width", style.node_border_width),
        ("hovered_node_border_width", style.hovered_node_border_width),
        (
            "hovered_node_border_offset",
            style.hovered_node_border_offset,
        ),
        (
            "selected_node_border_width",
            style.selected_node_border_width,
        ),
        (
            "selected_node_border_offset",
            style.selected_node_border_offset,
        ),
        ("pin_rounding", style.pin_rounding),
        ("pin_border_width", style.pin_border_width),
        ("link_strength", style.link_strength),
        ("scroll_duration", style.scroll_duration),
        ("flow_marker_distance", style.flow_marker_distance),
        ("flow_speed", style.flow_speed),
        ("flow_duration", style.flow_duration),
        ("pin_corners", style.pin_corners),
        ("pin_radius", style.pin_radius),
        ("pin_arrow_size", style.pin_arrow_size),
        ("pin_arrow_width", style.pin_arrow_width),
        ("group_rounding", style.group_rounding),
        ("group_border_width", style.group_border_width),
        ("highlight_connected_links", style.highlight_connected_links),
        ("snap_link_to_pin_dir", style.snap_link_to_pin_dir),
    ] {
        assert_finite_f32("NodeEditorStyle::apply()", name, value);
    }
    assert_finite_vec2(
        "NodeEditorStyle::apply()",
        "source_direction",
        style.source_direction,
    );
    assert_finite_vec2(
        "NodeEditorStyle::apply()",
        "target_direction",
        style.target_direction,
    );
    assert_finite_vec2(
        "NodeEditorStyle::apply()",
        "pivot_alignment",
        style.pivot_alignment,
    );
    assert_finite_vec2("NodeEditorStyle::apply()", "pivot_size", style.pivot_size);
    assert_finite_vec2("NodeEditorStyle::apply()", "pivot_scale", style.pivot_scale);
    for color in style.colors {
        assert_finite_vec4("NodeEditorStyle::apply()", "color", color);
    }
}

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} components must be finite"
    );
}

fn from_vec4(value: sys::ImVec4_c) -> [f32; 4] {
    [value.x, value.y, value.z, value.w]
}
