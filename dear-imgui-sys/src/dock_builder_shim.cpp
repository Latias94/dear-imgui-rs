#include "imgui.h"
#include "imgui_internal.h"

#include <climits>

namespace
{
bool HasActiveContentWindow(const ImGuiContext* context, const ImGuiDockNode* root)
{
    for (const ImGuiWindow* window : context->Windows)
    {
        if (window == nullptr || window->LastFrameActive != context->FrameCount || window->DockNode == nullptr)
            continue;

        const ImGuiDockNode* window_root = window->DockNode;
        while (window_root->ParentNode != nullptr)
            window_root = window_root->ParentNode;
        if (window_root == root)
            return true;
    }

    return false;
}
}

extern "C"
{
int dear_imgui_rs_dock_builder_keep_root_alive(ImGuiID root_id)
{
    ImGuiContext* context = ImGui::GetCurrentContext();
    if (context == nullptr || !context->WithinFrameScope || root_id == 0)
        return 0;

    ImGuiDockNode* root = ImGui::DockBuilderGetNode(root_id);
    if (root == nullptr || !root->IsRootNode())
        return 0;

    root->LastFrameAlive = context->FrameCount;
    return 1;
}

int dear_imgui_rs_dock_builder_root_has_active_content_window(ImGuiID root_id)
{
    ImGuiContext* context = ImGui::GetCurrentContext();
    if (context == nullptr || !context->WithinFrameScope || root_id == 0)
        return 0;

    const ImGuiDockNode* root = ImGui::DockBuilderGetNode(root_id);
    if (root == nullptr || !root->IsRootNode())
        return 0;

    return HasActiveContentWindow(context, root) ? 1 : 0;
}

int dear_imgui_rs_dock_builder_copy_node(
    ImGuiID source_root_id,
    ImGuiID destination_root_id,
    ImGuiID* remap_data,
    int remap_capacity)
{
    ImGuiContext* context = ImGui::GetCurrentContext();
    if (context == nullptr || !context->WithinFrameScope || source_root_id == 0 ||
        destination_root_id == 0 || source_root_id == destination_root_id)
        return -1;

    const ImGuiDockNode* source_root = ImGui::DockBuilderGetNode(source_root_id);
    if (source_root == nullptr || !source_root->IsRootNode())
        return -1;

    int node_count = 0;
    ImVector<const ImGuiDockNode*> pending;
    pending.push_back(source_root);
    while (!pending.empty())
    {
        const ImGuiDockNode* node = pending.back();
        pending.pop_back();
        if (node_count == INT_MAX / 2)
            return -2;
        node_count++;
        if (node->ChildNodes[0] != nullptr)
            pending.push_back(node->ChildNodes[0]);
        if (node->ChildNodes[1] != nullptr)
            pending.push_back(node->ChildNodes[1]);
    }

    const int required_capacity = node_count * 2;
    if (remap_data == nullptr || remap_capacity != required_capacity)
        return -2;

    ImVector<ImGuiID> remap;
    ImGui::DockBuilderCopyNode(source_root_id, destination_root_id, &remap);
    if (remap.Size != required_capacity)
        return -3;
    for (int index = 0; index < remap.Size; index++)
        remap_data[index] = remap[index];
    return remap.Size;
}
}
