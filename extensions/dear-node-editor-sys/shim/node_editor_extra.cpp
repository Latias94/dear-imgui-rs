#include "node_editor_extra.h"

#include "imgui.h"
#include "imgui_node_editor.h"

#include <cmath>
#include <cstdlib>
#include <memory>
#include <vector>

#if defined(__cpp_exceptions) || defined(_CPPUNWIND)
#define DNE_TRY try
#define DNE_CATCH(...) catch (...) { return __VA_ARGS__; }
#define DNE_CATCH_ABORT catch (...) { std::abort(); }
#else
#define DNE_TRY
#define DNE_CATCH(...)
#define DNE_CATCH_ABORT
#endif

#define DNE_REQUIRE_EDITOR(...) if (!ed::GetCurrentEditor()) return __VA_ARGS__

namespace ed = ax::NodeEditor;

struct DneCallbackBridge {
    DneConfigSession begin_save_session = nullptr;
    DneConfigSession end_save_session = nullptr;
    DneConfigSaveSettings save_settings = nullptr;
    DneConfigLoadSettings load_settings = nullptr;
    DneConfigSaveNodeSettings save_node_settings = nullptr;
    DneConfigLoadNodeSettings load_node_settings = nullptr;
    void* user_pointer = nullptr;
};

struct DneEditorContext {
    std::unique_ptr<DneCallbackBridge> bridge;
    ed::EditorContext* editor = nullptr;
};

static ed::NodeId node_id(uintptr_t value) { return ed::NodeId(value); }
static ed::PinId pin_id(uintptr_t value) { return ed::PinId(value); }
static ed::LinkId link_id(uintptr_t value) { return ed::LinkId(value); }

static uintptr_t value(ed::NodeId id) { return id.Get(); }
static uintptr_t value(ed::PinId id) { return id.Get(); }
static uintptr_t value(ed::LinkId id) { return id.Get(); }

static ImVec2 to_imgui(ImVec2_c value) { return ImVec2(value.x, value.y); }
static ImVec4 to_imgui(ImVec4_c value) { return ImVec4(value.x, value.y, value.z, value.w); }
static float to_imgui(float value) { return value; }
static ImVec2_c from_imgui(ImVec2 value) { return ImVec2_c{value.x, value.y}; }
static ImVec4_c from_imgui(ImVec4 value) { return ImVec4_c{value.x, value.y, value.z, value.w}; }
static float from_imgui(float value) { return value; }

static ed::EditorContext* raw_editor(DneEditorContext* ctx)
{
    return ctx ? ctx->editor : nullptr;
}

static ed::PinKind to_pin_kind(DnePinKind kind)
{
    return kind == DNE_PIN_KIND_OUTPUT ? ed::PinKind::Output : ed::PinKind::Input;
}

static ed::FlowDirection to_flow_direction(DneFlowDirection direction)
{
    return direction == DNE_FLOW_BACKWARD ? ed::FlowDirection::Backward : ed::FlowDirection::Forward;
}

static ed::CanvasSizeMode to_canvas_size_mode(DneCanvasSizeMode mode)
{
    switch (mode)
    {
    case DNE_CANVAS_SIZE_FIT_HORIZONTAL_VIEW:
        return ed::CanvasSizeMode::FitHorizontalView;
    case DNE_CANVAS_SIZE_CENTER_ONLY:
        return ed::CanvasSizeMode::CenterOnly;
    case DNE_CANVAS_SIZE_FIT_VERTICAL_VIEW:
    default:
        return ed::CanvasSizeMode::FitVerticalView;
    }
}

static ed::StyleColor to_style_color(DneStyleColor color)
{
    return static_cast<ed::StyleColor>(static_cast<int>(color));
}

static bool valid_style_color(DneStyleColor color)
{
    return static_cast<unsigned>(color) < static_cast<unsigned>(ed::StyleColor_Count);
}

static ed::StyleVar to_style_var(DneStyleVar var)
{
    return static_cast<ed::StyleVar>(static_cast<int>(var));
}

static bool valid_style_var(DneStyleVar var)
{
    return static_cast<unsigned>(var) < static_cast<unsigned>(ed::StyleVar_Count);
}

static bool valid_mouse_button(int button)
{
    return button >= 0 && button < ImGuiMouseButton_COUNT;
}

