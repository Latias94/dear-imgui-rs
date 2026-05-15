#include "node_editor_extra.h"

#include "imgui.h"
#include "imgui_node_editor.h"

#include <cstring>
#include <vector>

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
    ed::EditorContext* editor = nullptr;
    DneCallbackBridge* bridge = nullptr;
};

static ed::NodeId node_id(uintptr_t value) { return ed::NodeId(value); }
static ed::PinId pin_id(uintptr_t value) { return ed::PinId(value); }
static ed::LinkId link_id(uintptr_t value) { return ed::LinkId(value); }

static uintptr_t value(ed::NodeId id) { return id.Get(); }
static uintptr_t value(ed::PinId id) { return id.Get(); }
static uintptr_t value(ed::LinkId id) { return id.Get(); }

static ImVec2 to_imgui(ImVec2_c value) { return ImVec2(value.x, value.y); }
static ImVec4 to_imgui(ImVec4_c value) { return ImVec4(value.x, value.y, value.z, value.w); }
static ImVec2_c from_imgui(ImVec2 value) { return ImVec2_c{value.x, value.y}; }
static ImVec4_c from_imgui(ImVec4 value) { return ImVec4_c{value.x, value.y, value.z, value.w}; }

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

static ed::StyleVar to_style_var(DneStyleVar var)
{
    return static_cast<ed::StyleVar>(static_cast<int>(var));
}

static DneSaveReasonFlags from_save_reason(ed::SaveReasonFlags reason)
{
    return static_cast<DneSaveReasonFlags>(static_cast<uint32_t>(reason));
}

static void bridge_begin_save_session(void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (bridge && bridge->begin_save_session)
        bridge->begin_save_session(bridge->user_pointer);
}

static void bridge_end_save_session(void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (bridge && bridge->end_save_session)
        bridge->end_save_session(bridge->user_pointer);
}

static bool bridge_save_settings(const char* data, size_t size, ed::SaveReasonFlags reason, void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->save_settings)
        return false;
    return bridge->save_settings(data, size, from_save_reason(reason), bridge->user_pointer);
}

static size_t bridge_load_settings(char* data, void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->load_settings)
        return 0;
    return bridge->load_settings(data, bridge->user_pointer);
}

static bool bridge_save_node_settings(ed::NodeId node, const char* data, size_t size, ed::SaveReasonFlags reason, void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->save_node_settings)
        return false;
    return bridge->save_node_settings(value(node), data, size, from_save_reason(reason), bridge->user_pointer);
}

static size_t bridge_load_node_settings(ed::NodeId node, char* data, void* user_pointer)
{
    auto* bridge = static_cast<DneCallbackBridge*>(user_pointer);
    if (!bridge || !bridge->load_node_settings)
        return 0;
    return bridge->load_node_settings(value(node), data, bridge->user_pointer);
}

static void copy_config(ed::Config& out, DneCallbackBridge*& bridge, const DneConfig* config)
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
        bridge = new DneCallbackBridge();
        bridge->begin_save_session = config->begin_save_session;
        bridge->end_save_session = config->end_save_session;
        bridge->save_settings = config->save_settings;
        bridge->load_settings = config->load_settings;
        bridge->save_node_settings = config->save_node_settings;
        bridge->load_node_settings = config->load_node_settings;
        bridge->user_pointer = config->user_pointer;

        out.UserPointer = bridge;
        out.BeginSaveSession = bridge_begin_save_session;
        out.EndSaveSession = bridge_end_save_session;
        out.SaveSettings = bridge_save_settings;
        out.LoadSettings = bridge_load_settings;
        out.SaveNodeSettings = bridge_save_node_settings;
        out.LoadNodeSettings = bridge_load_node_settings;
    }
}

