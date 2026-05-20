use super::super::model::{NodeId, PinId, PinKind};

#[derive(Copy, Clone, Debug, Default)]
pub struct RightClickEvent {
    pub node: Option<NodeId>,
    pub pin: Option<(PinId, PinKind)>,
    pub mouse_pos: [f32; 2],
}
