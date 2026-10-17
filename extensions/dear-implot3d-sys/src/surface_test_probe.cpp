#include "imgui.h"
#include "imgui_internal.h"
#include "implot3d.h"
#include "implot3d-imgui-193-internal.h"
#include "cimplot3d.h"

namespace {
constexpr int kCapacity = 16;

struct SurfaceCapture {
    float xs[kCapacity]{};
    float ys[kCapacity]{};
    float zs[kCapacity]{};
    int x_count = 0;
    int y_count = 0;
    int point_count = 0;
};

thread_local SurfaceCapture capture;

void record_surface(
    const float* xs,
    const float* ys,
    const float* zs,
    int x_count,
    int y_count) {
    capture = SurfaceCapture{};
    if (xs == nullptr || ys == nullptr || zs == nullptr || x_count <= 0 || y_count <= 0) {
        return;
    }
    const long long point_count =
        static_cast<long long>(x_count) * static_cast<long long>(y_count);
    if (point_count <= 0 || point_count > kCapacity) {
        return;
    }

    capture.x_count = x_count;
    capture.y_count = y_count;
    capture.point_count = static_cast<int>(point_count);
    for (int index = 0; index < capture.point_count; ++index) {
        capture.xs[index] = xs[index];
        capture.ys[index] = ys[index];
        capture.zs[index] = zs[index];
    }
}
} // namespace

extern "C" void dear_implot3d_surface_probe_reset() {
    capture = SurfaceCapture{};
}

extern "C" void dear_implot3d_surface_probe_plot(
    const char* label_id,
    const float* xs,
    const float* ys,
    const float* zs,
    int x_count,
    int y_count,
    double scale_min,
    double scale_max,
    const ImPlot3DSpec_c spec) {
    record_surface(xs, ys, zs, x_count, y_count);
    ImPlot3D_PlotSurface_FloatPtr(
        label_id,
        xs,
        ys,
        zs,
        x_count,
        y_count,
        scale_min,
        scale_max,
        spec);
}

extern "C" int dear_implot3d_surface_probe_read(
    float* xs,
    float* ys,
    float* zs,
    int capacity,
    int* x_count,
    int* y_count) {
    if (xs == nullptr || ys == nullptr || zs == nullptr || x_count == nullptr ||
        y_count == nullptr || capacity < capture.point_count) {
        return -1;
    }
    *x_count = capture.x_count;
    *y_count = capture.y_count;
    for (int index = 0; index < capture.point_count; ++index) {
        xs[index] = capture.xs[index];
        ys[index] = capture.ys[index];
        zs[index] = capture.zs[index];
    }
    return capture.point_count;
}