extern "C" {

CIMGUI_API DneEditorContext* dne_create_editor(const DneConfig* config)
{
    ed::Config native_config;
    DneCallbackBridge* bridge = nullptr;
    copy_config(native_config, bridge, config);

    auto* handle = new DneEditorContext();
    handle->bridge = bridge;
    handle->editor = ed::CreateEditor(&native_config);
    if (!handle->editor)
    {
        delete bridge;
        delete handle;
        return nullptr;
    }
    return handle;
}

CIMGUI_API void dne_destroy_editor(DneEditorContext* ctx)
{
    if (!ctx)
        return;
    auto* raw = ctx->editor;
    auto* current = ed::GetCurrentEditor();
    if (raw)
    {
        ed::DestroyEditor(raw);
        if (current == raw)
            ed::SetCurrentEditor(nullptr);
    }
    delete ctx->bridge;
    delete ctx;
}

CIMGUI_API void* dne_editor_context_raw(DneEditorContext* ctx)
{
    return raw_editor(ctx);
}

CIMGUI_API void* dne_get_current_editor_raw()
{
    return ed::GetCurrentEditor();
}

CIMGUI_API void dne_set_current_editor_raw(void* ctx)
{
    ed::SetCurrentEditor(reinterpret_cast<ed::EditorContext*>(ctx));
}

CIMGUI_API void dne_set_current_editor(DneEditorContext* ctx)
{
    ed::SetCurrentEditor(raw_editor(ctx));
}

CIMGUI_API const char* dne_get_style_color_name(DneStyleColor color)
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

CIMGUI_API void dne_push_style_color(DneStyleColor color, ImVec4_c value)
{
    ed::PushStyleColor(to_style_color(color), to_imgui(value));
}

CIMGUI_API void dne_pop_style_color(int count)
{
    ed::PopStyleColor(count);
}

CIMGUI_API void dne_push_style_var_float(DneStyleVar var, float value)
{
    ed::PushStyleVar(to_style_var(var), value);
}

CIMGUI_API void dne_push_style_var_vec2(DneStyleVar var, ImVec2_c value)
{
    ed::PushStyleVar(to_style_var(var), to_imgui(value));
}

CIMGUI_API void dne_push_style_var_vec4(DneStyleVar var, ImVec4_c value)
{
    ed::PushStyleVar(to_style_var(var), to_imgui(value));
}

CIMGUI_API void dne_pop_style_var(int count)
{
    ed::PopStyleVar(count);
}

CIMGUI_API ImVec4_c dne_get_style_node_padding()
{
    return from_imgui(ed::GetStyle().NodePadding);
}

CIMGUI_API void dne_set_style_node_padding(ImVec4_c value)
{
    ed::GetStyle().NodePadding = to_imgui(value);
}

CIMGUI_API float dne_get_style_node_rounding()
{
    return ed::GetStyle().NodeRounding;
}

CIMGUI_API void dne_set_style_node_rounding(float value)
{
    ed::GetStyle().NodeRounding = value;
}

CIMGUI_API float dne_get_style_node_border_width()
{
    return ed::GetStyle().NodeBorderWidth;
}

CIMGUI_API void dne_set_style_node_border_width(float value)
{
    ed::GetStyle().NodeBorderWidth = value;
}

CIMGUI_API float dne_get_style_hovered_node_border_width()
{
    return ed::GetStyle().HoveredNodeBorderWidth;
}

CIMGUI_API void dne_set_style_hovered_node_border_width(float value)
{
    ed::GetStyle().HoveredNodeBorderWidth = value;
}

CIMGUI_API float dne_get_style_hovered_node_border_offset()
{
    return ed::GetStyle().HoverNodeBorderOffset;
}

CIMGUI_API void dne_set_style_hovered_node_border_offset(float value)
{
    ed::GetStyle().HoverNodeBorderOffset = value;
}

CIMGUI_API float dne_get_style_selected_node_border_width()
{
    return ed::GetStyle().SelectedNodeBorderWidth;
}

CIMGUI_API void dne_set_style_selected_node_border_width(float value)
{
    ed::GetStyle().SelectedNodeBorderWidth = value;
}

CIMGUI_API float dne_get_style_selected_node_border_offset()
{
    return ed::GetStyle().SelectedNodeBorderOffset;
}

CIMGUI_API void dne_set_style_selected_node_border_offset(float value)
{
    ed::GetStyle().SelectedNodeBorderOffset = value;
}

CIMGUI_API float dne_get_style_pin_rounding()
{
    return ed::GetStyle().PinRounding;
}

CIMGUI_API void dne_set_style_pin_rounding(float value)
{
    ed::GetStyle().PinRounding = value;
}

CIMGUI_API float dne_get_style_pin_border_width()
{
    return ed::GetStyle().PinBorderWidth;
}

CIMGUI_API void dne_set_style_pin_border_width(float value)
{
    ed::GetStyle().PinBorderWidth = value;
}

CIMGUI_API float dne_get_style_link_strength()
{
    return ed::GetStyle().LinkStrength;
}

CIMGUI_API void dne_set_style_link_strength(float value)
{
    ed::GetStyle().LinkStrength = value;
}

CIMGUI_API ImVec2_c dne_get_style_source_direction()
{
    return from_imgui(ed::GetStyle().SourceDirection);
}

CIMGUI_API void dne_set_style_source_direction(ImVec2_c value)
{
    ed::GetStyle().SourceDirection = to_imgui(value);
}

CIMGUI_API ImVec2_c dne_get_style_target_direction()
{
    return from_imgui(ed::GetStyle().TargetDirection);
}

CIMGUI_API void dne_set_style_target_direction(ImVec2_c value)
{
    ed::GetStyle().TargetDirection = to_imgui(value);
}

CIMGUI_API float dne_get_style_scroll_duration()
{
    return ed::GetStyle().ScrollDuration;
}

CIMGUI_API void dne_set_style_scroll_duration(float value)
{
    ed::GetStyle().ScrollDuration = value;
}

CIMGUI_API float dne_get_style_flow_marker_distance()
{
    return ed::GetStyle().FlowMarkerDistance;
}

CIMGUI_API void dne_set_style_flow_marker_distance(float value)
{
    ed::GetStyle().FlowMarkerDistance = value;
}

CIMGUI_API float dne_get_style_flow_speed()
{
    return ed::GetStyle().FlowSpeed;
}

CIMGUI_API void dne_set_style_flow_speed(float value)
{
    ed::GetStyle().FlowSpeed = value;
}

CIMGUI_API float dne_get_style_flow_duration()
{
    return ed::GetStyle().FlowDuration;
}

CIMGUI_API void dne_set_style_flow_duration(float value)
{
    ed::GetStyle().FlowDuration = value;
}

CIMGUI_API ImVec2_c dne_get_style_pivot_alignment()
{
    return from_imgui(ed::GetStyle().PivotAlignment);
}

CIMGUI_API void dne_set_style_pivot_alignment(ImVec2_c value)
{
    ed::GetStyle().PivotAlignment = to_imgui(value);
}

CIMGUI_API ImVec2_c dne_get_style_pivot_size()
{
    return from_imgui(ed::GetStyle().PivotSize);
}

CIMGUI_API void dne_set_style_pivot_size(ImVec2_c value)
{
    ed::GetStyle().PivotSize = to_imgui(value);
}

CIMGUI_API ImVec2_c dne_get_style_pivot_scale()
{
    return from_imgui(ed::GetStyle().PivotScale);
}

CIMGUI_API void dne_set_style_pivot_scale(ImVec2_c value)
{
    ed::GetStyle().PivotScale = to_imgui(value);
}

CIMGUI_API float dne_get_style_pin_corners()
{
    return ed::GetStyle().PinCorners;
}

CIMGUI_API void dne_set_style_pin_corners(float value)
{
    ed::GetStyle().PinCorners = value;
}

CIMGUI_API float dne_get_style_pin_radius()
{
    return ed::GetStyle().PinRadius;
}

CIMGUI_API void dne_set_style_pin_radius(float value)
{
    ed::GetStyle().PinRadius = value;
}

CIMGUI_API float dne_get_style_pin_arrow_size()
{
    return ed::GetStyle().PinArrowSize;
}

CIMGUI_API void dne_set_style_pin_arrow_size(float value)
{
    ed::GetStyle().PinArrowSize = value;
}

CIMGUI_API float dne_get_style_pin_arrow_width()
{
    return ed::GetStyle().PinArrowWidth;
}

CIMGUI_API void dne_set_style_pin_arrow_width(float value)
{
    ed::GetStyle().PinArrowWidth = value;
}

CIMGUI_API float dne_get_style_group_rounding()
{
    return ed::GetStyle().GroupRounding;
}

CIMGUI_API void dne_set_style_group_rounding(float value)
{
    ed::GetStyle().GroupRounding = value;
}

CIMGUI_API float dne_get_style_group_border_width()
{
    return ed::GetStyle().GroupBorderWidth;
}

CIMGUI_API void dne_set_style_group_border_width(float value)
{
    ed::GetStyle().GroupBorderWidth = value;
}

CIMGUI_API float dne_get_style_highlight_connected_links()
{
    return ed::GetStyle().HighlightConnectedLinks;
}

CIMGUI_API void dne_set_style_highlight_connected_links(float value)
{
    ed::GetStyle().HighlightConnectedLinks = value;
}

CIMGUI_API float dne_get_style_snap_link_to_pin_dir()
{
    return ed::GetStyle().SnapLinkToPinDir;
}

CIMGUI_API void dne_set_style_snap_link_to_pin_dir(float value)
{
    ed::GetStyle().SnapLinkToPinDir = value;
}

CIMGUI_API ImVec4_c dne_get_style_color(DneStyleColor color)
{
    return from_imgui(ed::GetStyle().Colors[to_style_color(color)]);
}

CIMGUI_API void dne_set_style_color(DneStyleColor color, ImVec4_c value)
{
    ed::GetStyle().Colors[to_style_color(color)] = to_imgui(value);
}

CIMGUI_API void dne_begin(const char* id, ImVec2_c size)
{
    ed::Begin(id, to_imgui(size));
}

CIMGUI_API void dne_end()
{
    ed::End();
}

CIMGUI_API void dne_begin_node(uintptr_t node)
{
    ed::BeginNode(node_id(node));
}

CIMGUI_API void dne_end_node()
{
    ed::EndNode();
}

CIMGUI_API void dne_begin_pin(uintptr_t pin, DnePinKind kind)
{
    ed::BeginPin(pin_id(pin), to_pin_kind(kind));
}

CIMGUI_API void dne_end_pin()
{
    ed::EndPin();
}

CIMGUI_API void dne_pin_rect(ImVec2_c a, ImVec2_c b)
{
    ed::PinRect(to_imgui(a), to_imgui(b));
}

CIMGUI_API void dne_pin_pivot_rect(ImVec2_c a, ImVec2_c b)
{
    ed::PinPivotRect(to_imgui(a), to_imgui(b));
}

CIMGUI_API void dne_pin_pivot_size(ImVec2_c size)
{
    ed::PinPivotSize(to_imgui(size));
}

CIMGUI_API void dne_pin_pivot_scale(ImVec2_c scale)
{
    ed::PinPivotScale(to_imgui(scale));
}

CIMGUI_API void dne_pin_pivot_alignment(ImVec2_c alignment)
{
    ed::PinPivotAlignment(to_imgui(alignment));
}

CIMGUI_API void dne_group(ImVec2_c size)
{
    ed::Group(to_imgui(size));
}

CIMGUI_API bool dne_begin_group_hint(uintptr_t node)
{
    return ed::BeginGroupHint(node_id(node));
}

CIMGUI_API ImVec2_c dne_get_group_min()
{
    return from_imgui(ed::GetGroupMin());
}

CIMGUI_API ImVec2_c dne_get_group_max()
{
    return from_imgui(ed::GetGroupMax());
}

CIMGUI_API ImDrawList* dne_get_hint_foreground_draw_list()
{
    return ed::GetHintForegroundDrawList();
}

CIMGUI_API ImDrawList* dne_get_hint_background_draw_list()
{
    return ed::GetHintBackgroundDrawList();
}

CIMGUI_API void dne_end_group_hint()
{
    ed::EndGroupHint();
}

CIMGUI_API ImDrawList* dne_get_node_background_draw_list(uintptr_t node)
{
    return ed::GetNodeBackgroundDrawList(node_id(node));
}

CIMGUI_API bool dne_link(uintptr_t link, uintptr_t start_pin, uintptr_t end_pin, ImVec4_c color, float thickness)
{
    return ed::Link(link_id(link), pin_id(start_pin), pin_id(end_pin), to_imgui(color), thickness);
}

CIMGUI_API void dne_flow(uintptr_t link, DneFlowDirection direction)
{
    ed::Flow(link_id(link), to_flow_direction(direction));
}

CIMGUI_API bool dne_begin_create(ImVec4_c color, float thickness)
{
    return ed::BeginCreate(to_imgui(color), thickness);
}

CIMGUI_API bool dne_query_new_link(uintptr_t* start_pin, uintptr_t* end_pin)
{
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

CIMGUI_API bool dne_query_new_link_styled(uintptr_t* start_pin, uintptr_t* end_pin, ImVec4_c color, float thickness)
{
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

CIMGUI_API bool dne_query_new_node(uintptr_t* pin)
{
    ed::PinId id;
    if (!ed::QueryNewNode(&id))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}

CIMGUI_API bool dne_query_new_node_styled(uintptr_t* pin, ImVec4_c color, float thickness)
{
    ed::PinId id;
    if (!ed::QueryNewNode(&id, to_imgui(color), thickness))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}

CIMGUI_API bool dne_accept_new_item()
{
    return ed::AcceptNewItem();
}

CIMGUI_API bool dne_accept_new_item_styled(ImVec4_c color, float thickness)
{
    return ed::AcceptNewItem(to_imgui(color), thickness);
}

CIMGUI_API void dne_reject_new_item()
{
    ed::RejectNewItem();
}

CIMGUI_API void dne_reject_new_item_styled(ImVec4_c color, float thickness)
{
    ed::RejectNewItem(to_imgui(color), thickness);
}

CIMGUI_API void dne_end_create()
{
    ed::EndCreate();
}

CIMGUI_API bool dne_begin_delete()
{
    return ed::BeginDelete();
}

CIMGUI_API bool dne_query_deleted_link(uintptr_t* link, uintptr_t* start_pin, uintptr_t* end_pin)
{
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

CIMGUI_API bool dne_query_deleted_node(uintptr_t* node)
{
    ed::NodeId id;
    if (!ed::QueryDeletedNode(&id))
        return false;
    if (node)
        *node = value(id);
    return true;
}

CIMGUI_API bool dne_accept_deleted_item(bool delete_dependencies)
{
    return ed::AcceptDeletedItem(delete_dependencies);
}

CIMGUI_API void dne_reject_deleted_item()
{
    ed::RejectDeletedItem();
}

CIMGUI_API void dne_end_delete()
{
    ed::EndDelete();
}

CIMGUI_API void dne_set_node_position(uintptr_t node, ImVec2_c editor_position)
{
    ed::SetNodePosition(node_id(node), to_imgui(editor_position));
}

CIMGUI_API void dne_set_group_size(uintptr_t node, ImVec2_c size)
{
    ed::SetGroupSize(node_id(node), to_imgui(size));
}

CIMGUI_API ImVec2_c dne_get_node_position(uintptr_t node)
{
    return from_imgui(ed::GetNodePosition(node_id(node)));
}

CIMGUI_API ImVec2_c dne_get_node_size(uintptr_t node)
{
    return from_imgui(ed::GetNodeSize(node_id(node)));
}

CIMGUI_API void dne_center_node_on_screen(uintptr_t node)
{
    ed::CenterNodeOnScreen(node_id(node));
}

CIMGUI_API void dne_set_node_z_position(uintptr_t node, float z)
{
    ed::SetNodeZPosition(node_id(node), z);
}

CIMGUI_API float dne_get_node_z_position(uintptr_t node)
{
    return ed::GetNodeZPosition(node_id(node));
}

CIMGUI_API void dne_restore_node_state(uintptr_t node)
{
    ed::RestoreNodeState(node_id(node));
}

CIMGUI_API void dne_suspend()
{
    ed::Suspend();
}

CIMGUI_API void dne_resume()
{
    ed::Resume();
}

CIMGUI_API bool dne_is_suspended()
{
    return ed::IsSuspended();
}

CIMGUI_API bool dne_is_active()
{
    return ed::IsActive();
}

CIMGUI_API bool dne_has_selection_changed()
{
    return ed::HasSelectionChanged();
}

CIMGUI_API int dne_get_selected_object_count()
{
    return ed::GetSelectedObjectCount();
}

CIMGUI_API int dne_get_selected_nodes(uintptr_t* nodes, int size)
{
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

CIMGUI_API int dne_get_selected_links(uintptr_t* links, int size)
{
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

CIMGUI_API bool dne_is_node_selected(uintptr_t node)
{
    return ed::IsNodeSelected(node_id(node));
}

CIMGUI_API bool dne_is_link_selected(uintptr_t link)
{
    return ed::IsLinkSelected(link_id(link));
}

CIMGUI_API void dne_clear_selection()
{
    ed::ClearSelection();
}

CIMGUI_API void dne_select_node(uintptr_t node, bool append)
{
    ed::SelectNode(node_id(node), append);
}

CIMGUI_API void dne_select_link(uintptr_t link, bool append)
{
    ed::SelectLink(link_id(link), append);
}

CIMGUI_API void dne_deselect_node(uintptr_t node)
{
    ed::DeselectNode(node_id(node));
}

CIMGUI_API void dne_deselect_link(uintptr_t link)
{
    ed::DeselectLink(link_id(link));
}

CIMGUI_API bool dne_delete_node(uintptr_t node)
{
    return ed::DeleteNode(node_id(node));
}

CIMGUI_API bool dne_delete_link(uintptr_t link)
{
    return ed::DeleteLink(link_id(link));
}

CIMGUI_API bool dne_has_any_links_node(uintptr_t node)
{
    return ed::HasAnyLinks(node_id(node));
}

CIMGUI_API bool dne_has_any_links_pin(uintptr_t pin)
{
    return ed::HasAnyLinks(pin_id(pin));
}

CIMGUI_API int dne_break_links_node(uintptr_t node)
{
    return ed::BreakLinks(node_id(node));
}

CIMGUI_API int dne_break_links_pin(uintptr_t pin)
{
    return ed::BreakLinks(pin_id(pin));
}

CIMGUI_API void dne_navigate_to_content(float duration)
{
    ed::NavigateToContent(duration);
}

CIMGUI_API void dne_navigate_to_selection(bool zoom_in, float duration)
{
    ed::NavigateToSelection(zoom_in, duration);
}

CIMGUI_API bool dne_show_node_context_menu(uintptr_t* node)
{
    ed::NodeId id;
    if (!ed::ShowNodeContextMenu(&id))
        return false;
    if (node)
        *node = value(id);
    return true;
}

CIMGUI_API bool dne_show_pin_context_menu(uintptr_t* pin)
{
    ed::PinId id;
    if (!ed::ShowPinContextMenu(&id))
        return false;
    if (pin)
        *pin = value(id);
    return true;
}

CIMGUI_API bool dne_show_link_context_menu(uintptr_t* link)
{
    ed::LinkId id;
    if (!ed::ShowLinkContextMenu(&id))
        return false;
    if (link)
        *link = value(id);
    return true;
}

CIMGUI_API bool dne_show_background_context_menu()
{
    return ed::ShowBackgroundContextMenu();
}

CIMGUI_API void dne_enable_shortcuts(bool enable)
{
    ed::EnableShortcuts(enable);
}

CIMGUI_API bool dne_are_shortcuts_enabled()
{
    return ed::AreShortcutsEnabled();
}

CIMGUI_API bool dne_begin_shortcut()
{
    return ed::BeginShortcut();
}

CIMGUI_API bool dne_accept_cut()
{
    return ed::AcceptCut();
}

CIMGUI_API bool dne_accept_copy()
{
    return ed::AcceptCopy();
}

CIMGUI_API bool dne_accept_paste()
{
    return ed::AcceptPaste();
}

CIMGUI_API bool dne_accept_duplicate()
{
    return ed::AcceptDuplicate();
}

CIMGUI_API bool dne_accept_create_node()
{
    return ed::AcceptCreateNode();
}

CIMGUI_API int dne_get_action_context_size()
{
    return ed::GetActionContextSize();
}

CIMGUI_API int dne_get_action_context_nodes(uintptr_t* nodes, int size)
{
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

CIMGUI_API int dne_get_action_context_links(uintptr_t* links, int size)
{
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

CIMGUI_API void dne_end_shortcut()
{
    ed::EndShortcut();
}

CIMGUI_API float dne_get_current_zoom()
{
    return ed::GetCurrentZoom();
}

CIMGUI_API bool dne_get_hovered_node(uintptr_t* node)
{
    auto id = ed::GetHoveredNode();
    if (!id)
        return false;
    if (node)
        *node = value(id);
    return true;
}

CIMGUI_API bool dne_get_hovered_pin(uintptr_t* pin)
{
    auto id = ed::GetHoveredPin();
    if (!id)
        return false;
    if (pin)
        *pin = value(id);
    return true;
}

CIMGUI_API bool dne_get_hovered_link(uintptr_t* link)
{
    auto id = ed::GetHoveredLink();
    if (!id)
        return false;
    if (link)
        *link = value(id);
    return true;
}

CIMGUI_API bool dne_get_double_clicked_node(uintptr_t* node)
{
    auto id = ed::GetDoubleClickedNode();
    if (!id)
        return false;
    if (node)
        *node = value(id);
    return true;
}

CIMGUI_API bool dne_get_double_clicked_pin(uintptr_t* pin)
{
    auto id = ed::GetDoubleClickedPin();
    if (!id)
        return false;
    if (pin)
        *pin = value(id);
    return true;
}

CIMGUI_API bool dne_get_double_clicked_link(uintptr_t* link)
{
    auto id = ed::GetDoubleClickedLink();
    if (!id)
        return false;
    if (link)
        *link = value(id);
    return true;
}

CIMGUI_API bool dne_is_background_clicked()
{
    return ed::IsBackgroundClicked();
}

CIMGUI_API bool dne_is_background_double_clicked()
{
    return ed::IsBackgroundDoubleClicked();
}

CIMGUI_API ImGuiMouseButton dne_get_background_click_button_index()
{
    return ed::GetBackgroundClickButtonIndex();
}

CIMGUI_API ImGuiMouseButton dne_get_background_double_click_button_index()
{
    return ed::GetBackgroundDoubleClickButtonIndex();
}

CIMGUI_API bool dne_get_link_pins(uintptr_t link, uintptr_t* start_pin, uintptr_t* end_pin)
{
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

CIMGUI_API bool dne_pin_had_any_links(uintptr_t pin)
{
    return ed::PinHadAnyLinks(pin_id(pin));
}

CIMGUI_API ImVec2_c dne_get_screen_size()
{
    return from_imgui(ed::GetScreenSize());
}

CIMGUI_API ImVec2_c dne_screen_to_canvas(ImVec2_c pos)
{
    return from_imgui(ed::ScreenToCanvas(to_imgui(pos)));
}

CIMGUI_API ImVec2_c dne_canvas_to_screen(ImVec2_c pos)
{
    return from_imgui(ed::CanvasToScreen(to_imgui(pos)));
}

CIMGUI_API int dne_get_node_count()
{
    return ed::GetNodeCount();
}

CIMGUI_API int dne_get_ordered_node_ids(uintptr_t* nodes, int size)
{
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

} // extern "C"
