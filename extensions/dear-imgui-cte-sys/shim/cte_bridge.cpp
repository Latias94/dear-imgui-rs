#include "imgui.h"
#include "imgui_internal.h"
#include "ImGuiColorTextEdit/TextEditor.h"
#include "ImGuiColorTextEdit/TextDiff.h"
#include "ImGuiColorTextEdit/extras/TrieAutoComplete.h"
#include "ImGuiColorTextEdit/extras/Notifications.h"
#include "ImGuiColorTextEdit/example/dejavu.h"
#include "cte_bridge.h"

#include <chrono>
#include <limits>
#include <new>
#include <string>
#include <string_view>
#include <vector>

struct DearImGuiCteAutocompleteConfig {
    TextEditor::AutoCompleteConfig value;
};

namespace {

bool valid_bytes(const char* data, size_t length) noexcept {
    return data != nullptr || length == 0;
}

TextEditor::AutoCompleteState* autocomplete_state(
    DearImGuiCteAutocompleteState* state) noexcept {
    return reinterpret_cast<TextEditor::AutoCompleteState*>(state);
}

const TextEditor::AutoCompleteState* autocomplete_state(
    const DearImGuiCteAutocompleteState* state) noexcept {
    return reinterpret_cast<const TextEditor::AutoCompleteState*>(state);
}

} // namespace

