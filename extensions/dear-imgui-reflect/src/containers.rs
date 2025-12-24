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

fn escape_field_path_key(key: &str) -> String {
    // Minimal escaping so a key can be embedded inside `["..."]`.
    key.replace('\\', "\\\\").replace('"', "\\\"")
}

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
        f(&mut map)
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

    for (index, elem) in value.iter_mut().enumerate() {
        if arr_settings.reorderable {
            let handle_label = format!("==##{label}_arr_handle_{index}");
            let _ = ui.small_button(&handle_label);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_ARRAY_ITEM")
                .begin_payload(index as i32)
            {
                ui.text("==");
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        let local_changed = if response::is_field_path_active() {
            let segment = format!("[{index}]");
            response::with_field_path(&segment, || T::imgui_value(ui, &elem_label, elem))
        } else {
            T::imgui_value(ui, &elem_label, elem)
        };
        changed |= local_changed;

        if arr_settings.reorderable
            && let Some(target) = ui.drag_drop_target()
        {
            if let Some(Ok(payload)) = target
                .accept_payload::<i32, _>("IMGUI_REFLECT_ARRAY_ITEM", imgui::DragDropFlags::NONE)
                && payload.delivery
            {
                let from = payload.data as usize;
                let to = index;
                move_op = Some((from, to));
            }
            target.pop();
        }
    }

    if let Some((from, to)) = move_op
        && from < N
        && to < N
        && from != to
    {
        value.swap(from, to);
        response::record_event(response::ReflectEvent::ArrayReordered {
            path: response::current_field_path(),
            from,
            to,
        });
        changed = true;
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
            let mut state = map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let key_buf = state.entry(popup_id.clone()).or_default();
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
        let mut should_clear_popup_state = false;

        {
            let mut key_state = map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let key_buf = key_state.entry(popup_id.clone()).or_default();

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
                for (idx, (existing_key, existing_value)) in map.iter().enumerate() {
                    let copy_label = format!("Copy from \"{existing_key}\"##{label}_copy_{idx}");
                    if ui.small_button(&copy_label) {
                        with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                            *temp_value = existing_value.clone();
                        });
                        changed = true;
                    }
                }
            }

            if ui.button("Add") && !key_buf.is_empty() && !map.contains_key(key_buf) {
                let inserted_key = key_buf.clone();
                with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                    let value = std::mem::take(temp_value);
                    map.insert(inserted_key.clone(), value);
                });
                response::record_event(response::ReflectEvent::MapInserted {
                    path: response::current_field_path(),
                    key: inserted_key,
                });
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                changed = true;
                should_clear_popup_state = true;
                ui.close_current_popup();
            }

            ui.same_line();

            if ui.button("Cancel") {
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                should_clear_popup_state = true;
                ui.close_current_popup();
            }
        }

        if should_clear_popup_state {
            map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .remove(&popup_id);
        }
    }
    // If the popup was closed externally (e.g. click outside), clear any
    // per-popup state to avoid unbounded growth across dynamically generated ids.
    if !ui.is_popup_open(&popup_id) {
        map_add_state()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&popup_id);
        with_map_add_value_state(|values| {
            values.remove(&(TypeId::of::<V>(), popup_id.clone()));
        });
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
                let local_changed = if response::is_field_path_active() {
                    let escaped = escape_field_path_key(key);
                    let segment = format!("[\"{escaped}\"]");
                    response::with_field_path(&segment, || V::imgui_value(ui, &value_label, value))
                } else {
                    V::imgui_value(ui, &value_label, value)
                };
                changed |= local_changed;

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
            let local_changed = if response::is_field_path_active() {
                let escaped = escape_field_path_key(key);
                let segment = format!("[\"{escaped}\"]");
                response::with_field_path(&segment, || V::imgui_value(ui, &value_label, value))
            } else {
                V::imgui_value(ui, &value_label, value)
            };
            changed |= local_changed;

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
    let mut changed = false;
    let mut key_to_remove: Option<String> = None;
    let mut clear_all = false;
    let mut rename_ops: Vec<(String, String)> = Vec::new();

    let popup_id = format!("add_map_item_popup##{label}");

    ui.same_line();
    let add_label = format!("+##{label}_add");
    if map_settings.insertable {
        if ui.small_button(&add_label) {
            let mut state = map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let key_buf = state.entry(popup_id.clone()).or_default();
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

    if let Some(_popup) = ui.begin_popup(&popup_id) {
        let mut should_clear_popup_state = false;

        {
            let mut key_state = map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let key_buf = key_state.entry(popup_id.clone()).or_default();

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
                for (idx, (existing_key, existing_value)) in map.iter().enumerate() {
                    let copy_label = format!("Copy from \"{existing_key}\"##{label}_copy_{idx}");
                    if ui.small_button(&copy_label) {
                        with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                            *temp_value = existing_value.clone();
                        });
                        changed = true;
                    }
                }
            }

            if ui.button("Add") && !key_buf.is_empty() && !map.contains_key(key_buf) {
                let inserted_key = key_buf.clone();
                with_temp_map_value::<V, _>(&popup_id, |temp_value| {
                    let value = std::mem::take(temp_value);
                    map.insert(inserted_key.clone(), value);
                });
                response::record_event(response::ReflectEvent::MapInserted {
                    path: response::current_field_path(),
                    key: inserted_key,
                });
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                changed = true;
                should_clear_popup_state = true;
                ui.close_current_popup();
            }

            ui.same_line();

            if ui.button("Cancel") {
                with_map_add_value_state(|values| {
                    values.remove(&(TypeId::of::<V>(), popup_id.clone()));
                });
                should_clear_popup_state = true;
                ui.close_current_popup();
            }
        }

        if should_clear_popup_state {
            map_add_state()
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .remove(&popup_id);
        }
    }
    if !ui.is_popup_open(&popup_id) {
        map_add_state()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&popup_id);
        with_map_add_value_state(|values| {
            values.remove(&(TypeId::of::<V>(), popup_id.clone()));
        });
    }

    let keys: Vec<String> = map.keys().cloned().collect();

    if map_settings.use_table {
        let columns = map_settings.columns.max(3);
        let table_id = format!("##map_table_{label}");

        if let Some(_table) = ui.begin_table(&table_id, columns) {
            for (index, key) in keys.iter().enumerate() {
                let Some(value) = map.get_mut(key) else {
                    continue;
                };

                ui.table_next_row();

                ui.table_next_column();
                let popup_id = format!("map_item_context_{index}##{label}");
                ui.text("==");
                if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                    ui.open_popup(&popup_id);
                }

                ui.table_next_column();
                let mut key_buf = key.clone();
                let key_label = format!("##{label}_key_{index}");
                changed |= String::imgui_value(ui, &key_label, &mut key_buf);
                if key_buf != *key && !key_buf.is_empty() {
                    rename_ops.push((key.clone(), key_buf));
                }

                ui.table_next_column();
                let value_label = format!("##{label}_value_{index}");
                let local_changed = if response::is_field_path_active() {
                    let escaped = escape_field_path_key(key);
                    let segment = format!("[\"{escaped}\"]");
                    response::with_field_path(&segment, || V::imgui_value(ui, &value_label, value))
                } else {
                    V::imgui_value(ui, &value_label, value)
                };
                changed |= local_changed;

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
            }
        }
    } else {
        for (index, key) in keys.iter().enumerate() {
            let Some(value) = map.get_mut(key) else {
                continue;
            };

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
            let local_changed = if response::is_field_path_active() {
                let escaped = escape_field_path_key(key);
                let segment = format!("[\"{escaped}\"]");
                response::with_field_path(&segment, || V::imgui_value(ui, &value_label, value))
            } else {
                V::imgui_value(ui, &value_label, value)
            };
            changed |= local_changed;

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
    enum VecOp {
        Insert { index: usize },
        Remove { index: usize },
        Move { from: usize, to: usize },
        Clear,
    }

    let mut changed = false;
    let mut pending_op: Option<VecOp> = None;

    // Inline "+" / "-" controls for inserting/removing elements.
    if vec_settings.insertable {
        ui.same_line();
        let add_label = format!("+##{label}_vec_add");
        if ui.small_button(&add_label) {
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
        let remove_label = format!("-##{label}_vec_remove");
        if ui.small_button(&remove_label) {
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
        let popup_id = format!("vec_item_context_{index}##{label}");
        let mut open_context_menu = false;

        if vec_settings.reorderable {
            let handle_label = format!("==##{label}_handle_{index}");
            let _ = ui.small_button(&handle_label);
            open_context_menu |= ui.is_item_clicked_with_button(imgui::MouseButton::Right);

            if let Some(_source) = ui
                .drag_drop_source_config("IMGUI_REFLECT_VEC_ITEM")
                .begin_payload(index as i32)
            {
                ui.text("==");
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        let (elem_changed, elem_right_clicked) = if response::is_field_path_active() {
            let segment = format!("[{index}]");
            response::with_field_path(&segment, || {
                let local_changed = T::imgui_value(ui, &elem_label, &mut value[index]);
                let right_clicked = ui.is_item_clicked_with_button(imgui::MouseButton::Right);
                (local_changed, right_clicked)
            })
        } else {
            let local_changed = T::imgui_value(ui, &elem_label, &mut value[index]);
            let right_clicked = ui.is_item_clicked_with_button(imgui::MouseButton::Right);
            (local_changed, right_clicked)
        };
        changed |= elem_changed;
        open_context_menu |= elem_right_clicked;

        if open_context_menu {
            ui.open_popup(&popup_id);
        }

        ui.popup(&popup_id, || {
            let can_insert = vec_settings.insertable;
            let can_remove = vec_settings.removable;
            let can_reorder = vec_settings.reorderable;

            if can_insert {
                if ui.menu_item("Insert before") && pending_op.is_none() {
                    pending_op = Some(VecOp::Insert { index });
                }
                if ui.menu_item("Insert after") && pending_op.is_none() {
                    pending_op = Some(VecOp::Insert { index: index + 1 });
                }
            } else {
                let _disabled = ui.begin_disabled();
                ui.menu_item("Insert before");
                ui.menu_item("Insert after");
                drop(_disabled);
            }

            if can_remove {
                if ui.menu_item("Remove item") && pending_op.is_none() {
                    pending_op = Some(VecOp::Remove { index });
                }
                if ui.menu_item("Clear all") && pending_op.is_none() {
                    pending_op = Some(VecOp::Clear);
                }
            } else {
                let _disabled = ui.begin_disabled();
                ui.menu_item("Remove item");
                ui.menu_item("Clear all");
                drop(_disabled);
            }

            if can_reorder {
                if index > 0 {
                    if ui.menu_item("Move up") && pending_op.is_none() {
                        pending_op = Some(VecOp::Move {
                            from: index,
                            to: index.saturating_sub(1),
                        });
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Move up");
                    drop(_disabled);
                }

                if index + 1 < value.len() {
                    if ui.menu_item("Move down") && pending_op.is_none() {
                        pending_op = Some(VecOp::Move {
                            from: index,
                            to: index + 1,
                        });
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Move down");
                    drop(_disabled);
                }
            } else {
                let _disabled = ui.begin_disabled();
                ui.menu_item("Move up");
                ui.menu_item("Move down");
                drop(_disabled);
            }
        });

        if vec_settings.reorderable
            && let Some(target) = ui.drag_drop_target()
        {
            if let Some(Ok(payload)) = target
                .accept_payload::<i32, _>("IMGUI_REFLECT_VEC_ITEM", imgui::DragDropFlags::NONE)
                && payload.delivery
            {
                let from = payload.data as usize;
                let to = index;
                move_op = Some((from, to));
            }
            target.pop();
        }
    }

    if let Some(op) = pending_op {
        match op {
            VecOp::Insert { index } => {
                let index = index.min(value.len());
                value.insert(index, T::default());
                response::record_event(response::ReflectEvent::VecInserted {
                    path: response::current_field_path(),
                    index,
                });
                changed = true;
            }
            VecOp::Remove { index } => {
                if index < value.len() {
                    value.remove(index);
                    response::record_event(response::ReflectEvent::VecRemoved {
                        path: response::current_field_path(),
                        index,
                    });
                    changed = true;
                }
            }
            VecOp::Move { from, to } => {
                let len = value.len();
                if from < len && to < len && from != to {
                    let item = value.remove(from);
                    value.insert(to, item);
                    response::record_event(response::ReflectEvent::VecReordered {
                        path: response::current_field_path(),
                        from,
                        to,
                    });
                    changed = true;
                }
            }
            VecOp::Clear => {
                if !value.is_empty() {
                    let previous_len = value.len();
                    value.clear();
                    response::record_event(response::ReflectEvent::VecCleared {
                        path: response::current_field_path(),
                        previous_len,
                    });
                    changed = true;
                }
            }
        }
    } else if let Some((from, to)) = move_op {
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Mutex, MutexGuard, OnceLock};

    use crate::response;
    use crate::{ImGuiValue, ReflectEvent, ReflectResponse};

    fn test_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn new_test_ctx() -> imgui::Context {
        let mut ctx = imgui::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
        ctx
    }

    #[derive(Clone, Debug, Default)]
    struct Probe {
        id: usize,
    }

    impl ImGuiValue for Probe {
        fn imgui_value(ui: &imgui::Ui, label: &str, value: &mut Self) -> bool {
            let _ = (ui, label);
            let id = value.id;
            response::record_event(ReflectEvent::VecInserted {
                path: response::current_field_path(),
                index: id,
            });
            false
        }
    }

    #[test]
    fn nested_vec_element_paths_include_index_segments() {
        let _guard = test_guard();
        let mut ctx = new_test_ctx();

        let mut values = vec![Probe { id: 0 }, Probe { id: 1 }];
        let vec_settings = VecSettings {
            insertable: false,
            removable: false,
            reorderable: false,
            dropdown: false,
        };

        let mut resp = ReflectResponse::default();
        {
            let ui = ctx.frame();
            response::with_response(&mut resp, || {
                response::with_field_path("items", || {
                    let _ = imgui_vec_with_settings(ui, "items", &mut values, &vec_settings);
                });
            });
        }
        ctx.render();

        let paths: Vec<Option<String>> = resp
            .events()
            .iter()
            .filter_map(|event| match event {
                ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].as_deref(), Some("items[0]"));
        assert_eq!(paths[1].as_deref(), Some("items[1]"));
    }

    #[test]
    fn nested_array_element_paths_include_index_segments() {
        let _guard = test_guard();
        let mut ctx = new_test_ctx();

        let mut values = [Probe { id: 0 }, Probe { id: 1 }];
        let arr_settings = ArraySettings {
            dropdown: false,
            reorderable: false,
        };

        let mut resp = ReflectResponse::default();
        {
            let ui = ctx.frame();
            response::with_response(&mut resp, || {
                response::with_field_path("arr", || {
                    let _ = imgui_array_with_settings(ui, "arr", &mut values, &arr_settings);
                });
            });
        }
        ctx.render();

        let paths: Vec<Option<String>> = resp
            .events()
            .iter()
            .filter_map(|event| match event {
                ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].as_deref(), Some("arr[0]"));
        assert_eq!(paths[1].as_deref(), Some("arr[1]"));
    }

    #[test]
    fn nested_map_value_paths_include_key_segments() {
        let _guard = test_guard();
        let mut ctx = new_test_ctx();

        let mut map = BTreeMap::from([
            ("a".to_owned(), Probe { id: 0 }),
            ("b\"c".to_owned(), Probe { id: 1 }),
        ]);
        let map_settings = MapSettings {
            dropdown: false,
            insertable: false,
            removable: false,
            use_table: false,
            columns: 3,
        };

        let mut resp = ReflectResponse::default();
        {
            let ui = ctx.frame();
            response::with_response(&mut resp, || {
                response::with_field_path("map", || {
                    let _ = imgui_btree_map_with_settings(ui, "map", &mut map, &map_settings);
                });
            });
        }
        ctx.render();

        let paths: Vec<Option<String>> = resp
            .events()
            .iter()
            .filter_map(|event| match event {
                ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].as_deref(), Some("map[\"a\"]"));
        assert_eq!(paths[1].as_deref(), Some("map[\"b\\\"c\"]"));
    }

    #[test]
    fn nested_tuple_element_paths_include_index_segments() {
        let _guard = test_guard();
        let mut ctx = new_test_ctx();

        let mut tuple = (Probe { id: 0 }, Probe { id: 1 });
        let mut resp = ReflectResponse::default();
        {
            let ui = ctx.frame();
            response::with_response(&mut resp, || {
                response::with_field_path("tup", || {
                    let _ = <(Probe, Probe) as ImGuiValue>::imgui_value(ui, "tup", &mut tuple);
                });
            });
        }
        ctx.render();

        let paths: Vec<Option<String>> = resp
            .events()
            .iter()
            .filter_map(|event| match event {
                ReflectEvent::VecInserted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].as_deref(), Some("tup[0]"));
        assert_eq!(paths[1].as_deref(), Some("tup[1]"));
    }
}