static bool valid_config(const DneConfig& config)
{
    if (!valid_mouse_button(config.drag_button_index) ||
        !valid_mouse_button(config.select_button_index) ||
        !valid_mouse_button(config.navigate_button_index) ||
        !valid_mouse_button(config.context_menu_button_index) ||
        (config.enable_smooth_zoom &&
            (!std::isfinite(config.smooth_zoom_power) || config.smooth_zoom_power <= 0.0f)))
        return false;

    const auto count = config.custom_zoom_level_count;
    const auto* levels = config.custom_zoom_levels;
    if (count < 0 || (count > 0 && !levels))
        return false;

    float previous = 0.0f;
    for (int index = 0; index < count; ++index)
    {
        const auto level = levels[index];
        if (!std::isfinite(level) || level <= previous)
            return false;
        previous = level;
    }
    return true;
}

static DneSaveReasonFlags from_save_reason(ed::SaveReasonFlags reason)
{
    return static_cast<DneSaveReasonFlags>(static_cast<uint32_t>(reason));
}

static void bridge_begin_save_session(void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (bridge && bridge->begin_save_session)
        bridge->begin_save_session(bridge->user_pointer);
}
DNE_CATCH()

static void bridge_end_save_session(void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (bridge && bridge->end_save_session)
        bridge->end_save_session(bridge->user_pointer);
}
DNE_CATCH()

static bool bridge_save_settings(const char* data, size_t size, ed::SaveReasonFlags reason, void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->save_settings)
        return false;
    return bridge->save_settings(data, size, from_save_reason(reason), bridge->user_pointer);
}
DNE_CATCH(false)

static size_t bridge_load_settings(char* data, void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->load_settings)
        return 0;
    return bridge->load_settings(data, bridge->user_pointer);
}
DNE_CATCH(0)

static bool bridge_save_node_settings(ed::NodeId node, const char* data, size_t size, ed::SaveReasonFlags reason, void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->save_node_settings)
        return false;
    return bridge->save_node_settings(value(node), data, size, from_save_reason(reason), bridge->user_pointer);
}
DNE_CATCH(false)

static size_t bridge_load_node_settings(ed::NodeId node, char* data, void* user_pointer) DNE_TRY
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->load_node_settings)
        return 0;
    return bridge->load_node_settings(value(node), data, bridge->user_pointer);
}
DNE_CATCH(0)

static void copy_config(ed::Config& out, std::unique_ptr<DneCallbackBridge>& bridge, const DneConfig* config)
{
    if (!config)
    {
        out.SettingsFile = nullptr;
        return;
    }

    out.SettingsFile = config->settings_file;
    out.CanvasSizeMode = to_canvas_size_mode(config->canvas_size_mode);
    out.DragButtonIndex = config->drag_button_index;
    out.SelectButtonIndex = config->select_button_index;
    out.NavigateButtonIndex = config->navigate_button_index;
    out.ContextMenuButtonIndex = config->context_menu_button_index;
    out.EnableSmoothZoom = config->enable_smooth_zoom;
    out.SmoothZoomPower = config->smooth_zoom_power;
    if (config->custom_zoom_levels && config->custom_zoom_level_count > 0)
    {
        out.CustomZoomLevels.reserve(config->custom_zoom_level_count);
        for (int i = 0; i < config->custom_zoom_level_count; ++i)
            out.CustomZoomLevels.push_back(config->custom_zoom_levels[i]);
    }

    if (config->begin_save_session || config->end_save_session || config->save_settings ||
        config->load_settings || config->save_node_settings || config->load_node_settings)
    {
        bridge = std::make_unique<DneCallbackBridge>();
        bridge->begin_save_session = config->begin_save_session;
        bridge->end_save_session = config->end_save_session;
        bridge->save_settings = config->save_settings;
        bridge->load_settings = config->load_settings;
        bridge->save_node_settings = config->save_node_settings;
        bridge->load_node_settings = config->load_node_settings;
        bridge->user_pointer = config->user_pointer;

        out.UserPointer = bridge.get();
        out.BeginSaveSession = bridge_begin_save_session;
        out.EndSaveSession = bridge_end_save_session;
        out.SaveSettings = bridge_save_settings;
        out.LoadSettings = bridge_load_settings;
        out.SaveNodeSettings = bridge_save_node_settings;
        out.LoadNodeSettings = bridge_load_node_settings;
    }
}

