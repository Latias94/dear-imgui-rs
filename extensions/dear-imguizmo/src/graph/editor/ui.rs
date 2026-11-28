use super::super::model::{Graph, GraphView};
use super::super::style::GraphStyle;
use super::{GraphEditorResponse, GraphEditorUi, Hooks, draw_core};

impl<'ui> GraphEditorUi<'ui> {
    pub fn draw(&self, graph: &mut Graph, view: &mut GraphView) -> GraphEditorResponse {
        self.ui.with_window_draw_list(|draw_list| {
            draw_core(
                self.ui,
                draw_list,
                graph,
                view,
                &GraphStyle::default(),
                &mut Hooks::default(),
            )
        })
    }
}
