// Script-based tests for Rust consumers.
// This file is part of dear-imgui-rs and is licensed under MIT OR Apache-2.0.

#include <string>
#include <atomic>
#include <cmath>
#include <cstring>
#include <unordered_map>
#include <utility>
#include <vector>

#define IMGUI_DEFINE_MATH_OPERATORS
#include "imgui.h"
#include "imgui_internal.h"

#include "imgui_te_context.h"
#include "imgui_te_engine.h" // ImGuiTestEngine_RegisterTest()

#include "cimgui_test_engine.h"
#include "cimgui_test_engine_internal.h"

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
    unsigned int MouseButtons = 0;
};

namespace {

namespace abi = dear_imgui_test_engine_abi;

class SpinGuard {
public:
    explicit SpinGuard(std::atomic_flag& flag) noexcept : flag_(flag) {
        while (flag_.test_and_set(std::memory_order_acquire)) {
        }
    }

    ~SpinGuard() noexcept { flag_.clear(std::memory_order_release); }

private:
    std::atomic_flag& flag_;
};

enum class ScriptState { CallerOwned, EngineOwned };

struct ScriptEntry {
    ScriptState State = ScriptState::CallerOwned;
    ImGuiTestEngine* Owner = nullptr;
};

std::atomic_flag g_script_lock = ATOMIC_FLAG_INIT;
std::unordered_map<ImGuiTestEngineScript*, ScriptEntry> g_scripts;

static ImGuiTestEngineStatus require_mutable_script(ImGuiTestEngineScript* script) noexcept {
    if (script == nullptr) {
        return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "script must not be null");
    }
    SpinGuard guard(g_script_lock);
    const auto found = g_scripts.find(script);
    if (found == g_scripts.end()) {
        return abi::fail(ImGuiTestEngineStatus_InvalidState, "script is not live");
    }
    if (found->second.State != ScriptState::CallerOwned) {
        return abi::fail(ImGuiTestEngineStatus_InvalidState, "registered script is immutable");
    }
    return ImGuiTestEngineStatus_Success;
}

static void register_script(ImGuiTestEngineScript* script) {
    SpinGuard guard(g_script_lock);
    g_scripts.insert_or_assign(script, ScriptEntry{});
}

static void unregister_script(ImGuiTestEngineScript* script) noexcept {
    SpinGuard guard(g_script_lock);
    g_scripts.erase(script);
}

static void transfer_script(ImGuiTestEngineScript* script, ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_script_lock);
    const auto found = g_scripts.find(script);
    if (found != g_scripts.end()) {
        found->second = {ScriptState::EngineOwned, engine};
    }
}

static void script_free_for_engine(ImGuiTestEngine* engine) noexcept {
    SpinGuard guard(g_script_lock);
    for (auto entry = g_scripts.begin(); entry != g_scripts.end();) {
        if (entry->second.State != ScriptState::EngineOwned || entry->second.Owner != engine) {
            ++entry;
            continue;
        }
        delete entry->first;
        entry = g_scripts.erase(entry);
        abi::increment(abi::Counter::ScriptDestroyed);
    }
}

static void report_script_error(const char* operation, const char* detail) noexcept {
    ImGuiTestEngine_Error(
        __FILE__,
        operation,
        __LINE__,
        ImGuiTestCheckFlags_None,
        "Rust script command rejected: %s",
        detail
    );
}

static ImGuiTable* find_script_table(ImGuiTestContext* ctx, const std::string& table_ref) {
    const ImGuiID table_id = ctx->GetID(table_ref.c_str());
    return ImGui::TableFindByID(table_id);
}

static bool validate_table_command(
    ImGuiTestContext* ctx,
    const ImGuiTestEngineScript::Cmd& cmd
) {
    ImGuiTable* table = find_script_table(ctx, cmd.A);
    if (table == nullptr) {
        report_script_error("validate_table_command", "table was not found");
        return false;
    }

    switch (cmd.Kind) {
        case ImGuiTestEngineScript::CmdKind::TableClickHeader:
        case ImGuiTestEngineScript::CmdKind::TableSetColumnEnabledByLabel:
            for (int column = 0; column < table->ColumnsCount; ++column) {
                const char* name = ImGui::TableGetColumnName(table, column);
                if (name != nullptr && cmd.B == name) {
                    return true;
                }
            }
            report_script_error("validate_table_command", "table column label was not found");
            return false;
        case ImGuiTestEngineScript::CmdKind::TableOpenContextMenu:
            if (cmd.I == -1 || (cmd.I >= 0 && cmd.I < table->ColumnsCount)) {
                return true;
            }
            report_script_error("validate_table_command", "table column is out of range");
            return false;
        case ImGuiTestEngineScript::CmdKind::TableSetColumnEnabled:
        case ImGuiTestEngineScript::CmdKind::TableResizeColumn:
            if (cmd.I >= 0 && cmd.I < table->ColumnsCount) {
                return true;
            }
            report_script_error("validate_table_command", "table column is out of range");
            return false;
        default:
            return true;
    }
}

