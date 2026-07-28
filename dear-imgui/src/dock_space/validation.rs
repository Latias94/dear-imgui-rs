use crate::{DockNodeFlags, Id, sys, ui::Ui};
use std::ffi::{CStr, CString};

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
) -> Result<Option<crate::context::binding::DockspaceSubmissionClaim>, DuplicateDockspaceSubmission>
{
    assert_docking_available(caller);
    let current_window_skips_items = current_window_submission
        && unsafe {
            let window = sys::igGetCurrentWindowRead();
            assert!(
                !window.is_null(),
                "{caller} requires a current ImGui window"
            );
            (*window).SkipItems
        };
    if flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) || current_window_skips_items {
        return Ok(None);
    }

    let frame = unsafe { sys::igGetFrameCount() };
    ui.binding()
        .claim_dockspace_submission(frame, id.raw())
        .map(Some)
        .ok_or(DuplicateDockspaceSubmission)
}

pub(crate) fn main_viewport_dockspace_host_name(caller: &str) -> CString {
    unsafe {
        let viewport = sys::igGetMainViewport();
        assert!(!viewport.is_null(), "{caller} requires a main viewport");
        CString::new(format!("WindowOverViewport_{:08X}", (*viewport).ID))
            .expect("generated viewport host name cannot contain a NUL byte")
    }
}

pub(crate) fn window_skips_items(name: &CStr) -> bool {
    unsafe {
        let window = sys::igFindWindowByName(name.as_ptr());
        !window.is_null() && (*window).SkipItems
    }
}

pub(crate) fn assert_nonzero_id(caller: &str, name: &str, id: Id) {
    assert!(id.raw() != 0, "{caller} {name} must be non-zero");
}

pub(super) fn optional_nonzero_id_raw(caller: &str, name: &str, id: Option<Id>) -> sys::ImGuiID {
    id.map_or(0, |id| {
        assert_nonzero_id(caller, name, id);
        id.raw()
    })
}

pub(crate) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}
