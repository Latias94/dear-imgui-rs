use super::TextEditor;
use crate::{
    CteResult,
    error::c_string,
    sys,
    validation::{validate_finite_vec2, validate_render_flags},
};
use dear_imgui_rs::{ChildFlags, Ui, WindowFlags};

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
    pub(crate) fn new<'ui, 'editor>(
        ui: &'ui Ui,
        editor: &'editor mut TextEditor,
        title: String,
    ) -> TextEditorRenderer<'ui, 'editor> {
        TextEditorRenderer {
            ui,
            editor,
            title,
            size: [0.0, 0.0],
            child_flags: ChildFlags::empty(),
            window_flags: WindowFlags::empty(),
        }
    }

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
        validate_render_flags(OPERATION, self.child_flags, self.window_flags)?;
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
