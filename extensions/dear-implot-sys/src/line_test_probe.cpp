#include "imgui.h"
#include "imgui_internal.h"
#include "implot.h"

// Exercise the production PlotLine renderer, rather than a second copy of its sampler helper.
extern "C" int dear_implot_line_probe_plot(
    ImPlotContext* plot_context,
    float weight,
    float pixel_density,
    bool anti_aliased,
    float* positions,
    float* uvs,
    float* expected_uvs,
    float* expected_width) {
    if (plot_context == nullptr || positions == nullptr || uvs == nullptr ||
        expected_uvs == nullptr || expected_width == nullptr || weight <= 0.0f ||
        pixel_density <= 0.0f) {
        return -1;
    }

    struct RestoreState {
        ImPlotContext* previous_plot_context;
        ImDrawList* owner;
        ImDrawFlags flags;
        float pixel_density;
        ~RestoreState() {
            owner->Flags = flags;
            owner->_SetPixelDensity(pixel_density);
            ImPlot::SetCurrentContext(previous_plot_context);
        }
    } restore{ImPlot::GetCurrentContext(), ImGui::GetWindowDrawList(),
              ImGui::GetWindowDrawList()->Flags, ImGui::GetWindowDrawList()->_InvFringeScale};

    ImPlot::SetCurrentContext(plot_context);
    ImDrawList* owner = restore.owner;
    owner->_SetPixelDensity(pixel_density);
    owner->Flags &= ~(ImDrawFlags_AALines | ImDrawFlags_AALineEnds);
    if (anti_aliased)
        owner->Flags |= ImDrawFlags_AALines;

    if (!ImPlot::BeginPlot("line texture regression", ImVec2(400.0f, 300.0f), ImPlotFlags_CanvasOnly))
        return -2;
    ImPlot::SetupAxesLimits(0.0, 1.0, 0.0, 1.0, ImPlotCond_Always);
    ImPlot::SetupFinish();
    ImDrawList& draw_list = *ImPlot::GetPlotDrawList();
    const int start = draw_list.VtxBuffer.Size;

    const float xs[] = {0.2f, 0.8f};
    const float ys[] = {0.3f, 0.7f};
    ImPlotSpec spec;
    spec.LineWeight = weight;
    spec.LineColor = ImVec4(0.13f, 0.47f, 0.79f, 1.0f);
    spec.Marker = ImPlotMarker_None;
    ImPlot::PlotLine("probe segment", xs, ys, 2, spec);
    const int count = draw_list.VtxBuffer.Size - start;
    if (count == 4) {
        for (int index = 0; index < 4; ++index) {
            const ImDrawVert& vertex = draw_list.VtxBuffer[start + index];
            positions[index * 2] = vertex.pos.x;
            positions[index * 2 + 1] = vertex.pos.y;
            uvs[index * 2] = vertex.uv.x;
            uvs[index * 2 + 1] = vertex.uv.y;
        }
    }

    // ImGui owns the atlas packing, texture selection, and fringe contract.
    const float thickness = ImMax(ImMax(1.0f, weight), owner->_FringeScale);
    ImVec2 uv0, uv1;
    float fringe = 0.0f;
    owner->_SelectLineTexture(ImMax(1.0f, thickness * owner->_InvFringeScale),
                              &uv0, &uv1, &fringe, owner->Flags);
    expected_uvs[0] = uv0.x;
    expected_uvs[1] = uv0.y;
    expected_uvs[2] = uv1.x;
    expected_uvs[3] = uv1.y;
    *expected_width = thickness + fringe;
    ImPlot::EndPlot();
    return count;
}


namespace MyImPlot {
void PlotCandlestick(const char*, const double*, const double*, const double*, const double*,
                     const double*, int, bool, float, ImVec4, ImVec4);
}

