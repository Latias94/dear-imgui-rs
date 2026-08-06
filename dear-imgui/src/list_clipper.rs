//! List clipper (virtualized lists)
//!
//! Wrapper around Dear ImGui's list clipper to efficiently display large
//! lists by only processing visible items.
//!
use std::ops::Range;

use crate::Ui;
use crate::sys;

mod registry;

use registry::ClipperHandle;

fn items_count_to_i32(items_count: usize, caller: &str) -> i32 {
    i32::try_from(items_count)
        .unwrap_or_else(|_| panic!("{caller} items_count exceeded Dear ImGui's i32 range"))
}

fn validate_items_height(items_height: f32, caller: &str) {
    assert!(
        items_height.is_finite(),
        "{caller} items_height must be finite"
    );
    assert!(
        items_height == -1.0 || items_height > 0.0,
        "{caller} items_height must be -1.0 for automatic measurement or a positive value"
    );
}

fn final_unknown_count_to_i32(items_count: usize, caller: &str) -> i32 {
    assert!(
        items_count <= i32::MAX as usize,
        "{caller} final_items_count exceeded Dear ImGui's i32 range"
    );
    items_count as i32
}

fn display_index_from_i32(index: i32, caller: &str) -> usize {
    assert!(index >= 0, "{caller} returned a negative display index");
    usize::try_from(index).expect("non-negative display index must fit usize")
}

pub(crate) unsafe fn forget_context_clippers(context: *mut sys::ImGuiContext) -> usize {
    unsafe { registry::forget_context(context) }
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

    /// Configure a clipper whose final item count is discovered during traversal.
    pub const fn unknown_count() -> UnknownCountListClipper {
        UnknownCountListClipper { items_height: -1.0 }
    }

    /// Manually set item height. If not set, the height of the first item is used for all subsequent rows.
    pub const fn items_height(mut self, items_height: f32) -> Self {
        self.items_height = items_height;
        self
    }

    pub fn begin(self, ui: &Ui) -> ListClipperToken<'_> {
        assert!(
            self.items_count < i32::MAX as usize,
            "ListClipper::begin() known items_count must be less than i32::MAX; use ListClipper::unknown_count() for the sentinel protocol"
        );
        validate_items_height(self.items_height, "ListClipper::begin()");
        let items_count = items_count_to_i32(self.items_count, "ListClipper::begin()");
        ListClipperToken::new(
            ActiveListClipper::begin(ui, items_count, self.items_height, "ListClipper::begin()"),
            self.items_count,
        )
    }
}

/// Builder for an unknown-count list clipper.
pub struct UnknownCountListClipper {
    items_height: f32,
}

impl UnknownCountListClipper {
    /// Set a fixed item height. The first submitted item is measured when omitted.
    pub const fn items_height(mut self, items_height: f32) -> Self {
        self.items_height = items_height;
        self
    }

    /// Begin unknown-count clipping.
    pub fn begin(self, ui: &Ui) -> UnknownCountListClipperToken<'_> {
        validate_items_height(self.items_height, "UnknownCountListClipper::begin()");
        UnknownCountListClipperToken {
            active: ActiveListClipper::begin(
                ui,
                i32::MAX,
                self.items_height,
                "UnknownCountListClipper::begin()",
            ),
        }
    }
}

struct ActiveListClipper<'ui> {
    ui: &'ui Ui,
    handle: ClipperHandle,
    stepped: bool,
    ended: bool,
    registered: bool,
    retain_registration_after_exhaustion: bool,
}

impl<'ui> ActiveListClipper<'ui> {
    fn begin(ui: &'ui Ui, items_count: i32, items_height: f32, caller: &str) -> Self {
        ui.run_with_bound_context(|| unsafe {
            registry::assert_can_begin(ui.context_raw(), caller);
            let ptr = sys::ImGuiListClipper_ImGuiListClipper();
            if ptr.is_null() {
                panic!("ImGuiListClipper_ImGuiListClipper() returned null");
            }
            sys::ImGuiListClipper_Begin(ptr, items_count, items_height);
            let handle = registry::register_current(ui.context_raw(), ptr, caller);
            Self {
                ui,
                handle,
                stepped: false,
                ended: false,
                registered: true,
                retain_registration_after_exhaustion: items_count == i32::MAX,
            }
        })
    }