static bool command_may_submit_nav_move(ImGuiTestEngineScript::CmdKind kind) noexcept;
static bool validate_runtime_command(
    ImGuiTestContext* ctx,
    const ImGuiTestEngineScript::Cmd& cmd
);

static void script_test_func_impl(ImGuiTestContext* ctx) {
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
        if (!validate_runtime_command(ctx, cmd)) {
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

static void script_test_func(ImGuiTestContext* ctx) noexcept {
    try {
        script_test_func_impl(ctx);
    } catch (const std::exception& error) {
        report_script_error("script_test_func", error.what());
    } catch (...) {
        report_script_error("script_test_func", "unknown C++ exception");
    }
}

} // namespace

// Called from cimgui_test_engine.cpp after engine lifecycle validation.
namespace dear_imgui_test_engine_abi {

void cleanup_scripts(ImGuiTestEngine* engine) noexcept { script_free_for_engine(engine); }

bool has_live_scripts() noexcept {
    SpinGuard guard(g_script_lock);
    return !g_scripts.empty();
}

} // namespace dear_imgui_test_engine_abi

namespace {

static ImGuiTestEngineStatus required_string(
    std::string& destination,
    const char* value,
    bool allow_empty = true
) {
    if (value == nullptr) {
        return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "string argument must not be null");
    }
    if (!allow_empty && value[0] == '\0') {
        return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "string argument must not be empty");
    }
    destination = value;
    return ImGuiTestEngineStatus_Success;
}

static ImGuiTestEngineStatus required_ref(
    std::string& destination,
    const char* value,
    bool allow_empty = true
) {
    ImGuiTestEngineStatus status = required_string(destination, value, allow_empty);
    if (status != ImGuiTestEngineStatus_Success) {
        return status;
    }
    constexpr std::size_t max_ref_length =
        sizeof(((ImGuiTestContext*)nullptr)->RefStr) - 2;
    return destination.size() <= max_ref_length
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "test reference is too long");
}

static ImGuiTestEngineStatus finite_value(float value) noexcept {
    return std::isfinite(value)
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "floating-point argument must be finite");
}

static ImGuiTestEngineStatus nonnegative_value(float value) noexcept {
    const ImGuiTestEngineStatus status = finite_value(value);
    if (status != ImGuiTestEngineStatus_Success) {
        return status;
    }
    return value >= 0.0f
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "floating-point argument must not be negative");
}

static ImGuiTestEngineStatus positive_count(int value) noexcept {
    return value > 0
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "count or frame argument must be positive");
}

static ImGuiTestEngineStatus script_limit(int value) noexcept {
    return value == -1 || value > 0
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "script limit must be -1 or positive");
}

static ImGuiTestEngineStatus mouse_button(int value) noexcept {
    return value >= 0 && value < ImGuiMouseButton_COUNT
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "mouse button is out of range");
}

static ImGuiTestEngineStatus key_chord(int value) noexcept {
    if (value < 0 || (value & ~(ImGuiMod_Mask_ | 0x0fff)) != 0) {
        return abi::fail(ImGuiTestEngineStatus_OutOfRange, "key chord contains unknown bits");
    }
    const int key = value & ~ImGuiMod_Mask_;
    return key >= ImGuiKey_NamedKey_BEGIN && key < ImGuiKey_NamedKey_END
               ? ImGuiTestEngineStatus_Success
               : abi::fail(ImGuiTestEngineStatus_OutOfRange, "key chord has no valid named key");
}

static bool command_requires_released_mouse(ImGuiTestEngineScript::CmdKind kind) noexcept {
    using Kind = ImGuiTestEngineScript::CmdKind;
    switch (kind) {
        case Kind::SetRef:
        case Kind::ItemClick:
        case Kind::ItemClickWithButton:
        case Kind::ItemDoubleClick:
        case Kind::ItemOpen:
        case Kind::ItemClose:
        case Kind::ItemCheck:
        case Kind::ItemUncheck:
        case Kind::ItemInputInt:
        case Kind::ItemInputStr:
        case Kind::MouseClick:
        case Kind::MouseClickMulti:
        case Kind::MouseDoubleClick:
        case Kind::MouseDragWithDelta:
        case Kind::MouseClickOnVoid:
        case Kind::ItemHold:
        case Kind::ItemHoldForFrames:
        case Kind::ItemDragOverAndHold:
        case Kind::ItemDragAndDrop:
        case Kind::ItemDragWithDelta:
        case Kind::TabClose:
        case Kind::ComboClick:
        case Kind::ComboClickAll:
        case Kind::ItemOpenAll:
        case Kind::ItemCloseAll:
        case Kind::TableClickHeader:
        case Kind::TableOpenContextMenu:
        case Kind::TableSetColumnEnabled:
        case Kind::TableSetColumnEnabledByLabel:
        case Kind::TableResizeColumn:
        case Kind::MenuClick:
        case Kind::MenuCheck:
        case Kind::MenuUncheck:
        case Kind::MenuCheckAll:
        case Kind::MenuUncheckAll:
        case Kind::WindowClose:
        case Kind::WindowCollapse:
        case Kind::WindowFocus:
        case Kind::WindowMove:
        case Kind::WindowResize:
            return true;
        default:
            return false;
    }
}

