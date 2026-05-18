//! Legacy columns API
//!
//! Thin wrappers for the old Columns layout system. New code should prefer
//! the `table` API (`widget::table`) which supersedes Columns with more
//! features and better user experience.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
use crate::sys;
use crate::{Id, Ui};
use bitflags::bitflags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

fn current_columns() -> *mut sys::ImGuiOldColumns {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        if window.is_null() {
            std::ptr::null_mut()
        } else {
            (*window).DC.CurrentColumns
        }
    }
}

fn assert_no_current_columns(caller: &str) {
    assert!(
        current_columns().is_null(),
        "{caller} cannot be called while another legacy columns layout is active"
    );
}

fn assert_current_columns(caller: &str) -> *mut sys::ImGuiOldColumns {
    let columns = current_columns();
    assert!(
        !columns.is_null(),
        "{caller} must be called inside a legacy columns layout"
    );
    columns
}

fn columns_count_to_i32(count: usize, caller: &str) -> i32 {
    assert!(count >= 1, "{caller} count must be at least 1");
    i32::try_from(count)
        .unwrap_or_else(|_| panic!("{caller} count exceeded Dear ImGui's i32 range"))
}

fn column_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 1, "{caller} returned an invalid legacy column count");
    usize::try_from(raw).expect("positive legacy column count must fit usize")
}

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn validate_old_column_flags(caller: &str, flags: OldColumnFlags) {
    let unsupported = flags.bits() & !OldColumnFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiOldColumnFlags bits: 0x{unsupported:X}"
    );
}

fn assert_valid_column_in(
    columns: *mut sys::ImGuiOldColumns,
    column: OldColumnIndex,
    caller: &str,
) -> i32 {
    let column_index = column.into_i32(caller);
    let column_count = unsafe { (*columns).Count };
    assert!(
        (0..column_count).contains(&column_index),
        "{caller} column index {column_index} is outside the current legacy columns range 0..{column_count}"
    );
    column_index
}

fn resolve_column_ref(column: OldColumnRef, caller: &str) -> i32 {
    let columns = assert_current_columns(caller);
    let column_index = match column {
        OldColumnRef::Current => unsafe { (*columns).Current },
        OldColumnRef::Index(index) => index.into_i32(caller),
    };
    let column_count = unsafe { (*columns).Count };
    assert!(
        (0..column_count).contains(&column_index),
        "{caller} column index {column_index} is outside the current legacy columns range 0..{column_count}"
    );
    column_index
}

fn resolve_column_query_ref(column: OldColumnRef, caller: &str) -> i32 {
    match column {
        OldColumnRef::Current if current_columns().is_null() => -1,
        _ => resolve_column_ref(column, caller),
    }
}

fn resolve_column_offset_ref(offset: OldColumnOffsetRef, caller: &str) -> i32 {
    let columns = assert_current_columns(caller);
    let column_index = match offset {
        OldColumnOffsetRef::Current => unsafe { (*columns).Current },
        OldColumnOffsetRef::Column(index) => index.into_i32(caller),
        OldColumnOffsetRef::Trailing => unsafe { (*columns).Count },
    };
    let upper_bound = unsafe { (*columns).Count };
    assert!(
        (0..=upper_bound).contains(&column_index),
        "{caller} column offset index {column_index} is outside the current legacy columns offset range 0..={upper_bound}"
    );
    column_index
}

fn resolve_column_offset_query_ref(offset: OldColumnOffsetRef, caller: &str) -> i32 {
    match offset {
        OldColumnOffsetRef::Current if current_columns().is_null() => -1,
        _ => resolve_column_offset_ref(offset, caller),
    }
}

/// Concrete zero-based legacy Columns API column index.
///
/// This represents a real column only. Dear ImGui's `-1` current-column sentinel
/// is represented by [`OldColumnRef::Current`] and [`OldColumnOffsetRef::Current`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OldColumnIndex(usize);

impl OldColumnIndex {
    /// The first legacy column.
    pub const ZERO: Self = Self(0);

    /// Create a legacy column index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    fn into_i32(self, caller: &str) -> i32 {
        i32::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's i32 range");
        })
    }

    #[inline]
    fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative column index");
        Self(usize::try_from(raw).expect("non-negative column index must fit usize"))
    }
}

impl From<usize> for OldColumnIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<OldColumnIndex> for usize {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for OldColumnIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<OldColumnIndex> for usize {
    #[inline]
    fn eq(&self, other: &OldColumnIndex) -> bool {
        *self == other.get()
    }
}

/// Legacy column selector for APIs that accept Dear ImGui's current-column sentinel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OldColumnRef {
    /// Use the current legacy column.
    #[default]
    Current,
    /// Use a concrete legacy column index.
    Index(OldColumnIndex),
}

