//! Built-in editors and shared widget layout for reflected values.

#[cfg(any(feature = "glam", feature = "mint"))]
mod math;

use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use crate::{
    BoolStyle, ImGuiValue, Inspector, MemberSettings, TupleRenderMode, TupleSettings, imgui,
    imgui_array_with_settings, imgui_btree_map_with_settings, imgui_hash_map_with_settings,
    imgui_vec_with_settings,
};

impl ImGuiValue for bool {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        Self::imgui_value_with_member(inspector, label, value, None)
    }

    fn imgui_value_with_member(
        inspector: &mut Inspector<'_, '_>,
        label: &str,
        value: &mut Self,
        member: Option<&MemberSettings>,
    ) -> bool {
        render_bool(inspector, label, value, member, label, None, [None; 2])
    }
}

#[doc(hidden)]
pub fn render_bool(
    inspector: &Inspector<'_, '_>,
    label: &str,
    value: &mut bool,
    member: Option<&MemberSettings>,
    field_id: &str,
    style: Option<BoolStyle>,
    [false_text, true_text]: [Option<&str>; 2],
) -> bool {
    let ui = inspector.ui();
    let style = style.unwrap_or_else(|| {
        member
            .and_then(|member| member.bools.as_ref())
            .unwrap_or_else(|| inspector.settings().bools())
            .style
    });
    let caption = || label.split("##").next().unwrap_or_default();
    match style {
        BoolStyle::Checkbox => ui.checkbox(label, value),
        BoolStyle::Button => {
            let _field_id = ui.push_id(field_id);
            let caption = caption();
            let state_label = if *value {
                true_text.unwrap_or("On")
            } else {
                false_text.unwrap_or("Off")
            };
            let state_label = state_label.split("##").next().unwrap_or_default();
            let id_suffix = if state_label.ends_with('#') {
                " ###bool_button"
            } else {
                "###bool_button"
            };
            let button_label = if caption.is_empty() {
                format!("{state_label}{id_suffix}")
            } else {
                format!("{caption}: {state_label}{id_suffix}")
            };
            if ui.button(&button_label) {
                *value = !*value;
                true
            } else {
                false
            }
        }
        BoolStyle::Dropdown => {
            let mut index = usize::from(*value);
            let captions = [false_text.unwrap_or("False"), true_text.unwrap_or("True")];
            let changed = ui.combo_simple_string(label, &mut index, &captions);
            if changed {
                *value = index == 1;
            }
            changed
        }
        BoolStyle::Radio => {
            let _field_id = ui.push_id(field_id);
            let caption = caption();
            if !caption.is_empty() {
                ui.text(caption);
            }
            let select_true = {
                let _choice_id = ui.push_id("true");
                ui.radio_button(true_text.unwrap_or("True"), *value)
            };
            let select_false = {
                let _choice_id = ui.push_id("false");
                ui.radio_button(false_text.unwrap_or("False"), !*value)
            };
            let next = select_true || (*value && !select_false);
            let changed = next != *value;
            *value = next;
            changed
        }
    }
}

#[doc(hidden)]
pub fn render_enum_selection(
    ui: &imgui::Ui,
    label: &str,
    labels: &[&str],
    current: usize,
    radio: bool,
) -> usize {
    let Some(&current_caption) = labels.get(current) else {
        return current;
    };
    let mut index = current;
    if radio {
        let caption = label.split("##").next().unwrap_or_default();
        if !caption.is_empty() {
            ui.text(caption);
        }
        ui.with_indent(|| {
            for (variant, &caption) in labels.iter().enumerate() {
                let _variant_id = ui.push_id(variant);
                if ui.radio_button(caption, variant == current) {
                    index = variant;
                }
            }
        });
    } else if let Some(_combo) = ui.begin_combo(label, current_caption) {
        for (variant, &caption) in labels.iter().enumerate() {
            let _variant_id = ui.push_id(variant);
            let selected = variant == current;
            if ui.selectable_config(caption).selected(selected).build() {
                index = variant;
            }
            if selected {
                ui.set_item_default_focus();
            }
        }
    }
    index
}

impl ImGuiValue for String {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        render_text(
            inspector,
            label,
            value,
            TextWidget::Input { hint: None },
            None,
        )
    }
}

impl ImGuiValue for &str {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        display_text(inspector.ui(), label, value);
        false
    }
}

#[doc(hidden)]
pub enum TextWidget<'a> {
    Input {
        hint: Option<&'a str>,
    },
    Multiline {
        lines: Option<f32>,
        auto_resize: bool,
    },
    Display,
}

#[doc(hidden)]
pub fn render_text(
    inspector: &Inspector<'_, '_>,
    label: &str,
    value: &mut String,
    widget: TextWidget<'_>,
    width: Option<f32>,
) -> bool {
    let ui = inspector.ui();
    let _width = width.map(|width| ui.push_item_width(width));
    let read_only = inspector.is_read_only();
    match widget {
        TextWidget::Input { hint } => {
            let input = ui.input_text(label, value).read_only(read_only);
            if let Some(hint) = hint {
                input.hint(hint).build()
            } else {
                input.build()
            }
        }
        TextWidget::Multiline { lines, auto_resize } => {
            let _auto_width = auto_resize.then(|| ui.push_item_width_text(value.as_str()));
            let lines = if auto_resize {
                Some((1 + value.bytes().filter(|&byte| byte == b'\n').count()) as f32)
            } else {
                lines
            };
            let height = lines.map_or(0.0, |lines| ui.text_line_height_with_spacing() * lines);
            ui.input_text_multiline(label, value, [0.0, height])
                .read_only(read_only)
                .build()
        }
        TextWidget::Display => {
            display_text(ui, label, value);
            false
        }
    }
}