static bool command_may_submit_nav_move(ImGuiTestEngineScript::CmdKind kind) noexcept {
    using Kind = ImGuiTestEngineScript::CmdKind;
    switch (kind) {
        case Kind::ItemClick:
        case Kind::ItemClickWithButton:
        case Kind::ItemDoubleClick:
        case Kind::ItemInputInt:
        case Kind::ItemInputStr:
        case Kind::NavMoveTo:
            return true;
        default:
            return false;
    }
}

static bool validate_runtime_command(
    ImGuiTestContext* ctx,
    const ImGuiTestEngineScript::Cmd& cmd
) {
    using Kind = ImGuiTestEngineScript::CmdKind;
    if ((cmd.Kind == Kind::TableClickHeader ||
         cmd.Kind == Kind::TableOpenContextMenu ||
         cmd.Kind == Kind::TableSetColumnEnabled ||
         cmd.Kind == Kind::TableSetColumnEnabledByLabel ||
         cmd.Kind == Kind::TableResizeColumn) &&
        !validate_table_command(ctx, cmd)) {
        return false;
    }

    if (ctx->UiContext->NavMoveSubmitted &&
        (cmd.Kind == Kind::NavMoveTo ||
         (ctx->InputMode != ImGuiInputSource_Mouse && command_may_submit_nav_move(cmd.Kind)))) {
        report_script_error("validate_runtime_command", "a navigation move is already pending");
        return false;
    }

    if (cmd.Kind == Kind::WindowMove) {
        ImGuiWindow* window = ctx->GetWindowByRef(cmd.A.c_str());
        if (window == nullptr) {
            report_script_error("validate_runtime_command", "window was not found");
            return false;
        }
#ifdef IMGUI_HAS_DOCK
        if (window->DockNode != nullptr && window->DockNode->TabBar != nullptr &&
            ImGui::TabBarFindTabByID(window->DockNode->TabBar, window->TabId) == nullptr) {
            report_script_error("validate_runtime_command", "docked window has no matching tab");
            return false;
        }
#endif
    }

    return true;
}

static ImGuiTestEngineStatus next_mouse_state(
    const ImGuiTestEngineScript& script,
    const ImGuiTestEngineScript::Cmd& command,
    unsigned int* out_state
) noexcept {
    unsigned int state = script.MouseButtons;
    const unsigned int button_mask =
        command.I >= 0 && command.I < ImGuiMouseButton_COUNT ? 1u << command.I : 0u;

    if (command.Kind == ImGuiTestEngineScript::CmdKind::MouseDown) {
        if ((state & button_mask) != 0) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "mouse button is already down");
        }
        state |= button_mask;
    } else if (command.Kind == ImGuiTestEngineScript::CmdKind::MouseUp) {
        if ((state & button_mask) == 0) {
            return abi::fail(ImGuiTestEngineStatus_InvalidState, "mouse button is not down");
        }
        state &= ~button_mask;
    } else if (command_requires_released_mouse(command.Kind) && state != 0) {
        return abi::fail(
            ImGuiTestEngineStatus_InvalidState,
            "command requires all scripted mouse buttons to be released"
        );
    }

    *out_state = state;
    return ImGuiTestEngineStatus_Success;
}

template <typename Configure>
ImGuiTestEngineStatus append_command(
    const char* operation,
    ImGuiTestEngineScript* script,
    Configure&& configure
) {
    return abi::boundary(operation, [&]() {
        const ImGuiTestEngineStatus live_status = require_mutable_script(script);
        if (live_status != ImGuiTestEngineStatus_Success) {
            return live_status;
        }
        ImGuiTestEngineScript::Cmd command;
        const ImGuiTestEngineStatus configure_status = configure(command);
        if (configure_status != ImGuiTestEngineStatus_Success) {
            return configure_status;
        }
        unsigned int next_state = 0;
        const ImGuiTestEngineStatus order_status = next_mouse_state(*script, command, &next_state);
        if (order_status != ImGuiTestEngineStatus_Success) {
            return order_status;
        }
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_ScriptVectorGrowth);
        script->Cmds.push_back(std::move(command));
        script->MouseButtons = next_state;
        return ImGuiTestEngineStatus_Success;
    });
}

