use super::{TextEditor, validate_finite_vec2};
use crate::{CteError, CteResult, error::c_string, sys};
use dear_imgui_rs::{ChildFlags, Ui, WindowFlags};

/// Extension methods for rendering a [`TextEditor`] from a Dear ImGui frame.
pub trait CteUiExt {
    fn text_editor<'ui, 'editor>(
        &'ui self,
        editor: &'editor mut TextEditor,
        title: impl Into<String>,
    ) -> TextEditorRenderer<'ui, 'editor>;
}

impl CteUiExt for Ui {
    fn text_editor<'ui, 'editor>(
        &'ui self,
        editor: &'editor mut TextEditor,
        title: impl Into<String>,
    ) -> TextEditorRenderer<'ui, 'editor> {
        TextEditorRenderer {
            ui: self,
            editor,
            title: title.into(),
            size: [0.0, 0.0],
            child_flags: ChildFlags::empty(),
            window_flags: WindowFlags::empty(),
        }
    }
}

/// Builder for one TextEditor render submission.
#[must_use = "call build() to render the editor"]
pub struct TextEditorRenderer<'ui, 'editor> {
    ui: &'ui Ui,
    editor: &'editor mut TextEditor,
    title: String,
    size: [f32; 2],
    child_flags: ChildFlags,
    window_flags: WindowFlags,
}

impl TextEditorRenderer<'_, '_> {
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    pub fn child_flags(mut self, flags: ChildFlags) -> Self {
        self.child_flags = flags;
        self
    }

    pub fn window_flags(mut self, flags: WindowFlags) -> Self {
        self.window_flags = flags;
        self
    }

    /// Renders the editor and returns whether its text changed this frame.
    pub fn build(self) -> CteResult<bool> {
        const OPERATION: &str = "TextEditorRenderer::build";
        self.editor.binding.require_ui(OPERATION, self.ui)?;
        validate_finite_vec2(OPERATION, "size", self.size)?;
        if !ChildFlags::all().contains(self.child_flags) {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "child_flags",
                requirement: "a supported ChildFlags combination",
            });
        }
        if !WindowFlags::all().contains(self.window_flags) {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "window_flags",
                requirement: "a supported WindowFlags combination",
            });
        }
        let title = c_string(OPERATION, &self.title)?;
        let child_flags = self.child_flags.bits() as i32;
        let _active_ui = self.editor.callbacks.enter_ui(self.ui);
        self.editor.try_with_context(OPERATION, |raw| unsafe {
            sys::TextEditor_Render(
                raw,
                title.as_ptr(),
                self.size.into(),
                child_flags,
                self.window_flags.bits(),
            )
        })
    }
}
