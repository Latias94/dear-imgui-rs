#include "imgui.h"
#include "imgui_internal.h"

void DearImGuiRsShowDemoWindowWithoutFontAtlas(bool* p_open);
void DearImGuiRsShowMetricsWindowWithoutFontAtlas(bool* p_open);
void DearImGuiRsShowStyleEditorWithoutFontAtlas(ImGuiStyle* ref);

extern "C"
{
void dear_imgui_rs_show_demo_window_without_font_atlas(bool* p_open)
{
    DearImGuiRsShowDemoWindowWithoutFontAtlas(p_open);
}

void dear_imgui_rs_show_metrics_window_without_font_atlas(bool* p_open)
{
    DearImGuiRsShowMetricsWindowWithoutFontAtlas(p_open);
}

void dear_imgui_rs_show_style_editor_without_font_atlas(ImGuiStyle* ref)
{
    DearImGuiRsShowStyleEditorWithoutFontAtlas(ref);
}

void dear_imgui_rs_show_font_atlas_debug_panel()
{
    ImGui::ShowFontAtlas(ImGui::GetIO().Fonts);
}
}
