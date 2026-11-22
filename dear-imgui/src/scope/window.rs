use super::{ScopeSnapshot, TableScope};
use crate::{Ui, sys};

#[derive(Debug)]
pub(super) struct WindowBoundary {
    pub(super) first_id: u64,
    host_depth: usize,
    table: Option<TableScope>,
    draw_list: *mut sys::ImDrawList,
}

/// The window `Begin` instance and table cell hosting a native extension.
///
/// Obtained from [`Ui::with_native_window_scope`]. A native extension can borrow this handle
/// to check its host before making a call and to balance core tokens around application code.
#[derive(Debug)]
pub struct NativeWindowScope<'ui> {
    ui: &'ui Ui,
    host: ScopeSnapshot,
    depth: usize,
}

impl NativeWindowScope<'_> {
    /// Returns whether this extension is current in its original frame, window `Begin`, and
    /// table cell. A nested extension has its own identity even if it uses the same window.
    pub fn is_current(&self) -> bool {
        self.ui
            .ctx_binding
            .with_bound_context(|| self.is_current_bound())
    }

    fn is_current_bound(&self) -> bool {
        let tracker = self.ui.native_scopes.borrow();
        // The public handle is only ever borrowed from its closure, so its stack position
        // cannot be reused while an extension can still call this method.
        tracker
            .window_boundaries
            .last()
            .is_some_and(|boundary| boundary.host_depth == self.depth)
            && self.ui.current_native_scope() == self.host
    }

    /// Runs application code and closes any core native scopes it leaves open, even on unwind.
    /// The surrounding host and its enclosing tokens stay active throughout the call.
    ///
    /// For a nested native `Begin`/`End` pair, construct its guard inside an outer `run`
    /// and call `run` again around application code. The inner call closes application tokens
    /// before the guard ends. The outer call defers earlier tokens until after the guard ends.
    /// Both calls retain the same extension identity.
    ///
    /// # Panics
    ///
    /// Panics if called from a different window or table cell, or during scope order recovery.
    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        self.ui.run_with_bound_context(|| {
            assert!(
                self.is_current_bound(),
                "native extension requires its own window scope and table cell"
            );
            let _boundary = WindowBoundaryGuard::enter(self);
            f()
        })
    }
}

impl Ui {
    /// Keeps the current window and table cell active while a native extension uses them.
    ///
    /// Dropping a core token opened before this boundary defers its native `End` or `Pop`.
    /// Tokens dropped across a nested window or table wait for that container to end as well.
    /// On return or unwind, the boundary closes the core scopes opened inside it in native
    /// stack order. Tokens retained by the caller can then be dropped harmlessly.
    ///
    /// Construct the extension's RAII guard inside this closure and invoke application code
    /// through [`NativeWindowScope::run`]. This restores the core scopes before the guard's
    /// destructor ends the extension, while the outer boundary still holds the host open.
    ///
    /// Inside a table, select a cell before entering. Its row, column, and draw channel are
    /// fixed for the duration of the extension. Nested tables can use their own cells normally.
    /// The extension has exclusive use of its host draw list's channel splitter.
    ///
    /// # Panics
    ///
    /// Panics without a current window, between table cells, or during scope order recovery.
    pub fn with_native_window_scope<R>(&self, f: impl FnOnce(&NativeWindowScope<'_>) -> R) -> R {
        self.run_with_bound_context(|| {
            let host = self.current_native_scope();
            assert!(
                host.window.is_some(),
                "native extension requires a current window"
            );
            if host.table.is_some() {
                // Before the first cell, even querying sort specs can initialize the table's
                // draw splitter and push its clip rect. A list clipper can also end a row
                // while its previous CurrentColumn value is still set.
                assert!(
                    host.column >= 0 && unsafe { (*sys::igGetCurrentTable()).IsInsideRow },
                    "native extension inside a table requires a current table cell"
                );
            }
            let depth = self.native_scopes.borrow().window_boundaries.len();
            let scope = NativeWindowScope {
                ui: self,
                host,
                depth,
            };
            let _boundary = WindowBoundaryGuard::enter(&scope);
            f(&scope)
        })
    }

    pub(crate) fn native_window_scope_holds_table(&self, table: Option<TableScope>) -> bool {
        let tracker = self.native_scopes.borrow();
        table.is_some()
            && tracker
                .window_boundaries
                .iter()
                .any(|scope| scope.table == table)
    }

    pub(crate) fn native_window_scope_depth(&self) -> usize {
        self.native_scopes.borrow().window_boundaries.len()
    }

    pub(crate) fn assert_host_table_can_change(&self, table: Option<TableScope>, operation: &str) {
        assert!(
            !self.native_window_scope_holds_table(table),
            "{operation} cannot change the table cell hosting a native extension"
        );
    }

    pub(crate) fn assert_draw_list_splitter_available(&self, draw_list: *mut sys::ImDrawList) {
        let tracker = self.native_scopes.borrow();
        assert!(
            !tracker
                .window_boundaries
                .iter()
                .any(|scope| scope.draw_list == draw_list),
            "a native extension is using this window's draw list splitter"
        );
    }
}

struct WindowBoundaryGuard<'ui>(&'ui Ui);

impl<'ui> WindowBoundaryGuard<'ui> {
    // The caller has bound the Ui and checked that the host is current.
    fn enter(scope: &NativeWindowScope<'ui>) -> Self {
        let mut tracker = scope.ui.native_scopes.borrow_mut();
        let first_id = tracker.next_id;
        tracker.window_boundaries.push(WindowBoundary {
            first_id,
            host_depth: scope.depth,
            table: scope.host.table,
            draw_list: unsafe { (*sys::igGetCurrentWindowRead()).DrawList },
        });
        Self(scope.ui)
    }
}

impl Drop for WindowBoundaryGuard<'_> {
    fn drop(&mut self) {
        let ui = self.0;
        // Both constructors keep the Ui's context bound until this guard has dropped.
        {
            let mut tracker = ui.native_scopes.borrow_mut();
            let first = tracker
                .window_boundaries
                .last()
                .expect("active native window scope")
                .first_id;
            for entry in tracker
                .entries
                .values_mut()
                .filter(|entry| entry.id >= first)
            {
                entry.abandoned = true;
            }
            tracker.refresh_pending_recovery();
        }
        ui.finish_abandoned_native_scopes();
        {
            let mut tracker = ui.native_scopes.borrow_mut();
            tracker
                .window_boundaries
                .pop()
                .expect("active native window scope");
            tracker.refresh_pending_recovery();
        }
        // The native extension has finished its work at this depth. Restore the clipper
        // temporary stack before releasing any enclosing window or table that was deferred.
        unsafe { crate::list_clipper::drain_finished_clippers(ui) };
        ui.finish_abandoned_native_scopes();
    }
}
