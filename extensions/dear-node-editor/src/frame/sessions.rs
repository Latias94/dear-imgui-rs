use super::core::{FrameToken, NodeEditorFrame, ScopeKind};
use super::queries::{collect_link_ids, collect_node_ids};
use super::validation::{assert_finite_vec4, assert_non_negative_finite_f32};
use crate::{LinkId, NodeId, PinId, sys, vec4};
use std::cell::Cell;

impl<'ui> NodeEditorFrame<'ui> {
    /// Queries the current creation candidate before drawing its preview. A successful
    /// query enters screen space and keeps its clip rectangle until the callback returns.
    pub fn with_create<R>(
        &self,
        color: [f32; 4],
        thickness: f32,
        draw: impl FnOnce(CreateCandidate, &CreateResponse<'_>) -> R,
    ) -> Option<R> {
        self.scoped(
            || {
                let session = unsafe { self.begin_create(color, thickness)? };
                let candidate = if let Some((start, end)) = session.query_new_link() {
                    CreateCandidate::Link { start, end }
                } else {
                    CreateCandidate::Node {
                        pin: session.query_new_node()?,
                    }
                };
                Some((candidate, session))
            },
            |active| {
                active
                    .as_ref()
                    .map(|(item, session)| draw(*item, &session.response))
            },
        )
    }

    /// # Safety
    /// The first successful candidate query suspends the canvas and pushes a clip rectangle.
    /// Keep earlier drawing scopes active and balance subsequent ones before ending the
    /// session. [`Self::with_create`] owns the query and brackets application drawing.
    pub unsafe fn begin_create<'a>(
        &'a self,
        color: [f32; 4],
        thickness: f32,
    ) -> Option<CreateSession<'a>> {
        assert_finite_vec4("NodeEditorFrame::begin_create()", "color", color);
        assert_non_negative_finite_f32("NodeEditorFrame::begin_create()", "thickness", thickness);
        self.require_scope(None, "NodeEditorFrame::begin_create()");
        let active = self.with_current("NodeEditorFrame::begin_create()", || unsafe {
            sys::dne_begin_create(vec4(color), thickness)
        });
        // Upstream enters its create interaction even when no item is offered.
        if !active {
            self.with_current("NodeEditorFrame::begin_create()", || unsafe {
                sys::dne_end_create()
            });
            return None;
        }
        Some(CreateSession {
            response: CreateResponse {
                scope: self.entered(ScopeKind::Create),
            },
        })
    }

    pub fn begin_delete<'a>(&'a self) -> Option<DeleteSession<'a>> {
        self.require_scope(None, "NodeEditorFrame::begin_delete()");
        self.with_current("NodeEditorFrame::begin_delete()", || unsafe {
            sys::dne_begin_delete()
        })
        .then(|| DeleteSession {
            scope: self.entered(ScopeKind::Delete),
            candidate: Cell::new(false),
        })
    }

    pub fn begin_shortcut<'a>(&'a self) -> Option<ShortcutSession<'a>> {
        self.require_scope(None, "NodeEditorFrame::begin_shortcut()");
        self.with_current("NodeEditorFrame::begin_shortcut()", || unsafe {
            sys::dne_begin_shortcut()
        })
        .then(|| ShortcutSession {
            scope: self.entered(ScopeKind::Shortcut),
        })
    }
}

pub struct CreateSession<'a> {
    response: CreateResponse<'a>,
}

/// Candidate returned by the upstream creation interaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateCandidate {
    Link { start: PinId, end: PinId },
    Node { pin: PinId },
}

/// Accepts or rejects the creation candidate currently being previewed.
pub struct CreateResponse<'a> {
    scope: FrameToken<'a>,
}