extern "C" int dear_implot_rectangle_probe_plot(
    ImPlotContext* plot_context, int kind, bool invert_x, bool invert_y, float* bounds) {
    if (plot_context == nullptr || bounds == nullptr)
        return -1;
    ImPlotContext* previous = ImPlot::GetCurrentContext();
    ImPlot::SetCurrentContext(plot_context);
    if (!ImPlot::BeginPlot("rectangle contract", ImVec2(400.0f, 300.0f), ImPlotFlags_CanvasOnly)) {
        ImPlot::SetCurrentContext(previous);
        return -2;
    }
    ImPlot::SetupAxes(nullptr, nullptr, invert_x ? ImPlotAxisFlags_Invert : 0,
                      invert_y ? ImPlotAxisFlags_Invert : 0);
    ImPlot::SetupAxesLimits(0.0, 1.0, 0.0, 1.0, ImPlotCond_Always);
    ImPlot::SetupFinish();
    ImDrawList& draw_list = *ImPlot::GetPlotDrawList();
    const int start = draw_list.VtxBuffer.Size;
    if (kind == 0) {
        const float xs[] = {0.1f, 0.3f, 0.5f, 0.7f, 0.9f};
        const float ys[] = {0.0f, 1.0f, 1.0f, 0.0f, 1.0f};
        ImPlotSpec spec;
        spec.FillColor = ImVec4(0.13f, 0.47f, 0.79f, 1.0f);
        ImPlot::PlotDigital("digital", xs, ys, 5, spec);
    } else if (kind == 1) {
        const float values[] = {7.0f, 7.0f, 7.0f, 7.0f};
        ImPlot::PlotHeatmap("constant heatmap", values, 2, 2, 0.0, 0.0, nullptr,
                            ImPlotPoint(0.2, 0.3), ImPlotPoint(0.8, 0.7));
    } else {
        const double xs[] = {0.3, 0.7};
        const double opens[] = {0.3, 0.7};
        const double closes[] = {0.7, 0.3};
        const double lows[] = {0.2, 0.2};
        const double highs[] = {0.8, 0.8};
        MyImPlot::PlotCandlestick("candles", xs, opens, closes, lows, highs, 2, false,
                                 0.15f, ImVec4(0, 1, 0, 1), ImVec4(1, 0, 0, 1));
    }
    const int count = draw_list.VtxBuffer.Size - start;
    bounds[0] = bounds[1] = FLT_MAX;
    bounds[2] = bounds[3] = -FLT_MAX;
    for (int index = start; index < draw_list.VtxBuffer.Size; ++index) {
        const ImVec2& pos = draw_list.VtxBuffer[index].pos;
        bounds[0] = ImMin(bounds[0], pos.x);
        bounds[1] = ImMin(bounds[1], pos.y);
        bounds[2] = ImMax(bounds[2], pos.x);
        bounds[3] = ImMax(bounds[3], pos.y);
    }
    if (kind == 2) {
        // Both bullish and bearish candle bodies must produce their distinct opaque color.
        bool green_body = false;
        bool red_body = false;
        int green_count = 0;
        int red_count = 0;
        for (int index = start; index < draw_list.VtxBuffer.Size; ++index) {
            const ImDrawVert& vertex = draw_list.VtxBuffer[index];
            if (vertex.col == IM_COL32(0, 255, 0, 255)) ++green_count;
            if (vertex.col == IM_COL32(255, 0, 0, 255)) ++red_count;
        }
        green_body = green_count >= 8;
        red_body = red_count >= 8;
        const ImVec2 plot_min = ImPlot::GetPlotPos();
        const ImVec2 plot_max = plot_min + ImPlot::GetPlotSize();
        if (!green_body || !red_body || bounds[0] < plot_min.x - 2.0f ||
            bounds[1] < plot_min.y - 2.0f || bounds[2] > plot_max.x + 2.0f ||
            bounds[3] > plot_max.y + 2.0f) {
            ImPlot::EndPlot();
            ImPlot::SetCurrentContext(previous);
            return -3;
        }
    }
    ImPlot::EndPlot();
    ImPlot::SetCurrentContext(previous);
    return count;
}
