//! ImSequencer implementation
//!
//! This module provides timeline/sequencer functionality for animations.

use crate::GuizmoResult;
use dear_imgui::{DrawListMut, Ui};

bitflags::bitflags! {
    /// Sequencer options flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SequencerOptions: u32 {
        /// No editing allowed
        const EDIT_NONE = 0;
        /// Allow editing start/end points
        const EDIT_STARTEND = 1 << 1;
        /// Allow changing frame
        const CHANGE_FRAME = 1 << 3;
        /// Allow adding items
        const ADD = 1 << 4;
        /// Allow deleting items
        const DEL = 1 << 5;
        /// Allow copy/paste
        const COPYPASTE = 1 << 6;
        /// All editing options
        const EDIT_ALL = Self::EDIT_STARTEND.bits() | Self::CHANGE_FRAME.bits();
    }
}

/// Trait for sequencer interface
pub trait SequenceInterface {
    /// Get minimum frame number
    fn get_frame_min(&self) -> i32;
    /// Get maximum frame number
    fn get_frame_max(&self) -> i32;
    /// Get number of items
    fn get_item_count(&self) -> i32;
    /// Get item label
    fn get_item_label(&self, index: i32) -> String;
    /// Get item type index
    fn get_item_type_index(&self, index: i32) -> i32;
    /// Get item start frame
    fn get_item_start(&self, index: i32) -> i32;
    /// Get item end frame
    fn get_item_end(&self, index: i32) -> i32;
    /// Get item color
    fn get_item_color(&self, index: i32) -> u32;
}

/// Sequencer widget
pub struct Sequencer;

impl Sequencer {
    /// Create a new sequencer
    pub fn new() -> Self {
        Self
    }

    /// Render the sequencer
    pub fn render<T: SequenceInterface>(
        &self,
        ui: &Ui,
        sequence: &T,
        current_frame: &mut i32,
        expanded: &mut bool,
        selected_entry: &mut Option<i32>,
        first_frame: &mut i32,
        options: SequencerOptions,
    ) -> GuizmoResult<bool> {
        let mut modified = false;

        // Get available space
        let content_region = ui.content_region_avail();
        let sequencer_height = if *expanded { 200.0 } else { 100.0 };

        // Create child window for sequencer
        ui.child_window("Sequencer")
            .size([content_region[0], sequencer_height])
            .border(true)
            .build(ui, || {
                let draw_list = ui.get_window_draw_list();
                let window_pos = ui.cursor_screen_pos();
                let window_size = [content_region[0], sequencer_height];

                // Draw timeline background
                if self
                    .draw_timeline_background(&draw_list, window_pos, window_size)
                    .is_err()
                {
                    return;
                }

                // Draw frame ruler
                if self
                    .draw_frame_ruler(&draw_list, window_pos, window_size, sequence, *first_frame)
                    .is_err()
                {
                    return;
                }

                // Draw sequence items
                if *expanded {
                    for i in 0..sequence.get_item_count() {
                        if let Ok(item_modified) = self.draw_sequence_item(
                            &draw_list,
                            ui,
                            sequence,
                            i,
                            window_pos,
                            window_size,
                            *first_frame,
                            selected_entry,
                            options,
                        ) {
                            modified = modified || item_modified;
                        }
                    }
                }

                // Draw current frame indicator
                if self
                    .draw_current_frame_indicator(
                        &draw_list,
                        window_pos,
                        window_size,
                        *current_frame,
                        *first_frame,
                    )
                    .is_err()
                {
                    return;
                }

                // Handle mouse interaction
                if let Ok(mouse_modified) = self.handle_mouse_interaction(
                    ui,
                    sequence,
                    current_frame,
                    first_frame,
                    window_pos,
                    window_size,
                    options,
                ) {
                    modified = modified || mouse_modified;
                }
            });

        // Toggle expand/collapse button
        if ui.button(if *expanded { "Collapse" } else { "Expand" }) {
            *expanded = !*expanded;
        }

        Ok(modified)
    }

