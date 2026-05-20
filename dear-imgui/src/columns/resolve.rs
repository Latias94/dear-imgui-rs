use crate::sys;

use super::index::{OldColumnIndex, OldColumnOffsetRef, OldColumnRef};
use super::state::{assert_current_columns, current_columns};

pub(super) fn assert_valid_column_in(
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

pub(super) fn resolve_column_query_ref(column: OldColumnRef, caller: &str) -> i32 {
    match column {
        OldColumnRef::Current if current_columns().is_null() => -1,
        _ => resolve_column_ref(column, caller),
    }
}

pub(super) fn resolve_column_offset_ref(offset: OldColumnOffsetRef, caller: &str) -> i32 {
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

pub(super) fn resolve_column_offset_query_ref(offset: OldColumnOffsetRef, caller: &str) -> i32 {
    match offset {
        OldColumnOffsetRef::Current if current_columns().is_null() => -1,
        _ => resolve_column_offset_ref(offset, caller),
    }
}
