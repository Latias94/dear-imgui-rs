use super::path_state::{
    escape_field_path_key, map_add_state, with_map_add_value_state, with_temp_map_value,
};
use super::*;
use crate::MapSettings;
use std::collections::BTreeMap;
use std::hash::BuildHasher;

/// Public helper for rendering `HashMap<String, V, S>` using explicit `MapSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// lets callers supply per-member map settings.
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

pub(super) fn imgui_hash_map_body<V, S>(
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
        if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
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
                        if ui.is_item_hovered_with_flags(
                            imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED,
                        ) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        ui.menu_item("Clear all");
                        if ui.is_item_hovered_with_flags(
                            imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED,
                        ) {
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
                    if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    ui.menu_item("Clear all");
                    if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
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

pub(super) fn imgui_btree_map_body<V>(
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
        if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
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
                        if ui.is_item_hovered_with_flags(
                            imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED,
                        ) {
                            ui.set_item_tooltip("Removal disabled in MapSettings");
                        }
                        ui.menu_item("Clear all");
                        if ui.is_item_hovered_with_flags(
                            imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED,
                        ) {
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
                    if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
                        ui.set_item_tooltip("Removal disabled in MapSettings");
                    }
                    ui.menu_item("Clear all");
                    if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
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
