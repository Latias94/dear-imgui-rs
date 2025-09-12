//! ImCurveEdit implementation
//!
//! This module provides curve editing functionality for animations and bezier curves.

use crate::{types::Vec2, GuizmoResult};
use dear_imgui::{DrawListMut, Ui};

/// Curve types supported by the editor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CurveType {
    /// No curve
    None = 0,
    /// Discrete steps
    Discrete = 1,
    /// Linear interpolation
    Linear = 2,
    /// Smooth interpolation
    Smooth = 3,
    /// Bezier curve
    Bezier = 4,
}

/// Edit point for curve manipulation
#[derive(Debug, Clone, PartialEq)]
pub struct EditPoint {
    /// Curve index
    pub curve_index: i32,
    /// Point index within the curve
    pub point_index: i32,
}

impl PartialOrd for EditPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EditPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.curve_index.cmp(&other.curve_index) {
            std::cmp::Ordering::Equal => self.point_index.cmp(&other.point_index),
            other => other,
        }
    }
}

impl Eq for EditPoint {}

/// Trait for curve delegate interface
pub trait CurveDelegate {
    /// Get the number of curves
    fn get_curve_count(&self) -> usize;
    /// Check if a curve is visible
    fn is_curve_visible(&self, curve_index: usize) -> bool;
    /// Get the number of points in a curve
    fn get_point_count(&self, curve_index: usize) -> usize;
    /// Get curve color
    fn get_curve_color(&self, curve_index: usize) -> u32;
    /// Get a point value
    fn get_point(&self, curve_index: usize, point_index: usize) -> Vec2;
    /// Get curve type
    fn get_curve_type(&self, curve_index: usize) -> CurveType;
    /// Edit a point (return true if changed)
    fn edit_point(&mut self, curve_index: usize, point_index: usize, value: Vec2) -> bool;
    /// Add a point
    fn add_point(&mut self, curve_index: usize, value: Vec2);
    /// Delete a point
    fn delete_point(&mut self, curve_index: usize, point_index: usize);
}

/// Curve editor widget
pub struct CurveEditor {
    /// Current selection
    selection: Vec<EditPoint>,
    /// Whether the editor is focused
    focused: bool,
}

impl CurveEditor {
    /// Create a new curve editor
    pub fn new() -> Self {
        Self {
            selection: Vec::new(),
            focused: false,
        }
    }

    /// Render the curve editor
    pub fn edit<T: CurveDelegate>(
        &mut self,
        ui: &Ui,
        delegate: &mut T,
        size: Vec2,
        id: u32,
    ) -> GuizmoResult<bool> {
        let mut modified = false;

        // Create child window for curve editor
        ui.child_window(format!("CurveEditor_{}", id))
            .size([size.x, size.y])
            .border(true)
            .build(ui, || {
                let draw_list = ui.get_window_draw_list();
                let window_pos = ui.cursor_screen_pos();
                let window_size = [size.x, size.y];

                // Draw background grid
                if self.draw_grid(&draw_list, window_pos, window_size).is_err() {
                    return;
                }

                // Draw curve
                if self
                    .draw_curve(&draw_list, delegate, window_pos, window_size)
                    .is_err()
                {
                    return;
                }

                // Draw control points
                if let Ok(points_modified) =
                    self.draw_control_points(&draw_list, ui, delegate, window_pos, window_size)
                {
                    modified = modified || points_modified;
                }

                // Handle mouse interaction
                if let Ok(interaction_modified) =
                    self.handle_mouse_interaction(ui, delegate, window_pos, window_size)
                {
                    modified = modified || interaction_modified;
                }
            });

        Ok(modified)
    }

    /// Get current selection
    pub fn get_selection(&self) -> &[EditPoint] {
        &self.selection
    }

