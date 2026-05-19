use dear_imgui_rs as imgui;

#[test]
fn condition_values_match_dear_imgui_cond() {
    assert_eq!(
        imgui::Condition::Always as i32,
        imgui::sys::ImGuiCond_Always
    );
    assert_eq!(imgui::Condition::Once as i32, imgui::sys::ImGuiCond_Once);
    assert_eq!(
        imgui::Condition::FirstUseEver as i32,
        imgui::sys::ImGuiCond_FirstUseEver
    );
    assert_eq!(
        imgui::Condition::Appearing as i32,
        imgui::sys::ImGuiCond_Appearing
    );

    for cond in [
        imgui::Condition::Always,
        imgui::Condition::Once,
        imgui::Condition::FirstUseEver,
        imgui::Condition::Appearing,
    ] {
        let cond = cond as i32;
        assert!(cond.is_positive());
        assert!((cond as u32).is_power_of_two());
    }
}

#[test]
fn drag_drop_payload_condition_values_match_supported_imgui_cond() {
    assert_eq!(
        imgui::DragDropPayloadCond::Always as i32,
        imgui::sys::ImGuiCond_Always
    );
    assert_eq!(
        imgui::DragDropPayloadCond::Once as i32,
        imgui::sys::ImGuiCond_Once
    );
}

#[test]
fn drag_drop_flags_are_split_by_source_and_target_domain() {
    assert_eq!(
        imgui::DragDropSourceFlags::NO_PREVIEW_TOOLTIP.bits(),
        imgui::sys::ImGuiDragDropFlags_SourceNoPreviewTooltip as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::NO_DISABLE_HOVER.bits(),
        imgui::sys::ImGuiDragDropFlags_SourceNoDisableHover as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::NO_HOLD_TO_OPEN_OTHERS.bits(),
        imgui::sys::ImGuiDragDropFlags_SourceNoHoldToOpenOthers as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::ALLOW_NULL_ID.bits(),
        imgui::sys::ImGuiDragDropFlags_SourceAllowNullID as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::EXTERN.bits(),
        imgui::sys::ImGuiDragDropFlags_SourceExtern as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::PAYLOAD_AUTO_EXPIRE.bits(),
        imgui::sys::ImGuiDragDropFlags_PayloadAutoExpire as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::PAYLOAD_NO_CROSS_CONTEXT.bits(),
        imgui::sys::ImGuiDragDropFlags_PayloadNoCrossContext as u32
    );
    assert_eq!(
        imgui::DragDropSourceFlags::PAYLOAD_NO_CROSS_PROCESS.bits(),
        imgui::sys::ImGuiDragDropFlags_PayloadNoCrossProcess as u32
    );

    assert_eq!(
        imgui::DragDropTargetFlags::BEFORE_DELIVERY.bits(),
        imgui::sys::ImGuiDragDropFlags_AcceptBeforeDelivery as u32
    );
    assert_eq!(
        imgui::DragDropTargetFlags::NO_DRAW_DEFAULT_RECT.bits(),
        imgui::sys::ImGuiDragDropFlags_AcceptNoDrawDefaultRect as u32
    );
    assert_eq!(
        imgui::DragDropTargetFlags::NO_PREVIEW_TOOLTIP.bits(),
        imgui::sys::ImGuiDragDropFlags_AcceptNoPreviewTooltip as u32
    );
    assert_eq!(
        imgui::DragDropTargetFlags::DRAW_AS_HOVERED.bits(),
        imgui::sys::ImGuiDragDropFlags_AcceptDrawAsHovered as u32
    );
    assert_eq!(
        imgui::DragDropTargetFlags::PEEK_ONLY.bits(),
        imgui::sys::ImGuiDragDropFlags_AcceptPeekOnly as u32
    );

    let source_bits = imgui::DragDropSourceFlags::all().bits();
    let target_bits = imgui::DragDropTargetFlags::all().bits();
    assert_eq!(source_bits & target_bits, 0);
}

