//! Text filtering functionality for Dear ImGui
//!
//! This module provides a text filter system that allows users to filter content
//! based on text patterns. The filter supports include/exclude syntax similar to
//! many search interfaces.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let mut filter = TextFilter::new("Search");
//!
//! // Draw the filter input
//! filter.draw();
//!
//! // Test if text passes the filter
//! if filter.pass_filter("some text") {
//!     // Display matching content
//! }
//! ```
//!
//! # Filter Syntax
//!
//! The filter supports the following syntax:
//! - `word` - Include items containing "word"
//! - `-word` - Exclude items containing "word"
//! - `word1,word2` - Include items containing "word1" OR "word2"
//! - `word1,-word2` - Include items containing "word1" but NOT "word2"

use crate::{Ui, sys};
use std::ffi::CString;
use std::ptr;

/// Helper to parse and apply text filters
///
/// This struct provides text filtering functionality similar to many search interfaces.
/// It supports include/exclude patterns and can be used to filter lists of items.
///
/// # Examples
///
/// ```no_run
/// # use dear_imgui_rs::*;
/// # let mut ctx = Context::create();
/// # let ui = ctx.frame();
/// // Create a filter with default empty pattern
/// let mut filter = TextFilter::new("Search".to_string());
///
/// // Create a filter with initial pattern
/// let mut filter_with_pattern = TextFilter::new_with_filter(
///     "Advanced Search".to_string(),
///     "include,-exclude".to_string()
/// );
/// ```
pub struct TextFilter {
    label: CString,
    raw: *mut sys::ImGuiTextFilter,
}

impl TextFilter {
    /// Creates a new TextFilter with an empty filter.
    ///
    /// This is equivalent to [`new_with_filter`](Self::new_with_filter) with `filter` set to `""`.
    ///
    /// # Arguments
    /// * `label` - The label to display for the filter input
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let filter = TextFilter::new("Search");
    /// ```
    pub fn new(label: impl Into<String>) -> Self {
        Self::new_with_filter(label, "")
    }

