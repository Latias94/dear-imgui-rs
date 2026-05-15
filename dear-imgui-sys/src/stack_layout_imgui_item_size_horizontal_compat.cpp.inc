    int dear_imgui_stack_layout_type_after = 0;
    const bool dear_imgui_stack_layout_active_after = dear_imgui_stack_current_layout_type(window, &dear_imgui_stack_layout_type_after);
    const bool dear_imgui_layout_is_horizontal = dear_imgui_stack_layout_active_after
        ? dear_imgui_stack_layout_type_after == ImGuiLayoutType_Horizontal
        : window->DC.LayoutType == ImGuiLayoutType_Horizontal;
    if (dear_imgui_layout_is_horizontal)
        SameLine();
