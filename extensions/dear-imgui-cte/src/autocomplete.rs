use crate::{
    CteError, CteResult, Language, Position, Selection,
    callbacks::{AutocompleteCallback, CallbackSlot, abort_on_panic, check_status},
    error::c_string,
    sys,
    text_editor::TextEditor,
};
use dear_imgui_rs::{Key, KeyChord, KeyMods};
use std::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    ptr::{self, NonNull},
    slice, str,
    time::Duration,
};

const MAX_AUTOCOMPLETE_DELAY_MS: u128 = i64::MAX as u128;

/// Configuration copied into an editor when autocomplete is installed.
#[must_use]
#[derive(Clone, Debug)]
pub struct AutocompleteConfig {
    trigger_on_typing: bool,
    trigger_on_shortcut: bool,
    trigger_in_comments: bool,
    trigger_in_strings: bool,
    shortcut: KeyChord,
    auto_insert_single_suggestion: bool,
    trigger_delay: Duration,
    no_suggestions_label: String,
    suggestion_width: usize,
}

impl Default for AutocompleteConfig {
    fn default() -> Self {
        let modifier = if cfg!(target_vendor = "apple") {
            KeyMods::SUPER
        } else {
            KeyMods::CTRL
        };
        Self {
            trigger_on_typing: true,
            trigger_on_shortcut: true,
            trigger_in_comments: false,
            trigger_in_strings: false,
            shortcut: KeyChord::new(Key::Space).with_mods(modifier),
            auto_insert_single_suggestion: false,
            trigger_delay: Duration::from_millis(200),
            no_suggestions_label: "No suggestions".to_owned(),
            suggestion_width: 30,
        }
    }
}

impl AutocompleteConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trigger_on_typing(mut self, enabled: bool) -> Self {
        self.trigger_on_typing = enabled;
        self
    }

    pub fn trigger_on_shortcut(mut self, enabled: bool) -> Self {
        self.trigger_on_shortcut = enabled;
        self
    }

    pub fn trigger_in_comments(mut self, enabled: bool) -> Self {
        self.trigger_in_comments = enabled;
        self
    }

    pub fn trigger_in_strings(mut self, enabled: bool) -> Self {
        self.trigger_in_strings = enabled;
        self
    }

    pub fn shortcut(mut self, shortcut: KeyChord) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub fn auto_insert_single_suggestion(mut self, enabled: bool) -> Self {
        self.auto_insert_single_suggestion = enabled;
        self
    }

    /// Sets the activation delay.
    ///
    /// Delays longer than `i64::MAX` milliseconds are rejected when the configuration is installed.
    pub fn trigger_delay(mut self, delay: Duration) -> Self {
        self.trigger_delay = delay;
        self
    }

    pub fn no_suggestions_label(mut self, label: impl Into<String>) -> Self {
        self.no_suggestions_label = label.into();
        self
    }

    /// Sets the popup width in glyph columns, or `0` to let Dear ImGui auto-fit it.
    pub fn suggestion_width(mut self, width: usize) -> Self {
        self.suggestion_width = width;
        self
    }
}

/// Copied syntax context for one autocomplete request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutocompleteContext {
    pub in_identifier: bool,
    pub in_number: bool,
    pub in_comment: bool,
    pub in_string: bool,
    pub language: Option<Language>,
}

/// Callback-scoped access to the current autocomplete request.
///
/// The native state cannot escape the callback because the lifetime is tied to the
/// invocation and the raw pointer is private.
///
/// ```compile_fail
/// use dear_imgui_cte::AutocompleteRequest;
///
/// fn extend_lifetime<'a>(
///     request: &'a AutocompleteRequest<'a>,
/// ) -> &'static AutocompleteRequest<'static> {
///     request
/// }
/// ```
pub struct AutocompleteRequest<'callback> {
    raw: NonNull<sys::DearImGuiCteAutocompleteState>,
    _callback: PhantomData<&'callback mut sys::DearImGuiCteAutocompleteState>,
}

