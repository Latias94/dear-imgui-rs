// Script-based tests for Rust consumers.
// This file is part of dear-imgui-rs and is licensed under MIT OR Apache-2.0.

#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#define IMGUI_DEFINE_MATH_OPERATORS
#include "imgui.h"
#include "imgui_internal.h"

#include "imgui_te_context.h"
#include "imgui_te_engine.h" // ImGuiTestEngine_RegisterTest()

#include "cimgui_test_engine.h"

// Definition matching the forward declaration in cimgui_test_engine.h.
struct ImGuiTestEngineScript {
    enum class CmdKind {
        SetRef,
        ItemClick,
        ItemDoubleClick,
        ItemOpen,
        ItemClose,
        ItemCheck,
        ItemUncheck,
        ItemInputInt,
        ItemInputStr,
        MouseMove,
        MouseMoveToVoid,
        MouseClickOnVoid,
        MouseWheel,
        KeyDown,
        KeyUp,
        KeyPress,
        KeyHold,
        KeyChars,
        KeyCharsAppend,
        KeyCharsAppendEnter,
        KeyCharsReplace,
        KeyCharsReplaceEnter,
        ItemHold,
        ItemDragWithDelta,
        ScrollToItemX,
        ScrollToItemY,
        MenuClick,
        MenuCheck,
        MenuUncheck,
        Sleep,
        AssertItemExists,
        AssertItemVisible,
        AssertItemReadIntEq,
        AssertItemReadStrEq,
        WaitForItem,
        WaitForItemVisible,
        AssertItemChecked,
        AssertItemOpened,
        WaitForItemChecked,
        WaitForItemOpened,
        Yield,
    };

    struct Cmd {
        CmdKind Kind{};
        std::string A{};
        std::string B{};
        int I = 0;
        int J = 0;
        float F = 0.0f;
        float G = 0.0f;
    };

    std::string Category{};
    std::vector<Cmd> Cmds{};
};