extern "C" {

CIMGUI_API DneEditorContext* dne_create_editor(const DneConfig* config) DNE_TRY
{
    if (config && !valid_config(*config))
        return nullptr;

    auto handle = std::make_unique<DneEditorContext>();
    ed::Config native_config;
    copy_config(native_config, handle->bridge, config);
    handle->editor = ed::CreateEditor(&native_config);
    return handle->editor ? handle.release() : nullptr;
}
DNE_CATCH(nullptr)

CIMGUI_API void dne_destroy_editor(DneEditorContext* ctx) DNE_TRY
{
    const std::unique_ptr<DneEditorContext> handle(ctx);
    if (auto* raw = raw_editor(handle.get()))
        ed::DestroyEditor(raw);
}
DNE_CATCH_ABORT

CIMGUI_API void* dne_editor_context_raw(DneEditorContext* ctx) DNE_TRY
{
    return raw_editor(ctx);
}
DNE_CATCH(nullptr)

CIMGUI_API void* dne_get_current_editor_raw() DNE_TRY
{
    return ed::GetCurrentEditor();
}
DNE_CATCH(nullptr)

CIMGUI_API void dne_set_current_editor_raw(void* ctx) DNE_TRY
{
    ed::SetCurrentEditor(reinterpret_cast<ed::EditorContext*>(ctx));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_set_current_editor(DneEditorContext* ctx) DNE_TRY
{
    ed::SetCurrentEditor(raw_editor(ctx));
}
DNE_CATCH_ABORT

CIMGUI_API const char* dne_get_style_color_name(DneStyleColor color) DNE_TRY
{
    switch (color)
    {
    case DNE_STYLE_COLOR_BG:
        return "Bg";
    case DNE_STYLE_COLOR_GRID:
        return "Grid";
    case DNE_STYLE_COLOR_NODE_BG:
        return "NodeBg";
    case DNE_STYLE_COLOR_NODE_BORDER:
        return "NodeBorder";
    case DNE_STYLE_COLOR_HOVERED_NODE_BORDER:
        return "HovNodeBorder";
    case DNE_STYLE_COLOR_SELECTED_NODE_BORDER:
        return "SelNodeBorder";
    case DNE_STYLE_COLOR_NODE_SELECTION_RECT:
        return "NodeSelRect";
    case DNE_STYLE_COLOR_NODE_SELECTION_RECT_BORDER:
        return "NodeSelRectBorder";
    case DNE_STYLE_COLOR_HOVERED_LINK_BORDER:
        return "HovLinkBorder";
    case DNE_STYLE_COLOR_SELECTED_LINK_BORDER:
        return "SelLinkBorder";
    case DNE_STYLE_COLOR_HIGHLIGHT_LINK_BORDER:
        return "HighlightLinkBorder";
    case DNE_STYLE_COLOR_LINK_SELECTION_RECT:
        return "LinkSelRect";
    case DNE_STYLE_COLOR_LINK_SELECTION_RECT_BORDER:
        return "LinkSelRectBorder";
    case DNE_STYLE_COLOR_PIN_RECT:
        return "PinRect";
    case DNE_STYLE_COLOR_PIN_RECT_BORDER:
        return "PinRectBorder";
    case DNE_STYLE_COLOR_FLOW:
        return "Flow";
    case DNE_STYLE_COLOR_FLOW_MARKER:
        return "FlowMarker";
    case DNE_STYLE_COLOR_GROUP_BG:
        return "GroupBg";
    case DNE_STYLE_COLOR_GROUP_BORDER:
        return "GroupBorder";
    case DNE_STYLE_COLOR_COUNT:
    default:
        return nullptr;
    }
}
DNE_CATCH(nullptr)

CIMGUI_API void dne_push_style_color(DneStyleColor color, ImVec4_c value) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (!valid_style_color(color))
        return;
    ed::PushStyleColor(to_style_color(color), to_imgui(value));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_pop_style_color(int count) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (count > 0)
        ed::PopStyleColor(count);
}
DNE_CATCH_ABORT

CIMGUI_API void dne_push_style_var_float(DneStyleVar var, float value) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (valid_style_var(var))
        ed::PushStyleVar(to_style_var(var), value);
}
DNE_CATCH_ABORT

