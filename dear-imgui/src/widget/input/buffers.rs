use crate::sys;
use std::ffi::c_int;
use std::mem;

const MAX_INPUT_BUFFER_SIZE: usize = i32::MAX as usize - 1;

pub(super) fn validate_input_buffer_size(size: usize) {
    // InputTextEx takes an int and allocates its internal TextA with buf_size + 1.
    assert!(
        size <= MAX_INPUT_BUFFER_SIZE,
        "InputText buffer exceeds Dear ImGui's i32 size range"
    );
}

pub(super) struct StringInputBuffer<'a> {
    target: &'a mut String,
    pub(super) bytes: Vec<u8>,
}

impl<'a> StringInputBuffer<'a> {
    pub(super) fn new(target: &'a mut String, capacity_hint: Option<usize>) -> Self {
        let spare = capacity_hint.unwrap_or(0);
        let needed = target.len().saturating_add(spare).saturating_add(1);
        validate_input_buffer_size(needed);
        let size = target.capacity().min(MAX_INPUT_BUFFER_SIZE).max(needed);

        if spare == 0 {
            // A full String only needs a terminator to display its existing contents.
            target.reserve_exact(1);
        } else {
            // Amortize the requested spare bytes as the field grows across frames.
            target.reserve(spare + 1);
        }
        let mut bytes = mem::take(target).into_bytes();
        bytes.resize(size, 0);
        Self { target, bytes }
    }
}

impl Drop for StringInputBuffer<'_> {
    fn drop(&mut self) {
        finish_string_input_buffer(self.target, mem::take(&mut self.bytes));
    }
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
