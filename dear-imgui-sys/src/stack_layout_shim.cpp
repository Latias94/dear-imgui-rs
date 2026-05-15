// Stack layout compatibility shim for blueprint-style node editor examples.
//
// Dear ImGui does not ship BeginHorizontal/BeginVertical/Spring as official
// public APIs. imgui-node-editor's blueprints example uses a vendored stack
// layout extension from the imgui-node-editor tree. This file provides a
// repository-owned C ABI shim with compatible behavior, implemented outside
// the Dear ImGui and imgui-node-editor submodules so upstream updates remain
// clean.
//
// References:
// - https://github.com/thedmd/imgui-node-editor
// - https://github.com/ocornut/imgui/discussions/6458
//
// The algorithm follows the MIT-licensed stack layout extension vendored by
// imgui-node-editor while storing all extra state in this shim instead of
// patching ImGuiWindowTempData.

#define IMGUI_DEFINE_MATH_OPERATORS
#include <imgui.h>
#include <imgui_internal.h>

#include <algorithm>
#include <cstdint>
#include <memory>
#include <unordered_map>
#include <vector>

namespace
{
enum class StackLayoutItemType
{
    Item,
    Spring
};

struct StackLayoutItem
{
    StackLayoutItemType Type;
    ImRect MeasuredBounds;
    float SpringWeight;
    float SpringSpacing;
    float SpringSize;
    float CurrentAlign;
    float CurrentAlignOffset;
    unsigned int VertexIndexBegin;
    unsigned int VertexIndexEnd;

    explicit StackLayoutItem(StackLayoutItemType type)
        : Type(type)
        , MeasuredBounds(0, 0, 0, 0)
        , SpringWeight(1.0f)
        , SpringSpacing(-1.0f)
        , SpringSize(0.0f)
        , CurrentAlign(0.0f)
        , CurrentAlignOffset(0.0f)
        , VertexIndexBegin(0)
        , VertexIndexEnd(0)
    {
    }
};

struct StackLayout
{
    ImGuiID Id;
    ImGuiLayoutType Type;
    bool Live;
    ImVec2 Size;
    ImVec2 CurrentSize;
    ImVec2 MinimumSize;
    ImVec2 MeasuredSize;
    std::vector<StackLayoutItem> Items;
    int CurrentItemIndex;
    int ParentItemIndex;
    StackLayout* Parent;
    StackLayout* FirstChild;
    StackLayout* NextSibling;
    float Align;
    float Indent;
    ImVec2 StartPos;
    ImVec2 StartCursorMaxPos;