CIMGUI_API void dne_push_style_var_vec2(DneStyleVar var, ImVec2_c value) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (valid_style_var(var))
        ed::PushStyleVar(to_style_var(var), to_imgui(value));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_push_style_var_vec4(DneStyleVar var, ImVec4_c value) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (valid_style_var(var))
        ed::PushStyleVar(to_style_var(var), to_imgui(value));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_pop_style_var(int count) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (count > 0)
        ed::PopStyleVar(count);
}
DNE_CATCH_ABORT

#define DNE_STYLE_ACCESSORS(name, member, type) \
    CIMGUI_API type dne_get_style_##name() DNE_TRY \
    { \
        DNE_REQUIRE_EDITOR({}); \
        return from_imgui(ed::GetStyle().member); \
    } \
    DNE_CATCH({}) \
    CIMGUI_API void dne_set_style_##name(type value) DNE_TRY \
    { \
        DNE_REQUIRE_EDITOR(); \
        ed::GetStyle().member = to_imgui(value); \
    } \
    DNE_CATCH()

DNE_STYLE_ACCESSORS(node_padding, NodePadding, ImVec4_c)
DNE_STYLE_ACCESSORS(node_rounding, NodeRounding, float)
DNE_STYLE_ACCESSORS(node_border_width, NodeBorderWidth, float)
DNE_STYLE_ACCESSORS(hovered_node_border_width, HoveredNodeBorderWidth, float)
DNE_STYLE_ACCESSORS(hovered_node_border_offset, HoverNodeBorderOffset, float)
DNE_STYLE_ACCESSORS(selected_node_border_width, SelectedNodeBorderWidth, float)
DNE_STYLE_ACCESSORS(selected_node_border_offset, SelectedNodeBorderOffset, float)
DNE_STYLE_ACCESSORS(pin_rounding, PinRounding, float)
DNE_STYLE_ACCESSORS(pin_border_width, PinBorderWidth, float)
DNE_STYLE_ACCESSORS(link_strength, LinkStrength, float)
DNE_STYLE_ACCESSORS(source_direction, SourceDirection, ImVec2_c)
DNE_STYLE_ACCESSORS(target_direction, TargetDirection, ImVec2_c)
DNE_STYLE_ACCESSORS(scroll_duration, ScrollDuration, float)
DNE_STYLE_ACCESSORS(flow_marker_distance, FlowMarkerDistance, float)
DNE_STYLE_ACCESSORS(flow_speed, FlowSpeed, float)
DNE_STYLE_ACCESSORS(flow_duration, FlowDuration, float)
DNE_STYLE_ACCESSORS(pivot_alignment, PivotAlignment, ImVec2_c)
DNE_STYLE_ACCESSORS(pivot_size, PivotSize, ImVec2_c)
DNE_STYLE_ACCESSORS(pivot_scale, PivotScale, ImVec2_c)
DNE_STYLE_ACCESSORS(pin_corners, PinCorners, float)
DNE_STYLE_ACCESSORS(pin_radius, PinRadius, float)
DNE_STYLE_ACCESSORS(pin_arrow_size, PinArrowSize, float)
DNE_STYLE_ACCESSORS(pin_arrow_width, PinArrowWidth, float)
DNE_STYLE_ACCESSORS(group_rounding, GroupRounding, float)
DNE_STYLE_ACCESSORS(group_border_width, GroupBorderWidth, float)
DNE_STYLE_ACCESSORS(highlight_connected_links, HighlightConnectedLinks, float)
DNE_STYLE_ACCESSORS(snap_link_to_pin_dir, SnapLinkToPinDir, float)

#undef DNE_STYLE_ACCESSORS

CIMGUI_API ImVec4_c dne_get_style_color(DneStyleColor color) DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    if (!valid_style_color(color))
        return {};
    return from_imgui(ed::GetStyle().Colors[to_style_color(color)]);
}
DNE_CATCH({})

CIMGUI_API void dne_set_style_color(DneStyleColor color, ImVec4_c value) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (valid_style_color(color))
        ed::GetStyle().Colors[to_style_color(color)] = to_imgui(value);
}
DNE_CATCH()

