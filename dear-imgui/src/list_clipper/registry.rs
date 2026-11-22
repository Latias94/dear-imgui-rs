use std::cell::RefCell;
use std::collections::HashMap;

use crate::scope::{TableScope, WindowScope};
use crate::{Ui, sys};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClipperScope {
    context: *mut sys::ImGuiContext,
    window: WindowScope,
    table: Option<TableScope>,
}

#[derive(Clone, Copy)]
struct ClipperEntry {
    handle: ClipperHandle,
    ptr: *mut sys::ImGuiListClipper,
    scope: ClipperScope,
    abandoned: bool,
}

#[derive(Clone, Copy)]
pub(super) struct ClipperHandle {
    id: u64,
    context: *mut sys::ImGuiContext,
    boundary_depth: usize,
}

struct ClipperRegistry {
    next_id: u64,
    by_context: HashMap<usize, Vec<ClipperEntry>>,
}

impl Default for ClipperRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            by_context: HashMap::new(),
        }
    }
}

thread_local! {
    static CLIPPERS: RefCell<ClipperRegistry> = RefCell::new(ClipperRegistry::default());
}

unsafe fn current_scope(ui: &Ui, caller: &str) -> ClipperScope {
    let context = ui.context_raw();
    assert!(
        !context.is_null(),
        "{caller} requires a valid ImGui context"
    );
    assert_eq!(
        unsafe { sys::igGetCurrentContext() },
        context,
        "{caller} requires the clipper's ImGui context to be current"
    );
    let scope = ui.current_native_scope();
    let window = scope
        .window()
        .unwrap_or_else(|| panic!("{caller} requires an active ImGui window"));
    ClipperScope {
        context,
        window,
        table: scope.table(),
    }
}

unsafe fn try_current_scope(ui: &Ui) -> Option<ClipperScope> {
    if ui.native_scope_recovery_pending() {
        return None;
    }
    let context = ui.context_raw();
    if context.is_null() || unsafe { sys::igGetCurrentContext() } != context {
        return None;
    }
    let scope = ui.current_native_scope();
    // End can seek the cursor and close a table row. Release native storage without changing
    // layout when an outer clipper is dropped from inside an extension using that row.
    if ui.native_window_scope_holds_table(scope.table()) {
        return None;
    }
    Some(ClipperScope {
        context,
        window: scope.window()?,
        table: scope.table(),
    })
}

pub(super) unsafe fn assert_can_begin(ui: &Ui, caller: &str) {
    let scope = unsafe { current_scope(ui, caller) };
    ui.assert_host_table_can_change(scope.table, caller);
    let context = scope.context;
    CLIPPERS.with(|registry| {
        let registry = registry.borrow();
        let Some(entry) = registry
            .by_context
            .get(&(context as usize))
            .and_then(|stack| stack.last())
        else {
            return;
        };
        if entry.handle.boundary_depth == ui.native_window_scope_depth() {
            assert_eq!(
                entry.scope, scope,
                "{caller} cannot begin a nested list clipper in a different ImGui window or table scope"
            );
        }
    });
}

pub(super) unsafe fn register_current(
    ui: &Ui,
    ptr: *mut sys::ImGuiListClipper,
    caller: &str,
) -> ClipperHandle {
    assert!(!ptr.is_null(), "{caller} received a null list clipper");
    let scope = unsafe { current_scope(ui, caller) };
    let context = scope.context;
    CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let id = registry.next_id;
        registry.next_id = registry
            .next_id
            .checked_add(1)
            .expect("list clipper token counter overflowed");
        let handle = ClipperHandle {
            id,
            context,
            boundary_depth: ui.native_window_scope_depth(),
        };
        registry
            .by_context
            .entry(context as usize)
            .or_default()
            .push(ClipperEntry {
                handle,
                ptr,
                scope,
                abandoned: false,
            });
        handle
    })
}

pub(super) unsafe fn assert_current(
    ui: &Ui,
    handle: ClipperHandle,
    caller: &str,
) -> *mut sys::ImGuiListClipper {
    assert_eq!(
        handle.context,
        ui.context_raw(),
        "{caller} used a list clipper with a foreign Ui"
    );
    let scope = unsafe { current_scope(ui, caller) };
    CLIPPERS.with(|registry| {
        let registry = registry.borrow();
        let stack = registry
            .by_context
            .get(&(handle.context as usize))
            .unwrap_or_else(|| panic!("{caller} used a list clipper that is no longer active"));
        let entry = stack
            .last()
            .unwrap_or_else(|| panic!("{caller} used a list clipper that is no longer active"));
        assert_eq!(
            entry.handle.id, handle.id,
            "{caller} must follow list clipper LIFO order; a nested clipper is still active"
        );
        assert!(
            !entry.abandoned,
            "{caller} used an abandoned list clipper token"
        );
        assert_eq!(
            entry.scope, scope,
            "{caller} must run in the exact frame, window Begin, and table instance where the clipper began"
        );
        entry.ptr
    })
}

