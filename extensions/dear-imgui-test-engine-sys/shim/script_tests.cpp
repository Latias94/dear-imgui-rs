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
        ItemClickWithButton,
        ItemDoubleClick,
        ItemOpen,
        ItemClose,
        ItemCheck,
        ItemUncheck,
        ItemInputInt,
        ItemInputStr,
        MouseMove,
        MouseMoveToPos,
        MouseTeleportToPos,
        MouseMoveToVoid,
        MouseClick,
        MouseClickMulti,
        MouseDoubleClick,
        MouseDown,
        MouseUp,
        MouseLiftDragThreshold,
        MouseDragWithDelta,
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
        ItemHoldForFrames,
        ItemDragOverAndHold,
        ItemDragAndDrop,
        ItemDragWithDelta,
        ScrollToX,
        ScrollToY,
        ScrollToPosX,
        ScrollToPosY,
        ScrollToItemX,
        ScrollToItemY,
        ScrollToTop,
        ScrollToBottom,
        TabClose,
        ComboClick,
        ComboClickAll,
        ItemOpenAll,
        ItemCloseAll,
        TableClickHeader,
        TableOpenContextMenu,
        TableSetColumnEnabled,
        TableSetColumnEnabledByLabel,
        TableResizeColumn,
        MenuClick,
        MenuCheck,
        MenuUncheck,
        MenuCheckAll,
        MenuUncheckAll,
        SetInputMode,
        NavMoveTo,
        NavActivate,
        NavInput,
        WindowClose,
        WindowCollapse,
        WindowFocus,
        WindowBringToFront,
        WindowMove,
        WindowResize,
        Sleep,
        AssertItemExists,
        AssertItemVisible,
        AssertItemReadIntEq,
        AssertItemReadStrEq,
        AssertItemReadFloatEq,
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
            case ImGuiTestEngineScript::CmdKind::ItemClickWithButton:
                ctx->ItemClick(cmd.A.c_str(), static_cast<ImGuiMouseButton>(cmd.I));
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
            case ImGuiTestEngineScript::CmdKind::MouseMoveToPos:
                ctx->MouseMoveToPos(ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseTeleportToPos:
                ctx->MouseTeleportToPos(ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseMoveToVoid:
                ctx->MouseMoveToVoid();
                break;
            case ImGuiTestEngineScript::CmdKind::MouseClick:
                ctx->MouseClick(static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseClickMulti:
                ctx->MouseClickMulti(static_cast<ImGuiMouseButton>(cmd.I), cmd.J);
                break;
            case ImGuiTestEngineScript::CmdKind::MouseDoubleClick:
                ctx->MouseDoubleClick(static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseDown:
                ctx->MouseDown(static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseUp:
                ctx->MouseUp(static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseLiftDragThreshold:
                ctx->MouseLiftDragThreshold(static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseDragWithDelta:
                ctx->MouseDragWithDelta(ImVec2(cmd.F, cmd.G), static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::MouseClickOnVoid:
                for (int n = 0; n < cmd.J; n++) {
                    ctx->MouseClickOnVoid(static_cast<ImGuiMouseButton>(cmd.I));
                }
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
            case ImGuiTestEngineScript::CmdKind::ItemHoldForFrames:
                ctx->ItemHoldForFrames(cmd.A.c_str(), cmd.I);
                break;
            case ImGuiTestEngineScript::CmdKind::ItemDragOverAndHold:
                ctx->ItemDragOverAndHold(cmd.A.c_str(), cmd.B.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemDragAndDrop:
                ctx->ItemDragAndDrop(cmd.A.c_str(), cmd.B.c_str(), static_cast<ImGuiMouseButton>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::ItemDragWithDelta:
                ctx->ItemDragWithDelta(cmd.A.c_str(), ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToX:
                ctx->ScrollToX(cmd.A.c_str(), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToY:
                ctx->ScrollToY(cmd.A.c_str(), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToPosX:
                ctx->ScrollToPosX(cmd.A.c_str(), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToPosY:
                ctx->ScrollToPosY(cmd.A.c_str(), cmd.F);
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToItemX:
                ctx->ScrollToItemX(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToItemY:
                ctx->ScrollToItemY(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToTop:
                ctx->ScrollToTop(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ScrollToBottom:
                ctx->ScrollToBottom(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::TabClose:
                ctx->TabClose(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ComboClick:
                ctx->ComboClick(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ComboClickAll:
                ctx->ComboClickAll(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::ItemOpenAll:
                ctx->ItemOpenAll(cmd.A.c_str(), cmd.I, cmd.J);
                break;
            case ImGuiTestEngineScript::CmdKind::ItemCloseAll:
                ctx->ItemCloseAll(cmd.A.c_str(), cmd.I, cmd.J);
                break;
            case ImGuiTestEngineScript::CmdKind::TableClickHeader:
                (void)ctx->TableClickHeader(cmd.A.c_str(), cmd.B.c_str(), static_cast<ImGuiKeyChord>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::TableOpenContextMenu:
                ctx->TableOpenContextMenu(cmd.A.c_str(), cmd.I);
                break;
            case ImGuiTestEngineScript::CmdKind::TableSetColumnEnabled:
                ctx->TableSetColumnEnabled(cmd.A.c_str(), cmd.I, cmd.J != 0);
                break;
            case ImGuiTestEngineScript::CmdKind::TableSetColumnEnabledByLabel:
                ctx->TableSetColumnEnabled(cmd.A.c_str(), cmd.B.c_str(), cmd.I != 0);
                break;
            case ImGuiTestEngineScript::CmdKind::TableResizeColumn:
                ctx->TableResizeColumn(cmd.A.c_str(), cmd.I, cmd.F);
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
            case ImGuiTestEngineScript::CmdKind::MenuCheckAll:
                ctx->MenuCheckAll(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::MenuUncheckAll:
                ctx->MenuUncheckAll(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::SetInputMode:
                ctx->SetInputMode(static_cast<ImGuiInputSource>(cmd.I));
                break;
            case ImGuiTestEngineScript::CmdKind::NavMoveTo:
                ctx->NavMoveTo(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::NavActivate:
                ctx->NavActivate();
                break;
            case ImGuiTestEngineScript::CmdKind::NavInput:
                ctx->NavInput();
                break;
            case ImGuiTestEngineScript::CmdKind::WindowClose:
                ctx->WindowClose(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::WindowCollapse:
                ctx->WindowCollapse(cmd.A.c_str(), cmd.I != 0);
                break;
            case ImGuiTestEngineScript::CmdKind::WindowFocus:
                ctx->WindowFocus(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::WindowBringToFront:
                ctx->WindowBringToFront(cmd.A.c_str());
                break;
            case ImGuiTestEngineScript::CmdKind::WindowMove:
                ctx->WindowMove(cmd.A.c_str(), ImVec2(cmd.F, cmd.G));
                break;
            case ImGuiTestEngineScript::CmdKind::WindowResize:
                ctx->WindowResize(cmd.A.c_str(), ImVec2(cmd.F, cmd.G));
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
            case ImGuiTestEngineScript::CmdKind::AssertItemReadFloatEq: {
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
                float got = ctx->ItemReadAsFloat(cmd.A.c_str());
                float diff = got - cmd.F;
                if (diff < 0.0f) {
                    diff = -diff;
                }
                float eps = cmd.G;
                if (eps < 0.0f) {
                    eps = -eps;
                }
                if (diff > eps) {
                    ImGuiTestEngine_Error(
                        __FILE__,
                        __func__,
                        __LINE__,
                        ImGuiTestCheckFlags_None,
                        "Script assertion failed: ItemReadAsFloat('%s') == %f, expected %f (eps=%f, ref='%s')",
                        cmd.A.c_str(),
                        got,
                        cmd.F,
                        eps,
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

void imgui_test_engine_script_item_click_with_button(ImGuiTestEngineScript* script, const char* ref, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemClickWithButton;
    cmd.A = ref ? ref : "";
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
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

void imgui_test_engine_script_mouse_move_to_pos(ImGuiTestEngineScript* script, float x, float y) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseMoveToPos;
    cmd.F = x;
    cmd.G = y;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_teleport_to_pos(ImGuiTestEngineScript* script, float x, float y) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseTeleportToPos;
    cmd.F = x;
    cmd.G = y;
    script->Cmds.push_back(std::move(cmd));
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

void imgui_test_engine_script_mouse_click(ImGuiTestEngineScript* script, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseClick;
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_click_multi(ImGuiTestEngineScript* script, int button, int count) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseClickMulti;
    cmd.I = button;
    cmd.J = count;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_double_click(ImGuiTestEngineScript* script, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseDoubleClick;
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_down(ImGuiTestEngineScript* script, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseDown;
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_up(ImGuiTestEngineScript* script, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseUp;
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_lift_drag_threshold(ImGuiTestEngineScript* script, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseLiftDragThreshold;
    cmd.I = button;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_mouse_drag_with_delta(ImGuiTestEngineScript* script, float dx, float dy, int button) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MouseDragWithDelta;
    cmd.I = button;
    cmd.F = dx;
    cmd.G = dy;
    script->Cmds.push_back(std::move(cmd));
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

void imgui_test_engine_script_item_hold_for_frames(ImGuiTestEngineScript* script, const char* ref, int frames) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemHoldForFrames;
    cmd.A = ref ? ref : "";
    cmd.I = frames;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_drag_over_and_hold(
    ImGuiTestEngineScript* script,
    const char* ref_src,
    const char* ref_dst
) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemDragOverAndHold;
    cmd.A = ref_src ? ref_src : "";
    cmd.B = ref_dst ? ref_dst : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_drag_and_drop(
    ImGuiTestEngineScript* script,
    const char* ref_src,
    const char* ref_dst,
    int button
) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemDragAndDrop;
    cmd.A = ref_src ? ref_src : "";
    cmd.B = ref_dst ? ref_dst : "";
    cmd.I = button;
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

void imgui_test_engine_script_scroll_to_x(ImGuiTestEngineScript* script, const char* ref, float scroll_x) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToX;
    cmd.A = ref ? ref : "";
    cmd.F = scroll_x;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_y(ImGuiTestEngineScript* script, const char* ref, float scroll_y) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToY;
    cmd.A = ref ? ref : "";
    cmd.F = scroll_y;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_pos_x(ImGuiTestEngineScript* script, const char* window_ref, float pos_x) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToPosX;
    cmd.A = window_ref ? window_ref : "";
    cmd.F = pos_x;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_pos_y(ImGuiTestEngineScript* script, const char* window_ref, float pos_y) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToPosY;
    cmd.A = window_ref ? window_ref : "";
    cmd.F = pos_y;
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

void imgui_test_engine_script_scroll_to_top(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToTop;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_scroll_to_bottom(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ScrollToBottom;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_tab_close(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TabClose;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_combo_click(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ComboClick;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_combo_click_all(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ComboClickAll;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_open_all(ImGuiTestEngineScript* script, const char* ref_parent, int depth, int passes) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemOpenAll;
    cmd.A = ref_parent ? ref_parent : "";
    cmd.I = depth;
    cmd.J = passes;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_item_close_all(ImGuiTestEngineScript* script, const char* ref_parent, int depth, int passes) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::ItemCloseAll;
    cmd.A = ref_parent ? ref_parent : "";
    cmd.I = depth;
    cmd.J = passes;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_table_click_header(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    const char* label,
    int key_mods
) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TableClickHeader;
    cmd.A = table_ref ? table_ref : "";
    cmd.B = label ? label : "";
    cmd.I = key_mods;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_table_open_context_menu(ImGuiTestEngineScript* script, const char* table_ref, int column_n) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TableOpenContextMenu;
    cmd.A = table_ref ? table_ref : "";
    cmd.I = column_n;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_table_set_column_enabled(ImGuiTestEngineScript* script, const char* table_ref, int column_n, bool enabled) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TableSetColumnEnabled;
    cmd.A = table_ref ? table_ref : "";
    cmd.I = column_n;
    cmd.J = enabled ? 1 : 0;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_table_set_column_enabled_by_label(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    const char* label,
    bool enabled
) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TableSetColumnEnabledByLabel;
    cmd.A = table_ref ? table_ref : "";
    cmd.B = label ? label : "";
    cmd.I = enabled ? 1 : 0;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_table_resize_column(ImGuiTestEngineScript* script, const char* table_ref, int column_n, float width) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::TableResizeColumn;
    cmd.A = table_ref ? table_ref : "";
    cmd.I = column_n;
    cmd.F = width;
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

void imgui_test_engine_script_menu_check_all(ImGuiTestEngineScript* script, const char* ref_parent) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MenuCheckAll;
    cmd.A = ref_parent ? ref_parent : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_menu_uncheck_all(ImGuiTestEngineScript* script, const char* ref_parent) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::MenuUncheckAll;
    cmd.A = ref_parent ? ref_parent : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_set_input_mode(ImGuiTestEngineScript* script, int input_source) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::SetInputMode;
    cmd.I = input_source;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_nav_move_to(ImGuiTestEngineScript* script, const char* ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::NavMoveTo;
    cmd.A = ref ? ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_nav_activate(ImGuiTestEngineScript* script) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::NavActivate,
    });
}

void imgui_test_engine_script_nav_input(ImGuiTestEngineScript* script) {
    if (script == nullptr) {
        return;
    }
    script->Cmds.push_back(ImGuiTestEngineScript::Cmd{
        ImGuiTestEngineScript::CmdKind::NavInput,
    });
}

void imgui_test_engine_script_window_close(ImGuiTestEngineScript* script, const char* window_ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowClose;
    cmd.A = window_ref ? window_ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_window_collapse(ImGuiTestEngineScript* script, const char* window_ref, bool collapsed) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowCollapse;
    cmd.A = window_ref ? window_ref : "";
    cmd.I = collapsed ? 1 : 0;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_window_focus(ImGuiTestEngineScript* script, const char* window_ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowFocus;
    cmd.A = window_ref ? window_ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_window_bring_to_front(ImGuiTestEngineScript* script, const char* window_ref) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowBringToFront;
    cmd.A = window_ref ? window_ref : "";
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_window_move(ImGuiTestEngineScript* script, const char* window_ref, float x, float y) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowMove;
    cmd.A = window_ref ? window_ref : "";
    cmd.F = x;
    cmd.G = y;
    script->Cmds.push_back(std::move(cmd));
}

void imgui_test_engine_script_window_resize(ImGuiTestEngineScript* script, const char* window_ref, float w, float h) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::WindowResize;
    cmd.A = window_ref ? window_ref : "";
    cmd.F = w;
    cmd.G = h;
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

void imgui_test_engine_script_assert_item_read_float_eq(
    ImGuiTestEngineScript* script,
    const char* ref,
    float expected,
    float epsilon
) {
    if (script == nullptr) {
        return;
    }
    ImGuiTestEngineScript::Cmd cmd;
    cmd.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadFloatEq;
    cmd.A = ref ? ref : "";
    cmd.F = expected;
    cmd.G = epsilon;
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