#[test]
fn slider_and_drag_flags_match_supported_imgui_subsets() {
    assert!(
        !imgui::SliderFlags::all().contains(imgui::SliderFlags::from_bits_retain(
            imgui::sys::ImGuiSliderFlags_WrapAround
        ))
    );

    assert_eq!(
        imgui::DragFlags::WRAP_AROUND.bits(),
        imgui::sys::ImGuiSliderFlags_WrapAround
    );

    assert_eq!(
        imgui::SliderFlags::ALWAYS_CLAMP.bits(),
        imgui::DragFlags::ALWAYS_CLAMP.bits()
    );
    assert_eq!(
        imgui::SliderFlags::LOGARITHMIC.bits(),
        imgui::DragFlags::LOGARITHMIC.bits()
    );
}

#[test]
fn table_column_setup_flags_exclude_status_flags() {
    let status_bits = imgui::sys::ImGuiTableColumnFlags_IsEnabled
        | imgui::sys::ImGuiTableColumnFlags_IsVisible
        | imgui::sys::ImGuiTableColumnFlags_IsSorted
        | imgui::sys::ImGuiTableColumnFlags_IsHovered;

    assert!(
        !imgui::TableColumnFlags::all()
            .intersects(imgui::TableColumnFlags::from_bits_retain(status_bits))
    );

    let state_flags = imgui::TableColumnStateFlags::from_bits_retain(status_bits);
    assert!(state_flags.contains(imgui::TableColumnStateFlags::IS_ENABLED));
    assert!(state_flags.contains(imgui::TableColumnStateFlags::IS_VISIBLE));
    assert!(state_flags.contains(imgui::TableColumnStateFlags::IS_SORTED));
    assert!(state_flags.contains(imgui::TableColumnStateFlags::IS_HOVERED));
}

#[test]
fn combo_options_keep_mutually_exclusive_bits_out_of_flags() {
    let height_bits = imgui::sys::ImGuiComboFlags_HeightSmall
        | imgui::sys::ImGuiComboFlags_HeightRegular
        | imgui::sys::ImGuiComboFlags_HeightLarge
        | imgui::sys::ImGuiComboFlags_HeightLargest;
    let preview_bits = imgui::sys::ImGuiComboFlags_NoArrowButton
        | imgui::sys::ImGuiComboFlags_NoPreview
        | imgui::sys::ImGuiComboFlags_WidthFitPreview;

    assert!(
        !imgui::ComboBoxFlags::all().intersects(imgui::ComboBoxFlags::from_bits_retain(
            height_bits | preview_bits
        ))
    );

    let no_preview = imgui::ComboBoxOptions::new()
        .preview_mode(imgui::ComboBoxPreviewMode::NoPreview)
        .height(imgui::ComboBoxHeight::Large);
    assert_eq!(
        no_preview.bits(),
        imgui::sys::ImGuiComboFlags_NoPreview | imgui::sys::ImGuiComboFlags_HeightLarge
    );
}

#[test]
fn table_options_keep_single_choice_masks_out_of_flags() {
    let table_sizing_bits = imgui::sys::ImGuiTableFlags_SizingFixedFit
        | imgui::sys::ImGuiTableFlags_SizingFixedSame
        | imgui::sys::ImGuiTableFlags_SizingStretchProp
        | imgui::sys::ImGuiTableFlags_SizingStretchSame;
    let column_width_bits = imgui::sys::ImGuiTableColumnFlags_WidthFixed
        | imgui::sys::ImGuiTableColumnFlags_WidthStretch;
    let column_indent_bits = imgui::sys::ImGuiTableColumnFlags_IndentEnable
        | imgui::sys::ImGuiTableColumnFlags_IndentDisable;

    assert!(
        !imgui::TableFlags::all()
            .intersects(imgui::TableFlags::from_bits_retain(table_sizing_bits))
    );
    assert!(
        !imgui::TableColumnFlags::all().intersects(imgui::TableColumnFlags::from_bits_retain(
            column_width_bits | column_indent_bits
        ))
    );

    assert_eq!(
        imgui::TableOptions::new()
            .sizing_policy(imgui::TableSizingPolicy::StretchProp)
            .bits(),
        imgui::sys::ImGuiTableFlags_SizingStretchProp
    );
    assert_eq!(
        imgui::TableColumnIndent::Disable.bits(),
        imgui::sys::ImGuiTableColumnFlags_IndentDisable
    );
}

