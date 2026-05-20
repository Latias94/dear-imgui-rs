use super::*;
use crate::ArraySettings;

/// Public helper for rendering fixed-size arrays using explicit `ArraySettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementations for
/// arrays, while allowing callers to provide per-member settings.
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

pub(super) fn imgui_array_body_inner<T, const N: usize>(
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
            if let Some(Ok(payload)) = target.accept_payload::<i32, _>(
                "IMGUI_REFLECT_ARRAY_ITEM",
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
