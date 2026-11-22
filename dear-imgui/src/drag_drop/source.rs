use super::flags::{DragDropPayloadCond, DragDropSourceFlags, validate_drag_drop_source_flags};
use super::payload::{TYPED_PAYLOAD_TAG, TypedPayload, TypedPayloadName, write_typed_payload};
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
    /// Returns a tooltip token if dragging started. While a source or target is open in this
    /// context, or when the item has no ID and `ALLOW_NULL_ID` is unset, returns `None`.
    #[inline]
    pub fn begin(self) -> Option<DragDropSourceTooltip<'ui>> {
        self.begin_payload(())
    }

    /// Begin drag source with typed payload
    ///
    /// ImGui copies the data for Rust typed targets in this context. The application name can
    /// contain 1 to 32 bytes. Names starting with `_` are reserved for native payloads.
    /// Use [`Self::begin_payload_unchecked`] to send a native payload with its own layout.
    ///
    /// # Arguments
    /// * `payload` - Data to transfer (must be Copy + 'static)
    ///
    /// Returns a tooltip token if dragging started, `None` otherwise.
    #[inline]
    #[doc(alias = "BeginDragDropSource", alias = "SetDragDropPayload")]
    pub fn begin_payload<P: Copy + 'static>(
        mut self,
        payload: P,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        let name = TypedPayloadName::new(self.name.as_ref());
        let payload_size = std::mem::size_of::<TypedPayload<P>>();
        assert!(
            payload_size <= i32::MAX as usize,
            "DragDropSource::begin_payload() payload size exceeds Dear ImGui's i32 payload range"
        );
        let mut storage = std::mem::MaybeUninit::uninit();
        write_typed_payload(&mut storage, name, payload);
        self.flags |= DragDropSourceFlags::PAYLOAD_NO_CROSS_CONTEXT
            | DragDropSourceFlags::PAYLOAD_NO_CROSS_PROCESS;
        unsafe {
            self.submit_payload(
                TYPED_PAYLOAD_TAG.as_ptr(),
                storage.as_ptr().cast(),
                payload_size,
            )
        }
    }

    /// Begin drag source with raw payload data (unsafe)
    ///
    /// # Safety
    /// The caller must ensure:
    /// - `ptr` points to valid data of `size` bytes
    /// - The data remains readable until this call returns; Dear ImGui copies it on submission
    /// - The bytes match every native target's layout for this name and do not overlap
    ///   ImGui's destination payload storage. The Rust typed transport tag is reserved.
    ///
    /// # Arguments
    /// * `ptr` - Pointer to payload data
    /// * `size` - Size of payload data in bytes
    pub unsafe fn begin_payload_unchecked(
        &self,
        ptr: *const ffi::c_void,
        size: usize,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        let name = self.name.as_ref();
        validate_payload_submission(name, ptr, size, "DragDropSource::begin_payload_unchecked()");
        assert_ne!(
            name.as_bytes(),
            TYPED_PAYLOAD_TAG.to_bytes(),
            "reserved Rust typed payload tag"
        );
        crate::string::with_scratch_txt(name, |name| unsafe {
            self.submit_payload(name, ptr, size)
        })
    }

    unsafe fn submit_payload(
        &self,
        name: *const ffi::c_char,
        ptr: *const ffi::c_void,
        size: usize,
    ) -> Option<DragDropSourceTooltip<'ui>> {
        validate_drag_drop_source_flags("DragDropSource::begin_payload_unchecked()", self.flags);
        self.ui.run_with_bound_context(|| unsafe {
            if !self.ui.drag_drop_scope_available() {
                return None;
            }
            if !self
                .flags
                .intersects(DragDropSourceFlags::EXTERN | DragDropSourceFlags::ALLOW_NULL_ID)
                && (*self.ui.ctx).LastItemData.ID == 0
            {
                return None;
            }
            let should_begin = sys::igBeginDragDropSource(self.flags.bits() as i32);

            if should_begin {
                sys::igSetDragDropPayload(name, ptr, size, self.cond as i32);

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