#[test]
fn color_options_split_independent_flags_by_widget_domain() {
    let display_bits = (imgui::sys::ImGuiColorEditFlags_DisplayRGB
        | imgui::sys::ImGuiColorEditFlags_DisplayHSV
        | imgui::sys::ImGuiColorEditFlags_DisplayHex) as u32;
    let data_type_bits =
        (imgui::sys::ImGuiColorEditFlags_Uint8 | imgui::sys::ImGuiColorEditFlags_Float) as u32;
    let picker_bits = (imgui::sys::ImGuiColorEditFlags_PickerHueBar
        | imgui::sys::ImGuiColorEditFlags_PickerHueWheel) as u32;
    let input_bits = (imgui::sys::ImGuiColorEditFlags_InputRGB
        | imgui::sys::ImGuiColorEditFlags_InputHSV) as u32;

    assert_eq!(
        imgui::ColorEditFlags::all().bits()
            & (display_bits | data_type_bits | picker_bits | input_bits),
        0
    );
    assert_eq!(
        imgui::ColorPickerFlags::all().bits()
            & (display_bits | data_type_bits | picker_bits | input_bits),
        0
    );
    assert_eq!(
        imgui::ColorButtonFlags::all().bits()
            & (display_bits | data_type_bits | picker_bits | input_bits),
        0
    );

    assert!(imgui::ColorEditFlags::all().contains(imgui::ColorEditFlags::NO_PICKER));
    assert!(
        !imgui::ColorEditFlags::all().intersects(imgui::ColorEditFlags::from_bits_retain(
            imgui::sys::ImGuiColorEditFlags_NoSidePreview as u32
                | imgui::sys::ImGuiColorEditFlags_NoBorder as u32
        ))
    );
    assert!(imgui::ColorPickerFlags::all().contains(imgui::ColorPickerFlags::NO_SIDE_PREVIEW));
    assert!(
        !imgui::ColorPickerFlags::all().intersects(imgui::ColorPickerFlags::from_bits_retain(
            imgui::sys::ImGuiColorEditFlags_NoPicker as u32
                | imgui::sys::ImGuiColorEditFlags_NoBorder as u32
                | imgui::sys::ImGuiColorEditFlags_NoDragDrop as u32
        ))
    );
    assert!(imgui::ColorButtonFlags::all().contains(imgui::ColorButtonFlags::NO_BORDER));
    assert!(
        !imgui::ColorButtonFlags::all().intersects(imgui::ColorButtonFlags::from_bits_retain(
            imgui::sys::ImGuiColorEditFlags_NoPicker as u32
                | imgui::sys::ImGuiColorEditFlags_NoSidePreview as u32
                | imgui::sys::ImGuiColorEditFlags_AlphaBar as u32
        ))
    );

    assert_eq!(
        imgui::ColorEditOptions::new()
            .display_mode(imgui::ColorDisplayMode::Hsv)
            .data_type(imgui::ColorDataType::Uint8)
            .picker_mode(imgui::ColorPickerMode::HueBar)
            .input_mode(imgui::ColorInputMode::Rgb)
            .bits(),
        (imgui::sys::ImGuiColorEditFlags_DisplayHSV
            | imgui::sys::ImGuiColorEditFlags_Uint8
            | imgui::sys::ImGuiColorEditFlags_PickerHueBar
            | imgui::sys::ImGuiColorEditFlags_InputRGB) as u32
    );
}

