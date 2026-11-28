#include <SDL3/SDL.h>

#include "backends/imgui_impl_sdl3.h"
#include "native_bridge.h"

#include <climits>
#include <cstdlib>
#include <utility>

namespace {
thread_local bool suppress_upstream_mouse_leave_check = false;

struct SuppressMouseLeaveCheck {
    bool previous = std::exchange(suppress_upstream_mouse_leave_check, true);
    ~SuppressMouseLeaveCheck() { suppress_upstream_mouse_leave_check = previous; }
};

bool mouse_leave_due(int pending_frame, int current_frame, int buttons_down) {
    return pending_frame != 0 && pending_frame <= current_frame && buttons_down == 0;
}

SDL_Window* SDLCALL session_keyboard_focus();
bool SDLCALL session_capture_mouse(bool enabled);
bool SDLCALL session_set_cursor(SDL_Cursor* cursor);
bool SDLCALL session_show_cursor();
bool SDLCALL session_hide_cursor();
bool SDLCALL session_start_text_input(SDL_Window* window);
bool SDLCALL session_stop_text_input(SDL_Window* window);
void SDLCALL session_destroy_window(SDL_Window* window);
} // namespace

namespace ImGui {
int DearImguiSdl3FrameCountForBackend() {
    return suppress_upstream_mouse_leave_check ? INT_MAX : ImGui::GetFrameCount();
}

ImGuiViewport* DearImguiSdl3ViewportForWindow(void* handle) {
    // Window ID zero has no target. A Context can contain uncreated viewports with null handles.
    return handle ? ImGui::FindViewportByPlatformHandle(handle) : nullptr;
}
} // namespace ImGui

// Intercept only the vendored backend's calls. SDL and application callbacks keep their ABI.
#define SDL_GL_SetAttribute dear_imgui_sdl3_hook_gl_set_attribute
#define SDL_GL_CreateContext dear_imgui_sdl3_hook_gl_create_context
#define SDL_GL_MakeCurrent dear_imgui_sdl3_hook_gl_make_current
#define SDL_GL_SetSwapInterval dear_imgui_sdl3_hook_gl_set_swap_interval
#define SDL_GL_SwapWindow dear_imgui_sdl3_hook_gl_swap_window
#define SDL_CreateWindow dear_imgui_sdl3_hook_create_window
#define SDL_DestroyWindow session_destroy_window
#define SDL_GetKeyboardFocus session_keyboard_focus
#define SDL_CaptureMouse session_capture_mouse
#define SDL_SetCursor session_set_cursor
#define SDL_ShowCursor session_show_cursor
#define SDL_HideCursor session_hide_cursor
#define SDL_StartTextInput session_start_text_input
#define SDL_StopTextInput session_stop_text_input
#define ImGui_ImplSDL3_NewFrame ImGui_ImplSDL3_NewFrame_Upstream
#define ImGui_ImplSDL3_Shutdown ImGui_ImplSDL3_Shutdown_Upstream
#define FindViewportByPlatformHandle(handle) DearImguiSdl3ViewportForWindow(handle)
#define GetFrameCount() DearImguiSdl3FrameCountForBackend()

#include "backends/imgui_impl_sdl3.cpp"

#undef GetFrameCount
#undef FindViewportByPlatformHandle
#undef ImGui_ImplSDL3_Shutdown
#undef ImGui_ImplSDL3_NewFrame
#undef SDL_StopTextInput
#undef SDL_StartTextInput
#undef SDL_HideCursor
#undef SDL_ShowCursor
#undef SDL_SetCursor
#undef SDL_CaptureMouse
#undef SDL_GetKeyboardFocus
#undef SDL_DestroyWindow
#undef SDL_CreateWindow
#undef SDL_GL_SwapWindow
#undef SDL_GL_SetSwapInterval
#undef SDL_GL_MakeCurrent
#undef SDL_GL_CreateContext
#undef SDL_GL_SetAttribute

