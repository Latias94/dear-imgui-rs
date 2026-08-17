mod config;
mod editing;
mod render;

pub use render::TextEditorRenderer;

use crate::{
    CteError, CteResult, Language, Palette, Position, Selection, callbacks::CallbackRegistry,
    context::CteContextBinding, error::c_string, sys,
};
use dear_imgui_rs::{Context, ContextId};
use std::{
    ffi::{CStr, c_char},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    ptr::NonNull,
    rc::Rc,
};

/// An owned, context-bound ImGuiColorTextEdit editor.
///
/// The editor is intentionally neither [`Send`] nor [`Sync`]. Every native call is
/// made while its originating Dear ImGui context is current.
pub struct TextEditor {
    raw: NonNull<sys::TextEditor>,
    binding: CteContextBinding,
    pub(crate) callbacks: CallbackRegistry,
    pub(crate) trie: Option<NonNull<sys::TrieAutoComplete>>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl TextEditor {
    /// Creates an editor bound to `context`.
    pub fn try_create(context: &Context) -> CteResult<Self> {
        let binding = CteContextBinding::new(context);
        let raw = binding.try_with_bound_context("TextEditor::try_create", || unsafe {
            sys::TextEditor_TextEditor()
        })?;
        let raw = NonNull::new(raw).ok_or(CteError::CreationFailed {
            object: "TextEditor",
        })?;
        Ok(Self {
            raw,
            binding,
            callbacks: CallbackRegistry::new(),
            trie: None,
            _not_send_sync: PhantomData,
        })
    }

    /// Creates an editor and panics if native allocation fails.
    pub fn create(context: &Context) -> Self {
        Self::try_create(context).expect("failed to create cimCTE TextEditor")
    }

    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn context_id(&self) -> ContextId {
        self.binding.id()
    }

    /// Returns the raw editor pointer.
    ///
    /// # Safety
    ///
    /// The pointer may only be used while the owning Dear ImGui context is current. The
    /// caller must preserve every cimCTE precondition, ownership and pointer-lifetime rule,
    /// and invariant relied on by this safe wrapper. In particular, the caller must not
    /// destroy the editor, retain borrowed native pointers, install callbacks, set invalid
    /// configuration values, or call `TextEditor_SetImGuiContext` through this pointer.
    pub unsafe fn as_raw(&self) -> *mut sys::TextEditor {
        self.raw_ptr()
    }

    /// Replaces the entire document.
    pub fn set_text(&mut self, text: &str) -> CteResult<()> {
        let text = c_string("TextEditor::set_text", text)?;
        self.with_context("TextEditor::set_text", |raw| unsafe {
            sys::TextEditor_SetText(raw, text.as_ptr())
        });
        Ok(())
    }

    /// Returns an owned copy of the complete document.
    pub fn text(&self) -> CteResult<String> {
        self.with_context("TextEditor::text", |raw| unsafe {
            let raw_text = sys::TextEditor_GetText_alloc(raw);
            let allocation = AllocatedText::new(raw_text, "TextEditor::text")?;
            copy_c_string("TextEditor::text", allocation.as_ptr())
        })
    }

    /// Returns an owned copy of the selected text for one cursor.
    pub fn cursor_text(&self, cursor: usize) -> CteResult<String> {
        self.with_context("TextEditor::cursor_text", |raw| unsafe {
            validate_cursor(raw, "TextEditor::cursor_text", cursor)?;
            copy_c_string(
                "TextEditor::cursor_text",
                sys::TextEditor_GetCursorText(raw, cursor),
            )
        })
    }

    /// Returns an owned copy of one document line.
    pub fn line_text(&self, line: usize) -> CteResult<String> {
        self.with_context("TextEditor::line_text", |raw| unsafe {
            validate_line(raw, "TextEditor::line_text", line)?;
            copy_c_string(
                "TextEditor::line_text",
                sys::TextEditor_GetLineText(raw, line),
            )
        })
    }

    /// Returns an owned copy of a document selection.
    pub fn section_text(&self, selection: Selection) -> CteResult<String> {
        self.with_context("TextEditor::section_text", |raw| unsafe {
            validate_selection(raw, "TextEditor::section_text", selection)?;
            copy_c_string(
                "TextEditor::section_text",
                sys::TextEditor_GetSectionText_DocSelection(raw, selection.into_raw()),
            )
        })
    }

    /// Replaces a document selection.
    pub fn replace_section(&mut self, selection: Selection, text: &str) -> CteResult<()> {
        let text = c_string("TextEditor::replace_section", text)?;
        self.with_context("TextEditor::replace_section", |raw| unsafe {
            validate_selection(raw, "TextEditor::replace_section", selection)?;
            sys::TextEditor_ReplaceSectionText_DocSelection(
                raw,
                selection.into_raw(),
                text.as_ptr(),
            );
            Ok(())
        })
    }

    /// Removes all document text.
    pub fn clear_text(&mut self) {
        self.with_context("TextEditor::clear_text", |raw| unsafe {
            sys::TextEditor_ClearText(raw)
        });
    }

    pub fn is_empty(&self) -> bool {
        self.with_context("TextEditor::is_empty", |raw| unsafe {
            sys::TextEditor_IsEmpty(raw)
        })
    }

    pub fn line_count(&self) -> usize {
        self.with_context("TextEditor::line_count", |raw| unsafe {
            sys::TextEditor_GetLineCount(raw)
        })
    }

    /// Sets a built-in language, or disables language-aware behavior with `None`.
    pub fn set_language(&mut self, language: Option<Language>) {
        let language = language.map_or(std::ptr::null(), Language::as_raw);
        self.with_context("TextEditor::set_language", |raw| unsafe {
            sys::TextEditor_SetLanguage(raw, language)
        });
    }

    /// Returns the selected built-in language.
    pub fn language(&self) -> Option<Language> {
        self.with_context("TextEditor::language", |raw| unsafe {
            Language::from_raw(sys::TextEditor_GetLanguage(raw))
        })
    }

    pub fn has_language(&self) -> bool {
        self.with_context("TextEditor::has_language", |raw| unsafe {
            sys::TextEditor_HasLanguage(raw)
        })
    }

    /// Returns an owned copy of the current language name.
    pub fn language_name(&self) -> CteResult<String> {
        self.with_context("TextEditor::language_name", |raw| unsafe {
            copy_c_string(
                "TextEditor::language_name",
                sys::TextEditor_GetLanguageName(raw),
            )
        })
    }

    /// Copies a Rust palette into the editor.
    pub fn set_palette(&mut self, palette: &Palette) -> CteResult<()> {
        self.with_context("TextEditor::set_palette", |raw| {
            palette.with_native(|native| unsafe { sys::TextEditor_SetPalette(raw, native) })
        })
    }

    /// Returns an owned copy of the editor palette.
    pub fn palette(&self) -> Palette {
        self.with_context("TextEditor::palette", |raw| unsafe {
            Palette::copy_from_raw(sys::TextEditor_GetPalette(raw))
        })
    }

    pub(crate) fn with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::TextEditor) -> R,
    ) -> R {
        self.try_with_context(operation, f)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    pub(crate) fn try_with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::TextEditor) -> R,
    ) -> CteResult<R> {
        let raw = self.raw_ptr();
        self.binding.try_with_bound_context(operation, || f(raw))
    }

    fn raw_ptr(&self) -> *mut sys::TextEditor {
        self.raw.as_ptr()
    }
}

