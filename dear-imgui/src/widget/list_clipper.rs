use crate::ui::Ui;
use dear_imgui_sys as sys;

/// List clipper for efficiently displaying large lists
///
/// The ListClipper is a helper to display large lists efficiently by only rendering
/// the visible items. This is particularly useful for lists with thousands of items.
///
/// # Example
///
/// ```rust,no_run
/// # use dear_imgui::{Context, ListClipper};
/// # let mut ctx = Context::new().unwrap();
/// # let mut frame = ctx.frame();
/// # let items: Vec<String> = (0..10000).map(|i| format!("Item {}", i)).collect();
/// # frame.window("Large List").show(|ui| {
/// let mut clipper = ListClipper::new(items.len());
/// clipper.begin(ui);
///
/// while clipper.step() {
///     for i in clipper.display_start()..clipper.display_end() {
///         ui.text(&items[i]);
///     }
/// }
///
/// clipper.end();
/// # });
/// ```
pub struct ListClipper {
    clipper: sys::ImGuiListClipper,
    items_count: i32,
    items_height: f32,
}

impl ListClipper {
    /// Create a new ListClipper
    ///
    /// # Arguments
    ///
    /// * `items_count` - Total number of items in the list
    pub fn new(items_count: usize) -> Self {
        Self {
            clipper: unsafe { std::mem::zeroed() },
            items_count: items_count as i32,
            items_height: -1.0, // Auto-detect height
        }
    }

    /// Create a new ListClipper with a specific item height
    ///
    /// # Arguments
    ///
    /// * `items_count` - Total number of items in the list
    /// * `items_height` - Height of each item in pixels
    pub fn new_with_height(items_count: usize, items_height: f32) -> Self {
        Self {
            clipper: unsafe { std::mem::zeroed() },
            items_count: items_count as i32,
            items_height,
        }
    }

    /// Begin clipping. Call this before the loop.
    ///
    /// # Arguments
    ///
    /// * `ui` - The UI context
    pub fn begin(&mut self, _ui: &mut Ui) {
        unsafe {
            sys::ImGuiListClipper_Begin(&mut self.clipper, self.items_count, self.items_height);
        }
    }

    /// Step the clipper. Call this in a while loop.
    ///
    /// Returns `true` if there are more items to display.
    pub fn step(&mut self) -> bool {
        unsafe { sys::ImGuiListClipper_Step(&mut self.clipper) }
    }

    /// End clipping. Call this after the loop.
    pub fn end(&mut self) {
        unsafe {
            sys::ImGuiListClipper_End(&mut self.clipper);
        }
    }

    /// Get the start index of items to display
    pub fn display_start(&self) -> usize {
        self.clipper.DisplayStart as usize
    }

    /// Get the end index of items to display (exclusive)
    pub fn display_end(&self) -> usize {
        self.clipper.DisplayEnd as usize
    }

    /// Get the total number of items
    pub fn items_count(&self) -> usize {
        self.items_count as usize
    }

    /// Get the height of each item
    pub fn items_height(&self) -> f32 {
        self.items_height
    }

    /// Force the clipper to display a specific range of items
    ///
    /// This is useful when you need to ensure certain items are visible.
    ///
    /// # Arguments
    ///
    /// * `item_min` - Start index (inclusive)
    /// * `item_max` - End index (exclusive)
    pub fn force_display_range(&mut self, item_min: usize, item_max: usize) {
        // Note: This functionality may not be available in all ImGui versions
        // For now, we'll implement a basic version
        self.clipper.DisplayStart = item_min as i32;
        self.clipper.DisplayEnd = item_max as i32;
    }
}

impl Drop for ListClipper {
    fn drop(&mut self) {
        // Ensure the clipper is properly ended
        unsafe {
            if self.clipper.ItemsCount > 0 {
                sys::ImGuiListClipper_End(&mut self.clipper);
            }
        }
    }
}

/// # Widgets: List Clipper
impl<'frame> Ui<'frame> {
    /// Create and use a list clipper for efficient large list rendering
    ///
    /// This is a convenience method that handles the clipper lifecycle automatically.
    ///
    /// # Arguments
    ///
    /// * `items_count` - Total number of items in the list
    /// * `render_fn` - Function to render items in the visible range
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let items: Vec<String> = (0..10000).map(|i| format!("Item {}", i)).collect();
    /// # frame.window("Large List").show(|ui| {
    /// ui.list_clipper(items.len(), |ui, start, end| {
    ///     for i in start..end {
    ///         ui.text(&items[i]);
    ///     }
    /// });
    /// # });
    /// ```
    pub fn list_clipper<F>(&mut self, items_count: usize, mut render_fn: F)
    where
        F: FnMut(&mut Ui, usize, usize),
    {
        let mut clipper = ListClipper::new(items_count);
        clipper.begin(self);

        while clipper.step() {
            render_fn(self, clipper.display_start(), clipper.display_end());
        }

        clipper.end();
    }

    /// Create and use a list clipper with specific item height
    ///
    /// # Arguments
    ///
    /// * `items_count` - Total number of items in the list
    /// * `items_height` - Height of each item in pixels
    /// * `render_fn` - Function to render items in the visible range
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let items: Vec<String> = (0..10000).map(|i| format!("Item {}", i)).collect();
    /// # frame.window("Large List").show(|ui| {
    /// ui.list_clipper_with_height(items.len(), 20.0, |ui, start, end| {
    ///     for i in start..end {
    ///         ui.text(&items[i]);
    ///     }
    /// });
    /// # });
    /// ```
    pub fn list_clipper_with_height<F>(
        &mut self,
        items_count: usize,
        items_height: f32,
        mut render_fn: F,
    ) where
        F: FnMut(&mut Ui, usize, usize),
    {
        let mut clipper = ListClipper::new_with_height(items_count, items_height);
        clipper.begin(self);

        while clipper.step() {
            render_fn(self, clipper.display_start(), clipper.display_end());
        }

        clipper.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_clipper_creation() {
        let clipper = ListClipper::new(1000);
        assert_eq!(clipper.items_count(), 1000);
        assert_eq!(clipper.items_height(), -1.0);
    }

    #[test]
    fn test_list_clipper_with_height() {
        let clipper = ListClipper::new_with_height(500, 25.0);
        assert_eq!(clipper.items_count(), 500);
        assert_eq!(clipper.items_height(), 25.0);
    }

    #[test]
    fn test_display_range_initial() {
        let clipper = ListClipper::new(100);
        // Initially, display range should be 0..0 before stepping
        assert_eq!(clipper.display_start(), 0);
        assert_eq!(clipper.display_end(), 0);
    }
}
