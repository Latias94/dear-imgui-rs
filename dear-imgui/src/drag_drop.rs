#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    // We intentionally keep explicit casts for FFI clarity; avoid auto-fix churn.
    clippy::unnecessary_cast
)]
//! Drag and Drop functionality for Dear ImGui
//!
//! This module provides a complete drag and drop system that allows users to transfer
//! data between UI elements. The system consists of drag sources and drop targets,
//! with type-safe payload management.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a drag source
//! ui.button("Drag me!");
//! if let Some(source) = ui.drag_drop_source_config("MY_DATA").begin() {
//!     ui.text("Dragging...");
//!     source.end();
//! }
//!
//! // Create a drop target
//! ui.button("Drop here!");
//! if let Some(target) = ui.drag_drop_target() {
//!     if target
//!         .accept_payload_empty("MY_DATA", DragDropTargetFlags::NONE)
//!         .is_some()
//!     {
//!         println!("Data dropped!");
//!     }
//!     target.pop();
//! }
//! ```

use std::{any, ffi};

use crate::{Ui, sys};

/// Condition for updating a drag and drop payload.
///
/// Dear ImGui only accepts `Always` and `Once` for `SetDragDropPayload`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
#[allow(clippy::unnecessary_cast)]
pub enum DragDropPayloadCond {
    /// Update the payload every frame while dragging.
    Always = sys::ImGuiCond_Always as i32,
    /// Update the payload once when the drag starts.
    Once = sys::ImGuiCond_Once as i32,
}

bitflags::bitflags! {
    /// Flags for drag and drop sources.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DragDropSourceFlags: u32 {
        /// No flags
        const NONE = 0;

        /// Disable preview tooltip during drag
        const NO_PREVIEW_TOOLTIP = sys::ImGuiDragDropFlags_SourceNoPreviewTooltip as u32;
        /// Don't disable hover during drag
        const NO_DISABLE_HOVER = sys::ImGuiDragDropFlags_SourceNoDisableHover as u32;
        /// Don't open tree nodes/headers when hovering during drag
        const NO_HOLD_TO_OPEN_OTHERS = sys::ImGuiDragDropFlags_SourceNoHoldToOpenOthers as u32;
        /// Allow items without unique ID to be drag sources
        const ALLOW_NULL_ID = sys::ImGuiDragDropFlags_SourceAllowNullID as u32;
        /// External drag source (from outside ImGui)
        const EXTERN = sys::ImGuiDragDropFlags_SourceExtern as u32;
        /// Automatically expire payload if source stops being submitted
        const PAYLOAD_AUTO_EXPIRE = sys::ImGuiDragDropFlags_PayloadAutoExpire as u32;
        /// Hint that the payload may not be copied outside the current Dear ImGui context
        const PAYLOAD_NO_CROSS_CONTEXT = sys::ImGuiDragDropFlags_PayloadNoCrossContext as u32;
        /// Hint that the payload may not be copied outside the current process
        const PAYLOAD_NO_CROSS_PROCESS = sys::ImGuiDragDropFlags_PayloadNoCrossProcess as u32;
    }
}

impl Default for DragDropSourceFlags {
    fn default() -> Self {
        Self::NONE
    }
}

bitflags::bitflags! {
    /// Flags for drag and drop targets.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DragDropTargetFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Accept payload before mouse button is released
        const BEFORE_DELIVERY = sys::ImGuiDragDropFlags_AcceptBeforeDelivery as u32;
        /// Don't draw default highlight rectangle when hovering
        const NO_DRAW_DEFAULT_RECT = sys::ImGuiDragDropFlags_AcceptNoDrawDefaultRect as u32;
        /// Don't show preview tooltip from source
        const NO_PREVIEW_TOOLTIP = sys::ImGuiDragDropFlags_AcceptNoPreviewTooltip as u32;
        /// Render accepting target as hovered (e.g. allow Button() as drop target)
        const DRAW_AS_HOVERED = sys::ImGuiDragDropFlags_AcceptDrawAsHovered as u32;
        /// Convenience flag for peeking (BEFORE_DELIVERY | NO_DRAW_DEFAULT_RECT)
        const PEEK_ONLY = sys::ImGuiDragDropFlags_AcceptPeekOnly as u32;
    }
}

impl Default for DragDropTargetFlags {
    fn default() -> Self {
        Self::NONE
    }
}

fn validate_drag_drop_source_flags(caller: &str, flags: DragDropSourceFlags) {
    let unsupported = flags.bits() & !DragDropSourceFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDragDropFlags source bits: 0x{unsupported:X}"
    );
}

