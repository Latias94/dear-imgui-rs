use crate::{
    CteError, CteResult, Position, Selection, error::c_string, sys, validation::duration_millis_i32,
};
use dear_imgui_rs::Ui;
use std::{
    cell::{Cell, UnsafeCell},
    ffi::{CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    rc::Rc,
    slice, str,
    time::Duration,
};

use crate::text_editor::TextEditor;

/// Whether a transaction inserted or deleted text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextChangeKind {
    Insert,
    Delete,
}

/// A text transaction view valid only for one callback invocation.
///
/// ```compile_fail
/// use dear_imgui_cte::TextChange;
///
/// fn extend_lifetime<'a>(change: TextChange<'a>) -> TextChange<'static> {
///     change
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextChange<'a> {
    pub kind: TextChangeKind,
    pub range: Selection,
    pub text: &'a str,
}

/// Copied line-decorator geometry for one render callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecoratorEvent {
    pub line: usize,
    pub width: f32,
    pub height: f32,
    pub glyph_size: [f32; 2],
}

/// Copied caret geometry and style for one render callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretEvent {
    pub glyph_position: [f32; 2],
    pub glyph_size: [f32; 2],
    pub visible: bool,
    pub color: u32,
    pub cursor: usize,
}

/// Copied document position for a context-menu or hover callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PopupEvent {
    pub position: Position,
}

type EmptyCallback = dyn FnMut();
type TransactionCallback = dyn for<'a> FnMut(TextChange<'a>);
type DecoratorCallback = dyn FnMut(&Ui, DecoratorEvent);
type CaretCallback = dyn FnMut(&Ui, CaretEvent);
type PopupCallback = dyn FnMut(&Ui, PopupEvent);

pub(crate) type AutocompleteCallback = dyn for<'a> FnMut(&mut crate::AutocompleteRequest<'a>);

pub(crate) struct CallbackSlot<T: ?Sized> {
    invoking: Cell<bool>,
    callback: UnsafeCell<Box<T>>,
}

impl<T: ?Sized> CallbackSlot<T> {
    pub(crate) fn new(callback: Box<T>) -> Box<Self> {
        Box::new(Self {
            invoking: Cell::new(false),
            callback: UnsafeCell::new(callback),
        })
    }

    pub(crate) fn userdata(&self) -> *mut c_void {
        (self as *const Self).cast_mut().cast()
    }

    pub(crate) fn invoke<R>(&self, reentrant: R, invoke: impl FnOnce(&mut T) -> R) -> R {
        if self.invoking.replace(true) {
            return reentrant;
        }
        let _guard = InvocationGuard(&self.invoking);
        let callback = unsafe { &mut **self.callback.get() };
        invoke(callback)
    }
}

struct InvocationGuard<'a>(&'a Cell<bool>);

impl Drop for InvocationGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

struct ActiveUi {
    current: Cell<*const Ui>,
}

pub(crate) struct ActiveUiGuard {
    active: Rc<ActiveUi>,
    previous: *const Ui,
}

impl Drop for ActiveUiGuard {
    fn drop(&mut self) {
        self.active.current.set(self.previous);
    }
}

struct UiCallbackSlot<T: ?Sized> {
    active_ui: Rc<ActiveUi>,
    callback: CallbackSlot<T>,
}

impl<T: ?Sized> UiCallbackSlot<T> {
    fn new(active_ui: Rc<ActiveUi>, callback: Box<T>) -> Box<Self> {
        Box::new(Self {
            active_ui,
            callback: CallbackSlot {
                invoking: Cell::new(false),
                callback: UnsafeCell::new(callback),
            },
        })
    }

    fn userdata(&self) -> *mut c_void {
        (self as *const Self).cast_mut().cast()
    }

    fn invoke(&self, invoke: impl FnOnce(&mut T, &Ui)) {
        let ui = self.active_ui.current.get();
        let Some(ui) = (unsafe { ui.as_ref() }) else {
            return;
        };
        self.callback.invoke((), |callback| invoke(callback, ui));
    }
}

