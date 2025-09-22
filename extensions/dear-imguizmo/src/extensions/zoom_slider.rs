//! Zoom slider (ImZoomSlider) - simplified Rust port

use dear_imgui::{DrawListMut, Ui};

bitflags::bitflags! {
    /// Flags for zoom slider
    pub struct ZoomSliderFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Vertical orientation
        const VERTICAL = 1 << 0;
        /// Hide anchors
        const NO_ANCHORS = 1 << 1;
        /// Hide middle carets
        const NO_MIDDLE_CARETS = 1 << 2;
        /// Disable mouse wheel zoom
        const NO_WHEEL = 1 << 3;
    }
}

/// Zoom slider widget
pub fn zoom_slider<T: Copy + PartialOrd + Into<f32> + From<f32>>(
    ui: &Ui,
    lower: T,
    higher: T,
    view_lower: &mut T,
    view_higher: &mut T,
    wheel_ratio: f32,
    flags: ZoomSliderFlags,
) -> bool {
    let mut interacted = false;
    let draw_list = ui.get_window_draw_list();
    let is_vertical = flags.contains(ZoomSliderFlags::VERTICAL);
    let canvas_pos = ui.cursor_screen_pos();
    let canvas_size = ui.content_region_avail();
    let length = if is_vertical {
        ui.item_rect_size()[1]
    } else {
        canvas_size[0]
    };
    let scroll_bar_size = if is_vertical {
        [14.0, length]
    } else {
        [length, 14.0]
    };

    ui.invisible_button("ImZoomSlider", scroll_bar_size);

    let lb: f32 = lower.into();
    let hb: f32 = higher.into();
    let mut vl: f32 = (*view_lower).into();
    let mut vh: f32 = (*view_higher).into();

    // clamp
    if vl < lb {
        vl = lb;
    }
    if vh > hb {
        vh = hb;
    }
    if vl > vh {
        std::mem::swap(&mut vl, &mut vh);
    }

    let min = ui.item_rect_min();
    let max = ui.item_rect_max();
    let comp = if is_vertical { 1 } else { 0 };
    let bar_min = min;
    let bar_max = max;

    // draw background
    draw_list
        .add_rect(bar_min, bar_max, 0xFF101010)
        .filled(true)
        .build();
    draw_list.add_rect(bar_min, bar_max, 0xFF222222).build();

    // thumb positions
    let len = if is_vertical {
        bar_max[1] - bar_min[1]
    } else {
        bar_max[0] - bar_min[0]
    };
    let s = len.max(1.0);
    let start = ((vl - lb) / (hb - lb)) * s + bar_min[comp];
    let end = ((vh - lb) / (hb - lb)) * s + bar_min[comp];
    let thumb_min = if is_vertical {
        [bar_min[0], start]
    } else {
        [start, bar_min[1]]
    };
    let thumb_max = if is_vertical {
        [bar_max[0], end]
    } else {
        [end, bar_max[1]]
    };

    let in_scroll = point_in_rect(ui.io().mouse_pos(), thumb_min, thumb_max);
    let bar_color = if in_scroll {
        ui.style_color(dear_imgui::StyleColor::FrameBgHovered)
    } else {
        ui.style_color(dear_imgui::StyleColor::FrameBg)
    };
    draw_list
        .add_rect(thumb_min, thumb_max, bar_color)
        .filled(true)
        .build();

    // click to jump thumb
    if ui.is_mouse_clicked(dear_imgui::MouseButton::Left)
        && !in_scroll
        && point_in_rect(ui.io().mouse_pos(), bar_min, bar_max)
    {
        let ratio =
            (ui.io().mouse_pos()[comp] - bar_min[comp]) / (bar_max[comp] - bar_min[comp]).max(1.0);
        let size = hb - lb;
        let half = (vh - vl) * 0.5;
        let middle = ratio * size + lb;
        vl = middle - half;
        vh = middle + half;
        interacted = true;
    }

    // wheel zoom
    if !flags.contains(ZoomSliderFlags::NO_WHEEL) && in_scroll && ui.io().mouse_wheel() != 0.0 {
        let ratio = (ui.io().mouse_pos()[comp] - start) / (end - start).max(1.0);
        let amount = ui.io().mouse_wheel() * wheel_ratio * (vh - vl);
        vl -= ratio * amount;
        vh += (1.0 - ratio) * amount;
        interacted = true;
    }

    // write back
    *view_lower = T::from(vl.clamp(lb, hb));
    *view_higher = T::from(vh.clamp(lb, hb));
    interacted
}

fn point_in_rect(mouse: [f32; 2], min: [f32; 2], max: [f32; 2]) -> bool {
    mouse[0] >= min[0] && mouse[0] <= max[0] && mouse[1] >= min[1] && mouse[1] <= max[1]
}
