use super::*;

pub(super) fn escape_field_path_key(key: &str) -> String {
    // Minimal escaping so a key can be embedded inside `["..."]`.
    key.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Per-popup temporary key buffers for map insertion popups, keyed by popup id.
///
/// This allows users to type a key for a new map entry across multiple frames
/// before confirming insertion, similar to ImReflect's `temp_key` storage.
static MAP_ADD_KEY_STATE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub(super) fn map_add_state() -> &'static Mutex<HashMap<String, String>> {
    MAP_ADD_KEY_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    /// Per-popup temporary value buffers for map insertion popups, keyed by
    /// `(TypeId, popup_id)`. This allows users to edit the value for a new
    /// entry across multiple frames before confirming insertion.
    static MAP_ADD_VALUE_STATE: RefCell<HashMap<(TypeId, String), Box<dyn Any>>> =
        RefCell::new(HashMap::new());
}

pub(super) fn with_map_add_value_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<(TypeId, String), Box<dyn Any>>) -> R,
{
    MAP_ADD_VALUE_STATE.with(|cell| {
        let mut map = cell.borrow_mut();
        f(&mut map)
    })
}

pub(super) fn with_temp_map_value<V, F>(popup_id: &str, f: F)
where
    V: Default + 'static,
    F: FnOnce(&mut V),
{
    let key = (TypeId::of::<V>(), popup_id.to_string());
    with_map_add_value_state(|values| {
        let entry = values
            .entry(key.clone())
            .or_insert_with(|| Box::<V>::new(V::default()) as Box<dyn Any>);
        let temp_value = entry
            .downcast_mut::<V>()
            .expect("map_add_value_state type mismatch");
        f(temp_value)
    })
}
