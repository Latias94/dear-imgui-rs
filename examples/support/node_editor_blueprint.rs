use dear_imgui_rs::{
    DrawCornerFlags, DrawListMut, HorizontalStackLayoutToken, IdStackToken, PolylineFlags,
    StackLayoutId, Ui, VerticalStackLayoutToken, texture::TextureData,
};
use dear_node_editor::{
    NodeEditorFrame, NodeId, NodeToken, PinId, PinKind, PinToken, StyleVar, StyleVarToken,
};
use std::ffi::c_void;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconType {
    Flow,
    Circle,
    Square,
    Grid,
    #[allow(dead_code)]
    RoundSquare,
    #[allow(dead_code)]
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Invalid,
    Begin,
    Header,
    Content,
    Input,
    Output,
    Middle,
    End,
}

pub struct BlueprintNodeBuilder<'ui, 'frame> {
    editor: &'frame NodeEditorFrame<'ui>,
    ui: &'frame Ui,
    node: Option<NodeToken<'frame>>,
    node_padding: Option<StyleVarToken<'frame>>,
    id_stack: Option<IdStackToken<'frame>>,
    node_layout: Option<VerticalStackLayoutToken<'frame>>,
    main_layout: Option<HorizontalStackLayoutToken<'frame>>,
    section_layout: Option<VerticalStackLayoutToken<'frame>>,
    stage_style: Vec<StyleVarToken<'frame>>,
    current_node_id: NodeId,
    current_stage: Stage,
    header_color: [f32; 4],
    header_min: [f32; 2],
    header_max: [f32; 2],
    content_min: [f32; 2],
    content_max: [f32; 2],
    has_header: bool,
    ended: bool,
}

impl<'ui, 'frame> BlueprintNodeBuilder<'ui, 'frame> {
    pub fn begin(editor: &'frame NodeEditorFrame<'ui>, ui: &'frame Ui, node_id: NodeId) -> Self {
        let node_padding = editor.push_style_var_vec4(StyleVar::NodePadding, [8.0, 4.0, 8.0, 8.0]);
        let node = editor.begin_node(node_id);
        let id_stack = ui.push_id(node_id.raw() as *const c_void);

        let mut builder = Self {
            editor,
            ui,
            node: Some(node),
            node_padding: Some(node_padding),
            id_stack: Some(id_stack),
            node_layout: None,
            main_layout: None,
            section_layout: None,
            stage_style: Vec::new(),
            current_node_id: node_id,
            current_stage: Stage::Invalid,
            header_color: [1.0, 1.0, 1.0, 1.0],
            header_min: [0.0, 0.0],
            header_max: [0.0, 0.0],
            content_min: [0.0, 0.0],
            content_max: [0.0, 0.0],
            has_header: false,
            ended: false,
        };
        builder.set_stage(Stage::Begin);
        builder
    }

    pub fn header<R>(&mut self, color: [f32; 4], f: impl FnOnce() -> R) -> R {
        self.header_color = color;
        self.set_stage(Stage::Header);
        f()
    }

    pub fn end_header(&mut self) {
        self.set_stage(Stage::Content);
    }