namespace {

static std::unordered_map<ImGuiTestEngine*, std::vector<ImGuiTestEngineScript*>> g_scripts_by_engine;

static void script_free_for_engine(ImGuiTestEngine* engine) {
    auto it = g_scripts_by_engine.find(engine);
    if (it == g_scripts_by_engine.end()) {
        return;
    }
    for (ImGuiTestEngineScript* script : it->second) {
        delete script;
    }
    g_scripts_by_engine.erase(it);
}

static void script_test_func(ImGuiTestContext* ctx) {
    if (ctx == nullptr || ctx->Test == nullptr) {
        return;
    }
    auto* script = static_cast<ImGuiTestEngineScript*>(ctx->Test->UserData);
    if (script == nullptr) {
        return;
    }

    for (const ImGuiTestEngineScript::Cmd& cmd : script->Cmds) {
        if (ctx->IsError()) {
            return;
        }
        switch (cmd.Kind) {
            case ImGuiTestEngineScript::CmdKind::SetRef:
                ctx->SetRef(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemClick:
                ctx->ItemClick(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemDoubleClick:
                ctx->ItemDoubleClick(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemOpen:
                ctx->ItemOpen(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemClose:
                ctx->ItemClose(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemCheck:
                ctx->ItemCheck(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemUncheck:
                ctx->ItemUncheck(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemInputInt:
                ctx->ItemInputValue(cmd.A.c_str(), cmd.I);
                break;
            case ImGuiTestEngineScript::CmdKind::ItemInputStr:
                ctx->ItemInputValue(cmd.A.c_str(), cmd.B.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MouseMove:
                ctx->MouseMove(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MouseMoveToVoid:
                ctx->MouseMoveToVoid();
                break;
            case ImGuiTestEngineScript::CmdKind::MouseClickOnVoid:
                ctx->MouseClickMulti(static_cast<ImGuiMouseButton>(cmd.I), cmd.J);
                break;
            case ImGuiTestEngineScript::CmdKind::MouseWheel:
                ctx->MouseWheel(ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::KeyDown:
                ctx->KeyDown(static_cast<ImGuiKeyChord>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::KeyUp:
                ctx->KeyUp(static_cast<ImGuiKeyChord>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::KeyPress:
                ctx->KeyPress(static_cast<ImGuiKeyChord>(cmd.I), cmd.J);
                break;
            case ImGuiTestEngineScript::CmdKind::KeyHold:
                ctx->KeyHold(static_cast<ImGuiKeyChord>(cmd.I), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::KeyChars:
                ctx->KeyChars(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::KeyCharsAppend:
                ctx->KeyCharsAppend(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::KeyCharsAppendEnter:
                ctx->KeyCharsAppendEnter(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::KeyCharsReplace:
                ctx->KeyCharsReplace(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::KeyCharsReplaceEnter:
                ctx->KeyCharsReplaceEnter(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemHold:
                ctx->ItemHold(cmd.A.c_str(), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::ItemDragWithDelta:
                ctx->ItemDragWithDelta(cmd.A.c_str(), ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToItemX:
                ctx->ScrollToItemX(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToItemY:
                ctx->ScrollToItemY(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MenuClick:
                ctx->MenuClick(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MenuCheck:
                ctx->MenuCheck(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MenuUncheck:
                ctx->MenuUncheck(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::Sleep:
                ctx->Sleep(cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::AssertItemExists: {
                if (!ctx->ItemExists(cmd.A.c_str())) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::AssertItemVisible: {
                if (!ctx->ItemExists(cmd.A.c_str())) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str());
                if ((info.StatusFlags & ImGuiItemStatusFlags_Visible) == 0) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item is not visible: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::AssertItemReadIntEq: {
                if (!ctx->ItemExists(cmd.A.c_str())) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                int got = ctx->ItemReadAsInt(cmd.A.c_str());
                if (got != cmd.I) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: ItemReadAsInt('%s') == %d, expected %d (ref='%s')",
                        cmd.A.c_str(),
                        got,
                        cmd.I,
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::AssertItemReadStrEq: {
                if (!ctx->ItemExists(cmd.A.c_str())) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                const char* got = ctx->ItemReadAsString(cmd.A.c_str());
                std::string got_s = got ? got : "";
                if (got_s != cmd.B) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: ItemReadAsString('%s') == '%s', expected '%s' (ref='%s')",
                        cmd.A.c_str(),
                        got_s.c_str(),
                        cmd.B.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::WaitForItem: {
                int max_frames = cmd.I;
                if (max_frames < 1) {
                    max_frames = 1;
                }
                for (int n = 0; n < max_frames; n++) {
                    if (ctx->ItemExists(cmd.A.c_str())) {
                        break;
                    }
                    ctx->Yield(1);
                    if (ctx->IsError()) {
                        return;
                    }
                }
                if (!ctx->ItemExists(cmd.A.c_str())) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Timed out waiting for item: '%s' (%d frames, ref='%s')",
                        cmd.A.c_str(),
                        max_frames,
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::WaitForItemVisible: {
                int max_frames = cmd.I;
                if (max_frames < 1) {
                    max_frames = 1;
                }
                bool ok = false;
                for (int n = 0; n < max_frames; n++) {
                    ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str(), ImGuiTestOpFlags_NoError);
                    if (info.ID != 0 && (info.StatusFlags & ImGuiItemStatusFlags_Visible) != 0) {
                        ok = true;
                        break;
                    }
                    ctx->Yield(1);
                    if (ctx->IsError()) {
                        return;
                    }
                }
                if (!ok) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Timed out waiting for item to be visible: '%s' (%d frames, ref='%s')",
                        cmd.A.c_str(),
                        max_frames,
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::AssertItemChecked: {
                ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str(), ImGuiTestOpFlags_NoError);
                if (info.ID == 0) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                if ((info.StatusFlags & ImGuiItemStatusFlags_Checked) == 0) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item is not checked: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::AssertItemOpened: {
                ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str(), ImGuiTestOpFlags_NoError);
                if (info.ID == 0) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item does not exist: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                if ((info.StatusFlags & ImGuiItemStatusFlags_Opened) == 0) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: item is not opened: '%s' (ref='%s')",
                        cmd.A.c_str(),
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::WaitForItemChecked: {
                int max_frames = cmd.I;
                if (max_frames < 1) {
                    max_frames = 1;
                }
                bool ok = false;
                for (int n = 0; n < max_frames; n++) {
                    ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str(), ImGuiTestOpFlags_NoError);
                    if (info.ID != 0 && (info.StatusFlags & ImGuiItemStatusFlags_Checked) != 0) {
                        ok = true;
                        break;
                    }
                    ctx->Yield(1);
                    if (ctx->IsError()) {
                        return;
                    }
                }
                if (!ok) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Timed out waiting for item to be checked: '%s' (%d frames, ref='%s')",
                        cmd.A.c_str(),
                        max_frames,
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::WaitForItemOpened: {
                int max_frames = cmd.I;
                if (max_frames < 1) {
                    max_frames = 1;
                }
                bool ok = false;
                for (int n = 0; n < max_frames; n++) {
                    ImGuiTestItemInfo info = ctx->ItemInfo(cmd.A.c_str(), ImGuiTestOpFlags_NoError);
                    if (info.ID != 0 && (info.StatusFlags & ImGuiItemStatusFlags_Opened) != 0) {
                        ok = true;
                        break;
                    }
                    ctx->Yield(1);
                    if (ctx->IsError()) {
                        return;
                    }
                }
                if (!ok) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Timed out waiting for item to be opened: '%s' (%d frames, ref='%s')",
                        cmd.A.c_str(),
                        max_frames,
                        ctx->RefStr
                    );
                    return;
                }
                break;
            }
            case ImGuiTestEngineScript::CmdKind::Yield:
                ctx->Yield(cmd.I);
                break;
        }
    }
}

} // namespace

// Called from cimgui_test_engine.cpp to ensure we don't leak scripts.
void imgui_test_engine__script_cleanup(ImGuiTestEngine* engine) { script_free_for_engine(engine); }

extern "C" {

ImGuiTestEngineScript* imgui_test_engine_script_create(void) { return new ImGuiTestEngineScript(); }

void imgui_test_engine_script_destroy(ImGuiTestEngineScript* script) { delete script; }

void imgui_test_engine_script_set_ref(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::SetRef,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_click(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemClick,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_double_click(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemDoubleClick,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_open(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemOpen,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_close(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemClose,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_check(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemCheck,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_uncheck(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemUncheck,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_item_input_int(ImGuiTestEngineScript* script, const char* ref, int v) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::ItemInputInt,
        ref ? ref : "",
        {},
        v,
    });
}

void imgui_test_engine_script_item_input_str(ImGuiTestEngineScript* script, const char* ref, const char* v) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemInputStr;
    cmd.A = ref ? ref : "";
    cmd.B = v ? v : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_move(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::MouseMove,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_mouse_move_to_void(ImGuiTestEngineScript* script) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::MouseMoveToVoid,
        {},
        {},
        0,
    });
}

void imgui_test_engine_script_mouse_click_on_void(ImGuiTestEngineScript* script, int button, int count) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseClickOnVoid;
    cmd.I = button;
    cmd.J = count;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_wheel(ImGuiTestEngineScript* script, float dx, float dy) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseWheel;
    cmd.F = dx;
    cmd.G = dy;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_down(ImGuiTestEngineScript* script, int key_chord) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::KeyDown,
        {},
        {},
        key_chord,
    });
}

void imgui_test_engine_script_key_up(ImGuiTestEngineScript* script, int key_chord) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::KeyUp,
        {},
        {},
        key_chord,
    });
}

void imgui_test_engine_script_key_press(ImGuiTestEngineScript* script, int key_chord, int count) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyPress;
    cmd.I = key_chord;
    cmd.J = count;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_hold(ImGuiTestEngineScript* script, int key_chord, float time_in_seconds) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyHold;
    cmd.I = key_chord;
    cmd.F = time_in_seconds;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_chars(ImGuiTestEngineScript* script, const char* chars) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyChars;
    cmd.A = chars ? chars : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_chars_append(ImGuiTestEngineScript* script, const char* chars) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyCharsAppend;
    cmd.A = chars ? chars : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_chars_append_enter(ImGuiTestEngineScript* script, const char* chars) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyCharsAppendEnter;
    cmd.A = chars ? chars : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_chars_replace(ImGuiTestEngineScript* script, const char* chars) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyCharsReplace;
    cmd.A = chars ? chars : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_key_chars_replace_enter(ImGuiTestEngineScript* script, const char* chars) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::KeyCharsReplaceEnter;
    cmd.A = chars ? chars : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_hold(ImGuiTestEngineScript* script, const char* ref, float time_in_seconds) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemHold;
    cmd.A = ref ? ref : "";
    cmd.F = time_in_seconds;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_drag_with_delta(ImGuiTestEngineScript* script, const char* ref, float dx, float dy) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemDragWithDelta;
    cmd.A = ref ? ref : "";
    cmd.F = dx;
    cmd.G = dy;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_item_x(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToItemX;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_item_y(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToItemY;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_menu_click(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MenuClick;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_menu_check(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MenuCheck;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_menu_uncheck(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MenuUncheck;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_sleep(ImGuiTestEngineScript* script, float time_in_seconds) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::Sleep;
    cmd.F = time_in_seconds;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_assert_item_exists(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::AssertItemExists,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_assert_item_visible(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::AssertItemVisible,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_assert_item_read_int_eq(ImGuiTestEngineScript* script, const char* ref, int expected) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadIntEq;
    cmd.A = ref ? ref : "";
    cmd.I = expected;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_assert_item_read_str_eq(ImGuiTestEngineScript* script, const char* ref, const char* expected) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadStrEq;
    cmd.A = ref ? ref : "";
    cmd.B = expected ? expected : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_wait_for_item(ImGuiTestEngineScript* script, const char* ref, int max_frames) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::WaitForItem,
        ref ? ref : "",
        {},
        max_frames,
    });
}

void imgui_test_engine_script_wait_for_item_visible(ImGuiTestEngineScript* script, const char* ref, int max_frames) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::WaitForItemVisible,
        ref ? ref : "",
        {},
        max_frames,
    });
}

void imgui_test_engine_script_assert_item_checked(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::AssertItemChecked,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_assert_item_opened(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::AssertItemOpened,
        ref ? ref : "",
        {},
        0,
    });
}

void imgui_test_engine_script_wait_for_item_checked(ImGuiTestEngineScript* script, const char* ref, int max_frames) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::WaitForItemChecked,
        ref ? ref : "",
        {},
        max_frames,
    });
}

void imgui_test_engine_script_wait_for_item_opened(ImGuiTestEngineScript* script, const char* ref, int max_frames) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::WaitForItemOpened,
        ref ? ref : "",
        {},
        max_frames,
    });
}

void imgui_test_engine_script_yield(ImGuiTestEngineScript* script, int frames) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::Yield;
    cmd.I = frames;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_register_script_test(
    ImGuiTestEngine* engine,
    const char* category,
    const char* name,
    ImGuiTestEngineScript* script
) {
    if (engine == nullptr || script == nullptr || category == nullptr || name == nullptr) {
        return;
    }

    script->Category = category;

    // Register and make sure the test name is owned (category is kept alive by the script).
    ImGuiTest* t = ImGuiTestEngine_RegisterTest(engine, script->Category.c_str(), name, __FILE__, __LINE__);
    t->SetOwnedName(name);
    t->UserData = script;
    t->GuiFunc = nullptr;
    t->TestFunc = script_test_func;

    g_scripts_by_engine[engine].push_back(script);
}

} // extern "C"