impl OldColumnRef {
    /// Current legacy column.
    pub const CURRENT: Self = Self::Current;

    /// Select a concrete legacy column.
    #[inline]
    pub const fn index(index: OldColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<OldColumnIndex> for OldColumnRef {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for OldColumnRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Index(OldColumnIndex::new(index))
    }
}

/// Legacy column offset-line selector.
///
/// Offset APIs operate on column boundary lines. Concrete column indices select
/// the start line for that column, while [`OldColumnOffsetRef::Trailing`] selects
/// the right-most line after the final column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OldColumnOffsetRef {
    /// Use the current legacy column boundary.
    #[default]
    Current,
    /// Use the start boundary for a concrete legacy column.
    Column(OldColumnIndex),
    /// Use the right-most boundary after the final legacy column.
    Trailing,
}

impl OldColumnOffsetRef {
    /// Current legacy column boundary.
    pub const CURRENT: Self = Self::Current;

    /// Right-most boundary after the final legacy column.
    pub const TRAILING: Self = Self::Trailing;

    /// Select the start boundary for a concrete legacy column.
    #[inline]
    pub const fn column(index: OldColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<OldColumnIndex> for OldColumnOffsetRef {
    #[inline]
    fn from(index: OldColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<usize> for OldColumnOffsetRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Column(OldColumnIndex::new(index))
    }
}

bitflags! {
    /// Flags for old columns system
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct OldColumnFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiOldColumnFlags_None as i32;
        /// Disable column dividers
        const NO_BORDER = sys::ImGuiOldColumnFlags_NoBorder as i32;
        /// Disable resizing columns by dragging dividers
        const NO_RESIZE = sys::ImGuiOldColumnFlags_NoResize as i32;
        /// Disable column width preservation when the total width changes
        const NO_PRESERVE_WIDTHS = sys::ImGuiOldColumnFlags_NoPreserveWidths as i32;
        /// Disable forcing columns to fit within window
        const NO_FORCE_WITHIN_WINDOW = sys::ImGuiOldColumnFlags_NoForceWithinWindow as i32;
        /// Restore pre-1.51 behavior of extending the parent window contents size
        const GROW_PARENT_CONTENTS_SIZE = sys::ImGuiOldColumnFlags_GrowParentContentsSize as i32;
    }
}

impl Default for OldColumnFlags {
    fn default() -> Self {
        OldColumnFlags::NONE
    }
}

/// # Columns
impl Ui {
    /// Creates columns layout.
    ///
    /// # Arguments
    /// * `count` - Number of columns (must be >= 1)
    /// * `id` - Optional ID for the columns (can be empty string)
    /// * `border` - Whether to draw borders between columns
    #[doc(alias = "Columns")]
    pub fn columns(&self, count: usize, id: impl AsRef<str>, border: bool) {
        let count = columns_count_to_i32(count, "Ui::columns()");
        unsafe { sys::igColumns(count, self.scratch_txt(id), border) }
    }

    /// Begin columns layout with advanced flags.
    ///
    /// # Arguments
    /// * `id` - ID for the columns
    /// * `count` - Number of columns (must be >= 1)
    /// * `flags` - Column flags
    #[doc(alias = "BeginColumns")]
    pub fn begin_columns(&self, id: impl AsRef<str>, count: usize, flags: OldColumnFlags) {
        let count = columns_count_to_i32(count, "Ui::begin_columns()");
        validate_old_column_flags("Ui::begin_columns()", flags);
        assert_no_current_columns("Ui::begin_columns()");
        unsafe { sys::igBeginColumns(self.scratch_txt(id), count, flags.bits()) }
    }

    /// Begin columns layout with advanced flags and return a token that ends columns on drop.
    #[doc(alias = "BeginColumns")]
    pub fn begin_columns_token(
        &self,
        id: impl AsRef<str>,
        count: usize,
        flags: OldColumnFlags,
    ) -> ColumnsToken<'_> {
        self.begin_columns(id, count, flags);
        ColumnsToken { ui: self }
    }

    /// End columns layout.
    #[doc(alias = "EndColumns")]
    pub fn end_columns(&self) {
        assert_current_columns("Ui::end_columns()");
        unsafe { sys::igEndColumns() }
    }

    /// Switches to the next column.
    ///
    /// If the current row is finished, switches to first column of the next row
    #[doc(alias = "NextColumn")]
    pub fn next_column(&self) {
        unsafe { sys::igNextColumn() }
    }

