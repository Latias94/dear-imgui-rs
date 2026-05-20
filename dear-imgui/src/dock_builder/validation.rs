use crate::dock_space::assert_nonzero_id;
use crate::{Id, sys};
use std::os::raw::c_void;

pub(super) fn assert_existing_dock_node(caller: &str, node_id: Id) {
    assert_nonzero_id(caller, "node_id", node_id);
    let ctx = unsafe { sys::igGetCurrentContext() };
    assert!(!ctx.is_null(), "{caller} requires a current ImGui context");
    let node = unsafe { sys::igDockBuilderGetNode(node_id.into()) };
    assert!(!node.is_null(), "{caller} requires an existing dock node");
}

pub(super) fn dock_node_depth_from_i32(raw: i32) -> usize {
    usize::try_from(raw).expect("Dear ImGui returned a negative dock node depth")
}

pub(super) unsafe fn free_imgui_id_vector(vector: &mut sys::ImVector_ImGuiID) {
    if !vector.Data.is_null() {
        unsafe {
            sys::igMemFree(vector.Data as *mut c_void);
        }
        vector.Size = 0;
        vector.Capacity = 0;
        vector.Data = std::ptr::null_mut();
    }
}
