use super::*;
use crate::MapSettings;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::hash::BuildHasher;

/// Public helper for rendering `HashMap<String, V, S>` using explicit `MapSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// lets callers supply per-member map settings.
pub fn imgui_hash_map_with_settings<V, S>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    draw_map(inspector, label, map.len(), map_settings, |inspector| {
        imgui_hash_map_body(inspector, label, map, map_settings)
    })
}

/// Public helper for rendering `BTreeMap<String, V>` using explicit `MapSettings`.
pub fn imgui_btree_map_with_settings<V>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    draw_map(inspector, label, map.len(), map_settings, |inspector| {
        imgui_btree_map_body(inspector, label, map, map_settings)
    })
}

fn draw_map(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    len: usize,
    map_settings: &MapSettings,
    draw_body: impl FnOnce(&mut Inspector<'_, '_>) -> bool,
) -> bool {
    let ui = inspector.ui();
    let _container_id = ui.push_id(label);
    let mut header_label = container_header(label, len);
    if map_settings.dropdown {
        header_label.push_str("###reflect_map");
        if let Some(_node) = ui.tree_node(&header_label) {
            draw_body(inspector)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        draw_body(inspector)
    }
}

fn imgui_hash_map_body<V, S>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    map: &mut HashMap<String, V, S>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    let ui = inspector.ui();
    let mut changed = false;

    // Popup id used for the "add new entry" dialog.
    let popup_id = "add_map_item_popup";
    // Capture the owner scope before BeginPopup switches to the popup window's
    // ID stack. Every operation for this popup must use the same identity.
    let draft_identity = inspector.map_draft_identity::<V>(popup_id);

    // "+" button to open a popup where the user can type a key for the new entry.
    // When insertion is disabled via MapSettings, we still render a disabled button
    // with a tooltip to mirror ImReflect's behavior.
    ui.same_line();
    let add_label = "+##map_add";
    if map_settings.insertable {
        if ui.small_button(add_label) {
            inspector.with_map_draft::<V, _>(
                draft_identity,
                || {
                    let mut idx = map.len();
                    loop {
                        let candidate = format!("{label}_{}", idx);
                        if !map.contains_key(&candidate) {
                            break candidate;
                        }
                        idx += 1;
                    }
                },
                |_, _, _| {},
            );
            ui.open_popup(popup_id);
        }
    } else {
        let _disabled = ui.begin_disabled();
        let _ = ui.small_button(add_label);
        drop(_disabled);
        if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
            ui.set_item_tooltip("Insertion disabled in MapSettings");
        }
    }

    if let Some(_popup) = ui.begin_popup(popup_id) {
        let initial_key = {
            let mut idx = map.len();
            loop {
                let candidate = format!("{label}_{}", idx);
                if !map.contains_key(&candidate) {
                    break candidate;
                }
                idx += 1;
            }
        };
        let should_clear_popup_state = inspector.with_map_draft::<V, _>(
            draft_identity,
            || initial_key,
            |inspector, key_buf, temp_value| {
                ui.text("Add map entry");

                let _ = String::imgui_value(inspector, "Key##new_key", key_buf);

                changed |= V::imgui_value(inspector, "Value##new_value", temp_value);

                if !map.is_empty() {
                    ui.separator();
                    ui.text("Copy value from existing entry:");
                    for (existing_key, existing_value) in map.iter() {
                        let id = map_key_id(existing_key);
                        let _entry_id = ui.push_id(id.as_ref());
                        let copy_label = format!("Copy from \"{existing_key}\"###copy_value");
                        if ui.small_button(&copy_label) {
                            *temp_value = existing_value.clone();
                            changed = true;
                        }
                    }
                }

                if ui.button("Add") && !key_buf.is_empty() && !map.contains_key(key_buf) {
                    let inserted_key = key_buf.clone();
                    map.insert(inserted_key.clone(), std::mem::take(temp_value));
                    inspector.record_event(crate::ReflectEvent::MapInserted {
                        path: inspector.current_path(),
                        key: inserted_key,
                    });
                    changed = true;
                    ui.close_current_popup();
                    true
                } else {
                    ui.same_line();
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                        true
                    } else {
                        false
                    }
                }
            },
        );

        if should_clear_popup_state {
            inspector.clear_map_draft(draft_identity);
        }
    }
    // If the popup was closed externally (e.g. click outside), clear any
    // per-popup state to avoid unbounded growth across dynamically generated ids.
    if !ui.is_popup_open(popup_id) {
        inspector.clear_map_draft(draft_identity);
    }
    let (rows_changed, edits) = draw_map_rows(inspector, map.iter_mut(), map_settings);
    changed |= rows_changed;

    if edits.clear_all {
        let previous_len = map.len();
        map.clear();
        inspector.record_event(crate::ReflectEvent::MapCleared {
            path: inspector.current_path(),
            previous_len,
        });
        changed = true;
    } else {
        if let Some(k) = edits.key_to_remove {
            map.remove(&k);
            inspector.record_event(crate::ReflectEvent::MapRemoved {
                path: inspector.current_path(),
                key: k,
            });
            changed = true;
        }

        for (old_key, new_key) in edits.renames {
            if map.contains_key(&new_key) {
                continue;
            }
            if let Some(value) = map.remove(&old_key) {
                map.insert(new_key.clone(), value);
                inspector.record_event(crate::ReflectEvent::MapRenamed {
                    path: inspector.current_path(),
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
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    map: &mut BTreeMap<String, V>,
    map_settings: &MapSettings,
) -> bool
where
    V: ImGuiValue + Default + Clone + 'static,
{
    let ui = inspector.ui();
    let mut changed = false;

    let popup_id = "add_map_item_popup";
    // Capture the owner scope before BeginPopup switches to the popup window's
    // ID stack. Every operation for this popup must use the same identity.
    let draft_identity = inspector.map_draft_identity::<V>(popup_id);

    ui.same_line();
    let add_label = "+##map_add";
    if map_settings.insertable {
        if ui.small_button(add_label) {
            inspector.with_map_draft::<V, _>(
                draft_identity,
                || {
                    let mut idx = map.len();
                    loop {
                        let candidate = format!("{label}_{}", idx);
                        if !map.contains_key(&candidate) {
                            break candidate;
                        }
                        idx += 1;
                    }
                },
                |_, _, _| {},
            );
            ui.open_popup(popup_id);
        }
    } else {
        let _disabled = ui.begin_disabled();
        let _ = ui.small_button(add_label);
        drop(_disabled);
        if ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED) {
            ui.set_item_tooltip("Insertion disabled in MapSettings");
        }
    }

    if let Some(_popup) = ui.begin_popup(popup_id) {
        let initial_key = {
            let mut idx = map.len();
            loop {
                let candidate = format!("{label}_{}", idx);
                if !map.contains_key(&candidate) {
                    break candidate;
                }
                idx += 1;
            }
        };
        let should_clear_popup_state = inspector.with_map_draft::<V, _>(
            draft_identity,
            || initial_key,
            |inspector, key_buf, temp_value| {
                ui.text("Add map entry");

                let _ = String::imgui_value(inspector, "Key##new_key", key_buf);

                changed |= V::imgui_value(inspector, "Value##new_value", temp_value);

                if !map.is_empty() {
                    ui.separator();
                    ui.text("Copy value from existing entry:");
                    for (existing_key, existing_value) in map.iter() {
                        let id = map_key_id(existing_key);
                        let _entry_id = ui.push_id(id.as_ref());
                        let copy_label = format!("Copy from \"{existing_key}\"###copy_value");
                        if ui.small_button(&copy_label) {
                            *temp_value = existing_value.clone();
                            changed = true;
                        }
                    }
                }

                if ui.button("Add") && !key_buf.is_empty() && !map.contains_key(key_buf) {
                    let inserted_key = key_buf.clone();
                    map.insert(inserted_key.clone(), std::mem::take(temp_value));
                    inspector.record_event(crate::ReflectEvent::MapInserted {
                        path: inspector.current_path(),
                        key: inserted_key,
                    });
                    changed = true;
                    ui.close_current_popup();
                    true
                } else {
                    ui.same_line();
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                        true
                    } else {
                        false
                    }
                }
            },
        );

        if should_clear_popup_state {
            inspector.clear_map_draft(draft_identity);
        }
    }
    if !ui.is_popup_open(popup_id) {
        inspector.clear_map_draft(draft_identity);
    }
    let (rows_changed, edits) = draw_map_rows(inspector, map.iter_mut(), map_settings);
    changed |= rows_changed;

    if edits.clear_all {
        let previous_len = map.len();
        map.clear();
        inspector.record_event(crate::ReflectEvent::MapCleared {
            path: inspector.current_path(),
            previous_len,
        });
        changed = true;
    } else {
        if let Some(k) = edits.key_to_remove {
            map.remove(&k);
            inspector.record_event(crate::ReflectEvent::MapRemoved {
                path: inspector.current_path(),
                key: k,
            });
            changed = true;
        }

        for (old_key, new_key) in edits.renames {
            if map.contains_key(&new_key) {
                continue;
            }
            if let Some(value) = map.remove(&old_key) {
                map.insert(new_key.clone(), value);
                inspector.record_event(crate::ReflectEvent::MapRenamed {
                    path: inspector.current_path(),
                    from: old_key,
                    to: new_key,
                });
                changed = true;
            }
        }
    }

    changed
}

#[derive(Default)]
struct MapRowEdits {
    key_to_remove: Option<String>,
    clear_all: bool,
    renames: Vec<(String, String)>,
}

fn draw_map_rows<'a, V: ImGuiValue + 'a>(
    inspector: &mut Inspector<'_, '_>,
    entries: impl Iterator<Item = (&'a String, &'a mut V)>,
    settings: &MapSettings,
) -> (bool, MapRowEdits) {
    let ui = inspector.ui();
    let mut edits = MapRowEdits::default();
    let _table = if settings.use_table {
        let Some(table) = ui.begin_table("##map_table", settings.columns.max(3)) else {
            return (false, edits);
        };
        Some(table)
    } else {
        None
    };

    let mut changed = false;
    for (key, value) in entries {
        if settings.use_table {
            ui.table_next_row();
            ui.table_next_column();
        }
        let id = map_key_id(key);
        let _row_id = ui.push_id(id.as_ref());
        let popup_id = "map_item_context";
        ui.text("==");
        if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
            ui.open_popup(popup_id);
        }

        if settings.use_table {
            ui.table_next_column();
        } else {
            ui.same_line();
        }
        if let Some(new_key) = edit_map_key(inspector, key) {
            edits.renames.push((key.clone(), new_key));
        }

        if settings.use_table {
            ui.table_next_column();
        } else {
            ui.same_line();
        }
        {
            let _path = inspector.tracks_nested_paths().then(|| {
                let escaped = escape_field_path_key(key);
                inspector.push_path(format!("[\"{escaped}\"]"))
            });
            changed |= inspector.render_value("##value", value);
        }

        ui.popup(popup_id, || {
            let _disabled = ui.begin_disabled_with_cond(!settings.removable);
            if ui.menu_item("Remove item") {
                edits.key_to_remove = Some(key.clone());
            }
            if !settings.removable
                && ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED)
            {
                ui.set_item_tooltip("Removal disabled in MapSettings");
            }
            if ui.menu_item("Clear all") {
                edits.clear_all = true;
            }
            if !settings.removable
                && ui.is_item_hovered_with_flags(imgui::ItemHoveredFlags::ALLOW_WHEN_DISABLED)
            {
                ui.set_item_tooltip("Removal disabled in MapSettings");
            }
        });
    }

    (changed, edits)
}

fn map_key_id(key: &str) -> Cow<'_, str> {
    if key.contains(['%', '#', '\0']) {
        Cow::Owned(
            key.replace('%', "%25")
                .replace('#', "%23")
                .replace('\0', "%00"),
        )
    } else {
        Cow::Borrowed(key)
    }
}

fn edit_map_key(inspector: &Inspector<'_, '_>, key: &str) -> Option<String> {
    let ui = inspector.ui();
    let mut draft = key.to_owned();
    let submitted = ui.with_item_flag(imgui::ItemFlags::LIVE_EDIT_ON_INPUT_TEXT, false, || {
        ui.input_text("##key", &mut draft).build()
    });

    let active = ui.is_item_active();
    if !submitted && !active {
        return None;
    }
    if !ui.is_item_visible()
        || ui.item_flags().contains(imgui::ItemStateFlags::DISABLED)
        || ui.is_key_pressed(imgui::Key::Escape)
    {
        return None;
    }
    let was_editing = inspector.track_map_key_focus(ui.item_id(), active);
    (submitted && was_editing && !draft.is_empty() && draft != key).then_some(draft)
}