impl AutocompleteRequest<'_> {
    /// Returns the current search term without copying it beyond this callback.
    pub fn search_term(&self) -> CteResult<&str> {
        const OPERATION: &str = "AutocompleteRequest::search_term";
        let mut text = ptr::null();
        let mut len = 0;
        let status = unsafe {
            sys::dear_imgui_cte_autocomplete_state_get_search_term(
                self.raw.as_ptr(),
                &mut text,
                &mut len,
            )
        };
        check_status(OPERATION, status)?;
        let bytes = unsafe { bytes_from_raw(text, len, OPERATION)? };
        str::from_utf8(bytes).map_err(|source| CteError::InvalidUtf8 {
            operation: OPERATION,
            source,
        })
    }

    pub fn range(&self) -> CteResult<Selection> {
        const OPERATION: &str = "AutocompleteRequest::range";
        let mut start = sys::DocPos_c::default();
        let mut end = sys::DocPos_c::default();
        let status = unsafe {
            sys::dear_imgui_cte_autocomplete_state_get_range(
                self.raw.as_ptr(),
                &mut start,
                &mut end,
            )
        };
        check_status(OPERATION, status)?;
        Ok(Selection::new(
            Position::from_raw(start),
            Position::from_raw(end),
        ))
    }

    pub fn context(&self) -> CteResult<AutocompleteContext> {
        const OPERATION: &str = "AutocompleteRequest::context";
        let mut context = sys::DearImGuiCteAutocompleteContextView::default();
        let status = unsafe {
            sys::dear_imgui_cte_autocomplete_state_get_context(self.raw.as_ptr(), &mut context)
        };
        check_status(OPERATION, status)?;
        Ok(AutocompleteContext {
            in_identifier: context.in_identifier,
            in_number: context.in_number,
            in_comment: context.in_comment,
            in_string: context.in_string,
            language: Language::from_raw(context.language),
        })
    }

    pub fn clear_suggestions(&mut self) -> CteResult<()> {
        const OPERATION: &str = "AutocompleteRequest::clear_suggestions";
        let status =
            unsafe { sys::dear_imgui_cte_autocomplete_state_clear_suggestions(self.raw.as_ptr()) };
        check_status(OPERATION, status)
    }

    pub fn add_suggestion(&mut self, suggestion: &str) -> CteResult<()> {
        const OPERATION: &str = "AutocompleteRequest::add_suggestion";
        let suggestion = c_string(OPERATION, suggestion)?;
        let status = unsafe {
            sys::dear_imgui_cte_autocomplete_state_add_suggestion(
                self.raw.as_ptr(),
                suggestion.as_ptr(),
                suggestion.as_bytes().len(),
            )
        };
        check_status(OPERATION, status)
    }

    pub fn set_suggestions<I, S>(&mut self, suggestions: I) -> CteResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        const OPERATION: &str = "AutocompleteRequest::set_suggestions";
        let suggestions = suggestions
            .into_iter()
            .map(|value| c_string(OPERATION, value.as_ref()))
            .collect::<CteResult<Vec<_>>>()?;
        self.clear_suggestions()?;
        for suggestion in suggestions {
            let status = unsafe {
                sys::dear_imgui_cte_autocomplete_state_add_suggestion(
                    self.raw.as_ptr(),
                    suggestion.as_ptr(),
                    suggestion.as_bytes().len(),
                )
            };
            check_status(OPERATION, status)?;
        }
        Ok(())
    }

    /// Marks whether suggestions will arrive asynchronously on the render thread.
    pub fn set_promised(&mut self, promised: bool) -> CteResult<()> {
        const OPERATION: &str = "AutocompleteRequest::set_promised";
        let status = unsafe {
            sys::dear_imgui_cte_autocomplete_state_set_promise(self.raw.as_ptr(), promised)
        };
        check_status(OPERATION, status)
    }
}

/// A transient view proving that Trie autocomplete is owned by its editor.
///
/// ```compile_fail
/// use dear_imgui_cte::{TextEditor, TrieAutocomplete};
///
/// fn extend_lifetime(editor: &TextEditor) -> TrieAutocomplete<'static> {
///     editor.trie_autocomplete().unwrap()
/// }
/// ```
#[derive(Clone, Copy)]
pub struct TrieAutocomplete<'editor> {
    editor: &'editor TextEditor,
}

impl TrieAutocomplete<'_> {
    /// Queries the native attachment while the editor's owning context is current.
    pub fn is_connected(self) -> CteResult<bool> {
        let trie = self
            .editor
            .trie
            .expect("TrieAutocomplete cannot exist without a native attachment");
        self.editor
            .try_with_context("TrieAutocomplete::is_connected", |_editor| unsafe {
                sys::TrieAutoComplete_IsConnected(trie.as_ptr())
            })
    }
}

