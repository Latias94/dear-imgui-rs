use std::collections::HashSet;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PinId(pub u32);
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinkId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PinKind {
    Input,
    Output,
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub id: PinId,
    pub label: String,
    pub kind: PinKind,
    pub color: Option<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub pos: [f32; 2],
    pub title: String,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
}

impl Node {
    pub fn new<P: Vec2Like, S: Into<String>>(id: NodeId, pos: P, title: S) -> Self {
        Self {
            id,
            pos: pos.to_array(),
            title: title.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

impl Pin {
    pub fn new<S: Into<String>>(id: PinId, label: S, kind: PinKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            color: None,
        }
    }
    pub fn colored<S: Into<String>>(id: PinId, label: S, kind: PinKind, color: [f32; 4]) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            color: Some(color),
        }
    }
    pub fn set_color(&mut self, color: Option<[f32; 4]>) {
        self.color = color;
    }
}

#[derive(Clone, Debug)]
pub struct Link {
    pub id: LinkId,
    pub from: PinId,
    pub to: PinId,
}

#[derive(Default, Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    next_node_id: u32,
    next_pin_id: u32,
    next_link_id: u32,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn alloc_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        NodeId(id)
    }
    pub fn alloc_pin_id(&mut self) -> PinId {
        let id = self.next_pin_id;
        self.next_pin_id += 1;
        PinId(id)
    }
    pub fn alloc_link_id(&mut self) -> LinkId {
        let id = self.next_link_id;
        self.next_link_id += 1;
        LinkId(id)
    }
}

#[derive(Debug)]
pub struct GraphView {
    pub pan: [f32; 2],
    pub zoom: f32,
    pub(super) zoom_target: f32,
    // transient state
    pub(super) last_mouse: [f32; 2],
    pub(super) dragging_node: Option<(NodeId, [f32; 2])>, // (node, offset in world)
    pub(super) active_pin: Option<PinId>,
    // selection
    pub selected_nodes: HashSet<NodeId>,
    pub selected_links: HashSet<LinkId>,
    pub(super) box_select_start: Option<[f32; 2]>,
    // hover states
    pub(super) hovered_node: Option<NodeId>,
    pub(super) hovered_link: Option<LinkId>,
    pub(super) hovered_pin: Option<(PinId, NodeId, PinKind)>,
    // reconnect state
    pub(super) reconnecting: Option<(LinkId, LinkEnd)>,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            pan: [0.0, 0.0],
            zoom: 1.0,
            zoom_target: 1.0,
            last_mouse: [0.0, 0.0],
            dragging_node: None,
            active_pin: None,
            selected_nodes: HashSet::new(),
            selected_links: HashSet::new(),
            box_select_start: None,
            hovered_node: None,
            hovered_link: None,
            hovered_pin: None,
            reconnecting: None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum LinkEnd {
    From,
    To,
}

// Simple 2D vector adaptor for glam/mint/[f32;2]
pub trait Vec2Like {
    fn to_array(self) -> [f32; 2];
}
impl Vec2Like for [f32; 2] {
    fn to_array(self) -> [f32; 2] {
        self
    }
}
impl Vec2Like for (f32, f32) {
    fn to_array(self) -> [f32; 2] {
        [self.0, self.1]
    }
}
#[cfg(feature = "glam")]
impl Vec2Like for glam::Vec2 {
    fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}
#[cfg(feature = "mint")]
impl Vec2Like for mint::Vector2<f32> {
    fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }
}
