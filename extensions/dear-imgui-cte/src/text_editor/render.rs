use super::TextEditor;
use crate::{CteError, CteResult, error::c_string, sys, vec2};
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
        if !self.size.into_iter().all(f32::is_finite) {
            return Err(CteError::NonFinite {
                operation: OPERATION,
                parameter: "size",
            });
        }
        if self.child_flags.bits() & !ChildFlags::all().bits() != 0 {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "child_flags",
                requirement: "a supported ChildFlags combination",
            });
        }
        if self.window_flags.bits() & !WindowFlags::all().bits() != 0 {
            return Err(CteError::InvalidValue {
                operation: OPERATION,
                parameter: "window_flags",
                requirement: "a supported WindowFlags combination",
            });
        }
        let title = c_string(OPERATION, &self.title)?;
        let child_flags =
            i32::try_from(self.child_flags.bits()).map_err(|_| CteError::InvalidValue {
                operation: OPERATION,
                parameter: "child_flags",
                requirement: "representable by ImGuiChildFlags",
            })?;
        self.editor.try_with_context(OPERATION, |raw| unsafe {
            sys::TextEditor_Render(
                raw,
                title.as_ptr(),
                vec2(self.size),
                child_flags,
                self.window_flags.bits(),
            )
        })
    }
}