pub(crate) struct CallbackRegistry {
    active_ui: Rc<ActiveUi>,
    change: Option<Box<CallbackSlot<EmptyCallback>>>,
    transaction: Option<Box<CallbackSlot<TransactionCallback>>>,
    decorator: Option<Box<UiCallbackSlot<DecoratorCallback>>>,
    caret: Option<Box<UiCallbackSlot<CaretCallback>>>,
    line_number_context: Option<Box<UiCallbackSlot<PopupCallback>>>,
    text_context: Option<Box<UiCallbackSlot<PopupCallback>>>,
    text_hover: Option<Box<UiCallbackSlot<PopupCallback>>>,
    language_change: Option<Box<CallbackSlot<EmptyCallback>>>,
    pub(crate) autocomplete: Option<Box<CallbackSlot<AutocompleteCallback>>>,
}

impl CallbackRegistry {
    pub(crate) fn new() -> Self {
        Self {
            active_ui: Rc::new(ActiveUi {
                current: Cell::new(ptr::null()),
            }),
            change: None,
            transaction: None,
            decorator: None,
            caret: None,
            line_number_context: None,
            text_context: None,
            text_hover: None,
            language_change: None,
            autocomplete: None,
        }
    }

    pub(crate) fn enter_ui(&self, ui: &Ui) -> ActiveUiGuard {
        let previous = self.active_ui.current.replace(ui);
        ActiveUiGuard {
            active: Rc::clone(&self.active_ui),
            previous,
        }
    }

    pub(crate) fn clear_owned(&mut self) {
        drop(self.take_owned());
    }

    pub(crate) fn take_owned(&mut self) -> Self {
        std::mem::replace(self, Self::new())
    }

    pub(crate) fn trie_conflict(&self) -> Option<&'static str> {
        if self.change.is_some() {
            Some("change callback")
        } else if self.language_change.is_some() {
            Some("language-change callback")
        } else if self.autocomplete.is_some() {
            Some("custom autocomplete")
        } else {
            None
        }
    }
}

impl TextEditor {
    /// Installs a delayed document-change callback.
    pub fn set_change_callback<F>(&mut self, delay: Duration, callback: F) -> CteResult<()>
    where
        F: FnMut() + 'static,
    {
        const OPERATION: &str = "TextEditor::set_change_callback";
        self.reject_trie_conflict(OPERATION)?;
        let delay_ms = duration_millis_i32(OPERATION, "delay", delay)?;
        self.clear_change_callback()?;
        let slot = CallbackSlot::new(Box::new(callback) as Box<EmptyCallback>);
        let userdata = slot.userdata();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_set_change_callback(
                raw,
                Some(change_trampoline),
                userdata,
                delay_ms,
            )
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.change = Some(slot);
        Ok(())
    }

    pub fn clear_change_callback(&mut self) -> CteResult<()> {
        if self.trie.is_some() {
            return Ok(());
        }
        const OPERATION: &str = "TextEditor::clear_change_callback";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_set_change_callback(raw, None, ptr::null_mut(), 0)
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.change = None;
        Ok(())
    }