pub(super) unsafe fn assert_can_modify(
    ui: &Ui,
    handle: ClipperHandle,
    caller: &str,
) -> *mut sys::ImGuiListClipper {
    assert_eq!(
        handle.boundary_depth,
        ui.native_window_scope_depth(),
        "{caller} cannot modify a list clipper across a native window scope"
    );
    unsafe { assert_current(ui, handle, caller) }
}

pub(super) unsafe fn assert_can_seek(
    ui: &Ui,
    handle: ClipperHandle,
    caller: &str,
) -> *mut sys::ImGuiListClipper {
    let ptr = unsafe { assert_can_modify(ui, handle, caller) };
    ui.assert_host_table_can_change(ui.current_native_scope().table(), caller);
    ptr
}

pub(super) unsafe fn release(ui: &Ui, handle: ClipperHandle) {
    let registered = CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let stack = registry.by_context.get_mut(&(handle.context as usize))?;
        let entry = stack
            .iter_mut()
            .rev()
            .find(|entry| entry.handle.id == handle.id)?;
        entry.abandoned = true;
        Some(())
    });
    if registered.is_some() {
        unsafe { drain_finished(ui) };
    }
}

// Called with the Ui bound, after dropping a token or leaving a native extension boundary.
pub(crate) unsafe fn drain_finished(ui: &Ui) {
    let depth = ui.native_window_scope_depth();
    let entries_to_destroy = CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let key = ui.context_raw() as usize;
        let Some(stack) = registry.by_context.get_mut(&key) else {
            return Vec::new();
        };
        let mut entries = Vec::new();
        while stack.last().is_some_and(|entry| {
            entry.handle.boundary_depth > depth
                || (entry.handle.boundary_depth == depth && entry.abandoned)
        }) {
            entries.push(stack.pop().expect("clipper stack was checked as non-empty"));
        }
        if stack.is_empty() {
            registry.by_context.remove(&key);
        }
        entries
    });

    if !entries_to_destroy.is_empty() {
        let current_scope = unsafe { try_current_scope(ui) };
        for entry in entries_to_destroy {
            if entry.handle.boundary_depth == depth && current_scope == Some(entry.scope) {
                unsafe { destroy_in_original_scope(entry) };
            } else {
                // An escaped clipper must not seek a cursor now owned by its enclosing extension.
                unsafe { destroy_without_layout(entry) };
            }
        }
    }
}

unsafe fn destroy_in_original_scope(entry: ClipperEntry) {
    debug_assert_eq!(unsafe { sys::igGetCurrentContext() }, entry.scope.context);
    debug_assert_eq!(unsafe { (*entry.ptr).Ctx }, entry.scope.context);
    unsafe { sys::ImGuiListClipper_destroy(entry.ptr) };
}

unsafe fn destroy_without_layout(entry: ClipperEntry) {
    let context = entry.scope.context;
    debug_assert!(!context.is_null());
    debug_assert_eq!(unsafe { (*entry.ptr).Ctx }, context);

    unsafe {
        // A negative count suppresses End()'s cursor seek while preserving its
        // native temporary-stack restoration, including nested back-pointers.
        (*entry.ptr).ItemsCount = -1;
        sys::ImGuiListClipper_End(entry.ptr);
        sys::ImGuiListClipper_destroy(entry.ptr);
    }
}

pub(super) unsafe fn forget_context(context: *mut sys::ImGuiContext) -> usize {
    let entries = CLIPPERS.with(|registry| {
        registry
            .borrow_mut()
            .by_context
            .remove(&(context as usize))
            .unwrap_or_default()
    });
    let abandoned = entries.len();
    for entry in entries.into_iter().rev() {
        unsafe { destroy_without_layout(entry) };
    }
    abandoned
}

#[cfg(test)]
pub(super) fn active_count(context: *mut sys::ImGuiContext) -> usize {
    CLIPPERS.with(|registry| {
        registry
            .borrow()
            .by_context
            .get(&(context as usize))
            .map_or(0, Vec::len)
    })
}
