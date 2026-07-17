use std::cell::RefCell;
use std::collections::HashMap;

use crate::sys;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClipperScope {
    context: *mut sys::ImGuiContext,
    frame: i32,
    window: *mut sys::ImGuiWindow,
    window_begin_count: i16,
    table: *mut sys::ImGuiTable,
    table_instance: i16,
}

#[derive(Clone, Copy)]
struct ClipperEntry {
    id: u64,
    ptr: *mut sys::ImGuiListClipper,
    scope: ClipperScope,
    abandoned: bool,
}

enum ClipperCleanup {
    OriginalScope(ClipperEntry),
    WithoutLayout(ClipperEntry),
}

#[derive(Clone, Copy)]
pub(super) struct ClipperHandle {
    id: u64,
    ptr: *mut sys::ImGuiListClipper,
    context: *mut sys::ImGuiContext,
}

impl ClipperHandle {
    pub(super) fn ptr(self) -> *mut sys::ImGuiListClipper {
        self.ptr
    }
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

unsafe fn current_scope(context: *mut sys::ImGuiContext, caller: &str) -> ClipperScope {
    assert!(
        !context.is_null(),
        "{caller} requires a valid ImGui context"
    );
    assert_eq!(
        unsafe { sys::igGetCurrentContext() },
        context,
        "{caller} requires the clipper's ImGui context to be current"
    );
    let window = unsafe { sys::igGetCurrentWindow() };
    assert!(
        !window.is_null(),
        "{caller} requires an active ImGui window"
    );
    let table = unsafe { sys::igGetCurrentTable() };
    ClipperScope {
        context,
        frame: unsafe { (*context).FrameCount },
        window,
        window_begin_count: unsafe { (*window).BeginCount },
        table,
        table_instance: unsafe {
            if table.is_null() {
                -1
            } else {
                (*table).InstanceCurrent
            }
        },
    }
}

unsafe fn try_current_scope(context: *mut sys::ImGuiContext) -> Option<ClipperScope> {
    if context.is_null() || unsafe { sys::igGetCurrentContext() } != context {
        return None;
    }
    let window = unsafe { sys::igGetCurrentWindow() };
    if window.is_null() {
        return None;
    }
    let table = unsafe { sys::igGetCurrentTable() };
    Some(ClipperScope {
        context,
        frame: unsafe { (*context).FrameCount },
        window,
        window_begin_count: unsafe { (*window).BeginCount },
        table,
        table_instance: unsafe {
            if table.is_null() {
                -1
            } else {
                (*table).InstanceCurrent
            }
        },
    })
}

pub(super) unsafe fn assert_can_begin(context: *mut sys::ImGuiContext, caller: &str) {
    let scope = unsafe { current_scope(context, caller) };
    CLIPPERS.with(|registry| {
        let registry = registry.borrow();
        let Some(entry) = registry
            .by_context
            .get(&(context as usize))
            .and_then(|stack| stack.last())
        else {
            return;
        };
        assert_eq!(
            entry.scope, scope,
            "{caller} cannot begin a nested list clipper in a different ImGui window or table scope"
        );
    });
}

pub(super) unsafe fn register_current(
    context: *mut sys::ImGuiContext,
    ptr: *mut sys::ImGuiListClipper,
    caller: &str,
) -> ClipperHandle {
    assert!(!ptr.is_null(), "{caller} received a null list clipper");
    let scope = unsafe { current_scope(context, caller) };
    CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let id = registry.next_id;
        registry.next_id = registry
            .next_id
            .checked_add(1)
            .expect("list clipper token counter overflowed");
        registry
            .by_context
            .entry(context as usize)
            .or_default()
            .push(ClipperEntry {
                id,
                ptr,
                scope,
                abandoned: false,
            });
        ClipperHandle { id, ptr, context }
    })
}