    /// Installs a callback for each inserted or deleted transaction segment.
    pub fn set_transaction_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: for<'a> FnMut(TextChange<'a>) + 'static,
    {
        const OPERATION: &str = "TextEditor::set_transaction_callback";
        self.clear_transaction_callback()?;
        let slot = CallbackSlot::new(Box::new(callback) as Box<TransactionCallback>);
        let userdata = slot.userdata();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_transaction_callback(
                raw,
                Some(transaction_trampoline),
                userdata,
            )
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.transaction = Some(slot);
        Ok(())
    }

    pub fn clear_transaction_callback(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::clear_transaction_callback";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_transaction_callback(raw, None, ptr::null_mut())
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.transaction = None;
        Ok(())
    }

    /// Installs a line decorator called while the editor is rendering.
    pub fn set_line_decorator<F>(&mut self, width: usize, callback: F) -> CteResult<()>
    where
        F: FnMut(&Ui, DecoratorEvent) + 'static,
    {
        const OPERATION: &str = "TextEditor::set_line_decorator";
        if width == 0 {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "width",
                requirement: "greater than zero",
            });
        }
        self.clear_line_decorator()?;
        let slot = UiCallbackSlot::new(
            Rc::clone(&self.callbacks.active_ui),
            Box::new(callback) as Box<DecoratorCallback>,
        );
        let userdata = slot.userdata();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_line_decorator(raw, width, Some(decorator_trampoline), userdata)
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.decorator = Some(slot);
        Ok(())
    }

    pub fn clear_line_decorator(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::clear_line_decorator";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_line_decorator(raw, 0, None, ptr::null_mut())
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.decorator = None;
        Ok(())
    }

    /// Installs a custom caret renderer called while the editor is rendering.
    pub fn set_custom_caret_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: FnMut(&Ui, CaretEvent) + 'static,
    {
        const OPERATION: &str = "TextEditor::set_custom_caret_callback";
        self.clear_custom_caret_callback()?;
        let slot = UiCallbackSlot::new(
            Rc::clone(&self.callbacks.active_ui),
            Box::new(callback) as Box<CaretCallback>,
        );
        let userdata = slot.userdata();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_custom_caret_callback(raw, Some(caret_trampoline), userdata)
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.caret = Some(slot);
        Ok(())
    }

    pub fn clear_custom_caret_callback(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::clear_custom_caret_callback";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_custom_caret_callback(raw, None, ptr::null_mut())
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.caret = None;
        Ok(())
    }

    /// Installs content for the popup opened by right-clicking a line number.
    ///
    /// The callback runs inside the open popup during editor rendering. Submit popup widgets
    /// through the provided [`Ui`]; [`PopupEvent::position`] identifies the clicked document line.
    pub fn set_line_number_context_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: FnMut(&Ui, PopupEvent) + 'static,
    {
        self.install_popup_callback(
            PopupKind::LineNumberContext,
            Box::new(callback) as Box<PopupCallback>,
        )
    }

    pub fn clear_line_number_context_callback(&mut self) -> CteResult<()> {
        self.clear_popup_callback(PopupKind::LineNumberContext)
    }

    /// Installs content for the popup opened by right-clicking the text area.
    ///
    /// The callback runs inside the open popup during editor rendering. Submit popup widgets
    /// through the provided [`Ui`]; [`PopupEvent::position`] identifies the clicked position.
    pub fn set_text_context_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: FnMut(&Ui, PopupEvent) + 'static,
    {
        self.install_popup_callback(
            PopupKind::TextContext,
            Box::new(callback) as Box<PopupCallback>,
        )
    }

    pub fn clear_text_context_callback(&mut self) -> CteResult<()> {
        self.clear_popup_callback(PopupKind::TextContext)
    }

    /// Installs content for the popup opened while a text glyph is hovered.
    ///
    /// The callback runs inside the open popup during editor rendering. Submit popup widgets
    /// through the provided [`Ui`]; [`PopupEvent::position`] identifies the hovered word start.
    pub fn set_text_hover_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: FnMut(&Ui, PopupEvent) + 'static,
    {
        self.install_popup_callback(
            PopupKind::TextHover,
            Box::new(callback) as Box<PopupCallback>,
        )
    }

    pub fn clear_text_hover_callback(&mut self) -> CteResult<()> {
        self.clear_popup_callback(PopupKind::TextHover)
    }

    pub fn set_language_change_callback<F>(&mut self, callback: F) -> CteResult<()>
    where
        F: FnMut() + 'static,
    {
        const OPERATION: &str = "TextEditor::set_language_change_callback";
        self.reject_trie_conflict(OPERATION)?;
        self.clear_language_change_callback()?;
        let slot = CallbackSlot::new(Box::new(callback) as Box<EmptyCallback>);
        let userdata = slot.userdata();
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_language_change_callback(
                raw,
                Some(language_change_trampoline),
                userdata,
            )
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.language_change = Some(slot);
        Ok(())
    }

    pub fn clear_language_change_callback(&mut self) -> CteResult<()> {
        if self.trie.is_some() {
            return Ok(());
        }
        const OPERATION: &str = "TextEditor::clear_language_change_callback";
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_set_language_change_callback(raw, None, ptr::null_mut())
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.language_change = None;
        Ok(())
    }

    /// Calls `callback` synchronously for every known identifier.
    pub fn for_each_identifier<F>(&self, mut callback: F) -> CteResult<()>
    where
        F: FnMut(&str),
    {
        const OPERATION: &str = "TextEditor::for_each_identifier";
        let mut state = IdentifierState {
            invoking: Cell::new(false),
            callback: UnsafeCell::new(&mut callback),
        };
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_iterate_identifiers(
                raw,
                Some(identifier_trampoline::<F>),
                (&mut state as *mut IdentifierState<'_, F>).cast(),
            )
        })?;
        check_status(OPERATION, status)
    }

    /// Replaces every selected range with the callback result.
    /// Replaces each single-line selection with the callback result.
    ///
    /// The selected upstream snapshot applies a multi-line selection filter more than
    /// once, so this safe wrapper rejects multi-line selections. All callback outputs
    /// are validated before the native transaction starts.
    pub fn filter_selections<F>(&mut self, mut callback: F) -> CteResult<()>
    where
        F: FnMut(&str) -> String,
    {
        const OPERATION: &str = "TextEditor::filter_selections";
        if self.is_read_only() {
            return Ok(());
        }

        let mut outputs = Vec::new();
        for cursor in 0..self.cursor_count() {
            let selection = self.cursor_selection(cursor)?;
            if selection.start.line != selection.end.line {
                return Err(CteError::InvalidValue {
                    operation: OPERATION,
                    parameter: "selection",
                    requirement: "every selection must be contained within one line",
                });
            }

            let line = self.line_text(selection.end.line)?;
            if selection.end.column != 0 && !line.is_empty() {
                let input = self.cursor_text(cursor)?;
                let output = callback(&input);
                outputs.push(c_string(OPERATION, &output)?);
            }
        }

        self.apply_filter_outputs(OPERATION, false, outputs)
    }

    /// Replaces every document line with the callback result.
    ///
    /// Line filters cannot add or remove line breaks. All callback outputs are
    /// validated before the native transaction starts.
    pub fn filter_lines<F>(&mut self, mut callback: F) -> CteResult<()>
    where
        F: FnMut(&str) -> String,
    {
        const OPERATION: &str = "TextEditor::filter_lines";
        if self.is_read_only() {
            return Ok(());
        }

        let mut outputs = Vec::with_capacity(self.line_count());
        for line in 0..self.line_count() {
            let input = self.line_text(line)?;
            let output = callback(&input);
            if output.contains(['\r', '\n']) {
                return Err(CteError::InvalidValue {
                    operation: OPERATION,
                    parameter: "callback output",
                    requirement: "must not contain line breaks",
                });
            }
            outputs.push(c_string(OPERATION, &output)?);
        }

        self.apply_filter_outputs(OPERATION, true, outputs)
    }

    /// Clears all safe callbacks and disables Trie autocomplete.
    pub fn clear_callbacks(&mut self) -> CteResult<()> {
        const OPERATION: &str = "TextEditor::clear_callbacks";
        self.disable_trie_autocomplete()?;
        let status = self.try_with_context(OPERATION, |raw| unsafe {
            sys::dear_imgui_cte_text_editor_clear_callbacks(raw)
        })?;
        check_status(OPERATION, status)?;
        self.callbacks.clear_owned();
        Ok(())
    }

    fn install_popup_callback(
        &mut self,
        kind: PopupKind,
        callback: Box<PopupCallback>,
    ) -> CteResult<()> {
        self.clear_popup_callback(kind)?;
        let slot = UiCallbackSlot::new(Rc::clone(&self.callbacks.active_ui), callback);
        let userdata = slot.userdata();
        let status = self.try_with_context(kind.set_operation(), |raw| unsafe {
            match kind {
                PopupKind::LineNumberContext => {
                    sys::dear_imgui_cte_set_line_number_context_callback(
                        raw,
                        Some(popup_trampoline),
                        userdata,
                    )
                }
                PopupKind::TextContext => sys::dear_imgui_cte_set_text_context_callback(
                    raw,
                    Some(popup_trampoline),
                    userdata,
                ),
                PopupKind::TextHover => sys::dear_imgui_cte_set_text_hover_callback(
                    raw,
                    Some(popup_trampoline),
                    userdata,
                ),
            }
        })?;
        check_status(kind.set_operation(), status)?;
        *kind.slot(&mut self.callbacks) = Some(slot);
        Ok(())
    }

    fn clear_popup_callback(&mut self, kind: PopupKind) -> CteResult<()> {
        let status = self.try_with_context(kind.clear_operation(), |raw| unsafe {
            match kind {
                PopupKind::LineNumberContext => {
                    sys::dear_imgui_cte_set_line_number_context_callback(raw, None, ptr::null_mut())
                }
                PopupKind::TextContext => {
                    sys::dear_imgui_cte_set_text_context_callback(raw, None, ptr::null_mut())
                }
                PopupKind::TextHover => {
                    sys::dear_imgui_cte_set_text_hover_callback(raw, None, ptr::null_mut())
                }
            }
        })?;
        check_status(kind.clear_operation(), status)?;
        *kind.slot(&mut self.callbacks) = None;
        Ok(())
    }

    fn apply_filter_outputs(
        &mut self,
        operation: &'static str,
        lines: bool,
        outputs: Vec<CString>,
    ) -> CteResult<()> {
        let mut state = FilterState {
            invoking: Cell::new(false),
            outputs,
            next: 0,
        };
        let status = self.try_with_context(operation, |raw| unsafe {
            let callback: sys::DearImGuiCteFilterCallback = Some(filter_trampoline);
            let userdata = (&mut state as *mut FilterState).cast();
            if lines {
                sys::dear_imgui_cte_filter_lines(raw, callback, userdata)
            } else {
                sys::dear_imgui_cte_filter_selections(raw, callback, userdata)
            }
        })?;
        check_status(operation, status)?;
        if state.next != state.outputs.len() {
            return Err(CteError::NativeStatus {
                operation,
                status: sys::DearImGuiCteStatus_CallbackFailed as u32,
            });
        }
        self.invalidate_layout();
        Ok(())
    }

    pub(crate) fn reject_trie_conflict(&self, operation: &'static str) -> CteResult<()> {
        if self.trie.is_some() {
            return Err(CteError::CallbackConflict {
                operation,
                active: "Trie autocomplete",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum PopupKind {
    LineNumberContext,
    TextContext,
    TextHover,
}

impl PopupKind {
    fn set_operation(self) -> &'static str {
        match self {
            Self::LineNumberContext => "TextEditor::set_line_number_context_callback",
            Self::TextContext => "TextEditor::set_text_context_callback",
            Self::TextHover => "TextEditor::set_text_hover_callback",
        }
    }

    fn clear_operation(self) -> &'static str {
        match self {
            Self::LineNumberContext => "TextEditor::clear_line_number_context_callback",
            Self::TextContext => "TextEditor::clear_text_context_callback",
            Self::TextHover => "TextEditor::clear_text_hover_callback",
        }
    }

    fn slot(
        self,
        registry: &mut CallbackRegistry,
    ) -> &mut Option<Box<UiCallbackSlot<PopupCallback>>> {
        match self {
            Self::LineNumberContext => &mut registry.line_number_context,
            Self::TextContext => &mut registry.text_context,
            Self::TextHover => &mut registry.text_hover,
        }
    }
}

struct IdentifierState<'a, F> {
    invoking: Cell<bool>,
    callback: UnsafeCell<&'a mut F>,
}

struct FilterState {
    invoking: Cell<bool>,
    outputs: Vec<CString>,
    next: usize,
}

pub(crate) fn check_status(
    operation: &'static str,
    status: sys::DearImGuiCteStatus,
) -> CteResult<()> {
    if status == sys::DearImGuiCteStatus_Ok {
        Ok(())
    } else {
        Err(CteError::NativeStatus {
            operation,
            status: status as u32,
        })
    }
}

pub(crate) fn abort_on_panic<R>(message: &'static str, f: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("{message}");
            std::process::abort();
        }
    }
}

