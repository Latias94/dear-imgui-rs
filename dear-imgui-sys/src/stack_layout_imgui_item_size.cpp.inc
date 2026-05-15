

    int dear_imgui_stack_layout_type = 0;
    if (dear_imgui_stack_current_layout_type(window, &dear_imgui_stack_layout_type))
    {
        // Mirrors imgui-node-editor's vendored stack layout ItemSize() branch.
        // The extension needs both vertical and horizontal cursor advancement
        // to be controlled by the active stack layout instead of ImGui's
        // regular window layout mode.
        if (dear_imgui_stack_layout_type == ImGuiLayoutType_Vertical)
        {
            const float offset_to_match_baseline_y = (text_baseline_y >= 0) ? ImMax(0.0f, window->DC.CurrLineTextBaseOffset - text_baseline_y) : 0.0f;
            const float line_height = ImMax(window->DC.CurrLineSize.y, size.y + offset_to_match_baseline_y);
            window->DC.CursorPosPrevLine.x = window->DC.CursorPos.x + size.x;
            window->DC.CursorPosPrevLine.y = window->DC.CursorPos.y;
            window->DC.CursorPos.x = IM_TRUNC(window->Pos.x + window->DC.Indent.x + window->DC.ColumnsOffset.x);
            window->DC.CursorPos.y = IM_TRUNC(window->DC.CursorPos.y + line_height + g.Style.ItemSpacing.y);
            window->DC.CursorMaxPos.x = ImMax(window->DC.CursorMaxPos.x, window->DC.CursorPosPrevLine.x);
            window->DC.CursorMaxPos.y = ImMax(window->DC.CursorMaxPos.y, window->DC.CursorPos.y - g.Style.ItemSpacing.y);
            window->DC.PrevLineSize.x = 0.0f;
            window->DC.PrevLineSize.y = line_height;
            window->DC.CurrLineSize.y = 0.0f;
            window->DC.PrevLineTextBaseOffset = ImMax(window->DC.CurrLineTextBaseOffset, text_baseline_y);
            window->DC.CurrLineTextBaseOffset = 0.0f;
            window->DC.IsSameLine = window->DC.IsSetPos = false;
            return;
        }

        const float line_width = ImMax(window->DC.CurrLineSize.x, size.x);
        window->DC.CursorPosPrevLine.x = window->DC.CursorPos.x;
        window->DC.CursorPosPrevLine.y = window->DC.CursorPos.y + size.y;
        window->DC.CursorPos.x = IM_TRUNC(window->DC.CursorPos.x + line_width + g.Style.ItemSpacing.x);
        window->DC.CursorPos.y = IM_TRUNC(window->DC.CursorPosPrevLine.y - size.y);
        window->DC.CursorMaxPos.x = ImMax(window->DC.CursorMaxPos.x, window->DC.CursorPos.x - g.Style.ItemSpacing.x);
        window->DC.CursorMaxPos.y = ImMax(window->DC.CursorMaxPos.y, window->DC.CursorPosPrevLine.y);
        window->DC.PrevLineSize.x = line_width;
        window->DC.PrevLineSize.y = 0.0f;
        window->DC.CurrLineSize.x = 0.0f;
        window->DC.PrevLineTextBaseOffset = ImMax(window->DC.CurrLineTextBaseOffset, text_baseline_y);
        window->DC.CurrLineTextBaseOffset = window->DC.PrevLineTextBaseOffset;
        window->DC.IsSameLine = window->DC.IsSetPos = false;
        return;
    }