    StackLayout(ImGuiID id, ImGuiLayoutType type)
        : Id(id)
        , Type(type)
        , Live(false)
        , Size(0, 0)
        , CurrentSize(0, 0)
        , MinimumSize(0, 0)
        , MeasuredSize(0, 0)
        , CurrentItemIndex(0)
        , ParentItemIndex(0)
        , Parent(nullptr)
        , FirstChild(nullptr)
        , NextSibling(nullptr)
        , Align(-1.0f)
        , Indent(0.0f)
        , StartPos(0, 0)
        , StartCursorMaxPos(0, 0)
    {
    }
};

struct WindowStackLayoutState
{
    int Frame = -1;
    std::unordered_map<ImGuiID, std::unique_ptr<StackLayout>> Layouts;
    std::vector<StackLayout*> LayoutStack;
    StackLayout* CurrentLayout = nullptr;
    StackLayoutItem* CurrentLayoutItem = nullptr;
};

struct ContextStackLayoutState
{
    std::unordered_map<ImGuiID, WindowStackLayoutState> Windows;
};

std::unordered_map<ImGuiContext*, ContextStackLayoutState> GStackLayoutStates;
thread_local ImGuiContext* GActiveStackLayoutContext = nullptr;
thread_local ImGuiID GActiveStackLayoutWindowId = 0;
thread_local int GActiveStackLayoutFrame = -1;
thread_local WindowStackLayoutState* GActiveStackLayoutState = nullptr;

constexpr float DEFAULT_LAYOUT_ALIGN = 0.5f;

void ClearFastActiveLayoutState(WindowStackLayoutState& state)
{
    if (GActiveStackLayoutState != &state)
        return;

    GActiveStackLayoutContext = nullptr;
    GActiveStackLayoutWindowId = 0;
    GActiveStackLayoutFrame = -1;
    GActiveStackLayoutState = nullptr;
}

void UpdateFastActiveLayoutState(WindowStackLayoutState& state)
{
    if (!state.CurrentLayout)
    {
        ClearFastActiveLayoutState(state);
        return;
    }

    ImGuiContext& g = *GImGui;
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    GActiveStackLayoutContext = &g;
    GActiveStackLayoutWindowId = window->ID;
    GActiveStackLayoutFrame = g.FrameCount;
    GActiveStackLayoutState = &state;
}

WindowStackLayoutState* FindFastActiveLayoutState(ImGuiWindow* window)
{
    if (!GImGui || !window || !GActiveStackLayoutState)
        return nullptr;

    ImGuiContext& g = *GImGui;
    if (GActiveStackLayoutContext != &g
        || GActiveStackLayoutWindowId != window->ID
        || GActiveStackLayoutFrame != g.FrameCount)
        return nullptr;

    if (!GActiveStackLayoutState->CurrentLayout)
        return nullptr;

    return GActiveStackLayoutState;
}

WindowStackLayoutState& GetWindowState()
{
    ImGuiContext& g = *GImGui;
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    auto& context_state = GStackLayoutStates[&g];
    auto& state = context_state.Windows[window->ID];

    if (state.Frame != g.FrameCount)
    {
        for (auto it = state.Layouts.begin(); it != state.Layouts.end();)
        {
            if (!it->second->Live)
                it = state.Layouts.erase(it);
            else
            {
                it->second->Live = false;
                ++it;
            }
        }

        state.LayoutStack.clear();
        state.CurrentLayout = nullptr;
        state.CurrentLayoutItem = nullptr;
        ClearFastActiveLayoutState(state);
        state.Frame = g.FrameCount;
    }

    return state;
}

StackLayout* FindLayout(WindowStackLayoutState& state, ImGuiID id, ImGuiLayoutType type)
{
    IM_ASSERT(type == ImGuiLayoutType_Horizontal || type == ImGuiLayoutType_Vertical);

    auto it = state.Layouts.find(id);
    if (it == state.Layouts.end())
        return nullptr;

    StackLayout* layout = it->second.get();
    if (layout->Type != type)
    {
        layout->Type = type;
        layout->MinimumSize = ImVec2(0.0f, 0.0f);
        layout->Items.clear();
    }

    return layout;
}

StackLayout* CreateNewLayout(WindowStackLayoutState& state, ImGuiID id, ImGuiLayoutType type, ImVec2 size)
{
    IM_ASSERT(type == ImGuiLayoutType_Horizontal || type == ImGuiLayoutType_Vertical);

    auto layout = std::make_unique<StackLayout>(id, type);
    layout->Size = size;
    StackLayout* result = layout.get();
    state.Layouts[id] = std::move(layout);
    return result;
}

void SignedIndent(float indent)
{
    if (indent > 0.0f)
        ImGui::Indent(indent);
    else if (indent < 0.0f)
        ImGui::Unindent(-indent);
}

void PushLayout(WindowStackLayoutState& state, StackLayout* layout)
{
    if (layout)
    {
        layout->Parent = state.CurrentLayout;
        if (layout->Parent != nullptr)
            layout->ParentItemIndex = layout->Parent->CurrentItemIndex;

        if (state.CurrentLayout)
        {
            layout->NextSibling = state.CurrentLayout->FirstChild;
            layout->FirstChild = nullptr;
            state.CurrentLayout->FirstChild = layout;
        }
        else
        {
            layout->NextSibling = nullptr;
            layout->FirstChild = nullptr;
        }
    }

    state.LayoutStack.push_back(layout);
    state.CurrentLayout = layout;
    state.CurrentLayoutItem = nullptr;
    UpdateFastActiveLayoutState(state);
}

void PopLayout(WindowStackLayoutState& state, StackLayout* layout)
{
    IM_ASSERT(!state.LayoutStack.empty());
    IM_ASSERT(state.LayoutStack.back() == layout);

    state.LayoutStack.pop_back();

    if (!state.LayoutStack.empty())
    {
        state.CurrentLayout = state.LayoutStack.back();
        if (state.CurrentLayout
            && state.CurrentLayout->CurrentItemIndex >= 0
            && state.CurrentLayout->CurrentItemIndex < static_cast<int>(state.CurrentLayout->Items.size()))
            state.CurrentLayoutItem = &state.CurrentLayout->Items[state.CurrentLayout->CurrentItemIndex];
        else
            state.CurrentLayoutItem = nullptr;
    }
    else
    {
        state.CurrentLayout = nullptr;
        state.CurrentLayoutItem = nullptr;
    }
    UpdateFastActiveLayoutState(state);
}

StackLayoutItem* GenerateLayoutItem(WindowStackLayoutState& state, StackLayout& layout, StackLayoutItemType type)
{
    IM_ASSERT(layout.CurrentItemIndex <= static_cast<int>(layout.Items.size()));

    if (layout.CurrentItemIndex < static_cast<int>(layout.Items.size()))
    {
        StackLayoutItem& item = layout.Items[layout.CurrentItemIndex];
        if (item.Type != type)
            item = StackLayoutItem(type);
    }
    else
    {
        layout.Items.emplace_back(type);
    }

    state.CurrentLayoutItem = &layout.Items[layout.CurrentItemIndex];
    return state.CurrentLayoutItem;
}

ImVec2 CalculateLayoutSize(StackLayout& layout, bool collapse_springs)
{
    ImVec2 bounds(0.0f, 0.0f);

    if (layout.Type == ImGuiLayoutType_Vertical)
    {
        for (StackLayoutItem& item : layout.Items)
        {
            ImVec2 item_size = item.MeasuredBounds.GetSize();
            if (item.Type == StackLayoutItemType::Item)
            {
                bounds.x = ImMax(bounds.x, item_size.x);
                bounds.y += item_size.y;
            }
            else
            {
                bounds.y += ImFloor(item.SpringSpacing);
                if (!collapse_springs)
                    bounds.y += item.SpringSize;
            }
        }
    }
    else
    {
        for (StackLayoutItem& item : layout.Items)
        {
            ImVec2 item_size = item.MeasuredBounds.GetSize();
            if (item.Type == StackLayoutItemType::Item)
            {
                bounds.x += item_size.x;
                bounds.y = ImMax(bounds.y, item_size.y);
            }
            else
            {
                bounds.x += ImFloor(item.SpringSpacing);
                if (!collapse_springs)
                    bounds.x += item.SpringSize;
            }
        }
    }

    return bounds;
}

float CalculateLayoutItemAlignmentOffset(StackLayout& layout, StackLayoutItem& item)
{
    if (item.CurrentAlign <= 0.0f)
        return 0.0f;

    ImVec2 item_size = item.MeasuredBounds.GetSize();
    float layout_extent = layout.Type == ImGuiLayoutType_Horizontal ? layout.CurrentSize.y : layout.CurrentSize.x;
    float item_extent = layout.Type == ImGuiLayoutType_Horizontal ? item_size.y : item_size.x;

    if (item_extent <= 0.0f)
        return 0.0f;

    return ImFloor(item.CurrentAlign * (layout_extent - item_extent));
}

void TranslateLayoutItem(StackLayoutItem& item, const ImVec2& offset)
{
    if ((offset.x == 0.0f && offset.y == 0.0f) || item.VertexIndexBegin == item.VertexIndexEnd)
        return;

    ImDrawList* draw_list = ImGui::GetWindowDrawList();
    ImDrawVert* begin = draw_list->VtxBuffer.Data + item.VertexIndexBegin;
    ImDrawVert* end = draw_list->VtxBuffer.Data + item.VertexIndexEnd;

    for (ImDrawVert* vtx = begin; vtx < end; ++vtx)
    {
        vtx->pos.x += offset.x;
        vtx->pos.y += offset.y;
    }
}

ImVec2 BalanceLayoutItemAlignment(StackLayout& layout, StackLayoutItem& item)
{
    ImVec2 position_correction(0.0f, 0.0f);
    if (item.CurrentAlign > 0.0f)
    {
        float item_align_offset = CalculateLayoutItemAlignmentOffset(layout, item);
        if (item.CurrentAlignOffset != item_align_offset)
        {
            float offset = item_align_offset - item.CurrentAlignOffset;
            if (layout.Type == ImGuiLayoutType_Horizontal)
                position_correction.y = offset;
            else
                position_correction.x = offset;

            TranslateLayoutItem(item, position_correction);
            item.CurrentAlignOffset = item_align_offset;
        }
    }

    return position_correction;
}

void BalanceLayoutItemsAlignment(StackLayout& layout)
{
    for (StackLayoutItem& item : layout.Items)
        BalanceLayoutItemAlignment(layout, item);
}

void BalanceLayoutSprings(StackLayout& layout)
{
    float total_spring_weight = 0.0f;
    int last_spring_item_index = -1;
    for (int i = 0; i < static_cast<int>(layout.Items.size()); ++i)
    {
        StackLayoutItem& item = layout.Items[i];
        if (item.Type == StackLayoutItemType::Spring)
        {
            total_spring_weight += item.SpringWeight;
            last_spring_item_index = i;
        }
    }

    const bool is_horizontal = layout.Type == ImGuiLayoutType_Horizontal;
    const bool is_auto_sized = ((is_horizontal ? layout.Size.x : layout.Size.y) <= 0.0f) && (layout.Parent == nullptr);
    const float occupied_space = is_horizontal ? layout.MinimumSize.x : layout.MinimumSize.y;
    const float available_space = is_auto_sized ? occupied_space : (is_horizontal ? layout.CurrentSize.x : layout.CurrentSize.y);
    const float free_space = ImMax(available_space - occupied_space, 0.0f);

    float span_start = 0.0f;
    float current_weight = 0.0f;
    for (int i = 0; i < static_cast<int>(layout.Items.size()); ++i)
    {
        StackLayoutItem& item = layout.Items[i];
        if (item.Type != StackLayoutItemType::Spring)
            continue;

        float last_spring_size = item.SpringSize;
        if (free_space > 0.0f && total_spring_weight > 0.0f)
        {
            float next_weight = current_weight + item.SpringWeight;
            float span_end = ImFloor(i == last_spring_item_index ? free_space : (free_space * next_weight / total_spring_weight));
            item.SpringSize = span_end - span_start;
            span_start = span_end;
            current_weight = next_weight;
        }
        else
        {
            item.SpringSize = 0.0f;
        }

        if (last_spring_size != item.SpringSize)
        {
            float difference = item.SpringSize - last_spring_size;
            ImVec2 offset = is_horizontal ? ImVec2(difference, 0.0f) : ImVec2(0.0f, difference);

            item.MeasuredBounds.Max += offset;

            for (int j = i + 1; j < static_cast<int>(layout.Items.size()); ++j)
            {
                StackLayoutItem& translated_item = layout.Items[j];
                TranslateLayoutItem(translated_item, offset);
                translated_item.MeasuredBounds.Min += offset;
                translated_item.MeasuredBounds.Max += offset;
            }
        }
    }
}

bool HasAnyNonZeroSpring(StackLayout& layout)
{
    for (StackLayoutItem& item : layout.Items)
        if (item.Type == StackLayoutItemType::Spring && item.SpringWeight > 0.0f)
            return true;
    return false;
}

void BalanceChildLayouts(StackLayout& layout)
{
    for (StackLayout* child = layout.FirstChild; child != nullptr; child = child->NextSibling)
    {
        if (child->Type == ImGuiLayoutType_Horizontal && child->Size.x <= 0.0f)
            child->CurrentSize.x = layout.CurrentSize.x;
        else if (child->Type == ImGuiLayoutType_Vertical && child->Size.y <= 0.0f)
            child->CurrentSize.y = layout.CurrentSize.y;

        BalanceChildLayouts(*child);

        if (HasAnyNonZeroSpring(*child)
            && child->ParentItemIndex >= 0
            && child->ParentItemIndex < static_cast<int>(layout.Items.size()))
        {
            StackLayoutItem& item = layout.Items[child->ParentItemIndex];
            if (child->Type == ImGuiLayoutType_Horizontal && child->Size.x <= 0.0f)
                item.MeasuredBounds.Max.x = ImMax(item.MeasuredBounds.Max.x, item.MeasuredBounds.Min.x + layout.CurrentSize.x);
            else if (child->Type == ImGuiLayoutType_Vertical && child->Size.y <= 0.0f)
                item.MeasuredBounds.Max.y = ImMax(item.MeasuredBounds.Max.y, item.MeasuredBounds.Min.y + layout.CurrentSize.y);
        }
    }

    BalanceLayoutSprings(layout);
    BalanceLayoutItemsAlignment(layout);
}

void BeginLayoutItem(WindowStackLayoutState& state, StackLayout& layout)
{
    ImGuiContext& g = *GImGui;
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    StackLayoutItem& item = *GenerateLayoutItem(state, layout, StackLayoutItemType::Item);

    item.CurrentAlign = layout.Align;
    if (item.CurrentAlign < 0.0f)
        item.CurrentAlign = DEFAULT_LAYOUT_ALIGN;

    item.CurrentAlignOffset = CalculateLayoutItemAlignmentOffset(layout, item);
    if (item.CurrentAlign > 0.0f)
    {
        if (layout.Type == ImGuiLayoutType_Horizontal)
        {
            window->DC.CursorPos.y += item.CurrentAlignOffset;
        }
        else
        {
            float new_position = window->DC.CursorPos.x + item.CurrentAlignOffset;
            SignedIndent(item.CurrentAlignOffset);
            window->DC.CursorPos.x = new_position;
        }
    }

    item.MeasuredBounds.Min = item.MeasuredBounds.Max = window->DC.CursorPos;
    item.VertexIndexBegin = item.VertexIndexEnd = window->DrawList->_VtxCurrentIdx;
}

void MeasureCurrentLayoutItem(ImGuiWindow* window, const ImVec2& bb_max)
{
    WindowStackLayoutState* state = FindFastActiveLayoutState(window);
    if (!state || !state->CurrentLayoutItem)
        return;

    state->CurrentLayoutItem->MeasuredBounds.Max =
        ImMax(state->CurrentLayoutItem->MeasuredBounds.Max, bb_max);
}

void EndLayoutItem(StackLayout& layout)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    IM_ASSERT(layout.CurrentItemIndex < static_cast<int>(layout.Items.size()));