unsafe extern "C" fn change_trampoline(userdata: *mut c_void) {
    let Some(slot) = (unsafe { userdata.cast::<CallbackSlot<EmptyCallback>>().as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in change callback", || {
        slot.invoke((), |callback| callback())
    });
}

unsafe extern "C" fn language_change_trampoline(userdata: *mut c_void) {
    let Some(slot) = (unsafe { userdata.cast::<CallbackSlot<EmptyCallback>>().as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in language-change callback", || {
        slot.invoke((), |callback| callback())
    });
}

unsafe extern "C" fn transaction_trampoline(
    userdata: *mut c_void,
    change: *const sys::DearImGuiCteChangeView,
) {
    let Some(slot) = (unsafe {
        userdata
            .cast::<CallbackSlot<TransactionCallback>>()
            .as_ref()
    }) else {
        return;
    };
    let Some(change) = (unsafe { change.as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in transaction callback", || {
        let text = unsafe { callback_str(change.text, change.text_len, "transaction callback") };
        let event = TextChange {
            kind: if change.insert {
                TextChangeKind::Insert
            } else {
                TextChangeKind::Delete
            },
            range: Selection::new(
                Position::from_raw(change.start),
                Position::from_raw(change.end),
            ),
            text,
        };
        slot.invoke((), |callback| callback(event));
    });
}

