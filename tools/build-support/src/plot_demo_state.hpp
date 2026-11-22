// Included after the plot internal header in the staged core and demo sources.
#ifndef IMGUI_DISABLE
#include <map>
#include <optional>

extern "C" {
void dear_imgui_rs_show_style_editor_without_font_atlas(ImGuiStyle*);
void dear_imgui_rs_show_metrics_window_without_font_atlas(bool*);
void dear_imgui_rs_show_demo_window_without_font_atlas(bool*);
}

namespace DearImGuiRs {

// Plot context layouts are part of the C bindings. Keep tool state separately and
// release it in the native DestroyContext hook, including for non-current contexts.
template<typename Context, typename State>
class PlotToolStorage {
    using States = std::map<Context*, State>;
    inline static States* states = nullptr;

public:
    static State& Get(Context* ctx) {
        if (states == nullptr)
            states = new States;
        return (*states)[ctx];
    }

    static void Forget(Context* ctx) {
        if (states != nullptr) {
            states->erase(ctx);
            if (states->empty()) {
                delete states;
                states = nullptr;
            }
        }
    }
};

#ifdef IMPLOT_VERSION
struct ImPlotToolState {
    std::optional<ImPlotStyle> reference;
    ImPlotColormap heatmap_colormap = ImPlotColormap_Viridis;
    ImPlotColormap widget_colormap = ImPlotColormap_Viridis;
};
using ImPlotTools = PlotToolStorage<ImPlotContext, ImPlotToolState>;
#endif

#ifdef IMPLOT3D_VERSION
struct ImPlot3DToolState {
    std::optional<ImPlot3DStyle> reference;
    ImPlot3DCol flash_color_idx = ImPlot3DCol_COUNT;
    float flash_color_time = 0.5f;
    ImVec4 flash_color_backup;
    ImPlot3DColormap marker_colormap = ImPlot3DColormap_Viridis;
    int surface_colormap = 5; // Jet in DemoSurfacePlots' list of names
};
using ImPlot3DTools = PlotToolStorage<ImPlot3DContext, ImPlot3DToolState>;
#endif

} // namespace DearImGuiRs
#endif // #ifndef IMGUI_DISABLE