fn validate_drag_drop_target_flags(caller: &str, flags: DragDropTargetFlags) {
    let unsupported = flags.bits() & !DragDropTargetFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDragDropFlags target bits: 0x{unsupported:X}"
    );
}

// Native widgets also use ordinary application names, e.g. the Dear ImGui demo. Only this
// tag identifies payloads with the Rust layout. The application name travels with the value.
const TYPED_PAYLOAD_TAG: &ffi::CStr = c"dear-imgui-rs/typed-v1";

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct TypedPayloadName([u8; MAX_PAYLOAD_TYPE_LEN]);

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
struct TypedPayload<T: Copy> {
    name: TypedPayloadName,
    type_id: any::TypeId,
    pub(super) data: T,
}

fn write_typed_payload<T: Copy + 'static>(
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
unsafe fn decode_typed_payload<T: 'static + Copy>(
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

/// Drag drop target for accepting payloads
///
/// This struct is created by [`Ui::drag_drop_target`] and provides
/// methods for accepting different types of payloads.
///
/// The target must finish in its originating window `Begin` scope before another source or
/// target can begin.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndDragDropTarget")]
pub struct DragDropTarget<'ui>(
    pub(super) &'ui Ui,
    pub(super) crate::scope::NativeScopeToken<'ui>,
);

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
    /// Returns payload info if accepted, `None` otherwise, including after the target's
    /// enclosing native window scope has closed it.
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
    /// Returns `None` for a different application name or a native payload. For a matching
    /// name, returns `Ok(payload)` if the Rust type matches or `Err(PayloadIsWrongType)` otherwise.
    /// The application name must contain 1 to 32 bytes. Names starting with `_` are reserved
    /// for native payloads.
    ///
    /// # Arguments
    /// * `name` - Payload type name (must match source name)
    /// * `flags` - Accept flags
    ///
    /// Returns `Some(Result<payload, error>)` if payload exists, `None` otherwise, including
    /// after the enclosing native window scope has ended this target.
    #[doc(alias = "AcceptDragDropPayload")]
    pub fn accept_payload<T: 'static + Copy, Name: AsRef<str>>(
        &self,
        name: Name,
        flags: DragDropTargetFlags,
    ) -> Option<Result<DragDropPayloadPod<T>, PayloadIsWrongType>> {
        let name = TypedPayloadName::new(name.as_ref());
        validate_drag_drop_target_flags("DragDropTarget::accept_payload()", flags);
        self.0.run_with_bound_context(|| unsafe {
            // AcceptDragDropPayload records the target even during preview. Check the logical
            // name before asking ImGui to accept this drag.
            if self.1.is_finished() || !name.matches(sys::igGetDragDropPayload()) {
                return None;
            }
            let payload =
                sys::igAcceptDragDropPayload(TYPED_PAYLOAD_TAG.as_ptr(), flags.bits() as i32);
            payload
                .as_ref()
                .map(|payload| decode_typed_payload(DragDropPayload::from_raw(*payload)))
        })
    }

    /// Accept raw payload data (unsafe)
    ///
    /// # Safety
    /// Read the returned pointer only while this exact payload is active and use the layout
    /// associated with its native name. Another submission can replace the bytes, and ending
    /// the target after delivery releases them, even within the same frame.
    ///
    /// # Arguments
    /// * `name` - Payload type name
    /// * `flags` - Accept flags
    pub unsafe fn accept_payload_unchecked(
        &self,
        name: impl AsRef<str>,
        flags: DragDropTargetFlags,
    ) -> Option<DragDropPayload> {
        let name = name.as_ref();
        validate_payload_type_name(name, "DragDropTarget::accept_payload_unchecked()");
        validate_drag_drop_target_flags("DragDropTarget::accept_payload_unchecked()", flags);
        let inner = self.0.with_scratch_txt(name, |name| unsafe {
            if self.1.is_finished() {
                std::ptr::null()
            } else {
                sys::igAcceptDragDropPayload(name, flags.bits() as i32)
            }
        });

        if inner.is_null() {
            None
        } else {
            Some(DragDropPayload::from_raw(unsafe { *inner }))
        }
    }

    /// End the drag drop target
    ///
    /// This is called automatically when the token is dropped.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later drag-drop scope is active or this target is outside its
    /// originating window `Begin` scope.
    pub fn pop(self) {
        // Drop will handle cleanup
    }
}

impl Drop for DragDropTarget<'_> {
    fn drop(&mut self) {
        self.1.finish();
    }
}