unsafe extern "C" fn decorator_trampoline(userdata: *mut c_void, decorator: *mut sys::Decorator) {
    let Some(slot) = (unsafe {
        userdata
            .cast::<UiCallbackSlot<DecoratorCallback>>()
            .as_ref()
    }) else {
        return;
    };
    let Some(decorator) = (unsafe { decorator.as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in line-decorator callback", || {
        let event = DecoratorEvent {
            line: decorator.line,
            width: decorator.width,
            height: decorator.height,
            glyph_size: [decorator.glyphSize.x, decorator.glyphSize.y],
        };
        slot.invoke(|callback, ui| callback(ui, event));
    });
}

unsafe extern "C" fn caret_trampoline(userdata: *mut c_void, caret: *const sys::CustomCaret) {
    let Some(slot) = (unsafe { userdata.cast::<UiCallbackSlot<CaretCallback>>().as_ref() }) else {
        return;
    };
    let Some(caret) = (unsafe { caret.as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in custom-caret callback", || {
        let event = CaretEvent {
            glyph_position: [caret.glyphPos.x, caret.glyphPos.y],
            glyph_size: [caret.glyphSize.x, caret.glyphSize.y],
            visible: caret.caretVisible,
            color: caret.caretColor,
            cursor: caret.cursorIndex,
        };
        slot.invoke(|callback, ui| callback(ui, event));
    });
}