    StackLayoutItem& item = layout.Items[layout.CurrentItemIndex];
    item.VertexIndexEnd = window->DrawList->_VtxCurrentIdx;

    if (item.CurrentAlign > 0.0f && layout.Type == ImGuiLayoutType_Vertical)
        SignedIndent(-item.CurrentAlignOffset);

    ImVec2 position_correction = BalanceLayoutItemAlignment(layout, item);
    item.MeasuredBounds.Min += position_correction;
    item.MeasuredBounds.Max += position_correction;

    if (layout.Type == ImGuiLayoutType_Horizontal)
        window->DC.CursorPos.y = layout.StartPos.y;
    else
        window->DC.CursorPos.x = layout.StartPos.x;

    layout.CurrentItemIndex++;
}

void AddLayoutSpring(WindowStackLayoutState& state, StackLayout& layout, float weight, float spacing)
{
    ImGuiContext& g = *GImGui;
    ImGuiWindow* window = g.CurrentWindow;
    StackLayoutItem* previous_item = &layout.Items[layout.CurrentItemIndex];

    if (layout.Type == ImGuiLayoutType_Horizontal)
        window->DC.CursorPos.x = previous_item->MeasuredBounds.Max.x;
    else
        window->DC.CursorPos.y = previous_item->MeasuredBounds.Max.y;

    EndLayoutItem(layout);

    StackLayoutItem* spring_item = GenerateLayoutItem(state, layout, StackLayoutItemType::Spring);
    spring_item->MeasuredBounds.Min = spring_item->MeasuredBounds.Max = window->DC.CursorPos;
    spring_item->VertexIndexBegin = spring_item->VertexIndexEnd = window->DrawList->_VtxCurrentIdx;

    if (weight < 0.0f)
        weight = 0.0f;
    spring_item->SpringWeight = weight;

    if (spacing < 0.0f)
        spacing = layout.Type == ImGuiLayoutType_Horizontal ? g.Style.ItemSpacing.x : g.Style.ItemSpacing.y;
    spring_item->SpringSpacing = spacing;

    if (spring_item->SpringSize > 0.0f || spacing > 0.0f)
    {
        ImVec2 spring_size;
        ImVec2 spring_spacing;
        if (layout.Type == ImGuiLayoutType_Horizontal)
        {
            spring_spacing = ImVec2(0.0f, g.Style.ItemSpacing.y);
            spring_size = ImVec2(spacing + spring_item->SpringSize, layout.CurrentSize.y);
        }
        else
        {
            spring_spacing = ImVec2(g.Style.ItemSpacing.x, 0.0f);
            spring_size = ImVec2(layout.CurrentSize.x, spacing + spring_item->SpringSize);
        }

        ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImFloor(spring_spacing));
        ImGui::Dummy(ImFloor(spring_size));
        ImGui::PopStyleVar();
        spring_item->MeasuredBounds.Max = ImMax(spring_item->MeasuredBounds.Max, g.LastItemData.Rect.Max);
        spring_item->VertexIndexEnd = window->DrawList->_VtxCurrentIdx;
    }

    layout.CurrentItemIndex++;
    BeginLayoutItem(state, layout);
}

