#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#define IMGUI_TEST_ENGINE_ABI_NOEXCEPT noexcept
#else
#define IMGUI_TEST_ENGINE_ABI_NOEXCEPT
#endif

typedef struct ImGuiContext ImGuiContext;
typedef struct ImGuiTestEngine ImGuiTestEngine;
typedef struct ImGuiTestEngineScript ImGuiTestEngineScript;

typedef enum ImGuiTestEngineStatus {
    ImGuiTestEngineStatus_Success = 0,
    ImGuiTestEngineStatus_InvalidArgument = 1,
    ImGuiTestEngineStatus_InvalidState = 2,
    ImGuiTestEngineStatus_NotFound = 3,
    ImGuiTestEngineStatus_OutOfRange = 4,
    ImGuiTestEngineStatus_Exception = 5,
    ImGuiTestEngineStatus_Unsupported = 6,
    ImGuiTestEngineStatus_CaptureFailed = 7,
} ImGuiTestEngineStatus;

typedef bool (*ImGuiTestEngineCaptureCallback_c)(
    uint32_t viewport_id,
    int x,
    int y,
    int width,
    int height,
    uint32_t* pixels,
    void* user_data
);

typedef enum ImGuiTestEngineRunSpeed {
    ImGuiTestEngineRunSpeed_Fast = 0,
    ImGuiTestEngineRunSpeed_Normal = 1,
    ImGuiTestEngineRunSpeed_Cinematic = 2,
} ImGuiTestEngineRunSpeed;

typedef enum ImGuiTestEngineVerboseLevel {
    ImGuiTestEngineVerboseLevel_Silent = 0,
    ImGuiTestEngineVerboseLevel_Error = 1,
    ImGuiTestEngineVerboseLevel_Warning = 2,
    ImGuiTestEngineVerboseLevel_Info = 3,
    ImGuiTestEngineVerboseLevel_Debug = 4,
    ImGuiTestEngineVerboseLevel_Trace = 5,
} ImGuiTestEngineVerboseLevel;

typedef enum ImGuiTestEngineGroup {
    ImGuiTestEngineGroup_Unknown = -1,
    ImGuiTestEngineGroup_Tests = 0,
    ImGuiTestEngineGroup_Perfs = 1,
} ImGuiTestEngineGroup;

typedef enum ImGuiTestEngineBuiltinTestSuite {
    ImGuiTestEngineBuiltinTestSuite_NativeDefaults = 0,
    ImGuiTestEngineBuiltinTestSuite_UpstreamDocking = 1,
    ImGuiTestEngineBuiltinTestSuite_UpstreamViewports = 2,
} ImGuiTestEngineBuiltinTestSuite;

typedef enum ImGuiTestEngineRunFlags {
    ImGuiTestEngineRunFlags_None = 0,
    ImGuiTestEngineRunFlags_GuiFuncDisable = 1 << 0,
    ImGuiTestEngineRunFlags_GuiFuncOnly = 1 << 1,
    ImGuiTestEngineRunFlags_NoSuccessMsg = 1 << 2,
    ImGuiTestEngineRunFlags_EnableRawInputs = 1 << 3,
    ImGuiTestEngineRunFlags_RunFromGui = 1 << 4,
    ImGuiTestEngineRunFlags_RunFromCommandLine = 1 << 5,
    ImGuiTestEngineRunFlags_NoError = 1 << 10,
    ImGuiTestEngineRunFlags_ShareVars = 1 << 11,
    ImGuiTestEngineRunFlags_ShareTestContext = 1 << 12,
} ImGuiTestEngineRunFlags;

typedef enum ImGuiTestEngineCaptureFlags {
    ImGuiTestEngineCaptureFlags_None = 0,
    ImGuiTestEngineCaptureFlags_StitchAll = 1 << 0,
    ImGuiTestEngineCaptureFlags_IncludeOtherWindows = 1 << 1,
    ImGuiTestEngineCaptureFlags_IncludePopups = 1 << 2,
    ImGuiTestEngineCaptureFlags_HideMouseCursor = 1 << 3,
} ImGuiTestEngineCaptureFlags;

