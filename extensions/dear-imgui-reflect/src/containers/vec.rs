use super::*;

/// Public helper for rendering `Vec<T>` using explicit `VecSettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementation but
/// lets callers supply per-member vector settings.
pub fn imgui_vec_with_settings<T>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    let ui = inspector.ui();
    let _container_id = ui.push_id(label);
    let mut header_label = container_header(label, value.len());

    if vec_settings.dropdown {
        header_label.push_str("###reflect_vec");
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_vec_body(inspector, label, value, vec_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_vec_body(inspector, label, value, vec_settings)
    }
}

fn imgui_vec_body<T>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    value: &mut Vec<T>,
    vec_settings: &VecSettings,
) -> bool
where
    T: ImGuiValue + Default,
{
    let ui = inspector.ui();
    enum VecOp {
        Insert { index: usize },
        Remove { index: usize },
        Move { from: usize, to: usize },
        Clear,
    }

    let mut changed = false;
    let mut pending_op = None;

    // Inline "+" / "-" controls for inserting/removing elements.
    if vec_settings.insertable {
        ui.same_line();
        if ui.small_button("+##reflect_vec_add") {
            let new_index = value.len();
            value.push(T::default());
            inspector.record_event(crate::ReflectEvent::VecInserted {
                path: inspector.current_path(),
                index: new_index,
            });
            changed = true;
        }
    }

    if vec_settings.removable && !value.is_empty() {
        ui.same_line();
        if ui.small_button("-##reflect_vec_remove") {
            let removed_index = value.len().saturating_sub(1);
            value.pop();
            inspector.record_event(crate::ReflectEvent::VecRemoved {
                path: inspector.current_path(),
                index: removed_index,
            });
            changed = true;
        }
    }

    let len = value.len();
    let payload_tag = vec_settings.reorderable.then(|| {
        let container_addr = std::ptr::from_ref(value).addr();
        let view_id = ui.get_id("reflect_vec_drag").raw();
        format!("Reflect{container_addr:x}:{view_id:x}")
    });
    let mut drag_move = None;

    for (index, elem) in value.iter_mut().enumerate() {
        let row_id = index.to_string();
        let _row_id = ui.push_id(&row_id);
        let popup_id = "reflect_vec_item_context";
        let (handle_id, mut open_context_menu) = if let Some(tag) = &payload_tag {
            ui.small_button("==##drag");
            let handle_id = ui.item_id();
            let clicked = ui.is_item_clicked_with_button(imgui::MouseButton::Right);

            if let Some(_source) = ui
                .drag_drop_source_config(tag)
                .condition(imgui::DragDropPayloadCond::Once)
                .begin_payload((index, len))
            {
                ui.text("==");
            }
            capture_vec_drop(ui, tag, index, len, &mut drag_move);
            ui.same_line();
            (Some(handle_id), clicked)
        } else {
            (None, false)
        };

        let elem_label = format!("{label}[{index}]");
        {
            let _path = inspector
                .tracks_nested_paths()
                .then(|| inspector.push_path(format!("[{index}]")));
            changed |= inspector.render_value(&elem_label, elem);
        }
        if handle_id.is_none() || handle_id != Some(ui.item_id()) {
            open_context_menu |= ui.is_item_clicked_with_button(imgui::MouseButton::Right);
            if let Some(tag) = &payload_tag {
                capture_vec_drop(ui, tag, index, len, &mut drag_move);
            }
        }

        if open_context_menu {
            ui.open_popup(popup_id);
        }

        ui.popup(popup_id, || {
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
                            to: index - 1,
                        });
                    }
                } else {
                    let _disabled = ui.begin_disabled();
                    ui.menu_item("Move up");
                    drop(_disabled);
                }

                if index + 1 < len {
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
    }

    if let Some(op) = pending_op.or(drag_move.map(|(from, to)| VecOp::Move { from, to })) {
        match op {
            VecOp::Insert { index } => {
                let index = index.min(value.len());
                value.insert(index, T::default());
                inspector.record_event(crate::ReflectEvent::VecInserted {
                    path: inspector.current_path(),
                    index,
                });
                changed = true;
            }
            VecOp::Remove { index } => {
                if index < value.len() {
                    value.remove(index);
                    inspector.record_event(crate::ReflectEvent::VecRemoved {
                        path: inspector.current_path(),
                        index,
                    });
                    changed = true;
                }
            }
            VecOp::Move { from, to } => {
                let len = value.len();
                if from < len && to < len && from != to {
                    if from < to {
                        value[from..=to].rotate_left(1);
                    } else {
                        value[to..=from].rotate_right(1);
                    }
                    inspector.record_event(crate::ReflectEvent::VecReordered {
                        path: inspector.current_path(),
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
                    inspector.record_event(crate::ReflectEvent::VecCleared {
                        path: inspector.current_path(),
                        previous_len,
                    });
                    changed = true;
                }
            }
        }
    }

    changed
}

fn capture_vec_drop(
    ui: &imgui::Ui,
    tag: &str,
    index: usize,
    len: usize,
    drag_move: &mut Option<(usize, usize)>,
) {
    if let Some(target) = ui.drag_drop_target()
        && !ui.item_flags().contains(imgui::ItemStateFlags::DISABLED)
        && let Some(Ok(payload)) =
            target.accept_payload::<(usize, usize), _>(tag, imgui::DragDropTargetFlags::NONE)
        && payload.delivery
    {
        let (from, source_len) = payload.data;
        if source_len == len {
            *drag_move = Some((from, index));
        }
    }
}
