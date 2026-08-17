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
            window_flags: WindowFlags::NO_MOVE | WindowFlags::HORIZONTAL_SCROLLBAR,
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

    /// Adds window flags while preserving the flags required by the upstream editor.
    ///
    /// `NO_MOVE` keeps drag-selection inside the editor instead of moving its host window, while
    /// `HORIZONTAL_SCROLLBAR` lets the editor create its horizontal scrollbar when needed.
    pub fn window_flags(mut self, flags: WindowFlags) -> Self {
        self.window_flags = flags | WindowFlags::NO_MOVE | WindowFlags::HORIZONTAL_SCROLLBAR;
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
        let changed = self.editor.try_with_context(OPERATION, |raw| unsafe {
            sys::TextEditor_Render(
                raw,
                title.as_ptr(),
                self.size.into(),
                child_flags,
                self.window_flags.bits(),
            )
        })?;
        self.editor.layout_ready |= self.ui.is_item_visible();
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::{Context, FramePrepareOptions};

    #[test]
    fn renderer_restores_upstream_interaction_flags() {
        let mut context = Context::create();
        context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless CTE tests require the legacy font-atlas capability")
            .build();
        let mut editor = TextEditor::create(&context);
        assert_eq!(editor.line_height(), None);
        assert_eq!(editor.glyph_width(), None);
        let ui = context.frame();

        let renderer = TextEditorRenderer::new(ui, &mut editor, "Source".to_owned());
        assert_eq!(
            renderer.window_flags,
            WindowFlags::NO_MOVE | WindowFlags::HORIZONTAL_SCROLLBAR
        );

        let renderer = renderer.window_flags(WindowFlags::NO_SAVED_SETTINGS);
        assert!(
            renderer
                .window_flags
                .contains(WindowFlags::NO_MOVE | WindowFlags::HORIZONTAL_SCROLLBAR)
        );
        assert!(
            renderer
                .window_flags
                .contains(WindowFlags::NO_SAVED_SETTINGS)
        );
    }
}
