//! `ImGuiValue` implementations for common Rust types.
//!
//! This module contains the default widget behavior for primitive scalars,
//! strings, options, container types and tuple-like values. Container editors
//! delegate to helpers in the `containers` module, which centralize shared UI
//! patterns and response event emission.

use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use crate::{
    ImGuiValue, Inspector, TupleRenderMode, TupleSettings, imgui, imgui_array_with_settings,
    imgui_btree_map_with_settings, imgui_hash_map_with_settings, imgui_vec_with_settings,
};

// Primitive ImGuiValue implementations

/// ImGui editor for a `bool` using a checkbox.
impl ImGuiValue for bool {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.checkbox(label, value)
    }
}

/// ImGui editor for a 32-bit signed integer.
impl ImGuiValue for i32 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_int(label, value)
    }
}

/// ImGui editor for a 32-bit float.
impl ImGuiValue for f32 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_float(label, value)
    }
}

/// ImGui editor for a 64-bit float.
impl ImGuiValue for f64 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_double(label, value)
    }
}

/// ImGui editor for an owned UTF-8 string.
impl ImGuiValue for String {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_text(label, value).build()
    }
}

/// ImGui editor for an ImString buffer (zero-copy).
impl ImGuiValue for imgui::ImString {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_text_imstr(label, value).build()
    }
}

// Integer scalar types via InputScalar

impl ImGuiValue for i8 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u8 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for i16 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u16 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u32 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for i64 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for u64 {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for isize {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

impl ImGuiValue for usize {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        ui.input_scalar(label, value).build()
    }
}

// Small fixed-size arrays treated as containers (with optional reordering).

impl ImGuiValue for [f32; 2] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

impl ImGuiValue for [f32; 3] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

impl ImGuiValue for [f32; 4] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

impl ImGuiValue for [i32; 2] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

impl ImGuiValue for [i32; 3] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

impl ImGuiValue for [i32; 4] {
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().arrays();
        imgui_array_with_settings(inspector, label, value, settings)
    }
}

// Basic map views for string-keyed maps: edit values in-place, keys displayed
// as labels, with simple insertion/removal helpers controlled by MapSettings.

impl<V, S> ImGuiValue for HashMap<String, V, S>
where
    V: ImGuiValue + Default + Clone + 'static,
    S: BuildHasher,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, map: &mut Self) -> bool {
        let settings = inspector.settings().maps();
        imgui_hash_map_with_settings(inspector, label, map, settings)
    }
}

impl<V> ImGuiValue for BTreeMap<String, V>
where
    V: ImGuiValue + Default + Clone + 'static,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, map: &mut Self) -> bool {
        let settings = inspector.settings().maps();
        imgui_btree_map_with_settings(inspector, label, map, settings)
    }
}

// Optional values rendered as a checkbox plus nested editor when enabled.
impl<T> ImGuiValue for Option<T>
where
    T: ImGuiValue + Default,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let ui = inspector.ui();
        let mut enabled = value.is_some();
        let mut changed = ui.checkbox(label, &mut enabled);

        match (enabled, value.as_mut()) {
            (true, Some(inner)) => {
                let _indent = ui.begin_indent();
                let inner_label = format!("{label}##value");
                changed |= T::imgui_value(inspector, &inner_label, inner);
            }
            (true, None) => {
                *value = Some(T::default());
                changed = true;
                if let Some(inner) = value.as_mut() {
                    let _indent = ui.begin_indent();
                    let inner_label = format!("{label}##value");
                    changed |= T::imgui_value(inspector, &inner_label, inner);
                }
            }
            (false, Some(_)) => {
                *value = None;
                changed = true;
            }
            (false, None) => {}
        }

        changed
    }
}