    fn include_items_by_index(&mut self, item_begin: i32, item_end: i32, caller: &str) {
        self.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(self.handle, caller);
            sys::ImGuiListClipper_IncludeItemsByIndex(ptr, item_begin, item_end);
        });
    }

    fn step(&mut self, caller: &str) -> bool {
        self.stepped = true;
        let has_range = self.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(self.handle, caller);
            sys::ImGuiListClipper_Step(ptr)
        });
        if !has_range {
            self.ended = true;
            if !self.retain_registration_after_exhaustion {
                self.ui.run_with_bound_context(|| unsafe {
                    registry::complete(self.handle);
                });
                self.registered = false;
            }
        }
        has_range
    }

    fn end(&mut self, caller: &str) {
        if !self.ended {
            self.ui.run_with_bound_context(|| unsafe {
                let ptr = registry::assert_current(self.handle, caller);
                sys::ImGuiListClipper_End(ptr);
            });
            self.ended = true;
            self.ui.run_with_bound_context(|| unsafe {
                registry::complete(self.handle);
            });
            self.registered = false;
        }
    }

    fn display_start(&self, caller: &str) -> usize {
        self.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(self.handle, caller);
            display_index_from_i32((*ptr).DisplayStart, caller)
        })
    }

    fn display_end(&self, caller: &str) -> usize {
        self.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(self.handle, caller);
            display_index_from_i32((*ptr).DisplayEnd, caller)
        })
    }
}

impl Drop for ActiveListClipper<'_> {
    fn drop(&mut self) {
        self.ui.run_with_bound_context(|| unsafe {
            if self.registered {
                registry::release(self.handle);
            } else {
                sys::ImGuiListClipper_destroy(self.handle.ptr());
            }
        });
    }
}

/// List clipper is a mechanism to efficiently implement scrolling of
/// large lists with random access.
///
/// For example you have a list of 1 million buttons, and the list
/// clipper will help you only draw the ones which are visible.
///
/// Nested clippers must be operated in LIFO order and from the same window and table scope where
/// they began. Dropping tokens out of order is supported; native cleanup is deferred until all
/// clippers above the dropped token have exited.
pub struct ListClipperToken<'ui> {
    active: ActiveListClipper<'ui>,
    items_count: usize,
}

impl<'ui> ListClipperToken<'ui> {
    fn new(active: ActiveListClipper<'ui>, items_count: usize) -> Self {
        Self {
            active,
            items_count,
        }
    }

    /// Keep one item from being clipped, regardless of its visibility.
    ///
    /// This must be called before the first [`Self::step`].
    #[doc(alias = "IncludeItemByIndex")]
    pub fn include_item_by_index(&mut self, item_index: usize) {
        let item_end = item_index
            .checked_add(1)
            .expect("ListClipperToken::include_item_by_index() index overflowed");
        self.include_items_by_index(item_index..item_end);
    }

    /// Keep a half-open item range from being clipped, regardless of visibility.
    ///
    /// This must be called before the first [`Self::step`]. Empty ranges are a no-op.
    #[doc(alias = "IncludeItemsByIndex")]
    pub fn include_items_by_index(&mut self, items: Range<usize>) {
        assert!(
            !self.active.ended && !self.active.stepped,
            "ListClipperToken::include_items_by_index() must be called before the first step()"
        );
        assert!(
            items.start <= items.end,
            "ListClipperToken::include_items_by_index() range start must not exceed its end"
        );
        assert!(
            items.end <= self.items_count,
            "ListClipperToken::include_items_by_index() range exceeds the item count"
        );
        if items.is_empty() {
            return;
        }
        let item_begin = items_count_to_i32(
            items.start,
            "ListClipperToken::include_items_by_index() range start",
        );
        let item_end = items_count_to_i32(
            items.end,
            "ListClipperToken::include_items_by_index() range end",
        );
        self.active.include_items_by_index(
            item_begin,
            item_end,
            "ListClipperToken::include_items_by_index()",
        );
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
        if self.active.ended {
            panic!("ListClipperToken::step() called after the clipper has ended");
        }
        self.active.step("ListClipperToken::step()")
    }

    /// This is automatically called back the final call to
    /// `step`. You can call it sooner but typically not needed.
    pub fn end(&mut self) {
        self.active.end("ListClipperToken::end()");
    }

