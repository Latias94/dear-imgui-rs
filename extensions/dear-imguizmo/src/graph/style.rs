use std::num::NonZeroU32;

/// Positive grid interval for drawing major grid lines in the graph editor.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GraphGridMajorInterval(NonZeroU32);

impl GraphGridMajorInterval {
    /// Default major-grid interval used by [`GraphStyle`].
    pub const DEFAULT: Self = Self::new(10);

    /// Create a positive grid interval from a non-zero value.
    ///
    /// Panics if `interval` exceeds Dear ImGui's signed `int` range used by
    /// the internal grid index math.
    #[inline]
    pub const fn from_nonzero(interval: NonZeroU32) -> Self {
        assert!(
            interval.get() <= i32::MAX as u32,
            "GraphGridMajorInterval::from_nonzero() interval exceeded i32::MAX"
        );
        Self(interval)
    }

    /// Create a positive grid interval.
    ///
    /// Panics if `interval` is zero or exceeds Dear ImGui's signed `int` range
    /// used by the internal grid index math.
    #[inline]
    pub const fn new(interval: usize) -> Self {
        assert!(
            interval > 0,
            "GraphGridMajorInterval::new() requires a non-zero interval"
        );
        assert!(
            interval <= i32::MAX as usize,
            "GraphGridMajorInterval::new() interval exceeded i32::MAX"
        );
        match NonZeroU32::new(interval as u32) {
            Some(interval) => Self(interval),
            None => unreachable!(),
        }
    }

    /// Return the positive interval as a Rust count.
    #[inline]
    pub const fn get(self) -> usize {
        self.0.get() as usize
    }

    #[inline]
    pub(super) fn raw_i32(self) -> i32 {
        self.0.get() as i32
    }
}

impl From<NonZeroU32> for GraphGridMajorInterval {
    fn from(interval: NonZeroU32) -> Self {
        Self::from_nonzero(interval)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GraphStyle {
    pub background_color: [f32; 4],
    pub grid_visible: bool,
    pub grid_spacing: f32,
    pub grid_color: [f32; 4],
    pub grid_color2: [f32; 4],
    pub grid_major_every: GraphGridMajorInterval,
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
            grid_major_every: GraphGridMajorInterval::DEFAULT,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_grid_major_interval_accepts_positive_values() {
        let interval = GraphGridMajorInterval::new(4);

        assert_eq!(interval.get(), 4);
        assert_eq!(interval.raw_i32(), 4);
        assert_eq!(
            GraphStyle::default().grid_major_every,
            GraphGridMajorInterval::DEFAULT
        );
    }

    #[test]
    fn graph_grid_major_interval_rejects_zero() {
        assert!(std::panic::catch_unwind(|| GraphGridMajorInterval::new(0)).is_err());
    }

    #[test]
    fn graph_grid_major_interval_rejects_values_outside_imgui_int_range() {
        assert!(
            std::panic::catch_unwind(|| GraphGridMajorInterval::new(i32::MAX as usize + 1))
                .is_err()
        );
    }

    #[test]
    fn graph_grid_major_interval_accepts_nonzero_u32() {
        let interval = NonZeroU32::new(8).unwrap();

        assert_eq!(GraphGridMajorInterval::from(interval).get(), 8);
    }
}
