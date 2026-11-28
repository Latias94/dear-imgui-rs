//! List boxes
//!
//! Classic list-box widget and builder for fixed-height item selection.
//!
use std::borrow::Cow;

use crate::Ui;
use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::sys;
use crate::validation::assert_finite_vec2;

/// # List Box Widgets
impl Ui {
    /// Constructs a new list box builder.
    #[doc(alias = "ListBox")]
    pub fn list_box_config<T: AsRef<str>>(&self, label: T) -> ListBox<'_, T> {
        ListBox {
            ui: self,
            label,
            size: [0.0, 0.0],
        }
    }
}

/// Builder for a list box widget
#[derive(Clone, Debug)]
#[must_use]
pub struct ListBox<'ui, T> {
    ui: &'ui Ui,
    label: T,
    size: [f32; 2],
}

impl<T: AsRef<str>> ListBox<'_, T> {
    /// Sets the width and height. Zero uses the corresponding default dimension.
    /// A negative dimension aligns that edge relative to the available content region.
    #[inline]
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = size.into();
        self
    }

    /// Builds the visible list box and runs `f` with its owning Context current.
    ///
    /// Returns `None` without calling `f` if the list box is clipped.
    #[doc(alias = "BeginListBox", alias = "EndListBox")]
    pub fn build<R>(self, f: impl FnOnce() -> R) -> Option<R> {
        assert_finite_vec2("ListBox::build()", "size", self.size);
        let ui = self.ui;
        let label = self.label.as_ref();
        ui.with_bound_context(|| {
            let visible = ui.with_scratch_c_string(label, |label| unsafe {
                sys::igBeginListBox(label, self.size.into())
            });
            if !visible {
                return None;
            }
            let guard = ListBoxGuard(ui.begin_native_scope(NativeScopePop::EndListBox, "ListBox"));
            let result = f();
            drop(guard);
            Some(result)
        })
    }

    /// Builds a simple list box for choosing from a slice of values
    pub fn build_simple<V, L>(self, current_item: &mut usize, items: &[V], label_fn: &L) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let ui = self.ui;
        self.build(|| {
            let mut changed = false;
            for (idx, item) in items.iter().enumerate() {
                let text = label_fn(item);
                let selected = idx == *current_item;
                if ui.selectable_config(&text).selected(selected).build() {
                    *current_item = idx;
                    changed = true;
                }
            }
            changed
        })
        .unwrap_or(false)
    }

    /// Builds a simple list box for choosing from a slice of values using an `i32` index.
    ///
    /// This is useful when you want to represent \"no selection\" with `-1`, matching Dear ImGui's
    /// list-box patterns that use an `int*` index.
    pub fn build_simple_i32<V, L>(self, current_item: &mut i32, items: &[V], label_fn: &L) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let ui = self.ui;
        self.build(|| {
            let mut changed = false;
            for (idx, item) in items.iter().enumerate() {
                let Ok(index) = i32::try_from(idx) else {
                    break;
                };
                let text = label_fn(item);
                let selected = index == *current_item;
                if ui.selectable_config(&text).selected(selected).build() {
                    *current_item = index;
                    changed = true;
                }
            }
            changed
        })
        .unwrap_or(false)
    }
}

struct ListBoxGuard<'ui>(NativeScopeToken<'ui>);

impl Drop for ListBoxGuard<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}
