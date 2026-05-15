#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus) && !defined(CIMGUI_DEFINE_ENUMS_AND_STRUCTS)
#include "imgui.h"
#include "imgui_internal.h"
#endif
#include "cimgui.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct DneEditorContext DneEditorContext;

typedef enum DnePinKind {
    DNE_PIN_KIND_INPUT = 0,
    DNE_PIN_KIND_OUTPUT = 1,
} DnePinKind;

typedef enum DneFlowDirection {
    DNE_FLOW_FORWARD = 0,
    DNE_FLOW_BACKWARD = 1,
} DneFlowDirection;

typedef enum DneCanvasSizeMode {
    DNE_CANVAS_SIZE_FIT_VERTICAL_VIEW = 0,
    DNE_CANVAS_SIZE_FIT_HORIZONTAL_VIEW = 1,
    DNE_CANVAS_SIZE_CENTER_ONLY = 2,
} DneCanvasSizeMode;

typedef enum DneSaveReasonFlags {
    DNE_SAVE_REASON_NONE = 0x00000000,
    DNE_SAVE_REASON_NAVIGATION = 0x00000001,
    DNE_SAVE_REASON_POSITION = 0x00000002,
    DNE_SAVE_REASON_SIZE = 0x00000004,
    DNE_SAVE_REASON_SELECTION = 0x00000008,
    DNE_SAVE_REASON_ADD_NODE = 0x00000010,
    DNE_SAVE_REASON_REMOVE_NODE = 0x00000020,
    DNE_SAVE_REASON_USER = 0x00000040,
} DneSaveReasonFlags;

typedef enum DneStyleColor {
    DNE_STYLE_COLOR_BG = 0,
    DNE_STYLE_COLOR_GRID,
    DNE_STYLE_COLOR_NODE_BG,
    DNE_STYLE_COLOR_NODE_BORDER,
    DNE_STYLE_COLOR_HOVERED_NODE_BORDER,
    DNE_STYLE_COLOR_SELECTED_NODE_BORDER,
    DNE_STYLE_COLOR_NODE_SELECTION_RECT,
    DNE_STYLE_COLOR_NODE_SELECTION_RECT_BORDER,
    DNE_STYLE_COLOR_HOVERED_LINK_BORDER,
    DNE_STYLE_COLOR_SELECTED_LINK_BORDER,
    DNE_STYLE_COLOR_HIGHLIGHT_LINK_BORDER,
    DNE_STYLE_COLOR_LINK_SELECTION_RECT,
    DNE_STYLE_COLOR_LINK_SELECTION_RECT_BORDER,
    DNE_STYLE_COLOR_PIN_RECT,
    DNE_STYLE_COLOR_PIN_RECT_BORDER,
    DNE_STYLE_COLOR_FLOW,
    DNE_STYLE_COLOR_FLOW_MARKER,
    DNE_STYLE_COLOR_GROUP_BG,
    DNE_STYLE_COLOR_GROUP_BORDER,
    DNE_STYLE_COLOR_COUNT,
} DneStyleColor;