CIMGUI_API void dne_begin(const char* id, ImVec2_c size) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (id && ImGui::GetCurrentContext() && std::isfinite(size.x) && std::isfinite(size.y))
        ed::Begin(id, to_imgui(size));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_end() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    if (ImGui::GetCurrentContext())
        ed::End();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_begin_node(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::BeginNode(node_id(node));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_end_node() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndNode();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_begin_pin(uintptr_t pin, DnePinKind kind) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::BeginPin(pin_id(pin), to_pin_kind(kind));
}
DNE_CATCH_ABORT

CIMGUI_API void dne_end_pin() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndPin();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_pin_rect(ImVec2_c a, ImVec2_c b) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::PinRect(to_imgui(a), to_imgui(b));
}
DNE_CATCH()

CIMGUI_API void dne_pin_pivot_rect(ImVec2_c a, ImVec2_c b) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::PinPivotRect(to_imgui(a), to_imgui(b));
}
DNE_CATCH()

CIMGUI_API void dne_pin_pivot_size(ImVec2_c size) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::PinPivotSize(to_imgui(size));
}
DNE_CATCH()

CIMGUI_API void dne_pin_pivot_scale(ImVec2_c scale) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::PinPivotScale(to_imgui(scale));
}
DNE_CATCH()

CIMGUI_API void dne_pin_pivot_alignment(ImVec2_c alignment) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::PinPivotAlignment(to_imgui(alignment));
}
DNE_CATCH()

CIMGUI_API void dne_group(ImVec2_c size) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::Group(to_imgui(size));
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_begin_group_hint(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::BeginGroupHint(node_id(node));
}
DNE_CATCH_ABORT

CIMGUI_API ImVec2_c dne_get_group_min() DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::GetGroupMin());
}
DNE_CATCH({})

CIMGUI_API ImVec2_c dne_get_group_max() DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::GetGroupMax());
}
DNE_CATCH({})

CIMGUI_API ImDrawList* dne_get_hint_foreground_draw_list() DNE_TRY
{
    DNE_REQUIRE_EDITOR(nullptr);
    return ed::GetHintForegroundDrawList();
}
DNE_CATCH_ABORT

CIMGUI_API ImDrawList* dne_get_hint_background_draw_list() DNE_TRY
{
    DNE_REQUIRE_EDITOR(nullptr);
    return ed::GetHintBackgroundDrawList();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_end_group_hint() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndGroupHint();
}
DNE_CATCH_ABORT

CIMGUI_API ImDrawList* dne_get_node_background_draw_list(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(nullptr);
    return ed::GetNodeBackgroundDrawList(node_id(node));
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_link(uintptr_t link, uintptr_t start_pin, uintptr_t end_pin, ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::Link(link_id(link), pin_id(start_pin), pin_id(end_pin), to_imgui(color), thickness);
}
DNE_CATCH(false)

CIMGUI_API void dne_flow(uintptr_t link, DneFlowDirection direction) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::Flow(link_id(link), to_flow_direction(direction));
}
DNE_CATCH()

