use super::flags::{DragDropPayloadCond, DragDropSourceFlags, validate_drag_drop_source_flags};
use super::payload::{TypedPayload, write_typed_payload};
use super::validation::{validate_payload_submission, validate_typed_payload_name};
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
    /// The data must be `Copy + 'static`. Names starting with `_` are reserved for native payloads.
    ///
    /// # Arguments
    /// * `payload` - Data to transfer (must be Copy + 'static)
    ///
    /// Returns a tooltip token if dragging started, `None` otherwise.
    #[inline]
    #[doc(alias = "BeginDragDropSource", alias = "SetDragDropPayload")]
    pub fn begin_payload<P: Copy + 'static>(
        self,
        payload: P,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        let name = self.name.as_ref();
        validate_typed_payload_name(name);
        let payload_size = std::mem::size_of::<TypedPayload<P>>();
        assert!(
            payload_size <= i32::MAX as usize,
            "DragDropSource::begin_payload() payload size exceeds Dear ImGui's i32 payload range"
        );
        let mut storage = std::mem::MaybeUninit::uninit();
        write_typed_payload(&mut storage, payload);
        unsafe { self.submit_payload(name, storage.as_ptr().cast(), payload_size) }
    }

    /// Begin drag source with raw payload data (unsafe)
    ///
    /// # Safety
    /// The caller must ensure:
    /// - `ptr` points to valid data of `size` bytes
    /// - The data remains readable until this call returns; Dear ImGui copies it on submission
    /// - The bytes match every target's layout for this name. Use a distinct name for raw
    ///   data when typed Rust targets expect a type header and value.
    ///
    /// # Arguments
    /// * `ptr` - Pointer to payload data
    /// * `size` - Size of payload data in bytes
    pub unsafe fn begin_payload_unchecked(
        &self,
        ptr: *const ffi::c_void,
        size: usize,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        unsafe { self.submit_payload(self.name.as_ref(), ptr, size) }
    }

    unsafe fn submit_payload(
        &self,
        name: &str,
        ptr: *const ffi::c_void,
        size: usize,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        validate_payload_submission(name, ptr, size, "DragDropSource::begin_payload_unchecked()");
        validate_drag_drop_source_flags("DragDropSource::begin_payload_unchecked()", self.flags);
        self.ui.run_with_bound_context(|| unsafe {
            let should_begin = sys::igBeginDragDropSource(self.flags.bits() as i32);

            if should_begin {
                sys::igSetDragDropPayload(self.ui.scratch_txt(name), ptr, size, self.cond as i32);

                Some(DragDropSourceTooltip::new(self.ui))
            } else {
                None
            }
        })
    }
}

/// Token representing an active drag source tooltip
///
/// While this token exists, you can add UI elements that will be shown
/// as a tooltip during the drag operation.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::drag_drop_source_config`] closure composition when possible.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndDragDropSource")]
pub struct DragDropSourceTooltip<'ui> {
    scope: crate::scope::NativeScopeToken<'ui>,
}

impl<'ui> DragDropSourceTooltip<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(
                crate::scope::NativeScopePop::EndDragDropSource,
                "DragDropSourceTooltip",
            ),
        }
    }

    /// End the drag source tooltip manually
    ///
    /// This is called automatically when the token is dropped.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // Drop will handle cleanup
    }
}

impl Drop for DragDropSourceTooltip<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