static ImGuiTestEngineStatus append_ref_command(
    const char* operation,
    ImGuiTestEngineScript* script,
    ImGuiTestEngineScript::CmdKind kind,
    const char* ref
) {
    return append_command(operation, script, [&](ImGuiTestEngineScript::Cmd& command) {
        command.Kind = kind;
        return required_ref(command.A, ref);
    });
}

static ImGuiTestEngineStatus append_empty_command(
    const char* operation,
    ImGuiTestEngineScript* script,
    ImGuiTestEngineScript::CmdKind kind
) {
    return append_command(operation, script, [&](ImGuiTestEngineScript::Cmd& command) {
        command.Kind = kind;
        return ImGuiTestEngineStatus_Success;
    });
}

static ImGuiTestEngineStatus append_button_command(
    const char* operation,
    ImGuiTestEngineScript* script,
    ImGuiTestEngineScript::CmdKind kind,
    int button
) {
    return append_command(operation, script, [&](ImGuiTestEngineScript::Cmd& command) {
        const ImGuiTestEngineStatus status = mouse_button(button);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        command.Kind = kind;
        command.I = button;
        return ImGuiTestEngineStatus_Success;
    });
}

static ImGuiTestEngineStatus append_key_command(
    const char* operation,
    ImGuiTestEngineScript* script,
    ImGuiTestEngineScript::CmdKind kind,
    int chord
) {
    return append_command(operation, script, [&](ImGuiTestEngineScript::Cmd& command) {
        const ImGuiTestEngineStatus status = key_chord(chord);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        command.Kind = kind;
        command.I = chord;
        return ImGuiTestEngineStatus_Success;
    });
}

} // namespace

extern "C" {

ImGuiTestEngineStatus imgui_test_engine_script_create(ImGuiTestEngineScript** out_script) {
    return abi::boundary("imgui_test_engine_script_create", [&]() {
        if (out_script == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "out_script must not be null");
        }
        *out_script = nullptr;
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_ScriptAllocation);
        ImGuiTestEngineScript* script = new ImGuiTestEngineScript();
        try {
            register_script(script);
        } catch (...) {
            delete script;
            throw;
        }
        abi::increment(abi::Counter::ScriptCreated);
        *out_script = script;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_destroy(ImGuiTestEngineScript* script) {
    return abi::boundary("imgui_test_engine_script_destroy", [&]() {
        if (script == nullptr) {
            return abi::fail(ImGuiTestEngineStatus_InvalidArgument, "script must not be null");
        }
        const ImGuiTestEngineStatus status = require_mutable_script(script);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        delete script;
        unregister_script(script);
        abi::increment(abi::Counter::ScriptDestroyed);
        return ImGuiTestEngineStatus_Success;
    });
}

#define SCRIPT_REF_FUNCTION(function_name, kind_name)                                      \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, const char* ref) { \
        return append_ref_command(#function_name, script, ImGuiTestEngineScript::CmdKind::kind_name, ref); \
    }

#define SCRIPT_EMPTY_FUNCTION(function_name, kind_name)                         \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script) {        \
        return append_empty_command(                                            \
            #function_name, script, ImGuiTestEngineScript::CmdKind::kind_name   \
        );                                                                      \
    }

#define SCRIPT_BUTTON_FUNCTION(function_name, kind_name)                        \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, int button) { \
        return append_button_command(                                           \
            #function_name, script, ImGuiTestEngineScript::CmdKind::kind_name, button \
        );                                                                      \
    }

#define SCRIPT_KEY_FUNCTION(function_name, kind_name)                           \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, int chord) { \
        return append_key_command(                                              \
            #function_name, script, ImGuiTestEngineScript::CmdKind::kind_name, chord \
        );                                                                      \
    }

SCRIPT_REF_FUNCTION(imgui_test_engine_script_set_ref, SetRef)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_click, ItemClick)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_double_click, ItemDoubleClick)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_open, ItemOpen)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_close, ItemClose)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_check, ItemCheck)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_item_uncheck, ItemUncheck)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_mouse_move, MouseMove)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_scroll_to_item_x, ScrollToItemX)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_scroll_to_item_y, ScrollToItemY)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_scroll_to_top, ScrollToTop)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_scroll_to_bottom, ScrollToBottom)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_tab_close, TabClose)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_combo_click, ComboClick)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_combo_click_all, ComboClickAll)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_menu_click, MenuClick)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_menu_check, MenuCheck)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_menu_uncheck, MenuUncheck)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_menu_check_all, MenuCheckAll)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_menu_uncheck_all, MenuUncheckAll)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_nav_move_to, NavMoveTo)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_window_close, WindowClose)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_window_focus, WindowFocus)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_window_bring_to_front, WindowBringToFront)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_assert_item_exists, AssertItemExists)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_assert_item_visible, AssertItemVisible)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_assert_item_checked, AssertItemChecked)
SCRIPT_REF_FUNCTION(imgui_test_engine_script_assert_item_opened, AssertItemOpened)

