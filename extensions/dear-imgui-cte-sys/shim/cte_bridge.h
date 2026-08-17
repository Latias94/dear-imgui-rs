#ifndef DEAR_IMGUI_CTE_BRIDGE_H
#define DEAR_IMGUI_CTE_BRIDGE_H

#include "cimCTE.h"
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#define DEAR_IMGUI_CTE_NOEXCEPT noexcept
#else
#define DEAR_IMGUI_CTE_NOEXCEPT
#endif

typedef enum DearImGuiCteStatus_ {
    DearImGuiCteStatus_Ok = 0,
    DearImGuiCteStatus_NullArgument = 1,
    DearImGuiCteStatus_InvalidValue = 2,
    DearImGuiCteStatus_CallbackFailed = 3,
} DearImGuiCteStatus;

typedef struct DearImGuiCteAutocompleteConfig DearImGuiCteAutocompleteConfig;
typedef struct DearImGuiCteAutocompleteState DearImGuiCteAutocompleteState;

typedef struct DearImGuiCteAutocompleteContextView {
    bool in_identifier;
    bool in_number;
    bool in_comment;
    bool in_string;
    const Language* language;
    void* application_userdata;
} DearImGuiCteAutocompleteContextView;

typedef struct DearImGuiCteChangeView {
    bool insert;
    DocPos_c start;
    DocPos_c end;
    const char* text;
    size_t text_len;
} DearImGuiCteChangeView;

typedef void (*DearImGuiCteChangeCallback)(void* userdata);
typedef void (*DearImGuiCteTransactionCallback)(
    void* userdata,
    const DearImGuiCteChangeView* change);
typedef void* (*DearImGuiCteInsertCallback)(void* userdata, size_t line);
typedef void (*DearImGuiCteDeleteCallback)(void* userdata, size_t line, void* line_data);
typedef void (*DearImGuiCteLineDataCallback)(void* userdata, size_t line, void* line_data);
typedef void (*DearImGuiCteDecoratorCallback)(void* userdata, Decorator* decorator);
typedef void (*DearImGuiCteCaretCallback)(void* userdata, const CustomCaret* caret);
typedef void (*DearImGuiCtePopupCallback)(void* userdata, PopupData* popup);
typedef void (*DearImGuiCteIdentifierCallback)(
    void* userdata,
    const char* identifier,
    size_t identifier_len);
typedef DearImGuiCteStatus (*DearImGuiCteFilterCallback)(
    void* userdata,
    const char* input,
    size_t input_len,
    const char** output,
    size_t* output_len);
typedef void (*DearImGuiCteAutocompleteCallback)(
    void* userdata,
    DearImGuiCteAutocompleteState* state);

/* Bridge-owned editor functions require pointers returned by dear_imgui_cte_text_editor_create. */
CIMGUI_API TextEditor*
dear_imgui_cte_text_editor_create(void) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API void dear_imgui_cte_text_editor_destroy(
    TextEditor* editor) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_text_editor_set_change_callback(
    TextEditor* editor,
    DearImGuiCteChangeCallback callback,
    void* userdata,
    int delay_ms) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_text_editor_reset_autocomplete(
    TextEditor* editor) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_text_editor_clear_callbacks(
    TextEditor* editor) DEAR_IMGUI_CTE_NOEXCEPT;

CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_change_callback(
    TextEditor* editor,
    DearImGuiCteChangeCallback callback,
    void* userdata,
    int delay_ms) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_transaction_callback(
    TextEditor* editor,
    DearImGuiCteTransactionCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_insert_callback(
    TextEditor* editor,
    DearImGuiCteInsertCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_delete_callback(
    TextEditor* editor,
    DearImGuiCteDeleteCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_iterate_line_data(
    TextEditor* editor,
    DearImGuiCteLineDataCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_line_decorator(
    TextEditor* editor,
    size_t width,
    DearImGuiCteDecoratorCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_custom_caret_callback(
    TextEditor* editor,
    DearImGuiCteCaretCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_line_number_context_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_text_context_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_text_hover_callback(
    TextEditor* editor,
    DearImGuiCtePopupCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_set_language_change_callback(
    TextEditor* editor,
    DearImGuiCteChangeCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_iterate_identifiers(
    TextEditor* editor,
    DearImGuiCteIdentifierCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_filter_selections(
    TextEditor* editor,
    DearImGuiCteFilterCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_filter_lines(
    TextEditor* editor,
    DearImGuiCteFilterCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_clear_callbacks(
    TextEditor* editor) DEAR_IMGUI_CTE_NOEXCEPT;

CIMGUI_API DearImGuiCteAutocompleteConfig*
dear_imgui_cte_autocomplete_config_create(void) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API void dear_imgui_cte_autocomplete_config_destroy(
    DearImGuiCteAutocompleteConfig* config) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_triggers(
    DearImGuiCteAutocompleteConfig* config,
    bool on_typing,
    bool on_shortcut,
    bool in_comments,
    bool in_strings) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_shortcut(
    DearImGuiCteAutocompleteConfig* config,
    ImGuiKeyChord shortcut) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_auto_insert_single(
    DearImGuiCteAutocompleteConfig* config,
    bool enabled) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_trigger_delay(
    DearImGuiCteAutocompleteConfig* config,
    uint64_t delay_ms) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_no_suggestions_label(
    DearImGuiCteAutocompleteConfig* config,
    const char* label,
    size_t label_len) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_suggestion_width(
    DearImGuiCteAutocompleteConfig* config,
    size_t width) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_config_set_callback(
    DearImGuiCteAutocompleteConfig* config,
    DearImGuiCteAutocompleteCallback callback,
    void* userdata) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_text_editor_set_autocomplete_config(
    TextEditor* editor,
    const DearImGuiCteAutocompleteConfig* config) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_text_editor_set_autocomplete_suggestions(
    TextEditor* editor,
    const char* const* suggestions,
    const size_t* suggestion_lengths,
    size_t suggestion_count) DEAR_IMGUI_CTE_NOEXCEPT;

CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_search_term(
    const DearImGuiCteAutocompleteState* state,
    const char** text,
    size_t* text_len) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_range(
    const DearImGuiCteAutocompleteState* state,
    DocPos_c* start,
    DocPos_c* end) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_get_context(
    const DearImGuiCteAutocompleteState* state,
    DearImGuiCteAutocompleteContextView* context) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_clear_suggestions(
    DearImGuiCteAutocompleteState* state) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_add_suggestion(
    DearImGuiCteAutocompleteState* state,
    const char* suggestion,
    size_t suggestion_len) DEAR_IMGUI_CTE_NOEXCEPT;
CIMGUI_API DearImGuiCteStatus dear_imgui_cte_autocomplete_state_set_promise(
    DearImGuiCteAutocompleteState* state,
    bool promised) DEAR_IMGUI_CTE_NOEXCEPT;

#undef DEAR_IMGUI_CTE_NOEXCEPT
#endif
