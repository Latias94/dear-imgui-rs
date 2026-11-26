use super::validation::assert_finite_vec2;
use crate::{EditorContext, PinId, context::FrameState, sys, vec2};
use dear_imgui_rs::{NativeWindowScope, Ui};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    ffi::CString,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScopeKind {
    Node,
    Pin,
    Group,
    GroupHint,
    Create,
    Delete,
    Shortcut,
    Suspension,
    StyleColor,
    StyleVar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeLane {
    Build,
    Color,
    Var,
}

impl ScopeKind {
    fn lane(self) -> ScopeLane {
        match self {
            Self::StyleColor => ScopeLane::Color,
            Self::StyleVar => ScopeLane::Var,
            _ => ScopeLane::Build,
        }
    }

    // The caller has bound the editor that opened this scope.
    unsafe fn end_native(self) {
        unsafe {
            match self {
                Self::Node | Self::Group => sys::dne_end_node(),
                Self::Pin => sys::dne_end_pin(),
                Self::GroupHint => sys::dne_end_group_hint(),
                Self::Create => sys::dne_end_create(),
                Self::Delete => sys::dne_end_delete(),
                Self::Shortcut => sys::dne_end_shortcut(),
                Self::Suspension => sys::dne_resume(),
                Self::StyleColor => sys::dne_pop_style_color(1),
                Self::StyleVar => sys::dne_pop_style_var(1),
            }
        }
    }
}

struct ScopeEntry {
    kind: ScopeKind,
    released: bool,
}

#[derive(Default)]
struct ScopeStack {
    entries: Vec<Option<ScopeEntry>>,
}

impl ScopeStack {
    fn push(&mut self, kind: ScopeKind) -> usize {
        let index = self.entries.len();
        self.entries.push(Some(ScopeEntry {
            kind,
            released: false,
        }));
        index
    }

    fn release(&mut self, index: usize) {
        let entry = self.entries[index].as_mut().expect("active scope");
        entry.released = true;
        if entry.kind == ScopeKind::Node {
            // A pin borrows its node. Any pin left when the node is released
            // was forgotten, while a suspension can still have a live token.
            for child in self.entries[index + 1..].iter_mut().flatten() {
                if child.kind == ScopeKind::Pin {
                    child.released = true;
                }
            }
        }
    }

    fn pop_released(&mut self) -> Option<ScopeKind> {
        let mut seen = [false; 3];
        let index = self.entries.iter().rposition(|entry| {
            entry.as_ref().is_some_and(|entry| {
                let first = !std::mem::replace(&mut seen[entry.kind.lane() as usize], true);
                first && entry.released
            })
        })?;
        let entry = self.entries[index].take().expect("released scope");
        while self.entries.last().is_some_and(Option::is_none) {
            self.entries.pop();
        }
        Some(entry.kind)
    }

    fn current_build_scope(&mut self) -> Option<&mut ScopeEntry> {
        self.entries
            .iter_mut()
            .rev()
            .flatten()
            .find(|entry| entry.kind.lane() == ScopeLane::Build && !entry.released)
    }
}

pub(super) struct FrameToken<'a> {
    pub(super) frame: &'a NodeEditorFrame<'a>,
    index: usize,
}

impl Drop for FrameToken<'_> {
    fn drop(&mut self) {
        self.frame.scopes.borrow_mut().release(self.index);
        self.frame.editor.with_current("node-editor scope end", || {
            if self.frame.at_host() {
                self.frame.flush_released();
            } else {
                self.frame.deferred.set(true);
            }
        });
    }
}

/// RAII token for an active node-editor frame.
///
/// A [`Ui::popup`] inside [`Self::with_suspended`] can access this editor's selection,
/// inspect nodes and links, and queue deletion requests. Drawing and interaction
/// sessions use the original canvas.
pub struct NodeEditorFrame<'ui> {
    pub(super) ui: &'ui Ui,
    editor: &'ui EditorContext,
    host: Option<&'ui NativeWindowScope<'ui>>,
    deferred: Cell<bool>,
    scopes: RefCell<ScopeStack>,
    pub(super) submitted_pins: RefCell<HashSet<PinId>>,
}

impl<'ui> NodeEditorFrame<'ui> {
    pub(crate) fn new(
        ui: &'ui Ui,
        editor: &'ui EditorContext,
        id: impl AsRef<str>,
        size: [f32; 2],
        host: Option<&'ui NativeWindowScope<'ui>>,
    ) -> Self {
        assert_finite_vec2("Ui::node_editor()", "size", size);
        assert_eq!(
            ui.context_id(),
            editor.imgui_context_id(),
            "Ui::node_editor() requires a Ui from the editor's owning Dear ImGui context"
        );
        let id = CString::new(id.as_ref()).expect("node editor id cannot contain NUL bytes");
        editor.with_current("Ui::node_editor", || unsafe {
            // Begin can already invoke the application's settings handler.
            assert_eq!(
                editor.frame_state.get(),
                FrameState::Idle,
                "Ui::node_editor() requires the previous frame for this editor to be ended"
            );
            editor.frame_state.set(FrameState::Active);
            sys::dne_begin(id.as_ptr(), vec2(size))
        });
        Self {
            ui,
            editor,
            host,
            deferred: Cell::new(false),
            scopes: RefCell::default(),
            submitted_pins: RefCell::new(HashSet::new()),
        }
    }