#[test]
fn tab_item_options_keep_placement_out_of_flags() {
    let placement_bits =
        imgui::sys::ImGuiTabItemFlags_Leading | imgui::sys::ImGuiTabItemFlags_Trailing;
    let fitting_policy_bits = imgui::sys::ImGuiTabBarFlags_FittingPolicyMixed
        | imgui::sys::ImGuiTabBarFlags_FittingPolicyShrink
        | imgui::sys::ImGuiTabBarFlags_FittingPolicyScroll;

    assert!(
        !imgui::TabItemFlags::all()
            .intersects(imgui::TabItemFlags::from_bits_retain(placement_bits))
    );
    assert!(
        !imgui::TabBarFlags::all()
            .intersects(imgui::TabBarFlags::from_bits_retain(fitting_policy_bits))
    );
    assert_eq!(
        imgui::TabItemOptions::new()
            .placement(imgui::TabItemPlacement::Trailing)
            .bits(),
        imgui::sys::ImGuiTabItemFlags_Trailing
    );
    assert_eq!(
        imgui::TabBarOptions::new()
            .fitting_policy(imgui::TabBarFittingPolicy::Scroll)
            .bits(),
        imgui::sys::ImGuiTabBarFlags_FittingPolicyScroll
    );
}

#[test]
fn popup_context_options_keep_mouse_buttons_out_of_flags() {
    let mouse_button_bits = imgui::sys::ImGuiPopupFlags_MouseButtonLeft
        | imgui::sys::ImGuiPopupFlags_MouseButtonRight
        | imgui::sys::ImGuiPopupFlags_MouseButtonMiddle;

    assert!(!imgui::PopupContextFlags::all().intersects(
        imgui::PopupContextFlags::from_bits_retain(mouse_button_bits)
    ));
    assert!(
        !imgui::PopupOpenFlags::all().intersects(imgui::PopupOpenFlags::from_bits_retain(
            imgui::sys::ImGuiPopupFlags_AnyPopupId
        ))
    );
    assert!(
        !imgui::PopupQueryFlags::all().intersects(imgui::PopupQueryFlags::from_bits_retain(
            imgui::sys::ImGuiPopupFlags_NoReopen
        ))
    );
    assert_eq!(
        imgui::PopupContextOptions::new().bits(),
        imgui::sys::ImGuiPopupFlags_MouseButtonRight
    );
    assert_eq!(
        imgui::PopupContextOptions::new()
            .mouse_button(imgui::PopupContextMouseButton::Left)
            .bits(),
        imgui::sys::ImGuiPopupFlags_MouseButtonLeft
    );
}

#[test]
fn multi_select_options_keep_click_policy_out_of_flags() {
    let click_policy_bits = imgui::sys::ImGuiMultiSelectFlags_SelectOnAuto
        | imgui::sys::ImGuiMultiSelectFlags_SelectOnClickAlways
        | imgui::sys::ImGuiMultiSelectFlags_SelectOnClickRelease;
    let box_select_bits = imgui::sys::ImGuiMultiSelectFlags_BoxSelect1d
        | imgui::sys::ImGuiMultiSelectFlags_BoxSelect2d;
    let scope_bits =
        imgui::sys::ImGuiMultiSelectFlags_ScopeWindow | imgui::sys::ImGuiMultiSelectFlags_ScopeRect;
    let nav_wrap_bits = imgui::sys::ImGuiMultiSelectFlags_NavWrapX;

    assert!(
        !imgui::MultiSelectFlags::all().intersects(imgui::MultiSelectFlags::from_bits_retain(
            click_policy_bits | box_select_bits | scope_bits | nav_wrap_bits
        ))
    );
    assert_eq!(
        imgui::MultiSelectOptions::new()
            .click_policy(imgui::MultiSelectClickPolicy::ClickRelease)
            .box_select(imgui::MultiSelectBoxSelect::TwoDimensional)
            .scope(imgui::MultiSelectScopeKind::Rect)
            .bits(),
        imgui::sys::ImGuiMultiSelectFlags_SelectOnClickRelease
            | imgui::sys::ImGuiMultiSelectFlags_BoxSelect2d
            | imgui::sys::ImGuiMultiSelectFlags_ScopeRect
    );
    assert_eq!(
        imgui::MultiSelectOptions::new()
            .scope(imgui::MultiSelectScopeKind::WindowWithNavWrapX)
            .bits(),
        imgui::sys::ImGuiMultiSelectFlags_ScopeWindow | imgui::sys::ImGuiMultiSelectFlags_NavWrapX
    );
}
