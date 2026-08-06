//! State storage utilities
//!
//! Dear ImGui provides a per-window key/value storage (`ImGuiStorage`) that is
//! used by many widgets and can also be used by custom widgets to persist state.
//!
use crate::{Id, sys};
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A non-owning reference to an `ImGuiStorage` belonging to the current context.
#[derive(Copy, Clone, Debug)]
pub struct StateStorage<'ui> {
    raw: NonNull<sys::ImGuiStorage>,
    _phantom: PhantomData<&'ui mut sys::ImGuiStorage>,
}

impl<'ui> StateStorage<'ui> {
    /// # Safety
    /// `raw` must be a valid, non-null pointer to an `ImGuiStorage`.
    pub unsafe fn from_raw(raw: *mut sys::ImGuiStorage) -> Self {
        let raw = NonNull::new(raw).expect("StateStorage::from_raw() requires non-null pointer");
        Self {
            raw,
            _phantom: PhantomData,
        }
    }

    /// Returns the raw `ImGuiStorage*`.
    pub fn as_raw(self) -> *mut sys::ImGuiStorage {
        self.raw.as_ptr()
    }

    /// Clears all storage entries.
    pub fn clear(&mut self) {
        unsafe { sys::ImGuiStorage_Clear(self.raw.as_ptr()) }
    }

    pub fn get_int(&self, key: Id, default: i32) -> i32 {
        unsafe { sys::ImGuiStorage_GetInt(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_int(&mut self, key: Id, value: i32) {
        unsafe { sys::ImGuiStorage_SetInt(self.raw.as_ptr(), key.raw(), value) }
    }

    pub fn get_bool(&self, key: Id, default: bool) -> bool {
        unsafe { sys::ImGuiStorage_GetBool(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_bool(&mut self, key: Id, value: bool) {
        unsafe { sys::ImGuiStorage_SetBool(self.raw.as_ptr(), key.raw(), value) }
    }

    pub fn get_float(&self, key: Id, default: f32) -> f32 {
        unsafe { sys::ImGuiStorage_GetFloat(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_float(&mut self, key: Id, value: f32) {
        unsafe { sys::ImGuiStorage_SetFloat(self.raw.as_ptr(), key.raw(), value) }
    }
}

/// Owns an `ImGuiStorage` and clears it on drop.
///
/// This is useful when you want to keep widget state outside of the current
/// window storage (e.g. sharing state across windows or providing custom storage
/// for a widget subtree via [`crate::Ui::with_state_storage`]).
#[derive(Debug, Default)]
pub struct OwnedStateStorage {
    raw: sys::ImGuiStorage,
}

impl OwnedStateStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_mut(&mut self) -> &mut sys::ImGuiStorage {
        &mut self.raw
    }

    pub fn as_ref(&self) -> &sys::ImGuiStorage {
        &self.raw
    }

    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiStorage {
        &mut self.raw as *mut sys::ImGuiStorage
    }

    pub fn as_raw(&self) -> *const sys::ImGuiStorage {
        &self.raw as *const sys::ImGuiStorage
    }
}

impl Drop for OwnedStateStorage {
    fn drop(&mut self) {
        unsafe { sys::ImGuiStorage_Clear(self.as_raw_mut()) }
    }
}

struct StateStorageOverride {
    previous: *mut sys::ImGuiStorage,
}

impl Drop for StateStorageOverride {
    fn drop(&mut self) {
        unsafe { sys::igSetStateStorage(self.previous) }
    }
}

impl crate::ui::Ui {
    /// Accesses the current window's state storage inside a non-escaping closure.
    ///
    /// The storage view cannot outlive this call. The owning context remains current for the
    /// duration of `f`, including nested calls into other contexts.
    #[doc(alias = "GetStateStorage")]
    pub fn with_current_state_storage<R>(
        &self,
        f: impl for<'storage> FnOnce(StateStorage<'storage>) -> R,
    ) -> R {
        self.run_with_bound_context(|| unsafe {
            f(StateStorage::from_raw(sys::igGetStateStorage()))
        })
    }

    /// Overrides the current state storage while `f` runs.
    ///
    /// The owning context remains current throughout the call. Nested overrides restore in LIFO
    /// order, and restoration also runs if `f` panics. The replacement storage and its scoped view
    /// cannot escape the closure.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::{Context, OwnedStateStorage, StateStorage};
    ///
    /// let mut context = Context::create();
    /// let ui = context.frame();
    /// let mut replacement = OwnedStateStorage::new();
    /// let escaped: StateStorage<'_> =
    ///     ui.with_state_storage(&mut replacement, |storage| storage);
    /// # let _ = escaped;
    /// ```
    #[doc(alias = "SetStateStorage")]
    pub fn with_state_storage<R>(
        &self,
        storage: &mut OwnedStateStorage,
        f: impl for<'storage> FnOnce(StateStorage<'storage>) -> R,
    ) -> R {
        self.run_with_bound_context(|| {
            let replacement = storage.as_raw_mut();
            let scoped_storage = unsafe { StateStorage::from_raw(replacement) };
            let previous = unsafe { sys::igGetStateStorage() };
            unsafe { sys::igSetStateStorage(replacement) };
            let storage_override = StateStorageOverride { previous };
            let result = f(scoped_storage);
            drop(storage_override);
            result
        })
    }

    /// Set the storage ID for the next item.
    #[doc(alias = "SetNextItemStorageID")]
    pub fn set_next_item_storage_id(&self, storage_id: Id) {
        self.run_with_bound_context(|| unsafe { sys::igSetNextItemStorageID(storage_id.raw()) });
    }
}
