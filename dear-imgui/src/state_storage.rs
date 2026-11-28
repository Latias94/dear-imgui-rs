//! State storage utilities
//!
//! Dear ImGui provides a per-window key/value storage (`ImGuiStorage`) that is
//! used by many widgets and can also be used by custom widgets to persist state.
//!
use crate::context::binding::CTX_MUTEX;
use crate::{ContextThreadLease, Id, ImGuiResult, sys};
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
    /// `raw` must point to an initialized `ImGuiStorage` that remains live and accessible
    /// on this thread for `'ui`, including through any copies of the returned view. Its
    /// allocation must remain exclusively owned by the storage and its keys must stay sorted.
    /// A Context or [`ContextThreadLease`] must cover any mutation that can allocate or free
    /// entries through Dear ImGui.
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
        let _guard = CTX_MUTEX.lock();
        unsafe { sys::ImGuiStorage_Clear(self.raw.as_ptr()) }
    }

    pub fn get_int(&self, key: Id, default: i32) -> i32 {
        unsafe { sys::ImGuiStorage_GetInt(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_int(&mut self, key: Id, value: i32) {
        let _guard = CTX_MUTEX.lock();
        unsafe { sys::ImGuiStorage_SetInt(self.raw.as_ptr(), key.raw(), value) }
    }

    pub fn get_bool(&self, key: Id, default: bool) -> bool {
        unsafe { sys::ImGuiStorage_GetBool(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_bool(&mut self, key: Id, value: bool) {
        self.set_int(key, i32::from(value));
    }

    pub fn get_float(&self, key: Id, default: f32) -> f32 {
        unsafe { sys::ImGuiStorage_GetFloat(self.raw.as_ptr(), key.raw(), default) }
    }

    pub fn set_float(&mut self, key: Id, value: f32) {
        let _guard = CTX_MUTEX.lock();
        unsafe { sys::ImGuiStorage_SetFloat(self.raw.as_ptr(), key.raw(), value) }
    }
}

/// Owns an `ImGuiStorage` and clears it on drop.
///
/// This is useful when you want to keep widget state outside of the current
/// window storage (e.g. sharing state across windows or providing custom storage
/// for a widget subtree via [`crate::Ui::with_state_storage`]).
#[derive(Debug)]
pub struct OwnedStateStorage {
    raw: sys::ImGuiStorage,
    _thread_lease: ContextThreadLease,
}

impl OwnedStateStorage {
    /// Creates storage on the Dear ImGui owner thread.
    ///
    /// # Panics
    ///
    /// Panics if the thread lease cannot be acquired. Use [`Self::try_new`] to handle this error.
    pub fn new() -> Self {
        Self::try_new().expect("failed to acquire Dear ImGui thread ownership for state storage")
    }

    /// Reserves the current thread for this storage's native entries and creates empty storage.
    pub fn try_new() -> ImGuiResult<Self> {
        let thread_lease = ContextThreadLease::acquire()?;
        Ok(Self {
            raw: sys::ImGuiStorage::default(),
            _thread_lease: thread_lease,
        })
    }

    /// Borrows the native storage for direct mutation.
    ///
    /// # Safety
    ///
    /// Preserve the native vector's allocation, length, capacity, and sorted-key invariants.
    /// This storage must remain the sole owner of any allocation it will clear on drop.
    ///
    /// ```compile_fail,E0133
    /// let mut storage = dear_imgui_rs::OwnedStateStorage::new();
    /// let _ = storage.as_mut();
    /// ```
    pub unsafe fn as_mut(&mut self) -> &mut sys::ImGuiStorage {
        &mut self.raw
    }

    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiStorage {
        &mut self.raw as *mut sys::ImGuiStorage
    }

    pub fn as_raw(&self) -> *const sys::ImGuiStorage {
        &self.raw as *const sys::ImGuiStorage
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.view_mut().clear();
    }

    /// Reads an integer, returning `default` when the key is absent.
    pub fn get_int(&self, key: Id, default: i32) -> i32 {
        unsafe { sys::ImGuiStorage_GetInt(self.as_raw().cast_mut(), key.raw(), default) }
    }

    /// Inserts or replaces an integer entry.
    pub fn set_int(&mut self, key: Id, value: i32) {
        self.view_mut().set_int(key, value);
    }

    /// Reads a boolean, returning `default` when the key is absent.
    pub fn get_bool(&self, key: Id, default: bool) -> bool {
        unsafe { sys::ImGuiStorage_GetBool(self.as_raw().cast_mut(), key.raw(), default) }
    }

    /// Inserts or replaces a boolean entry.
    pub fn set_bool(&mut self, key: Id, value: bool) {
        self.view_mut().set_bool(key, value);
    }

    /// Reads a float, returning `default` when the key is absent.
    pub fn get_float(&self, key: Id, default: f32) -> f32 {
        unsafe { sys::ImGuiStorage_GetFloat(self.as_raw().cast_mut(), key.raw(), default) }
    }

    /// Inserts or replaces a float entry.
    pub fn set_float(&mut self, key: Id, value: f32) {
        self.view_mut().set_float(key, value);
    }

    fn view_mut(&mut self) -> StateStorage<'_> {
        unsafe { StateStorage::from_raw(self.as_raw_mut()) }
    }
}

impl AsRef<sys::ImGuiStorage> for OwnedStateStorage {
    fn as_ref(&self) -> &sys::ImGuiStorage {
        &self.raw
    }
}

impl Default for OwnedStateStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OwnedStateStorage {
    fn drop(&mut self) {
        self.clear();
    }
}

struct StateStorageOverride {
    window: NonNull<sys::ImGuiWindow>,
    previous: *mut sys::ImGuiStorage,
}

impl Drop for StateStorageOverride {
    fn drop(&mut self) {
        // The frame borrow keeps the window alive even if the closure changed the current window.
        unsafe { (*self.window.as_ptr()).DC.StateStorage = self.previous };
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
    /// order, including if `f` panics or leaves a different window current. The replacement
    /// storage and its scoped view cannot escape the closure.
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
            let window = NonNull::new(unsafe { sys::igGetCurrentWindowRead() })
                .expect("with_state_storage requires a current window");
            let previous = unsafe {
                std::ptr::replace(&raw mut (*window.as_ptr()).DC.StateStorage, replacement)
            };
            let storage_override = StateStorageOverride { window, previous };
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