impl TextEditor {
    /// Installs custom autocomplete and keeps its callback alive with this editor.
    pub fn set_autocomplete<F>(&mut self, config: &AutocompleteConfig, callback: F) -> CteResult<()>
    where
        F: for<'a> FnMut(&mut AutocompleteRequest<'a>) + 'static,
    {
        const OPERATION: &str = "TextEditor::set_autocomplete";
        self.reject_trie_conflict(OPERATION)?;
        let delay_ms = config.trigger_delay.as_millis();
        if delay_ms > MAX_AUTOCOMPLETE_DELAY_MS {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "trigger_delay",
                requirement: "at most i64::MAX milliseconds",
            });
        }
        let delay_ms = delay_ms as u64;
        let label = c_string(OPERATION, &config.no_suggestions_label)?;

        let slot = CallbackSlot::new(Box::new(callback) as Box<AutocompleteCallback>);
        let native = NativeAutocompleteConfig::create()?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_triggers(
                native.raw(),
                config.trigger_on_typing,
                config.trigger_on_shortcut,
                config.trigger_in_comments,
                config.trigger_in_strings,
            )
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_shortcut(
                native.raw(),
                config.shortcut.raw(),
            )
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_auto_insert_single(
                native.raw(),
                config.auto_insert_single_suggestion,
            )
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_trigger_delay(native.raw(), delay_ms)
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_no_suggestions_label(
                native.raw(),
                label.as_ptr(),
                label.as_bytes().len(),
            )
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_suggestion_width(
                native.raw(),
                config.suggestion_width,
            )
        })?;
        check_status(OPERATION, unsafe {
            sys::dear_imgui_cte_autocomplete_config_set_callback(
                native.raw(),
                Some(autocomplete_trampoline),
                slot.userdata(),
            )
        })?;
        self.clear_autocomplete()?;
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_set_autocomplete_config(raw, native.raw())
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.autocomplete = Some(slot);
        Ok(())
    }

    pub fn clear_autocomplete(&mut self) -> CteResult<()> {
        if self.trie.is_some() {
            return Ok(());
        }
        const OPERATION: &str = "TextEditor::clear_autocomplete";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_reset_autocomplete(raw)
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.autocomplete = None;
        Ok(())
    }

    /// Copies asynchronous suggestions into the editor on its render thread.
    pub fn set_autocomplete_suggestions<I, S>(&mut self, suggestions: I) -> CteResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        const OPERATION: &str = "TextEditor::set_autocomplete_suggestions";
        self.reject_trie_conflict(OPERATION)?;
        let suggestions = suggestions
            .into_iter()
            .map(|value| c_string(OPERATION, value.as_ref()))
            .collect::<CteResult<Vec<_>>>()?;
        let pointers = suggestions
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>();
        let lengths = suggestions
            .iter()
            .map(|value| value.as_bytes().len())
            .collect::<Vec<_>>();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_set_autocomplete_suggestions(
                raw,
                pointers.as_ptr(),
                lengths.as_ptr(),
                pointers.len(),
            )
        })?;
        check_status(OPERATION, status)
    }

    /// Connects the built-in identifier Trie, replacing an existing Trie attachment.
    pub fn enable_trie_autocomplete(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::enable_trie_autocomplete";
        if let Some(active) = self.callbacks.trie_conflict() {
            return Err(CteError::CallbackConflict {
                operation: OPERATION,
                active,
            });
        }

        enum EnableTrieResult {
            Connected(NonNull<sys::TrieAutoComplete>),
            CreationFailed,
            ResetFailed(sys::DearImGuiCteStatus),
            NotConnected,
        }

        let old = self.trie.take();
        let result = match self.try_with_context(OPERATION, |raw| unsafe {
            let Some(new) = NonNull::new(sys::TrieAutoComplete_TrieAutoComplete()) else {
                return EnableTrieResult::CreationFailed;
            };
            if let Some(old) = old {
                let status = sys::dear_imgui_cte_text_editor_reset_autocomplete(raw);
                if status != sys::DearImGuiCteStatus_Ok {
                    sys::TrieAutoComplete_destroy(new.as_ptr());
                    return EnableTrieResult::ResetFailed(status);
                }
                sys::TrieAutoComplete_Disconnect(old.as_ptr());
                sys::TrieAutoComplete_destroy(old.as_ptr());
            }
            sys::TrieAutoComplete_Connect(new.as_ptr(), raw);
            if sys::TrieAutoComplete_IsConnected(new.as_ptr()) {
                EnableTrieResult::Connected(new)
            } else {
                sys::TrieAutoComplete_destroy(new.as_ptr());
                EnableTrieResult::NotConnected
            }
        }) {
            Ok(result) => result,
            Err(error) => {
                self.trie = old;
                return Err(error);
            }
        };
        match result {
            EnableTrieResult::Connected(new) => {
                self.trie = Some(new);
                Ok(())
            }
            EnableTrieResult::CreationFailed => {
                self.trie = old;
                Err(CteError::CreationFailed {
                    object: "TrieAutoComplete",
                })
            }
            EnableTrieResult::ResetFailed(status) => {
                self.trie = old;
                Err(CteError::NativeStatus {
                    operation: OPERATION,
                    status: status as u32,
                })
            }
            EnableTrieResult::NotConnected => Err(CteError::NativeStatus {
                operation: OPERATION,
                status: sys::DearImGuiCteStatus_CallbackFailed as u32,
            }),
        }
    }

    pub fn disable_trie_autocomplete(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::disable_trie_autocomplete";
        let Some(trie) = self.trie.take() else {
            return Ok(());
        };
        match self.try_with_context(OPERATION, |raw| unsafe {
            check_status(
                OPERATION,
                sys::dear_imgui_cte_text_editor_reset_autocomplete(raw),
            )?;
            sys::TrieAutoComplete_Disconnect(trie.as_ptr());
            sys::TrieAutoComplete_destroy(trie.as_ptr());
            Ok(())
        }) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) | Err(error) => {
                self.trie = Some(trie);
                Err(error)
            }
        }
    }

    pub fn trie_autocomplete(&self) -> Option<TrieAutocomplete<'_>> {
        self.trie
            .is_some()
            .then_some(TrieAutocomplete { editor: self })
    }
}