    /// First item to call, updated each call to `step`
    pub fn display_start(&self) -> usize {
        self.active
            .display_start("ListClipperToken::display_start()")
    }

    /// End of items to call (exclusive), updated each call to `step`
    pub fn display_end(&self) -> usize {
        self.active.display_end("ListClipperToken::display_end()")
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

/// Active unknown-count list clipper.
///
/// Call [`Self::finish`] with the discovered item count so Dear ImGui can restore the final cursor
/// position and scrollbar extent. Nested clippers follow the same LIFO and UI-scope rules as
/// [`ListClipperToken`].
#[must_use = "call finish(final_items_count) to finalize an unknown-count list"]
pub struct UnknownCountListClipperToken<'ui> {
    active: ActiveListClipper<'ui>,
}

impl UnknownCountListClipperToken<'_> {
    /// Keep one index from being clipped. Call this before [`Self::next_range`].
    #[doc(alias = "IncludeItemByIndex")]
    pub fn include_item_by_index(&mut self, item_index: usize) {
        let item_end = item_index
            .checked_add(1)
            .expect("UnknownCountListClipperToken::include_item_by_index() index overflowed");
        self.include_items_by_index(item_index..item_end);
    }

    /// Keep a half-open index range from being clipped. Call this before [`Self::next_range`].
    #[doc(alias = "IncludeItemsByIndex")]
    pub fn include_items_by_index(&mut self, items: Range<usize>) {
        assert!(
            !self.active.ended && !self.active.stepped,
            "UnknownCountListClipperToken::include_items_by_index() must be called before the first next_range()"
        );
        assert!(
            items.start <= items.end,
            "UnknownCountListClipperToken::include_items_by_index() range start must not exceed its end"
        );
        assert!(
            items.end <= i32::MAX as usize,
            "UnknownCountListClipperToken::include_items_by_index() range exceeded Dear ImGui's i32 range"
        );
        if items.is_empty() {
            return;
        }
        let start = items_count_to_i32(
            items.start,
            "UnknownCountListClipperToken::include_items_by_index() range start",
        );
        let end = items_count_to_i32(
            items.end,
            "UnknownCountListClipperToken::include_items_by_index() range end",
        );
        self.active.include_items_by_index(
            start,
            end,
            "UnknownCountListClipperToken::include_items_by_index()",
        );
    }

    /// Advance to the next range that may contain visible items.
    ///
    /// Once exhausted, this method remains fused and returns `None` without entering FFI again.
    pub fn next_range(&mut self) -> Option<Range<usize>> {
        if self.active.ended {
            return None;
        }
        let has_range = self
            .active
            .step("UnknownCountListClipperToken::next_range()");
        if !has_range {
            return None;
        }
        Some(
            self.active
                .display_start("UnknownCountListClipperToken::next_range() start")
                ..self
                    .active
                    .display_end("UnknownCountListClipperToken::next_range() end"),
        )
    }

