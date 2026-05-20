use super::flags::{DragDropPayloadCond, DragDropSourceFlags, validate_drag_drop_source_flags};
use super::payload::{TypedPayload, make_typed_payload};
use super::validation::validate_payload_submission;
use crate::{Ui, sys};
use std::ffi;

/// Builder for creating drag drop sources
///
/// This struct is created by [`Ui::drag_drop_source_config`] and provides
/// a fluent interface for configuring drag sources.
#[derive(Debug)]
pub struct DragDropSource<'ui, T> {
    pub(super) name: T,
    pub(super) flags: DragDropSourceFlags,
    pub(super) cond: DragDropPayloadCond,
    pub(super) ui: &'ui Ui,
}

impl<'ui, T: AsRef<str>> DragDropSource<'ui, T> {
    /// Set flags for this drag source
    ///
    /// # Arguments
    /// * `flags` - Combination of source-related `DragDropSourceFlags`
    #[inline]
    pub fn flags(mut self, flags: DragDropSourceFlags) -> Self {
        validate_drag_drop_source_flags("DragDropSource::flags()", flags);
        self.flags = flags;
        self
    }

    /// Set condition for when to update the payload
    ///
    /// # Arguments
    /// * `cond` - When to update the payload data
    #[inline]
    pub fn condition(mut self, cond: DragDropPayloadCond) -> Self {
        self.cond = cond;
        self
    }

    /// Begin drag source with empty payload
    ///
    /// This is the safest option for simple drag and drop operations.
    /// Use shared state or other mechanisms to transfer actual data.
    ///
    /// Returns a tooltip token if dragging started, `None` otherwise.
    #[inline]
    pub fn begin(self) -> Option<DragDropSourceTooltip<'ui>> {
        self.begin_payload(())
    }

    /// Begin drag source with typed payload
    ///
    /// The payload data will be copied and managed by ImGui.
    /// The data must be `Copy + 'static` for safety.
    ///
    /// # Arguments
    /// * `payload` - Data to transfer (must be Copy + 'static)
    ///
    /// Returns a tooltip token if dragging started, `None` otherwise.
    #[inline]
    pub fn begin_payload<P: Copy + 'static>(
        self,
        payload: P,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        unsafe {
            let payload_size = std::mem::size_of::<TypedPayload<P>>();
            assert!(
                payload_size <= i32::MAX as usize,
                "DragDropSource::begin_payload() payload size exceeds Dear ImGui's i32 payload range"
            );

            let payload = make_typed_payload(payload);
            self.begin_payload_unchecked(&payload as *const _ as *const ffi::c_void, payload_size)
        }
    }

    /// Begin drag source with raw payload data (unsafe)
    ///
    /// # Safety
    /// The caller must ensure:
    /// - `ptr` points to valid data of `size` bytes
    /// - The data remains valid for the duration of the drag operation
    /// - The data layout matches what targets expect
    ///
    /// # Arguments
    /// * `ptr` - Pointer to payload data
    /// * `size` - Size of payload data in bytes
    pub unsafe fn begin_payload_unchecked(
        &self,
        ptr: *const ffi::c_void,
        size: usize,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        validate_payload_submission(
            self.name.as_ref(),
            ptr,
            size,
            "DragDropSource::begin_payload_unchecked()",
        );
        validate_drag_drop_source_flags("DragDropSource::begin_payload_unchecked()", self.flags);
        unsafe {
            let should_begin = sys::igBeginDragDropSource(self.flags.bits() as i32);

            if should_begin {
                sys::igSetDragDropPayload(
                    self.ui.scratch_txt(self.name.as_ref()),
                    ptr,
                    size,
                    self.cond as i32,
                );

                Some(DragDropSourceTooltip::new(self.ui))
            } else {
                None
            }
        }
    }
}

/// Token representing an active drag source tooltip
///
/// While this token exists, you can add UI elements that will be shown
/// as a tooltip during the drag operation.
#[derive(Debug)]
pub struct DragDropSourceTooltip<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> DragDropSourceTooltip<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// End the drag source tooltip manually
    ///
    /// This is called automatically when the token is dropped.
    pub fn end(self) {
        // Drop will handle cleanup
    }
}

impl Drop for DragDropSourceTooltip<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndDragDropSource();
        }
    }
}
