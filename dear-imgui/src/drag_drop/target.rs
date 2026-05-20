use super::flags::{DragDropTargetFlags, validate_drag_drop_target_flags};
use super::payload::{
    DragDropPayload, DragDropPayloadEmpty, DragDropPayloadPod, PayloadIsWrongType,
    decode_typed_payload,
};
use super::validation::validate_payload_type_name;
use crate::{Ui, sys};

/// Drag drop target for accepting payloads
///
/// This struct is created by [`Ui::drag_drop_target`] and provides
/// methods for accepting different types of payloads.
#[derive(Debug)]
pub struct DragDropTarget<'ui>(pub(super) &'ui Ui);

impl<'ui> DragDropTarget<'ui> {
    /// Accept an empty payload
    ///
    /// This is the safest option for drag and drop operations.
    /// Use this when you only need to know that a drop occurred,
    /// not transfer actual data.
    ///
    /// # Arguments
    /// * `name` - Payload type name (must match source name)
    /// * `flags` - Accept flags
    ///
    /// Returns payload info if accepted, `None` otherwise.
    pub fn accept_payload_empty(
        &self,
        name: impl AsRef<str>,
        flags: DragDropTargetFlags,
    ) -> Option<DragDropPayloadEmpty> {
        self.accept_payload(name, flags)?
            .ok()
            .map(|payload_pod: DragDropPayloadPod<()>| DragDropPayloadEmpty {
                preview: payload_pod.preview,
                delivery: payload_pod.delivery,
            })
    }

    /// Accept a typed payload
    ///
    /// Attempts to accept a payload with the specified type.
    /// Returns `Ok(payload)` if the type matches, `Err(PayloadIsWrongType)` if not.
    ///
    /// # Arguments
    /// * `name` - Payload type name (must match source name)
    /// * `flags` - Accept flags
    ///
    /// Returns `Some(Result<payload, error>)` if payload exists, `None` otherwise.
    pub fn accept_payload<T: 'static + Copy, Name: AsRef<str>>(
        &self,
        name: Name,
        flags: DragDropTargetFlags,
    ) -> Option<Result<DragDropPayloadPod<T>, PayloadIsWrongType>> {
        let output = unsafe { self.accept_payload_unchecked(name, flags) };

        output.map(decode_typed_payload)
    }

    /// Accept raw payload data (unsafe)
    ///
    /// # Safety
    /// The returned pointer and size are managed by ImGui and may become
    /// invalid at any time. The caller must not access the data after
    /// the drag operation completes.
    ///
    /// # Arguments
    /// * `name` - Payload type name
    /// * `flags` - Accept flags
    pub unsafe fn accept_payload_unchecked(
        &self,
        name: impl AsRef<str>,
        flags: DragDropTargetFlags,
    ) -> Option<DragDropPayload> {
        validate_payload_type_name(name.as_ref(), "DragDropTarget::accept_payload_unchecked()");
        validate_drag_drop_target_flags("DragDropTarget::accept_payload_unchecked()", flags);
        let inner =
            unsafe { sys::igAcceptDragDropPayload(self.0.scratch_txt(name), flags.bits() as i32) };

        if inner.is_null() {
            None
        } else {
            Some(DragDropPayload::from_raw(unsafe { *inner }))
        }
    }

    /// End the drag drop target
    ///
    /// This is called automatically when the token is dropped.
    pub fn pop(self) {
        // Drop will handle cleanup
    }
}

impl Drop for DragDropTarget<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndDragDropTarget();
        }
    }
}
