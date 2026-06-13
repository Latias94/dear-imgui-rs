//! List clipper (virtualized lists)
//!
//! Wrapper around Dear ImGui's list clipper to efficiently display large
//! lists by only processing visible items.
//!
use std::ops::Range;

use crate::Ui;
use crate::sys;

fn items_count_to_i32(items_count: usize, caller: &str) -> i32 {
    i32::try_from(items_count)
        .unwrap_or_else(|_| panic!("{caller} items_count exceeded Dear ImGui's i32 range"))
}

fn display_index_from_i32(index: i32, caller: &str) -> usize {
    assert!(index >= 0, "{caller} returned a negative display index");
    usize::try_from(index).expect("non-negative display index must fit usize")
}

/// Used to render only the visible items when displaying a
/// long list of items in a scrollable area.
///
/// For example, you can have a huge list of checkboxes.
/// Without the clipper you have to call `ui.checkbox(...)`
/// for every one, even if 99% of of them are not visible in
/// the current frame. Using the `ListClipper`, you can only
/// call `ui.checkbox(...)` for the currently visible items.
///
/// Note the efficiency of list clipper relies on the height
/// of each item being cheaply calculated. The current rust
/// bindings only works with a fixed height for all items.
pub struct ListClipper {
    items_count: usize,
    items_height: f32,
}

impl ListClipper {
    /// Begins configuring a list clipper.
    pub const fn new(items_count: usize) -> Self {
        ListClipper {
            items_count,
            items_height: -1.0,
        }
    }

    /// Manually set item height. If not set, the height of the first item is used for all subsequent rows.
    pub const fn items_height(mut self, items_height: f32) -> Self {
        self.items_height = items_height;
        self
    }

    pub fn begin(self, ui: &Ui) -> ListClipperToken<'_> {
        assert!(
            self.items_height.is_finite(),
            "ListClipper::begin() items_height must be finite"
        );
        let items_count = items_count_to_i32(self.items_count, "ListClipper::begin()");
        ui.run_with_bound_context(|| unsafe {
            let ptr = sys::ImGuiListClipper_ImGuiListClipper();
            if ptr.is_null() {
                panic!("ImGuiListClipper_ImGuiListClipper() returned null");
            }
            sys::ImGuiListClipper_Begin(ptr, items_count, self.items_height);
            ListClipperToken::new(ui, ptr)
        })
    }
}

/// List clipper is a mechanism to efficiently implement scrolling of
/// large lists with random access.
///
/// For example you have a list of 1 million buttons, and the list
/// clipper will help you only draw the ones which are visible.
pub struct ListClipperToken<'ui> {
    ui: &'ui Ui,
    list_clipper: *mut sys::ImGuiListClipper,
    ended: bool,
}

impl<'ui> ListClipperToken<'ui> {
    fn new(ui: &'ui Ui, list_clipper: *mut sys::ImGuiListClipper) -> Self {
        Self {
            ui,
            list_clipper,
            ended: false,
        }
    }

    /// Progress the list clipper.
    ///
    /// If this returns returns `true` then the you can loop between
    /// between `clipper.display_range()`.
    /// If this returns false, you must stop calling this method.
    ///
    /// Calling step again after it returns `false` will cause imgui
    /// to abort. This mirrors the C++ interface.
    ///
    /// It is recommended to use the iterator interface!
    pub fn step(&mut self) -> bool {
        if self.ended {
            panic!("ListClipperToken::step() called after the clipper has ended");
        }
        let ret = self
            .ui
            .run_with_bound_context(|| unsafe { sys::ImGuiListClipper_Step(self.list_clipper) });
        if !ret {
            self.ended = true;
        }
        ret
    }

    /// This is automatically called back the final call to
    /// `step`. You can call it sooner but typically not needed.
    pub fn end(&mut self) {
        if !self.ended {
            self.ui.run_with_bound_context(|| unsafe {
                sys::ImGuiListClipper_End(self.list_clipper);
            });
            self.ended = true;
        }
    }

