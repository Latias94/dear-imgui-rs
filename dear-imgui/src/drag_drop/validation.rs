use std::ffi;

pub(super) const MAX_PAYLOAD_TYPE_LEN: usize = 32;

pub(super) fn validate_payload_type_name(name: &str, caller: &str) {
    assert!(
        name.len() <= MAX_PAYLOAD_TYPE_LEN,
        "{caller} payload type name must be at most {MAX_PAYLOAD_TYPE_LEN} bytes"
    );
}

pub(super) fn validate_payload_data(ptr: *const ffi::c_void, size: usize, caller: &str) {
    assert!(
        size <= i32::MAX as usize,
        "{caller} payload size exceeds Dear ImGui's i32 payload range"
    );
    assert!(
        (size == 0 && ptr.is_null()) || (size > 0 && !ptr.is_null()),
        "{caller} payload pointer and size must both be empty or both be non-empty"
    );
}

pub(super) fn validate_payload_submission(
    name: &str,
    ptr: *const ffi::c_void,
    size: usize,
    caller: &str,
) {
    validate_payload_type_name(name, caller);
    validate_payload_data(ptr, size, caller);
}
