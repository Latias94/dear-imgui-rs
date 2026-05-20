use crate::sys;

pub(super) fn current_columns() -> *mut sys::ImGuiOldColumns {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        if window.is_null() {
            std::ptr::null_mut()
        } else {
            (*window).DC.CurrentColumns
        }
    }
}

pub(super) fn assert_no_current_columns(caller: &str) {
    assert!(
        current_columns().is_null(),
        "{caller} cannot be called while another legacy columns layout is active"
    );
}

pub(super) fn assert_current_columns(caller: &str) -> *mut sys::ImGuiOldColumns {
    let columns = current_columns();
    assert!(
        !columns.is_null(),
        "{caller} must be called inside a legacy columns layout"
    );
    columns
}