    /// Returns the index of the current column
    #[doc(alias = "GetColumnIndex")]
    pub fn current_column_index(&self) -> OldColumnIndex {
        OldColumnIndex::from_i32(
            unsafe { sys::igGetColumnIndex() },
            "Ui::current_column_index()",
        )
    }

    /// Returns the width of the current column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn current_column_width(&self) -> f32 {
        unsafe { sys::igGetColumnWidth(-1) }
    }

    /// Returns the width of the given column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn column_width(&self, column: impl Into<OldColumnRef>) -> f32 {
        let column_index = resolve_column_query_ref(column.into(), "Ui::column_width()");
        unsafe { sys::igGetColumnWidth(column_index) }
    }

    /// Sets the width of the current column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_current_column_width(&self, width: f32) {
        assert_current_columns("Ui::set_current_column_width()");
        assert_non_negative_f32("Ui::set_current_column_width()", "width", width);
        unsafe { sys::igSetColumnWidth(-1, width) };
    }

    /// Sets the width of the given column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_column_width(&self, column: impl Into<OldColumnIndex>, width: f32) {
        let columns = assert_current_columns("Ui::set_column_width()");
        let column_index = assert_valid_column_in(columns, column.into(), "Ui::set_column_width()");
        assert_non_negative_f32("Ui::set_column_width()", "width", width);
        unsafe { sys::igSetColumnWidth(column_index, width) };
    }

    /// Returns the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn current_column_offset(&self) -> f32 {
        unsafe { sys::igGetColumnOffset(-1) }
    }

    /// Returns the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn column_offset(&self, offset: impl Into<OldColumnOffsetRef>) -> f32 {
        let column_index = resolve_column_offset_query_ref(offset.into(), "Ui::column_offset()");
        unsafe { sys::igGetColumnOffset(column_index) }
    }

    /// Sets the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_current_column_offset(&self, offset_x: f32) {
        assert_current_columns("Ui::set_current_column_offset()");
        assert_non_negative_f32("Ui::set_current_column_offset()", "offset_x", offset_x);
        unsafe { sys::igSetColumnOffset(-1, offset_x) };
    }

    /// Sets the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_column_offset(&self, offset: impl Into<OldColumnOffsetRef>, offset_x: f32) {
        let column_index = resolve_column_offset_ref(offset.into(), "Ui::set_column_offset()");
        assert_non_negative_f32("Ui::set_column_offset()", "offset_x", offset_x);
        unsafe { sys::igSetColumnOffset(column_index, offset_x) };
    }

    /// Returns the current amount of columns
    #[doc(alias = "GetColumnsCount")]
    pub fn column_count(&self) -> usize {
        column_count_from_i32(unsafe { sys::igGetColumnsCount() }, "Ui::column_count()")
    }

    // ============================================================================
    // Advanced column utilities
    // ============================================================================

    /// Push column clip rect for the given column index.
    /// This is useful for custom drawing within columns.
    #[doc(alias = "PushColumnClipRect")]
    pub fn push_column_clip_rect(&self, column: impl Into<OldColumnIndex>) {
        let columns = assert_current_columns("Ui::push_column_clip_rect()");
        let column_index =
            assert_valid_column_in(columns, column.into(), "Ui::push_column_clip_rect()");
        unsafe { sys::igPushColumnClipRect(column_index) }
    }

    /// Push columns background for drawing.
    #[doc(alias = "PushColumnsBackground")]
    pub fn push_columns_background(&self) {
        assert_current_columns("Ui::push_columns_background()");
        unsafe { sys::igPushColumnsBackground() }
    }

    /// Pop columns background.
    #[doc(alias = "PopColumnsBackground")]
    pub fn pop_columns_background(&self) {
        assert_current_columns("Ui::pop_columns_background()");
        unsafe { sys::igPopColumnsBackground() }
    }

    /// Get columns ID for the given string ID and count.
    #[doc(alias = "GetColumnsID")]
    pub fn get_columns_id(&self, str_id: impl AsRef<str>, count: usize) -> Id {
        let count = columns_count_to_i32(count, "Ui::get_columns_id()");
        unsafe { Id::from(sys::igGetColumnsID(self.scratch_txt(str_id), count)) }
    }

    // ============================================================================
    // Column state utilities
    // ============================================================================

    /// Check if any column in the current legacy columns set is being resized.
    ///
    /// Returns `false` when the current window is not inside a legacy columns set.
    pub fn is_any_column_resizing(&self) -> bool {
        unsafe {
            let window = sys::igGetCurrentWindowRead();
            if window.is_null() {
                return false;
            }

            let columns = (*window).DC.CurrentColumns;
            if columns.is_null() {
                return false;
            }

            (*columns).IsBeingResized
        }
    }

    /// Get the total width of all columns.
    pub fn get_columns_total_width(&self) -> f32 {
        if current_columns().is_null() {
            return self.current_column_width();
        }

        let count = self.column_count();

        let mut total_width = 0.0;
        for i in 0..count {
            total_width += self.column_width(i);
        }
        total_width
    }

    /// Set all columns to equal width.
    pub fn set_columns_equal_width(&self) {
        let count = self.column_count();
        if count <= 1 {
            return;
        }

        let total_width = self.get_columns_total_width();
        let equal_width = total_width / count as f32;

        for i in 0..count {
            self.set_column_width(i, equal_width);
        }
    }

    /// Get column width as a percentage of total width.
    pub fn get_column_width_percentage(&self, column: impl Into<OldColumnRef>) -> f32 {
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return 0.0;
        }

        let column_width = self.column_width(column);
        (column_width / total_width) * 100.0
    }

    /// Set column width as a percentage of total width.
    pub fn set_column_width_percentage(&self, column: impl Into<OldColumnIndex>, percentage: f32) {
        assert_non_negative_f32(
            "Ui::set_column_width_percentage()",
            "percentage",
            percentage,
        );
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return;
        }

        let new_width = (total_width * percentage) / 100.0;
        self.set_column_width(column, new_width);
    }
}