struct NativeAutocompleteConfig(NonNull<sys::DearImGuiCteAutocompleteConfig>);

impl NativeAutocompleteConfig {
    fn create() -> CteResult<Self> {
        NonNull::new(unsafe { sys::dear_imgui_cte_autocomplete_config_create() })
            .map(Self)
            .ok_or(CteError::CreationFailed {
                object: "autocomplete configuration",
            })
    }

    fn raw(&self) -> *mut sys::DearImGuiCteAutocompleteConfig {
        self.0.as_ptr()
    }
}

impl Drop for NativeAutocompleteConfig {
    fn drop(&mut self) {
        unsafe { sys::dear_imgui_cte_autocomplete_config_destroy(self.0.as_ptr()) };
    }
}

unsafe extern "C" fn autocomplete_trampoline(
    userdata: *mut c_void,
    state: *mut sys::DearImGuiCteAutocompleteState,
) {
    let Some(slot) = (unsafe {
        userdata
            .cast::<CallbackSlot<AutocompleteCallback>>()
            .as_ref()
    }) else {
        return;
    };
    let Some(raw) = NonNull::new(state) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in autocomplete callback", || {
        let mut request = AutocompleteRequest {
            raw,
            _callback: PhantomData,
        };
        slot.invoke((), |callback| callback(&mut request));
    });
}

unsafe fn bytes_from_raw<'a>(
    raw: *const c_char,
    len: usize,
    operation: &'static str,
) -> CteResult<&'a [u8]> {
    if len == 0 {
        Ok(&[])
    } else if raw.is_null() {
        Err(CteError::NullResult { operation })
    } else {
        Ok(unsafe { slice::from_raw_parts(raw.cast(), len) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_matches_the_upstream_apple_contract() {
        let expected_modifier = if cfg!(target_vendor = "apple") {
            KeyMods::SUPER
        } else {
            KeyMods::CTRL
        };

        assert_eq!(
            AutocompleteConfig::default().shortcut,
            KeyChord::new(Key::Space).with_mods(expected_modifier)
        );
    }
}
