use crate::{
    Notifications, NotificationsRenderer, TextDiff, TextDiffRenderer, TextEditor,
    TextEditorRenderer,
};
use dear_imgui_rs::Ui;

/// Extension methods for rendering cimCTE widgets from a Dear ImGui frame.
pub trait CteUiExt {
    fn text_editor<'ui, 'editor>(
        &'ui self,
        editor: &'editor mut TextEditor,
        title: impl Into<String>,
    ) -> TextEditorRenderer<'ui, 'editor>;

    fn text_diff<'ui, 'diff>(
        &'ui self,
        diff: &'diff mut TextDiff,
        title: impl Into<String>,
    ) -> TextDiffRenderer<'ui, 'diff>;

    fn notifications<'ui, 'notifications>(
        &'ui self,
        notifications: &'notifications mut Notifications,
    ) -> NotificationsRenderer<'ui, 'notifications>;
}

impl CteUiExt for Ui {
    fn text_editor<'ui, 'editor>(
        &'ui self,
        editor: &'editor mut TextEditor,
        title: impl Into<String>,
    ) -> TextEditorRenderer<'ui, 'editor> {
        TextEditorRenderer::new(self, editor, title.into())
    }

    fn text_diff<'ui, 'diff>(
        &'ui self,
        diff: &'diff mut TextDiff,
        title: impl Into<String>,
    ) -> TextDiffRenderer<'ui, 'diff> {
        TextDiffRenderer::new(self, diff, title.into())
    }

    fn notifications<'ui, 'notifications>(
        &'ui self,
        notifications: &'notifications mut Notifications,
    ) -> NotificationsRenderer<'ui, 'notifications> {
        NotificationsRenderer::new(self, notifications)
    }
}