void BeginLayout(ImGuiID id, ImGuiLayoutType type, ImVec2 size, float align)
{
    WindowStackLayoutState& state = GetWindowState();
    ImGuiWindow* window = ImGui::GetCurrentWindow();

    ImGui::PushID(reinterpret_cast<const void*>(static_cast<uintptr_t>(id)));

    StackLayout* layout = FindLayout(state, id, type);
    if (!layout)
        layout = CreateNewLayout(state, id, type, size);

    layout->Live = true;
    PushLayout(state, layout);

    if (layout->Size.x != size.x || layout->Size.y != size.y)
        layout->Size = size;

    layout->Align = align < 0.0f ? -1.0f : ImClamp(align, 0.0f, 1.0f);
    layout->CurrentItemIndex = 0;
    layout->CurrentSize.x = layout->Size.x > 0.0f ? layout->Size.x : layout->MinimumSize.x;
    layout->CurrentSize.y = layout->Size.y > 0.0f ? layout->Size.y : layout->MinimumSize.y;
    layout->StartPos = window->DC.CursorPos;
    layout->StartCursorMaxPos = window->DC.CursorMaxPos;

    if (type == ImGuiLayoutType_Vertical)
    {
        ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImVec2(0.0f, 0.0f));
        ImGui::Dummy(ImVec2(0.0f, 0.0f));
        ImGui::PopStyleVar();

        layout->Indent = layout->StartPos.x - window->DC.CursorPos.x;
        SignedIndent(layout->Indent);
    }

    BeginLayoutItem(state, *layout);
}

