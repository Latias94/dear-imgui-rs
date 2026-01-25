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
/// for a widget subtree via `Ui::push_state_storage`).
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

/// RAII token that restores the previous state storage on drop.
#[must_use]
pub struct StateStorageToken {
    prev: *mut sys::ImGuiStorage,
}

impl Drop for StateStorageToken {
    fn drop(&mut self) {
        unsafe { sys::igSetStateStorage(self.prev) }
    }
}

impl crate::ui::Ui {
    /// Returns the current window's state storage.
    #[doc(alias = "GetStateStorage")]
    pub fn state_storage(&self) -> StateStorage<'_> {
        unsafe { StateStorage::from_raw(sys::igGetStateStorage()) }
    }

    /// Overrides the current state storage until the returned token is dropped.
    #[doc(alias = "SetStateStorage")]
    pub fn push_state_storage(&self, storage: &mut sys::ImGuiStorage) -> StateStorageToken {
        unsafe {
            let prev = sys::igGetStateStorage();
            sys::igSetStateStorage(storage as *mut sys::ImGuiStorage);
            StateStorageToken { prev }
        }
    }

    /// Set the storage ID for the next item.
    #[doc(alias = "SetNextItemStorageID")]
    pub fn set_next_item_storage_id(&self, storage_id: Id) {
        unsafe { sys::igSetNextItemStorageID(storage_id.raw()) }
    }
}