typedef enum DneStyleVar {
    DNE_STYLE_VAR_NODE_PADDING = 0,
    DNE_STYLE_VAR_NODE_ROUNDING,
    DNE_STYLE_VAR_NODE_BORDER_WIDTH,
    DNE_STYLE_VAR_HOVERED_NODE_BORDER_WIDTH,
    DNE_STYLE_VAR_SELECTED_NODE_BORDER_WIDTH,
    DNE_STYLE_VAR_PIN_ROUNDING,
    DNE_STYLE_VAR_PIN_BORDER_WIDTH,
    DNE_STYLE_VAR_LINK_STRENGTH,
    DNE_STYLE_VAR_SOURCE_DIRECTION,
    DNE_STYLE_VAR_TARGET_DIRECTION,
    DNE_STYLE_VAR_SCROLL_DURATION,
    DNE_STYLE_VAR_FLOW_MARKER_DISTANCE,
    DNE_STYLE_VAR_FLOW_SPEED,
    DNE_STYLE_VAR_FLOW_DURATION,
    DNE_STYLE_VAR_PIVOT_ALIGNMENT,
    DNE_STYLE_VAR_PIVOT_SIZE,
    DNE_STYLE_VAR_PIVOT_SCALE,
    DNE_STYLE_VAR_PIN_CORNERS,
    DNE_STYLE_VAR_PIN_RADIUS,
    DNE_STYLE_VAR_PIN_ARROW_SIZE,
    DNE_STYLE_VAR_PIN_ARROW_WIDTH,
    DNE_STYLE_VAR_GROUP_ROUNDING,
    DNE_STYLE_VAR_GROUP_BORDER_WIDTH,
    DNE_STYLE_VAR_HIGHLIGHT_CONNECTED_LINKS,
    DNE_STYLE_VAR_SNAP_LINK_TO_PIN_DIR,
    DNE_STYLE_VAR_HOVERED_NODE_BORDER_OFFSET,
    DNE_STYLE_VAR_SELECTED_NODE_BORDER_OFFSET,
    DNE_STYLE_VAR_COUNT,
} DneStyleVar;

typedef void (*DneConfigSession)(void* user_pointer);
typedef bool (*DneConfigSaveSettings)(
    const char* data,
    size_t size,
    DneSaveReasonFlags reason,
    void* user_pointer);
typedef size_t (*DneConfigLoadSettings)(char* data, void* user_pointer);
typedef bool (*DneConfigSaveNodeSettings)(
    uintptr_t node_id,
    const char* data,
    size_t size,
    DneSaveReasonFlags reason,
    void* user_pointer);
typedef size_t (*DneConfigLoadNodeSettings)(
    uintptr_t node_id,
    char* data,
    void* user_pointer);

typedef struct DneConfig {
    const char* settings_file;
    DneConfigSession begin_save_session;
    DneConfigSession end_save_session;
    DneConfigSaveSettings save_settings;
    DneConfigLoadSettings load_settings;
    DneConfigSaveNodeSettings save_node_settings;
    DneConfigLoadNodeSettings load_node_settings;
    void* user_pointer;
    const float* custom_zoom_levels;
    int custom_zoom_level_count;
    DneCanvasSizeMode canvas_size_mode;
    int drag_button_index;
    int select_button_index;
    int navigate_button_index;
    int context_menu_button_index;
    bool enable_smooth_zoom;
    float smooth_zoom_power;
} DneConfig;

CIMGUI_API DneEditorContext* dne_create_editor(const DneConfig* config);
CIMGUI_API void dne_destroy_editor(DneEditorContext* ctx);
CIMGUI_API void* dne_editor_context_raw(DneEditorContext* ctx);
CIMGUI_API void* dne_get_current_editor_raw(void);
CIMGUI_API void dne_set_current_editor_raw(void* ctx);
CIMGUI_API void dne_set_current_editor(DneEditorContext* ctx);