unsafe extern "C" fn popup_trampoline(userdata: *mut c_void, popup: *mut sys::PopupData) {
    let Some(slot) = (unsafe { userdata.cast::<UiCallbackSlot<PopupCallback>>().as_ref() }) else {
        return;
    };
    let Some(popup) = (unsafe { popup.as_ref() }) else {
        return;
    };
    abort_on_panic("dear-imgui-cte: panic in popup callback", || {
        let event = PopupEvent {
            position: Position::from_raw(popup.pos),
        };
        slot.invoke(|callback, ui| callback(ui, event));
    });
}

unsafe extern "C" fn identifier_trampoline<F: FnMut(&str)>(
    userdata: *mut c_void,
    identifier: *const c_char,
    identifier_len: usize,
) {
    let Some(state) = (unsafe { userdata.cast::<IdentifierState<'_, F>>().as_mut() }) else {
        return;
    };
    if state.invoking.replace(true) {
        return;
    }
    let _guard = InvocationGuard(&state.invoking);
    abort_on_panic("dear-imgui-cte: panic in identifier callback", || {
        let identifier = unsafe { callback_str(identifier, identifier_len, "identifier callback") };
        let callback = unsafe { &mut **state.callback.get() };
        callback(identifier);
    });
}

unsafe extern "C" fn filter_trampoline(
    userdata: *mut c_void,
    _input: *const c_char,
    _input_len: usize,
    output: *mut *const c_char,
    output_len: *mut usize,
) -> sys::DearImGuiCteStatus {
    let Some(state) = (unsafe { userdata.cast::<FilterState>().as_mut() }) else {
        return sys::DearImGuiCteStatus_NullArgument;
    };
    if output.is_null() || output_len.is_null() || state.invoking.replace(true) {
        return sys::DearImGuiCteStatus_CallbackFailed;
    }
    let _guard = InvocationGuard(&state.invoking);
    let Some(value) = state.outputs.get(state.next) else {
        return sys::DearImGuiCteStatus_CallbackFailed;
    };
    state.next += 1;
    unsafe {
        *output = value.as_ptr();
        *output_len = value.as_bytes().len();
    }
    sys::DearImGuiCteStatus_Ok
}

unsafe fn callback_str<'a>(raw: *const c_char, len: usize, name: &str) -> &'a str {
    let bytes = unsafe { callback_bytes(raw, len) };
    match str::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("dear-imgui-cte: invalid UTF-8 in {name}");
            std::process::abort();
        }
    }
}