    /// First item to call, updated each call to `step`
    pub fn display_start(&self) -> usize {
        display_index_from_i32(
            unsafe { (*self.list_clipper).DisplayStart },
            "ListClipperToken::display_start()",
        )
    }

    /// End of items to call (exclusive), updated each call to `step`
    pub fn display_end(&self) -> usize {
        display_index_from_i32(
            unsafe { (*self.list_clipper).DisplayEnd },
            "ListClipperToken::display_end()",
        )
    }

    /// Visible item range for the current step.
    pub fn display_range(&self) -> Range<usize> {
        self.display_start()..self.display_end()
    }

    /// Get an iterator which outputs all visible indexes. This is the
    /// recommended way of using the clipper.
    pub fn iter(self) -> ListClipperIterator<'ui> {
        ListClipperIterator::new(self)
    }
}

impl Drop for ListClipperToken<'_> {
    fn drop(&mut self) {
        self.ui.run_with_bound_context(|| unsafe {
            sys::ImGuiListClipper_destroy(self.list_clipper);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn step_after_end_panics_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_step_after_end").build(|| {
            let mut clipper = ListClipper::new(0).begin(ui);
            clipper.end();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = clipper.step();
            }));

            assert!(result.is_err());
        });
    }

    #[test]
    fn end_after_step_false_is_a_noop() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_end_after_step_false").build(|| {
            let mut clipper = ListClipper::new(0).begin(ui);
            assert!(!clipper.step());
            clipper.end();
        });
    }

    #[test]
    fn begin_rejects_invalid_inputs_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_invalid_inputs").build(|| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _clipper = ListClipper::new(usize::MAX).begin(ui);
                }))
                .is_err()
            );

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _clipper = ListClipper::new(1usize).items_height(f32::NAN).begin(ui);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn iterator_and_display_range_use_usize_indices() {
        let mut ctx = setup_context();
        ctx.io_mut().set_display_size([512.0, 512.0]);
        let ui = ctx.frame();

        ui.window("list_clipper_usize_indices")
            .size([256.0, 256.0], crate::Condition::Always)
            .build(|| {
                let mut clipper = ListClipper::new(3usize).items_height(1.0).begin(ui);
                while clipper.step() {
                    for index in clipper.display_range() {
                        let _: usize = index;
                        ui.text(format!("row {index}"));
                    }
                }

                let indices: Vec<usize> = ListClipper::new(3usize)
                    .items_height(1.0)
                    .begin(ui)
                    .iter()
                    .inspect(|index| ui.text(format!("row {index}")))
                    .collect();
                assert_eq!(indices, vec![0, 1, 2]);
            });
    }
}

pub struct ListClipperIterator<'ui> {
    list_clipper: ListClipperToken<'ui>,
    exhausted: bool,
    last_value: Option<usize>,
}

impl<'ui> ListClipperIterator<'ui> {
    fn new(list_clipper: ListClipperToken<'ui>) -> Self {
        Self {
            list_clipper,
            exhausted: false,
            last_value: None,
        }
    }
}

impl Iterator for ListClipperIterator<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.last_value {
                let next_value = value + 1;

                if next_value >= self.list_clipper.display_end() {
                    self.last_value = None;
                } else {
                    self.last_value = Some(next_value);
                }

                return Some(value);
            }

            if self.exhausted {
                // If the clipper is exhausted, don't call step again!
                return None;
            }

            // Advance the clipper
            let ret = self.list_clipper.step();
            if !ret {
                self.exhausted = true;
                return None;
            }

            // Setup iteration for this step's chunk
            let start = self.list_clipper.display_start();
            let end = self.list_clipper.display_end();

            if start < end {
                let next_value = start + 1;
                if next_value < end {
                    self.last_value = Some(next_value);
                }
                return Some(start);
            } else {
                self.last_value = None;
            }
        }
    }
}