DearImGuiCteStatus dear_imgui_cte_set_change_callback(
    TextEditor* editor,
    DearImGuiCteChangeCallback callback,
    void* userdata,
    int delay_ms) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (delay_ms < 0) {
        return DearImGuiCteStatus_InvalidValue;
    }
    if (callback == nullptr) {
        editor->SetChangeCallback(nullptr, delay_ms);
    } else {
        editor->SetChangeCallback([callback, userdata]() { callback(userdata); }, delay_ms);
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_transaction_callback(
    TextEditor* editor,
    DearImGuiCteTransactionCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->SetTransactionCallback(nullptr);
    } else {
        editor->SetTransactionCallback(
            [callback, userdata](const std::vector<TextEditor::Change>& changes) {
                for (const auto& change : changes) {
                    const DearImGuiCteChangeView view{
                        change.insert,
                        {change.start.line, change.start.index},
                        {change.end.line, change.end.index},
                        change.text.data(),
                        change.text.size(),
                    };
                    callback(userdata, &view);
                }
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_insert_callback(
    TextEditor* editor,
    DearImGuiCteInsertCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->SetInsertor(nullptr);
    } else {
        editor->SetInsertor(
            [callback, userdata](size_t line) { return callback(userdata, line); });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_delete_callback(
    TextEditor* editor,
    DearImGuiCteDeleteCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->SetDeletor(nullptr);
    } else {
        editor->SetDeletor([callback, userdata](size_t line, void* line_data) {
            callback(userdata, line, line_data);
        });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_iterate_line_data(
    TextEditor* editor,
    DearImGuiCteLineDataCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr || callback == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    editor->IterateUserData([callback, userdata](size_t line, void* line_data) {
        callback(userdata, line, line_data);
    });
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_line_decorator(
    TextEditor* editor,
    size_t width,
    DearImGuiCteDecoratorCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->ClearLineDecorator();
    } else {
        editor->SetLineDecorator(
            width,
            [callback, userdata](TextEditor::Decorator& decorator) {
                callback(userdata, &decorator);
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_custom_caret_callback(
    TextEditor* editor,
    DearImGuiCteCaretCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->ClearCustomCaretRenderer();
    } else {
        editor->SetCustomCaretRenderer(
            [callback, userdata](const TextEditor::CustomCaret& caret) {
                callback(userdata, &caret);
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_line_number_context_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->ClearLineNumberContextMenuCallback();
    } else {
        editor->SetLineNumberContextMenuCallback(
            [callback, userdata](TextEditor::PopupData& popup) {
                callback(userdata, &popup);
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_text_context_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->ClearTextContextMenuCallback();
    } else {
        editor->SetTextContextMenuCallback(
            [callback, userdata](TextEditor::PopupData& popup) {
                callback(userdata, &popup);
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_text_hover_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->ClearTextHoverCallback();
    } else {
        editor->SetTextHoverCallback(
            [callback, userdata](TextEditor::PopupData& popup) {
                callback(userdata, &popup);
            });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_set_language_change_callback(
    TextEditor* editor,
    DearImGuiCteChangeCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (callback == nullptr) {
        editor->SetLanguageChangeCallback(nullptr);
    } else {
        editor->SetLanguageChangeCallback([callback, userdata]() { callback(userdata); });
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_iterate_identifiers(
    TextEditor* editor,
    DearImGuiCteIdentifierCallback callback,
    void* userdata) noexcept {
    if (editor == nullptr || callback == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    editor->IterateIdentifiers([callback, userdata](const std::string& identifier) {
        callback(userdata, identifier.data(), identifier.size());
    });
    return DearImGuiCteStatus_Ok;
}

namespace {

DearImGuiCteStatus filter_text(
    TextEditor* editor,
    DearImGuiCteFilterCallback callback,
    void* userdata,
    bool lines) noexcept {
    if (editor == nullptr || callback == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    DearImGuiCteStatus callback_status = DearImGuiCteStatus_Ok;
    const auto filter = [callback, userdata, &callback_status](std::string_view input) {
        if (callback_status != DearImGuiCteStatus_Ok) {
            return std::string(input);
        }
        const char* output = nullptr;
        size_t output_len = 0;
        callback_status = callback(
            userdata,
            input.data(),
            input.size(),
            &output,
            &output_len);
        if (callback_status != DearImGuiCteStatus_Ok) {
            return std::string(input);
        }
        if (!valid_bytes(output, output_len)) {
            callback_status = DearImGuiCteStatus_InvalidValue;
            return std::string(input);
        }
        return output_len == 0 ? std::string() : std::string(output, output_len);
    };
    if (lines) {
        editor->FilterLines(filter);
    } else {
        editor->FilterSelections(filter);
    }
    return callback_status;
}

} // namespace

DearImGuiCteStatus dear_imgui_cte_filter_selections(
    TextEditor* editor,
    DearImGuiCteFilterCallback callback,
    void* userdata) noexcept {
    return filter_text(editor, callback, userdata, false);
}

DearImGuiCteStatus dear_imgui_cte_filter_lines(
    TextEditor* editor,
    DearImGuiCteFilterCallback callback,
    void* userdata) noexcept {
    return filter_text(editor, callback, userdata, true);
}

DearImGuiCteStatus dear_imgui_cte_clear_callbacks(TextEditor* editor) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    editor->SetChangeCallback(nullptr);
    editor->SetTransactionCallback(nullptr);
    editor->SetInsertor(nullptr);
    editor->SetDeletor(nullptr);
    editor->ClearLineDecorator();
    editor->ClearCustomCaretRenderer();
    editor->ClearLineNumberContextMenuCallback();
    editor->ClearTextContextMenuCallback();
    editor->ClearTextHoverCallback();
    editor->SetLanguageChangeCallback(nullptr);
    editor->SetAutoCompleteConfig(nullptr);
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteAutocompleteConfig* dear_imgui_cte_autocomplete_config_create(void) noexcept {
    return new (std::nothrow) DearImGuiCteAutocompleteConfig{};
}

void dear_imgui_cte_autocomplete_config_destroy(
    DearImGuiCteAutocompleteConfig* config) noexcept {
    delete config;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_triggers(
    DearImGuiCteAutocompleteConfig* config,
    bool on_typing,
    bool on_shortcut,
    bool in_comments,
    bool in_strings) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    config->value.triggerOnTyping = on_typing;
    config->value.triggerOnShortcut = on_shortcut;
    config->value.triggerInComments = in_comments;
    config->value.triggerInStrings = in_strings;
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_shortcut(
    DearImGuiCteAutocompleteConfig* config,
    ImGuiKeyChord shortcut) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    config->value.triggerShortcut = shortcut;
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_auto_insert_single(
    DearImGuiCteAutocompleteConfig* config,
    bool enabled) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    config->value.autoInsertSingleSuggestions = enabled;
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_trigger_delay(
    DearImGuiCteAutocompleteConfig* config,
    uint64_t delay_ms) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    using Milliseconds = std::chrono::milliseconds;
    if (delay_ms > static_cast<uint64_t>(std::numeric_limits<Milliseconds::rep>::max())) {
        return DearImGuiCteStatus_InvalidValue;
    }
    config->value.triggerDelay = Milliseconds(static_cast<Milliseconds::rep>(delay_ms));
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_no_suggestions_label(
    DearImGuiCteAutocompleteConfig* config,
    const char* label,
    size_t label_len) noexcept {
    if (config == nullptr || !valid_bytes(label, label_len)) {
        return DearImGuiCteStatus_NullArgument;
    }
    config->value.noSuggestionsLabel.assign(label_len == 0 ? "" : label, label_len);
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_suggestion_width(
    DearImGuiCteAutocompleteConfig* config,
    size_t width) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (width == 0) {
        return DearImGuiCteStatus_InvalidValue;
    }
    config->value.suggestionWidth = width;
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_callback(
    DearImGuiCteAutocompleteConfig* config,
    DearImGuiCteAutocompleteCallback callback,
    void* userdata) noexcept {
    if (config == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    config->value.userData = userdata;
    if (callback == nullptr) {
        config->value.callback = nullptr;
    } else {
        config->value.callback =
            [callback, userdata](TextEditor::AutoCompleteState& state) {
                callback(
                    userdata,
                    reinterpret_cast<DearImGuiCteAutocompleteState*>(&state));
            };
    }
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_text_editor_set_autocomplete_config(
    TextEditor* editor,
    const DearImGuiCteAutocompleteConfig* config) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    editor->SetAutoCompleteConfig(config == nullptr ? nullptr : &config->value);
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_text_editor_set_autocomplete_suggestions(
    TextEditor* editor,
    const char* const* suggestions,
    const size_t* suggestion_lengths,
    size_t suggestion_count) noexcept {
    if (editor == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    if (suggestion_count != 0 && (suggestions == nullptr || suggestion_lengths == nullptr)) {
        return DearImGuiCteStatus_NullArgument;
    }
    std::vector<std::string> owned;
    owned.reserve(suggestion_count);
    for (size_t index = 0; index < suggestion_count; ++index) {
        if (!valid_bytes(suggestions[index], suggestion_lengths[index])) {
            return DearImGuiCteStatus_NullArgument;
        }
        owned.emplace_back(
            suggestion_lengths[index] == 0 ? "" : suggestions[index],
            suggestion_lengths[index]);
    }
    editor->SetAutoCompleteSuggestions(owned);
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_search_term(
    const DearImGuiCteAutocompleteState* state,
    const char** text,
    size_t* text_len) noexcept {
    if (state == nullptr || text == nullptr || text_len == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    const auto* value = autocomplete_state(state);
    *text = value->searchTerm.data();
    *text_len = value->searchTerm.size();
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_range(
    const DearImGuiCteAutocompleteState* state,
    DocPos_c* start,
    DocPos_c* end) noexcept {
    if (state == nullptr || start == nullptr || end == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    const auto* value = autocomplete_state(state);
    *start = {value->searchTermStart.line, value->searchTermStart.index};
    *end = {value->searchTermEnd.line, value->searchTermEnd.index};
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_context(
    const DearImGuiCteAutocompleteState* state,
    DearImGuiCteAutocompleteContextView* context) noexcept {
    if (state == nullptr || context == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    const auto* value = autocomplete_state(state);
    *context = {
        value->inIdentifier,
        value->inNumber,
        value->inComment,
        value->inString,
        value->language,
        value->userData,
    };
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_clear_suggestions(
    DearImGuiCteAutocompleteState* state) noexcept {
    if (state == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    autocomplete_state(state)->suggestions.clear();
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_add_suggestion(
    DearImGuiCteAutocompleteState* state,
    const char* suggestion,
    size_t suggestion_len) noexcept {
    if (state == nullptr || !valid_bytes(suggestion, suggestion_len)) {
        return DearImGuiCteStatus_NullArgument;
    }
    autocomplete_state(state)->suggestions.emplace_back(
        suggestion_len == 0 ? "" : suggestion,
        suggestion_len);
    return DearImGuiCteStatus_Ok;
}

DearImGuiCteStatus dear_imgui_cte_autocomplete_state_set_promise(
    DearImGuiCteAutocompleteState* state,
    bool promised) noexcept {
    if (state == nullptr) {
        return DearImGuiCteStatus_NullArgument;
    }
    autocomplete_state(state)->suggestionsPromise = promised;
    return DearImGuiCteStatus_Ok;
}
