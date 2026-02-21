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
        ItemCheck,
        ItemUncheck,
        ItemInputInt,
        ItemInputStr,
        MouseMove,
        KeyDown,
        KeyPress,
        Sleep,
        AssertItemExists,
        AssertItemVisible,
        WaitForItem,
        Yield,
    };

    struct Cmd {
        CmdKind Kind{};
        std::string A{};
        std::string B{};
        int I = 0;
        int J = 0;
        float F = 0.0f;
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
            case ImGuiTestEngineScript::CmdKind::KeyDown:
                ctx->KeyDown(static_cast<ImGuiKeyChord>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::KeyPress:
                ctx->KeyPress(static_cast<ImGuiKeyChord>(cmd.I), cmd.J);
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