CIMGUI_API bool dne_begin_create(ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::BeginCreate(to_imgui(color), thickness);
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_query_new_link(uintptr_t* start_pin, uintptr_t* end_pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId start;
    ed::PinId end;
    if (!ed::QueryNewLink(&start, &end))
        return false;
    if (start_pin)
        *start_pin = value(start);
    if (end_pin)
        *end_pin = value(end);
    return true;
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_query_new_link_styled(uintptr_t* start_pin, uintptr_t* end_pin, ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId start;
    ed::PinId end;
    if (!ed::QueryNewLink(&start, &end, to_imgui(color), thickness))
        return false;
    if (start_pin)
        *start_pin = value(start);
    if (end_pin)
        *end_pin = value(end);
    return true;
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_query_new_node(uintptr_t* pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId id;
    if (!ed::QueryNewNode(&id))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_query_new_node_styled(uintptr_t* pin, ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId id;
    if (!ed::QueryNewNode(&id, to_imgui(color), thickness))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_accept_new_item() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptNewItem();
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_new_item_styled(ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptNewItem(to_imgui(color), thickness);
}
DNE_CATCH(false)

CIMGUI_API void dne_reject_new_item() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::RejectNewItem();
}
DNE_CATCH()

CIMGUI_API void dne_reject_new_item_styled(ImVec4_c color, float thickness) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::RejectNewItem(to_imgui(color), thickness);
}
DNE_CATCH()

CIMGUI_API void dne_end_create() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndCreate();
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_begin_delete() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::BeginDelete();
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_query_deleted_link(uintptr_t* link, uintptr_t* start_pin, uintptr_t* end_pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::LinkId link_id_out;
    ed::PinId start;
    ed::PinId end;
    if (!ed::QueryDeletedLink(&link_id_out, &start, &end))
        return false;
    if (link)
        *link = value(link_id_out);
    if (start_pin)
        *start_pin = value(start);
    if (end_pin)
        *end_pin = value(end);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_query_deleted_node(uintptr_t* node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::NodeId id;
    if (!ed::QueryDeletedNode(&id))
        return false;
    if (node)
        *node = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_deleted_item(bool delete_dependencies) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptDeletedItem(delete_dependencies);
}
DNE_CATCH(false)

CIMGUI_API void dne_reject_deleted_item() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::RejectDeletedItem();
}
DNE_CATCH()

CIMGUI_API void dne_end_delete() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndDelete();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_set_node_position(uintptr_t node, ImVec2_c editor_position) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::SetNodePosition(node_id(node), to_imgui(editor_position));
}
DNE_CATCH()

CIMGUI_API void dne_set_group_size(uintptr_t node, ImVec2_c size) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::SetGroupSize(node_id(node), to_imgui(size));
}
DNE_CATCH()

CIMGUI_API ImVec2_c dne_get_node_position(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::GetNodePosition(node_id(node)));
}
DNE_CATCH({})

CIMGUI_API ImVec2_c dne_get_node_size(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::GetNodeSize(node_id(node)));
}
DNE_CATCH({})

CIMGUI_API void dne_center_node_on_screen(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::CenterNodeOnScreen(node_id(node));
}
DNE_CATCH()

CIMGUI_API void dne_set_node_z_position(uintptr_t node, float z) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::SetNodeZPosition(node_id(node), z);
}
DNE_CATCH()

CIMGUI_API float dne_get_node_z_position(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0.0f);
    return ed::GetNodeZPosition(node_id(node));
}
DNE_CATCH(0.0f)

CIMGUI_API void dne_restore_node_state(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::RestoreNodeState(node_id(node));
}
DNE_CATCH()

CIMGUI_API void dne_suspend() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::Suspend();
}
DNE_CATCH_ABORT

CIMGUI_API void dne_resume() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::Resume();
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_is_suspended() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsSuspended();
}
DNE_CATCH(false)

CIMGUI_API bool dne_is_active() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsActive();
}
DNE_CATCH(false)

CIMGUI_API bool dne_has_selection_changed() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::HasSelectionChanged();
}
DNE_CATCH(false)

CIMGUI_API int dne_get_selected_object_count() DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    return ed::GetSelectedObjectCount();
}
DNE_CATCH(0)

CIMGUI_API int dne_get_selected_nodes(uintptr_t* nodes, int size) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    if (size <= 0)
        return 0;
    std::vector<ed::NodeId> tmp(static_cast<size_t>(size));
    int count = ed::GetSelectedNodes(tmp.data(), size);
    int copy_count = count < size ? count : size;
    if (nodes)
    {
        for (int i = 0; i < copy_count; ++i)
            nodes[i] = value(tmp[static_cast<size_t>(i)]);
    }
    return count;
}
DNE_CATCH(0)

CIMGUI_API int dne_get_selected_links(uintptr_t* links, int size) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    if (size <= 0)
        return 0;
    std::vector<ed::LinkId> tmp(static_cast<size_t>(size));
    int count = ed::GetSelectedLinks(tmp.data(), size);
    int copy_count = count < size ? count : size;
    if (links)
    {
        for (int i = 0; i < copy_count; ++i)
            links[i] = value(tmp[static_cast<size_t>(i)]);
    }
    return count;
}
DNE_CATCH(0)

CIMGUI_API bool dne_is_node_selected(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsNodeSelected(node_id(node));
}
DNE_CATCH(false)

