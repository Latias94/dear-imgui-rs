//! Shared container helpers for dear-imgui-reflect.
//!
//! This module centralizes the editing logic for arrays, vectors and
//! string-keyed maps, including the temporary state needed for map insertion
//! popups and the emission of [`ReflectEvent`](crate::ReflectEvent) values.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;
use std::sync::{Mutex, OnceLock};

use crate::response;
use crate::{ArraySettings, ImGuiValue, MapSettings, VecSettings, imgui};

/// Per-popup temporary key buffers for map insertion popups, keyed by popup id.
///
/// This allows users to type a key for a new map entry across multiple frames
/// before confirming insertion, similar to ImReflect's `temp_key` storage.
static MAP_ADD_KEY_STATE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn map_add_state() -> &'static Mutex<HashMap<String, String>> {
    MAP_ADD_KEY_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    /// Per-popup temporary value buffers for map insertion popups, keyed by
    /// `(TypeId, popup_id)`. This allows users to edit the value for a new
    /// entry across multiple frames before confirming insertion.
    static MAP_ADD_VALUE_STATE: RefCell<HashMap<(TypeId, String), Box<dyn Any>>> =
        RefCell::new(HashMap::new());
}

fn with_map_add_value_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<(TypeId, String), Box<dyn Any>>) -> R,
{
    MAP_ADD_VALUE_STATE.with(|cell| {
        let mut map = cell.borrow_mut();
        f(&mut *map)
    })
}

fn with_temp_map_value<V, F>(popup_id: &str, f: F)
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

/// Public helper for rendering fixed-size arrays using explicit `ArraySettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementations for
/// `[f32; N]` / `[i32; N]`, but allows callers (such as the derive macro) to
/// supply per-member settings layered on top of global defaults.
pub fn imgui_array_with_settings<T, const N: usize>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let header_label = format!("{label} [{N}]");

    if arr_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_array_body_inner(ui, label, value, arr_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_array_body_inner(ui, label, value, arr_settings)
    }
}

fn imgui_array_body_inner<T, const N: usize>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let mut changed = false;
    let mut move_op: Option<(usize, usize)> = None;

    for index in 0..N {
        if arr_settings.reorderable {
            let handle_label = format!("==##{label}_arr_handle_{index}");
            ui.text(&handle_label);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_ARRAY_ITEM")
                // Text() items do not have an ID, so allow a null ID here to
                // avoid Dear ImGui assertions when starting a drag from this
                // label.
                .flags(imgui::DragDropFlags::SOURCE_ALLOW_NULL_ID)
                .begin_payload(index as i32)
            {
                ui.text(&handle_label);
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        changed |= T::imgui_value(ui, &elem_label, &mut value[index]);

        if arr_settings.reorderable {
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target.accept_payload::<i32, _>(
                    "IMGUI_REFLECT_ARRAY_ITEM",
                    imgui::DragDropFlags::NONE,
                ) {
                    if payload.delivery {
                        let from = payload.data as usize;
                        let to = index;
                        move_op = Some((from, to));
                    }
                }
                target.pop();
            }
        }
    }

    if let Some((from, to)) = move_op {
        if from < N && to < N && from != to {
            value.swap(from, to);
            response::record_event(response::ReflectEvent::ArrayReordered {
                path: response::current_field_path(),
                from,
                to,
            });
            changed = true;
        }
    }

    changed
}

/// Public helper for rendering `HashMap<String, V, S>` using explicit `MapSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// allows callers (such as the derive macro) to supply per-member map settings.
pub fn imgui_hash_map_with_settings<V, S>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    let mut changed = false;
    let header_label = format!("{label} [{}]", map.len());

    if map_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            changed |= imgui_hash_map_body(ui, label, map, map_settings);
        }
    } else {
        ui.text(&header_label);
        changed |= imgui_hash_map_body(ui, label, map, map_settings);
    }

    changed
}

/// Public helper for rendering `BTreeMap<String, V>` using explicit `MapSettings`.
pub fn imgui_btree_map_with_settings<V>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    let mut changed = false;
    let header_label = format!("{label} [{}]", map.len());

    if map_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            changed |= imgui_btree_map_body(ui, label, map, map_settings);
        }
    } else {
        ui.text(&header_label);
        changed |= imgui_btree_map_body(ui, label, map, map_settings);
    }

    changed
}