    /// Creates a new TextFilter with a custom filter pattern.
    ///
    /// # Arguments
    /// * `label` - The label to display for the filter input
    /// * `filter` - The initial filter pattern
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let filter = TextFilter::new_with_filter(
    ///     "Search",
    ///     "include,-exclude"
    /// );
    /// ```
    pub fn new_with_filter(label: impl Into<String>, filter: impl AsRef<str>) -> Self {
        let label = CString::new(label.into()).expect("TextFilter label contained null byte");
        let filter_ptr = crate::string::tls_scratch_txt(filter);
        unsafe {
            let raw = sys::ImGuiTextFilter_ImGuiTextFilter(filter_ptr);
            Self { label, raw }
        }
    }

    /// Builds the TextFilter with its current filter pattern.
    ///
    /// You can use [`pass_filter`](Self::pass_filter) after calling this method.
    /// If you want to control the filter with an InputText, use [`draw`](Self::draw) instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let mut filter = TextFilter::new_with_filter(
    ///     "Search".to_string(),
    ///     "test".to_string()
    /// );
    /// filter.build();
    ///
    /// if filter.pass_filter("test string") {
    ///     println!("Text matches filter!");
    /// }
    /// ```
    pub fn build(&mut self) {
        unsafe {
            sys::ImGuiTextFilter_Build(self.raw);
        }
    }

    /// Draws an InputText widget to control the filter.
    ///
    /// This is equivalent to [`draw_with_size`](Self::draw_with_size) with `size` set to `0.0`.
    /// Returns `true` if the filter was modified.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut filter = TextFilter::new("Search");
    ///
    /// if filter.draw() {
    ///     println!("Filter was modified!");
    /// }
    /// ```
    pub fn draw(&mut self) -> bool {
        self.draw_with_size(0.0)
    }

    /// Draws an InputText widget to control the filter with a specific width.
    ///
    /// # Arguments
    /// * `width` - The width of the input text widget (0.0 for default width)
    ///
    /// Returns `true` if the filter was modified.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut filter = TextFilter::new("Search");
    ///
    /// if filter.draw_with_size(200.0) {
    ///     println!("Filter was modified!");
    /// }
    /// ```
    pub fn draw_with_size(&mut self, width: f32) -> bool {
        unsafe { sys::ImGuiTextFilter_Draw(self.raw, self.label.as_ptr(), width) }
    }

    /// Returns true if the filter is not empty.
    ///
    /// An empty filter (no pattern specified) will match all text.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let empty_filter = TextFilter::new("Search");
    /// assert!(!empty_filter.is_active());
    ///
    /// let active_filter = TextFilter::new_with_filter(
    ///     "Search",
    ///     "test"
    /// );
    /// assert!(active_filter.is_active());
    /// ```
    pub fn is_active(&self) -> bool {
        // IsActive() is an inline method: return !Filters.empty();
        // We need to check if the Filters vector is empty
        unsafe { (*self.raw).Filters.Size > 0 }
    }

    /// Returns true if the text matches the filter.
    ///
    /// [`draw`](Self::draw) or [`build`](Self::build) must be called **before** this function.
    ///
    /// # Arguments
    /// * `text` - The text to test against the filter
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let mut filter = TextFilter::new_with_filter(
    ///     "Search",
    ///     "test"
    /// );
    /// filter.build();
    ///
    /// assert!(filter.pass_filter("test string"));
    /// assert!(!filter.pass_filter("example string"));
    /// ```
    pub fn pass_filter(&self, text: &str) -> bool {
        let text_ptr = crate::string::tls_scratch_txt(text);
        unsafe { sys::ImGuiTextFilter_PassFilter(self.raw, text_ptr, ptr::null()) }
    }

    /// Returns true if the text range matches the filter.
    ///
    /// This version allows you to specify both start and end pointers for the text.
    ///
    /// # Arguments
    /// * `start` - The start of the text to test
    /// * `end` - The end of the text to test
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let mut filter = TextFilter::new_with_filter(
    ///     "Search",
    ///     "test"
    /// );
    /// filter.build();
    ///
    /// assert!(filter.pass_filter_with_end("test", " string"));
    /// ```
    pub fn pass_filter_with_end(&self, start: &str, end: &str) -> bool {
        let (start_ptr, end_ptr) = crate::string::tls_scratch_txt_two(start, end);
        unsafe { sys::ImGuiTextFilter_PassFilter(self.raw, start_ptr, end_ptr) }
    }

    /// Clears the filter pattern.
    ///
    /// This sets the filter to an empty state, which will match all text.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// let mut filter = TextFilter::new_with_filter(
    ///     "Search",
    ///     "test"
    /// );
    ///
    /// assert!(filter.is_active());
    /// filter.clear();
    /// assert!(!filter.is_active());
    /// ```
    pub fn clear(&mut self) {
        // Clear() is an inline method: InputBuf[0] = 0; Build();
        unsafe {
            (*self.raw).InputBuf[0] = 0;
            sys::ImGuiTextFilter_Build(self.raw);
        }
    }
}

impl Drop for TextFilter {
    fn drop(&mut self) {
        unsafe { sys::ImGuiTextFilter_destroy(self.raw) }
    }
}

impl Ui {
    /// Creates a new TextFilter with an empty pattern.
    ///
    /// This is a convenience method equivalent to [`TextFilter::new`].
    ///
    /// # Arguments
    /// * `label` - The label to display for the filter input
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let filter = ui.text_filter("Search");
    /// ```
    pub fn text_filter(&self, label: impl Into<String>) -> TextFilter {
        TextFilter::new(label)
    }

    /// Creates a new TextFilter with a custom filter pattern.
    ///
    /// This is a convenience method equivalent to [`TextFilter::new_with_filter`].
    ///
    /// # Arguments
    /// * `label` - The label to display for the filter input
    /// * `filter` - The initial filter pattern
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let filter = ui.text_filter_with_filter(
    ///     "Search",
    ///     "include,-exclude"
    /// );
    /// ```
    pub fn text_filter_with_filter(
        &self,
        label: impl Into<String>,
        filter: impl AsRef<str>,
    ) -> TextFilter {
        TextFilter::new_with_filter(label, filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_filter_build_and_pass_filter_work() {
        let _ctx = crate::Context::create();

        let mut filter = TextFilter::new("Search");
        filter.build();
        assert!(filter.pass_filter("anything"));

        let mut filter = TextFilter::new_with_filter("Search", "abc");
        filter.build();
        assert!(filter.pass_filter("xxabcxx"));
        assert!(!filter.pass_filter("xxdefxx"));
    }
}