pub(super) unsafe fn assert_current(
    handle: ClipperHandle,
    caller: &str,
) -> *mut sys::ImGuiListClipper {
    let scope = unsafe { current_scope(handle.context, caller) };
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
            entry.id, handle.id,
            "{caller} must follow list clipper LIFO order; a nested clipper is still active"
        );
        assert_eq!(
            entry.ptr, handle.ptr,
            "{caller} encountered an invalid list clipper registration"
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

pub(super) unsafe fn complete(handle: ClipperHandle) {
    let current_scope = unsafe { try_current_scope(handle.context) };
    let entries_to_destroy = CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let key = handle.context as usize;
        let stack = registry
            .by_context
            .get_mut(&key)
            .expect("completed list clipper was not registered");
        let entry = stack
            .last()
            .expect("completed list clipper stack was empty");
        assert_eq!(
            entry.id, handle.id,
            "completed list clipper must be the active native LIFO entry"
        );
        assert_eq!(
            entry.ptr, handle.ptr,
            "completed list clipper registration was invalid"
        );
        assert!(!entry.abandoned, "completed list clipper was abandoned");
        let _completed = stack
            .pop()
            .expect("completed list clipper stack entry disappeared");

        let mut entries = Vec::new();
        while stack.last().is_some_and(|entry| entry.abandoned) {
            let entry = stack.pop().expect("abandoned clipper entry disappeared");
            entries.push(if current_scope == Some(entry.scope) {
                ClipperCleanup::OriginalScope(entry)
            } else {
                ClipperCleanup::WithoutLayout(entry)
            });
        }
        if stack.is_empty() {
            registry.by_context.remove(&key);
        }
        entries
    });

    for cleanup in entries_to_destroy {
        match cleanup {
            ClipperCleanup::OriginalScope(entry) => unsafe { destroy_in_original_scope(entry) },
            ClipperCleanup::WithoutLayout(entry) => unsafe { destroy_without_layout(entry) },
        }
    }
}

pub(super) unsafe fn release(handle: ClipperHandle) {
    if handle.context.is_null() || handle.ptr.is_null() {
        return;
    }
    let current_scope = unsafe { try_current_scope(handle.context) };

    let entries_to_destroy = CLIPPERS.with(|registry| {
        let mut registry = registry.borrow_mut();
        let key = handle.context as usize;
        let Some(stack) = registry.by_context.get_mut(&key) else {
            return Vec::new();
        };
        let Some(position) = stack.iter().position(|entry| entry.id == handle.id) else {
            return Vec::new();
        };
        if position + 1 != stack.len() {
            stack[position].abandoned = true;
            return Vec::new();
        }
        let entry = stack.pop().expect("clipper stack was checked as non-empty");
        let mut entries = vec![if current_scope == Some(entry.scope) && !entry.abandoned {
            ClipperCleanup::OriginalScope(entry)
        } else {
            ClipperCleanup::WithoutLayout(entry)
        }];
        while stack.last().is_some_and(|entry| entry.abandoned) {
            let entry = stack.pop().expect("abandoned clipper entry disappeared");
            entries.push(if current_scope == Some(entry.scope) {
                ClipperCleanup::OriginalScope(entry)
            } else {
                ClipperCleanup::WithoutLayout(entry)
            });
        }
        if stack.is_empty() {
            registry.by_context.remove(&key);
        }
        entries
    });

    for cleanup in entries_to_destroy {
        match cleanup {
            ClipperCleanup::OriginalScope(entry) => unsafe { destroy_in_original_scope(entry) },
            ClipperCleanup::WithoutLayout(entry) => unsafe { destroy_without_layout(entry) },
        }
    }
}

unsafe fn destroy_in_original_scope(entry: ClipperEntry) {
    debug_assert_eq!(unsafe { sys::igGetCurrentContext() }, entry.scope.context);
    debug_assert_eq!(unsafe { (*entry.ptr).Ctx }, entry.scope.context);

    debug_assert_eq!(
        unsafe { try_current_scope(entry.scope.context) },
        Some(entry.scope)
    );
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
