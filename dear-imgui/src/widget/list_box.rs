use std::borrow::Cow;

use crate::Ui;
use crate::sys;

/// Builder for a list box widget
#[derive(Clone, Debug)]
#[must_use]
pub struct ListBox<T> {
    label: T,
    size: [f32; 2],
}

impl<T: AsRef<str>> ListBox<T> {
    /// Constructs a new list box builder.
    #[doc(alias = "ListBoxHeaderVec2", alias = "ListBoxHeaderInt")]
    pub fn new(label: T) -> ListBox<T> {
        ListBox {
            label,
            size: [0.0, 0.0],
        }
    }

    /// Sets the list box size based on the given width and height
    /// If width or height are 0 or smaller, a default value is calculated
    /// Helper to calculate the size of a listbox and display a label on the right.
    /// Tip: To have a list filling the entire window width, PushItemWidth(-1) and pass an non-visible label e.g. "##empty"
    ///
    /// Default: [0.0, 0.0], in which case the combobox calculates a sensible width and height
    #[inline]
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = size.into();
        self
    }
    /// Creates a list box and starts appending to it.
    ///
    /// Returns `Some(ListBoxToken)` if the list box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the list box is not open and no content should be rendered.
    #[must_use]
    pub fn begin(self, ui: &Ui) -> Option<ListBoxToken<'_>> {
        let size_vec = sys::ImVec2 {
            x: self.size[0],
            y: self.size[1],
        };
        let should_render = unsafe { sys::igBeginListBox(ui.scratch_txt(self.label), size_vec) };
        if should_render {
            Some(ListBoxToken::new(ui))
        } else {
            None
        }
    }
    /// Creates a list box and runs a closure to construct the list contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if the list box is not open.
    pub fn build<R, F: FnOnce() -> R>(self, ui: &Ui, f: F) -> Option<R> {
        self.begin(ui).map(|_list| f())
    }
}

/// Tracks a list box that can be ended by calling `.end()`
/// or by dropping
pub struct ListBoxToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> ListBoxToken<'ui> {
    /// Creates a new list box token
    pub fn new(ui: &'ui Ui) -> Self {
        Self { ui }
    }

    /// Ends the list box
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ListBoxToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndListBox();
        }
    }
}

/// # Convenience functions
impl<T: AsRef<str>> ListBox<T> {
    /// Builds a simple list box for choosing from a slice of values
    pub fn build_simple<V, L>(
        self,
        ui: &Ui,
        current_item: &mut usize,
        items: &[V],
        label_fn: &L,
    ) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let mut result = false;
        let lb = self;
        if let Some(_cb) = lb.begin(ui) {
            for (idx, item) in items.iter().enumerate() {
                let text = label_fn(item);
                let selected = idx == *current_item;
                if ui.selectable_config(&text).selected(selected).build() {
                    *current_item = idx;
                    result = true;
                }
            }
        }
        result
    }
}