SCRIPT_EMPTY_FUNCTION(imgui_test_engine_script_mouse_move_to_void, MouseMoveToVoid)
SCRIPT_EMPTY_FUNCTION(imgui_test_engine_script_nav_activate, NavActivate)
SCRIPT_EMPTY_FUNCTION(imgui_test_engine_script_nav_input, NavInput)

SCRIPT_BUTTON_FUNCTION(imgui_test_engine_script_mouse_click, MouseClick)
SCRIPT_BUTTON_FUNCTION(imgui_test_engine_script_mouse_double_click, MouseDoubleClick)
SCRIPT_BUTTON_FUNCTION(imgui_test_engine_script_mouse_down, MouseDown)
SCRIPT_BUTTON_FUNCTION(imgui_test_engine_script_mouse_up, MouseUp)
SCRIPT_BUTTON_FUNCTION(imgui_test_engine_script_mouse_lift_drag_threshold, MouseLiftDragThreshold)

SCRIPT_KEY_FUNCTION(imgui_test_engine_script_key_down, KeyDown)
SCRIPT_KEY_FUNCTION(imgui_test_engine_script_key_up, KeyUp)

#undef SCRIPT_REF_FUNCTION
#undef SCRIPT_EMPTY_FUNCTION
#undef SCRIPT_BUTTON_FUNCTION
#undef SCRIPT_KEY_FUNCTION