/// Tuple and pair-style values rendered in line or grid mode.
///
/// This helper is used both by the built-in `ImGuiValue` implementations for
/// small tuples and by the derive macro for struct fields that contain tuple
/// types, allowing consistent layout behavior across both paths.
pub fn imgui_tuple_body<F>(
    inspector: &mut Inspector<'_, '_>,
    label: &str,
    element_count: usize,
    settings: &TupleSettings,
    mut render_element: F,
) -> bool
where
    F: FnMut(&mut Inspector<'_, '_>, usize) -> bool,
{
    let ui = inspector.ui();
    let mut changed = false;

    let mut render_inner = |ui: &imgui::Ui, changed: &mut bool| match settings.render_mode {
        TupleRenderMode::Line => {
            let _id = ui.push_id(label);
            for index in 0..element_count {
                if index > 0 {
                    ui.same_line();
                }
                let local_changed = if inspector.is_path_active() {
                    let segment = format!("[{index}]");
                    let _path = inspector.push_path(segment);
                    render_element(inspector, index)
                } else {
                    render_element(inspector, index)
                };
                *changed |= local_changed;
            }
        }
        TupleRenderMode::Grid => {
            let columns = settings.columns.max(1).min(element_count.max(1));
            let table_id = format!("##tuple_table_{label}");

            if let Some(_table) = ui.begin_table(&table_id, columns) {
                if let Some(min_width) = settings.min_width {
                    for _ in 0..columns {
                        ui.table_setup_column_fixed_width(
                            "",
                            imgui::TableColumnFlags::NONE,
                            min_width,
                            None,
                        );
                    }
                }

                for index in 0..element_count {
                    ui.table_next_column();
                    let _id = ui.push_id(index as i32);
                    let local_changed = if inspector.is_path_active() {
                        let segment = format!("[{index}]");
                        let _path = inspector.push_path(segment);
                        render_element(inspector, index)
                    } else {
                        render_element(inspector, index)
                    };
                    *changed |= local_changed;
                }
            }
        }
    };

    if settings.dropdown {
        let _id = ui.push_id(label);
        if let Some(_node) = ui.tree_node(label) {
            render_inner(ui, &mut changed);
        }
    } else {
        // Outer label placement for non-dropdown tuples.
        ui.text(label);
        if settings.same_line {
            ui.same_line();
        }
        render_inner(ui, &mut changed);
    }

    changed
}

impl<A, B> ImGuiValue for (A, B)
where
    A: ImGuiValue,
    B: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b) = *value;
        imgui_tuple_body(
            inspector,
            label,
            2,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                _ => false,
            },
        )
    }
}

impl<A, B, C> ImGuiValue for (A, B, C)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b, ref mut c) = *value;
        imgui_tuple_body(
            inspector,
            label,
            3,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                _ => false,
            },
        )
    }
}

impl<A, B, C, D> ImGuiValue for (A, B, C, D)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d) = *value;
        imgui_tuple_body(
            inspector,
            label,
            4,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                3 => D::imgui_value(inspector, "##3", d),
                _ => false,
            },
        )
    }
}

impl<A, B, C, D, E> ImGuiValue for (A, B, C, D, E)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e) = *value;
        imgui_tuple_body(
            inspector,
            label,
            5,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                3 => D::imgui_value(inspector, "##3", d),
                4 => E::imgui_value(inspector, "##4", e),
                _ => false,
            },
        )
    }
}

impl<A, B, C, D, E, F> ImGuiValue for (A, B, C, D, E, F)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e, ref mut f) = *value;
        imgui_tuple_body(
            inspector,
            label,
            6,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                3 => D::imgui_value(inspector, "##3", d),
                4 => E::imgui_value(inspector, "##4", e),
                5 => F::imgui_value(inspector, "##5", f),
                _ => false,
            },
        )
    }
}

impl<A, B, C, D, E, F, G> ImGuiValue for (A, B, C, D, E, F, G)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
    G: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (ref mut a, ref mut b, ref mut c, ref mut d, ref mut e, ref mut f, ref mut g) = *value;
        imgui_tuple_body(
            inspector,
            label,
            7,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                3 => D::imgui_value(inspector, "##3", d),
                4 => E::imgui_value(inspector, "##4", e),
                5 => F::imgui_value(inspector, "##5", f),
                6 => G::imgui_value(inspector, "##6", g),
                _ => false,
            },
        )
    }
}