fn display_text(ui: &imgui::Ui, label: &str, value: &str) {
    let caption = label.split("##").next().unwrap_or_default();
    if caption.is_empty() {
        ui.text_wrapped(value);
    } else {
        ui.text_wrapped(format!("{caption}: {value}"));
    }
    if ui.is_item_hovered() {
        ui.set_item_tooltip(value);
    }
}

macro_rules! impl_container {
    ([$($generics:tt)*] $container:ty, $settings:ident, $render:ident
        $(where $($bounds:tt)*)?) => {
        impl<$($generics)*> ImGuiValue for $container $(where $($bounds)*)? {
            fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
                Self::imgui_value_with_member(inspector, label, value, None)
            }

            fn imgui_value_with_member(
                inspector: &mut Inspector<'_, '_>,
                label: &str,
                value: &mut Self,
                member: Option<&MemberSettings>,
            ) -> bool {
                let settings = member
                    .and_then(|member| member.$settings.as_ref())
                    .unwrap_or_else(|| inspector.settings().$settings());
                $render(inspector, label, value, settings)
            }
        }
    };
}

impl_container!([T, const N: usize] [T; N], arrays, imgui_array_with_settings
    where T: ImGuiValue);
impl_container!([T] Vec<T>, vec, imgui_vec_with_settings
    where T: ImGuiValue + Default);
impl_container!([V, S] HashMap<String, V, S>, maps, imgui_hash_map_with_settings
    where V: ImGuiValue + Default + Clone + 'static, S: BuildHasher);
impl_container!([V] BTreeMap<String, V>, maps, imgui_btree_map_with_settings
    where V: ImGuiValue + Default + Clone + 'static);

impl<T: ImGuiValue + Default> ImGuiValue for Option<T> {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        Self::imgui_value_with_member(inspector, label, value, None)
    }

    fn imgui_value_with_member(
        inspector: &mut Inspector<'_, '_>,
        label: &str,
        value: &mut Self,
        member: Option<&MemberSettings>,
    ) -> bool {
        let ui = inspector.ui();
        let mut enabled = value.is_some();
        let changed = ui.checkbox(label, &mut enabled);
        if enabled {
            let inner = value.get_or_insert_with(T::default);
            let inner_label = format!("{label}##value");
            changed
                | ui.with_indent(|| {
                    T::imgui_value_with_member(inspector, &inner_label, inner, member)
                })
        } else {
            *value = None;
            changed
        }
    }
}

/// Renders tuple elements in the configured line, grid, or dropdown layout.
pub fn imgui_tuple_body(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    element_count: usize,
    settings: &TupleSettings,
    mut render_element: impl FnMut(&mut Inspector<'_, '_>, usize) -> bool,
) -> bool {
    const MAX_TABLE_COLUMNS: usize = 511;

    let ui = inspector.ui();
    let _id = ui.push_id(label);
    let _node = if settings.dropdown {
        let Some(node) = ui.tree_node(label) else {
            return false;
        };
        Some(node)
    } else {
        ui.text(label);
        if settings.same_line && element_count != 0 {
            ui.same_line();
        }
        None
    };

    let mut render = |index: usize| {
        let _path = inspector
            .tracks_nested_paths()
            .then(|| inspector.push_path(format!("[{index}]")));
        render_element(inspector, index)
    };
    let mut changed = false;

    match settings.render_mode {
        TupleRenderMode::Line => {
            for index in 0..element_count {
                if index > 0 {
                    ui.same_line();
                }
                changed |= render(index);
            }
        }
        TupleRenderMode::Grid if element_count != 0 => {
            let columns = settings
                .columns
                .clamp(1, element_count.min(MAX_TABLE_COLUMNS));
            if let Some(_table) = ui.begin_table("##tuple_table", columns) {
                if let Some(min_width) = settings.min_width {
                    for _ in 0..columns {
                        ui.table_setup_column_fixed_width(
                            "",
                            imgui::TableColumnFlags::NONE,
                            min_width,
                        );
                    }
                }
                for index in 0..element_count {
                    ui.table_next_column();
                    changed |= render(index);
                }
            }
        }
        TupleRenderMode::Grid => {}
    }
    changed
}

impl ImGuiValue for () {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, _value: &mut Self) -> bool {
        inspector.ui().text(label);
        false
    }
}

macro_rules! impl_tuple {
    ($($ty:ident : $index:tt),+) => {
        impl<$($ty: ImGuiValue),+> ImGuiValue for ($($ty,)+) {
            fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
                Self::imgui_value_with_member(inspector, label, value, None)
            }

            fn imgui_value_with_member(
                inspector: &mut Inspector<'_, '_>,
                label: &str,
                value: &mut Self,
                member: Option<&MemberSettings>,
            ) -> bool {
                let settings = member
                    .and_then(|member| member.tuples.as_ref())
                    .unwrap_or_else(|| inspector.settings().tuples());
                let len = [$($index),+].len();
                imgui_tuple_body(inspector, label, len, settings, |inspector, index| match index {
                    $($index => inspector.render_value(concat!("##", stringify!($index)), &mut value.$index),)+
                    _ => false,
                })
            }
        }
    };
    ([$($done:tt)*] $ty:ident : $index:tt $(, $next:ident : $next_index:tt)*) => {
        impl_tuple!($($done)* $ty : $index);
        impl_tuple!([$($done)* $ty : $index,] $($next : $next_index),*);
    };
    ([$($done:tt)*]) => {};
}

impl_tuple!(
    [] A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7,
    I: 8, J: 9, K: 10, L: 11, M: 12, N: 13, O: 14, P: 15
);
