use crate::{DockNodeFlags, Id, sys, ui::Ui};
use std::ffi::{CStr, CString};

pub(crate) const MAX_DOCKSPACE_HOST_NAME_BYTES: usize = 236;
const MIN_TRUNCATABLE_DOCKSPACE_SIZE: f32 = -2_147_483_648.0;
const MAX_TRUNCATABLE_DOCKSPACE_SIZE_EXCLUSIVE: f32 = 2_147_483_648.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateDockspaceSubmission;

pub(crate) fn assert_docking_available(caller: &str) {
    unsafe {
        let context = sys::igGetCurrentContext();
        assert!(
            !context.is_null(),
            "{caller} requires a valid ImGui context"
        );
        assert!(
            (*context).WithinFrameScope,
            "{caller} requires an open Dear ImGui frame"
        );

        let io = sys::igGetIO_Nil();
        assert!(!io.is_null(), "{caller} requires a valid ImGui IO object");
        let requested = (*io).ConfigFlags & sys::ImGuiConfigFlags_DockingEnable as i32;
        let active = (*context).ConfigFlagsCurrFrame & sys::ImGuiConfigFlags_DockingEnable as i32;
        assert_eq!(
            requested, active,
            "{caller} cannot change ConfigFlags::DOCKING_ENABLE while a frame is open"
        );
        assert!(
            active != 0,
            "{caller} requires ConfigFlags::DOCKING_ENABLE before the first frame"
        );
    }
}

pub(crate) fn claim_dockspace_submission(
    ui: &Ui,
    caller: &str,
    id: Id,
    flags: DockNodeFlags,
    current_window_submission: bool,
) -> Result<Option<crate::context::binding::DockspaceFrameClaim>, DuplicateDockspaceSubmission> {
    assert_docking_available(caller);
    let current_window_skips_items =
        current_window_submission && current_window_skips_items(caller);
    if flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) || current_window_skips_items {
        return Ok(None);
    }

    let frame = unsafe { sys::igGetFrameCount() };
    ui.binding()
        .claim_dockspace_submission(frame, id.raw())
        .map(Some)
        .ok_or(DuplicateDockspaceSubmission)
}

pub(crate) fn assert_existing_dockspace_node_is_root(caller: &str, id: Id) {
    let node = unsafe { sys::igDockBuilderGetNode(id.raw()) };
    assert!(
        node.is_null() || unsafe { sys::ImGuiDockNode_IsRootNode(node) },
        "{caller} ID {id:?} resolves to a child of another dock tree"
    );
}

pub(crate) fn assert_dockspace_has_no_active_content(caller: &str, id: Id) {
    assert!(
        unsafe { sys::dear_imgui_rs_dock_builder_root_has_active_content_window(id.raw()) } == 0,
        "{caller} must submit dockspace {id:?} before its hosted windows"
    );
}

pub(crate) fn current_window_skips_items(caller: &str) -> bool {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        assert!(
            !window.is_null(),
            "{caller} requires a current ImGui window"
        );
        (*window).SkipItems
    }
}

pub(crate) fn main_viewport_dockspace_host_name(caller: &str) -> CString {
    unsafe {
        let viewport = sys::igGetMainViewport();
        assert!(!viewport.is_null(), "{caller} requires a main viewport");
        CString::new(format!("WindowOverViewport_{:08X}", (*viewport).ID))
            .expect("generated viewport host name cannot contain a NUL byte")
    }
}

pub(crate) fn assert_dockspace_host_name_supported(caller: &str) {
    let name_len = current_dockspace_host_name_len(caller);
    assert!(
        name_len <= MAX_DOCKSPACE_HOST_NAME_BYTES,
        "{caller} host window name is {name_len} bytes; DockSpace supports at most \
         {MAX_DOCKSPACE_HOST_NAME_BYTES} bytes without truncating its internal window identity"
    );
}

pub(crate) fn current_dockspace_host_name_len(caller: &str) -> usize {
    unsafe {
        let window = sys::igGetCurrentWindowRead();
        assert!(
            !window.is_null(),
            "{caller} requires a current ImGui window"
        );
        let name = (*window).Name;
        assert!(!name.is_null(), "{caller} requires a named host window");
        CStr::from_ptr(name).to_bytes().len()
    }
}

pub(crate) fn assert_nonzero_id(caller: &str, name: &str, id: Id) {
    assert!(id.raw() != 0, "{caller} {name} must be non-zero");
}

pub(crate) fn assert_dockspace_size(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value
            .iter()
            .all(|component| is_valid_dockspace_size_component(*component)),
        "{caller} {name} components must be finite and safely truncatable to i32"
    );
}

pub(crate) fn is_valid_dockspace_size_component(value: f32) -> bool {
    value.is_finite()
        && (MIN_TRUNCATABLE_DOCKSPACE_SIZE..MAX_TRUNCATABLE_DOCKSPACE_SIZE_EXCLUSIVE)
            .contains(&value)
}

#[cfg(test)]
mod tests {
    use super::assert_dockspace_size;

    #[test]
    fn dockspace_size_accepts_exact_native_cast_boundaries() {
        let upper_exclusive = 2_147_483_648.0f32;
        let largest_valid = f32::from_bits(upper_exclusive.to_bits() - 1);

        assert_dockspace_size("test", "size", [-2_147_483_648.0, largest_valid]);
        for invalid in [f32::NAN, f32::INFINITY, f32::MAX, upper_exclusive] {
            assert!(
                std::panic::catch_unwind(|| {
                    assert_dockspace_size("test", "size", [invalid, 0.0]);
                })
                .is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(|| {
                assert_dockspace_size("test", "size", [-f32::MAX, 0.0]);
            })
            .is_err()
        );
    }
}