// Exercise the production PlotLine renderer, rather than a second copy of its sampler helper.
extern "C" int dear_implot3d_line_probe_plot(
    ImPlot3DContext* plot_context,
    float weight,
    float pixel_density,
    bool anti_aliased,
    float alpha,
    float* positions,
    float* uvs,
    float* expected_uvs,
    float* expected_width,
    ImU32* colors,
    ImU32* expected_color) {
    if (plot_context == nullptr || positions == nullptr || uvs == nullptr ||
        expected_uvs == nullptr || expected_width == nullptr || colors == nullptr ||
        expected_color == nullptr || weight <= 0.0f ||
        pixel_density <= 0.0f) {
        return -1;
    }

    struct RestoreState {
        ImPlot3DContext* previous_plot_context;
        ImDrawList* owner;
        ImDrawFlags flags;
        float pixel_density;
        ~RestoreState() {
            owner->Flags = flags;
            owner->_SetPixelDensity(pixel_density);
            ImPlot3D::SetCurrentContext(previous_plot_context);
        }
    } restore{ImPlot3D::GetCurrentContext(), ImGui::GetWindowDrawList(),
              ImGui::GetWindowDrawList()->Flags, ImGui::GetWindowDrawList()->_InvFringeScale};

    ImPlot3D::SetCurrentContext(plot_context);
    ImDrawList* owner = restore.owner;
    owner->_SetPixelDensity(pixel_density);
    owner->Flags &= ~(ImDrawFlags_AALines | ImDrawFlags_AALineEnds);
    if (anti_aliased)
        owner->Flags |= ImDrawFlags_AALines;

    if (!ImPlot3D::BeginPlot("line texture regression", ImVec2(400.0f, 300.0f), ImPlot3DFlags_CanvasOnly))
        return -2;
    ImPlot3D::SetupAxesLimits(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, ImPlot3DCond_Always);
    ImPlot3D::SetupLock();
    ImDrawList3D& draw_list = ImPlot3D::GetCurrentPlot()->DrawList;
    const int start = draw_list.VtxBuffer.Size;

    const float xs[] = {0.2f, 0.8f};
    const float ys[] = {0.3f, 0.7f};
    const float zs[] = {0.4f, 0.6f};
    ImPlot3DSpec spec;
    spec.LineWeight = weight;
    spec.LineColor = ImVec4(0.13f, 0.47f, 0.79f, alpha);
    spec.Marker = ImPlot3DMarker_None;
    ImPlot3D::PlotLine("probe segment", xs, ys, zs, 2, spec);
    const int count = draw_list.VtxBuffer.Size - start;
    if (count == 4) {
        for (int index = 0; index < 4; ++index) {
            const ImDrawVert& vertex = draw_list.VtxBuffer[start + index];
            colors[index] = vertex.col;
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
    ImPlot3D::EndPlot();
    // Compare color coverage with the actual core primitive, including its alpha rounding.
    const int reference_start = owner->VtxBuffer.Size;
    owner->_AddLine(ImVec2(10, 10), ImVec2(110, 10), ImGui::ColorConvertFloat4ToU32(spec.LineColor),
                    ImMax(1.0f, weight), ImDrawFlags_None);
    *expected_color = owner->VtxBuffer[reference_start].col;
    return count;
}


extern "C" int dear_implot3d_legend_probe_frame(
    ImPlot3DContext* plot_context, bool covered, float* state) {
    if (plot_context == nullptr || state == nullptr)
        return -1;
    ImPlot3DContext* previous = ImPlot3D::GetCurrentContext();
    ImPlot3D::SetCurrentContext(plot_context);
    const ImGuiWindowFlags window_flags = ImGuiWindowFlags_NoSavedSettings |
        ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoScrollWithMouse;
    ImGui::SetNextWindowPos(ImVec2(20.0f, 20.0f), ImGuiCond_Always);
    ImGui::SetNextWindowSize(ImVec2(520.0f, 440.0f), ImGuiCond_Always);
    ImGui::Begin("legend background window", nullptr, window_flags);
    int result = -2;
    if (ImPlot3D::BeginPlot("overflowing legend", ImVec2(400.0f, 300.0f),
                            ImPlot3DFlags_NoTitle | ImPlot3DFlags_NoMenus | ImPlot3DFlags_NoMouseText)) {
        ImPlot3D::SetupLegend(ImPlot3DLocation_NorthWest);
        for (int index = 0; index < 40; ++index) {
            char label[32];
            ImFormatString(label, IM_ARRAYSIZE(label), "Legend entry %02d", index);
            ImPlot3D::PlotDummy(label);
        }
        ImPlot3DPlot* plot = ImPlot3D::GetCurrentPlot();
        const ImGuiID legend_id = plot->Items.ID;
        ImPlot3D::EndPlot();
        const ImPlot3DLegend& legend = plot->Items.Legend;
        const ImVec2 center = legend.RectClamped.GetCenter();
        state[0] = center.x;
        state[1] = center.y;
        state[2] = legend.Scroll.y;
        state[3] = legend.Rect.GetHeight() - legend.RectClamped.GetHeight();
        result = (legend.Hovered ? 1 : 0) |
                 (ImGui::GetKeyOwner(ImGuiKey_MouseWheelY) == legend_id ? 2 : 0);
    }
    ImGui::End();
    if (covered) {
        ImGui::SetNextWindowPos(ImVec2(20.0f, 20.0f), ImGuiCond_Always);
        ImGui::SetNextWindowSize(ImVec2(520.0f, 440.0f), ImGuiCond_Always);
        ImGui::Begin("legend covering window", nullptr, window_flags);
        ImGui::TextUnformatted("This foreground window owns mouse hover.");
        ImGui::End();
    }
    ImPlot3D::SetCurrentContext(previous);
    return result;
}
