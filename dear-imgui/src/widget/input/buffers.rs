use crate::sys;
use std::ffi::c_int;

pub(super) fn make_string_input_buffer(buf: &String, capacity_hint: Option<usize>) -> Vec<u8> {
    let spare_hint = capacity_hint.unwrap_or(0);
    let desired = buf
        .capacity()
        .saturating_add(1)
        .max(buf.len().saturating_add(spare_hint).saturating_add(1))
        .max(1);
    assert!(
        desired <= i32::MAX as usize,
        "InputText buffer exceeds Dear ImGui's i32 callback range"
    );

    let mut buffer = Vec::with_capacity(desired);
    buffer.extend_from_slice(buf.as_bytes());
    buffer.push(0);
    buffer.resize(desired, 0);
    buffer
}

pub(super) unsafe fn resize_string_input_buffer(
    buffer: &mut Vec<u8>,
    requested_i32: i32,
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    if requested_i32 < 0 {
        return 0;
    }

    let requested = requested_i32 as usize;
    if requested > buffer.len() {
        buffer.resize(requested, 0);
        unsafe {
            (*data).Buf = buffer.as_mut_ptr() as *mut _;
            (*data).BufDirty = true;
        }
    }
    0
}

pub(super) fn finish_string_input_buffer(target: &mut String, mut buffer: Vec<u8>) {
    let len = buffer
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(buffer.len());
    buffer.truncate(len);

    let preserved_capacity = target.capacity().max(buffer.capacity());
    let mut text = match String::from_utf8(buffer) {
        Ok(text) => text,
        Err(err) => {
            let bytes = err.into_bytes();
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    if text.capacity() < preserved_capacity {
        text.reserve(preserved_capacity.saturating_sub(text.len()));
    }
    *target = text;
}