CIMGUI_API bool dne_is_link_selected(uintptr_t link) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsLinkSelected(link_id(link));
}
DNE_CATCH(false)

CIMGUI_API void dne_clear_selection() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::ClearSelection();
}
DNE_CATCH()

CIMGUI_API void dne_select_node(uintptr_t node, bool append) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::SelectNode(node_id(node), append);
}
DNE_CATCH()

CIMGUI_API void dne_select_link(uintptr_t link, bool append) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::SelectLink(link_id(link), append);
}
DNE_CATCH()

CIMGUI_API void dne_deselect_node(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::DeselectNode(node_id(node));
}
DNE_CATCH()

CIMGUI_API void dne_deselect_link(uintptr_t link) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::DeselectLink(link_id(link));
}
DNE_CATCH()

CIMGUI_API bool dne_delete_node(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::DeleteNode(node_id(node));
}
DNE_CATCH(false)

CIMGUI_API bool dne_delete_link(uintptr_t link) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::DeleteLink(link_id(link));
}
DNE_CATCH(false)

CIMGUI_API bool dne_has_any_links_node(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::HasAnyLinks(node_id(node));
}
DNE_CATCH(false)

CIMGUI_API bool dne_has_any_links_pin(uintptr_t pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::HasAnyLinks(pin_id(pin));
}
DNE_CATCH(false)

CIMGUI_API int dne_break_links_node(uintptr_t node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    return ed::BreakLinks(node_id(node));
}
DNE_CATCH(0)

CIMGUI_API int dne_break_links_pin(uintptr_t pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    return ed::BreakLinks(pin_id(pin));
}
DNE_CATCH(0)

CIMGUI_API void dne_navigate_to_content(float duration) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::NavigateToContent(duration);
}
DNE_CATCH()

CIMGUI_API void dne_navigate_to_selection(bool zoom_in, float duration) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::NavigateToSelection(zoom_in, duration);
}
DNE_CATCH()

CIMGUI_API bool dne_show_node_context_menu(uintptr_t* node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::NodeId id;
    if (!ed::ShowNodeContextMenu(&id))
        return false;
    if (node)
        *node = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_show_pin_context_menu(uintptr_t* pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId id;
    if (!ed::ShowPinContextMenu(&id))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_show_link_context_menu(uintptr_t* link) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::LinkId id;
    if (!ed::ShowLinkContextMenu(&id))
        return false;
    if (link)
        *link = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_show_background_context_menu() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::ShowBackgroundContextMenu();
}
DNE_CATCH(false)

CIMGUI_API void dne_enable_shortcuts(bool enable) DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EnableShortcuts(enable);
}
DNE_CATCH()

CIMGUI_API bool dne_are_shortcuts_enabled() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AreShortcutsEnabled();
}
DNE_CATCH(false)

CIMGUI_API bool dne_begin_shortcut() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::BeginShortcut();
}
DNE_CATCH_ABORT

CIMGUI_API bool dne_accept_cut() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptCut();
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_copy() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptCopy();
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_paste() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptPaste();
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_duplicate() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptDuplicate();
}
DNE_CATCH(false)

CIMGUI_API bool dne_accept_create_node() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::AcceptCreateNode();
}
DNE_CATCH(false)

CIMGUI_API int dne_get_action_context_size() DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    return ed::GetActionContextSize();
}
DNE_CATCH(0)

CIMGUI_API int dne_get_action_context_nodes(uintptr_t* nodes, int size) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    if (size <= 0)
        return 0;
    std::vector<ed::NodeId> tmp(static_cast<size_t>(size));
    int count = ed::GetActionContextNodes(tmp.data(), size);
    int copy_count = count < size ? count : size;
    if (nodes)
    {
        for (int i = 0; i < copy_count; ++i)
            nodes[i] = value(tmp[static_cast<size_t>(i)]);
    }
    return count;
}
DNE_CATCH(0)

CIMGUI_API int dne_get_action_context_links(uintptr_t* links, int size) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    if (size <= 0)
        return 0;
    std::vector<ed::LinkId> tmp(static_cast<size_t>(size));
    int count = ed::GetActionContextLinks(tmp.data(), size);
    int copy_count = count < size ? count : size;
    if (links)
    {
        for (int i = 0; i < copy_count; ++i)
            links[i] = value(tmp[static_cast<size_t>(i)]);
    }
    return count;
}
DNE_CATCH(0)

