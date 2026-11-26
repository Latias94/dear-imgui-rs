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
                    changed |= row_changed;
                }
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