impl CreateSession<'_> {
    pub fn query_new_link(&self) -> Option<(PinId, PinId)> {
        self.response
            .scope
            .frame
            .with_current("CreateSession::query_new_link()", || {
                let mut start = 0usize;
                let mut end = 0usize;
                unsafe { sys::dne_query_new_link(&mut start, &mut end) }
                    .then_some((PinId(start), PinId(end)))
            })
    }

    pub fn query_new_link_styled(&self, color: [f32; 4], thickness: f32) -> Option<(PinId, PinId)> {
        assert_finite_vec4("CreateSession::query_new_link_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::query_new_link_styled()",
            "thickness",
            thickness,
        );
        self.response
            .scope
            .frame
            .with_current("CreateSession::query_new_link_styled()", || {
                let mut start = 0usize;
                let mut end = 0usize;
                unsafe {
                    sys::dne_query_new_link_styled(&mut start, &mut end, vec4(color), thickness)
                }
                .then_some((PinId(start), PinId(end)))
            })
    }

    pub fn query_new_node(&self) -> Option<PinId> {
        self.response
            .scope
            .frame
            .with_current("CreateSession::query_new_node()", || {
                let mut pin = 0usize;
                unsafe { sys::dne_query_new_node(&mut pin) }.then_some(PinId(pin))
            })
    }

    pub fn query_new_node_styled(&self, color: [f32; 4], thickness: f32) -> Option<PinId> {
        assert_finite_vec4("CreateSession::query_new_node_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::query_new_node_styled()",
            "thickness",
            thickness,
        );
        self.response
            .scope
            .frame
            .with_current("CreateSession::query_new_node_styled()", || {
                let mut pin = 0usize;
                unsafe { sys::dne_query_new_node_styled(&mut pin, vec4(color), thickness) }
                    .then_some(PinId(pin))
            })
    }

    pub fn accept_new_item(&self) -> bool {
        self.response.accept_new_item()
    }
    pub fn accept_new_item_styled(&self, color: [f32; 4], thickness: f32) -> bool {
        self.response.accept_new_item_styled(color, thickness)
    }
    pub fn reject_new_item(&self) {
        self.response.reject_new_item()
    }
    pub fn reject_new_item_styled(&self, color: [f32; 4], thickness: f32) {
        self.response.reject_new_item_styled(color, thickness)
    }
    pub fn end(self) {}
}

impl CreateResponse<'_> {
    pub fn accept_new_item(&self) -> bool {
        self.scope
            .frame
            .with_current("CreateSession::accept_new_item()", || unsafe {
                sys::dne_accept_new_item()
            })
    }

    pub fn accept_new_item_styled(&self, color: [f32; 4], thickness: f32) -> bool {
        assert_finite_vec4("CreateSession::accept_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::accept_new_item_styled()",
            "thickness",
            thickness,
        );
        self.scope
            .frame
            .with_current("CreateSession::accept_new_item_styled()", || unsafe {
                sys::dne_accept_new_item_styled(vec4(color), thickness)
            })
    }

    pub fn reject_new_item(&self) {
        self.scope
            .frame
            .with_current("CreateSession::reject_new_item()", || unsafe {
                sys::dne_reject_new_item()
            });
    }

    pub fn reject_new_item_styled(&self, color: [f32; 4], thickness: f32) {
        assert_finite_vec4("CreateSession::reject_new_item_styled()", "color", color);
        assert_non_negative_finite_f32(
            "CreateSession::reject_new_item_styled()",
            "thickness",
            thickness,
        );
        self.scope
            .frame
            .with_current("CreateSession::reject_new_item_styled()", || unsafe {
                sys::dne_reject_new_item_styled(vec4(color), thickness)
            });
    }
}

pub struct DeleteSession<'a> {
    scope: FrameToken<'a>,
    candidate: Cell<bool>,
}

