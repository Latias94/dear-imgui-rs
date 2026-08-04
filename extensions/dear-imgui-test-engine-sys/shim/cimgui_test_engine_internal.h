#pragma once

#include "cimgui_test_engine.h"

#include <cstddef>
#include <exception>
#include <new>
#include <utility>

enum ImGuiTestRunSpeed : int;

namespace dear_imgui_test_engine_abi {

constexpr std::size_t kDiagnosticCapacity = 2048;

void clear_error() noexcept;
ImGuiTestEngineStatus fail(ImGuiTestEngineStatus status, const char* message) noexcept;
ImGuiTestEngineStatus fail_exception(const char* operation, const char* message) noexcept;
void maybe_inject(ImGuiTestEngineExceptionPoint point);

template <typename Fn>
ImGuiTestEngineStatus boundary(const char* operation, Fn&& function) noexcept {
    clear_error();
    try {
        return std::forward<Fn>(function)();
    } catch (const std::bad_alloc&) {
        return fail_exception(operation, "allocation failed");
    } catch (const std::exception& error) {
        return fail_exception(operation, error.what());
    } catch (...) {
        return fail_exception(operation, "unknown C++ exception");
    }
}

ImGuiTestEngineStatus require_engine(ImGuiTestEngine* engine) noexcept;
void register_engine(ImGuiTestEngine* engine);
void begin_destroy_engine(ImGuiTestEngine* engine) noexcept;
void cancel_destroy_engine(ImGuiTestEngine* engine) noexcept;
void finish_destroy_engine(ImGuiTestEngine* engine) noexcept;
bool has_live_engines() noexcept;
bool has_active_run(ImGuiTestEngine* engine) noexcept;
void release_process_binding(ImGuiTestEngine* engine, ImGuiContext* context) noexcept;
bool begin_presentation(ImGuiTestEngine* engine) noexcept;
bool presentation_pending(ImGuiTestEngine* engine) noexcept;
void finish_presentation(ImGuiTestEngine* engine) noexcept;
void trace_presentation(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationEvent event
);
bool set_presentation_trace(
    ImGuiTestEngine* engine,
    ImGuiTestEnginePresentationTraceCallback_c callback,
    void* user_data
) noexcept;
bool get_capture_state(
    ImGuiTestEngine* engine,
    ImGuiTestEngineCaptureState_c* out_state
) noexcept;
bool set_interactive_capture_state(
    ImGuiTestEngine* engine,
    bool capturing,
    bool picking
) noexcept;
void finish_capture_rollback(ImGuiTestEngine* engine) noexcept;
void record_capture_rollback(ImGuiTestEngine* engine);
void record_capture_wait(ImGuiTestEngine* engine);
void commit_capture_rollback(ImGuiTestEngine* engine) noexcept;
bool capture_provider_failed(ImGuiTestEngine* engine) noexcept;
bool take_capture_provider_failure(ImGuiTestEngine* engine) noexcept;
void clear_capture_provider(ImGuiTestEngine* engine) noexcept;
bool capture_abort_requested(ImGuiTestEngine* engine) noexcept;
bool capture_wait_pending(ImGuiTestEngine* engine) noexcept;
void clear_capture_abort(ImGuiTestEngine* engine) noexcept;
void begin_screenshot_config(ImGuiTestEngine* engine, ImGuiTestRunSpeed run_speed) noexcept;
void restore_screenshot_config(ImGuiTestEngine* engine) noexcept;
void begin_window_move_config(ImGuiTestEngine* engine, bool value) noexcept;
void restore_window_move_config(ImGuiTestEngine* engine) noexcept;
void begin_video_config(ImGuiTestEngine* engine) noexcept;
void restore_video_config(ImGuiTestEngine* engine) noexcept;
void request_capture_abort(ImGuiTestEngine* engine) noexcept;
void cancel_capture(ImGuiTestEngine* engine) noexcept;
void finish_capture_wait(ImGuiTestEngine* engine) noexcept;

enum class Counter {
    EngineCreated,
    EngineDestroyed,
    EngineStarted,
    EngineStopped,
    EngineUnbound,
    ScriptCreated,
    ScriptDestroyed,
    ScriptRegistered,
};

void increment(Counter counter) noexcept;
ImGuiTestEngineLifecycleCounters_c counters() noexcept;
void reset_counters() noexcept;

class ScopedCurrentContext {
public:
    explicit ScopedCurrentContext(ImGuiContext* context) noexcept;
    ~ScopedCurrentContext() noexcept;

    ScopedCurrentContext(const ScopedCurrentContext&) = delete;
    ScopedCurrentContext& operator=(const ScopedCurrentContext&) = delete;

private:
    ImGuiContext* previous_;
    ImGuiContext* target_;
};

// Implemented by script_tests.cpp. The cleanup function is no-throw and is
// called only after the engine has passed lifecycle validation.
void cleanup_scripts(ImGuiTestEngine* engine) noexcept;
bool has_live_scripts() noexcept;
void register_imgui_hooks() noexcept;
void register_context_shutdown_observer(ImGuiTestEngine* engine, ImGuiContext* context);
void unregister_context_shutdown_observer(
    ImGuiTestEngine* engine,
    ImGuiContext* context
) noexcept;

} // namespace dear_imgui_test_engine_abi