fn imgui_hash_map_body<V, S>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    let mut changed = false;
    let mut key_to_remove: Option<String> = None;
    let mut clear_all = false;
    let mut rename_ops: Vec<(String, String)> = Vec::new();

    // Popup id used for the "add new entry" dialog.
    let popup_id = format!("add_map_item_popup##{label}");

    // "+" button to open a popup where the user can type a key for the new entry.
    // When insertion is disabled via MapSettings, we still render a disabled button
    // with a tooltip to mirror ImReflect's behavior.
    ui.same_line();
    let add_label = format!("+##{label}_add");
    if map_settings.insertable {
        if ui.small_button(&add_label) {
            let mut state = map_add_state().lock().unwrap();
            let key_buf = state.entry(popup_id.clone()).or_insert_with(String::new);
            if key_buf.is_empty() {
                let mut idx = map.len();
                loop {
                    let candidate = format!("{label}_{}", idx);
                    if !map.contains_key(&candidate) {
                        *key_buf = candidate;
                        break;
                    }
                    idx += 1;
                }
            }
            ui.open_popup(&popup_id);
        }
    } else {
        let _disabled = ui.begin_disabled();
        let _ = ui.small_button(&add_label);
        drop(_disabled);
        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
            ui.set_item_tooltip("Insertion disabled in MapSettings");
        }
    }

    // Add-entry popup: let the user confirm insertion with a custom key and
    // a pre-edited value (optionally copied from an existing entry).
    if let Some(_popup) = ui.begin_popup(&popup_id) {
        let mut key_state = map_add_state().lock().unwrap();
        let key_buf = key_state
            .entry(popup_id.clone())
            .or_insert_with(String::new);

        ui.text("Add map entry");

        let key_label = format!("Key##{label}_new_key");
        let _ = String::imgui_value(ui, &key_label, key_buf);

        let value_label = format!("Value##{label}_new_value");
        with_temp_map_value::<V, _>(&popup_id, |temp_value| {
            changed |= V::imgui_value(ui, &value_label, temp_value);
        });

        if !map.is_empty() {
            ui.separator();
            ui.text("Copy value from existing entry:");
            let mut idx = 0usize;
            for (existing_key, existing_value) in map.iter() {
                let copy_label = format!("Copy from \"{existing_key}\"##{label}_copy_{idx}");
                if ui.small_button(&copy_label) {
                    with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                        *temp_value = existing_value.clone();
                    });
                    changed = true;
                }
                idx += 1;
            }
        }

        if ui.button("Add") {
            if !key_buf.is_empty() && !map.contains_key(key_buf) {
                with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                    let value = std::mem::take(temp_value);
                    map.insert(key_buf.clone(), value);
                });
                response::record_event(response::ReflectEvent::MapInserted {
                    path: response::current_field_path(),
                    key: key_buf.clone(),
                });
                key_buf.clear();
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                changed = true;
                ui.close_current_popup();
            }
        }

        ui.same_line();

        if ui.button("Cancel") {
            key_buf.clear();
            with_map_add_value_state(|values| {
                values.remove(&(TypeId::of::<V>(), popup_id.clone()));
            });
            ui.close_current_popup();
        }
    }
    let mut index = 0usize;

    if map_settings.use_table {
        let columns = map_settings.columns.max(3);
        let table_id = format!("##map_table_{label}");

        if let Some(_table) = ui.begin_table(&table_id, columns) {
            for (key, value) in map.iter_mut() {
                ui.table_next_row();

                // Column 0: handle.
                ui.table_next_column();
                let popup_id = format!("map_item_context_{index}##{label}");
                ui.text("==");
                if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                    ui.open_popup(&popup_id);
                }

                // Column 1: key.
                ui.table_next_column();
                let mut key_buf = key.clone();
                let key_label = format!("##{label}_key_{index}");
                changed |= String::imgui_value(ui, &key_label, &mut key_buf);
                if key_buf != *key && !key_buf.is_empty() {
                    rename_ops.push((key.clone(), key_buf));
                }

                // Column 2: value.
                ui.table_next_column();
                let value_label = format!("##{label}_value_{index}");
                changed |= V::imgui_value(ui, &value_label, value);

                ui.popup(&popup_id, || {
                    if map_settings.removable {
                        if ui.menu_item("Remove item") {
                            key_to_remove = Some(key.clone());
                        }
                        if ui.menu_item("Clear all") {
                            clear_all = true;
                        }
                    } else {
                        let _disabled = ui.begin_disabled();
                        ui.menu_item("Remove item");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        ui.menu_item("Clear all");
                        if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        drop(_disabled);
                    }
                });

                index += 1;
            }
        }
    } else {
        for (key, value) in map.iter_mut() {
            let popup_id = format!("map_item_context_{index}##{label}");
            ui.text("==");
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                ui.open_popup(&popup_id);
            }
            ui.same_line();

            let mut key_buf = key.clone();
            let key_label = format!("##{label}_key_{index}");
            changed |= String::imgui_value(ui, &key_label, &mut key_buf);

            if key_buf != *key && !key_buf.is_empty() {
                rename_ops.push((key.clone(), key_buf));
            }

            ui.same_line();

            let value_label = format!("##{label}_value_{index}");
            changed |= V::imgui_value(ui, &value_label, value);

            ui.popup(&popup_id, || {
                if map_settings.removable {
                    if ui.menu_item("Remove item") {
                        key_to_remove = Some(key.clone());
                    }
                    if ui.menu_item("Clear all") {
                        clear_all = true;
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Remove item");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    ui.menu_item("Clear all");
                    if ui.is_item_hovered_with_flags(imgui::HoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    drop(_disabled);
                }
            });

            index += 1;
        }
    }

    if clear_all {
        let previous_len = map.len();
        map.clear();
        response::record_event(response::ReflectEvent::MapCleared {
            path: response::current_field_path(),
            previous_len,
        });
        changed = true;
    } else {
        if let Some(k) = key_to_remove {
            map.remove(&k);
            response::record_event(response::ReflectEvent::MapRemoved {
                path: response::current_field_path(),
                key: k,
            });
            changed = true;
        }

        // Apply any key renames that do not collide with existing entries.
        for (old_key, new_key) in rename_ops {
            if old_key == new_key {
                continue;
            }
            if map.contains_key(&new_key) {
                continue;
            }
            if let Some(value) = map.remove(&old_key) {
                map.insert(new_key.clone(), value);
                response::record_event(response::ReflectEvent::MapRenamed {
                    path: response::current_field_path(),
                    from: old_key,
                    to: new_key,
                });
                changed = true;
            }
        }
    }

    changed
}

