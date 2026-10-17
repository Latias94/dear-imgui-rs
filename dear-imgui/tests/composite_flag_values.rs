use dear_imgui_rs::{PopupQueryFlags, TableFlags, TreeNodeFlags, WindowFlags, sys};

#[test]
fn window_composites_match_upstream() {
    assert_eq!(WindowFlags::NO_NAV.bits(), sys::ImGuiWindowFlags_NoNav);
    assert_eq!(
        WindowFlags::NO_DECORATION.bits(),
        sys::ImGuiWindowFlags_NoDecoration
    );
    assert_eq!(
        WindowFlags::NO_INPUTS.bits(),
        sys::ImGuiWindowFlags_NoInputs
    );
}

#[test]
fn table_border_composites_match_upstream() {
    for (flags, expected) in [
        (TableFlags::BORDERS_H, sys::ImGuiTableFlags_BordersH),
        (TableFlags::BORDERS_V, sys::ImGuiTableFlags_BordersV),
        (TableFlags::BORDERS_INNER, sys::ImGuiTableFlags_BordersInner),
        (TableFlags::BORDERS_OUTER, sys::ImGuiTableFlags_BordersOuter),
        (TableFlags::BORDERS, sys::ImGuiTableFlags_Borders),
    ] {
        assert_eq!(flags.bits(), expected);
    }
}

#[test]
fn popup_and_tree_composites_match_upstream() {
    assert_eq!(
        PopupQueryFlags::ANY_POPUP.bits(),
        sys::ImGuiPopupFlags_AnyPopup
    );
    assert_eq!(
        TreeNodeFlags::COLLAPSING_HEADER.bits(),
        sys::ImGuiTreeNodeFlags_CollapsingHeader
    );
}
