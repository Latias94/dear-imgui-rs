use super::super::model::{Graph, GraphView};
use super::super::style::GraphStyle;
use super::{GraphEditorResponse, GraphEditorUi, draw_core};

impl<'ui> GraphEditorUi<'ui> {
    pub fn draw(&self, graph: &mut Graph, view: &mut GraphView) -> GraphEditorResponse {
        draw_core(self.ui, graph, view, &GraphStyle::default(), None)
    }
}
