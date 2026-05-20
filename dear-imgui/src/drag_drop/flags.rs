use crate::sys;

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

pub(super) fn validate_drag_drop_source_flags(caller: &str, flags: DragDropSourceFlags) {
    let unsupported = flags.bits() & !DragDropSourceFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDragDropFlags source bits: 0x{unsupported:X}"
    );
}

pub(super) fn validate_drag_drop_target_flags(caller: &str, flags: DragDropTargetFlags) {
    let unsupported = flags.bits() & !DragDropTargetFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDragDropFlags target bits: 0x{unsupported:X}"
    );
}