    /// Finalize the list with its discovered item count.
    ///
    /// Automatic height requires one submitted measurement item followed by another
    /// [`Self::next_range`] call before finishing a non-empty list. Finalization seeks directly;
    /// it never drains unsubmitted ranges, which is important inside frozen table rows.
    #[doc(alias = "SeekCursorForItem")]
    pub fn finish(mut self, final_items_count: usize) {
        let final_items_count =
            final_unknown_count_to_i32(final_items_count, "UnknownCountListClipperToken::finish()");

        let items_height = self.active.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(
                self.active.handle,
                "UnknownCountListClipperToken::finish()",
            );
            (*ptr).ItemsHeight
        });
        if final_items_count == 0 && !(items_height.is_finite() && items_height > 0.0) {
            self.active.end("UnknownCountListClipperToken::finish()");
            return;
        }
        assert!(
            items_height.is_finite() && items_height > 0.0,
            "UnknownCountListClipperToken::finish() could not determine a positive item height"
        );
        self.active.ui.run_with_bound_context(|| unsafe {
            let ptr = registry::assert_current(
                self.active.handle,
                "UnknownCountListClipperToken::finish()",
            );
            sys::ImGuiListClipper_SeekCursorForItem(ptr, final_items_count);
        });
        self.active.end("UnknownCountListClipperToken::finish()");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas().build();
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
                    let _clipper = ListClipper::new(i32::MAX as usize).begin(ui);
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

    #[test]
    fn include_items_returns_non_visible_ranges_and_enforces_call_order() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_includes")
            .size([96.0, 96.0], crate::Condition::Always)
            .build(|| {
                let mut clipper = ListClipper::new(1_000usize).items_height(16.0).begin(ui);
                clipper.include_item_by_index(900);
                clipper.include_items_by_index(950..953);

                let mut included = [false; 4];
                while clipper.step() {
                    for index in clipper.display_range() {
                        if index == 900 {
                            included[0] = true;
                        }
                        if (950..953).contains(&index) {
                            included[index - 949] = true;
                        }
                        ui.text(format!("row {index}"));
                    }
                }
                assert!(included.into_iter().all(|seen| seen));

                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        clipper.include_item_by_index(1);
                    }))
                    .is_err()
                );
            });
    }

    #[test]
    fn include_items_rejects_invalid_ranges_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_invalid_includes").build(|| {
            for (start, end) in [(5usize, 4usize), (0, 11)] {
                let range = std::ops::Range { start, end };
                let mut clipper = ListClipper::new(10usize).begin(ui);
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        clipper.include_items_by_index(range.clone());
                    }))
                    .is_err()
                );
            }
        });
    }

    #[test]
    fn unknown_count_finish_restores_the_final_cursor_position() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("unknown_count_cursor").build(|| {
            let start_y = ui.cursor_screen_pos()[1];
            let mut clipper = ListClipper::unknown_count().items_height(10.0).begin(ui);
            assert!(clipper.next_range().is_some());
            clipper.finish(5);
            let end_y = ui.cursor_screen_pos()[1];
            assert!((end_y - start_y - 50.0).abs() < 0.01);
        });
    }

    #[test]
    fn unknown_count_next_range_is_fused_after_exhaustion() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("unknown_count_fused").build(|| {
            let mut clipper = ListClipper::unknown_count().items_height(10.0).begin(ui);
            let mut steps = 0;
            while clipper.next_range().is_some() {
                steps += 1;
                assert!(steps < 16, "unknown-count clipper did not exhaust");
            }
            assert!(clipper.next_range().is_none());
            clipper.finish(3);
        });
    }

    #[test]
    fn unknown_count_empty_list_needs_no_measured_height() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("unknown_count_empty").build(|| {
            ListClipper::unknown_count().begin(ui).finish(0);
        });
    }

    #[test]
    fn unknown_count_empty_list_restores_cursor_after_a_scrolled_seek() {
        let mut ctx = setup_context();
        {
            let ui = ctx.frame();
            ui.window("unknown_count_empty_scrolled")
                .size([96.0, 96.0], crate::Condition::Always)
                .build(|| {
                    ui.dummy([1.0, 2_000.0]);
                    ui.set_scroll_y(500.0);
                });
        }
        let _ = ctx.render_legacy();

        let ui = ctx.frame();
        ui.window("unknown_count_empty_scrolled")
            .size([96.0, 96.0], crate::Condition::Always)
            .build(|| {
                assert!(ui.scroll_y() > 0.0);
                let start_y = ui.cursor_screen_pos()[1];
                let mut clipper = ListClipper::unknown_count().items_height(10.0).begin(ui);
                let first_range = clipper
                    .next_range()
                    .expect("a fixed-height sentinel list should produce a visible range");
                assert!(first_range.start > 0);
                clipper.finish(0);
                let end_y = ui.cursor_screen_pos()[1];
                assert!((end_y - start_y).abs() < 0.01);
            });
    }

    #[test]
    fn unknown_count_finish_does_not_drain_frozen_table_rows() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("unknown_count_frozen_table_window").build(|| {
            ui.table("unknown_count_frozen_table")
                .flags(crate::TableFlags::SCROLL_Y)
                .outer_size([96.0, 64.0])
                .freeze(0, 1)
                .column("value")
                .done()
                .build(|ui| {
                    ListClipper::unknown_count()
                        .items_height(10.0)
                        .begin(ui)
                        .finish(0);
                });
        });
    }

    #[test]
    fn unknown_count_auto_height_measures_the_first_item() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("unknown_count_auto_height").build(|| {
            let mut clipper = ListClipper::unknown_count().begin(ui);
            assert_eq!(clipper.next_range(), Some(0..1));
            ui.dummy([1.0, 10.0]);

            let visible = clipper
                .next_range()
                .expect("measuring the first item should produce the visible range");
            let raw = clipper.active.handle.ptr();
            let measured_height = unsafe { (*raw).ItemsHeight };
            assert!(measured_height.is_finite() && measured_height > 0.0);
            for index in visible {
                if index >= 5 {
                    break;
                }
                ui.dummy([1.0, 10.0]);
            }
            let expected_y = unsafe {
                ((*raw).StartPosY + (*raw).StartSeekOffsetY + 5.0 * f64::from(measured_height))
                    as f32
            };

            clipper.finish(5);
            assert!((ui.cursor_screen_pos()[1] - expected_y).abs() < 0.01);
        });
    }

    #[test]
    fn out_of_order_drop_defers_native_cleanup_until_lifo_is_restored() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();

        ui.window("list_clipper_out_of_order_drop").build(|| {
            let outer = ListClipper::new(4).items_height(10.0).begin(ui);
            let inner = ListClipper::unknown_count().items_height(10.0).begin(ui);
            assert_eq!(registry::active_count(context), 2);

            drop(outer);
            assert_eq!(registry::active_count(context), 2);

            drop(inner);
            assert_eq!(registry::active_count(context), 0);

            let mut next = ListClipper::new(1).items_height(10.0).begin(ui);
            assert!(next.step());
            ui.dummy([1.0, 10.0]);
            assert!(!next.step());
        });
    }

    #[test]
    fn wrong_scope_drop_uses_layout_neutral_cleanup() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();

        ui.window("list_clipper_drop_scope_owner").build(|| {
            let mut clipper = Some(ListClipper::new(1).items_height(10.0).begin(ui));
            ui.window("list_clipper_drop_scope_other").build(|| {
                drop(clipper.take());
            });
            assert_eq!(registry::active_count(context), 0);

            let mut next = ListClipper::new(1).items_height(10.0).begin(ui);
            assert!(next.step());
            ui.dummy([1.0, 10.0]);
            assert!(!next.step());
        });
    }

    #[test]
    fn clipper_dropped_after_its_window_does_not_poison_the_frame() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();
        let mut clipper = None;

        ui.window("list_clipper_late_drop_owner").build(|| {
            clipper = Some(ListClipper::new(1).items_height(10.0).begin(ui));
        });
        drop(clipper.take());
        drop(clipper);
        assert_eq!(registry::active_count(context), 0);
        assert!(ctx.render_legacy().valid());
    }

    #[test]
    fn forgotten_clipper_rejects_only_the_current_frame() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();
        let clipper = ui
            .window("list_clipper_forgotten")
            .build(|| ListClipper::new(1).items_height(10.0).begin(ui))
            .expect("test window should be visible");
        std::mem::forget(clipper);
        assert_eq!(registry::active_count(context), 1);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx.render_legacy();
        }));
        assert!(result.is_err());
        assert_eq!(registry::active_count(context), 0);
        assert_eq!(
            ctx.frame_lifecycle_state(),
            crate::FrameLifecycleState::Idle
        );

        let ui = ctx.frame();
        ui.window("list_clipper_recovered_frame").build(|| {
            let mut next = ListClipper::new(1).items_height(10.0).begin(ui);
            assert!(next.step());
            ui.dummy([1.0, 10.0]);
            assert!(!next.step());
        });
        assert!(ctx.render_legacy().valid());
    }

    #[test]
    fn reopening_the_same_window_does_not_reuse_the_old_clipper_scope() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();
        let mut escaped = ui
            .window("list_clipper_reopened_scope")
            .build(|| ListClipper::new(1).items_height(10.0).begin(ui))
            .expect("test window should be visible");
        assert_eq!(registry::active_count(context), 1);

        ui.window("list_clipper_reopened_scope").build(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = escaped.step();
            }));
            assert!(result.is_err());
            drop(escaped);
            assert_eq!(registry::active_count(context), 0);

            drop(ListClipper::new(0).items_height(10.0).begin(ui));
        });

        assert_eq!(registry::active_count(context), 0);
        assert!(ctx.render_legacy().valid());
    }

    #[test]
    fn reopening_the_same_table_does_not_reuse_the_old_clipper_scope() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();

        ui.window("list_clipper_reopened_table_window").build(|| {
            let mut escaped = {
                let Some(_table) = ui.begin_table("list_clipper_reopened_table", 1) else {
                    panic!("test table should be visible");
                };
                ListClipper::new(1).items_height(10.0).begin(ui)
            };
            assert_eq!(registry::active_count(context), 1);

            let Some(_table) = ui.begin_table("list_clipper_reopened_table", 1) else {
                panic!("test table should be visible");
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = escaped.step();
            }));
            assert!(result.is_err());
            drop(escaped);
            assert_eq!(registry::active_count(context), 0);
        });

        assert!(ctx.render_legacy().valid());
    }

    #[test]
    fn nested_clipper_begin_requires_the_owner_window_and_table_scope() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_begin_scope_owner").build(|| {
            let mut outer = ListClipper::new(1).items_height(10.0).begin(ui);

            ui.window("list_clipper_begin_scope_other").build(|| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ListClipper::new(1).items_height(10.0).begin(ui);
                }));
                assert!(result.is_err());
            });

            {
                let Some(_table) = ui.begin_table("list_clipper_begin_scope_table", 1) else {
                    panic!("test table should be visible");
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ListClipper::new(1).items_height(10.0).begin(ui);
                }));
                assert!(result.is_err());
            }

            assert!(outer.step());
            ui.dummy([1.0, 10.0]);
            assert!(!outer.step());
        });
    }

    #[test]
    fn nested_clipper_operations_require_lifo_order() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_lifo_operations").build(|| {
            let mut outer = ListClipper::new(1).items_height(10.0).begin(ui);
            let inner = ListClipper::new(1).items_height(10.0).begin(ui);

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = outer.step();
            }));
            assert!(result.is_err());

            drop(inner);
            assert!(outer.step());
            ui.dummy([1.0, 10.0]);
            assert!(!outer.step());
        });
    }

    #[test]
    fn completed_inner_clipper_releases_the_native_lifo_scope_before_drop() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();

        ui.window("list_clipper_completed_inner").build(|| {
            let mut outer = ListClipper::new(1).items_height(10.0).begin(ui);
            let mut inner = ListClipper::new(1).items_height(10.0).begin(ui);

            assert!(inner.step());
            ui.dummy([1.0, 10.0]);
            assert!(!inner.step());
            assert_eq!(registry::active_count(context), 1);

            assert!(outer.step());
            ui.dummy([1.0, 10.0]);
            assert!(!outer.step());
            assert_eq!(registry::active_count(context), 0);

            drop(inner);
        });
    }

    #[test]
    fn explicitly_ended_token_does_not_block_a_different_window_scope() {
        let mut ctx = setup_context();
        let context = ctx.as_raw();
        let ui = ctx.frame();
        let ended = ui
            .window("list_clipper_explicit_end_owner")
            .build(|| {
                let mut clipper = ListClipper::new(1).items_height(10.0).begin(ui);
                clipper.end();
                clipper
            })
            .expect("test window should be visible");
        assert_eq!(registry::active_count(context), 0);

        ui.window("list_clipper_after_explicit_end").build(|| {
            let mut next = ListClipper::new(1).items_height(10.0).begin(ui);
            assert!(next.step());
            ui.dummy([1.0, 10.0]);
            assert!(!next.step());
        });

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ended.display_range();
            }))
            .is_err()
        );
        drop(ended);
    }

    #[test]
    fn clipper_operations_require_the_begin_window_and_table_scope() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("list_clipper_scope_owner").build(|| {
            let mut clipper = ListClipper::new(1).items_height(10.0).begin(ui);

            ui.window("list_clipper_other_window").build(|| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = clipper.step();
                }));
                assert!(result.is_err());
            });

            {
                let Some(_table) = ui.begin_table("list_clipper_other_table", 1) else {
                    panic!("test table should be visible");
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = clipper.step();
                }));
                assert!(result.is_err());
            }

            clipper.end();
        });
    }

    #[test]
    fn unknown_count_validates_final_count_before_ffi() {
        assert_eq!(
            final_unknown_count_to_i32(
                i32::MAX as usize,
                "unknown_count_validates_final_count_before_ffi"
            ),
            i32::MAX
        );

        let mut ctx = setup_context();
        let ui = ctx.frame();
        ui.window("unknown_count_overflow").build(|| {
            let clipper = ListClipper::unknown_count().items_height(10.0).begin(ui);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    clipper.finish(i32::MAX as usize + 1);
                }))
                .is_err()
            );
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