    /// Draw timeline background
    fn draw_timeline_background(
        &self,
        draw_list: &DrawListMut,
        window_pos: [f32; 2],
        window_size: [f32; 2],
    ) -> GuizmoResult<()> {
        // Draw background
        draw_list
            .add_rect(
                window_pos,
                [
                    window_pos[0] + window_size[0],
                    window_pos[1] + window_size[1],
                ],
                0xFF2D2D30, // Dark gray background
            )
            .filled(true)
            .build();

        Ok(())
    }

    /// Draw frame ruler
    fn draw_frame_ruler<T: SequenceInterface>(
        &self,
        draw_list: &DrawListMut,
        window_pos: [f32; 2],
        window_size: [f32; 2],
        sequence: &T,
        first_frame: i32,
    ) -> GuizmoResult<()> {
        let ruler_height = 30.0;
        let frame_width = 10.0;
        let visible_frames = (window_size[0] / frame_width) as i32;

        // Draw ruler background
        draw_list
            .add_rect(
                window_pos,
                [window_pos[0] + window_size[0], window_pos[1] + ruler_height],
                0xFF3C3C3C, // Slightly lighter gray
            )
            .filled(true)
            .build();

        // Draw frame numbers
        for i in 0..visible_frames {
            let frame_num = first_frame + i;
            if frame_num >= sequence.get_frame_min() && frame_num <= sequence.get_frame_max() {
                let x = window_pos[0] + i as f32 * frame_width;

                // Draw frame tick
                draw_list
                    .add_line(
                        [x, window_pos[1]],
                        [x, window_pos[1] + ruler_height],
                        0xFFFFFFFF, // White
                    )
                    .build();

                // Draw frame number every 10 frames
                if frame_num % 10 == 0 {
                    draw_list
                        .add_line(
                            [x, window_pos[1]],
                            [x, window_pos[1] + ruler_height * 0.5],
                            0xFFFFFF00, // Yellow for major ticks
                        )
                        .build();
                }
            }
        }

        Ok(())
    }

    /// Draw sequence item
    fn draw_sequence_item<T: SequenceInterface>(
        &self,
        draw_list: &DrawListMut,
        _ui: &Ui,
        sequence: &T,
        item_index: i32,
        window_pos: [f32; 2],
        window_size: [f32; 2],
        first_frame: i32,
        selected_entry: &mut Option<i32>,
        _options: SequencerOptions,
    ) -> GuizmoResult<bool> {
        let item_height = 20.0;
        let ruler_height = 30.0;
        let frame_width = 10.0;
        let y_offset = ruler_height + item_index as f32 * (item_height + 2.0);

        let start_frame = sequence.get_item_start(item_index);
        let end_frame = sequence.get_item_end(item_index);
        let item_color = sequence.get_item_color(item_index);

        // Calculate screen positions
        let start_x = window_pos[0] + (start_frame - first_frame) as f32 * frame_width;
        let end_x = window_pos[0] + (end_frame - first_frame) as f32 * frame_width;
        let item_y = window_pos[1] + y_offset;

        // Only draw if visible
        if end_x >= window_pos[0] && start_x <= window_pos[0] + window_size[0] {
            // Draw item rectangle
            let is_selected = *selected_entry == Some(item_index);
            let final_color = if is_selected {
                0xFFFFFFFF // White when selected
            } else {
                item_color
            };

            draw_list
                .add_rect(
                    [start_x.max(window_pos[0]), item_y],
                    [
                        end_x.min(window_pos[0] + window_size[0]),
                        item_y + item_height,
                    ],
                    final_color,
                )
                .filled(true)
                .build();

            // Draw item border
            draw_list
                .add_rect(
                    [start_x.max(window_pos[0]), item_y],
                    [
                        end_x.min(window_pos[0] + window_size[0]),
                        item_y + item_height,
                    ],
                    0xFF000000, // Black border
                )
                .build();
        }

        Ok(false) // No modification for now
    }

