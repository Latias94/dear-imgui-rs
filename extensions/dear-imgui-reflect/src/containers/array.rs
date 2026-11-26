use super::*;
use crate::ArraySettings;

/// Public helper for rendering fixed-size arrays using explicit `ArraySettings`.
///
/// This mirrors the behavior of the built-in `ImGuiValue` implementations for
/// arrays, while allowing callers to provide per-member settings.
pub fn imgui_array_with_settings<T, const N: usize>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let ui = inspector.ui();
    let _container_id = ui.push_id(label);
    let mut header_label = container_header(label, N);

    if arr_settings.dropdown {
        header_label.push_str("###reflect_array");
        if let Some(_node) = ui.tree_node(&header_label) {
            imgui_array_body_inner(inspector, label, value, arr_settings)
        } else {
            false
        }
    } else {
        ui.text(&header_label);
        imgui_array_body_inner(inspector, label, value, arr_settings)
    }
}

fn imgui_array_body_inner<T, const N: usize>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    value: &mut [T; N],
    arr_settings: &ArraySettings,
) -> bool
where
    T: ImGuiValue,
{
    let ui = inspector.ui();
    let mut changed = false;
    let mut move_op = None;
    let payload_tag = arr_settings.reorderable.then(|| {
        let container_addr = std::ptr::from_ref(value).addr();
        let view_id = ui.get_id("reflect_array_drag").raw();
        format!("Reflect{container_addr:x}:{view_id:x}")
    });

    for (index, elem) in value.iter_mut().enumerate() {
        let _row_id = ui.push_id(index);
        if let Some(tag) = &payload_tag {
            ui.small_button("==##drag");

            if let Some(_source) = ui
                .drag_drop_source_config(tag)
                .condition(imgui::DragDropPayloadCond::Once)
                .begin_payload(index)
            {
                ui.text("==");
            }

            ui.same_line();
        }

        let elem_label = format!("{label}[{index}]");
        let _path = inspector
            .tracks_nested_paths()
            .then(|| inspector.push_path(format!("[{index}]")));
        changed |= inspector.render_value(&elem_label, elem);

        if let Some(tag) = &payload_tag
            && let Some(target) = ui.drag_drop_target()
            && let Some(Ok(payload)) =
                target.accept_payload::<usize, _>(tag, imgui::DragDropTargetFlags::NONE)
            && payload.delivery
        {
            move_op = Some((payload.data, index));
        }
    }

    if let Some((from, to)) = move_op
        && from < N
        && to < N
        && from != to
    {
        value.swap(from, to);
        inspector.record_event(crate::ReflectEvent::ArrayReordered {
            path: inspector.current_path(),
            from,
            to,
        });
        changed = true;
    }

    changed
}