CIMGUI_API const char* dne_get_style_color_name(DneStyleColor color);
CIMGUI_API void dne_push_style_color(DneStyleColor color, ImVec4_c value);
CIMGUI_API void dne_pop_style_color(int count);
CIMGUI_API void dne_push_style_var_float(DneStyleVar var, float value);
CIMGUI_API void dne_push_style_var_vec2(DneStyleVar var, ImVec2_c value);
CIMGUI_API void dne_push_style_var_vec4(DneStyleVar var, ImVec4_c value);
CIMGUI_API void dne_pop_style_var(int count);
CIMGUI_API ImVec4_c dne_get_style_node_padding(void);
CIMGUI_API void dne_set_style_node_padding(ImVec4_c value);
CIMGUI_API float dne_get_style_node_rounding(void);
CIMGUI_API void dne_set_style_node_rounding(float value);
CIMGUI_API float dne_get_style_node_border_width(void);
CIMGUI_API void dne_set_style_node_border_width(float value);
CIMGUI_API float dne_get_style_hovered_node_border_width(void);
CIMGUI_API void dne_set_style_hovered_node_border_width(float value);
CIMGUI_API float dne_get_style_hovered_node_border_offset(void);
CIMGUI_API void dne_set_style_hovered_node_border_offset(float value);
CIMGUI_API float dne_get_style_selected_node_border_width(void);
CIMGUI_API void dne_set_style_selected_node_border_width(float value);
CIMGUI_API float dne_get_style_selected_node_border_offset(void);
CIMGUI_API void dne_set_style_selected_node_border_offset(float value);
CIMGUI_API float dne_get_style_pin_rounding(void);
CIMGUI_API void dne_set_style_pin_rounding(float value);
CIMGUI_API float dne_get_style_pin_border_width(void);
CIMGUI_API void dne_set_style_pin_border_width(float value);
CIMGUI_API float dne_get_style_link_strength(void);
CIMGUI_API void dne_set_style_link_strength(float value);
CIMGUI_API ImVec2_c dne_get_style_source_direction(void);
CIMGUI_API void dne_set_style_source_direction(ImVec2_c value);
CIMGUI_API ImVec2_c dne_get_style_target_direction(void);
CIMGUI_API void dne_set_style_target_direction(ImVec2_c value);
CIMGUI_API float dne_get_style_scroll_duration(void);
CIMGUI_API void dne_set_style_scroll_duration(float value);
CIMGUI_API float dne_get_style_flow_marker_distance(void);
CIMGUI_API void dne_set_style_flow_marker_distance(float value);
CIMGUI_API float dne_get_style_flow_speed(void);
CIMGUI_API void dne_set_style_flow_speed(float value);
CIMGUI_API float dne_get_style_flow_duration(void);
CIMGUI_API void dne_set_style_flow_duration(float value);
CIMGUI_API ImVec2_c dne_get_style_pivot_alignment(void);
CIMGUI_API void dne_set_style_pivot_alignment(ImVec2_c value);
CIMGUI_API ImVec2_c dne_get_style_pivot_size(void);
CIMGUI_API void dne_set_style_pivot_size(ImVec2_c value);
CIMGUI_API ImVec2_c dne_get_style_pivot_scale(void);
CIMGUI_API void dne_set_style_pivot_scale(ImVec2_c value);
CIMGUI_API float dne_get_style_pin_corners(void);
CIMGUI_API void dne_set_style_pin_corners(float value);
CIMGUI_API float dne_get_style_pin_radius(void);
CIMGUI_API void dne_set_style_pin_radius(float value);
CIMGUI_API float dne_get_style_pin_arrow_size(void);
CIMGUI_API void dne_set_style_pin_arrow_size(float value);
CIMGUI_API float dne_get_style_pin_arrow_width(void);
CIMGUI_API void dne_set_style_pin_arrow_width(float value);
CIMGUI_API float dne_get_style_group_rounding(void);
CIMGUI_API void dne_set_style_group_rounding(float value);
CIMGUI_API float dne_get_style_group_border_width(void);
CIMGUI_API void dne_set_style_group_border_width(float value);
CIMGUI_API float dne_get_style_highlight_connected_links(void);
CIMGUI_API void dne_set_style_highlight_connected_links(float value);
CIMGUI_API float dne_get_style_snap_link_to_pin_dir(void);
CIMGUI_API void dne_set_style_snap_link_to_pin_dir(float value);
CIMGUI_API ImVec4_c dne_get_style_color(DneStyleColor color);
CIMGUI_API void dne_set_style_color(DneStyleColor color, ImVec4_c value);

