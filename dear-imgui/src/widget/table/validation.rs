use crate::widget::table::{TABLE_MAX_COLUMNS, TableColumnIndex, TableColumnRef, TableFlags};
use crate::{Id, sys};

pub(crate) fn table_column_count_to_i32(column_count: usize) -> i32 {
    assert!(
        column_count > 0,
        "table column_count must be greater than zero"
    );
    assert!(
        column_count < TABLE_MAX_COLUMNS,
        "table column_count must be less than {TABLE_MAX_COLUMNS}"
    );
    i32::try_from(column_count).expect("table column_count exceeded ImGui's i32 range")
}

pub(crate) fn table_freeze_count_to_i32(caller: &str, name: &str, count: usize, max: usize) -> i32 {
    assert!(count < max, "{caller} {name} must be less than {max}");
    i32::try_from(count).expect("table freeze count exceeded ImGui's i32 range")
}

fn current_table() -> *mut sys::ImGuiTable {
    unsafe { sys::igGetCurrentTable() }
}

pub(crate) fn current_table_if_any() -> Option<*mut sys::ImGuiTable> {
    let table = current_table();
    (!table.is_null()).then_some(table)
}

pub(crate) fn assert_current_table(caller: &str) -> *mut sys::ImGuiTable {
    let table = current_table();
    assert!(
        !table.is_null(),
        "{caller} must be called inside a BeginTable/EndTable scope"
    );
    table
}

pub(crate) fn assert_valid_table_column_raw_in(
    table: *mut sys::ImGuiTable,
    column_n: i32,
    caller: &str,
) {
    let column_count = unsafe { (*table).ColumnsCount };
    assert!(
        (0..column_count).contains(&column_n),
        "{caller} column index {column_n} is outside the current table column range 0..{column_count}"
    );
}

pub(crate) fn assert_valid_table_column_in(
    table: *mut sys::ImGuiTable,
    column: TableColumnIndex,
    caller: &str,
) -> i32 {
    let column_n = column.into_i32(caller);
    assert_valid_table_column_raw_in(table, column_n, caller);
    column_n
}

pub(crate) fn assert_valid_table_column(column: TableColumnIndex, caller: &str) -> i32 {
    let table = assert_current_table(caller);
    assert_valid_table_column_in(table, column, caller)
}

pub(crate) fn assert_non_negative_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(crate) fn resolve_table_column(column: TableColumnRef, caller: &str) -> i32 {
    let table = assert_current_table(caller);
    let column_n = match column {
        TableColumnRef::Current => unsafe { (*table).CurrentColumn },
        TableColumnRef::Index(index) => index.into_i32(caller),
    };
    assert_valid_table_column_raw_in(table, column_n, caller);
    column_n
}

pub(crate) fn assert_current_table_has_flags(flags: TableFlags, caller: &str) {
    let table = assert_current_table(caller);
    let table_flags = TableFlags::from_bits_retain(unsafe { (*table).Flags });
    assert!(
        table_flags.contains(flags),
        "{caller} requires the current table to have {flags:?}"
    );
}

pub(crate) fn assert_table_setup_phase(caller: &str) {
    let table = assert_current_table(caller);
    assert!(
        !unsafe { (*table).IsLayoutLocked },
        "{caller} must be called before the first table row or column"
    );
}

pub(crate) fn assert_table_column_width_phase(caller: &str) {
    let table = assert_current_table(caller);
    assert!(
        !unsafe { (*table).IsLayoutLocked },
        "{caller} must be called before the table layout is locked"
    );
    assert!(
        unsafe { (*table).MinColumnWidth > 0.0 },
        "{caller} requires Dear ImGui table layout metrics to be initialized"
    );
}

pub(crate) fn assert_current_table_cell(caller: &str) {
    let table = assert_current_table(caller);
    let (current_column, column_count) = unsafe { ((*table).CurrentColumn, (*table).ColumnsCount) };
    assert!(
        (0..column_count).contains(&current_column),
        "{caller} must be called while a table cell is current"
    );
}

pub(crate) fn assert_current_table_row(caller: &str) {
    let table = assert_current_table(caller);
    assert!(
        unsafe { (*table).CurrentRow } >= 0,
        "{caller} must be called while a table row is current"
    );
}

pub(crate) fn assert_explicit_user_id(id: Id, caller: &str) -> Id {
    assert!(
        id.raw() != 0,
        "{caller} user_id must be non-zero; use None for automatic user id"
    );
    id
}

pub(crate) fn optional_user_id_raw(user_id: Option<Id>, caller: &str) -> sys::ImGuiID {
    user_id.map_or(0, |id| assert_explicit_user_id(id, caller).raw())
}

pub(crate) fn optional_user_id_from_raw(user_id: sys::ImGuiID) -> Option<Id> {
    (user_id != 0).then(|| Id::from(user_id))
}