    pub fn input<R>(&mut self, id: PinId, f: impl FnOnce(&PinToken<'_>) -> R) -> R {
        if self.current_stage == Stage::Begin {
            self.set_stage(Stage::Content);
        }

        let apply_padding = self.current_stage == Stage::Input;
        self.set_stage(Stage::Input);
        if apply_padding {
            self.ui.spring(0.0, -1.0);
        }

        let node = self.node.as_ref().expect("node is active");
        let pin = node.begin_pin(id, PinKind::Input);
        let horizontal =
            self.ui
                .begin_horizontal(StackLayoutId::pointer_value(id.raw()), [0.0, 0.0], -1.0);
        let result = f(&pin);
        horizontal.end();
        pin.end();
        result
    }

    pub fn middle<R>(&mut self, f: impl FnOnce() -> R) -> R {
        if self.current_stage == Stage::Begin {
            self.set_stage(Stage::Content);
        }
        self.set_stage(Stage::Middle);
        f()
    }

    pub fn output<R>(&mut self, id: PinId, f: impl FnOnce(&PinToken<'_>) -> R) -> R {
        if self.current_stage == Stage::Begin {
            self.set_stage(Stage::Content);
        }

        let apply_padding = self.current_stage == Stage::Output;
        self.set_stage(Stage::Output);
        if apply_padding {
            self.ui.spring(0.0, -1.0);
        }

        let node = self.node.as_ref().expect("node is active");
        let pin = node.begin_pin(id, PinKind::Output);
        let horizontal =
            self.ui
                .begin_horizontal(StackLayoutId::pointer_value(id.raw()), [0.0, 0.0], -1.0);
        let result = f(&pin);
        horizontal.end();
        pin.end();
        result
    }

    pub fn end(mut self, header_texture: &mut TextureData) {
        self.end_inner(header_texture);
    }

    fn end_inner(&mut self, header_texture: &mut TextureData) {
        if self.ended {
            return;
        }

        self.set_stage(Stage::End);
        if let Some(node) = self.node.take() {
            node.end();
        }

        if self.ui.is_item_visible() {
            self.draw_header(header_texture);
        }

        self.current_node_id = NodeId::new(0);
        self.id_stack.take();
        self.node_padding.take();
        self.set_stage(Stage::Invalid);
        self.ended = true;
    }

    fn set_stage(&mut self, stage: Stage) -> bool {
        if stage == self.current_stage {
            return false;
        }

        let old_stage = self.current_stage;
        self.current_stage = stage;

        match old_stage {
            Stage::Header => {
                self.main_layout.take();
                (self.header_min, self.header_max) = self.ui.item_rect();
                let item_spacing = self.ui.clone_style().item_spacing();
                self.ui.spring(0.0, item_spacing[1] * 2.0);
            }
            Stage::Input | Stage::Output => {
                self.stage_style.clear();
                self.ui.spring(1.0, 0.0);
                self.section_layout.take();
            }
            Stage::Middle => {
                self.section_layout.take();
            }
            _ => {}
        }

        match stage {
            Stage::Begin => {
                self.node_layout = Some(self.ui.begin_vertical("node", [0.0, 0.0], -1.0));
            }
            Stage::Header => {
                self.has_header = true;
                self.main_layout = Some(self.ui.begin_horizontal("header", [0.0, 0.0], -1.0));
            }
            Stage::Content => {
                if old_stage == Stage::Begin {
                    self.ui.spring(0.0, -1.0);
                }
                self.main_layout = Some(self.ui.begin_horizontal("content", [0.0, 0.0], -1.0));
                self.ui.spring(0.0, 0.0);
            }
            Stage::Input => {
                self.section_layout = Some(self.ui.begin_vertical("inputs", [0.0, 0.0], 0.0));
                self.stage_style.push(
                    self.editor
                        .push_style_var_vec2(StyleVar::PivotAlignment, [0.0, 0.5]),
                );
                self.stage_style.push(
                    self.editor
                        .push_style_var_vec2(StyleVar::PivotSize, [0.0, 0.0]),
                );
                if !self.has_header {
                    self.ui.spring(1.0, 0.0);
                }
            }
            Stage::Middle => {
                self.ui.spring(1.0, -1.0);
                self.section_layout = Some(self.ui.begin_vertical("middle", [0.0, 0.0], 1.0));
            }
            Stage::Output => {
                if old_stage == Stage::Middle || old_stage == Stage::Input {
                    self.ui.spring(1.0, -1.0);
                } else {
                    self.ui.spring(1.0, 0.0);
                }
                self.section_layout = Some(self.ui.begin_vertical("outputs", [0.0, 0.0], 1.0));
                self.stage_style.push(
                    self.editor
                        .push_style_var_vec2(StyleVar::PivotAlignment, [1.0, 0.5]),
                );
                self.stage_style.push(
                    self.editor
                        .push_style_var_vec2(StyleVar::PivotSize, [0.0, 0.0]),
                );
                if !self.has_header {
                    self.ui.spring(1.0, 0.0);
                }
            }
            Stage::End => {
                if old_stage == Stage::Input {
                    self.ui.spring(1.0, 0.0);
                }
                if old_stage != Stage::Begin {
                    self.main_layout.take();
                }
                (self.content_min, self.content_max) = self.ui.item_rect();
                self.node_layout.take();
            }
            Stage::Invalid => {}
        }

        true
    }

    fn draw_header(&self, header_texture: &mut TextureData) {
        if self.header_max[0] <= self.header_min[0] || self.header_max[1] <= self.header_min[1] {
            return;
        }

        let node_style = self.editor.style();
        let imgui_style = self.ui.clone_style();
        let alpha = imgui_style.alpha();
        let half_border_width = node_style.node_border_width * 0.5;
        let texture_width = header_texture.width().max(1) as f32;
        let texture_height = header_texture.height().max(1) as f32;
        let uv = [
            (self.header_max[0] - self.header_min[0]) / (4.0 * texture_width),
            (self.header_max[1] - self.header_min[1]) / (4.0 * texture_height),
        ];
        let header_color = [
            self.header_color[0],
            self.header_color[1],
            self.header_color[2],
            alpha,
        ];
        let draw_list = self.editor.node_background_draw_list(self.current_node_id);

        draw_list.add_image_rounded(
            header_texture,
            [
                self.header_min[0] - (8.0 - half_border_width),
                self.header_min[1] - (4.0 - half_border_width),
            ],
            [
                self.header_max[0] + (8.0 - half_border_width),
                self.header_max[1],
            ],
            [0.0, 0.0],
            uv,
            header_color,
            node_style.node_rounding,
            DrawCornerFlags::TOP,
        );

        if self.content_min[1] > self.header_max[1] {
            draw_list
                .add_line(
                    [
                        self.header_min[0] - (8.0 - half_border_width),
                        self.header_max[1] - 0.5,
                    ],
                    [
                        self.header_max[0] + (8.0 - half_border_width),
                        self.header_max[1] - 0.5,
                    ],
                    [1.0, 1.0, 1.0, alpha * 32.0 / 255.0],
                )
                .thickness(1.0)
                .build();
        }
    }
}

impl Drop for BlueprintNodeBuilder<'_, '_> {
    fn drop(&mut self) {
        self.stage_style.clear();
        self.section_layout.take();
        self.main_layout.take();
        self.node_layout.take();
        self.node.take();
        self.id_stack.take();
        self.node_padding.take();
    }
}

pub fn icon(
    ui: &Ui,
    size: [f32; 2],
    icon_type: IconType,
    filled: bool,
    color: [f32; 4],
    inner_color: [f32; 4],
) {
    if ui.is_rect_visible_with_size(size) {
        let cursor_pos = ui.cursor_screen_pos();
        let draw_list = ui.get_window_draw_list();
        draw_icon(
            &draw_list,
            cursor_pos,
            [cursor_pos[0] + size[0], cursor_pos[1] + size[1]],
            icon_type,
            filled,
            color,
            inner_color,
        );
    }

    ui.dummy(size);
}

pub fn draw_icon(
    draw_list: &DrawListMut<'_>,
    a: [f32; 2],
    b: [f32; 2],
    icon_type: IconType,
    filled: bool,
    color: [f32; 4],
    inner_color: [f32; 4],
) {
    let rect_w = b[0] - a[0];
    let rect_h = b[1] - a[1];
    let rect_center_x = (a[0] + b[0]) * 0.5;
    let rect_center_y = (a[1] + b[1]) * 0.5;
    let outline_scale = rect_w / 24.0;

    if icon_type == IconType::Flow {
        draw_flow_icon(draw_list, a, b, filled, color, inner_color, outline_scale);
        return;
    }

    let rect_y = a[1];
    let mut center = [rect_center_x, rect_center_y];
    let mut triangle_start = rect_center_x + 0.32 * rect_w;

    let rect_offset = -((rect_w * 0.25 * 0.25) as i32) as f32;
    center[0] += rect_offset * 0.5;

    match icon_type {
        IconType::Circle => {
            let radius = 0.5 * rect_w / 2.0;
            if filled {
                draw_list
                    .add_circle(center, radius, color)
                    .filled(true)
                    .build();
            } else {
                let radius = radius - 0.5;
                if inner_color[3] > 0.0 {
                    draw_list
                        .add_circle(center, radius, inner_color)
                        .filled(true)
                        .build();
                }
                draw_list
                    .add_circle(center, radius, color)
                    .thickness(2.0 * outline_scale)
                    .build();
            }
        }
        IconType::Square => {
            let radius = 0.5 * rect_w / 2.0 - if filled { 0.0 } else { 0.5 };
            let p0 = [center[0] - radius, center[1] - radius];
            let p1 = [center[0] + radius, center[1] + radius];
            if filled {
                draw_list
                    .add_rect(p0, p1, color)
                    .filled(true)
                    .flags(DrawCornerFlags::ALL)
                    .build();
            } else {
                if inner_color[3] > 0.0 {
                    draw_list
                        .add_rect(p0, p1, inner_color)
                        .filled(true)
                        .flags(DrawCornerFlags::ALL)
                        .build();
                }
                draw_list
                    .add_rect(p0, p1, color)
                    .thickness(2.0 * outline_scale)
                    .flags(DrawCornerFlags::ALL)
                    .build();
            }
        }
        IconType::Grid => {
            let radius = 0.5 * rect_w / 2.0;
            let cell = (radius / 3.0).ceil();
            let base_tl = [
                (center[0] - cell * 2.5).floor(),
                (center[1] - cell * 2.5).floor(),
            ];
            let mut tl = base_tl;
            let mut br = [base_tl[0] + cell, base_tl[1] + cell];
            for row in 0..3 {
                tl[0] = base_tl[0];
                br[0] = base_tl[0] + cell;
                draw_list.add_rect(tl, br, color).filled(true).build();
                tl[0] += cell * 2.0;
                br[0] += cell * 2.0;
                if row != 1 || filled {
                    draw_list.add_rect(tl, br, color).filled(true).build();
                }
                tl[0] += cell * 2.0;
                br[0] += cell * 2.0;
                draw_list.add_rect(tl, br, color).filled(true).build();
                tl[1] += cell * 2.0;
                br[1] += cell * 2.0;
            }
            triangle_start = br[0] + cell + 1.0 / 24.0 * rect_w;
        }
        IconType::RoundSquare => {
            let radius = 0.5 * rect_w / 2.0 - if filled { 0.0 } else { 0.5 };
            let corner_radius = radius * 0.5;
            let p0 = [center[0] - radius, center[1] - radius];
            let p1 = [center[0] + radius, center[1] + radius];
            if filled {
                draw_list
                    .add_rect(p0, p1, color)
                    .filled(true)
                    .rounding(corner_radius)
                    .flags(DrawCornerFlags::ALL)
                    .build();
            } else {
                if inner_color[3] > 0.0 {
                    draw_list
                        .add_rect(p0, p1, inner_color)
                        .filled(true)
                        .rounding(corner_radius)
                        .flags(DrawCornerFlags::ALL)
                        .build();
                }
                draw_list
                    .add_rect(p0, p1, color)
                    .rounding(corner_radius)
                    .thickness(2.0 * outline_scale)
                    .flags(DrawCornerFlags::ALL)
                    .build();
            }
        }
        IconType::Diamond => draw_diamond_icon(
            draw_list,
            center,
            rect_w,
            filled,
            color,
            inner_color,
            outline_scale,
        ),
        IconType::Flow => unreachable!(),
    }

    if matches!(
        icon_type,
        IconType::Circle | IconType::Square | IconType::Grid
    ) {
        draw_fallback_triangle(
            draw_list,
            triangle_start,
            rect_w,
            rect_y,
            rect_h,
            rect_center_y,
            color,
        );
    }
}

fn draw_flow_icon(
    draw_list: &DrawListMut<'_>,
    a: [f32; 2],
    b: [f32; 2],
    filled: bool,
    color: [f32; 4],
    inner_color: [f32; 4],
    outline_scale: f32,
) {
    let rect_w = b[0] - a[0];
    let origin_scale = rect_w / 24.0;
    let offset_x = 1.0 * origin_scale;
    let margin = 2.0 * origin_scale;
    let rounding = 0.1 * origin_scale;
    let tip_round = 0.7;
    let canvas = [
        a[0] + margin + offset_x,
        a[1] + margin,
        b[0] - margin + offset_x,
        b[1] - margin,
    ];
    let canvas_w = canvas[2] - canvas[0];
    let canvas_h = canvas[3] - canvas[1];
    let left = canvas[0] + canvas_w * 0.5 * 0.3;
    let right = canvas[0] + canvas_w - canvas_w * 0.5 * 0.3;
    let top = canvas[1] + canvas_h * 0.5 * 0.2;
    let bottom = canvas[1] + canvas_h - canvas_h * 0.5 * 0.2;
    let center_y = (top + bottom) * 0.5;
    let tip_top = [canvas[0] + canvas_w * 0.5, top];
    let tip_right = [right, center_y];
    let tip_bottom = [canvas[0] + canvas_w * 0.5, bottom];

    path_flow_icon(
        draw_list, left, top, bottom, rounding, tip_top, tip_right, tip_bottom, tip_round,
    );
    if filled {
        draw_list.path_fill_convex(color);
    } else {
        if inner_color[3] > 0.0 {
            draw_list.path_fill_convex(inner_color);
            path_flow_icon(
                draw_list, left, top, bottom, rounding, tip_top, tip_right, tip_bottom, tip_round,
            );
        }
        draw_list.path_stroke(color, PolylineFlags::CLOSED, 2.0 * outline_scale);
    }
}

fn path_flow_icon(
    draw_list: &DrawListMut<'_>,
    left: f32,
    top: f32,
    bottom: f32,
    rounding: f32,
    tip_top: [f32; 2],
    tip_right: [f32; 2],
    tip_bottom: [f32; 2],
    tip_round: f32,
) {
    draw_list.path_line_to([left, top + rounding]);
    draw_list.path_bezier_cubic_curve_to([left, top], [left, top], [left + rounding, top], 0);
    draw_list.path_line_to(tip_top);
    draw_list.path_line_to([
        tip_top[0] + (tip_right[0] - tip_top[0]) * tip_round,
        tip_top[1] + (tip_right[1] - tip_top[1]) * tip_round,
    ]);
    draw_list.path_bezier_cubic_curve_to(
        tip_right,
        tip_right,
        [
            tip_bottom[0] + (tip_right[0] - tip_bottom[0]) * tip_round,
            tip_bottom[1] + (tip_right[1] - tip_bottom[1]) * tip_round,
        ],
        0,
    );
    draw_list.path_line_to(tip_bottom);
    draw_list.path_line_to([left + rounding, bottom]);
    draw_list.path_bezier_cubic_curve_to(
        [left, bottom],
        [left, bottom],
        [left, bottom - rounding],
        0,
    );
}

fn draw_diamond_icon(
    draw_list: &DrawListMut<'_>,
    center: [f32; 2],
    rect_w: f32,
    filled: bool,
    color: [f32; 4],
    inner_color: [f32; 4],
    outline_scale: f32,
) {
    let radius = 0.607 * rect_w / 2.0 - if filled { 0.0 } else { 0.5 };
    path_diamond(draw_list, center, radius);
    if filled {
        draw_list.path_fill_convex(color);
    } else {
        if inner_color[3] > 0.0 {
            draw_list.path_fill_convex(inner_color);
            path_diamond(draw_list, center, radius);
        }
        draw_list.path_stroke(color, PolylineFlags::CLOSED, 2.0 * outline_scale);
    }
}

fn path_diamond(draw_list: &DrawListMut<'_>, center: [f32; 2], radius: f32) {
    draw_list.path_line_to([center[0], center[1] - radius]);
    draw_list.path_line_to([center[0] + radius, center[1]]);
    draw_list.path_line_to([center[0], center[1] + radius]);
    draw_list.path_line_to([center[0] - radius, center[1]]);
}

fn draw_fallback_triangle(
    draw_list: &DrawListMut<'_>,
    triangle_start: f32,
    rect_w: f32,
    rect_y: f32,
    rect_h: f32,
    rect_center_y: f32,
    color: [f32; 4],
) {
    let triangle_tip = triangle_start + rect_w * (0.45 - 0.32);
    draw_list
        .add_triangle(
            [triangle_tip.ceil(), rect_y + rect_h * 0.5],
            [triangle_start, rect_center_y + 0.15 * rect_h],
            [triangle_start, rect_center_y - 0.15 * rect_h],
            color,
        )
        .filled(true)
        .build();
}