CIMGUI_API void dne_begin(const char* id, ImVec2_c size);
CIMGUI_API void dne_end(void);
CIMGUI_API void dne_begin_node(uintptr_t node_id);
CIMGUI_API void dne_end_node(void);
CIMGUI_API void dne_begin_pin(uintptr_t pin_id, DnePinKind kind);
CIMGUI_API void dne_end_pin(void);
CIMGUI_API void dne_pin_rect(ImVec2_c a, ImVec2_c b);
CIMGUI_API void dne_pin_pivot_rect(ImVec2_c a, ImVec2_c b);
CIMGUI_API void dne_pin_pivot_size(ImVec2_c size);
CIMGUI_API void dne_pin_pivot_scale(ImVec2_c scale);
CIMGUI_API void dne_pin_pivot_alignment(ImVec2_c alignment);
CIMGUI_API void dne_group(ImVec2_c size);
CIMGUI_API bool dne_begin_group_hint(uintptr_t node_id);
CIMGUI_API ImVec2_c dne_get_group_min(void);
CIMGUI_API ImVec2_c dne_get_group_max(void);
CIMGUI_API ImDrawList* dne_get_hint_foreground_draw_list(void);
CIMGUI_API ImDrawList* dne_get_hint_background_draw_list(void);
CIMGUI_API void dne_end_group_hint(void);
CIMGUI_API ImDrawList* dne_get_node_background_draw_list(uintptr_t node_id);
CIMGUI_API bool dne_link(uintptr_t link_id, uintptr_t start_pin_id, uintptr_t end_pin_id, ImVec4_c color, float thickness);
CIMGUI_API void dne_flow(uintptr_t link_id, DneFlowDirection direction);

CIMGUI_API bool dne_begin_create(ImVec4_c color, float thickness);
CIMGUI_API bool dne_query_new_link(uintptr_t* start_pin_id, uintptr_t* end_pin_id);
CIMGUI_API bool dne_query_new_link_styled(uintptr_t* start_pin_id, uintptr_t* end_pin_id, ImVec4_c color, float thickness);
CIMGUI_API bool dne_query_new_node(uintptr_t* pin_id);
CIMGUI_API bool dne_query_new_node_styled(uintptr_t* pin_id, ImVec4_c color, float thickness);
CIMGUI_API bool dne_accept_new_item(void);
CIMGUI_API bool dne_accept_new_item_styled(ImVec4_c color, float thickness);
CIMGUI_API void dne_reject_new_item(void);
CIMGUI_API void dne_reject_new_item_styled(ImVec4_c color, float thickness);
CIMGUI_API void dne_end_create(void);

CIMGUI_API bool dne_begin_delete(void);
CIMGUI_API bool dne_query_deleted_link(uintptr_t* link_id, uintptr_t* start_pin_id, uintptr_t* end_pin_id);
CIMGUI_API bool dne_query_deleted_node(uintptr_t* node_id);
CIMGUI_API bool dne_accept_deleted_item(bool delete_dependencies);
CIMGUI_API void dne_reject_deleted_item(void);
CIMGUI_API void dne_end_delete(void);

CIMGUI_API void dne_set_node_position(uintptr_t node_id, ImVec2_c editor_position);
CIMGUI_API void dne_set_group_size(uintptr_t node_id, ImVec2_c size);
CIMGUI_API ImVec2_c dne_get_node_position(uintptr_t node_id);
CIMGUI_API ImVec2_c dne_get_node_size(uintptr_t node_id);
CIMGUI_API void dne_center_node_on_screen(uintptr_t node_id);
CIMGUI_API void dne_set_node_z_position(uintptr_t node_id, float z);
CIMGUI_API float dne_get_node_z_position(uintptr_t node_id);
CIMGUI_API void dne_restore_node_state(uintptr_t node_id);