unsafe fn callback_bytes<'a>(raw: *const c_char, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else if raw.is_null() {
        eprintln!("dear-imgui-cte: null pointer with non-zero callback length");
        std::process::abort();
    } else {
        unsafe { slice::from_raw_parts(raw.cast(), len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn callback_trampoline_reentry_is_a_no_op_and_resets_after_return() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_callback = Rc::clone(&calls);
        let userdata = Rc::new(Cell::new(ptr::null_mut()));
        let userdata_for_callback = Rc::clone(&userdata);
        let slot = CallbackSlot::new(Box::new(move || {
            calls_for_callback.set(calls_for_callback.get() + 1);
            unsafe { change_trampoline(userdata_for_callback.get()) };
        }) as Box<EmptyCallback>);
        userdata.set(slot.userdata());

        unsafe { change_trampoline(slot.userdata()) };
        unsafe { change_trampoline(slot.userdata()) };

        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn ui_trampolines_copy_event_values_and_receive_the_active_ui() {
        use dear_imgui_rs::{Context, FramePrepareOptions};

        let transaction_seen = Rc::new(RefCell::new(None));
        let seen = Rc::clone(&transaction_seen);
        let transaction = CallbackSlot::new(Box::new(move |event: TextChange<'_>| {
            *seen.borrow_mut() = Some((event.kind, event.range, event.text.to_owned()));
        }) as Box<TransactionCallback>);
        let transaction_text = "change";
        let raw_transaction = sys::DearImGuiCteChangeView {
            insert: false,
            start: Position::new(2, 3).into_raw(),
            end: Position::new(4, 5).into_raw(),
            text: transaction_text.as_ptr().cast(),
            text_len: transaction_text.len(),
        };
        unsafe { transaction_trampoline(transaction.userdata(), &raw_transaction) };
        assert_eq!(
            transaction_seen.borrow().clone(),
            Some((
                TextChangeKind::Delete,
                Selection::new(Position::new(2, 3), Position::new(4, 5)),
                transaction_text.to_owned(),
            ))
        );

        let mut context = Context::create();
        context.prepare_frame(FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0));
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let expected_context = context.id();
        let ui = context.frame();
        let registry = CallbackRegistry::new();
        let _active = registry.enter_ui(ui);

        let expected_decorator = DecoratorEvent {
            line: 7,
            width: 2.0,
            height: 18.0,
            glyph_size: [8.0, 16.0],
        };
        let decorator_seen = Rc::new(Cell::new(None));
        let seen = Rc::clone(&decorator_seen);
        let decorator = UiCallbackSlot::new(
            Rc::clone(&registry.active_ui),
            Box::new(move |ui: &Ui, event: DecoratorEvent| {
                assert_eq!(ui.context_id(), expected_context);
                seen.set(Some(event));
            }) as Box<DecoratorCallback>,
        );
        let mut raw_decorator = sys::Decorator {
            line: 7,
            width: 2.0,
            height: 18.0,
            glyphSize: [8.0, 16.0].into(),
            userData: ptr::null_mut(),
        };
        unsafe { decorator_trampoline(decorator.userdata(), &mut raw_decorator) };
        assert_eq!(decorator_seen.get(), Some(expected_decorator));

        let expected_caret = CaretEvent {
            glyph_position: [10.0, 20.0],
            glyph_size: [8.0, 16.0],
            visible: true,
            color: 0x1122_3344,
            cursor: 3,
        };
        let caret_seen = Rc::new(Cell::new(None));
        let seen = Rc::clone(&caret_seen);
        let caret = UiCallbackSlot::new(
            Rc::clone(&registry.active_ui),
            Box::new(move |_ui: &Ui, event: CaretEvent| seen.set(Some(event)))
                as Box<CaretCallback>,
        );
        let raw_caret = sys::CustomCaret {
            drawList: ptr::null_mut(),
            glyphPos: [10.0, 20.0].into(),
            glyphSize: [8.0, 16.0].into(),
            caretVisible: true,
            caretColor: 0x1122_3344,
            cursorIndex: 3,
        };
        unsafe { caret_trampoline(caret.userdata(), &raw_caret) };
        assert_eq!(caret_seen.get(), Some(expected_caret));

        let popup_seen = Rc::new(Cell::new(None));
        let seen = Rc::clone(&popup_seen);
        let popup = UiCallbackSlot::new(
            Rc::clone(&registry.active_ui),
            Box::new(move |_ui: &Ui, event: PopupEvent| seen.set(Some(event.position)))
                as Box<PopupCallback>,
        );
        let mut raw_popup = sys::PopupData {
            pos: Position::new(4, 5).into_raw(),
            userData: ptr::null_mut(),
        };
        unsafe { popup_trampoline(popup.userdata(), &mut raw_popup) };
        assert_eq!(popup_seen.get(), Some(Position::new(4, 5)));

        drop(context.render_legacy());
    }
}