fn imgui_btree_map_body<V>(
    ui: &imgui::Ui,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    // Reuse HashMap body implementation by temporarily copying into a HashMap
    // to preserve ordering and semantics. This keeps the UI behavior consistent
    // between HashMap and BTreeMap editors.
    let mut temp: HashMap<String, V> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let changed = imgui_hash_map_body(ui, label, &mut temp, map_settings);

    if changed {
        map.clear();
        for (k, v) in temp {
            map.insert(k, v);
        }
    }

    changed
}

/// Public helper for rendering `Vec<T>` using explicit `VecSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// allows callers (such as the derive macro) to supply per-member vector
/// settings layered on top of global defaults.
pub fn imgui_vec_with_settings<T>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    // Show element count in the header label.
    let header_label = format!("{label} [{}]", value.len());

    if vec_settings.dropdown {
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_vec_body(ui, label, value, vec_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_vec_body(ui, label, value, vec_settings)
    }
}

fn imgui_vec_body<T>(
    ui: &imgui::Ui,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    let mut changed = false;

    // Inline "+" / "-" controls for inserting/removing elements.
    if vec_settings.insertable {
        ui.same_line();
        if ui.small_button("+") {
            let new_index = value.len();
            value.push(T::default());
            response::record_event(response::ReflectEvent::VecInserted {
                path: response::current_field_path(),
                index: new_index,
            });
            changed = true;
        }
    }

    if vec_settings.removable && !value.is_empty() {
        ui.same_line();
        if ui.small_button("-") {
            let removed_index = value.len().saturating_sub(1);
            value.pop();
            response::record_event(response::ReflectEvent::VecRemoved {
                path: response::current_field_path(),
                index: removed_index,
            });
            changed = true;
        }
    }

    // Optional drag-and-drop reordering state captured for this frame.
    let mut move_op: Option<(usize, usize)> = None;

    // Render each element as "label[index]" with an optional drag handle.
    for index in 0..value.len() {
        if vec_settings.reorderable {
            let handle_label = format!("==##{label}_handle_{index}");
            ui.text(&handle_label);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_VEC_ITEM")
                // Text() items do not have an ID, so we must allow a null ID
                // here to avoid Dear ImGui's internal assertion when starting
                // a drag from this label.
                .flags(imgui::DragDropFlags::SOURCE_ALLOW_NULL_ID)
                .begin_payload(index as i32)
            {
                ui.text(&handle_label);
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        changed |= T::imgui_value(ui, &elem_label, &mut value[index]);

        if vec_settings.reorderable {
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target
                    .accept_payload::<i32, _>("IMGUI_REFLECT_VEC_ITEM", imgui::DragDropFlags::NONE)
                {
                    if payload.delivery {
                        let from = payload.data as usize;
                        let to = index;
                        move_op = Some((from, to));
                    }
                }
                target.pop();
            }
        }
    }

    if let Some((from, to)) = move_op {
        let len = value.len();
        if from < len && to < len && from != to {
            let item = value.remove(from);
            let insert_index = if from < to { to.saturating_sub(1) } else { to };
            value.insert(insert_index, item);
            response::record_event(response::ReflectEvent::VecReordered {
                path: response::current_field_path(),
                from,
                to: insert_index,
            });
            changed = true;
        }
    }

    changed
}