void EndLayout(ImGuiLayoutType type)
{
    WindowStackLayoutState& state = GetWindowState();
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    IM_ASSERT(state.CurrentLayout != nullptr);
    IM_ASSERT(state.CurrentLayout->Type == type);

    StackLayout* layout = state.CurrentLayout;
    EndLayoutItem(*layout);

    if (layout->CurrentItemIndex < static_cast<int>(layout->Items.size()))
        layout->Items.erase(layout->Items.begin() + layout->CurrentItemIndex, layout->Items.end());

    if (layout->Type == ImGuiLayoutType_Vertical)
        SignedIndent(-layout->Indent);

    PopLayout(state, layout);

    const bool auto_width = layout->Size.x <= 0.0f;
    const bool auto_height = layout->Size.y <= 0.0f;

    ImVec2 new_size = layout->Size;
    if (auto_width)
        new_size.x = layout->CurrentSize.x;
    if (auto_height)
        new_size.y = layout->CurrentSize.y;

    ImVec2 new_minimum_size = CalculateLayoutSize(*layout, true);
    if (new_minimum_size.x != layout->MinimumSize.x || new_minimum_size.y != layout->MinimumSize.y)
    {
        layout->MinimumSize = new_minimum_size;
        if (auto_width)
            new_size.x = new_minimum_size.x;
        if (auto_height)
            new_size.y = new_minimum_size.y;
    }

    if (!auto_width)
        new_size.x = layout->Size.x;
    if (!auto_height)
        new_size.y = layout->Size.y;

    layout->CurrentSize = new_size;

    ImVec2 measured_size = new_size;
    if ((auto_width || auto_height) && layout->Parent)
    {
        if (layout->Type == ImGuiLayoutType_Horizontal && auto_width && layout->Parent->CurrentSize.x > 0.0f)
            layout->CurrentSize.x = layout->Parent->CurrentSize.x;
        else if (layout->Type == ImGuiLayoutType_Vertical && auto_height && layout->Parent->CurrentSize.y > 0.0f)
            layout->CurrentSize.y = layout->Parent->CurrentSize.y;

        BalanceLayoutSprings(*layout);
        measured_size = layout->CurrentSize;
    }

    layout->CurrentSize = new_size;

    ImGui::PopID();

    ImVec2 current_layout_item_max(0.0f, 0.0f);
    if (state.CurrentLayoutItem)
        current_layout_item_max = ImMax(state.CurrentLayoutItem->MeasuredBounds.Max, layout->StartPos + new_size);

    window->DC.CursorPos = layout->StartPos;
    window->DC.CursorMaxPos = layout->StartCursorMaxPos;
    ImGui::ItemSize(new_size);
    ImGui::ItemAdd(ImRect(layout->StartPos, layout->StartPos + measured_size), 0, nullptr, ImGuiItemFlags_NoTabStop);

    if (state.CurrentLayoutItem)
        state.CurrentLayoutItem->MeasuredBounds.Max = current_layout_item_max;

    if (layout->Parent == nullptr)
        BalanceChildLayouts(*layout);
}
}