impl<A, B, C, D, E, F, G, H> ImGuiValue for (A, B, C, D, E, F, G, H)
where
    A: ImGuiValue,
    B: ImGuiValue,
    C: ImGuiValue,
    D: ImGuiValue,
    E: ImGuiValue,
    F: ImGuiValue,
    G: ImGuiValue,
    H: ImGuiValue,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let tuple_settings = inspector.settings().tuples();

        let (
            ref mut a,
            ref mut b,
            ref mut c,
            ref mut d,
            ref mut e,
            ref mut f,
            ref mut g,
            ref mut h,
        ) = *value;
        imgui_tuple_body(
            inspector,
            label,
            8,
            tuple_settings,
            |inspector, index| match index {
                0 => A::imgui_value(inspector, "##0", a),
                1 => B::imgui_value(inspector, "##1", b),
                2 => C::imgui_value(inspector, "##2", c),
                3 => D::imgui_value(inspector, "##3", d),
                4 => E::imgui_value(inspector, "##4", e),
                5 => F::imgui_value(inspector, "##5", f),
                6 => G::imgui_value(inspector, "##6", g),
                7 => H::imgui_value(inspector, "##7", h),
                _ => false,
            },
        )
    }
}

// Editable vectors with basic insertion/removal and drag-to-reorder support.
impl<T> ImGuiValue for Vec<T>
where
    T: ImGuiValue + Default,
{
    fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
        let settings = inspector.settings().vec();
        imgui_vec_with_settings(inspector, label, value, settings)
    }
}

// Optional math crate integrations

/// ImGui editors for `glam` vector types when the `glam` feature is enabled.
#[cfg(feature = "glam")]
mod glam_impls {
    use crate::{ImGuiValue, Inspector};
    use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

    impl ImGuiValue for Vec2 {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = value.to_array();
            let changed = ui.input_float2(label, &mut arr).build();
            if changed {
                *value = Vec2::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Vec3 {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = value.to_array();
            let changed = ui.input_float3(label, &mut arr).build();
            if changed {
                *value = Vec3::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Vec4 {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = value.to_array();
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                *value = Vec4::from_array(arr);
            }
            changed
        }
    }

    impl ImGuiValue for Quat {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            // Represent the quaternion as (x, y, z, w) and allow direct editing.
            // After editing, renormalize to keep it as a unit quaternion.
            let mut arr = value.to_array();
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                let mut q = Quat::from_xyzw(arr[0], arr[1], arr[2], arr[3]);
                // Avoid NaNs from zero-length quaternions; fall back to identity.
                if q.length_squared() > 0.0 {
                    q = q.normalize();
                } else {
                    q = Quat::IDENTITY;
                }
                *value = q;
            }
            changed
        }
    }

    impl ImGuiValue for Mat4 {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            // Render the 4x4 matrix as four rows of input_float4 widgets.
            // This is primarily intended for debugging/inspection.
            let mut cols = value.to_cols_array();
            let mut changed = false;

            // ImGui uses row-major visual layout; convert 16-element column-major
            // storage into four row slices for editing.
            for row in 0..4 {
                let mut row_vals = [cols[row], cols[4 + row], cols[8 + row], cols[12 + row]];
                let row_label = format!("{label} [{row}]");
                let row_changed = ui.input_float4(&row_label, &mut row_vals).build();
                if row_changed {
                    cols[row] = row_vals[0];
                    cols[4 + row] = row_vals[1];
                    cols[8 + row] = row_vals[2];
                    cols[12 + row] = row_vals[3];
                }
                changed |= row_changed;
            }

            if changed {
                *value = Mat4::from_cols_array(&cols);
            }
            changed
        }
    }
}

/// ImGui editors for `mint` vector types when the `mint` feature is enabled.
#[cfg(feature = "mint")]
mod mint_impls {
    use crate::{ImGuiValue, Inspector};
    use mint::{Vector2, Vector3, Vector4};

    impl ImGuiValue for Vector2<f32> {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = [value.x, value.y];
            let changed = ui.input_float2(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
            }
            changed
        }
    }

    impl ImGuiValue for Vector3<f32> {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = [value.x, value.y, value.z];
            let changed = ui.input_float3(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
                value.z = arr[2];
            }
            changed
        }
    }

    impl ImGuiValue for Vector4<f32> {
        fn imgui_value(inspector: &mut Inspector<'_, '_>, label: &str, value: &mut Self) -> bool {
            let ui = inspector.ui();
            let mut arr = [value.x, value.y, value.z, value.w];
            let changed = ui.input_float4(label, &mut arr).build();
            if changed {
                value.x = arr[0];
                value.y = arr[1];
                value.z = arr[2];
                value.w = arr[3];
            }
            changed
        }
    }
}
