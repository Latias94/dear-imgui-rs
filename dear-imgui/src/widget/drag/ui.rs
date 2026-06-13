use std::ptr;

use crate::Ui;
use crate::internal::DataTypeKind;
use crate::sys;

use super::Drag;

impl Ui {
    /// Creates a new drag slider widget. Returns true if the value has been edited.
    pub fn drag<T: AsRef<str>, K: DataTypeKind>(&self, label: T, value: &mut K) -> bool {
        Drag::new(label).build(self, value)
    }

    /// Creates a new unbuilt Drag.
    pub fn drag_config<T: AsRef<str>, K: DataTypeKind>(&self, label: T) -> Drag<K, T> {
        Drag::new(label)
    }

    /// Creates a drag float2 slider (2 floats)
    #[doc(alias = "DragFloat2")]
    pub fn drag_float2(&self, label: impl AsRef<str>, values: &mut [f32; 2]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat2(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag float3 slider (3 floats)
    #[doc(alias = "DragFloat3")]
    pub fn drag_float3(&self, label: impl AsRef<str>, values: &mut [f32; 3]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat3(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag float4 slider (4 floats)
    #[doc(alias = "DragFloat4")]
    pub fn drag_float4(&self, label: impl AsRef<str>, values: &mut [f32; 4]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat4(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag int2 slider (2 ints)
    #[doc(alias = "DragInt2")]
    pub fn drag_int2(&self, label: impl AsRef<str>, values: &mut [i32; 2]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt2(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int3 slider (3 ints)
    #[doc(alias = "DragInt3")]
    pub fn drag_int3(&self, label: impl AsRef<str>, values: &mut [i32; 3]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt3(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int4 slider (4 ints)
    #[doc(alias = "DragInt4")]
    pub fn drag_int4(&self, label: impl AsRef<str>, values: &mut [i32; 4]) -> bool {
        self.run_with_bound_context(|| unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt4(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }
}