/// Token representing an active columns layout.
#[must_use]
pub struct ColumnsToken<'ui> {
    ui: &'ui Ui,
}

impl Drop for ColumnsToken<'_> {
    fn drop(&mut self) {
        self.ui.end_columns();
    }
}

#[cfg(test)]
mod tests {
    use super::{OldColumnFlags, OldColumnIndex, OldColumnOffsetRef, OldColumnRef};

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn is_any_column_resizing_reads_current_columns_state() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_resize_test").build(|| {
            assert!(!ui.is_any_column_resizing());

            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            assert!(!window.is_null());

            let columns = unsafe { (*window).DC.CurrentColumns };
            assert!(!columns.is_null());
            assert!(!ui.is_any_column_resizing());

            unsafe {
                (*columns).IsBeingResized = true;
            }

            assert!(ui.is_any_column_resizing());
        });
    }

    #[test]
    fn columns_reject_invalid_counts_and_nested_layouts() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_invalid_counts").build(|| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.columns(0, "bad_columns", true);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _columns = ui.begin_columns_token("bad_columns", 0, OldColumnFlags::NONE);
                }))
                .is_err()
            );

            let _columns = ui.begin_columns_token("outer_columns", 2, OldColumnFlags::NONE);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _nested = ui.begin_columns_token("nested_columns", 2, OldColumnFlags::NONE);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn columns_reject_out_of_range_indices_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_index_bounds").build(|| {
            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.column_width(2);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width(2, 10.0);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.column_offset(3);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.push_column_clip_rect(2);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn columns_reject_invalid_flags_and_numeric_inputs_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_numeric_bounds").build(|| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _columns = ui.begin_columns_token(
                        "bad_flags",
                        2,
                        OldColumnFlags::from_bits_retain(1 << 16),
                    );
                }))
                .is_err()
            );

            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);
            assert_eq!(ui.column_count(), 2);
            assert_eq!(ui.current_column_index(), OldColumnIndex::ZERO);

            let _ = ui.current_column_width();
            let _ = ui.column_width(OldColumnRef::Current);
            let _ = ui.column_width(OldColumnIndex::new(1));
            let _ = ui.current_column_offset();
            let _ = ui.column_offset(OldColumnOffsetRef::Current);
            let _ = ui.column_offset(OldColumnOffsetRef::Trailing);

            ui.set_current_column_width(32.0);
            ui.set_current_column_offset(0.0);
            ui.set_column_width(OldColumnIndex::new(1), 16.0);
            ui.set_column_offset(OldColumnIndex::new(1), 8.0);
            ui.set_column_offset(OldColumnOffsetRef::Trailing, 96.0);
            ui.set_column_width_percentage(OldColumnIndex::new(1), 25.0);

            ui.set_current_column_width(0.0);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width(1, f32::NAN);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_current_column_offset(-1.0);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_offset(1, f32::INFINITY);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width_percentage(1, -1.0);
                }))
                .is_err()
            );
        });
    }
}