const MAX_PAYLOAD_TYPE_LEN: usize = 32;

fn validate_payload_type_name(name: &str, caller: &str) {
    assert!(
        name.len() <= MAX_PAYLOAD_TYPE_LEN,
        "{caller} payload type name must be at most {MAX_PAYLOAD_TYPE_LEN} bytes"
    );
}

fn validate_payload_data(ptr: *const ffi::c_void, size: usize, caller: &str) {
    assert!(
        size <= i32::MAX as usize,
        "{caller} payload size exceeds Dear ImGui's i32 payload range"
    );
    assert!(
        (size == 0 && ptr.is_null()) || (size > 0 && !ptr.is_null()),
        "{caller} payload pointer and size must both be empty or both be non-empty"
    );
}

fn validate_payload_submission(name: &str, ptr: *const ffi::c_void, size: usize, caller: &str) {
    validate_payload_type_name(name, caller);
    validate_payload_data(ptr, size, caller);
}

impl Ui {
    pub(super) fn drag_drop_scope_available(&self) -> bool {
        // Dear ImGui has one pair of source/target flags for the entire context.
        unsafe { !(*self.ctx).DragDropWithinSource && !(*self.ctx).DragDropWithinTarget }
    }

    /// Creates a new drag drop source configuration
    ///
    /// # Arguments
    /// * `name` - Identifier for this drag source (must match target name)
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.button("Drag me!");
    /// if let Some(source) = ui.drag_drop_source_config("MY_DATA")
    ///     .flags(DragDropSourceFlags::NO_PREVIEW_TOOLTIP)
    ///     .begin() {
    ///     ui.text("Custom drag tooltip");
    ///     source.end();
    /// }
    /// ```
    pub fn drag_drop_source_config<T: AsRef<str>>(&self, name: T) -> DragDropSource<'_, T> {
        DragDropSource {
            name,
            flags: DragDropSourceFlags::NONE,
            cond: DragDropPayloadCond::Always,
            ui: self,
        }
    }

    /// Creates a drag drop target for the last item
    ///
    /// Returns `Some(DragDropTarget)` if the last item can accept drops. While a source or
    /// target is open in this context, further attempts return `None`.
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.button("Drop target");
    /// if let Some(target) = ui.drag_drop_target() {
    ///     if target.accept_payload_empty("MY_DATA", DragDropTargetFlags::NONE).is_some() {
    ///         println!("Received drop!");
    ///     }
    ///     target.pop();
    /// }
    /// ```
    #[doc(alias = "BeginDragDropTarget")]
    pub fn drag_drop_target(&self) -> Option<DragDropTarget<'_>> {
        let should_begin = self.run_with_bound_context(|| {
            self.drag_drop_scope_available() && unsafe { sys::igBeginDragDropTarget() }
        });
        if should_begin {
            Some(DragDropTarget(
                self,
                self.begin_native_scope(
                    crate::scope::NativeScopePop::EndDragDropTarget,
                    "DragDropTarget",
                ),
            ))
        } else {
            None
        }
    }

    /// Returns the current drag and drop payload, if any.
    ///
    /// This is a convenience wrapper over `ImGui::GetDragDropPayload`.
    ///
    /// The metadata is copied. Dear ImGui owns the data pointer, which can be invalidated
    /// by another submission or by ending the target after delivery in the same frame.
    #[doc(alias = "GetDragDropPayload")]
    pub fn drag_drop_payload(&self) -> Option<DragDropPayload> {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igGetDragDropPayload();
            if ptr.is_null() {
                return None;
            }
            Some(DragDropPayload::from_raw(*ptr))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi, mem::MaybeUninit};

    fn payload_bytes(value: u8) -> Vec<MaybeUninit<u8>> {
        let mut payload = MaybeUninit::uninit();
        write_typed_payload(&mut payload, TypedPayloadName::new("item"), value);
        let size = std::mem::size_of_val(&payload);
        let mut out = vec![MaybeUninit::uninit(); size];
        unsafe {
            std::ptr::copy_nonoverlapping(payload.as_ptr().cast(), out.as_mut_ptr(), size);
        }
        out
    }

    #[test]
    fn typed_payload_initializes_its_trailing_padding() {
        let bytes = payload_bytes(7);
        let end_of_data = std::mem::offset_of!(TypedPayload<u8>, data) + 1;
        assert!(
            bytes[end_of_data..]
                .iter()
                .all(|byte| unsafe { byte.assume_init() == 0 })
        );
    }

    #[test]
    fn typed_payload_can_be_read_unaligned() {
        let bytes = payload_bytes(7u8);
        let mut buf = vec![MaybeUninit::uninit(); 1 + bytes.len()];
        buf[1..].copy_from_slice(&bytes);
        let payload = DragDropPayload {
            data: buf[1..].as_ptr().cast(),
            size: bytes.len(),
            preview: true,
            delivery: false,
        };
        let decoded = unsafe { decode_typed_payload::<u8>(payload) }.unwrap();
        assert_eq!(decoded.data, 7u8);
        assert!(decoded.preview);
        assert!(!decoded.delivery);
    }

    #[test]
    fn payload_submission_rejects_imgui_assert_conditions_before_ffi() {
        let byte = 1u8;
        let ptr = std::ptr::from_ref(&byte).cast::<ffi::c_void>();
        let long_name = "x".repeat(MAX_PAYLOAD_TYPE_LEN + 1);

        assert!(
            std::panic::catch_unwind(|| {
                validate_payload_submission(
                    &long_name,
                    ptr,
                    1,
                    "payload_submission_rejects_imgui_assert_conditions_before_ffi",
                );
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                validate_payload_submission(
                    "payload",
                    std::ptr::null(),
                    1,
                    "payload_submission_rejects_imgui_assert_conditions_before_ffi",
                );
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                validate_payload_submission(
                    "payload",
                    ptr,
                    0,
                    "payload_submission_rejects_imgui_assert_conditions_before_ffi",
                );
            })
            .is_err()
        );
    }

    #[test]
    fn drag_drop_flag_domain_validation_rejects_wrong_domain_bits() {
        validate_drag_drop_source_flags("test", DragDropSourceFlags::all());
        validate_drag_drop_target_flags("test", DragDropTargetFlags::all());

        let target_bit_in_source =
            DragDropSourceFlags::from_bits_retain(DragDropTargetFlags::BEFORE_DELIVERY.bits());
        assert!(
            std::panic::catch_unwind(|| {
                validate_drag_drop_source_flags("test", target_bit_in_source);
            })
            .is_err()
        );

        let source_bit_in_target =
            DragDropTargetFlags::from_bits_retain(DragDropSourceFlags::NO_PREVIEW_TOOLTIP.bits());
        assert!(
            std::panic::catch_unwind(|| {
                validate_drag_drop_target_flags("test", source_bit_in_target);
            })
            .is_err()
        );

        let known_bits = DragDropSourceFlags::all().bits() | DragDropTargetFlags::all().bits();
        let unknown_bit = (0..u32::BITS)
            .map(|bit| 1u32 << bit)
            .find(|bit| known_bits & bit == 0)
            .expect("Dear ImGui drag-drop flags should not consume every u32 bit");

        assert!(
            std::panic::catch_unwind(|| {
                validate_drag_drop_source_flags(
                    "test",
                    DragDropSourceFlags::from_bits_retain(unknown_bit),
                );
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                validate_drag_drop_target_flags(
                    "test",
                    DragDropTargetFlags::from_bits_retain(unknown_bit),
                );
            })
            .is_err()
        );
    }

    #[test]
    fn typed_payload_rejects_a_different_type_of_the_same_size() {
        let mut value = std::mem::MaybeUninit::uninit();
        write_typed_payload(&mut value, TypedPayloadName::new("item"), 42_u32);
        let payload = DragDropPayload {
            data: value.as_ptr().cast(),
            size: std::mem::size_of_val(&value),
            preview: false,
            delivery: true,
        };
        assert_eq!(payload.size, std::mem::size_of::<TypedPayload<i32>>());
        assert!(unsafe { decode_typed_payload::<i32>(payload) }.is_err());
    }

    #[test]
    #[should_panic(expected = "reserved for Dear ImGui")]
    fn typed_payload_names_exclude_native_layouts() {
        TypedPayloadName::new("_native");
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn typed_payload_requires_a_name() {
        TypedPayloadName::new("");
    }

    #[test]
    fn typed_accept_rejects_trailing_payload_bytes() {
        let mut buf = payload_bytes(7u8);
        buf.push(MaybeUninit::new(0));

        let payload = DragDropPayload {
            data: buf.as_ptr().cast::<ffi::c_void>(),
            size: buf.len(),
            preview: false,
            delivery: false,
        };

        assert_ne!(payload.size, std::mem::size_of::<TypedPayload<u8>>());
        assert!(unsafe { decode_typed_payload::<u8>(payload) }.is_err());
    }
}
