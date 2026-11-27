use std::ptr;

use crate::Ui;
use crate::internal::DataTypeKind;
use crate::sys;

use super::{Drag, DragRange};

impl Ui {
    /// Creates a new drag slider widget. Returns true if the value has been edited.
    pub fn drag<T: AsRef<str>, K: DataTypeKind>(&self, label: T, value: &mut K) -> bool {
        self.drag_config(label).build(value)
    }

    /// Creates a new unbuilt Drag.
    pub fn drag_config<T: AsRef<str>, K: DataTypeKind>(&self, label: T) -> Drag<'_, K, T> {
        Drag::new(self, label)
    }

    /// Creates a drag float slider.
    #[doc(alias = "DragFloat")]
    pub fn drag_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        self.drag(label, value)
    }

    /// Creates a drag float slider builder.
    #[doc(alias = "DragFloat")]
    pub fn drag_float_config<L: AsRef<str>>(&self, label: L) -> Drag<'_, f32, L> {
        self.drag_config(label)
    }

    /// Creates a drag integer slider.
    #[doc(alias = "DragInt")]
    pub fn drag_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        self.drag(label, value)
    }

    /// Creates a drag integer slider builder.
    #[doc(alias = "DragInt")]
    pub fn drag_int_config<L: AsRef<str>>(&self, label: L) -> Drag<'_, i32, L> {
        self.drag_config(label)
    }

    /// Creates a drag float range slider.
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2(&self, label: impl AsRef<str>, min: &mut f32, max: &mut f32) -> bool {
        self.drag_float_range2_config(label).build(min, max)
    }

    /// Creates a drag float range slider builder.
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2_config<L: AsRef<str>>(&self, label: L) -> DragRange<'_, f32, L> {
        DragRange::new(self, label)
    }

    /// Creates a drag integer range slider.
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2(&self, label: impl AsRef<str>, min: &mut i32, max: &mut i32) -> bool {
        self.drag_int_range2_config(label).build(min, max)
    }

    /// Creates a drag integer range slider builder.
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2_config<L: AsRef<str>>(&self, label: L) -> DragRange<'_, i32, L> {
        DragRange::new(self, label)
    }

    /// Creates a drag float2 slider (2 floats)
    #[doc(alias = "DragFloat2")]
    pub fn drag_float2(&self, label: impl AsRef<str>, values: &mut [f32; 2]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
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
        self.with_scratch_txt(label, |label_cstr| unsafe {
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
        self.with_scratch_txt(label, |label_cstr| unsafe {
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
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt2(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int3 slider (3 ints)
    #[doc(alias = "DragInt3")]
    pub fn drag_int3(&self, label: impl AsRef<str>, values: &mut [i32; 3]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt3(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int4 slider (4 ints)
    #[doc(alias = "DragInt4")]
    pub fn drag_int4(&self, label: impl AsRef<str>, values: &mut [i32; 4]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt4(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }
}