namespace {
// SDL has one mouse and one cursor. Only the session owning the relevant focus may change
// them. Remember active changes so another Context's idle frame cannot undo them.
struct MouseCapture {
    ImGui_ImplSDL3_Data* owner = nullptr;
    SDL_Window* mouse_window = nullptr;
    SDL_Window* keyboard_window = nullptr;
};

thread_local MouseCapture mouse_capture;
thread_local ImGui_ImplSDL3_Data* cursor_hidden_by = nullptr;

// SDL suspends native IME when keyboard focus moves, but keeps its window's active flag.
// Remember the window we started so focus can move between sessions without inheriting input.
thread_local SDL_WindowID started_text_input = 0;

bool session_owns_window(SDL_Window* window) {
    if (!window || !ImGui::GetCurrentContext())
        return false;
    const SDL_WindowID id = SDL_GetWindowID(window);
    return id != 0 && ImGui_ImplSDL3_GetViewportForWindowID(id) != nullptr;
}

SDL_Window* SDLCALL session_keyboard_focus() {
    SDL_Window* focus = SDL_GetKeyboardFocus();
    if (!session_owns_window(focus))
        return nullptr;
    // A stop deferred while another window had focus can now be applied. Upstream will
    // re-read the current Context's request and restart input if it is still wanted.
    auto* data = ImGui_ImplSDL3_GetBackendData();
    if (data->ImeWindow != focus && started_text_input == SDL_GetWindowID(focus)) {
        session_stop_text_input(focus);
        data->ImeDirty = true;
    }
    return focus;
}

bool release_mouse_capture() {
    mouse_capture = {};
    return SDL_CaptureMouse(false);
}

bool SDLCALL session_capture_mouse(bool enabled) {
    auto* data = ImGui_ImplSDL3_GetBackendData();
    if (!enabled)
        return mouse_capture.owner != data || release_mouse_capture();

    // SDL checks keyboard focus but captures the mouse-focus window. Both must be ours.
    SDL_Window* keyboard = SDL_GetKeyboardFocus();
    SDL_Window* mouse = SDL_GetMouseFocus();
    if (!session_owns_window(keyboard) || !session_owns_window(mouse))
        return false;
    // SDL records its capture request even if the driver fails to apply it.
    mouse_capture = { data, mouse, keyboard };
    return SDL_CaptureMouse(true);
}

bool SDLCALL session_set_cursor(SDL_Cursor* cursor) {
    return session_owns_window(SDL_GetMouseFocus()) && SDL_SetCursor(cursor);
}

bool SDLCALL session_show_cursor() {
    if (!session_owns_window(SDL_GetMouseFocus()) || !SDL_ShowCursor())
        return false;
    cursor_hidden_by = nullptr;
    return true;
}

bool SDLCALL session_hide_cursor() {
    if (!session_owns_window(SDL_GetMouseFocus()))
        return false;
    const bool owns_visibility = SDL_CursorVisible() || cursor_hidden_by != nullptr;
    if (!SDL_HideCursor())
        return false;
    if (owns_visibility)
        cursor_hidden_by = ImGui_ImplSDL3_GetBackendData();
    return true;
}

void release_cursor(ImGui_ImplSDL3_Data* data) {
    if (data->MouseLastCursor && SDL_GetCursor() == data->MouseLastCursor)
        SDL_SetCursor(SDL_GetDefaultCursor());
    data->MouseLastCursor = nullptr;
    if (cursor_hidden_by == data) {
        cursor_hidden_by = nullptr;
        SDL_ShowCursor();
    }
}

bool SDLCALL session_start_text_input(SDL_Window* window) {
    auto* data = ImGui_ImplSDL3_GetBackendData();
    const SDL_WindowID id = SDL_GetWindowID(window);
    if (started_text_input != 0 && started_text_input != id) {
        // The focused window is about to start input. Retire the previous request before
        // handing it the native IME, even if the previous Context has not run its next frame.
        SDL_Window* previous = SDL_GetWindowFromID(started_text_input);
        if (previous && !session_stop_text_input(previous))
            return false;
    }
    if (!SDL_StartTextInput(window))
        return false;
    started_text_input = id;
    // Upstream only records its IME window when the composition UI is visible. Plain text
    // input also needs to be stopped on focus change or shutdown.
    data->ImeWindow = window;
    return true;
}

bool SDLCALL session_stop_text_input(SDL_Window* window) {
    const SDL_WindowID id = SDL_GetWindowID(window);
    if (id == 0 || id != started_text_input)
        return true;
    // Cocoa, for example, uses one native field editor. SDL suspends the old window on
    // focus loss. Defer clearing its active flag while the focused window accepts text.
    SDL_Window* focus = SDL_GetKeyboardFocus();
    if (focus && focus != window && SDL_TextInputActive(focus))
        return true;
    if (!SDL_StopTextInput(window))
        return false;
    started_text_input = 0;
    return true;
}

void forget_input_window(ImGui_ImplSDL3_Data* data, SDL_Window* window) {
    if (window && data->ImeWindow == window) {
        session_stop_text_input(window);
        data->ImeWindow = nullptr;
        data->ImeDirty = true;
    }
    if (mouse_capture.owner == data
        && (mouse_capture.mouse_window == window || mouse_capture.keyboard_window == window))
        release_mouse_capture();
    if (window && SDL_GetMouseFocus() == window)
        release_cursor(data);
}

void destroy_window(SDL_Window* window) {
    if (window && SDL_GetWindowID(window) == started_text_input)
        started_text_input = 0;
    SDL_DestroyWindow(window);
}

void SDLCALL session_destroy_window(SDL_Window* window) {
    forget_input_window(ImGui_ImplSDL3_GetBackendData(), window);
    destroy_window(window);
}
} // namespace