    /// Draw current frame indicator
    fn draw_current_frame_indicator(
        &self,
        draw_list: &DrawListMut,
        window_pos: [f32; 2],
        window_size: [f32; 2],
        current_frame: i32,
        first_frame: i32,
    ) -> GuizmoResult<()> {
        let frame_width = 10.0;
        let x = window_pos[0] + (current_frame - first_frame) as f32 * frame_width;

        // Only draw if visible
        if x >= window_pos[0] && x <= window_pos[0] + window_size[0] {
            // Draw current frame line
            draw_list
                .add_line(
                    [x, window_pos[1]],
                    [x, window_pos[1] + window_size[1]],
                    0xFFFF0000, // Red line
                )
                .thickness(2.0)
                .build();

            // Draw current frame triangle at top
            let triangle_size = 8.0;
            draw_list
                .add_triangle(
                    [x, window_pos[1]],
                    [x - triangle_size * 0.5, window_pos[1] + triangle_size],
                    [x + triangle_size * 0.5, window_pos[1] + triangle_size],
                    0xFFFF0000, // Red triangle
                )
                .filled(true)
                .build();
        }

        Ok(())
    }

    /// Handle mouse interaction
    fn handle_mouse_interaction<T: SequenceInterface>(
        &self,
        ui: &Ui,
        sequence: &T,
        current_frame: &mut i32,
        first_frame: &mut i32,
        window_pos: [f32; 2],
        window_size: [f32; 2],
        options: SequencerOptions,
    ) -> GuizmoResult<bool> {
        let mut modified = false;

        if !options.contains(SequencerOptions::CHANGE_FRAME) {
            return Ok(false);
        }

        let io = ui.io();
        let mouse_pos = io.mouse_pos();
        let frame_width = 10.0;

        // Check if mouse is over sequencer
        if mouse_pos[0] >= window_pos[0]
            && mouse_pos[0] <= window_pos[0] + window_size[0]
            && mouse_pos[1] >= window_pos[1]
            && mouse_pos[1] <= window_pos[1] + window_size[1]
        {
            if ui.is_mouse_clicked(dear_imgui::MouseButton::Left) {
                // Calculate clicked frame
                let clicked_frame =
                    *first_frame + ((mouse_pos[0] - window_pos[0]) / frame_width) as i32;

                // Clamp to valid range
                let new_frame = clicked_frame
                    .max(sequence.get_frame_min())
                    .min(sequence.get_frame_max());

                if new_frame != *current_frame {
                    *current_frame = new_frame;
                    modified = true;
                }
            }

            // Handle scrolling to change first frame
            let mouse_wheel = io.mouse_wheel();
            if mouse_wheel != 0.0 {
                let scroll_amount = (mouse_wheel * 5.0) as i32;
                let new_first_frame = (*first_frame - scroll_amount)
                    .max(sequence.get_frame_min())
                    .min(sequence.get_frame_max() - (window_size[0] / frame_width) as i32);

                if new_first_frame != *first_frame {
                    *first_frame = new_first_frame;
                    modified = true;
                }
            }
        }

        Ok(modified)
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSequence;

    impl SequenceInterface for TestSequence {
        fn get_frame_min(&self) -> i32 {
            0
        }
        fn get_frame_max(&self) -> i32 {
            100
        }
        fn get_item_count(&self) -> i32 {
            3
        }
        fn get_item_label(&self, index: i32) -> String {
            format!("Item {}", index)
        }
        fn get_item_type_index(&self, _index: i32) -> i32 {
            0
        }
        fn get_item_start(&self, index: i32) -> i32 {
            index * 10
        }
        fn get_item_end(&self, index: i32) -> i32 {
            (index + 1) * 10
        }
        fn get_item_color(&self, _index: i32) -> u32 {
            0xFFFFFFFF
        }
    }

    #[test]
    fn test_sequencer_creation() {
        let _sequencer = Sequencer::new();
        // Basic creation test
    }

    #[test]
    fn test_sequence_interface() {
        let seq = TestSequence;
        assert_eq!(seq.get_frame_min(), 0);
        assert_eq!(seq.get_frame_max(), 100);
        assert_eq!(seq.get_item_count(), 3);
    }
}