extern "C"
{
void dear_imgui_stack_begin_horizontal_str(const char* str_id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(str_id), ImGuiLayoutType_Horizontal, size, align);
}

void dear_imgui_stack_begin_horizontal_ptr(const void* ptr_id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(ptr_id), ImGuiLayoutType_Horizontal, size, align);
}

void dear_imgui_stack_begin_horizontal_int(int id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(reinterpret_cast<void*>(static_cast<intptr_t>(id))), ImGuiLayoutType_Horizontal, size, align);
}

void dear_imgui_stack_begin_horizontal_id(ImGuiID id, ImVec2 size, float align)
{
    BeginLayout(id, ImGuiLayoutType_Horizontal, size, align);
}

void dear_imgui_stack_end_horizontal()
{
    EndLayout(ImGuiLayoutType_Horizontal);
}

void dear_imgui_stack_begin_vertical_str(const char* str_id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(str_id), ImGuiLayoutType_Vertical, size, align);
}

void dear_imgui_stack_begin_vertical_ptr(const void* ptr_id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(ptr_id), ImGuiLayoutType_Vertical, size, align);
}

void dear_imgui_stack_begin_vertical_int(int id, ImVec2 size, float align)
{
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    BeginLayout(window->GetID(reinterpret_cast<void*>(static_cast<intptr_t>(id))), ImGuiLayoutType_Vertical, size, align);
}