    // Editor-owned data is accessible while a popup is current. Native scope ends wait
    // for the canvas because they can touch the host draw list and clip stack.
    #[inline]
    pub(super) fn with_editor<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.editor.with_current(caller, f)
    }

    #[inline]
    pub(super) fn with_canvas<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.with_editor(caller, || {
            assert!(
                self.at_host(),
                "{caller} requires its original window, table cell and innermost node editor canvas"
            );
            if self.deferred.replace(false) {
                self.flush_released();
            }
            f()
        })
    }

    fn at_host(&self) -> bool {
        self.host.is_none_or(NativeWindowScope::is_current)
    }

    pub(super) fn scoped<T, R>(
        &self,
        begin: impl FnOnce() -> T,
        draw: impl FnOnce(&mut T) -> R,
    ) -> R {
        let run = |host: &NativeWindowScope<'_>| {
            host.run(|| {
                let mut token = begin();
                host.run(|| draw(&mut token))
            })
        };
        match self.host {
            Some(host) => run(host),
            None => self.ui.with_native_window_scope(run),
        }
    }

    fn flush_released(&self) {
        loop {
            let Some(kind) = self.scopes.borrow_mut().pop_released() else {
                break;
            };
            unsafe { kind.end_native() };
        }
    }

    pub(super) fn entered(&self, kind: ScopeKind) -> FrameToken<'_> {
        let index = self.scopes.borrow_mut().push(kind);
        FrameToken { frame: self, index }
    }

    pub(super) fn require_scope(&self, expected: Option<ScopeKind>, caller: &str) {
        let actual = self
            .scopes
            .borrow()
            .entries
            .iter()
            .rev()
            .flatten()
            .find(|entry| entry.kind.lane() == ScopeLane::Build && !entry.released)
            .map(|entry| entry.kind);
        assert_eq!(
            actual, expected,
            "{caller} requires {expected:?} scope, found {actual:?}"
        );
    }

    pub(super) fn mark_group(&self) {
        self.scopes
            .borrow_mut()
            .current_build_scope()
            .expect("active node")
            .kind = ScopeKind::Group;
    }

    /// Ends the editor after balancing any remaining native scopes.
    pub fn end(self) {}
}

/// Clear the active state before cleanup, but do not allow a new begin until
/// native End and any callbacks it invokes have returned.
fn finish_editor_frame(state: &Cell<FrameState>, finish: impl FnOnce()) {
    state.set(FrameState::Ending);
    finish();
    state.set(FrameState::Idle);
}

impl Drop for NodeEditorFrame<'_> {
    fn drop(&mut self) {
        let editor = self.editor;
        finish_editor_frame(&editor.frame_state, || {
            editor.with_current("NodeEditorFrame::end()", || unsafe {
                assert!(
                    self.at_host(),
                    "node-editor frame must end on its original canvas"
                );
                // A safe caller can forget a child token. Its native scope still
                // belongs to this frame and must be closed before the editor ends.
                for entry in self.scopes.get_mut().entries.drain(..).rev().flatten() {
                    entry.kind.end_native();
                }
                sys::dne_end();
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameState, ScopeKind, ScopeStack, finish_editor_frame};
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[test]
    fn closing_a_node_collects_its_forgotten_pin_after_the_last_suspension() {
        let mut stack = ScopeStack::default();
        let node = stack.push(ScopeKind::Node);
        stack.push(ScopeKind::Pin);
        let suspension = stack.push(ScopeKind::Suspension);
        stack.release(node);
        assert_eq!(stack.pop_released(), None);
        stack.release(suspension);
        assert_eq!(stack.pop_released(), Some(ScopeKind::Suspension));
        assert_eq!(stack.pop_released(), Some(ScopeKind::Pin));
        assert_eq!(stack.pop_released(), Some(ScopeKind::Node));
        assert_eq!(stack.pop_released(), None);
    }

    #[test]
    fn released_styles_do_not_wait_on_unrelated_native_scopes() {
        let mut stack = ScopeStack::default();
        let outer_color = stack.push(ScopeKind::StyleColor);
        let node = stack.push(ScopeKind::Node);
        let inner_color = stack.push(ScopeKind::StyleColor);
        let var = stack.push(ScopeKind::StyleVar);

        stack.release(outer_color);
        assert_eq!(stack.pop_released(), None);
        stack.release(inner_color);
        assert_eq!(stack.pop_released(), Some(ScopeKind::StyleColor));
        assert_eq!(stack.pop_released(), Some(ScopeKind::StyleColor));
        stack.release(node);
        assert_eq!(stack.pop_released(), Some(ScopeKind::Node));
        stack.release(var);
        assert_eq!(stack.pop_released(), Some(ScopeKind::StyleVar));
        assert!(stack.entries.is_empty());
    }

    #[test]
    fn frame_cleanup_blocks_a_new_frame_if_native_end_did_not_complete() {
        let state = Cell::new(FrameState::Active);
        let result = catch_unwind(AssertUnwindSafe(|| {
            finish_editor_frame(&state, || {
                assert_eq!(state.get(), FrameState::Ending);
                panic!("panic during native frame cleanup");
            });
        }));
        assert!(result.is_err());
        assert_eq!(state.get(), FrameState::Ending);
    }
}
