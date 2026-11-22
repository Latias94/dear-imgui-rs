use super::validation::{MAX_PAYLOAD_TYPE_LEN, validate_payload_type_name};
use crate::sys;
use std::{any, ffi};

// Native widgets also use ordinary application names, e.g. the Dear ImGui demo. Only this
// tag identifies payloads with the Rust layout. The application name travels with the value.
pub(super) const TYPED_PAYLOAD_TAG: &ffi::CStr = c"dear-imgui-rs/typed-v1";

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(super) struct TypedPayloadName([u8; MAX_PAYLOAD_TYPE_LEN]);

impl TypedPayloadName {
    pub(super) fn new(name: &str) -> Self {
        validate_payload_type_name(name, "typed drag-drop");
        assert!(
            !name.is_empty(),
            "typed drag-drop payload name must not be empty"
        );
        assert!(
            !name.starts_with('_'),
            "typed drag-drop payload names starting with '_' are reserved for Dear ImGui"
        );
        let mut bytes = [0; MAX_PAYLOAD_TYPE_LEN];
        for (dst, src) in bytes.iter_mut().zip(name.bytes()) {
            *dst = if src == 0 { b'?' } else { src };
        }
        Self(bytes)
    }

    /// `payload` must be null or the live payload of the bound context.
    pub(super) unsafe fn matches(self, payload: *const sys::ImGuiPayload) -> bool {
        if payload.is_null()
            || !unsafe {
                sys::ImGuiPayload_IsDataType(payload.cast_mut(), TYPED_PAYLOAD_TAG.as_ptr())
            }
        {
            return false;
        }
        let payload = unsafe { &*payload };
        !payload.Data.is_null()
            && payload.DataSize >= size_of::<Self>() as i32
            && unsafe { payload.Data.cast::<Self>().read_unaligned() } == self
    }
}

/// Wrapper for typed payloads with runtime type checking.
///
/// Important: payload memory is copied and stored by Dear ImGui in an unaligned byte buffer.
/// Never take `&TypedPayload<T>` from the raw pointer returned by `AcceptDragDropPayload()`.
/// Always copy out using `ptr::read_unaligned`.
#[repr(C)]
pub(super) struct TypedPayload<T: Copy> {
    name: TypedPayloadName,
    type_id: any::TypeId,
    pub(super) data: T,
}

pub(super) fn write_typed_payload<T: Copy + 'static>(
    out: &mut std::mem::MaybeUninit<TypedPayload<T>>,
    name: TypedPayloadName,
    data: T,
) {
    // Initialize wrapper padding in the allocation that C++ will copy. Moving a TypedPayload
    // by value could discard that padding. Any padding inside T remains opaque to the decoder.
    let ptr = out.as_mut_ptr();
    unsafe {
        ptr.write_bytes(0, 1);
        std::ptr::addr_of_mut!((*ptr).name).write(name);
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

/// The data must be a live Rust typed payload for the duration of the copy.
pub(super) unsafe fn decode_typed_payload<T: 'static + Copy>(
    payload: DragDropPayload,
) -> Result<DragDropPayloadPod<T>, PayloadIsWrongType> {
    if payload.data.is_null() || payload.size != std::mem::size_of::<TypedPayload<T>>() {
        return Err(PayloadIsWrongType);
    }

    // Every repr(C) typed payload starts with the same header. The body is a T only after
    // this check, even when a different producer type has the same size.
    let typed = payload.data.cast::<TypedPayload<T>>();
    let type_id = unsafe { (&raw const (*typed).type_id).read_unaligned() };
    if type_id != any::TypeId::of::<T>() {
        return Err(PayloadIsWrongType);
    }
    Ok(DragDropPayloadPod {
        data: unsafe { (&raw const (*typed).data).read_unaligned() },
        preview: payload.preview,
        delivery: payload.delivery,
    })
}
