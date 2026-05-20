use super::super::model::LinkId;
use super::events::RightClickEvent;

#[derive(Default, Debug)]
pub struct GraphEditorResponse {
    pub created_links: Vec<LinkId>,
    pub right_click: Option<RightClickEvent>,
}