impl Drop for TextEditor {
    fn drop(&mut self) {
        let raw = self.raw;
        let trie = self.trie.take();
        let callbacks = &mut self.callbacks;
        let mut callback_panic = None;
        let native_result = self
            .binding
            .try_with_bound_context("TextEditor::drop", || unsafe {
                if let Some(trie) = trie {
                    sys::TrieAutoComplete_Disconnect(trie.as_ptr());
                    sys::TrieAutoComplete_destroy(trie.as_ptr());
                }
                let _ = sys::dear_imgui_cte_clear_callbacks(raw.as_ptr());
                let owned = callbacks.take_owned();
                callback_panic = catch_unwind(AssertUnwindSafe(|| drop(owned))).err();
                sys::TextEditor_destroy(raw.as_ptr());
            });
        if native_result.is_err() {
            callbacks.clear_owned();
        } else if let Some(payload) = callback_panic {
            resume_unwind(payload);
        }
        // If context teardown already started, touching CTE state is no longer proven safe.
        // Native handles are intentionally leaked rather than calling into a dead context.
    }
}

struct AllocatedText(NonNull<c_char>);

impl AllocatedText {
    fn new(raw: *mut c_char, operation: &'static str) -> CteResult<Self> {
        NonNull::new(raw)
            .map(Self)
            .ok_or(CteError::NullResult { operation })
    }

    fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }
}

impl Drop for AllocatedText {
    fn drop(&mut self) {
        unsafe { sys::TextEditor_GetText_free(self.0.as_ptr()) };
    }
}

unsafe fn copy_c_string(operation: &'static str, raw: *const c_char) -> CteResult<String> {
    if raw.is_null() {
        return Err(CteError::NullResult { operation });
    }
    let value = unsafe { CStr::from_ptr(raw) };
    value
        .to_str()
        .map(str::to_owned)
        .map_err(|source| CteError::InvalidUtf8 { operation, source })
}

unsafe fn validate_line(
    raw: *mut sys::TextEditor,
    operation: &'static str,
    line: usize,
) -> CteResult<()> {
    let line_count = unsafe { sys::TextEditor_GetLineCount(raw) };
    if line >= line_count {
        return Err(CteError::LineOutOfBounds {
            operation,
            line,
            line_count,
        });
    }
    Ok(())
}

unsafe fn validate_cursor(
    raw: *mut sys::TextEditor,
    operation: &'static str,
    cursor: usize,
) -> CteResult<()> {
    let cursor_count = unsafe { sys::TextEditor_GetNumberOfCursors(raw) };
    if cursor >= cursor_count {
        return Err(CteError::CursorOutOfBounds {
            operation,
            cursor,
            cursor_count,
        });
    }
    Ok(())
}

unsafe fn validate_position(
    raw: *mut sys::TextEditor,
    operation: &'static str,
    position: Position,
) -> CteResult<()> {
    unsafe { validate_line(raw, operation, position.line)? };
    let line =
        unsafe { copy_c_string(operation, sys::TextEditor_GetLineText(raw, position.line))? };
    let column_count = line.chars().count();
    if position.column > column_count {
        return Err(CteError::ColumnOutOfBounds {
            operation,
            line: position.line,
            column: position.column,
            column_count,
        });
    }
    Ok(())
}

unsafe fn validate_selection(
    raw: *mut sys::TextEditor,
    operation: &'static str,
    selection: Selection,
) -> CteResult<()> {
    if !selection.is_ordered() {
        return Err(CteError::ReversedSelection { operation });
    }
    unsafe {
        validate_position(raw, operation, selection.start)?;
        validate_position(raw, operation, selection.end)
    }
}
