#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ImGuiContext ImGuiContext;
typedef struct ImGuiTestEngine ImGuiTestEngine;
typedef struct ImGuiTestEngineScript ImGuiTestEngineScript;

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

typedef struct ImGuiTestEngineResultSummary_c {
    int CountTested;
    int CountSuccess;
    int CountInQueue;
} ImGuiTestEngineResultSummary_c;

ImGuiTestEngine* imgui_test_engine_create_context(void);
void imgui_test_engine_destroy_context(ImGuiTestEngine* engine);

void imgui_test_engine_start(ImGuiTestEngine* engine, ImGuiContext* ui_ctx);
void imgui_test_engine_stop(ImGuiTestEngine* engine);
void imgui_test_engine_post_swap(ImGuiTestEngine* engine);

void imgui_test_engine_show_windows(ImGuiTestEngine* engine, bool* p_open);

void imgui_test_engine_queue_tests(
    ImGuiTestEngine* engine,
    ImGuiTestEngineGroup group,
    const char* filter,
    int run_flags
);

bool imgui_test_engine_is_test_queue_empty(ImGuiTestEngine* engine);
bool imgui_test_engine_try_abort_engine(ImGuiTestEngine* engine);
void imgui_test_engine_abort_current_test(ImGuiTestEngine* engine);

void imgui_test_engine_get_result_summary(
    ImGuiTestEngine* engine,
    ImGuiTestEngineResultSummary_c* out_summary
);

void imgui_test_engine_set_run_speed(ImGuiTestEngine* engine, ImGuiTestEngineRunSpeed speed);
void imgui_test_engine_set_verbose_level(
    ImGuiTestEngine* engine,
    ImGuiTestEngineVerboseLevel level
);
void imgui_test_engine_set_capture_enabled(ImGuiTestEngine* engine, bool enabled);

bool imgui_test_engine_is_running_tests(ImGuiTestEngine* engine);
bool imgui_test_engine_is_requesting_max_app_speed(ImGuiTestEngine* engine);

void imgui_test_engine_install_default_crash_handler(void);

// Register a small set of built-in demo tests (useful to validate integration).
// This does not start the engine; it only registers tests into the engine instance.
void imgui_test_engine_register_default_tests(ImGuiTestEngine* engine);

// Script tests: a small Rust-friendly API to register tests without writing C++ callbacks.
//
// The script is executed by the C++ test engine (ImGuiTestContext) when the test runs.
// It does not provide a GUI function: script tests are meant to drive your application's
// existing UI.
ImGuiTestEngineScript* imgui_test_engine_script_create(void);
void imgui_test_engine_script_destroy(ImGuiTestEngineScript* script);
void imgui_test_engine_script_set_ref(ImGuiTestEngineScript* script, const char* ref);
void imgui_test_engine_script_item_click(ImGuiTestEngineScript* script, const char* ref);
void imgui_test_engine_script_item_open(ImGuiTestEngineScript* script, const char* ref);
void imgui_test_engine_script_item_check(ImGuiTestEngineScript* script, const char* ref);
void imgui_test_engine_script_item_uncheck(ImGuiTestEngineScript* script, const char* ref);
void imgui_test_engine_script_item_input_int(ImGuiTestEngineScript* script, const char* ref, int v);
void imgui_test_engine_script_item_input_str(ImGuiTestEngineScript* script, const char* ref, const char* v);
void imgui_test_engine_script_yield(ImGuiTestEngineScript* script, int frames);
void imgui_test_engine_register_script_test(
    ImGuiTestEngine* engine,
    const char* category,
    const char* name,
    ImGuiTestEngineScript* script
);

#ifdef __cplusplus
}
#endif