    /// Set selection
    pub fn set_selection(&mut self, selection: Vec<EditPoint>) {
        self.selection = selection;
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Check if editor is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Draw background grid
    fn draw_grid(
        &self,
        draw_list: &DrawListMut,
        window_pos: [f32; 2],
        window_size: [f32; 2],
    ) -> GuizmoResult<()> {
        let grid_size = 20.0;
        let grid_color = 0xFF404040; // Dark gray

        // Draw vertical lines
        let mut x = 0.0;
        while x <= window_size[0] {
            draw_list
                .add_line(
                    [window_pos[0] + x, window_pos[1]],
                    [window_pos[0] + x, window_pos[1] + window_size[1]],
                    grid_color,
                )
                .build();
            x += grid_size;
        }

        // Draw horizontal lines
        let mut y = 0.0;
        while y <= window_size[1] {
            draw_list
                .add_line(
                    [window_pos[0], window_pos[1] + y],
                    [window_pos[0] + window_size[0], window_pos[1] + y],
                    grid_color,
                )
                .build();
            y += grid_size;
        }

        Ok(())
    }

    /// Draw curve
    fn draw_curve<T: CurveDelegate>(
        &self,
        draw_list: &DrawListMut,
        delegate: &T,
        window_pos: [f32; 2],
        window_size: [f32; 2],
    ) -> GuizmoResult<()> {
        let curve_count = delegate.get_curve_count();

        // Draw all visible curves
        for curve_index in 0..curve_count {
            if !delegate.is_curve_visible(curve_index) {
                continue;
            }

            let point_count = delegate.get_point_count(curve_index);
            if point_count < 2 {
                continue;
            }

            let curve_color = delegate.get_curve_color(curve_index);
            let segments = 100; // Number of segments to draw smooth curve

            // Draw curve segments
            for i in 0..segments {
                let t1 = i as f32 / segments as f32;
                let t2 = (i + 1) as f32 / segments as f32;

                let p1 = self.evaluate_curve(delegate, curve_index, t1)?;
                let p2 = self.evaluate_curve(delegate, curve_index, t2)?;

                // Convert to screen coordinates
                let screen_p1 = [
                    window_pos[0] + p1.x * window_size[0],
                    window_pos[1] + (1.0 - p1.y) * window_size[1], // Flip Y
                ];
                let screen_p2 = [
                    window_pos[0] + p2.x * window_size[0],
                    window_pos[1] + (1.0 - p2.y) * window_size[1], // Flip Y
                ];

                draw_list
                    .add_line(screen_p1, screen_p2, curve_color)
                    .build();
            }
        }

        Ok(())
    }

    /// Draw control points
    fn draw_control_points<T: CurveDelegate>(
        &self,
        draw_list: &DrawListMut,
        _ui: &Ui,
        delegate: &T,
        window_pos: [f32; 2],
        window_size: [f32; 2],
    ) -> GuizmoResult<bool> {
        let curve_count = delegate.get_curve_count();
        let point_radius = 4.0;
        let selected_color = 0xFFFF0000; // Red for selected
        let normal_color = 0xFFFFFFFF; // White for normal

        // Draw control points for all visible curves
        for curve_index in 0..curve_count {
            if !delegate.is_curve_visible(curve_index) {
                continue;
            }

            let point_count = delegate.get_point_count(curve_index);

            for point_index in 0..point_count {
                let point = delegate.get_point(curve_index, point_index);
                let is_selected = self.selection.iter().any(|ep| {
                    ep.curve_index == curve_index as i32 && ep.point_index == point_index as i32
                });

                // Convert to screen coordinates
                let screen_pos = [
                    window_pos[0] + point.x * window_size[0],
                    window_pos[1] + (1.0 - point.y) * window_size[1], // Flip Y
                ];

                let color = if is_selected {
                    selected_color
                } else {
                    normal_color
                };

                // Draw control point
                draw_list
                    .add_circle(screen_pos, point_radius, color)
                    .filled(true)
                    .build();

                // Draw border
                draw_list
                    .add_circle(
                        screen_pos,
                        point_radius,
                        0xFF000000, // Black border
                    )
                    .build();
            }
        }

        Ok(false) // No modification for now
    }

    /// Handle mouse interaction
    fn handle_mouse_interaction<T: CurveDelegate>(
        &mut self,
        ui: &Ui,
        _delegate: &mut T,
        _window_pos: [f32; 2],
        _window_size: [f32; 2],
    ) -> GuizmoResult<bool> {
        let io = ui.io();
        let _mouse_pos = io.mouse_pos();

        // TODO: Implement mouse interaction for curve editing
        // - Point selection
        // - Point dragging
        // - Adding/removing points

        Ok(false)
    }

    /// Evaluate curve at parameter t (0.0 to 1.0)
    fn evaluate_curve<T: CurveDelegate>(
        &self,
        delegate: &T,
        curve_index: usize,
        t: f32,
    ) -> GuizmoResult<Vec2> {
        let point_count = delegate.get_point_count(curve_index);
        if point_count == 0 {
            return Ok(Vec2::ZERO);
        }
        if point_count == 1 {
            return Ok(delegate.get_point(curve_index, 0));
        }

        // Simple linear interpolation for now
        // TODO: Implement proper curve interpolation (Bezier, spline, etc.)
        let segment_count = point_count - 1;
        let segment_t = t * segment_count as f32;
        let segment_index = (segment_t.floor() as usize).min(segment_count - 1);
        let local_t = segment_t - segment_index as f32;

        let p1 = delegate.get_point(curve_index, segment_index);
        let p2 = delegate.get_point(curve_index, (segment_index + 1).min(point_count - 1));

        Ok(p1.lerp(p2, local_t))
    }
}

impl Default for CurveEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCurveDelegate {
        curves: Vec<Vec<Vec2>>,
    }