void ImGui_ImplSDL3_NewFrame() {
    ImGui_ImplSDL3_Data* data = ImGui_ImplSDL3_GetBackendData();
    if (mouse_capture.owner && (SDL_GetMouseFocus() != mouse_capture.mouse_window
        || SDL_GetKeyboardFocus() != mouse_capture.keyboard_window))
        release_mouse_capture();

    if (!session_owns_window(SDL_GetMouseFocus()))
        release_cursor(data);
    else if (data->MouseLastCursor != SDL_GetCursor())
        data->MouseLastCursor = nullptr; // The last cursor may belong to another Context.

    {
        const SuppressMouseLeaveCheck suppress;
        ImGui_ImplSDL3_NewFrame_Upstream();
    }
    if (mouse_leave_due(data->MousePendingLeaveFrame, ImGui::GetFrameCount(), data->MouseButtonsDown)) {
        data->MouseWindowID = 0;
        data->MousePendingLeaveFrame = 0;
        ImGui::GetIO().AddMousePosEvent(-FLT_MAX, -FLT_MAX);
    }
}

void ImGui_ImplSDL3_Shutdown() {
    auto* data = ImGui_ImplSDL3_GetBackendData();
    if (data->ImeWindow)
        forget_input_window(data, data->ImeWindow);
    if (mouse_capture.owner == data)
        release_mouse_capture();
    release_cursor(data);
    ImGui_ImplSDL3_Shutdown_Upstream();
}

extern "C" bool dear_imgui_sdl3_mouse_leave_due_for_test(
    int pending_frame,
    int current_frame,
    int buttons_down
) try {
    return mouse_leave_due(pending_frame, current_frame, buttons_down);
} catch (...) { return false; }

extern "C" bool dear_imgui_sdl3_is_native_destroy_callback(void (*callback)(ImGuiViewport*)) try {
    return callback == ImGui_ImplSDL3_DestroyWindow;
} catch (...) { return false; }

extern "C" bool dear_imgui_sdl3_detach_leased_window(
    ImGuiViewport* viewport, SDL_Window* expected_window, SDL_GLContext* retained_context
) try {
    if (viewport == nullptr || retained_context == nullptr)
        return false;
    *retained_context = nullptr;
    auto* data = static_cast<ImGui_ImplSDL3_ViewportData*>(viewport->PlatformUserData);
    if (data == nullptr || !data->WindowOwned || data->Window != expected_window)
        return false;
    forget_input_window(ImGui_ImplSDL3_GetBackendData(), expected_window);
    *retained_context = data->GLContext;
    // Release ImGui-allocated sidecar memory while its Context/allocator contract is still live.
    // Native resources transfer to the exact Rust allocation pin and no longer need a Context.
    data->WindowOwned = false;
    ImGui_ImplSDL3_DestroyWindow(viewport);
    viewport->PlatformHandleRaw = nullptr;
    return true;
} catch (...) { std::abort(); }

extern "C" void dear_imgui_sdl3_destroy_leased_window(SDL_Window* window, SDL_GLContext context) try {
    if (context != nullptr)
        SDL_GL_DestroyContext(context);
    if (window != nullptr)
        destroy_window(window);
} catch (...) { std::abort(); }
