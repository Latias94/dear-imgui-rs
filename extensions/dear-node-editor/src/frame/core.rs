use super::validation::assert_finite_vec2;
use crate::{EditorContext, PinId, sys, vec2};
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

impl ScopeKind {
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
    entries: Vec<ScopeEntry>,
}

impl ScopeStack {
    fn push(&mut self, kind: ScopeKind) -> usize {
        let index = self.entries.len();
        self.entries.push(ScopeEntry {
            kind,
            released: false,
        });
        index
    }

    fn release(&mut self, index: usize) {
        self.entries[index].released = true;
        if self.entries[index].kind == ScopeKind::Node {
            // A pin borrows its node. Any pin left when the node is released
            // was forgotten, while a suspension can still have a live token.
            for child in &mut self.entries[index + 1..] {
                if child.kind == ScopeKind::Pin {
                    child.released = true;
                }
            }
        }
    }

    fn pop_released(&mut self) -> Option<ScopeKind> {
        self.entries
            .pop_if(|entry| entry.released)
            .map(|entry| entry.kind)
    }
}

pub(super) struct FrameToken<'a> {
    pub(super) frame: &'a NodeEditorFrame<'a>,
    kind: ScopeKind,
    index: usize,
}

impl Drop for FrameToken<'_> {
    fn drop(&mut self) {
        let stack = self.frame.stack(self.kind);
        stack.borrow_mut().release(self.index);
        self.frame
            ._editor
            .with_current("node-editor scope end", || {
                if self.frame.at_host() {
                    self.frame.flush_released(stack);
                } else {
                    self.frame.deferred.set(true);
                }
            });
    }
}

/// RAII token for an active node-editor frame.
pub struct NodeEditorFrame<'ui> {
    pub(super) _ui: &'ui Ui,
    pub(super) _editor: &'ui EditorContext,
    host: Option<&'ui NativeWindowScope<'ui>>,
    deferred: Cell<bool>,
    build_scopes: RefCell<ScopeStack>,
    style_colors: RefCell<ScopeStack>,
    style_vars: RefCell<ScopeStack>,
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
            assert!(
                !editor.frame_active.replace(true),
                "Ui::node_editor() requires the previous frame for this editor to be ended"
            );
            sys::dne_begin(id.as_ptr(), vec2(size))
        });
        Self {
            _ui: ui,
            _editor: editor,
            host,
            deferred: Cell::new(false),
            build_scopes: RefCell::default(),
            style_colors: RefCell::default(),
            style_vars: RefCell::default(),
            submitted_pins: RefCell::new(HashSet::new()),
        }
    }

    #[inline]
    pub(super) fn with_current<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self._editor.with_current(caller, || {
            assert!(
                self.at_host(),
                "{caller} requires its original window, table cell and innermost node editor canvas"
            );
            if self.deferred.replace(false) {
                for stack in [&self.build_scopes, &self.style_colors, &self.style_vars] {
                    self.flush_released(stack);
                }
            }
            f()
        })
    }

    fn at_host(&self) -> bool {
        self.host.is_none_or(NativeWindowScope::is_current)
    }

    pub(super) fn scoped<T, R>(&self, begin: impl FnOnce() -> T, draw: impl FnOnce(&T) -> R) -> R {
        let run = |host: &NativeWindowScope<'_>| {
            host.run(|| {
                let token = begin();
                host.run(|| draw(&token))
            })
        };
        match self.host {
            Some(host) => run(host),
            None => self._ui.with_native_window_scope(run),
        }
    }

    fn flush_released(&self, stack: &RefCell<ScopeStack>) {
        loop {
            let Some(kind) = stack.borrow_mut().pop_released() else {
                break;
            };
            unsafe { kind.end_native() };
        }
    }

    fn stack(&self, kind: ScopeKind) -> &RefCell<ScopeStack> {
        match kind {
            ScopeKind::StyleColor => &self.style_colors,
            ScopeKind::StyleVar => &self.style_vars,
            _ => &self.build_scopes,
        }
    }

    pub(super) fn entered(&self, kind: ScopeKind) -> FrameToken<'_> {
        let index = self.stack(kind).borrow_mut().push(kind);
        FrameToken {
            frame: self,
            kind,
            index,
        }
    }

    pub(super) fn require_scope(&self, expected: Option<ScopeKind>, caller: &str) {
        let actual = self
            .build_scopes
            .borrow()
            .entries
            .iter()
            .rev()
            .find(|entry| !entry.released)
            .map(|entry| entry.kind);
        assert_eq!(
            actual, expected,
            "{caller} requires {expected:?} scope, found {actual:?}"
        );
    }

    pub(super) fn mark_group(&self) {
        self.build_scopes
            .borrow_mut()
            .entries
            .last_mut()
            .expect("active node")
            .kind = ScopeKind::Group;
    }

    /// Ends the editor after balancing any remaining native scopes.
    pub fn end(self) {}
}

impl Drop for NodeEditorFrame<'_> {
    fn drop(&mut self) {
        self._editor
            .with_current("NodeEditorFrame::end()", || unsafe {
                assert!(
                    self.at_host(),
                    "node-editor frame must end on its original canvas"
                );
                // A safe caller can forget a child token. Its native scope still
                // belongs to this frame and must be closed before the editor ends.
                for stack in [
                    &mut self.build_scopes,
                    &mut self.style_colors,
                    &mut self.style_vars,
                ] {
                    for entry in stack.get_mut().entries.drain(..).rev() {
                        entry.kind.end_native();
                    }
                }
                sys::dne_end();
            });
        self._editor.frame_active.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::{ScopeKind, ScopeStack};

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
}