impl DeleteSession<'_> {
    pub fn query_deleted_link(&self) -> Option<(LinkId, PinId, PinId)> {
        let result =
            self.scope
                .frame
                .with_current("DeleteSession::query_deleted_link()", || {
                    let mut link = 0usize;
                    let mut start = 0usize;
                    let mut end = 0usize;
                    unsafe { sys::dne_query_deleted_link(&mut link, &mut start, &mut end) }
                        .then_some((LinkId(link), PinId(start), PinId(end)))
                });
        self.candidate.set(result.is_some());
        result
    }

    pub fn query_deleted_node(&self) -> Option<NodeId> {
        let result = self
            .scope
            .frame
            .with_current("DeleteSession::query_deleted_node()", || {
                let mut node = 0usize;
                unsafe { sys::dne_query_deleted_node(&mut node) }.then_some(NodeId(node))
            });
        self.candidate.set(result.is_some());
        result
    }

    /// Accepts the unconsumed candidate returned by the latest deletion query.
    ///
    /// Panics before FFI if no query succeeded, the query was exhausted, or that
    /// candidate was already accepted or rejected.
    pub fn accept_deleted_item(&self, delete_dependencies: bool) -> bool {
        self.scope
            .frame
            .with_current("DeleteSession::accept_deleted_item()", || unsafe {
                assert!(
                    self.candidate.replace(false),
                    "DeleteSession::accept_deleted_item() requires an unconsumed item returned by the latest query"
                );
                sys::dne_accept_deleted_item(delete_dependencies)
            })
    }

    /// Rejects the unconsumed candidate returned by the latest deletion query.
    ///
    /// Panics before FFI under the same conditions as [`Self::accept_deleted_item`].
    pub fn reject_deleted_item(&self) {
        self.scope
            .frame
            .with_current("DeleteSession::reject_deleted_item()", || unsafe {
                assert!(
                    self.candidate.replace(false),
                    "DeleteSession::reject_deleted_item() requires an unconsumed item returned by the latest query"
                );
                sys::dne_reject_deleted_item()
            });
    }

    pub fn end(self) {}
}

pub struct ShortcutSession<'a> {
    scope: FrameToken<'a>,
}

impl ShortcutSession<'_> {
    pub fn accept_cut(&self) -> bool {
        self.scope
            .frame
            .with_current("ShortcutSession::accept_cut()", || unsafe {
                sys::dne_accept_cut()
            })
    }

    pub fn accept_copy(&self) -> bool {
        self.scope
            .frame
            .with_current("ShortcutSession::accept_copy()", || unsafe {
                sys::dne_accept_copy()
            })
    }

    pub fn accept_paste(&self) -> bool {
        self.scope
            .frame
            .with_current("ShortcutSession::accept_paste()", || unsafe {
                sys::dne_accept_paste()
            })
    }

    pub fn accept_duplicate(&self) -> bool {
        self.scope
            .frame
            .with_current("ShortcutSession::accept_duplicate()", || unsafe {
                sys::dne_accept_duplicate()
            })
    }

    pub fn accept_create_node(&self) -> bool {
        self.scope
            .frame
            .with_current("ShortcutSession::accept_create_node()", || unsafe {
                sys::dne_accept_create_node()
            })
    }

    pub fn action_context_size(&self) -> usize {
        self.scope
            .frame
            .with_current("ShortcutSession::action_context_size()", || unsafe {
                sys::dne_get_action_context_size()
            })
            .max(0) as usize
    }

    pub fn action_context_nodes(&self) -> Vec<NodeId> {
        let count = self.action_context_size();
        self.scope
            .frame
            .with_current("ShortcutSession::action_context_nodes()", || {
                collect_node_ids(count, |ptr, len| unsafe {
                    sys::dne_get_action_context_nodes(ptr, len)
                })
            })
    }

    pub fn action_context_links(&self) -> Vec<LinkId> {
        let count = self.action_context_size();
        self.scope
            .frame
            .with_current("ShortcutSession::action_context_links()", || {
                collect_link_ids(count, |ptr, len| unsafe {
                    sys::dne_get_action_context_links(ptr, len)
                })
            })
    }

    pub fn end(self) {}
}
