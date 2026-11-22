use super::super::NodeEditor;
use crate::{MiniMapLocation, NodeId, sys};
use std::ffi::c_void;

impl NodeEditor<'_> {
    /// Draws a minimap over the submitted graph. Read its hovered nodes from
    /// [`PostEditor::minimap_hovered_nodes`](crate::PostEditor::minimap_hovered_nodes).
    pub fn minimap(&self, size_fraction: f32, location: MiniMapLocation) {
        assert!(self.host.is_current(), "minimap requires the editor canvas");
        assert!(
            size_fraction.is_finite() && size_fraction > 0.0 && size_fraction <= 1.0,
            "dear-imnodes: minimap size fraction must be finite and in (0, 1]"
        );
        self.frame.minimap.set(Some((size_fraction, location)));
    }
}

// The caller has bound the active editor. Keep the callback target borrowed for the entire
// native End. A positive size enables MiniMapUpdate, which clears both callback fields.
pub(super) unsafe fn end_editor(
    minimap: Option<(f32, MiniMapLocation)>,
    hovered: &mut Vec<NodeId>,
) {
    unsafe extern "C" fn record(id: i32, data: *mut c_void) {
        unsafe { (*data.cast::<Vec<NodeId>>()).push(NodeId::new(id)) };
    }
    unsafe {
        if let Some((size, location)) = minimap {
            sys::imnodes_MiniMap(
                size,
                location as sys::ImNodesMiniMapLocation,
                Some(record),
                std::ptr::from_mut(hovered).cast(),
            );
        }
        sys::imnodes_EndNodeEditor();
    }
}
