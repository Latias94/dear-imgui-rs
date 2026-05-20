use super::*;

/// Public helper for rendering `Vec<T>` using explicit `VecSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// lets callers supply per-member vector settings.
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

pub(super) fn imgui_vec_body<T>(
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
            if let Some(Ok(payload)) = target.accept_payload::<i32, _>(
                "IMGUI_REFLECT_VEC_ITEM",
                imgui::DragDropTargetFlags::NONE,
            ) && payload.delivery
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