    impl TestCurveDelegate {
        fn new() -> Self {
            Self {
                curves: vec![
                    vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)],
                    vec![Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0)],
                ],
            }
        }
    }

    impl CurveDelegate for TestCurveDelegate {
        fn get_curve_count(&self) -> usize {
            self.curves.len()
        }
        fn is_curve_visible(&self, _curve_index: usize) -> bool {
            true
        }
        fn get_point_count(&self, curve_index: usize) -> usize {
            self.curves.get(curve_index).map_or(0, |c| c.len())
        }
        fn get_curve_color(&self, _curve_index: usize) -> u32 {
            0xFFFFFFFF
        }
        fn get_point(&self, curve_index: usize, point_index: usize) -> Vec2 {
            self.curves[curve_index][point_index]
        }
        fn get_curve_type(&self, _curve_index: usize) -> CurveType {
            CurveType::Linear
        }
        fn edit_point(&mut self, curve_index: usize, point_index: usize, value: Vec2) -> bool {
            if let Some(curve) = self.curves.get_mut(curve_index) {
                if let Some(point) = curve.get_mut(point_index) {
                    *point = value;
                    return true;
                }
            }
            false
        }
        fn add_point(&mut self, curve_index: usize, value: Vec2) {
            if let Some(curve) = self.curves.get_mut(curve_index) {
                curve.push(value);
            }
        }
        fn delete_point(&mut self, curve_index: usize, point_index: usize) {
            if let Some(curve) = self.curves.get_mut(curve_index) {
                if point_index < curve.len() {
                    curve.remove(point_index);
                }
            }
        }
    }

    #[test]
    fn test_curve_editor_creation() {
        let editor = CurveEditor::new();
        assert!(!editor.is_focused());
        assert_eq!(editor.get_selection().len(), 0);
    }

    #[test]
    fn test_edit_point_ordering() {
        let p1 = EditPoint {
            curve_index: 0,
            point_index: 0,
        };
        let p2 = EditPoint {
            curve_index: 0,
            point_index: 1,
        };
        let p3 = EditPoint {
            curve_index: 1,
            point_index: 0,
        };

        assert!(p1 < p2);
        assert!(p2 < p3);
    }

    #[test]
    fn test_curve_delegate() {
        let mut delegate = TestCurveDelegate::new();
        assert_eq!(delegate.get_curve_count(), 2);
        assert_eq!(delegate.get_point_count(0), 2);

        let point = delegate.get_point(0, 0);
        assert_eq!(point, Vec2::new(0.0, 0.0));

        assert!(delegate.edit_point(0, 0, Vec2::new(0.5, 0.5)));
        let modified_point = delegate.get_point(0, 0);
        assert_eq!(modified_point, Vec2::new(0.5, 0.5));
    }
}