void dear_imgui_stack_begin_vertical_id(ImGuiID id, ImVec2 size, float align)
{
    BeginLayout(id, ImGuiLayoutType_Vertical, size, align);
}

void dear_imgui_stack_end_vertical()
{
    EndLayout(ImGuiLayoutType_Vertical);
}

void dear_imgui_stack_spring(float weight, float spacing)
{
    WindowStackLayoutState& state = GetWindowState();
    IM_ASSERT(state.CurrentLayout != nullptr);
    AddLayoutSpring(state, *state.CurrentLayout, weight, spacing);
}

void dear_imgui_stack_item_add(ImGuiWindow* window, ImVec2 bb_max)
{
    MeasureCurrentLayoutItem(window, bb_max);
}

bool dear_imgui_stack_current_layout_type(ImGuiWindow* window, int* layout_type)
{
    WindowStackLayoutState* state = FindFastActiveLayoutState(window);
    if (!state || !state->CurrentLayout)
        return false;

    if (layout_type)
        *layout_type = static_cast<int>(state->CurrentLayout->Type);
    return true;
}

void dear_imgui_stack_suspend_layout()
{
    WindowStackLayoutState& state = GetWindowState();
    PushLayout(state, nullptr);
}

void dear_imgui_stack_resume_layout()
{
    WindowStackLayoutState& state = GetWindowState();
    IM_ASSERT(state.CurrentLayout == nullptr);
    IM_ASSERT(!state.LayoutStack.empty());
    PopLayout(state, nullptr);
}
}