CIMGUI_API void dne_suspend(void);
CIMGUI_API void dne_resume(void);
CIMGUI_API bool dne_is_suspended(void);
CIMGUI_API bool dne_is_active(void);
CIMGUI_API bool dne_has_selection_changed(void);
CIMGUI_API int dne_get_selected_object_count(void);
CIMGUI_API int dne_get_selected_nodes(uintptr_t* nodes, int size);
CIMGUI_API int dne_get_selected_links(uintptr_t* links, int size);
CIMGUI_API bool dne_is_node_selected(uintptr_t node_id);
CIMGUI_API bool dne_is_link_selected(uintptr_t link_id);
CIMGUI_API void dne_clear_selection(void);
CIMGUI_API void dne_select_node(uintptr_t node_id, bool append);
CIMGUI_API void dne_select_link(uintptr_t link_id, bool append);
CIMGUI_API void dne_deselect_node(uintptr_t node_id);
CIMGUI_API void dne_deselect_link(uintptr_t link_id);

CIMGUI_API bool dne_delete_node(uintptr_t node_id);
CIMGUI_API bool dne_delete_link(uintptr_t link_id);
CIMGUI_API bool dne_has_any_links_node(uintptr_t node_id);
CIMGUI_API bool dne_has_any_links_pin(uintptr_t pin_id);
CIMGUI_API int dne_break_links_node(uintptr_t node_id);
CIMGUI_API int dne_break_links_pin(uintptr_t pin_id);
CIMGUI_API void dne_navigate_to_content(float duration);
CIMGUI_API void dne_navigate_to_selection(bool zoom_in, float duration);

CIMGUI_API bool dne_show_node_context_menu(uintptr_t* node_id);
CIMGUI_API bool dne_show_pin_context_menu(uintptr_t* pin_id);
CIMGUI_API bool dne_show_link_context_menu(uintptr_t* link_id);
CIMGUI_API bool dne_show_background_context_menu(void);

CIMGUI_API void dne_enable_shortcuts(bool enable);
CIMGUI_API bool dne_are_shortcuts_enabled(void);
CIMGUI_API bool dne_begin_shortcut(void);
CIMGUI_API bool dne_accept_cut(void);
CIMGUI_API bool dne_accept_copy(void);
CIMGUI_API bool dne_accept_paste(void);
CIMGUI_API bool dne_accept_duplicate(void);
CIMGUI_API bool dne_accept_create_node(void);
CIMGUI_API int dne_get_action_context_size(void);
CIMGUI_API int dne_get_action_context_nodes(uintptr_t* nodes, int size);
CIMGUI_API int dne_get_action_context_links(uintptr_t* links, int size);
CIMGUI_API void dne_end_shortcut(void);

CIMGUI_API float dne_get_current_zoom(void);
CIMGUI_API bool dne_get_hovered_node(uintptr_t* node_id);
CIMGUI_API bool dne_get_hovered_pin(uintptr_t* pin_id);
CIMGUI_API bool dne_get_hovered_link(uintptr_t* link_id);
CIMGUI_API bool dne_get_double_clicked_node(uintptr_t* node_id);
CIMGUI_API bool dne_get_double_clicked_pin(uintptr_t* pin_id);
CIMGUI_API bool dne_get_double_clicked_link(uintptr_t* link_id);
CIMGUI_API bool dne_is_background_clicked(void);
CIMGUI_API bool dne_is_background_double_clicked(void);
CIMGUI_API ImGuiMouseButton dne_get_background_click_button_index(void);
CIMGUI_API ImGuiMouseButton dne_get_background_double_click_button_index(void);
CIMGUI_API bool dne_get_link_pins(uintptr_t link_id, uintptr_t* start_pin_id, uintptr_t* end_pin_id);
CIMGUI_API bool dne_pin_had_any_links(uintptr_t pin_id);
CIMGUI_API ImVec2_c dne_get_screen_size(void);
CIMGUI_API ImVec2_c dne_screen_to_canvas(ImVec2_c pos);
CIMGUI_API ImVec2_c dne_canvas_to_screen(ImVec2_c pos);
CIMGUI_API int dne_get_node_count(void);
CIMGUI_API int dne_get_ordered_node_ids(uintptr_t* nodes, int size);

#ifdef __cplusplus
}
#endif