ImGuiTestEngineStatus imgui_test_engine_script_item_click_with_button(
    ImGuiTestEngineScript* script,
    const char* ref,
    int button
) {
    return append_command("imgui_test_engine_script_item_click_with_button", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) {
            status = mouse_button(button);
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemClickWithButton;
        command.I = button;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_input_int(
    ImGuiTestEngineScript* script,
    const char* ref,
    int value
) {
    return append_command("imgui_test_engine_script_item_input_int", script, [&](auto& command) {
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemInputInt;
        command.I = value;
        return required_ref(command.A, ref);
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_input_str(
    ImGuiTestEngineScript* script,
    const char* ref,
    const char* value
) {
    return append_command("imgui_test_engine_script_item_input_str", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) {
            status = required_string(command.B, value);
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemInputStr;
        return status;
    });
}

#define SCRIPT_POSITION_FUNCTION(function_name, kind_name)                      \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, float x, float y) { \
        return append_command(#function_name, script, [&](auto& command) {      \
            ImGuiTestEngineStatus status = finite_value(x);                    \
            if (status == ImGuiTestEngineStatus_Success) status = finite_value(y); \
            command.Kind = ImGuiTestEngineScript::CmdKind::kind_name;          \
            command.F = x;                                                      \
            command.G = y;                                                      \
            return status;                                                      \
        });                                                                     \
    }

SCRIPT_POSITION_FUNCTION(imgui_test_engine_script_mouse_move_to_pos, MouseMoveToPos)
SCRIPT_POSITION_FUNCTION(imgui_test_engine_script_mouse_teleport_to_pos, MouseTeleportToPos)
SCRIPT_POSITION_FUNCTION(imgui_test_engine_script_mouse_wheel, MouseWheel)

#undef SCRIPT_POSITION_FUNCTION

ImGuiTestEngineStatus imgui_test_engine_script_mouse_click_multi(
    ImGuiTestEngineScript* script,
    int button,
    int count
) {
    return append_command("imgui_test_engine_script_mouse_click_multi", script, [&](auto& command) {
        ImGuiTestEngineStatus status = mouse_button(button);
        if (status == ImGuiTestEngineStatus_Success) {
            status = positive_count(count);
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::MouseClickMulti;
        command.I = button;
        command.J = count;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_mouse_drag_with_delta(
    ImGuiTestEngineScript* script,
    float dx,
    float dy,
    int button
) {
    return append_command("imgui_test_engine_script_mouse_drag_with_delta", script, [&](auto& command) {
        ImGuiTestEngineStatus status = finite_value(dx);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(dy);
        if (status == ImGuiTestEngineStatus_Success) status = mouse_button(button);
        command.Kind = ImGuiTestEngineScript::CmdKind::MouseDragWithDelta;
        command.I = button;
        command.F = dx;
        command.G = dy;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_mouse_click_on_void(
    ImGuiTestEngineScript* script,
    int button,
    int count
) {
    return append_command("imgui_test_engine_script_mouse_click_on_void", script, [&](auto& command) {
        ImGuiTestEngineStatus status = mouse_button(button);
        if (status == ImGuiTestEngineStatus_Success) status = positive_count(count);
        command.Kind = ImGuiTestEngineScript::CmdKind::MouseClickOnVoid;
        command.I = button;
        command.J = count;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_key_press(
    ImGuiTestEngineScript* script,
    int chord,
    int count
) {
    return append_command("imgui_test_engine_script_key_press", script, [&](auto& command) {
        ImGuiTestEngineStatus status = key_chord(chord);
        if (status == ImGuiTestEngineStatus_Success) status = positive_count(count);
        command.Kind = ImGuiTestEngineScript::CmdKind::KeyPress;
        command.I = chord;
        command.J = count;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_key_hold(
    ImGuiTestEngineScript* script,
    int chord,
    float seconds
) {
    return append_command("imgui_test_engine_script_key_hold", script, [&](auto& command) {
        ImGuiTestEngineStatus status = key_chord(chord);
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(seconds);
        command.Kind = ImGuiTestEngineScript::CmdKind::KeyHold;
        command.I = chord;
        command.F = seconds;
        return status;
    });
}

#define SCRIPT_TEXT_FUNCTION(function_name, kind_name)                          \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, const char* chars) { \
        return append_command(#function_name, script, [&](auto& command) {      \
            command.Kind = ImGuiTestEngineScript::CmdKind::kind_name;          \
            return required_string(command.A, chars);                          \
        });                                                                     \
    }

SCRIPT_TEXT_FUNCTION(imgui_test_engine_script_key_chars, KeyChars)
SCRIPT_TEXT_FUNCTION(imgui_test_engine_script_key_chars_append, KeyCharsAppend)
SCRIPT_TEXT_FUNCTION(imgui_test_engine_script_key_chars_append_enter, KeyCharsAppendEnter)
SCRIPT_TEXT_FUNCTION(imgui_test_engine_script_key_chars_replace, KeyCharsReplace)
SCRIPT_TEXT_FUNCTION(imgui_test_engine_script_key_chars_replace_enter, KeyCharsReplaceEnter)

#undef SCRIPT_TEXT_FUNCTION

ImGuiTestEngineStatus imgui_test_engine_script_item_hold(
    ImGuiTestEngineScript* script,
    const char* ref,
    float seconds
) {
    return append_command("imgui_test_engine_script_item_hold", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(seconds);
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemHold;
        command.F = seconds;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_hold_for_frames(
    ImGuiTestEngineScript* script,
    const char* ref,
    int frames
) {
    return append_command("imgui_test_engine_script_item_hold_for_frames", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = positive_count(frames);
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemHoldForFrames;
        command.I = frames;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_drag_over_and_hold(
    ImGuiTestEngineScript* script,
    const char* source,
    const char* destination
) {
    return append_command("imgui_test_engine_script_item_drag_over_and_hold", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, source);
        if (status == ImGuiTestEngineStatus_Success) status = required_ref(command.B, destination);
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemDragOverAndHold;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_drag_and_drop(
    ImGuiTestEngineScript* script,
    const char* source,
    const char* destination,
    int button
) {
    return append_command("imgui_test_engine_script_item_drag_and_drop", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, source);
        if (status == ImGuiTestEngineStatus_Success) status = required_ref(command.B, destination);
        if (status == ImGuiTestEngineStatus_Success) status = mouse_button(button);
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemDragAndDrop;
        command.I = button;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_item_drag_with_delta(
    ImGuiTestEngineScript* script,
    const char* ref,
    float dx,
    float dy
) {
    return append_command("imgui_test_engine_script_item_drag_with_delta", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(dx);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(dy);
        command.Kind = ImGuiTestEngineScript::CmdKind::ItemDragWithDelta;
        command.F = dx;
        command.G = dy;
        return status;
    });
}

#define SCRIPT_REF_FLOAT_FUNCTION(function_name, kind_name)                     \
    ImGuiTestEngineStatus function_name(ImGuiTestEngineScript* script, const char* ref, float value) { \
        return append_command(#function_name, script, [&](auto& command) {      \
            ImGuiTestEngineStatus status = required_ref(command.A, ref);       \
            if (status == ImGuiTestEngineStatus_Success) status = finite_value(value); \
            command.Kind = ImGuiTestEngineScript::CmdKind::kind_name;          \
            command.F = value;                                                  \
            return status;                                                      \
        });                                                                     \
    }

SCRIPT_REF_FLOAT_FUNCTION(imgui_test_engine_script_scroll_to_x, ScrollToX)
SCRIPT_REF_FLOAT_FUNCTION(imgui_test_engine_script_scroll_to_y, ScrollToY)
SCRIPT_REF_FLOAT_FUNCTION(imgui_test_engine_script_scroll_to_pos_x, ScrollToPosX)
SCRIPT_REF_FLOAT_FUNCTION(imgui_test_engine_script_scroll_to_pos_y, ScrollToPosY)

#undef SCRIPT_REF_FLOAT_FUNCTION

#define SCRIPT_TREE_FUNCTION(function_name, kind_name)                          \
    ImGuiTestEngineStatus function_name(                                        \
        ImGuiTestEngineScript* script, const char* ref, int depth, int passes   \
    ) {                                                                         \
        return append_command(#function_name, script, [&](auto& command) {      \
            ImGuiTestEngineStatus status = required_ref(command.A, ref);       \
            if (status == ImGuiTestEngineStatus_Success) status = script_limit(depth); \
            if (status == ImGuiTestEngineStatus_Success) status = script_limit(passes); \
            command.Kind = ImGuiTestEngineScript::CmdKind::kind_name;          \
            command.I = depth;                                                  \
            command.J = passes;                                                 \
            return status;                                                      \
        });                                                                     \
    }

SCRIPT_TREE_FUNCTION(imgui_test_engine_script_item_open_all, ItemOpenAll)
SCRIPT_TREE_FUNCTION(imgui_test_engine_script_item_close_all, ItemCloseAll)

#undef SCRIPT_TREE_FUNCTION

ImGuiTestEngineStatus imgui_test_engine_script_table_click_header(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    const char* label,
    int key_mods
) {
    return append_command("imgui_test_engine_script_table_click_header", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, table_ref, false);
        if (status == ImGuiTestEngineStatus_Success) status = required_string(command.B, label, false);
        if (status == ImGuiTestEngineStatus_Success &&
            (key_mods < 0 || (key_mods & ~ImGuiMod_Mask_) != 0)) {
            status = abi::fail(ImGuiTestEngineStatus_OutOfRange, "key_mods contains non-modifier bits");
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::TableClickHeader;
        command.I = key_mods;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_table_open_context_menu(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    int column
) {
    return append_command("imgui_test_engine_script_table_open_context_menu", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, table_ref, false);
        if (status == ImGuiTestEngineStatus_Success && column < -1) {
            status = abi::fail(ImGuiTestEngineStatus_OutOfRange, "column must be -1 or nonnegative");
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::TableOpenContextMenu;
        command.I = column;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_table_set_column_enabled(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    int column,
    bool enabled
) {
    return append_command("imgui_test_engine_script_table_set_column_enabled", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, table_ref, false);
        if (status == ImGuiTestEngineStatus_Success && column < 0) {
            status = abi::fail(ImGuiTestEngineStatus_OutOfRange, "column must be nonnegative");
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::TableSetColumnEnabled;
        command.I = column;
        command.J = enabled ? 1 : 0;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_table_set_column_enabled_by_label(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    const char* label,
    bool enabled
) {
    return append_command(
        "imgui_test_engine_script_table_set_column_enabled_by_label",
        script,
        [&](auto& command) {
            ImGuiTestEngineStatus status = required_ref(command.A, table_ref, false);
            if (status == ImGuiTestEngineStatus_Success) status = required_string(command.B, label, false);
            command.Kind = ImGuiTestEngineScript::CmdKind::TableSetColumnEnabledByLabel;
            command.I = enabled ? 1 : 0;
            return status;
        }
    );
}

ImGuiTestEngineStatus imgui_test_engine_script_table_resize_column(
    ImGuiTestEngineScript* script,
    const char* table_ref,
    int column,
    float width
) {
    return append_command("imgui_test_engine_script_table_resize_column", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, table_ref, false);
        if (status == ImGuiTestEngineStatus_Success && column < 0) {
            status = abi::fail(ImGuiTestEngineStatus_OutOfRange, "column must be nonnegative");
        }
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(width);
        command.Kind = ImGuiTestEngineScript::CmdKind::TableResizeColumn;
        command.I = column;
        command.F = width;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_set_input_mode(
    ImGuiTestEngineScript* script,
    int input_source
) {
    return append_command("imgui_test_engine_script_set_input_mode", script, [&](auto& command) {
        if (input_source <= ImGuiInputSource_None || input_source >= ImGuiInputSource_COUNT) {
            return abi::fail(ImGuiTestEngineStatus_OutOfRange, "input source is out of range");
        }
        command.Kind = ImGuiTestEngineScript::CmdKind::SetInputMode;
        command.I = input_source;
        return ImGuiTestEngineStatus_Success;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_window_collapse(
    ImGuiTestEngineScript* script,
    const char* ref,
    bool collapsed
) {
    return append_command("imgui_test_engine_script_window_collapse", script, [&](auto& command) {
        command.Kind = ImGuiTestEngineScript::CmdKind::WindowCollapse;
        command.I = collapsed ? 1 : 0;
        return required_ref(command.A, ref);
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_window_move(
    ImGuiTestEngineScript* script,
    const char* ref,
    float x,
    float y
) {
    return append_command("imgui_test_engine_script_window_move", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(x);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(y);
        command.Kind = ImGuiTestEngineScript::CmdKind::WindowMove;
        command.F = x;
        command.G = y;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_window_resize(
    ImGuiTestEngineScript* script,
    const char* ref,
    float width,
    float height
) {
    return append_command("imgui_test_engine_script_window_resize", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(width);
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(height);
        command.Kind = ImGuiTestEngineScript::CmdKind::WindowResize;
        command.F = width;
        command.G = height;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_sleep(
    ImGuiTestEngineScript* script,
    float seconds
) {
    return append_command("imgui_test_engine_script_sleep", script, [&](auto& command) {
        command.Kind = ImGuiTestEngineScript::CmdKind::Sleep;
        command.F = seconds;
        return nonnegative_value(seconds);
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_int_eq(
    ImGuiTestEngineScript* script,
    const char* ref,
    int expected
) {
    return append_command("imgui_test_engine_script_assert_item_read_int_eq", script, [&](auto& command) {
        command.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadIntEq;
        command.I = expected;
        return required_ref(command.A, ref);
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_str_eq(
    ImGuiTestEngineScript* script,
    const char* ref,
    const char* expected
) {
    return append_command("imgui_test_engine_script_assert_item_read_str_eq", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = required_string(command.B, expected);
        command.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadStrEq;
        return status;
    });
}

ImGuiTestEngineStatus imgui_test_engine_script_assert_item_read_float_eq(
    ImGuiTestEngineScript* script,
    const char* ref,
    float expected,
    float epsilon
) {
    return append_command("imgui_test_engine_script_assert_item_read_float_eq", script, [&](auto& command) {
        ImGuiTestEngineStatus status = required_ref(command.A, ref);
        if (status == ImGuiTestEngineStatus_Success) status = finite_value(expected);
        if (status == ImGuiTestEngineStatus_Success) status = nonnegative_value(epsilon);
        command.Kind = ImGuiTestEngineScript::CmdKind::AssertItemReadFloatEq;
        command.F = expected;
        command.G = epsilon;
        return status;
    });
}

#define SCRIPT_WAIT_FUNCTION(function_name, kind_name)                          \
    ImGuiTestEngineStatus function_name(                                        \
        ImGuiTestEngineScript* script, const char* ref, int frames              \
    ) {                                                                         \
        return append_command(#function_name, script, [&](auto& command) {      \
            ImGuiTestEngineStatus status = required_ref(command.A, ref);       \
            if (status == ImGuiTestEngineStatus_Success) status = positive_count(frames); \
            command.Kind = ImGuiTestEngineScript::CmdKind::kind_name;          \
            command.I = frames;                                                 \
            return status;                                                      \
        });                                                                     \
    }

SCRIPT_WAIT_FUNCTION(imgui_test_engine_script_wait_for_item, WaitForItem)
SCRIPT_WAIT_FUNCTION(imgui_test_engine_script_wait_for_item_visible, WaitForItemVisible)
SCRIPT_WAIT_FUNCTION(imgui_test_engine_script_wait_for_item_checked, WaitForItemChecked)
SCRIPT_WAIT_FUNCTION(imgui_test_engine_script_wait_for_item_opened, WaitForItemOpened)

#undef SCRIPT_WAIT_FUNCTION

ImGuiTestEngineStatus imgui_test_engine_script_yield(ImGuiTestEngineScript* script, int frames) {
    return append_command("imgui_test_engine_script_yield", script, [&](auto& command) {
        command.Kind = ImGuiTestEngineScript::CmdKind::Yield;
        command.I = frames;
        return positive_count(frames);
    });
}

ImGuiTestEngineStatus imgui_test_engine_register_script_test(
    ImGuiTestEngine* engine,
    const char* category,
    const char* name,
    ImGuiTestEngineScript* script
) {
    return abi::boundary("imgui_test_engine_register_script_test", [&]() {
        ImGuiTestEngineStatus status = abi::require_engine(engine);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        status = require_mutable_script(script);
        if (status != ImGuiTestEngineStatus_Success) {
            return status;
        }
        if (category == nullptr || category[0] == '\0' || name == nullptr || name[0] == '\0') {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidArgument,
                "category and name must not be null or empty"
            );
        }
        if (ImGuiTestEngine_GetIO(engine).IsRunningTests) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "script tests cannot be registered while tests are running"
            );
        }
        if (script->MouseButtons != 0) {
            return abi::fail(
                ImGuiTestEngineStatus_InvalidState,
                "script cannot be registered with pressed mouse buttons"
            );
        }

        script->Category = category;
        abi::maybe_inject(ImGuiTestEngineExceptionPoint_UpstreamCall);
        ImGuiTest* test = nullptr;
        try {
            test = ImGuiTestEngine_RegisterTest(
                engine,
                script->Category.c_str(),
                name,
                __FILE__,
                __LINE__
            );
            if (test == nullptr) {
                return abi::fail(ImGuiTestEngineStatus_Exception, "upstream test registration failed");
            }
            test->SetOwnedName(name);
            test->UserData = script;
            test->GuiFunc = nullptr;
            test->TestFunc = script_test_func;
        } catch (...) {
            if (test != nullptr) {
                ImGuiTestEngine_UnregisterTest(engine, test);
            }
            throw;
        }

        transfer_script(script, engine);
        abi::increment(abi::Counter::ScriptRegistered);
        return ImGuiTestEngineStatus_Success;
    });
}

} // extern "C"
