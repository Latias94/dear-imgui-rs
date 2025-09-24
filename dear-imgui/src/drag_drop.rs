//! Drag and Drop functionality for Dear ImGui
//!
//! This module provides a complete drag and drop system that allows users to transfer
//! data between UI elements. The system consists of drag sources and drop targets,
//! with type-safe payload management.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui::*;
//! # let mut ctx = Context::create_or_panic();
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
//!     if target.accept_payload_empty("MY_DATA", DragDropFlags::empty()).is_some() {
//!         println!("Data dropped!");
//!     }
//!     target.pop();
//! }
//! ```

use crate::{Condition, Ui, sys};
use std::{any, ffi};

bitflags::bitflags! {
    /// Flags for drag and drop operations
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DragDropFlags: u32 {
        /// No flags
        const NONE = 0;

        // Source flags
        /// Disable preview tooltip during drag
        const SOURCE_NO_PREVIEW_TOOLTIP = sys::ImGuiDragDropFlags_SourceNoPreviewTooltip as u32;
        /// Don't disable hover during drag
        const SOURCE_NO_DISABLE_HOVER = sys::ImGuiDragDropFlags_SourceNoDisableHover as u32;
        /// Don't open tree nodes/headers when hovering during drag
        const SOURCE_NO_HOLD_TO_OPEN_OTHERS = sys::ImGuiDragDropFlags_SourceNoHoldToOpenOthers as u32;
        /// Allow items without unique ID to be drag sources
        const SOURCE_ALLOW_NULL_ID = sys::ImGuiDragDropFlags_SourceAllowNullID as u32;
        /// External drag source (from outside ImGui)
        const SOURCE_EXTERN = sys::ImGuiDragDropFlags_SourceExtern as u32;
        /// Automatically expire payload if source stops being submitted
        const PAYLOAD_AUTO_EXPIRE = sys::ImGuiDragDropFlags_PayloadAutoExpire as u32;

        // Target flags
        /// Accept payload before mouse button is released
        const ACCEPT_BEFORE_DELIVERY = sys::ImGuiDragDropFlags_AcceptBeforeDelivery as u32;
        /// Don't draw default highlight rectangle when hovering
        const ACCEPT_NO_DRAW_DEFAULT_RECT = sys::ImGuiDragDropFlags_AcceptNoDrawDefaultRect as u32;
        /// Don't show preview tooltip from source
        const ACCEPT_NO_PREVIEW_TOOLTIP = sys::ImGuiDragDropFlags_AcceptNoPreviewTooltip as u32;
        /// Convenience flag for peeking (ACCEPT_BEFORE_DELIVERY | ACCEPT_NO_DRAW_DEFAULT_RECT)
        const ACCEPT_PEEK_ONLY = sys::ImGuiDragDropFlags_AcceptPeekOnly as u32;
    }
}

impl Ui {
    /// Creates a new drag drop source configuration
    ///
    /// # Arguments
    /// * `name` - Identifier for this drag source (must match target name)
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// ui.button("Drag me!");
    /// if let Some(source) = ui.drag_drop_source_config("MY_DATA")
    ///     .flags(DragDropFlags::SOURCE_NO_PREVIEW_TOOLTIP)
    ///     .begin() {
    ///     ui.text("Custom drag tooltip");
    ///     source.end();
    /// }
    /// ```
    pub fn drag_drop_source_config<T: AsRef<str>>(&self, name: T) -> DragDropSource<'_, T> {
        DragDropSource {
            name,
            flags: DragDropFlags::NONE,
            cond: Condition::Always,
            ui: self,
        }
    }

    /// Creates a drag drop target for the last item
    ///
    /// Returns `Some(DragDropTarget)` if the last item can accept drops,
    /// `None` otherwise.
    ///
    /// # Example
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create_or_panic();
    /// # let ui = ctx.frame();
    /// ui.button("Drop target");
    /// if let Some(target) = ui.drag_drop_target() {
    ///     if target.accept_payload_empty("MY_DATA", DragDropFlags::NONE).is_some() {
    ///         println!("Received drop!");
    ///     }
    ///     target.pop();
    /// }
    /// ```
    #[doc(alias = "BeginDragDropTarget")]
    pub fn drag_drop_target(&self) -> Option<DragDropTarget<'_>> {
        let should_begin = unsafe { sys::igBeginDragDropTarget() };
        if should_begin {
            Some(DragDropTarget(self))
        } else {
            None
        }
    }
}

/// Builder for creating drag drop sources
///
/// This struct is created by [`Ui::drag_drop_source_config`] and provides
/// a fluent interface for configuring drag sources.
#[derive(Debug)]
pub struct DragDropSource<'ui, T> {
    name: T,
    flags: DragDropFlags,
    cond: Condition,
    ui: &'ui Ui,
}

impl<'ui, T: AsRef<str>> DragDropSource<'ui, T> {
    /// Set flags for this drag source
    ///
    /// # Arguments
    /// * `flags` - Combination of source-related `DragDropFlags`
    #[inline]
    pub fn flags(mut self, flags: DragDropFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set condition for when to update the payload
    ///
    /// # Arguments
    /// * `cond` - When to update the payload data
    #[inline]
    pub fn condition(mut self, cond: Condition) -> Self {
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
            let payload = TypedPayload::new(payload);
            self.begin_payload_unchecked(
                &payload as *const _ as *const ffi::c_void,
                std::mem::size_of::<TypedPayload<P>>(),
            )
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
        let should_begin = sys::igBeginDragDropSource(self.flags.bits() as i32);

        if should_begin {
            sys::igSetDragDropPayload(self.ui.scratch_txt(&self.name), ptr, size, self.cond as i32);

            Some(DragDropSourceTooltip::new(self.ui))
        } else {
            None
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

/// Drag drop target for accepting payloads
///
/// This struct is created by [`Ui::drag_drop_target`] and provides
/// methods for accepting different types of payloads.
#[derive(Debug)]
pub struct DragDropTarget<'ui>(&'ui Ui);

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
        flags: DragDropFlags,
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
        flags: DragDropFlags,
    ) -> Option<Result<DragDropPayloadPod<T>, PayloadIsWrongType>> {
        let output = unsafe { self.accept_payload_unchecked(name, flags) };

        output.map(|payload| {
            let typed_payload = unsafe { &*(payload.data as *const TypedPayload<T>) };

            if typed_payload.type_id == any::TypeId::of::<T>() {
                Ok(DragDropPayloadPod {
                    data: typed_payload.data,
                    preview: payload.preview,
                    delivery: payload.delivery,
                })
            } else {
                Err(PayloadIsWrongType)
            }
        })
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
        flags: DragDropFlags,
    ) -> Option<DragDropPayload> {
        let inner = sys::igAcceptDragDropPayload(self.0.scratch_txt(name), flags.bits() as i32);

        if inner.is_null() {
            None
        } else {
            let inner = *inner;
            Some(DragDropPayload {
                data: inner.Data,
                size: inner.DataSize as usize,
                preview: inner.Preview,
                delivery: inner.Delivery,
            })
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

// Payload types and utilities

/// Wrapper for typed payloads with runtime type checking
#[repr(C)]
struct TypedPayload<T> {
    type_id: any::TypeId,
    data: T,
}

impl<T: 'static> TypedPayload<T> {
    fn new(data: T) -> Self {
        Self {
            type_id: any::TypeId::of::<T>(),
            data,
        }
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

/// Error type for payload type mismatches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadIsWrongType;

impl std::fmt::Display for PayloadIsWrongType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "drag drop payload has wrong type")
    }
}

impl std::error::Error for PayloadIsWrongType {}
