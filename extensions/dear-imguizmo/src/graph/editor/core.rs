use dear_imgui_rs::Ui;

use super::super::model::{Graph, GraphView};
use super::super::style::GraphStyle;
use super::hooks::Hooks;

pub struct GraphEditorUi<'ui> {
    pub(super) ui: &'ui Ui,
}

pub trait GraphEditorExt {
    fn graph_editor(&self) -> GraphEditorUi<'_>;
    fn graph_editor_config(&self) -> GraphEditor<'_>;
}
impl GraphEditorExt for Ui {
    fn graph_editor(&self) -> GraphEditorUi<'_> {
        GraphEditorUi { ui: self }
    }
    fn graph_editor_config(&self) -> GraphEditor<'_> {
        GraphEditor::new(self)
    }
}

/// Builder-style API aligned with dear-imgui patterns
pub struct GraphEditor<'ui> {
    pub(super) ui: &'ui Ui,
    pub(super) graph: Option<&'ui mut Graph>,
    pub(super) view: Option<&'ui mut GraphView>,
    pub(super) style: GraphStyle,
    pub(super) hooks: Hooks<'ui>,
}
