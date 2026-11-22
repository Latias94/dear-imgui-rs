use crate::sys;
use std::{any, ffi};

/// Wrapper for typed payloads with runtime type checking.
///
/// Important: payload memory is copied and stored by Dear ImGui in an unaligned byte buffer.
/// Never take `&TypedPayload<T>` from the raw pointer returned by `AcceptDragDropPayload()`.
/// Always copy out using `ptr::read_unaligned`.
#[repr(C)]
#[derive(Copy, Clone)]
pub(super) struct TypedPayload<T: Copy> {
    pub(super) type_id: any::TypeId,
    pub(super) data: T,
}

pub(super) fn write_typed_payload<T: Copy + 'static>(
    out: &mut std::mem::MaybeUninit<TypedPayload<T>>,
    data: T,
) {
    // Initialize wrapper padding in the allocation that C++ will copy. Moving a TypedPayload
    // by value could discard that padding. Any padding inside T remains opaque to the decoder.
    let ptr = out.as_mut_ptr();
    unsafe {
        ptr.write_bytes(0, 1);
        std::ptr::addr_of_mut!((*ptr).type_id).write(any::TypeId::of::<T>());
        std::ptr::addr_of_mut!((*ptr).data).write(data);
    }
}

/// Empty payload (no data, just notification)
#[derive(Debug, Clone, Copy)]
pub struct DragDropPayloadEmpty {
    /// True when hovering over target
    pub preview: bool,
    /// True when payload should be delivered
    pub delivery: bool,
}

/// Typed payload with data
#[derive(Debug, Clone, Copy)]
pub struct DragDropPayloadPod<T> {
    /// The payload data
    pub data: T,
    /// True when hovering over target
    pub preview: bool,
    /// True when payload should be delivered
    pub delivery: bool,
}

/// Raw payload data
#[derive(Debug)]
pub struct DragDropPayload {
    /// Pointer to payload data (managed by ImGui)
    pub data: *const ffi::c_void,
    /// Size of payload data in bytes
    pub size: usize,
    /// True when hovering over target
    pub preview: bool,
    /// True when payload should be delivered
    pub delivery: bool,
}

impl DragDropPayload {
    pub(super) fn from_raw(inner: sys::ImGuiPayload) -> Self {
        let size = if inner.DataSize <= 0 || inner.Data.is_null() {
            0
        } else {
            inner.DataSize as usize
        };

        Self {
            data: inner.Data,
            size,
            preview: inner.Preview,
            delivery: inner.Delivery,
        }
    }
}

/// Error type for payload type mismatches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadIsWrongType;

impl std::fmt::Display for PayloadIsWrongType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "drag drop payload has wrong type")
    }
}

impl std::error::Error for PayloadIsWrongType {}

pub(super) fn decode_typed_payload<T: 'static + Copy>(
    payload: DragDropPayload,
) -> Result<DragDropPayloadPod<T>, PayloadIsWrongType> {
    if payload.data.is_null() || payload.size != std::mem::size_of::<TypedPayload<T>>() {
        return Err(PayloadIsWrongType);
    }

    // Every repr(C) typed payload starts with the same header. The body is a T only after
    // this check, even when a different producer type has the same size.
    let type_id = unsafe { payload.data.cast::<any::TypeId>().read_unaligned() };
    if type_id != any::TypeId::of::<T>() {
        return Err(PayloadIsWrongType);
    }
    let typed = payload.data.cast::<TypedPayload<T>>();
    Ok(DragDropPayloadPod {
        data: unsafe { (&raw const (*typed).data).read_unaligned() },
        preview: payload.preview,
        delivery: payload.delivery,
    })
}
