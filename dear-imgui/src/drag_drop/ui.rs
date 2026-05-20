use super::flags::{DragDropPayloadCond, DragDropSourceFlags};
use super::payload::DragDropPayload;
use super::source::DragDropSource;
use super::target::DragDropTarget;
use crate::{Ui, sys};

impl Ui {
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
    /// Returns `Some(DragDropTarget)` if the last item can accept drops,
    /// `None` otherwise.
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
        let should_begin = unsafe { sys::igBeginDragDropTarget() };
        if should_begin {
            Some(DragDropTarget(self))
        } else {
            None
        }
    }

    /// Returns the current drag and drop payload, if any.
    ///
    /// This is a convenience wrapper over `ImGui::GetDragDropPayload`.
    ///
    /// The returned payload is owned and managed by Dear ImGui and may become invalid
    /// after the drag operation completes. Do not cache it beyond the current frame.
    #[doc(alias = "GetDragDropPayload")]
    pub fn drag_drop_payload(&self) -> Option<DragDropPayload> {
        unsafe {
            let ptr = sys::igGetDragDropPayload();
            if ptr.is_null() {
                return None;
            }
            Some(DragDropPayload::from_raw(*ptr))
        }
    }
}