CIMGUI_API void dne_end_shortcut() DNE_TRY
{
    DNE_REQUIRE_EDITOR();
    ed::EndShortcut();
}
DNE_CATCH_ABORT

CIMGUI_API float dne_get_current_zoom() DNE_TRY
{
    DNE_REQUIRE_EDITOR(0.0f);
    return ed::GetCurrentZoom();
}
DNE_CATCH(0.0f)

CIMGUI_API bool dne_get_hovered_node(uintptr_t* node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetHoveredNode();
    if (!id)
        return false;
    if (node)
        *node = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_get_hovered_pin(uintptr_t* pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetHoveredPin();
    if (!id)
        return false;
    if (pin)
        *pin = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_get_hovered_link(uintptr_t* link) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetHoveredLink();
    if (!id)
        return false;
    if (link)
        *link = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_get_double_clicked_node(uintptr_t* node) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetDoubleClickedNode();
    if (!id)
        return false;
    if (node)
        *node = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_get_double_clicked_pin(uintptr_t* pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetDoubleClickedPin();
    if (!id)
        return false;
    if (pin)
        *pin = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_get_double_clicked_link(uintptr_t* link) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    auto id = ed::GetDoubleClickedLink();
    if (!id)
        return false;
    if (link)
        *link = value(id);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_is_background_clicked() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsBackgroundClicked();
}
DNE_CATCH(false)

CIMGUI_API bool dne_is_background_double_clicked() DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::IsBackgroundDoubleClicked();
}
DNE_CATCH(false)

CIMGUI_API ImGuiMouseButton dne_get_background_click_button_index() DNE_TRY
{
    DNE_REQUIRE_EDITOR(-1);
    return ed::GetBackgroundClickButtonIndex();
}
DNE_CATCH(-1)

CIMGUI_API ImGuiMouseButton dne_get_background_double_click_button_index() DNE_TRY
{
    DNE_REQUIRE_EDITOR(-1);
    return ed::GetBackgroundDoubleClickButtonIndex();
}
DNE_CATCH(-1)

CIMGUI_API bool dne_get_link_pins(uintptr_t link, uintptr_t* start_pin, uintptr_t* end_pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    ed::PinId start;
    ed::PinId end;
    if (!ed::GetLinkPins(link_id(link), &start, &end))
        return false;
    if (start_pin)
        *start_pin = value(start);
    if (end_pin)
        *end_pin = value(end);
    return true;
}
DNE_CATCH(false)

CIMGUI_API bool dne_pin_had_any_links(uintptr_t pin) DNE_TRY
{
    DNE_REQUIRE_EDITOR(false);
    return ed::PinHadAnyLinks(pin_id(pin));
}
DNE_CATCH(false)

CIMGUI_API ImVec2_c dne_get_screen_size() DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::GetScreenSize());
}
DNE_CATCH({})

CIMGUI_API ImVec2_c dne_screen_to_canvas(ImVec2_c pos) DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::ScreenToCanvas(to_imgui(pos)));
}
DNE_CATCH({})

CIMGUI_API ImVec2_c dne_canvas_to_screen(ImVec2_c pos) DNE_TRY
{
    DNE_REQUIRE_EDITOR({});
    return from_imgui(ed::CanvasToScreen(to_imgui(pos)));
}
DNE_CATCH({})

CIMGUI_API int dne_get_node_count() DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    return ed::GetNodeCount();
}
DNE_CATCH(0)

CIMGUI_API int dne_get_ordered_node_ids(uintptr_t* nodes, int size) DNE_TRY
{
    DNE_REQUIRE_EDITOR(0);
    if (size <= 0)
        return 0;
    std::vector<ed::NodeId> tmp(static_cast<size_t>(size));
    int count = ed::GetOrderedNodeIds(tmp.data(), size);
    int copy_count = count < size ? count : size;
    if (nodes)
    {
        for (int i = 0; i < copy_count; ++i)
            nodes[i] = value(tmp[static_cast<size_t>(i)]);
    }
    return count;
}
DNE_CATCH(0)

} // extern "C"