typedef struct ImGuiTestEngineResultSummary_c {
    int CountTested;
    int CountSuccess;
    int CountInQueue;
} ImGuiTestEngineResultSummary_c;

// The diagnostic is copied into an internal 2048-byte thread-local buffer. The
// required size includes the trailing NUL and describes the full stored
// diagnostic. Diagnostics longer than 2047 bytes are truncated before storage.
// Query with a null buffer and zero capacity.
ImGuiTestEngineStatus imgui_test_engine_get_last_error(
    char* buffer,
    size_t buffer_size,
    size_t* out_required_size
) IMGUI_TEST_ENGINE_ABI_NOEXCEPT;

ImGuiTestEngineStatus imgui_test_engine_create_context(ImGuiTestEngine** out_engine);
ImGuiTestEngineStatus imgui_test_engine_destroy_context(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_get_ui_context_target(
    ImGuiTestEngine* engine,
    ImGuiContext** out_ui_context
);
ImGuiTestEngineStatus imgui_test_engine_is_bound(ImGuiTestEngine* engine, bool* out_bound);
ImGuiTestEngineStatus imgui_test_engine_is_started(ImGuiTestEngine* engine, bool* out_started);
ImGuiTestEngineStatus imgui_test_engine_unbind(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_start(ImGuiTestEngine* engine, ImGuiContext* ui_ctx);
ImGuiTestEngineStatus imgui_test_engine_stop(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_pre_swap(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_post_swap(
    ImGuiTestEngine* engine,
    bool* out_presentation_completed
);
ImGuiTestEngineStatus imgui_test_engine_abort_presentation(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_install_capture_provider(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureCallback_c callback,
    void* user_data
);
ImGuiTestEngineStatus imgui_test_engine_clear_capture_provider(
    ImGuiTestEngine* engine,
    void* user_data
);
ImGuiTestEngineStatus imgui_test_engine_has_capture_provider(
    ImGuiTestEngine* engine,
    bool* out_installed
);
ImGuiTestEngineStatus imgui_test_engine_show_windows(ImGuiTestEngine* engine, bool* p_open);
// Enum-domain inputs use int so foreign callers can pass arbitrary values for
// validation without constructing invalid C++ enum objects.
ImGuiTestEngineStatus imgui_test_engine_queue_tests(
    ImGuiTestEngine* engine,
    int group,
    const char* filter,
    int run_flags
);
ImGuiTestEngineStatus imgui_test_engine_is_test_queue_empty(
    ImGuiTestEngine* engine,
    bool* out_empty
);
ImGuiTestEngineStatus imgui_test_engine_try_abort_engine(
    ImGuiTestEngine* engine,
    bool* out_aborted
);
ImGuiTestEngineStatus imgui_test_engine_abort_current_test(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_get_result_summary(
    ImGuiTestEngine* engine,
    ImGuiTestEngineResultSummary_c* out_summary
);
ImGuiTestEngineStatus imgui_test_engine_set_run_speed(
    ImGuiTestEngine* engine,
    int speed
);
ImGuiTestEngineStatus imgui_test_engine_set_verbose_level(
    ImGuiTestEngine* engine,
    int level
);
ImGuiTestEngineStatus imgui_test_engine_set_verbose_level_on_error(
    ImGuiTestEngine* engine,
    int level
);
ImGuiTestEngineStatus imgui_test_engine_set_log_to_tty(
    ImGuiTestEngine* engine,
    bool enabled
);
ImGuiTestEngineStatus imgui_test_engine_set_capture_output_enabled(
    ImGuiTestEngine* engine,
    bool enabled
);
ImGuiTestEngineStatus imgui_test_engine_is_running_tests(
    ImGuiTestEngine* engine,
    bool* out_running
);
ImGuiTestEngineStatus imgui_test_engine_is_requesting_max_app_speed(
    ImGuiTestEngine* engine,
    bool* out_requesting
);
ImGuiTestEngineStatus imgui_test_engine_install_default_crash_handler(void);
ImGuiTestEngineStatus imgui_test_engine_register_default_tests(ImGuiTestEngine* engine);
ImGuiTestEngineStatus imgui_test_engine_register_builtin_test_suite(
    ImGuiTestEngine* engine,
    int suite,
    int* out_registered_count
);
ImGuiTestEngineStatus imgui_test_engine_get_registered_test_count(
    ImGuiTestEngine* engine,
    const char* category,
    int* out_count
);
// The required size includes the trailing NUL. Query with a null buffer and
// zero capacity. Tests are returned in upstream registration order.
ImGuiTestEngineStatus imgui_test_engine_get_registered_test_name(
    ImGuiTestEngine* engine,
    const char* category,
    int index,
    char* buffer,
    size_t buffer_size,
    size_t* out_required_size
);
ImGuiTestEngineStatus imgui_test_engine_get_registered_test_succeeded(
    ImGuiTestEngine* engine,
    const char* category,
    int index,
    bool* out_succeeded
);

ImGuiTestEngineStatus imgui_test_engine_script_create(ImGuiTestEngineScript** out_script);
ImGuiTestEngineStatus imgui_test_engine_script_destroy(ImGuiTestEngineScript* script);
ImGuiTestEngineStatus imgui_test_engine_script_set_ref(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_click(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_click_with_button(ImGuiTestEngineScript* script, const char* ref, int button);
ImGuiTestEngineStatus imgui_test_engine_script_item_double_click(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_open(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_close(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_check(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_uncheck(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_item_input_int(ImGuiTestEngineScript* script, const char* ref, int v);
ImGuiTestEngineStatus imgui_test_engine_script_item_input_str(ImGuiTestEngineScript* script, const char* ref, const char* v);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_move(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_move_to_pos(ImGuiTestEngineScript* script, float x, float y);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_teleport_to_pos(ImGuiTestEngineScript* script, float x, float y);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_move_to_void(ImGuiTestEngineScript* script);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_click(ImGuiTestEngineScript* script, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_click_multi(ImGuiTestEngineScript* script, int button, int count);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_double_click(ImGuiTestEngineScript* script, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_down(ImGuiTestEngineScript* script, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_up(ImGuiTestEngineScript* script, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_lift_drag_threshold(ImGuiTestEngineScript* script, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_drag_with_delta(ImGuiTestEngineScript* script, float dx, float dy, int button);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_click_on_void(ImGuiTestEngineScript* script, int button, int count);
ImGuiTestEngineStatus imgui_test_engine_script_mouse_wheel(ImGuiTestEngineScript* script, float dx, float dy);
ImGuiTestEngineStatus imgui_test_engine_script_key_down(ImGuiTestEngineScript* script, int key_chord);
ImGuiTestEngineStatus imgui_test_engine_script_key_up(ImGuiTestEngineScript* script, int key_chord);
ImGuiTestEngineStatus imgui_test_engine_script_key_press(ImGuiTestEngineScript* script, int key_chord, int count);
ImGuiTestEngineStatus imgui_test_engine_script_key_hold(ImGuiTestEngineScript* script, int key_chord, float time_in_seconds);
ImGuiTestEngineStatus imgui_test_engine_script_sleep(ImGuiTestEngineScript* script, float time_in_seconds);
ImGuiTestEngineStatus imgui_test_engine_script_capture_screenshot_window(
    ImGuiTestEngineScript* script,
    const char* ref,
    int capture_flags
);
ImGuiTestEngineStatus imgui_test_engine_script_key_chars(ImGuiTestEngineScript* script, const char* chars);
ImGuiTestEngineStatus imgui_test_engine_script_key_chars_append(ImGuiTestEngineScript* script, const char* chars);
ImGuiTestEngineStatus imgui_test_engine_script_key_chars_append_enter(ImGuiTestEngineScript* script, const char* chars);
ImGuiTestEngineStatus imgui_test_engine_script_key_chars_replace(ImGuiTestEngineScript* script, const char* chars);
ImGuiTestEngineStatus imgui_test_engine_script_key_chars_replace_enter(ImGuiTestEngineScript* script, const char* chars);
ImGuiTestEngineStatus imgui_test_engine_script_item_hold(ImGuiTestEngineScript* script, const char* ref, float time_in_seconds);
ImGuiTestEngineStatus imgui_test_engine_script_item_hold_for_frames(ImGuiTestEngineScript* script, const char* ref, int frames);
ImGuiTestEngineStatus imgui_test_engine_script_item_drag_over_and_hold(ImGuiTestEngineScript* script, const char* ref_src, const char* ref_dst);
ImGuiTestEngineStatus imgui_test_engine_script_item_drag_and_drop(ImGuiTestEngineScript* script, const char* ref_src, const char* ref_dst, int button);
ImGuiTestEngineStatus imgui_test_engine_script_item_drag_with_delta(ImGuiTestEngineScript* script, const char* ref, float dx, float dy);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_x(ImGuiTestEngineScript* script, const char* ref, float scroll_x);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_y(ImGuiTestEngineScript* script, const char* ref, float scroll_y);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_pos_x(ImGuiTestEngineScript* script, const char* window_ref, float pos_x);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_pos_y(ImGuiTestEngineScript* script, const char* window_ref, float pos_y);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_item_x(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_item_y(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_top(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_scroll_to_bottom(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_tab_close(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_combo_click(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_combo_click_all(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_table_click_header(ImGuiTestEngineScript* script, const char* table_ref, const char* label, int key_mods);
ImGuiTestEngineStatus imgui_test_engine_script_menu_click(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_menu_check(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_menu_uncheck(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_menu_check_all(ImGuiTestEngineScript* script, const char* ref_parent);
ImGuiTestEngineStatus imgui_test_engine_script_menu_uncheck_all(ImGuiTestEngineScript* script, const char* ref_parent);
ImGuiTestEngineStatus imgui_test_engine_script_set_input_mode(ImGuiTestEngineScript* script, int input_source);
ImGuiTestEngineStatus imgui_test_engine_script_nav_move_to(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_nav_activate(ImGuiTestEngineScript* script);
ImGuiTestEngineStatus imgui_test_engine_script_nav_input(ImGuiTestEngineScript* script);
ImGuiTestEngineStatus imgui_test_engine_script_item_open_all(ImGuiTestEngineScript* script, const char* ref_parent, int depth, int passes);
ImGuiTestEngineStatus imgui_test_engine_script_item_close_all(ImGuiTestEngineScript* script, const char* ref_parent, int depth, int passes);
ImGuiTestEngineStatus imgui_test_engine_script_window_close(ImGuiTestEngineScript* script, const char* window_ref);
ImGuiTestEngineStatus imgui_test_engine_script_window_collapse(ImGuiTestEngineScript* script, const char* window_ref, bool collapsed);
ImGuiTestEngineStatus imgui_test_engine_script_window_focus(ImGuiTestEngineScript* script, const char* window_ref);
ImGuiTestEngineStatus imgui_test_engine_script_window_bring_to_front(ImGuiTestEngineScript* script, const char* window_ref);
ImGuiTestEngineStatus imgui_test_engine_script_window_move(ImGuiTestEngineScript* script, const char* window_ref, float x, float y);
ImGuiTestEngineStatus imgui_test_engine_script_window_resize(ImGuiTestEngineScript* script, const char* window_ref, float w, float h);
ImGuiTestEngineStatus imgui_test_engine_script_dock_into(
    ImGuiTestEngineScript* script,
    const char* source_ref,
    const char* destination_ref,
    int split_direction,
    bool outer_docking
);
ImGuiTestEngineStatus imgui_test_engine_script_table_open_context_menu(ImGuiTestEngineScript* script, const char* table_ref, int column_n);
ImGuiTestEngineStatus imgui_test_engine_script_table_set_column_enabled(ImGuiTestEngineScript* script, const char* table_ref, int column_n, bool enabled);
ImGuiTestEngineStatus imgui_test_engine_script_table_set_column_enabled_by_label(ImGuiTestEngineScript* script, const char* table_ref, const char* label, bool enabled);
ImGuiTestEngineStatus imgui_test_engine_script_table_resize_column(ImGuiTestEngineScript* script, const char* table_ref, int column_n, float width);
ImGuiTestEngineStatus imgui_test_engine_script_table_resize_column_by_label(ImGuiTestEngineScript* script, const char* table_ref, const char* label, float width);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_exists(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_visible(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_int_eq(ImGuiTestEngineScript* script, const char* ref, int expected);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_str_eq(ImGuiTestEngineScript* script, const char* ref, const char* expected);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_float_eq(ImGuiTestEngineScript* script, const char* ref, float expected, float epsilon);
ImGuiTestEngineStatus imgui_test_engine_script_wait_for_item(ImGuiTestEngineScript* script, const char* ref, int max_frames);
ImGuiTestEngineStatus imgui_test_engine_script_wait_for_item_visible(ImGuiTestEngineScript* script, const char* ref, int max_frames);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_checked(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_assert_item_opened(ImGuiTestEngineScript* script, const char* ref);
ImGuiTestEngineStatus imgui_test_engine_script_wait_for_item_checked(ImGuiTestEngineScript* script, const char* ref, int max_frames);
ImGuiTestEngineStatus imgui_test_engine_script_wait_for_item_opened(ImGuiTestEngineScript* script, const char* ref, int max_frames);
ImGuiTestEngineStatus imgui_test_engine_script_yield(ImGuiTestEngineScript* script, int frames);
ImGuiTestEngineStatus imgui_test_engine_register_script_test(
    ImGuiTestEngine* engine,
    const char* category,
    const char* name,
    ImGuiTestEngineScript* script
);

// Test-only fault injection and lifecycle accounting. Injection is one-shot and
// thread-local so boundary tests cannot affect unrelated callers.
typedef enum ImGuiTestEngineExceptionPoint {
    ImGuiTestEngineExceptionPoint_None = 0,
    ImGuiTestEngineExceptionPoint_EngineAllocation = 1,
    ImGuiTestEngineExceptionPoint_ScriptAllocation = 2,
    ImGuiTestEngineExceptionPoint_ScriptVectorGrowth = 3,
    ImGuiTestEngineExceptionPoint_UpstreamCall = 4,
} ImGuiTestEngineExceptionPoint;

typedef struct ImGuiTestEngineLifecycleCounters_c {
    uint64_t EnginesCreated;
    uint64_t EnginesDestroyed;
    uint64_t EnginesStarted;
    uint64_t EnginesStopped;
    uint64_t EnginesUnbound;
    uint64_t ScriptsCreated;
    uint64_t ScriptsDestroyed;
    uint64_t ScriptsRegistered;
} ImGuiTestEngineLifecycleCounters_c;

typedef struct ImGuiTestEngineCaptureState_c {
    bool PresentationPending;
    bool CaptureAbortRequested;
    bool CaptureWaitPending;
    bool ProviderInstalled;
    bool ContextCapturing;
    bool ToolCapturing;
    bool ToolPicking;
    bool IoCapturing;
    bool EngineAbort;
    bool CaptureRollbackValid;
    bool HiddenWindowBackupValid;
    bool ScreenshotConfigActive;
    bool WindowMoveConfigActive;
    bool VideoConfigActive;
} ImGuiTestEngineCaptureState_c;

typedef enum ImGuiTestEnginePresentationEvent {
    ImGuiTestEnginePresentationEvent_PreSwap = 1,
    ImGuiTestEnginePresentationEvent_PostSwap = 2,
} ImGuiTestEnginePresentationEvent;

typedef void (*ImGuiTestEnginePresentationTraceCallback_c)(
    ImGuiTestEnginePresentationEvent event,
    void* user_data
);

ImGuiTestEngineStatus imgui_test_engine_test_set_exception_injection(
    int point
);
ImGuiTestEngineStatus imgui_test_engine_test_get_lifecycle_counters(
    ImGuiTestEngineLifecycleCounters_c* out_counters
);
ImGuiTestEngineStatus imgui_test_engine_test_reset_lifecycle_counters(void);
ImGuiTestEngineStatus imgui_test_engine_test_set_presentation_trace(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationTraceCallback_c callback,
    void* user_data
);
ImGuiTestEngineStatus imgui_test_engine_test_get_capture_state(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureState_c* out_state
);
ImGuiTestEngineStatus imgui_test_engine_test_set_interactive_capture_state(
    ImGuiTestEngine* engine,
    bool capturing,
    bool picking
);

#ifdef __cplusplus
}
#endif

#undef IMGUI_TEST_ENGINE_ABI_NOEXCEPT
